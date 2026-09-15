use super::*;

#[test]
fn timestamp_clock_validation_and_wrap() {
    for bits in [0, 1, 35, 65, u32::MAX] {
        assert_eq!(timestamp_info(bits, 1.0).unwrap_err().status, UNSUPPORTED);
    }
    for period in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(timestamp_info(64, period).unwrap_err().status, UNSUPPORTED);
    }
    assert_eq!(timestamp_info(36, 2.5).unwrap(), (2.5, 36));
    assert_eq!(timestamp_info(64, 1.0).unwrap(), (1.0, 64));
    assert_eq!(timestamp_delta_ns(100, 100, 36, 2.5), 0.0);
    assert_eq!(timestamp_delta_ns(100, 104, 36, 2.5), 10.0);
    assert_eq!(timestamp_delta_ns((1 << 36) - 2, 3, 36, 2.5), 12.5);
    assert_eq!(timestamp_delta_ns(u64::MAX - 1, 3, 64, 2.0), 10.0);
    // The mathematical limit is documented: full extra wraps are not recoverable.
    assert_eq!(timestamp_delta_ns(7, 7, 64, 1.0), 0.0);
}

thread_local! {
    static REAL_QUERY: Cell<vk::PFN_vkGetQueryPoolResults> = const { Cell::new(None) };
    static QUERY_CALLS: Cell<u32> = const { Cell::new(0) };
    static QUERY_STATUS: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_SUCCESS) };
    static CREATE_CALLS: Cell<u32> = const { Cell::new(0) };
}

unsafe extern "C" fn fail_create(
    _device: vk::VkDevice,
    _info: *const vk::VkQueryPoolCreateInfo,
    _allocator: *const vk::VkAllocationCallbacks,
    _output: *mut vk::VkQueryPool,
) -> vk::VkResult {
    CREATE_CALLS.set(CREATE_CALLS.get() + 1);
    vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
}

unsafe extern "C" fn query(
    device: vk::VkDevice,
    pool: vk::VkQueryPool,
    first: u32,
    count: u32,
    size: usize,
    data: *mut std::ffi::c_void,
    stride: vk::VkDeviceSize,
    flags: vk::VkQueryResultFlags,
) -> vk::VkResult {
    QUERY_CALLS.set(QUERY_CALLS.get() + 1);
    assert_eq!(flags, vk::VkQueryResultFlagBits_VK_QUERY_RESULT_64_BIT);
    assert_eq!((first, count, size, stride), (0, 2, 16, 8));
    if QUERY_STATUS.get() != vk::VkResult_VK_SUCCESS {
        QUERY_STATUS.get()
    } else {
        unsafe {
            (REAL_QUERY.get().unwrap())(device, pool, first, count, size, data, stride, flags)
        }
    }
}

#[test]
#[ignore = "requires Vulkan; optional timing states, independent pools, and query failures"]
fn gpu_timing() {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut tested = 0;
    let mut timed_devices = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::new(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        tested += 1;

        // Declining timing never prevents ordinary execution and never allocates queries.
        let mut unsupported = Device::new(instance.clone(), physical).unwrap();
        let d = Rc::get_mut(&mut unsupported).unwrap();
        d.timestamp_bits = 0;
        d.f.vkCreateQueryPool = Some(fail_create);
        CREATE_CALLS.set(0);
        let mut plain = Batch::new(unsupported.clone()).unwrap();
        assert_eq!(plain.enable_timing().unwrap_err().status, UNSUPPORTED);
        assert!(!plain.timed && plain.steps.as_ref().unwrap().is_empty());
        let mut done = unsafe { plain.submit().unwrap() };
        done.wait().unwrap();
        assert_eq!(done.elapsed_ns().unwrap_err().status, INVALID_ARGUMENT);
        drop(done);
        assert_eq!(CREATE_CALLS.get(), 0);

        if device.timing_info().is_err() {
            continue;
        }
        timed_devices += 1;
        let kernel = Rc::new(unsafe { Kernel::new(device.clone(), &words, 16, &[]).unwrap() });
        let buffer = Buffer::new(device.clone(), 4).unwrap();
        buffer.write(0, &1u32.to_ne_bytes()).unwrap();
        let mut root = [0u8; 16];
        root[..8].copy_from_slice(&buffer.address().unwrap().to_ne_bytes());
        root[8..12].copy_from_slice(&1u32.to_ne_bytes());
        let mut completions = Vec::new();
        for _ in 0..2 {
            let mut batch = Batch::new(device.clone()).unwrap();
            batch.enable_timing().unwrap();
            batch.enable_timing().unwrap();
            batch
                .barrier(COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
                .unwrap();
            batch.dispatch(kernel.clone(), [1, 1, 1], &root).unwrap();
            let mut completion = unsafe { batch.submit().unwrap() };
            assert_eq!(batch.enable_timing().unwrap_err().status, INVALID_ARGUMENT);
            assert_eq!(
                completion.elapsed_ns().unwrap_err().status,
                INVALID_ARGUMENT
            );
            assert!(completion.submission.pending);
            completions.push(completion);
        }
        drop(kernel); // Recorded executables are retained; the raw-address buffer is not.
        assert_ne!(completions[0].queries, completions[1].queries);
        completions[1].wait().unwrap();
        assert_eq!(
            completions[0].elapsed_ns().unwrap_err().status,
            INVALID_ARGUMENT
        );
        for completion in &mut completions {
            completion.wait().unwrap();
            let elapsed = completion.elapsed_ns().unwrap();
            assert!(elapsed.is_finite() && elapsed >= 0.0);
            assert_eq!(completion.elapsed_ns().unwrap(), elapsed);
        }
        let mut value = 0u32;
        unsafe {
            buffer.read(0, (&mut value as *mut u32).cast(), 4).unwrap();
        }
        assert_eq!(value, 37);
        drop(completions);

        // Untimed batches never call query creation, even on a timestamp-capable device.
        let mut failing = Device::new(instance.clone(), physical).unwrap();
        Rc::get_mut(&mut failing).unwrap().f.vkCreateQueryPool = Some(fail_create);
        CREATE_CALLS.set(0);
        let mut discarded = Batch::new(failing.clone()).unwrap();
        discarded.enable_timing().unwrap();
        drop(discarded);
        assert_eq!(CREATE_CALLS.get(), 0, "Recording must not allocate queries");
        let mut batch = Batch::new(failing.clone()).unwrap();
        unsafe { batch.submit().unwrap() }.wait().unwrap();
        assert_eq!(CREATE_CALLS.get(), 0);
        drop(batch);
        let mut batch = Batch::new(failing.clone()).unwrap();
        batch.enable_timing().unwrap();
        assert!(
            matches!(unsafe { batch.submit() }, Err(e) if e.vk == vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY)
        );
        assert!(batch.steps.is_none());
        assert_eq!(CREATE_CALLS.get(), 1);
        drop(batch);
        assert_eq!(Rc::strong_count(&failing), 1);

        // Query failures cannot silently turn successful execution into a failed wait.
        let mut observed = Device::new(instance.clone(), physical).unwrap();
        let f = &mut Rc::get_mut(&mut observed).unwrap().f;
        REAL_QUERY.set(f.vkGetQueryPoolResults);
        f.vkGetQueryPoolResults = Some(query);
        QUERY_CALLS.set(0);
        let mut discarded = Batch::new(observed.clone()).unwrap();
        discarded.enable_timing().unwrap();
        drop(unsafe { discarded.submit().unwrap() }); // Pending destruction drains without reading timing.
        assert_eq!(QUERY_CALLS.get(), 0);
        drop(discarded);
        let mut batch = Batch::new(observed.clone()).unwrap();
        batch.enable_timing().unwrap();
        let mut done = unsafe { batch.submit().unwrap() };
        assert_eq!(done.elapsed_ns().unwrap_err().status, INVALID_ARGUMENT);
        assert_eq!(QUERY_CALLS.get(), 0);
        done.wait().unwrap();
        assert!(done.submission.resources.is_none());
        assert!(!done.queries.is_null());
        assert_eq!(QUERY_CALLS.get(), 0, "Retirement must not retrieve timing");
        for status in [
            vk::VkResult_VK_NOT_READY,
            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
        ] {
            QUERY_STATUS.set(status);
            assert_eq!(done.elapsed_ns().unwrap_err().vk, status);
            assert!(done.submission.resources.is_none() && !done.queries.is_null());
            done.wait().unwrap();
        }
        QUERY_STATUS.set(vk::VkResult_VK_SUCCESS);
        let elapsed = done.elapsed_ns().unwrap();
        assert!(elapsed.is_finite() && elapsed >= 0.0);
        assert!(
            done.queries.is_null(),
            "Cached duration no longer needs a query pool"
        );
        assert_eq!(done.elapsed_ns().unwrap(), elapsed);
        assert_eq!(QUERY_CALLS.get(), 3, "Successful timing must be cached");
        drop(done);
        drop(batch);

        let mut batch = Batch::new(observed.clone()).unwrap();
        batch.enable_timing().unwrap();
        let mut done = unsafe { batch.submit().unwrap() };
        done.wait().unwrap(); // Real completion before safely simulating loss on read.
        QUERY_STATUS.set(vk::VkResult_VK_ERROR_DEVICE_LOST);
        assert_eq!(
            done.elapsed_ns().unwrap_err().vk,
            vk::VkResult_VK_ERROR_DEVICE_LOST
        );
        let calls = QUERY_CALLS.get();
        assert!(observed.lost.get());
        assert!(done.elapsed_ns().is_err());
        assert_eq!(QUERY_CALLS.get(), calls);
        done.wait().unwrap(); // The original wait succeeded; its outcome is unchanged.
    }
    assert!(tested > 0, "No execution-capable Vulkan device found");
    println!("Verified optional timing on {timed_devices}/{tested} execution devices");
}
