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
    static ENCODED: Cell<usize> = const { Cell::new(0) };
    static SUBMITTED: Cell<usize> = const { Cell::new(0) };
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
    ENCODED.set(ENCODED.get() + 1);
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
    SUBMITTED.set(SUBMITTED.get() + 1);
    if FAILURE.get() == 5 {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    } else if FAILURE.get() == 6 {
        vk::VkResult_VK_ERROR_UNKNOWN
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
    ENCODED.set(0);
    SUBMITTED.set(0);
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

        for (failure, explicit) in (1..=5).flat_map(|failure| [(failure, false), (failure, true)]) {
            let device =
                Device::create_configured(instance.clone(), physical, false, configure).unwrap();
            let owner = Rc::new(RecordingStorage::new(device.clone()).unwrap());
            let new_batch = || {
                if explicit {
                    Batch::new_in(owner.clone()).unwrap()
                } else {
                    barrier_batch(&device, 1)
                }
            };
            unsafe { new_batch().submit().unwrap() }.wait().unwrap();
            assert_eq!(COUNTS.get(), [1, 0, 1]);
            let mut batch = new_batch();
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
            assert!(!owner.busy.get());
            assert!(owner.idle.borrow().is_none());
            assert!(device.command_storage.borrow().is_empty());
            assert_eq!(
                COUNTS.get()[0],
                1,
                "Warm preparation acquired existing storage"
            );
            assert_eq!(COUNTS.get()[1], 1, "Failed storage destroyed, never cached");
            FAILURE.set(0);
            if failure != 2 {
                unsafe { new_batch().submit().unwrap() }.wait().unwrap();
                assert_eq!(COUNTS.get()[0], 2, "Recovery creates fresh storage");
            }
            drop(batch);
            owner.trim().unwrap(); // Cleanup remains legal after loss.
            drop(owner);
            drop(device);
            assert_eq!(COUNTS.get()[0], COUNTS.get()[1]);
        }
        tested += 1;
    }
    assert!(tested > 0, "No execution-capable device");
}

#[test]
#[ignore = "requires Vulkan; explicit storage leases, oversized reuse, trim and owner destruction"]
fn gpu_owned_recording_storage() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::create_configured(instance.clone(), physical, false, configure) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let owner = Rc::new(RecordingStorage::new(d.clone()).unwrap());
        owner.trim().unwrap();
        let abandoned = Batch::new_in(owner.clone()).unwrap();
        assert!(Batch::new_in(owner.clone()).is_err() && owner.trim().is_err());
        drop(abandoned);
        assert!(!owner.busy.get());
        let mut saved = Vec::new();
        let mut saved_batches = Vec::new();
        for steps in [1, 256, 257, 4096, 1].into_iter().cycle().take(50) {
            let mut batch = Batch::new_in(owner.clone()).unwrap();
            for _ in 0..steps {
                batch.barrier(COMPUTE_WRITE, COMPUTE_READ).unwrap();
            }
            if d.timing_info().is_ok() {
                batch.enable_timing().unwrap();
            }
            let mut receipt = unsafe { batch.submit().unwrap() };
            assert!(owner.trim().is_err() && Batch::new_in(owner.clone()).is_err());
            receipt.wait().unwrap();
            assert!(!owner.busy.get() && owner.idle.borrow().is_some());
            assert!(
                d.command_storage.borrow().is_empty(),
                "Explicit storage bypasses device cache"
            );
            assert_eq!(COUNTS.get()[0], 1, "Large/small recordings reuse one pool");
            assert_eq!(COUNTS.get()[1], 0);
            saved.push(receipt);
            saved_batches.push(batch);
        }
        owner.trim().unwrap();
        owner.trim().unwrap();
        assert_eq!(COUNTS.get(), [1, 1, 50]);
        for receipt in &mut saved {
            if receipt.timed {
                assert!(receipt.elapsed_ns().unwrap().is_finite());
            }
            assert!(receipt.poll().unwrap());
        }
        // Storage is lazily reacquired after trim; destroying the public owner while
        // a batch is recorded or accepted never waits and cannot destroy active storage.
        let mut batch = Batch::new_in(owner.clone()).unwrap();
        let weak_owner = Rc::downgrade(&owner);
        drop(owner);
        let mut receipt = unsafe { batch.submit().unwrap() };
        assert!(weak_owner.upgrade().is_some());
        assert_eq!(COUNTS.get()[0], 2);
        receipt.wait().unwrap();
        assert!(
            weak_owner.upgrade().is_none(),
            "Consumed batch/receipt cannot pin owner"
        );
        assert_eq!(COUNTS.get()[1], 2);
        // Failed submission on an already-lost device consumes/relinquishes lease too.
        let owner = Rc::new(RecordingStorage::new(d.clone()).unwrap());
        let mut batch = Batch::new_in(owner.clone()).unwrap();
        d.lost.set(true); // No real work remains.
        assert!(unsafe { batch.submit() }.is_err());
        assert!(!owner.busy.get());
        owner.trim().unwrap();
        tested += 1;
    }
    assert!(tested > 0);
}

#[test]
#[ignore = "requires Vulkan; encode-once replay, ownership and retry/poison/failure paths"]
fn gpu_command_lists() {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::create_configured(instance.clone(), physical, false, configure) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let storage = Rc::new(RecordingStorage::new(d.clone()).unwrap());
        let mut batch = Batch::new_in(storage.clone()).unwrap();
        let buffer = Rc::new(Buffer::new(d.clone(), 12).unwrap());
        let weak_buffer = Rc::downgrade(&buffer);
        let kernel = Rc::new(unsafe { Kernel::new(d.clone(), &words, 16, &[]).unwrap() });
        let weak_kernel = Rc::downgrade(&kernel);
        let mut root = [0u8; 16];
        root[..8].copy_from_slice(&(buffer.address().unwrap() + 4).to_ne_bytes());
        root[8..12].copy_from_slice(&1u32.to_ne_bytes());
        batch
            .barrier(COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
            .unwrap();
        batch.dispatch(kernel, [1; 3], &root).unwrap();
        batch.retain_buffer(buffer.clone()).unwrap();
        root.fill(0); // Recorded roots must not refer to this caller storage.
        let list = Rc::new(unsafe { batch.compile().unwrap() });
        assert_eq!((ENCODED.get(), SUBMITTED.get()), (1, 0));
        assert!(storage.trim().is_err() && Batch::new_in(storage.clone()).is_err());
        assert!(unsafe { batch.compile() }.is_err() && unsafe { batch.submit() }.is_err());
        let mut receipts = Vec::new();
        for value in [1u32, 37, u32::MAX, 55].into_iter().cycle().take(40) {
            buffer
                .write(
                    0,
                    &[0x55aa55aau32, value, 0x12345678]
                        .into_iter()
                        .flat_map(u32::to_ne_bytes)
                        .collect::<Vec<_>>(),
                )
                .unwrap();
            let mut done = unsafe { list.submit().unwrap() };
            done.wait().unwrap();
            assert_eq!(done.elapsed_ns().unwrap_err().status, INVALID_ARGUMENT);
            let mut actual = [0u32; 3];
            unsafe {
                buffer.read(0, actual.as_mut_ptr().cast(), 12).unwrap();
            }
            assert_eq!(
                actual,
                [
                    0x55aa55aa,
                    value.wrapping_mul(3).wrapping_add(7),
                    0x12345678
                ]
            );
            receipts.push(done);
        }
        assert_eq!((ENCODED.get(), SUBMITTED.get()), (1, 40));
        assert_eq!(COUNTS.get(), [1, 0, 0], "No reset between executions");
        drop(buffer);
        assert!(weak_buffer.upgrade().is_some() && weak_kernel.upgrade().is_some());
        FAILURE.set(5); // A known unaccepted failure leaves the list executable.
        assert!(
            matches!(unsafe { list.submit() }, Err(e) if e.vk == vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY)
        );
        FAILURE.set(0);
        unsafe { list.submit().unwrap() }.wait().unwrap();
        assert_eq!(ENCODED.get(), 1);
        FAILURE.set(6); // Unknown result drains and permanently rejects new uses.
        assert!(
            matches!(unsafe { list.submit() }, Err(e) if e.vk == vk::VkResult_VK_ERROR_UNKNOWN)
        );
        FAILURE.set(0);
        let before = SUBMITTED.get();
        assert!(matches!(unsafe { list.submit() }, Err(e) if e.status == INVALID_ARGUMENT));
        assert_eq!(SUBMITTED.get(), before);
        drop(list);
        assert!(weak_buffer.upgrade().is_none() && weak_kernel.upgrade().is_none());
        storage.trim().unwrap();
        for done in &mut receipts {
            assert!(done.poll().unwrap());
        }
        assert_eq!(
            COUNTS.get()[1],
            1,
            "Poisoned recording destroyed, never cached"
        );

        if d.timing_info().is_ok() {
            let mut timed = Batch::new_in(storage.clone()).unwrap();
            timed.enable_timing().unwrap();
            assert!(matches!(unsafe { timed.compile() }, Err(e) if e.status == UNSUPPORTED));
            assert!(
                timed.steps.is_some(),
                "Unsupported timing does not consume recording"
            );
            let mut done = unsafe { timed.submit().unwrap() };
            done.wait().unwrap();
            assert!(done.elapsed_ns().unwrap().is_finite());
        }
        for failure in [3, 4] {
            let mut failed = Batch::new_in(storage.clone()).unwrap();
            FAILURE.set(failure);
            assert!(unsafe { failed.compile() }.is_err());
            FAILURE.set(0);
            assert!(failed.steps.is_none() && !storage.busy.get());
            storage.trim().unwrap();
        }
        // Untouched recording can be discarded without any execution.
        let mut unused = Batch::new_in(storage.clone()).unwrap();
        let list = unsafe { unused.compile().unwrap() };
        drop(list);
        storage.trim().unwrap();
        // Final-use retirement destroys the list; reset loss must reach this receipt.
        let mut final_batch = Batch::new_in(storage.clone()).unwrap();
        let final_list = Rc::new(unsafe { final_batch.compile().unwrap() });
        let mut final_done = unsafe { final_list.submit().unwrap() };
        let mut loss_batch = Batch::new(d.clone()).unwrap();
        drop(final_list);
        FAILURE.set(2); // Injection only occurs after the real timeline wait.
        assert_eq!(
            final_done.wait().unwrap_err().vk,
            vk::VkResult_VK_ERROR_DEVICE_LOST
        );
        FAILURE.set(0);
        assert!(d.lost.get() && !storage.busy.get());
        assert_eq!(
            final_done.poll().unwrap_err().vk,
            vk::VkResult_VK_ERROR_DEVICE_LOST
        );
        assert!(unsafe { loss_batch.compile() }.is_err());
        assert!(
            loss_batch.steps.is_none(),
            "Lost-device compile attempt consumes recording"
        );
        storage.trim().unwrap();
        assert_eq!(COUNTS.get()[0], COUNTS.get()[1]);
        tested += 1;
    }
    assert!(tested > 0);
}
