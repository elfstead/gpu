use super::*;

thread_local! {
    static SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    static GATE: Cell<vk::VkSemaphore> = const { Cell::new(ptr::null_mut()) };
    static CALLS: Cell<u32> = const { Cell::new(0) };
    static WAIT: Cell<vk::PFN_vkWaitSemaphores> = const { Cell::new(None) };
    static POLL_STATUS: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_SUCCESS) };
}

unsafe extern "C" fn gated_submit(
    q: vk::VkQueue,
    count: u32,
    info: *const vk::VkSubmitInfo2,
    fence: vk::VkFence,
) -> vk::VkResult {
    let call = CALLS.get();
    CALLS.set(call + 1);
    assert_eq!(count, 1);
    let mut submit = unsafe { *info };
    let gate = vk::VkSemaphoreSubmitInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
        semaphore: GATE.get(),
        value: 1,
        stageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        ..Default::default()
    };
    if call == 1 {
        submit.waitSemaphoreInfoCount = 1;
        submit.pWaitSemaphoreInfos = &gate;
    }
    unsafe { (SUBMIT.get().unwrap())(q, count, &submit, fence) }
}

unsafe extern "C" fn poll_error(
    d: vk::VkDevice,
    info: *const vk::VkSemaphoreWaitInfo,
    timeout: u64,
) -> vk::VkResult {
    if timeout == 0 && POLL_STATUS.get() != vk::VkResult_VK_SUCCESS {
        POLL_STATUS.get()
    } else {
        unsafe { (WAIT.get().unwrap())(d, info, timeout) }
    }
}

// Own completions inside the gate guard so panic cleanup opens the gate BEFORE
// any completion destructor waits. Host-signaling exists only in this test.
struct Gate {
    device: Rc<Device>,
    semaphore: vk::VkSemaphore,
    signal: vk::PFN_vkSignalSemaphore,
    opened: bool,
    completions: Vec<Completion>,
    caller_owner: Option<Rc<Buffer>>,
}

impl Gate {
    fn open(&mut self) {
        if !self.opened {
            let signal = vk::VkSemaphoreSignalInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SIGNAL_INFO,
                semaphore: self.semaphore,
                value: 1,
                ..Default::default()
            };
            // No simulated failures here; open before any draining destruction.
            let status = unsafe { (self.signal.unwrap())(self.device.handle, &signal) };
            assert_eq!(status, vk::VkResult_VK_SUCCESS);
            self.opened = true;
        }
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        self.open();
        self.completions.clear();
        unsafe {
            (self.device.f.vkDestroySemaphore.unwrap())(
                self.device.handle,
                self.semaphore,
                ptr::null(),
            )
        };
    }
}

#[test]
#[ignore = "requires Vulkan; gated scratch reuse compares caller ownership and explicit retention"]
fn gpu_retirement() {
    let instance = Arc::new(Instance::new().unwrap());
    let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        match Device::new(instance.clone(), physical) {
            Ok(_) => {}
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        }
        for assisted in [false, true] {
            let device = Device::create_configured(instance.clone(), physical, false, |f| {
                SUBMIT.set(f.vkQueueSubmit2);
                f.vkQueueSubmit2 = Some(gated_submit);
                WAIT.set(f.vkWaitSemaphores);
                f.vkWaitSemaphores = Some(poll_error);
            })
            .unwrap();
            let signal: vk::PFN_vkSignalSemaphore =
                unsafe { std::mem::transmute(instance.proc(c"vkSignalSemaphore")) };
            let ty = vk::VkSemaphoreTypeCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO,
                semaphoreType: vk::VkSemaphoreType_VK_SEMAPHORE_TYPE_TIMELINE,
                ..Default::default()
            };
            let info = vk::VkSemaphoreCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
                pNext: (&ty as *const vk::VkSemaphoreTypeCreateInfo).cast(),
                ..Default::default()
            };
            let mut semaphore = ptr::null_mut();
            assert_eq!(
                unsafe {
                    (device.f.vkCreateSemaphore.unwrap())(
                        device.handle,
                        &info,
                        ptr::null(),
                        &mut semaphore,
                    )
                },
                vk::VkResult_VK_SUCCESS
            );
            let mut gate = Gate {
                device: device.clone(),
                semaphore,
                signal,
                opened: false,
                completions: Vec::new(),
                caller_owner: None,
            };
            GATE.set(semaphore);
            CALLS.set(0);
            POLL_STATUS.set(vk::VkResult_VK_SUCCESS);
            let buffer = Rc::new(Buffer::new(device.clone(), 12).unwrap());
            gate.caller_owner = Some(buffer.clone());
            buffer
                .write(
                    0,
                    &[1u32, 2, 0xa5a5a5a5]
                        .into_iter()
                        .flat_map(u32::to_ne_bytes)
                        .collect::<Vec<_>>(),
                )
                .unwrap();
            let address = buffer.address().unwrap();
            let weak = Rc::downgrade(&buffer);
            let kernel = Rc::new(unsafe { Kernel::new(device.clone(), &words, 16).unwrap() });
            let mut discarded = Batch::new(device.clone()).unwrap();
            let temporary = Rc::new(Buffer::new(device.clone(), 4).unwrap());
            let temporary_weak = Rc::downgrade(&temporary);
            discarded.retain_buffer(temporary).unwrap();
            assert!(temporary_weak.upgrade().is_some());
            let foreign = Device::new(instance.clone(), physical).unwrap();
            assert_eq!(
                discarded
                    .retain_buffer(Rc::new(Buffer::new(foreign, 4).unwrap()))
                    .unwrap_err()
                    .status,
                INVALID_ARGUMENT
            );
            drop(discarded);
            assert!(temporary_weak.upgrade().is_none());
            for slot in [0, 1, 0] {
                if gate.completions.len() == 2 {
                    // The second submission cannot complete before we open the gate.
                    assert!(!gate.completions[1].poll().unwrap());
                    assert!(gate.completions[1].pending);
                    gate.completions[0].wait().unwrap();
                    assert!(gate.completions[0].poll().unwrap());
                    // Retire range 0 while range 1 remains unavailable. No host copies
                    // touch this shared allocation until all submissions finish.
                }
                let mut batch = Batch::new(device.clone()).unwrap();
                if device.timing_info().is_ok() {
                    batch.enable_timing().unwrap();
                }
                if assisted {
                    batch.retain_buffer(buffer.clone()).unwrap();
                    batch.retain_buffer(buffer.clone()).unwrap();
                    assert_eq!(batch.retained.len(), 1);
                }
                batch
                    .barrier(COMPUTE_READ | COMPUTE_WRITE, COMPUTE_READ | COMPUTE_WRITE)
                    .unwrap();
                let mut root = [0; 16];
                root[..8].copy_from_slice(&(address + slot * 4).to_ne_bytes());
                root[8..12].copy_from_slice(&1u32.to_ne_bytes());
                batch.dispatch(kernel.clone(), [1, 1, 1], &root).unwrap();
                gate.completions.push(unsafe { batch.submit().unwrap() });
                assert_eq!(
                    batch.retain_buffer(buffer.clone()).unwrap_err().status,
                    INVALID_ARGUMENT
                );
            }
            drop(buffer);
            if assisted {
                gate.caller_owner = None;
            }
            assert!(weak.upgrade().is_some());
            POLL_STATUS.set(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY);
            assert!(gate.completions[1].poll().is_err());
            assert!(gate.completions[1].pending && gate.completions[1].outcome.is_none());
            POLL_STATUS.set(vk::VkResult_VK_SUCCESS);
            assert!(!gate.completions[1].poll().unwrap());
            gate.open();
            gate.completions[2].wait().unwrap();
            assert!(gate.completions[1].poll().unwrap());
            if gate.completions[1].timed {
                assert!(gate.completions[1].elapsed_ns().unwrap().is_finite());
            }
            let mut output = [0u32; 3];
            unsafe {
                weak.upgrade()
                    .unwrap()
                    .read(0, output.as_mut_ptr().cast(), 12)
                    .unwrap()
            };
            assert_eq!(output, [37, 13, 0xa5a5a5a5]);
            gate.completions.clear();
            assert_eq!(weak.upgrade().is_some(), !assisted);
            gate.caller_owner = None;
            assert!(weak.upgrade().is_none());
            // Report simulated loss only after all real work has drained.
            let mut lost = unsafe { Batch::new(device.clone()).unwrap().submit().unwrap() };
            assert_eq!(
                unsafe { (device.f.vkQueueWaitIdle.unwrap())(device.queue) },
                vk::VkResult_VK_SUCCESS
            );
            POLL_STATUS.set(vk::VkResult_VK_ERROR_DEVICE_LOST);
            assert_eq!(
                lost.poll().unwrap_err().vk,
                vk::VkResult_VK_ERROR_DEVICE_LOST
            );
            assert!(!lost.pending && device.lost.get());
            assert!(lost.poll().is_err() && lost.wait().is_err());
            POLL_STATUS.set(vk::VkResult_VK_SUCCESS);
            println!("Gated scratch reuse passed: assisted={assisted}, range 0 reused while range 1 pending");
        }
        tested += 1;
    }
    assert!(tested > 0, "No execution-capable Vulkan device found");
}
