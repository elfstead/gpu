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
        assert!((*batch).inner.recording.is_none());
        assert_eq!(ogpu_completion_wait(done, &mut error), SUCCESS);
        assert!((*done).inner.submission.resources.is_none());
        // Keep both consumed batch and receipt alive through terminal observation.
        assert!((*batch).inner.transient.is_empty());
        assert!((*batch).inner.tables.is_empty());
        assert!((*batch).inner.retained.is_empty());
        ogpu_batch_destroy(batch);
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

#[test]
#[ignore = "requires Apple Silicon Metal GPU"]
fn barriers_across_submissions() {
    crate::boundary::native_scope(|| unsafe {
        let mut device = device();
        // Barrier in producer only, consumer only, or a separate empty batch.
        // Cover every blit/dispatch copy pairing, with no intervening CPU wait.
        for placement in 0..3 {
            for producer_unaligned in [false, true] {
                for consumer_unaligned in [false, true] {
                    for iteration in 0..8u8 {
                        let source = buffer(&mut device, 4096, 0);
                        let private = buffer(&mut device, 4096, 1);
                        let output = buffer(&mut device, 4096, 0);
                        let bytes: Vec<u8> = (0..4096)
                            .map(|i| (i as u8).wrapping_add(iteration))
                            .collect();
                        assert_eq!(
                            ogpu_buffer_write(
                                source,
                                0,
                                bytes.as_ptr().cast(),
                                4096,
                                ptr::null_mut()
                            ),
                            SUCCESS
                        );
                        let mut producer = OgpuBatch {
                            inner: Batch::new(device.inner.clone()).unwrap(),
                        };
                        let mut consumer = OgpuBatch {
                            inner: Batch::new(device.inner.clone()).unwrap(),
                        };
                        let source_offset = u64::from(producer_unaligned);
                        let output_offset = u64::from(consumer_unaligned);
                        assert_eq!(
                            ogpu_batch_copy_buffer(
                                &mut producer,
                                source,
                                source_offset,
                                private,
                                0,
                                4092,
                                ptr::null_mut()
                            ),
                            SUCCESS
                        );
                        if placement == 0 {
                            assert_eq!(
                                ogpu_batch_barrier(&mut producer, 64, 32, ptr::null_mut()),
                                SUCCESS
                            );
                        }
                        let mut produced = producer.inner.submit().unwrap();
                        let mut middle = None;
                        if placement == 2 {
                            let mut barrier = OgpuBatch {
                                inner: Batch::new(device.inner.clone()).unwrap(),
                            };
                            assert_eq!(
                                ogpu_batch_barrier(&mut barrier, 64, 32, ptr::null_mut()),
                                SUCCESS
                            );
                            middle = Some(barrier.inner.submit().unwrap());
                        }
                        if placement == 1 {
                            assert_eq!(
                                ogpu_batch_barrier(&mut consumer, 64, 32, ptr::null_mut()),
                                SUCCESS
                            );
                        }
                        assert_eq!(
                            ogpu_batch_copy_buffer(
                                &mut consumer,
                                private,
                                0,
                                output,
                                output_offset,
                                4092,
                                ptr::null_mut()
                            ),
                            SUCCESS
                        );
                        let mut consumed = consumer.inner.submit().unwrap();
                        consumed.observe(true).unwrap();
                        let mut actual = vec![0u8; 4092];
                        assert_eq!(
                            ogpu_buffer_read(
                                output,
                                output_offset,
                                actual.as_mut_ptr().cast(),
                                4092,
                                ptr::null_mut()
                            ),
                            SUCCESS
                        );
                        assert_eq!(actual, bytes[source_offset as usize..source_offset as usize + 4092],
                            "placement={placement} producer_unaligned={producer_unaligned} consumer_unaligned={consumer_unaligned}");
                        produced.observe(true).unwrap();
                        if let Some(done) = middle.as_mut() {
                            done.observe(true).unwrap();
                        }
                        ogpu_buffer_destroy(source);
                        ogpu_buffer_destroy(private);
                        ogpu_buffer_destroy(output);
                        assert!(device.inner.buffers.borrow().is_empty());
                    }
                }
            }
        }
    });
}

#[test]
#[ignore = "requires Apple Silicon Metal GPU"]
fn nullable_public_and_internal_allocations() {
    crate::boundary::native_scope(|| unsafe {
        let mut device = device();
        let mut output = ptr::dangling_mut();
        FAIL_NEXT_BUFFER_ALLOCATION.with(|flag| flag.set(true));
        assert_eq!(
            ogpu_buffer_create(&mut device, 16, 0, &mut output, ptr::null_mut()),
            INTERNAL_ERROR
        );
        assert!(output.is_null());
        let mut batch = Batch::new(device.inner.clone()).unwrap();
        for bytes in [&[][..], &[1, 2, 3, 4][..]] {
            FAIL_NEXT_BUFFER_ALLOCATION.with(|flag| flag.set(true));
            assert_eq!(batch.upload(bytes).unwrap_err().status, INTERNAL_ERROR);
            assert!(batch.transient.is_empty());
            assert!(batch.recording.is_some());
        }
        // Failure leaves the recording usable and does not add a nil allocation
        // to residency. A following real upload/submit still succeeds.
        batch.upload(&[1, 2, 3, 4]).unwrap();
        batch.submit().unwrap().observe(true).unwrap();
    });
}

#[test]
#[ignore = "requires Apple Silicon Metal GPU"]
fn feedback_pending_failure_and_retirement() {
    crate::boundary::native_scope(|| unsafe {
        let mut device = device();
        let source = buffer(&mut device, 16, 0);
        let output = buffer(&mut device, 16, 0);
        let weak = Rc::downgrade(&(*source).inner);
        let mut batch = OgpuBatch {
            inner: Batch::new(device.inner.clone()).unwrap(),
        };
        assert_eq!(
            ogpu_batch_copy_buffer(&mut batch, source, 1, output, 1, 7, ptr::null_mut()),
            SUCCESS
        );
        ogpu_buffer_destroy(source);
        // A CPU-controlled gate makes a pending poll deterministic.
        let gate = device.inner.raw.new_shared_event();
        let _: () = msg_send![device.inner.queue.0, waitForEvent: gate.as_ptr() value: 1u64];
        let mut done = OgpuCompletion {
            inner: batch.inner.submit().unwrap(),
        };
        let mut ready = 99;
        let pending_status = ogpu_completion_poll(&mut done, &mut ready, ptr::null_mut());
        // Unblock before assertions, so a regression cannot strand Drop waiting.
        gate.set_signaled_value(1);
        assert_eq!(pending_status, SUCCESS);
        assert_eq!(ready, 0);
        assert!(done.inner.submission.resources.is_some());
        assert!(weak.upgrade().is_some());
        // Wait for REAL native feedback before substituting its result. Unlike
        // the cached-outcome test, this exercises failure retirement itself.
        done.inner.feedback.observe(true).unwrap().unwrap();
        done.inner.feedback.inject_drained_error();
        assert_eq!(
            ogpu_completion_poll(&mut done, &mut ready, ptr::null_mut()),
            INTERNAL_ERROR
        );
        assert_eq!(ready, 0);
        assert!(done.inner.submission.resources.is_none());
        assert!(batch.inner.recording.is_none());
        assert!(weak.upgrade().is_none());
        assert_eq!(
            ogpu_completion_wait(&mut done, ptr::null_mut()),
            INTERNAL_ERROR
        );
        ogpu_buffer_destroy(output);
        assert!(device.inner.buffers.borrow().is_empty());
    });
}
