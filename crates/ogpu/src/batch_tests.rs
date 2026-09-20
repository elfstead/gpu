use super::*;

thread_local! {
    static DESTROY_COUNTS: Cell<[u32; 4]> = const { Cell::new([0; 4]) };
    static POOL_DESTROY: Cell<vk::PFN_vkDestroyCommandPool> = const { Cell::new(None) };
    static BUFFER_DESTROY: Cell<vk::PFN_vkDestroyBuffer> = const { Cell::new(None) };
    static PIPELINE_DESTROY: Cell<vk::PFN_vkDestroyPipeline> = const { Cell::new(None) };
    static QUERY_DESTROY: Cell<vk::PFN_vkDestroyQueryPool> = const { Cell::new(None) };
    static POOL_RESET: Cell<vk::PFN_vkResetCommandPool> = const { Cell::new(None) };
    static RESET_COUNT: Cell<u32> = const { Cell::new(0) };
}

macro_rules! counted_destroy {
    ($name:ident, $real:ident, $ty:ty, $index:expr) => {
        unsafe extern "C" fn $name(d: vk::VkDevice, h: $ty, a: *const vk::VkAllocationCallbacks) {
            let mut counts = DESTROY_COUNTS.get();
            if $index != 0 {
                assert_eq!(
                    counts[0] + RESET_COUNT.get(),
                    1,
                    "Native references must be invalidated first"
                );
            }
            assert_eq!(counts[$index], 0, "Destruction must happen exactly once");
            unsafe {
                ($real.get().unwrap())(d, h, a);
            }
            counts[$index] += 1;
            DESTROY_COUNTS.set(counts);
        }
    };
}
counted_destroy!(destroy_pool, POOL_DESTROY, vk::VkCommandPool, 0);
counted_destroy!(destroy_buffer, BUFFER_DESTROY, vk::VkBuffer, 1);
counted_destroy!(destroy_pipeline, PIPELINE_DESTROY, vk::VkPipeline, 2);
counted_destroy!(destroy_queries, QUERY_DESTROY, vk::VkQueryPool, 3);

unsafe extern "C" fn reset_pool(
    d: vk::VkDevice,
    p: vk::VkCommandPool,
    flags: vk::VkCommandPoolResetFlags,
) -> vk::VkResult {
    assert_eq!(
        DESTROY_COUNTS.get(),
        [0; 4],
        "Reset before releasing resources"
    );
    let status = unsafe { (POOL_RESET.get().unwrap())(d, p, flags) };
    assert_eq!(status, vk::VkResult_VK_SUCCESS);
    RESET_COUNT.set(RESET_COUNT.get() + 1);
    status
}

#[test]
#[ignore = "requires Vulkan; receipt lifetime, exact destruction order and lazy timing"]
fn gpu_completion_receipts() {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        for timed in [false, true] {
            for mode in 0..3 {
                let mut device = match Device::new(instance.clone(), physical) {
                    Ok(d) => d,
                    Err(e) if e.status == UNSUPPORTED => continue,
                    Err(e) => panic!("{e:?}"),
                };
                if timed && device.timing_info().is_err() {
                    continue;
                }
                let f = &mut Rc::get_mut(&mut device).unwrap().f;
                POOL_DESTROY.set(f.vkDestroyCommandPool);
                BUFFER_DESTROY.set(f.vkDestroyBuffer);
                PIPELINE_DESTROY.set(f.vkDestroyPipeline);
                QUERY_DESTROY.set(f.vkDestroyQueryPool);
                POOL_RESET.set(f.vkResetCommandPool);
                f.vkDestroyCommandPool = Some(destroy_pool);
                f.vkDestroyBuffer = Some(destroy_buffer);
                f.vkDestroyPipeline = Some(destroy_pipeline);
                f.vkDestroyQueryPool = Some(destroy_queries);
                f.vkResetCommandPool = Some(reset_pool);
                DESTROY_COUNTS.set([0; 4]);
                RESET_COUNT.set(0);
                let buffer = Rc::new(Buffer::new(device.clone(), 4).unwrap());
                buffer.write(0, &1u32.to_ne_bytes()).unwrap();
                let weak_buffer = Rc::downgrade(&buffer);
                let kernel =
                    Rc::new(unsafe { Kernel::new(device.clone(), &words, 16, &[]).unwrap() });
                let weak_kernel = Rc::downgrade(&kernel);
                let mut root = [0u8; 16];
                root[..8].copy_from_slice(&buffer.address().unwrap().to_ne_bytes());
                root[8..12].copy_from_slice(&1u32.to_ne_bytes());
                let mut batch = Batch::new(device.clone()).unwrap();
                if timed {
                    batch.enable_timing().unwrap();
                }
                batch.retain_buffer(buffer).unwrap();
                batch.dispatch(kernel, [1, 1, 1], &root).unwrap();
                let mut done = unsafe { batch.submit().unwrap() };
                // Keep the consumed handle alive through retirement as well as the receipt.
                assert!(weak_buffer.upgrade().is_some() && weak_kernel.upgrade().is_some());
                assert_eq!(DESTROY_COUNTS.get(), [0; 4]);
                if mode != 2 {
                    if mode == 0 {
                        done.wait().unwrap();
                    } else {
                        // Make this poll deterministic; pending polls are independently gated.
                        assert_eq!(
                            unsafe { (device.f.vkQueueWaitIdle.unwrap())(device.queue) },
                            vk::VkResult_VK_SUCCESS
                        );
                        assert!(done.poll().unwrap());
                    }
                    assert!(done.submission.resources.is_none());
                    assert!(weak_buffer.upgrade().is_none() && weak_kernel.upgrade().is_none());
                    assert_eq!(DESTROY_COUNTS.get(), [0, 1, 1, 0]);
                    assert_eq!(RESET_COUNT.get(), 1);
                    assert!(done.poll().unwrap());
                    done.wait().unwrap();
                    if timed {
                        let elapsed = done.elapsed_ns().unwrap();
                        assert!(elapsed.is_finite());
                        assert_eq!(done.elapsed_ns().unwrap(), elapsed);
                    }
                }
                drop(done); // Also tests draining destruction without prior observation.
                assert!(weak_buffer.upgrade().is_none() && weak_kernel.upgrade().is_none());
                assert_eq!(DESTROY_COUNTS.get(), [0, 1, 1, u32::from(timed)]);
                assert_eq!(RESET_COUNT.get(), 1);
                assert!(
                    matches!(unsafe { batch.submit() }, Err(e) if e.status == INVALID_ARGUMENT)
                );
                drop(batch);
                drop(device);
                assert_eq!(DESTROY_COUNTS.get(), [1, 1, 1, u32::from(timed)]);
                tested += 1;
            }
        }
    }
    assert!(tested >= 3);
}

#[test]
fn dispatch_grid_checks_each_axis_inclusively() {
    let valid_grid = |groups, limits| crate::contract::dispatch(groups, limits, 0, 0).is_ok();
    let limits = [7, 3, 5];
    assert!(valid_grid([1, 1, 1], limits));
    assert!(valid_grid(limits, limits));
    assert!(valid_grid([u32::MAX; 3], [u32::MAX; 3]));
    for axis in 0..3 {
        for invalid in [0, limits[axis] + 1, u32::MAX] {
            let mut grid = [1; 3];
            grid[axis] = invalid;
            assert!(!valid_grid(grid, limits));
        }
    }
    assert!(!valid_grid([3, 5, 7], limits));
}

thread_local! {
    static FAILURE: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY) };
    static REAL_WAIT: Cell<vk::PFN_vkWaitSemaphores> = const { Cell::new(None) };
    static REAL_IDLE: Cell<vk::PFN_vkQueueWaitIdle> = const { Cell::new(None) };
    static WAIT_CALLS: Cell<u32> = const { Cell::new(0) };
    static IDLE_CALLS: Cell<u32> = const { Cell::new(0) };
    static REPORT_LOSS: Cell<bool> = const { Cell::new(false) };
    static REAL_SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    static REAL_DESTROY_DEVICE: Cell<vk::PFN_vkDestroyDevice> = const { Cell::new(None) };
    static DESTROYED_DEVICES: Cell<u32> = const { Cell::new(0) };
    static SUBMIT_CALLS: Cell<u32> = const { Cell::new(0) };
}

// Replace one function at a time; all earlier allocations and destruction stay real
// Vulkan calls so validation can catch partial-construction leaks and invalid cleanup.
macro_rules! fail {
    ($name:ident($($arg:ident: $ty:ty),*)) => {
        unsafe extern "C" fn $name($($arg: $ty),*) -> vk::VkResult { FAILURE.get() }
    };
}
fail!(fail_pool(_d: vk::VkDevice, _i: *const vk::VkCommandPoolCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkCommandPool));
fail!(fail_allocate(_d: vk::VkDevice, _i: *const vk::VkCommandBufferAllocateInfo,
    _o: *mut vk::VkCommandBuffer));
fail!(fail_begin(_c: vk::VkCommandBuffer, _i: *const vk::VkCommandBufferBeginInfo));
fail!(fail_end(_c: vk::VkCommandBuffer));
fail!(fail_submit(_q: vk::VkQueue, _n: u32, _s: *const vk::VkSubmitInfo2, _f: vk::VkFence));
fail!(fail_semaphore(_d: vk::VkDevice, _i: *const vk::VkSemaphoreCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkSemaphore));

unsafe extern "C" fn counted_destroy_device(d: vk::VkDevice, a: *const vk::VkAllocationCallbacks) {
    DESTROYED_DEVICES.set(DESTROYED_DEVICES.get() + 1);
    unsafe { (REAL_DESTROY_DEVICE.get().unwrap())(d, a) };
}

unsafe extern "C" fn submit_once_failed(
    q: vk::VkQueue,
    n: u32,
    s: *const vk::VkSubmitInfo2,
    f: vk::VkFence,
) -> vk::VkResult {
    let call = SUBMIT_CALLS.get();
    SUBMIT_CALLS.set(call + 1);
    assert!(f.is_null());
    assert_eq!(n, 1);
    let signal = unsafe { &*(*s).pSignalSemaphoreInfos };
    assert_eq!(signal.value, u64::from(call) + 1);
    assert_eq!(signal.stageMask, vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT);
    if call == 0 {
        FAILURE.get()
    } else {
        unsafe { (REAL_SUBMIT.get().unwrap())(q, n, s, f) }
    }
}

#[test]
#[ignore = "requires Vulkan; timeline construction, gaps, limits, and out-of-order waits"]
fn gpu_timeline_boundaries() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        match Device::new(instance.clone(), physical) {
            Ok(_) => {}
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        }
        FAILURE.set(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY);
        DESTROYED_DEVICES.set(0);
        let failed = Device::create_configured(instance.clone(), physical, false, |f| {
            f.vkCreateSemaphore = Some(fail_semaphore);
            REAL_DESTROY_DEVICE.set(f.vkDestroyDevice);
            f.vkDestroyDevice = Some(counted_destroy_device);
        });
        assert!(matches!(failed, Err(e) if e.vk == FAILURE.get()));
        assert_eq!(DESTROYED_DEVICES.get(), 1);

        for failure in [
            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
            vk::VkResult_VK_ERROR_UNKNOWN,
        ] {
            FAILURE.set(failure);
            SUBMIT_CALLS.set(0);
            let device = Device::create_configured(instance.clone(), physical, false, |f| {
                REAL_SUBMIT.set(f.vkQueueSubmit2);
                f.vkQueueSubmit2 = Some(submit_once_failed);
            })
            .unwrap();
            let mut failed = Batch::new(device.clone()).unwrap();
            assert!(matches!(unsafe { failed.submit() }, Err(e) if e.vk == failure));
            assert_eq!(device.next_timeline.get(), 1);
            let mut first = unsafe { Batch::new(device.clone()).unwrap().submit().unwrap() };
            let mut second = unsafe { Batch::new(device.clone()).unwrap().submit().unwrap() };
            assert_eq!((first.timeline_value, second.timeline_value), (2, 3));
            second.wait().unwrap();
            first.wait().unwrap();
            second.wait().unwrap();
            assert_eq!(SUBMIT_CALLS.get(), 3);
        }

        let mut device = Device::new(instance.clone(), physical).unwrap();
        // Tighten the reported limit to exercise rejection without billions of submits.
        Rc::get_mut(&mut device).unwrap().max_timeline_difference = 1;
        device.next_timeline.set(1); // Burned value; counter still zero.
        let mut limited = Batch::new(device.clone()).unwrap();
        assert!(matches!(unsafe { limited.submit() }, Err(e) if e.status == OUT_OF_RANGE));
        assert_eq!(device.next_timeline.get(), 1);
        assert!(limited.steps.is_none());
        drop(limited);
        Rc::get_mut(&mut device).unwrap().max_timeline_difference = u64::MAX;
        device.next_timeline.set(u64::MAX);
        let mut exhausted = Batch::new(device.clone()).unwrap();
        assert!(matches!(unsafe { exhausted.submit() }, Err(e) if e.status == OUT_OF_RANGE));
        assert_eq!(device.next_timeline.get(), u64::MAX);
        assert!(exhausted.steps.is_none());
        tested += 1;
    }
    assert!(tested > 0, "No execution-capable Vulkan device found");
}

unsafe extern "C" fn flaky_idle(queue: vk::VkQueue) -> vk::VkResult {
    let call = IDLE_CALLS.get();
    IDLE_CALLS.set(call + 1);
    if call == 0 {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    } else {
        unsafe { (REAL_IDLE.get().unwrap())(queue) }
    }
}

unsafe extern "C" fn flaky_wait(
    device: vk::VkDevice,
    wait: *const vk::VkSemaphoreWaitInfo,
    timeout: u64,
) -> vk::VkResult {
    let call = WAIT_CALLS.get();
    WAIT_CALLS.set(call + 1);
    match call {
        0 => vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
        1 => vk::VkResult_VK_TIMEOUT,
        _ => {
            let status = unsafe { (REAL_WAIT.get().unwrap())(device, wait, timeout) };
            // Only simulate loss AFTER real work completes; never pretend pending real
            // GPU work can be destroyed just because a test returned DEVICE_LOST.
            if status == vk::VkResult_VK_SUCCESS && REPORT_LOSS.get() {
                vk::VkResult_VK_ERROR_DEVICE_LOST
            } else {
                status
            }
        }
    }
}

#[test]
#[ignore = "requires Vulkan; injects failures around real resource ownership"]
fn gpu_batch_failures() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        match Device::new(instance.clone(), physical) {
            Ok(_) => {}
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        }
        for point in 0..16 {
            let mut device = Device::new(instance.clone(), physical).unwrap();
            let timed = point >= 8;
            let point = point % 8;
            if timed && device.timing_info().is_err() {
                continue;
            }
            let f = &mut Rc::get_mut(&mut device).unwrap().f;
            FAILURE.set(match point {
                6 => vk::VkResult_VK_ERROR_UNKNOWN,
                7 => vk::VkResult_VK_ERROR_DEVICE_LOST,
                _ => vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            });
            IDLE_CALLS.set(0);
            REAL_IDLE.set(f.vkQueueWaitIdle);
            f.vkQueueWaitIdle = Some(flaky_idle);
            match point {
                0 => f.vkCreateCommandPool = Some(fail_pool),
                1 => f.vkAllocateCommandBuffers = Some(fail_allocate),
                2 => f.vkBeginCommandBuffer = Some(fail_begin),
                3 => f.vkEndCommandBuffer = Some(fail_end),
                _ => f.vkQueueSubmit2 = Some(fail_submit),
            }
            let mut batch = Batch::new(device.clone()).unwrap();
            let retained = Rc::new(Buffer::new(device.clone(), 4).unwrap());
            let weak = Rc::downgrade(&retained);
            batch.retain_buffer(retained.clone()).unwrap();
            drop(retained);
            if timed {
                batch.enable_timing().unwrap();
            }
            let error = match unsafe { batch.submit() } {
                Err(e) => e,
                Ok(_) => panic!("Injected failure did not fail"),
            };
            assert_eq!(error.vk, FAILURE.get());
            assert!(
                weak.upgrade().is_none(),
                "Failed submission must release explicit retention"
            );
            assert!(batch.steps.is_none());
            assert!(matches!(unsafe { batch.submit() }, Err(e) if e.status == INVALID_ARGUMENT));
            assert_eq!(device.lost.get(), point == 7);
            assert_eq!(IDLE_CALLS.get(), if point == 6 { 2 } else { 0 });
            drop(batch);
            assert_eq!(Rc::strong_count(&device), 1);
        }
        for case in 0..4 {
            let loss = case & 1 != 0;
            let timed = case & 2 != 0;
            let mut device = Device::new(instance.clone(), physical).unwrap();
            if timed && device.timing_info().is_err() {
                continue;
            }
            let f = &mut Rc::get_mut(&mut device).unwrap().f;
            REAL_WAIT.set(f.vkWaitSemaphores);
            f.vkWaitSemaphores = Some(flaky_wait);
            REPORT_LOSS.set(loss);
            WAIT_CALLS.set(0);
            let mut batch = Batch::new(device.clone()).unwrap();
            if timed {
                batch.enable_timing().unwrap();
            }
            let mut completion = unsafe { batch.submit().unwrap() };
            assert_eq!(WAIT_CALLS.get(), 0, "Submit must not call wait");
            let expected = if loss {
                vk::VkResult_VK_ERROR_DEVICE_LOST
            } else {
                vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
            };
            assert_eq!(completion.wait().unwrap_err().vk, expected);
            assert_eq!(WAIT_CALLS.get(), 3);
            assert_eq!(completion.poll().unwrap_err().vk, expected);
            if timed {
                assert_eq!(completion.elapsed_ns().unwrap_err().vk, expected);
            }
            assert!(!completion.submission.pending);
            assert!(
                completion.submission.resources.is_none(),
                "Drained errors and loss still retire resources"
            );
            assert_eq!(completion.wait().unwrap_err().vk, expected);
            drop(completion);
            assert_eq!(
                WAIT_CALLS.get(),
                3,
                "Repeated wait/destruction must not rewait"
            );
            assert_eq!(device.lost.get(), loss);
            if loss {
                assert!(Batch::new(device.clone()).is_err());
                assert!(Buffer::new(device.clone(), 4).is_err());
            }
            drop(batch);
            assert_eq!(Rc::strong_count(&device), 1);
        }
        tested += 1;
    }
    assert!(tested > 0, "No execution-capable Vulkan device found");
}

#[test]
#[ignore = "requires Vulkan; run with --ignored --nocapture"]
fn gpu_batches() {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::new(instance.clone(), physical) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let other_device = Device::new(instance.clone(), physical).unwrap();
        let kernel = Rc::new(unsafe { Kernel::new(device.clone(), &words, 16, &[]).unwrap() });
        let other_kernel = Rc::new(unsafe { Kernel::new(other_device, &words, 16, &[]).unwrap() });
        let buffer = Buffer::new(device.clone(), 4).unwrap();
        buffer.write(0, &1u32.to_ne_bytes()).unwrap();
        let mut root = [0u8; 16];
        root[..8].copy_from_slice(&buffer.address().unwrap().to_ne_bytes());
        root[8..12].copy_from_slice(&1u32.to_ne_bytes());
        // A discarded recording must never execute.
        let mut discarded = Batch::new(device.clone()).unwrap();
        discarded
            .dispatch(kernel.clone(), [1, 1, 1], &root)
            .unwrap();
        drop(discarded);
        let mut batch = Batch::new(device.clone()).unwrap();
        assert_eq!(
            batch
                .dispatch(other_kernel, [1, 1, 1], &root)
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        assert_eq!(
            batch
                .dispatch(kernel.clone(), [0, 1, 1], &root)
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        assert_eq!(
            batch
                .dispatch(kernel.clone(), [1, 1, 1], &root[..12])
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        assert_eq!(
            batch.barrier(0, COMPUTE_READ).unwrap_err().status,
            INVALID_ARGUMENT
        );
        assert!(batch.steps.as_ref().unwrap().is_empty());
        batch.dispatch(kernel.clone(), [1, 1, 1], &root).unwrap();
        // Rejected grids must neither remove earlier work nor append a command.
        // Some devices report UINT32_MAX; there is no representable over-limit count.
        for axis in 0..3 {
            for invalid in [
                Some(0),
                device.limits.maxComputeWorkGroupCount[axis].checked_add(1),
            ]
            .into_iter()
            .flatten()
            {
                let mut grid = [1; 3];
                grid[axis] = invalid;
                assert_eq!(
                    batch
                        .dispatch(kernel.clone(), grid, &root)
                        .unwrap_err()
                        .status,
                    INVALID_ARGUMENT
                );
                assert_eq!(batch.steps.as_ref().unwrap().len(), 1);
            }
        }
        batch
            .barrier(COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
            .unwrap();
        batch.dispatch(kernel.clone(), [1, 1, 1], &root).unwrap();
        // A second submission uses an explicit dependency on the first submission.
        let mut next = Batch::new(device.clone()).unwrap();
        next.barrier(COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
            .unwrap();
        next.dispatch(kernel.clone(), [1, 1, 1], &root).unwrap();
        root.fill(0); // Recording must have copied the original argument bytes.
        drop(kernel);
        drop(device);
        let mut first = unsafe { batch.submit().unwrap() };
        assert_eq!(batch.barrier(2, 1).unwrap_err().status, INVALID_ARGUMENT);
        assert!(matches!(unsafe { batch.submit() }, Err(e) if e.status == INVALID_ARGUMENT));
        let last = unsafe { next.submit().unwrap() };
        drop(batch);
        drop(next);
        // Dropping a pending completion drains it, and keeps all retained kernels live.
        drop(last);
        first.wait().unwrap();
        first.wait().unwrap();
        drop(first);
        let mut value = 0u32;
        unsafe {
            buffer.read(0, (&mut value as *mut u32).cast(), 4).unwrap();
        }
        assert_eq!(value, 118); // Three passes: 1 → 10 → 37 → 118.
        let mut empty = Batch::new(buffer.device.clone()).unwrap();
        unsafe {
            empty.submit().unwrap().wait().unwrap();
        }
        tested += 1;
    }
    assert!(tested > 0, "No execution-capable Vulkan device found");
}

#[test]
fn access_masks_are_explicit_and_checked() {
    assert_eq!(
        access(COMPUTE_READ).unwrap().flags,
        vk::VK_ACCESS_2_SHADER_READ_BIT
    );
    assert_eq!(
        access(COMPUTE_WRITE).unwrap().flags,
        vk::VK_ACCESS_2_SHADER_WRITE_BIT
    );
    assert_eq!(
        access(3).unwrap().flags,
        access(1).unwrap().flags | access(2).unwrap().flags
    );
    assert_eq!(
        access(VERTEX_READ | INDIRECT_READ).unwrap().stages,
        vk::VK_PIPELINE_STAGE_2_VERTEX_SHADER_BIT | vk::VK_PIPELINE_STAGE_2_DRAW_INDIRECT_BIT
    );
    assert_eq!(
        access(COLOR_READ).unwrap().flags,
        vk::VK_ACCESS_2_COLOR_ATTACHMENT_READ_BIT
    );
    for mask in [0, 512, 513, u32::MAX] {
        assert_eq!(access(mask).unwrap_err().status, INVALID_ARGUMENT);
    }
}

#[test]
fn unexpected_submit_errors_require_queue_draining() {
    assert!(submission_is_unaccepted(
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    ));
    assert!(submission_is_unaccepted(
        vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY
    ));
    assert!(submission_is_unaccepted(vk::VkResult_VK_ERROR_DEVICE_LOST));
    assert!(!submission_is_unaccepted(vk::VkResult_VK_ERROR_UNKNOWN));
}
