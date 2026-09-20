//! Real-driver storage reuse, bounds and failure fallback. No synthetic GPU completion.
use super::*;

#[test]
fn cache_admission_bounds_steps_roots_and_overflow() {
    assert!(cacheable_shape(0, [].into_iter()));
    assert!(cacheable_shape(
        MAX_CACHED_STEPS,
        [MAX_CACHED_ROOT_BYTES].into_iter()
    ));
    assert!(!cacheable_shape(MAX_CACHED_STEPS + 1, [].into_iter()));
    assert!(!cacheable_shape(2, [MAX_CACHED_ROOT_BYTES, 1].into_iter()));
    assert!(!cacheable_shape(2, [usize::MAX, 1].into_iter()));
}

thread_local! {
    static CREATE: Cell<vk::PFN_vkCreateCommandPool> = const { Cell::new(None) };
    static DESTROY: Cell<vk::PFN_vkDestroyCommandPool> = const { Cell::new(None) };
    static RESET: Cell<vk::PFN_vkResetCommandPool> = const { Cell::new(None) };
    static BEGIN: Cell<vk::PFN_vkBeginCommandBuffer> = const { Cell::new(None) };
    static END: Cell<vk::PFN_vkEndCommandBuffer> = const { Cell::new(None) };
    static SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    static COUNTS: Cell<[usize; 3]> = const { Cell::new([0; 3]) };
    static FAILURE: Cell<u32> = const { Cell::new(0) };
}

fn count(index: usize) {
    let mut counts = COUNTS.get();
    counts[index] += 1;
    COUNTS.set(counts);
}
unsafe extern "C" fn create(
    d: vk::VkDevice,
    i: *const vk::VkCommandPoolCreateInfo,
    a: *const vk::VkAllocationCallbacks,
    p: *mut vk::VkCommandPool,
) -> vk::VkResult {
    let status = unsafe { (CREATE.get().unwrap())(d, i, a, p) };
    if status == vk::VkResult_VK_SUCCESS {
        count(0);
    }
    status
}
unsafe extern "C" fn destroy(
    d: vk::VkDevice,
    p: vk::VkCommandPool,
    a: *const vk::VkAllocationCallbacks,
) {
    count(1);
    unsafe { (DESTROY.get().unwrap())(d, p, a) };
}
unsafe extern "C" fn reset(
    d: vk::VkDevice,
    p: vk::VkCommandPool,
    flags: vk::VkCommandPoolResetFlags,
) -> vk::VkResult {
    count(2);
    match FAILURE.get() {
        1 => vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
        // Only injected after a real successful wait; actual work is drained.
        2 => vk::VkResult_VK_ERROR_DEVICE_LOST,
        _ => unsafe { (RESET.get().unwrap())(d, p, flags) },
    }
}
unsafe extern "C" fn begin(
    c: vk::VkCommandBuffer,
    i: *const vk::VkCommandBufferBeginInfo,
) -> vk::VkResult {
    if FAILURE.get() == 3 {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    } else {
        unsafe { (BEGIN.get().unwrap())(c, i) }
    }
}
unsafe extern "C" fn end(c: vk::VkCommandBuffer) -> vk::VkResult {
    if FAILURE.get() == 4 {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    } else {
        unsafe { (END.get().unwrap())(c) }
    }
}
unsafe extern "C" fn submit(
    q: vk::VkQueue,
    n: u32,
    s: *const vk::VkSubmitInfo2,
    f: vk::VkFence,
) -> vk::VkResult {
    if FAILURE.get() == 5 {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    } else {
        unsafe { (SUBMIT.get().unwrap())(q, n, s, f) }
    }
}
fn configure(f: &mut Functions) {
    CREATE.set(f.vkCreateCommandPool);
    f.vkCreateCommandPool = Some(create);
    DESTROY.set(f.vkDestroyCommandPool);
    f.vkDestroyCommandPool = Some(destroy);
    RESET.set(f.vkResetCommandPool);
    f.vkResetCommandPool = Some(reset);
    BEGIN.set(f.vkBeginCommandBuffer);
    f.vkBeginCommandBuffer = Some(begin);
    END.set(f.vkEndCommandBuffer);
    f.vkEndCommandBuffer = Some(end);
    SUBMIT.set(f.vkQueueSubmit2);
    f.vkQueueSubmit2 = Some(submit);
    COUNTS.set([0; 3]);
    FAILURE.set(0);
}

fn barrier_batch(device: &Rc<Device>, steps: usize) -> Batch {
    let mut batch = Batch::new(device.clone()).unwrap();
    for _ in 0..steps {
        batch.barrier(COMPUTE_WRITE, COMPUTE_READ).unwrap();
    }
    batch
}

#[test]
#[ignore = "requires Vulkan; bounded command-storage reuse, warm-cache errors and query independence"]
fn gpu_command_storage() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::create_configured(instance.clone(), physical, false, configure) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let mut batch = barrier_batch(&device, 1);
        if device.timing_info().is_ok() {
            batch.enable_timing().unwrap();
        }
        let mut receipt = unsafe { batch.submit().unwrap() };
        receipt.wait().unwrap();
        let queries = receipt.queries;
        assert_eq!(COUNTS.get(), [1, 0, 1]);
        // Keep consumed batch + unread timing alive while the same storage is reused.
        for steps in [0, 1, MAX_CACHED_STEPS, MAX_CACHED_STEPS + 1, 1]
            .into_iter()
            .cycle()
            .take(100)
        {
            let before = COUNTS.get();
            let mut next = barrier_batch(&device, steps);
            if receipt.timed {
                next.enable_timing().unwrap();
            }
            let mut done = unsafe { next.submit().unwrap() };
            if receipt.timed {
                assert_ne!(done.queries, queries);
            }
            assert_eq!(
                device.command_storage.borrow().len(),
                usize::from(steps > MAX_CACHED_STEPS)
            );
            done.wait().unwrap();
            assert_eq!(device.command_storage.borrow().len(), 1);
            let after = COUNTS.get();
            if steps > MAX_CACHED_STEPS {
                assert_eq!(after, [before[0] + 1, before[1] + 1, before[2]]);
            } else {
                assert_eq!(after, [before[0], before[1], before[2] + 1]);
            }
        }
        if receipt.timed {
            assert!(receipt.elapsed_ns().unwrap().is_finite());
        }
        assert!(matches!(unsafe { batch.submit() }, Err(e) if e.status == INVALID_ARGUMENT));
        // More outstanding submissions than cache capacity; no unobserved collection.
        let mut submissions = Vec::new();
        for _ in 0..MAX_CACHED_STORAGE + 2 {
            submissions.push(unsafe { barrier_batch(&device, 1).submit().unwrap() });
        }
        assert!(device.command_storage.borrow().is_empty());
        submissions.last_mut().unwrap().wait().unwrap();
        assert_eq!(device.command_storage.borrow().len(), 1);
        assert!(submissions[0].submission.resources.is_some());
        for done in &mut submissions {
            done.wait().unwrap();
        }
        assert_eq!(device.command_storage.borrow().len(), MAX_CACHED_STORAGE);
        assert_eq!(COUNTS.get()[0] - COUNTS.get()[1], MAX_CACHED_STORAGE);
        drop(submissions);
        drop(receipt);
        drop(batch);
        drop(device);
        assert_eq!(
            COUNTS.get()[0],
            COUNTS.get()[1],
            "Final owner releases all storage"
        );

        for failure in 1..=5 {
            let device =
                Device::create_configured(instance.clone(), physical, false, configure).unwrap();
            unsafe { barrier_batch(&device, 1).submit().unwrap() }
                .wait()
                .unwrap();
            assert_eq!(COUNTS.get(), [1, 0, 1]);
            let mut batch = barrier_batch(&device, 1);
            let buffer = Rc::new(Buffer::new(device.clone(), 4).unwrap());
            let weak = Rc::downgrade(&buffer);
            batch.retain_buffer(buffer).unwrap();
            FAILURE.set(failure);
            if failure <= 2 {
                let mut receipt = unsafe { batch.submit().unwrap() };
                let result = receipt.wait();
                if failure == 1 {
                    result.unwrap();
                } else {
                    assert_eq!(result.unwrap_err().vk, vk::VkResult_VK_ERROR_DEVICE_LOST);
                    assert_eq!(
                        receipt.poll().unwrap_err().vk,
                        vk::VkResult_VK_ERROR_DEVICE_LOST
                    );
                    assert!(device.lost.get());
                }
            } else {
                assert!(
                    matches!(unsafe { batch.submit() }, Err(e) if e.vk == vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY)
                );
            }
            assert!(weak.upgrade().is_none());
            assert!(device.command_storage.borrow().is_empty());
            assert_eq!(
                COUNTS.get()[0],
                1,
                "Warm preparation acquired existing storage"
            );
            assert_eq!(COUNTS.get()[1], 1, "Failed storage destroyed, never cached");
            FAILURE.set(0);
            if failure != 2 {
                unsafe { barrier_batch(&device, 1).submit().unwrap() }
                    .wait()
                    .unwrap();
                assert_eq!(COUNTS.get()[0], 2, "Recovery creates fresh storage");
            }
            drop(batch);
            drop(device);
            assert_eq!(COUNTS.get()[0], COUNTS.get()[1]);
        }
        tested += 1;
    }
    assert!(tested > 0, "No execution-capable device");
}
