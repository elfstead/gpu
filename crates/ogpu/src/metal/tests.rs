//! Native acceptance tests. Run explicitly on Apple Silicon; never silently skip.
use super::*;
use crate::SUCCESS;

fn device() -> OgpuDevice {
    let raw = MetalDevice::system_default().expect("Apple Silicon GPU required");
    assert!(supported(&raw));
    OgpuDevice {
        inner: make_device(raw).unwrap(),
    }
}

unsafe fn buffer(device: &mut OgpuDevice, size: u64, placement: u32) -> *mut OgpuBuffer {
    let mut buffer = ptr::null_mut();
    assert_eq!(
        unsafe { ogpu_buffer_create(device, size, placement, &mut buffer, ptr::null_mut()) },
        SUCCESS
    );
    buffer
}

fn native_roundtrip(code: &[u8], format: u32) {
    crate::boundary::native_scope(|| unsafe {
        let mut device = device();
        let output = buffer(&mut device, 32, 0);
        let desc = OgpuShaderDesc {
            code: code.as_ptr().cast(),
            code_size: code.len() as u64,
            entry_point: c"fill".as_ptr(),
            constants: ptr::null(),
            constant_count: 0,
            format,
            local_size: [4, 1, 1],
            reserved: 0,
        };
        let mut kernel = ptr::null_mut();
        let mut error = OgpuError {
            vulkan_result: 0,
            message: [0; 256],
        };
        for bad in [1, 4100] {
            assert_eq!(
                ogpu_kernel_create(&mut device, &desc, bad, &mut kernel, &mut error),
                INVALID_ARGUMENT
            );
            assert!(kernel.is_null());
        }
        let status = ogpu_kernel_create(&mut device, &desc, 16, &mut kernel, &mut error);
        assert_eq!(
            status,
            SUCCESS,
            "{}",
            std::ffi::CStr::from_ptr(error.message.as_ptr()).to_string_lossy()
        );
        let mut root = [0u64, 7];
        assert_eq!(
            ogpu_buffer_device_address(output, &mut root[0], &mut error),
            SUCCESS
        );
        let mut batch = ptr::null_mut();
        assert_eq!(
            ogpu_batch_create(&mut device, &mut batch, &mut error),
            SUCCESS
        );
        assert_eq!(ogpu_batch_retain_buffer(batch, output, &mut error), SUCCESS);
        assert_eq!(
            ogpu_batch_dispatch(batch, kernel, 2, 1, 1, root.as_ptr().cast(), 16, &mut error),
            SUCCESS
        );
        root[1] = 99; // Recording must have copied these bytes.
        std::hint::black_box(root);
        ogpu_kernel_destroy(kernel);
        let mut done = ptr::null_mut();
        assert_eq!(ogpu_batch_submit(batch, &mut done, &mut error), SUCCESS);
        let mut duplicate = ptr::dangling_mut();
        assert_eq!(
            ogpu_batch_submit(batch, &mut duplicate, &mut error),
            INVALID_ARGUMENT
        );
        assert!(duplicate.is_null());
        ogpu_batch_destroy(batch);
        assert_eq!(ogpu_completion_wait(done, &mut error), SUCCESS);
        let mut data = [0u32; 8];
        assert_eq!(
            ogpu_buffer_read(output, 0, data.as_mut_ptr().cast(), 32, &mut error),
            SUCCESS
        );
        assert_eq!(data, [7, 8, 9, 10, 11, 12, 13, 14]);
        ogpu_buffer_destroy(output);
        assert!(
            device.inner.buffers.borrow().is_empty(),
            "Observed receipt must not retain allocations"
        );
        let mut ready = 0;
        assert_eq!(ogpu_completion_poll(done, &mut ready, &mut error), SUCCESS);
        assert_eq!(ready, 1);
        assert_eq!(ogpu_completion_wait(done, &mut error), SUCCESS);
        ogpu_completion_destroy(done);
    });
}

#[test]
#[ignore = "requires Apple Silicon Metal GPU"]
fn native_msl_without_translation() {
    native_roundtrip(include_bytes!("native-test.metal"), crate::SHADER_MSL);
}

#[test]
#[ignore = "requires Apple Silicon Metal GPU and xcrun Metal compiler"]
fn native_metallib_without_translation() {
    let directory = std::env::temp_dir().join(format!("ogpu-metal-test-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let source = directory.join("test.metal");
    let library = directory.join("test.metallib");
    let air = directory.join("test.air");
    std::fs::write(&source, include_bytes!("native-test.metal")).unwrap();
    let output = std::process::Command::new("xcrun")
        .args(["-sdk", "macosx", "metal", "-std=metal3.0", "-c"])
        .arg(&source)
        .arg("-o")
        .arg(&air)
        .output()
        .expect("xcrun Metal compiler required");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new("xcrun")
        .args(["-sdk", "macosx", "metallib"])
        .arg(&air)
        .arg("-o")
        .arg(&library)
        .output()
        .expect("xcrun metallib required");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    native_roundtrip(&std::fs::read(&library).unwrap(), crate::SHADER_METALLIB);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires Apple Silicon Metal GPU"]
fn byte_copies_retirement_and_registry() {
    crate::boundary::native_scope(|| unsafe {
        let mut device = device();
        let mut invalid = ptr::dangling_mut();
        let too_large = device.inner.raw.max_buffer_length() + 1;
        assert_eq!(
            ogpu_buffer_create(&mut device, too_large, 0, &mut invalid, ptr::null_mut()),
            OUT_OF_RANGE
        );
        assert!(invalid.is_null());
        for _ in 0..64 {
            ogpu_buffer_destroy(buffer(&mut device, 16, 0));
        }
        assert!(device.inner.buffers.borrow().is_empty());
        let source = buffer(&mut device, 32, 0);
        let private = buffer(&mut device, 32, 1);
        let output = buffer(&mut device, 32, 0);
        let bytes: Vec<u8> = (0..32).collect();
        assert_eq!(
            ogpu_buffer_write(source, 0, bytes.as_ptr().cast(), 32, ptr::null_mut()),
            SUCCESS
        );
        let mut batch = ptr::null_mut();
        assert_eq!(
            ogpu_batch_create(&mut device, &mut batch, ptr::null_mut()),
            SUCCESS
        );
        assert_eq!(
            ogpu_batch_copy_buffer(batch, source, 32, source, 32, 0, ptr::null_mut()),
            SUCCESS
        );
        assert!((*batch).inner.retained.is_empty());
        assert_eq!(
            ogpu_batch_copy_buffer(batch, source, 1, source, 2, 5, ptr::null_mut()),
            INVALID_ARGUMENT
        );
        assert_eq!(
            ogpu_batch_barrier(batch, 4, 2, ptr::null_mut()),
            UNSUPPORTED
        );
        assert_eq!(
            ogpu_batch_copy_buffer(batch, source, 0, private, 0, 32, ptr::null_mut()),
            SUCCESS
        );
        assert_eq!(ogpu_batch_barrier(batch, 64, 32, ptr::null_mut()), SUCCESS);
        assert_eq!(
            ogpu_batch_copy_buffer(batch, private, 1, private, 17, 7, ptr::null_mut()),
            SUCCESS
        );
        assert_eq!(ogpu_batch_barrier(batch, 64, 32, ptr::null_mut()), SUCCESS);
        assert_eq!(
            ogpu_batch_copy_buffer(batch, private, 0, output, 0, 32, ptr::null_mut()),
            SUCCESS
        );
        let weak = Rc::downgrade(&(*private).inner);
        ogpu_buffer_destroy(private);
        ogpu_buffer_destroy(source);
        let mut done = ptr::null_mut();
        assert_eq!(
            ogpu_batch_submit(batch, &mut done, ptr::null_mut()),
            SUCCESS
        );
        ogpu_batch_destroy(batch);
        assert!(weak.upgrade().is_some());
        assert_eq!(ogpu_completion_wait(done, ptr::null_mut()), SUCCESS);
        assert!(weak.upgrade().is_none());
        let mut data = [0u8; 32];
        assert_eq!(
            ogpu_buffer_read(output, 0, data.as_mut_ptr().cast(), 32, ptr::null_mut()),
            SUCCESS
        );
        let mut expected = bytes;
        expected.copy_within(1..8, 17);
        assert_eq!(data.as_slice(), expected);
        // Inject a cached failure only after real work has been drained. This
        // tests the C output contract without manufacturing unsafe device loss.
        (*done).inner.submission.outcome =
            Some(Err(fail(INTERNAL_ERROR, "injected terminal error")));
        let mut ready = 99;
        assert_eq!(
            ogpu_completion_poll(done, &mut ready, ptr::null_mut()),
            INTERNAL_ERROR
        );
        assert_eq!(ready, 0);
        assert_eq!(ogpu_completion_wait(done, ptr::null_mut()), INTERNAL_ERROR);
        ogpu_completion_destroy(done);
        ogpu_buffer_destroy(output);
        assert!(device.inner.buffers.borrow().is_empty());
    });
}
