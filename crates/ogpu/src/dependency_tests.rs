//! Split scopes, native event lifetime and serial replay. No fake successful GPU completion.
use super::*;

#[test]
fn command_list_modes_reject_unknown_flags() {
    assert!(!contract::compile_flags(0).unwrap());
    assert!(contract::compile_flags(1).unwrap());
    for flags in [2, 3, u32::MAX] {
        assert_eq!(
            contract::compile_flags(flags).unwrap_err().status,
            INVALID_ARGUMENT
        );
    }
}

thread_local! {
    static CREATE: Cell<vk::PFN_vkCreateEvent> = const { Cell::new(None) };
    static DESTROY: Cell<vk::PFN_vkDestroyEvent> = const { Cell::new(None) };
    static SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    static BEGIN: Cell<vk::PFN_vkBeginCommandBuffer> = const { Cell::new(None) };
    static WAIT: Cell<vk::PFN_vkWaitSemaphores> = const { Cell::new(None) };
    static CREATED: Cell<usize> = const { Cell::new(0) };
    static DESTROYED: Cell<usize> = const { Cell::new(0) };
    static FAIL_CREATE_AT: Cell<usize> = const { Cell::new(usize::MAX) };
    static SUBMIT_RESULT: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_SUCCESS) };
    static POLL_RESULT: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_SUCCESS) };
    static FAIL_BEGIN: Cell<bool> = const { Cell::new(false) };
    static BEGIN_FLAGS: Cell<u32> = const { Cell::new(u32::MAX) };
}

unsafe extern "C" fn create(
    d: vk::VkDevice,
    info: *const vk::VkEventCreateInfo,
    a: *const vk::VkAllocationCallbacks,
    out: *mut vk::VkEvent,
) -> vk::VkResult {
    if CREATED.get() == FAIL_CREATE_AT.get() {
        return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
    }
    let status = unsafe { (CREATE.get().unwrap())(d, info, a, out) };
    if status == vk::VkResult_VK_SUCCESS {
        CREATED.set(CREATED.get() + 1);
    }
    status
}
unsafe extern "C" fn destroy(
    d: vk::VkDevice,
    event: vk::VkEvent,
    a: *const vk::VkAllocationCallbacks,
) {
    DESTROYED.set(DESTROYED.get() + 1);
    unsafe { (DESTROY.get().unwrap())(d, event, a) };
}
unsafe extern "C" fn submit(
    q: vk::VkQueue,
    n: u32,
    s: *const vk::VkSubmitInfo2,
    fence: vk::VkFence,
) -> vk::VkResult {
    if SUBMIT_RESULT.get() != vk::VkResult_VK_SUCCESS {
        return SUBMIT_RESULT.get();
    }
    unsafe { (SUBMIT.get().unwrap())(q, n, s, fence) }
}
unsafe extern "C" fn begin(
    c: vk::VkCommandBuffer,
    info: *const vk::VkCommandBufferBeginInfo,
) -> vk::VkResult {
    BEGIN_FLAGS.set(unsafe { (*info).flags });
    if FAIL_BEGIN.get() {
        return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
    }
    unsafe { (BEGIN.get().unwrap())(c, info) }
}
unsafe extern "C" fn wait(
    d: vk::VkDevice,
    info: *const vk::VkSemaphoreWaitInfo,
    timeout: u64,
) -> vk::VkResult {
    // Inject only nonterminal poll results. Real wait always drains actual work.
    if timeout == 0 && POLL_RESULT.get() != vk::VkResult_VK_SUCCESS {
        return POLL_RESULT.get();
    }
    unsafe { (WAIT.get().unwrap())(d, info, timeout) }
}
fn configure(f: &mut Functions) {
    CREATE.set(f.vkCreateEvent);
    f.vkCreateEvent = Some(create);
    DESTROY.set(f.vkDestroyEvent);
    f.vkDestroyEvent = Some(destroy);
    SUBMIT.set(f.vkQueueSubmit2);
    f.vkQueueSubmit2 = Some(submit);
    BEGIN.set(f.vkBeginCommandBuffer);
    f.vkBeginCommandBuffer = Some(begin);
    WAIT.set(f.vkWaitSemaphores);
    f.vkWaitSemaphores = Some(wait);
    CREATED.set(0);
    DESTROYED.set(0);
    FAIL_CREATE_AT.set(usize::MAX);
    SUBMIT_RESULT.set(vk::VkResult_VK_SUCCESS);
    POLL_RESULT.set(vk::VkResult_VK_SUCCESS);
    FAIL_BEGIN.set(false);
}
fn point(batch: &mut Batch) -> u64 {
    batch
        .dependency_begin(COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
        .unwrap()
}
fn root(buffer: &Buffer) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&(buffer.address().unwrap() + 4).to_ne_bytes());
    bytes[8..12].copy_from_slice(&1u32.to_ne_bytes());
    bytes
}
fn initialize(buffer: &Buffer, value: u32) {
    buffer
        .write(
            0,
            &[0xcafeu32, value, 0xbeef]
                .into_iter()
                .flat_map(u32::to_ne_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
}
fn check(buffer: &Buffer, value: u32) {
    let mut bytes = [0u8; 12];
    unsafe { buffer.read(0, bytes.as_mut_ptr(), bytes.len()).unwrap() };
    assert_eq!(
        bytes,
        [0xcafeu32, value, 0xbeef]
            .into_iter()
            .flat_map(u32::to_ne_bytes)
            .collect::<Vec<_>>()
            .as_slice()
    );
}

#[test]
#[ignore = "requires Vulkan; split endpoints, serial reuse and lifetime/error ownership"]
fn gpu_split_dependencies() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    for physical in instance.physical_devices().unwrap() {
        let d = match Device::create_configured(instance.clone(), physical, false, configure) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let storage = Rc::new(RecordingStorage::new(d.clone()).unwrap());
        let mut batch = Batch::new_in(storage.clone()).unwrap();
        assert!(batch.dependency_begin(0, COMPUTE_READ).is_err());
        assert!(batch.dependency_begin(COMPUTE_WRITE, 512).is_err());
        assert!(batch.dependencies.is_empty() && batch.steps.as_ref().unwrap().is_empty());
        assert!(batch.dependency_end(0).is_err());
        let first = point(&mut batch);
        let second = point(&mut batch);
        let mut other = Batch::new(d.clone()).unwrap();
        let foreign = point(&mut other);
        assert!(other.dependency_end(first).is_err() && batch.dependency_end(foreign).is_err());
        assert!(unsafe { batch.submit() }.is_err() && batch.steps.is_some());
        assert!(unsafe { batch.compile_with_flags(0) }.is_err() && batch.steps.is_some());
        // Crossed intervals, then nested intervals; neither requires LIFO ends.
        batch.dependency_end(first).unwrap();
        batch.dependency_end(second).unwrap();
        assert!(batch.dependency_end(first).is_err());
        let third = point(&mut batch);
        let fourth = point(&mut batch);
        batch.dependency_end(fourth).unwrap();
        batch.dependency_end(third).unwrap();
        assert!(
            matches!(unsafe { batch.compile_with_flags(2) },Err(e) if e.status==INVALID_ARGUMENT)
        );
        assert!(matches!(unsafe { batch.compile_with_flags(1) },Err(e) if e.status==UNSUPPORTED));
        assert!(batch.steps.is_some());
        // Fail on the second allocation: first event must be released and lease freed.
        FAIL_CREATE_AT.set(CREATED.get() + 1);
        assert!(unsafe { batch.compile_with_flags(0) }.is_err());
        assert!(batch.steps.is_none() && !storage.busy.get());
        assert_eq!(CREATED.get(), DESTROYED.get());
        FAIL_CREATE_AT.set(usize::MAX);

        // Event ownership also unwinds if native command encoding cannot begin.
        let mut failed = Batch::new_in(storage.clone()).unwrap();
        let p = point(&mut failed);
        failed.dependency_end(p).unwrap();
        FAIL_BEGIN.set(true);
        assert!(unsafe { failed.compile_with_flags(0) }.is_err());
        FAIL_BEGIN.set(false);
        assert_eq!(CREATED.get(), DESTROYED.get());
        assert!(!storage.busy.get());

        // Valid crossed and nested intervals also reach native encoding/execution.
        let mut scopes = Batch::new_in(storage.clone()).unwrap();
        let a = point(&mut scopes);
        let b = point(&mut scopes);
        scopes.dependency_end(a).unwrap();
        scopes.dependency_end(b).unwrap();
        let c = point(&mut scopes);
        let e = point(&mut scopes);
        scopes.dependency_end(e).unwrap();
        scopes.dependency_end(c).unwrap();
        unsafe { scopes.submit().unwrap().wait().unwrap() };
        assert_eq!(CREATED.get(), DESTROYED.get());

        let kernel = Rc::new(unsafe { Kernel::new(d.clone(), &words, 16, &[]).unwrap() });
        let x = Rc::new(Buffer::new(d.clone(), 12).unwrap());
        let y = Rc::new(Buffer::new(d.clone(), 12).unwrap());
        for replay in [false, true] {
            initialize(&x, 1);
            initialize(&y, 37);
            let mut batch = Batch::new_in(storage.clone()).unwrap();
            batch.retain_buffer(x.clone()).unwrap();
            batch.retain_buffer(y.clone()).unwrap();
            batch
                .barrier(COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
                .unwrap();
            batch.dispatch(kernel.clone(), [1; 3], &root(&x)).unwrap();
            let p = point(&mut batch);
            batch.dispatch(kernel.clone(), [1; 3], &root(&y)).unwrap();
            batch.dependency_end(p).unwrap();
            batch.dispatch(kernel.clone(), [1; 3], &root(&x)).unwrap();
            if !replay {
                unsafe { batch.submit().unwrap().wait().unwrap() };
                check(&x, 37);
                check(&y, 118);
                assert!(!storage.busy.get());
                assert_eq!(CREATED.get(), DESTROYED.get());
                continue;
            }
            let list = Rc::new(unsafe { batch.compile_with_flags(0).unwrap() });
            assert_eq!(BEGIN_FLAGS.get(), 0);
            assert!(batch.dependency_end(p).is_err());
            let mut receipts = Vec::new();
            for v in [1u32, 37, u32::MAX, 99].into_iter().cycle().take(64) {
                initialize(&x, v);
                initialize(&y, v);
                let mut done = unsafe { list.submit().unwrap() };
                assert!(list.busy.get() && unsafe { list.submit() }.is_err());
                POLL_RESULT.set(vk::VkResult_VK_TIMEOUT);
                assert!(!done.poll().unwrap());
                POLL_RESULT.set(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY);
                assert!(done.poll().is_err());
                assert!(list.busy.get() && unsafe { list.submit() }.is_err());
                POLL_RESULT.set(vk::VkResult_VK_SUCCESS);
                done.wait().unwrap();
                assert!(!list.busy.get());
                check(
                    &x,
                    v.wrapping_mul(3)
                        .wrapping_add(7)
                        .wrapping_mul(3)
                        .wrapping_add(7),
                );
                check(&y, v.wrapping_mul(3).wrapping_add(7));
                receipts.push(done);
            }
            SUBMIT_RESULT.set(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY);
            assert!(unsafe { list.submit() }.is_err() && !list.busy.get() && !list.poisoned.get());
            SUBMIT_RESULT.set(vk::VkResult_VK_SUCCESS);
            unsafe { list.submit().unwrap().wait().unwrap() };
            SUBMIT_RESULT.set(vk::VkResult_VK_ERROR_UNKNOWN);
            assert!(unsafe { list.submit() }.is_err() && !list.busy.get() && list.poisoned.get());
            SUBMIT_RESULT.set(vk::VkResult_VK_SUCCESS);
            assert!(unsafe { list.submit() }.is_err());
            drop(list);
            assert!(!storage.busy.get());
            assert_eq!(CREATED.get(), DESTROYED.get());
            drop(receipts);
        }
        // No split points: explicit simultaneous use remains supported.
        let mut plain = Batch::new(d.clone()).unwrap();
        let list = Rc::new(unsafe { plain.compile_with_flags(1).unwrap() });
        assert_eq!(
            BEGIN_FLAGS.get(),
            vk::VkCommandBufferUsageFlagBits_VK_COMMAND_BUFFER_USAGE_SIMULTANEOUS_USE_BIT
        );
        let mut a = unsafe { list.submit().unwrap() };
        let mut b = unsafe { list.submit().unwrap() };
        a.wait().unwrap();
        b.wait().unwrap();
        drop(list);
        // Synthetic loss on rejected submission, with no actual work outstanding.
        let mut final_batch = Batch::new(d.clone()).unwrap();
        let p = point(&mut final_batch);
        final_batch.dependency_end(p).unwrap();
        let list = Rc::new(unsafe { final_batch.compile_with_flags(0).unwrap() });
        SUBMIT_RESULT.set(vk::VkResult_VK_ERROR_DEVICE_LOST);
        assert!(unsafe { list.submit() }.is_err() && !list.busy.get() && d.lost.get());
        SUBMIT_RESULT.set(vk::VkResult_VK_SUCCESS);
        drop(list);
        assert_eq!(CREATED.get(), DESTROYED.get());
        tested += 1;
    }
    assert!(tested > 0);
}
