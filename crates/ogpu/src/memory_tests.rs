use super::*;
use batch::{COMPUTE_READ, COMPUTE_WRITE, TRANSFER_READ, TRANSFER_WRITE};

#[test]
fn placement_selection_handles_discrete_uma_and_exclusions() {
    let host = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
    let local = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
    let coherent = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
    let cached = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_CACHED_BIT;
    let protected = vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_PROTECTED_BIT;
    let mut memory = vk::VkPhysicalDeviceMemoryProperties {
        memoryTypeCount: 4,
        ..Default::default()
    };
    for (i, flags) in [
        local,
        host | coherent | cached,
        local | host | coherent,
        local | protected,
    ]
    .into_iter()
    .enumerate()
    {
        memory.memoryTypes[i].propertyFlags = flags;
    }
    assert_eq!(memory_type(&memory, 15, Placement::Host), Some((1, true)));
    assert_eq!(
        memory_type(&memory, 15, Placement::Device),
        Some((0, false))
    );
    assert_eq!(memory_type(&memory, 2, Placement::Device), None);
    assert_eq!(memory_type(&memory, 1, Placement::Host), None);
    assert_eq!(memory_type(&memory, 8, Placement::Device), None);
    // UMA/BAR: the same memory type can implement either access contract.
    assert_eq!(memory_type(&memory, 4, Placement::Host), Some((2, true)));
    assert_eq!(memory_type(&memory, 4, Placement::Device), Some((2, true)));
    memory.memoryTypes[0].propertyFlags |=
        vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD;
    assert_eq!(memory_type(&memory, 1, Placement::Device), None);
}

thread_local! {
    static SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    static FAIL: Cell<bool> = const { Cell::new(false) };
}
unsafe extern "C" fn submit(
    q: vk::VkQueue,
    count: u32,
    info: *const vk::VkSubmitInfo2,
    fence: vk::VkFence,
) -> vk::VkResult {
    if FAIL.get() {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    } else {
        unsafe { SUBMIT.get().unwrap()(q, count, info, fence) }
    }
}

#[test]
#[ignore = "requires modern Vulkan; placement, byte copies, compute and retained-copy cleanup"]
fn gpu_buffer_transfers() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let device = match Device::create_configured(instance.clone(), physical, false, |f| {
            SUBMIT.set(f.vkQueueSubmit2);
            FAIL.set(false);
            f.vkQueueSubmit2 = Some(submit);
        }) {
            Ok(d) => d,
            Err(e) if e.status == UNSUPPORTED => continue,
            Err(e) => panic!("{e:?}"),
        };
        let host = Rc::new(Buffer::new(device.clone(), 64).unwrap());
        let local = Rc::new(Buffer::placed(device.clone(), 64, Placement::Device).unwrap());
        assert!(local.mapped.is_null());
        assert_eq!(local.write(0, &[]).unwrap_err().status, INVALID_ARGUMENT);
        let mut unchanged = [91u8; 64];
        assert_eq!(
            unsafe { local.read(0, unchanged.as_mut_ptr(), 1) }
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        assert_eq!(unchanged, [91; 64]);
        host.write(0, &[0x6b; 64]).unwrap();
        let mut batch = Batch::new(device.clone()).unwrap();
        for (so, dst, size, status) in [
            (64, 0, 1, OUT_OF_RANGE),
            (0, 64, 1, OUT_OF_RANGE),
            (usize::MAX, 0, 1, OUT_OF_RANGE),
            (0, 0, usize::MAX, OUT_OF_RANGE),
        ] {
            assert_eq!(
                batch
                    .copy_buffer(host.clone(), so, local.clone(), dst, size)
                    .unwrap_err()
                    .status,
                status
            );
        }
        assert_eq!(
            batch
                .copy_buffer(host.clone(), 0, host.clone(), 1, 4)
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        assert_eq!(
            batch
                .copy_buffer(host.clone(), 1, host.clone(), 0, 4)
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        batch
            .copy_buffer(host.clone(), 64, local.clone(), 64, 0)
            .unwrap();
        assert_eq!(Rc::strong_count(&host), 1);
        let foreign_device = Device::new(instance.clone(), physical).unwrap();
        let foreign = Rc::new(Buffer::new(foreign_device, 64).unwrap());
        assert_eq!(
            batch
                .copy_buffer(host.clone(), 0, foreign.clone(), 0, 1)
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        assert_eq!(
            batch
                .copy_buffer(foreign, 0, host.clone(), 0, 1)
                .unwrap_err()
                .status,
            INVALID_ARGUMENT
        );
        // Unaligned source/destination and odd length; guards and same-allocation copy.
        host.write(1, &[1, 2, 3, 4, 5, 6, 7]).unwrap();
        batch
            .copy_buffer(host.clone(), 1, local.clone(), 3, 7)
            .unwrap();
        batch.barrier(TRANSFER_WRITE, TRANSFER_READ).unwrap();
        batch.barrier(TRANSFER_READ, TRANSFER_WRITE).unwrap();
        batch
            .copy_buffer(local.clone(), 3, host.clone(), 17, 7)
            .unwrap();
        batch.barrier(TRANSFER_WRITE, TRANSFER_READ).unwrap();
        batch
            .copy_buffer(host.clone(), 17, host.clone(), 33, 7)
            .unwrap();
        let mut completion = unsafe { batch.submit() }.unwrap();
        assert!(batch
            .copy_buffer(host.clone(), 0, local.clone(), 0, 0)
            .is_err());
        completion.wait().unwrap();
        unsafe { host.read(0, unchanged.as_mut_ptr(), 64) }.unwrap();
        let mut expected = [0x6b; 64];
        for start in [1, 17, 33] {
            expected[start..start + 7].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7]);
        }
        assert_eq!(unchanged, expected);
        drop(completion);

        // CPU upload -> copy -> compute -> copy -> CPU read, across submissions.
        let words: Vec<_> = include_bytes!("../../../examples/shaders/roundtrip.spv")
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let kernel = Rc::new(unsafe { Kernel::new(device.clone(), &words, 16) }.unwrap());
        let data: Vec<u8> = (0..16u32).flat_map(u32::to_ne_bytes).collect();
        host.write(0, &data).unwrap();
        let mut upload = Batch::new(device.clone()).unwrap();
        upload
            .copy_buffer(host.clone(), 0, local.clone(), 0, 64)
            .unwrap();
        let mut uploaded = unsafe { upload.submit() }.unwrap();
        uploaded.wait().unwrap();
        let mut compute = Batch::new(device.clone()).unwrap();
        compute
            .barrier(TRANSFER_WRITE, COMPUTE_READ | COMPUTE_WRITE)
            .unwrap();
        let mut root = [0; 16];
        root[..8].copy_from_slice(&local.address().unwrap().to_ne_bytes());
        root[8..12].copy_from_slice(&16u32.to_ne_bytes());
        compute.dispatch(kernel, 1, &root).unwrap();
        compute.barrier(COMPUTE_WRITE, TRANSFER_READ).unwrap();
        compute.barrier(TRANSFER_READ, TRANSFER_WRITE).unwrap();
        compute
            .copy_buffer(local.clone(), 0, host.clone(), 0, 64)
            .unwrap();
        let weak = Rc::downgrade(&local);
        drop(local);
        drop(uploaded);
        assert!(weak.upgrade().is_some());
        let mut completed = unsafe { compute.submit() }.unwrap();
        completed.wait().unwrap();
        assert!(weak.upgrade().is_some()); // waiting doesn't retire ownership
        unsafe { host.read(0, unchanged.as_mut_ptr(), 64) }.unwrap();
        for (i, bytes) in unchanged.chunks_exact(4).enumerate() {
            assert_eq!(
                u32::from_ne_bytes(bytes.try_into().unwrap()),
                i as u32 * 3 + 7
            );
        }
        drop(completed);
        assert!(weak.upgrade().is_none());
        assert_eq!(Rc::strong_count(&host), 1);

        // Both endpoints released on discard and unsuccessful submission.
        for fail in [false, true] {
            let other = Rc::new(Buffer::placed(device.clone(), 64, Placement::Device).unwrap());
            let weak = Rc::downgrade(&other);
            let mut copy = Batch::new(device.clone()).unwrap();
            copy.copy_buffer(host.clone(), 0, other, 0, 64).unwrap();
            assert_eq!(Rc::strong_count(&host), 2);
            if fail {
                FAIL.set(true);
                assert!(unsafe { copy.submit() }.is_err());
                FAIL.set(false);
                assert!(weak.upgrade().is_none());
                assert!(unsafe { copy.submit() }.is_err());
            }
            drop(copy);
            assert!(weak.upgrade().is_none());
            assert_eq!(Rc::strong_count(&host), 1);
        }
        tested += 1;
    }
    assert!(tested > 0, "no modern Vulkan device tested");
}
