use super::*;

thread_local! {
    static FAILURE: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY) };
    static REAL_WAIT: Cell<vk::PFN_vkWaitForFences> = const { Cell::new(None) };
    static REAL_IDLE: Cell<vk::PFN_vkQueueWaitIdle> = const { Cell::new(None) };
    static WAIT_CALLS: Cell<u32> = const { Cell::new(0) };
    static IDLE_CALLS: Cell<u32> = const { Cell::new(0) };
    static REPORT_LOSS: Cell<bool> = const { Cell::new(false) };
}

// Replace one function at a time; all earlier allocations and destruction stay real
// Vulkan calls so validation can catch partial-construction leaks and invalid cleanup.
macro_rules! fail {
    ($name:ident($($arg:ident: $ty:ty),*)) => {
        unsafe extern "C" fn $name($($arg: $ty),*) -> vk::VkResult { FAILURE.get() }
    };
}
fail!(fail_fence(_d: vk::VkDevice, _i: *const vk::VkFenceCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkFence));
fail!(fail_pool(_d: vk::VkDevice, _i: *const vk::VkCommandPoolCreateInfo,
    _a: *const vk::VkAllocationCallbacks, _o: *mut vk::VkCommandPool));
fail!(fail_allocate(_d: vk::VkDevice, _i: *const vk::VkCommandBufferAllocateInfo,
    _o: *mut vk::VkCommandBuffer));
fail!(fail_begin(_c: vk::VkCommandBuffer, _i: *const vk::VkCommandBufferBeginInfo));
fail!(fail_end(_c: vk::VkCommandBuffer));
fail!(fail_submit(_q: vk::VkQueue, _n: u32, _s: *const vk::VkSubmitInfo, _f: vk::VkFence));

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
    count: u32,
    fences: *const vk::VkFence,
    all: vk::VkBool32,
    timeout: u64,
) -> vk::VkResult {
    let call = WAIT_CALLS.get();
    WAIT_CALLS.set(call + 1);
    match call {
        0 => vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
        1 => vk::VkResult_VK_TIMEOUT,
        _ => {
            let status = unsafe { (REAL_WAIT.get().unwrap())(device, count, fences, all, timeout) };
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
        for point in 0..8 {
            let mut device = Device::new(instance.clone(), physical).unwrap();
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
                0 | 7 => f.vkCreateFence = Some(fail_fence),
                1 => f.vkCreateCommandPool = Some(fail_pool),
                2 => f.vkAllocateCommandBuffers = Some(fail_allocate),
                3 => f.vkBeginCommandBuffer = Some(fail_begin),
                4 => f.vkEndCommandBuffer = Some(fail_end),
                _ => f.vkQueueSubmit = Some(fail_submit),
            }
            let mut batch = Batch::new(device.clone()).unwrap();
            let error = match unsafe { batch.submit() } {
                Err(e) => e,
                Ok(_) => panic!("Injected failure did not fail"),
            };
            assert_eq!(error.vk, FAILURE.get());
            assert!(batch.steps.is_none());
            assert!(matches!(unsafe { batch.submit() }, Err(e) if e.status == INVALID_ARGUMENT));
            assert_eq!(device.lost.get(), point == 7);
            assert_eq!(IDLE_CALLS.get(), if point == 6 { 2 } else { 0 });
            drop(batch);
            assert_eq!(Rc::strong_count(&device), 1);
        }
        for loss in [false, true] {
            let mut device = Device::new(instance.clone(), physical).unwrap();
            let f = &mut Rc::get_mut(&mut device).unwrap().f;
            REAL_WAIT.set(f.vkWaitForFences);
            f.vkWaitForFences = Some(flaky_wait);
            REPORT_LOSS.set(loss);
            WAIT_CALLS.set(0);
            let mut batch = Batch::new(device.clone()).unwrap();
            let mut completion = unsafe { batch.submit().unwrap() };
            assert_eq!(WAIT_CALLS.get(), 0, "Submit must not call wait");
            let expected = if loss {
                vk::VkResult_VK_ERROR_DEVICE_LOST
            } else {
                vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
            };
            assert_eq!(completion.wait().unwrap_err().vk, expected);
            assert_eq!(WAIT_CALLS.get(), 3);
            assert!(!completion.pending);
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
        let kernel = Rc::new(unsafe { Kernel::new(device.clone(), &words, 16).unwrap() });
        let other_kernel = Rc::new(unsafe { Kernel::new(other_device, &words, 16).unwrap() });
        let buffer = Buffer::new(device.clone(), 4).unwrap();
        buffer.write(0, &1u32.to_ne_bytes()).unwrap();
        let mut root = [0u8; 16];
        root[..8].copy_from_slice(&buffer.address().unwrap().to_ne_bytes());
        root[8..12].copy_from_slice(&1u32.to_ne_bytes());
        // A discarded recording must never execute.
        let mut discarded = Batch::new(device.clone()).unwrap();
        discarded.dispatch(kernel.clone(), 1, &root).unwrap();
        drop(discarded);
        let mut batch = Batch::new(device.clone()).unwrap();
        assert_eq!(
            batch.dispatch(other_kernel, 1, &root).unwrap_err().status,
            INVALID_ARGUMENT
        );
        assert_eq!(
            batch.dispatch(kernel.clone(), 0, &root).unwrap_err().status,
            INVALID_ARGUMENT
        );
        assert_eq!(
            batch
                .dispatch(kernel.clone(), 1, &root[..12])
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        assert_eq!(
            batch.barrier(0, COMPUTE_READ).unwrap_err().status,
            INVALID_ARGUMENT
        );
        assert!(batch.steps.as_ref().unwrap().is_empty());
        batch.dispatch(kernel.clone(), 1, &root).unwrap();
        batch
            .barrier(COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
            .unwrap();
        batch.dispatch(kernel.clone(), 1, &root).unwrap();
        // A second submission uses an explicit dependency on the first submission.
        let mut next = Batch::new(device.clone()).unwrap();
        next.barrier(COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
            .unwrap();
        next.dispatch(kernel.clone(), 1, &root).unwrap();
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
        vk::VkAccessFlagBits_VK_ACCESS_SHADER_READ_BIT
    );
    assert_eq!(
        access(COMPUTE_WRITE).unwrap().flags,
        vk::VkAccessFlagBits_VK_ACCESS_SHADER_WRITE_BIT
    );
    assert_eq!(
        access(3).unwrap().flags,
        access(1).unwrap().flags | access(2).unwrap().flags
    );
    assert_eq!(
        access(VERTEX_READ | INDIRECT_READ).unwrap().stages,
        vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_VERTEX_SHADER_BIT
            | vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_DRAW_INDIRECT_BIT
    );
    for mask in [0, 256, 257, u32::MAX] {
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
