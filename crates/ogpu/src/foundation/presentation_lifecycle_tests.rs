//! Inject WSI only: real device/sync identities, no display or native WSI calls.
use super::*;
use crate::foundation::{ogpu_next_presentation_api, rust};
use std::cell::RefCell;

#[test]
fn native_presentation_effects() {
    for native in [
        vk::VkResult_VK_SUCCESS,
        vk::VkResult_VK_SUBOPTIMAL_KHR,
        vk::VkResult_VK_ERROR_OUT_OF_DATE_KHR,
        vk::VkResult_VK_ERROR_SURFACE_LOST_KHR,
    ] {
        assert_eq!(batch_effect(native), EFFECT_ENQUEUED);
        for fence in [false, true] {
            let item = item_result(native, native, fence);
            assert_eq!(item.frame_effect, FRAME_RELEASED);
            assert_eq!(
                item.fence_effect,
                if fence { EFFECT_ENQUEUED } else { EFFECT_NONE }
            );
            assert_eq!(item.status, status(native).err().unwrap_or(OK));
        }
    }
    for oom in [
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
        vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
    ] {
        assert_eq!(batch_effect(oom), EFFECT_NONE);
        for item in [
            vk::VkResult_VK_SUCCESS,
            vk::VkResult_VK_ERROR_UNKNOWN,
            vk::VkResult_VK_ERROR_DEVICE_LOST,
        ] {
            assert_eq!(
                item_result(oom, item, true),
                PresentResult {
                    status: OUT_OF_MEMORY,
                    frame_effect: FRAME_RETAINED,
                    fence_effect: EFFECT_NONE,
                    reserved: 0,
                }
            );
        }
        assert_eq!(
            item_result(vk::VkResult_VK_ERROR_UNKNOWN, oom, true).frame_effect,
            FRAME_UNKNOWN
        );
    }
    for unknown in [
        vk::VkResult_VK_ERROR_UNKNOWN,
        vk::VkResult_VK_ERROR_DEVICE_LOST,
    ] {
        assert_eq!(batch_effect(unknown), EFFECT_UNKNOWN);
        assert_eq!(
            item_result(unknown, unknown, true).fence_effect,
            EFFECT_UNKNOWN
        );
        assert_eq!(
            item_result(unknown, unknown, false).fence_effect,
            EFFECT_NONE
        );
        assert_eq!(
            item_result(unknown, unknown, false).frame_effect,
            FRAME_UNKNOWN
        );
    }
}

fn api() -> PresentationApi {
    let mut table = std::mem::MaybeUninit::uninit();
    unsafe {
        assert_eq!(
            ogpu_next_presentation_api(
                VERSION,
                size_of::<PresentationApi>() as u32,
                table.as_mut_ptr()
            ),
            OK
        );
        table.assume_init()
    }
}
unsafe fn present_call(
    queue: &Queue,
    desc: &PresentDesc,
    out: &mut PresentOutcome,
) -> Result<(), Status> {
    let result =
        unsafe { api().queue_present.unwrap()(ptr::from_ref(queue).cast_mut(), desc, out) };
    if result == OK {
        Ok(())
    } else {
        Err(result)
    }
}
unsafe fn acquire_call(
    chain: &Swapchain,
    desc: &AcquireDesc,
    out: &mut Frame,
) -> Result<(), Status> {
    let result = unsafe { api().acquire.unwrap()(ptr::from_ref(chain).cast_mut(), desc, out) };
    if result == OK {
        Ok(())
    } else {
        Err(result)
    }
}

unsafe fn create_call(
    d: &Device,
    record: &Record,
    out: &mut ChainCreateOutcome,
) -> Result<(), Status> {
    let result = unsafe { api().chain_create.unwrap()(ptr::from_ref(d).cast_mut(), record, out) };
    if result == OK {
        Ok(())
    } else {
        Err(result)
    }
}
unsafe fn abandon_call(
    chain: &Swapchain,
    frames: &[Frame],
    scratch: HostSpan,
    effect: &mut u32,
) -> Result<(), Status> {
    let result = unsafe {
        api().abandon.unwrap()(
            ptr::from_ref(chain).cast_mut(),
            frames.len() as u32,
            frames.as_ptr(),
            scratch,
            effect,
        )
    };
    if result == OK {
        Ok(())
    } else {
        Err(result)
    }
}

#[derive(Default)]
struct Trace {
    result: vk::VkResult,
    item_results: Option<[vk::VkResult; 2]>,
    acquire_index: u32,
    calls: u32,
    destroyed: u32,
    chains: Vec<usize>,
    indices: Vec<u32>,
    waits: Vec<usize>,
    fences: Vec<usize>,
    enumerate_result: vk::VkResult,
}
thread_local! { static TRACE: RefCell<Trace> = RefCell::new(Trace::default()); }
fn reset(result: vk::VkResult) {
    TRACE.with_borrow_mut(|t| {
        *t = Trace {
            result,
            ..Default::default()
        }
    });
}
fn calls() -> u32 {
    TRACE.with_borrow(|t| t.calls)
}
unsafe extern "C" fn destroy(
    _: vk::VkDevice,
    _: vk::VkSwapchainKHR,
    _: *const vk::VkAllocationCallbacks,
) {
    TRACE.with_borrow_mut(|t| t.destroyed += 1);
}
unsafe extern "C" fn acquire(
    _: vk::VkDevice,
    _: vk::VkSwapchainKHR,
    _: u64,
    _: vk::VkSemaphore,
    _: vk::VkFence,
    out: *mut u32,
) -> vk::VkResult {
    TRACE.with_borrow_mut(|t| {
        t.calls += 1;
        unsafe {
            out.write(t.acquire_index);
        }
        t.result
    })
}
unsafe extern "C" fn release(
    _: vk::VkDevice,
    info: *const vk::VkReleaseSwapchainImagesInfoKHR,
) -> vk::VkResult {
    TRACE.with_borrow_mut(|t| {
        t.calls += 1;
        let info = unsafe { &*info };
        t.indices = unsafe {
            std::slice::from_raw_parts(info.pImageIndices, info.imageIndexCount as usize)
        }
        .to_vec();
        t.result
    })
}
unsafe extern "C" fn present(_: vk::VkQueue, info: *const vk::VkPresentInfoKHR) -> vk::VkResult {
    TRACE.with_borrow_mut(|t| {
        t.calls += 1;
        let info = unsafe { &*info };
        let count = info.swapchainCount as usize;
        t.chains = unsafe { std::slice::from_raw_parts(info.pSwapchains, count) }
            .iter()
            .map(|p| *p as usize)
            .collect();
        t.indices = unsafe { std::slice::from_raw_parts(info.pImageIndices, count) }.to_vec();
        t.waits = if info.waitSemaphoreCount == 0 {
            Vec::new()
        } else {
            unsafe {
                std::slice::from_raw_parts(info.pWaitSemaphores, info.waitSemaphoreCount as usize)
            }
            .iter()
            .map(|p| *p as usize)
            .collect()
        };
        let fences = unsafe { &*info.pNext.cast::<vk::VkSwapchainPresentFenceInfoKHR>() };
        assert_eq!(fences.swapchainCount, info.swapchainCount);
        t.fences = unsafe { std::slice::from_raw_parts(fences.pFences, count) }
            .iter()
            .map(|p| *p as usize)
            .collect();
        if let Some(results) = t.item_results {
            for (i, value) in results.iter().enumerate().take(count) {
                unsafe {
                    info.pResults.add(i).write(*value);
                }
            }
        }
        t.result
    })
}
unsafe extern "C" fn native_create(
    _: vk::VkDevice,
    _: *const vk::VkSwapchainCreateInfoKHR,
    _: *const vk::VkAllocationCallbacks,
    out: *mut vk::VkSwapchainKHR,
) -> vk::VkResult {
    TRACE.with_borrow_mut(|t| {
        t.calls += 1;
        // Deliberately scribble even on failure; that handle must not be adopted.
        unsafe {
            out.write(ptr::dangling_mut());
        }
        t.result
    })
}
unsafe extern "C" fn images(
    _: vk::VkDevice,
    _: vk::VkSwapchainKHR,
    count: *mut u32,
    images: *mut vk::VkImage,
) -> vk::VkResult {
    TRACE.with_borrow(|t| {
        unsafe {
            if !images.is_null() {
                images.write(ptr::dangling_mut());
            }
            count.write(1);
        }
        t.enumerate_result
    })
}
fn chain(d: &Device, domain: u32, handle: usize) -> Swapchain {
    let desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: 3,
        dimension: 2,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent { x: 8, y: 8, z: 1 },
        usage: 18,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domain_count: 0,
        concurrent_domains: ptr::null(),
    };
    Swapchain {
        device: d,
        surface: ptr::null(),
        handle: handle as vk::VkSwapchainKHR,
        f: Functions {
            vkCreateSwapchainKHR: Some(native_create),
            vkDestroySwapchainKHR: Some(destroy),
            vkGetSwapchainImagesKHR: Some(images),
            vkAcquireNextImageKHR: Some(acquire),
            vkReleaseSwapchainImagesKHR: Some(release),
            vkQueuePresentKHR: Some(present),
        },
        images: (0..2)
            .map(|_| {
                Image::from_swapchain(d, ptr::dangling_mut(), unsafe {
                    d.prepare_image(&desc).unwrap()
                })
            })
            .collect(),
        present_domains: vec![domain],
        generation: 1,
        state: Cell::new(CHAIN_ACTIVE),
    }
}

#[test]
#[ignore = "requires modern Vulkan; injected WSI lifecycle effects and failure atomicity"]
fn gpu_presentation_lifecycle() {
    let discovery = rust::discover().unwrap();
    let adapter = rust::adapters(&discovery)
        .iter()
        .find(|a| rust::adapter_features(a).baseline_supported != 0)
        .unwrap();
    let domain = rust::adapter_queues(adapter)
        .iter()
        .find(|q| q.flags & GRAPHICS != 0)
        .unwrap()
        .domain;
    let request = [QueueRequest {
        domain,
        count: 1,
        priority: 1.0,
    }];
    let mut d = Device::create(adapter, &request, FeatureBits::ZERO).unwrap();
    d.snapshot.features.enabled |= PRESENTATION;
    let foreign = Device::create(adapter, &request, FeatureBits::ZERO).unwrap();
    let binary = Binary::create(&d).unwrap();
    let foreign_binary = Binary::create(&foreign).unwrap();
    let fence = Fence::create(&d, false).unwrap();
    let foreign_fence = Fence::create(&foreign, false).unwrap();
    let a = chain(&d, domain, 1);
    let b = chain(&d, domain, 2);
    let queue = d.queue(domain, 0).unwrap();
    let acquire_desc = AcquireDesc {
        timeout_ns: 0,
        binary: ptr::from_ref(&*binary).cast_mut(),
        fence: ptr::from_ref(&*fence).cast_mut(),
    };
    let sentinel = a.frame(1, &acquire_desc);
    let mut frame = sentinel;
    unsafe {
        for (native, expected) in [
            (vk::VkResult_VK_TIMEOUT, TIMEOUT),
            (vk::VkResult_VK_NOT_READY, NOT_READY),
            (vk::VkResult_VK_ERROR_OUT_OF_DATE_KHR, OUT_OF_DATE),
            (vk::VkResult_VK_ERROR_SURFACE_LOST_KHR, SURFACE_LOST),
            (vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY, OUT_OF_MEMORY),
        ] {
            reset(native);
            assert_eq!(acquire_call(&a, &acquire_desc, &mut frame), Err(expected));
            assert_eq!(frame, sentinel);
            assert_eq!(calls(), 1);
        }
        reset(vk::VkResult_VK_SUCCESS);
        for invalid in [
            AcquireDesc {
                binary: ptr::null_mut(),
                fence: ptr::null_mut(),
                ..acquire_desc
            },
            AcquireDesc {
                binary: ptr::from_ref(&*foreign_binary).cast_mut(),
                ..acquire_desc
            },
            AcquireDesc {
                fence: ptr::from_ref(&*foreign_fence).cast_mut(),
                ..acquire_desc
            },
        ] {
            assert_eq!(acquire_call(&a, &invalid, &mut frame), Err(INVALID));
            assert_eq!(frame, sentinel);
        }
        assert_eq!(calls(), 0);
        for native in [vk::VkResult_VK_SUCCESS, vk::VkResult_VK_SUBOPTIMAL_KHR] {
            reset(native);
            assert_eq!(acquire_call(&a, &acquire_desc, &mut frame), status(native));
            assert_eq!(frame, a.frame(0, &acquire_desc));
        }
        // C output-only storage need not contain a Rust value before the call.
        let mut fresh = std::mem::MaybeUninit::<Frame>::uninit();
        reset(vk::VkResult_VK_SUCCESS);
        assert_eq!(
            api().acquire.unwrap()(
                ptr::from_ref(&a).cast_mut(),
                &acquire_desc,
                fresh.as_mut_ptr()
            ),
            OK
        );
        assert_eq!(fresh.assume_init(), a.frame(0, &acquire_desc));
        let mut info = std::mem::MaybeUninit::<ChainInfo>::uninit();
        api().chain_info_query.unwrap()(&a, info.as_mut_ptr());
        assert_eq!(info.assume_init(), a.info());
        let mut resize = std::mem::MaybeUninit::<ChainResizeOutcome>::uninit();
        assert_eq!(
            api().chain_resize.unwrap()(
                ptr::from_ref(&a).cast_mut(),
                ptr::null(),
                resize.as_mut_ptr()
            ),
            INVALID
        );
        assert_eq!(resize.assume_init().image_effect, IMAGES_UNCHANGED);
        let mut pacing = std::mem::MaybeUninit::<PacingHandle>::uninit();
        assert_eq!(
            api().chain_pacing_handle.unwrap()(&a, pacing.as_mut_ptr()),
            UNSUPPORTED
        );
        let mut requirements = std::mem::MaybeUninit::<HostRequirements>::uninit();
        assert_eq!(
            crate::foundation::ogpu_next_submit_scratch_requirements(
                &*d,
                0,
                ptr::null(),
                requirements.as_mut_ptr()
            ),
            OK
        );
        assert_eq!(requirements.assume_init().size, 0);
        assert_eq!(
            crate::foundation::ogpu_next_split_scratch_requirements(
                &*d,
                0,
                ptr::null(),
                requirements.as_mut_ptr()
            ),
            OK
        );
        assert_eq!(requirements.assume_init().size, 0);
        // Historical acquisition handles are not live inputs to present/abandon.
        frame.binary = ptr::dangling_mut();
        frame.fence = ptr::dangling_mut();
        let mut items = [
            PresentItem {
                frame,
                retire_fence: acquire_desc.fence,
                sync_interval: 0,
                native_flags: 0,
            },
            PresentItem {
                frame: b.frame(1, &acquire_desc),
                retire_fence: ptr::null_mut(),
                sync_interval: 0,
                native_flags: 0,
            },
        ];
        let waits = [acquire_desc.binary];
        let req = present_requirements(&d, 1, 2).unwrap();
        let mut storage = vec![0u64; req.size.div_ceil(8) as usize];
        let result_sentinel = PresentResult {
            status: 99,
            frame_effect: 99,
            fence_effect: 99,
            reserved: 99,
        };
        let mut results = [result_sentinel; 2];
        let mut desc = PresentDesc {
            wait_count: 1,
            item_count: 2,
            waits: waits.as_ptr(),
            items: items.as_ptr(),
            results: results.as_mut_ptr(),
            scratch: HostSpan {
                data: storage.as_mut_ptr().cast(),
                size: req.size,
            },
        };
        let none = PresentOutcome {
            work_effect: EFFECT_NONE,
            wait_effect: EFFECT_NONE,
        };
        let mut outcome = PresentOutcome {
            work_effect: 99,
            wait_effect: 99,
        };
        reset(vk::VkResult_VK_SUCCESS);
        // The invalid last entry must prevent the entire batch and preserve results.
        let good = items[1];
        for bad_frame in [
            Frame {
                generation: 2,
                ..good.frame
            },
            Frame {
                image: frame.image,
                ..good.frame
            },
            Frame {
                index: 2,
                ..good.frame
            },
            Frame {
                chain: ptr::null_mut(),
                ..good.frame
            },
            Frame {
                availability: AVAILABILITY_QUEUE_ORDERED,
                ..good.frame
            },
            Frame {
                ordered_queue: ptr::dangling_mut(),
                ..good.frame
            },
        ] {
            items[1].frame = bad_frame;
            desc.items = items.as_ptr();
            assert_eq!(present_call(queue, &desc, &mut outcome), Err(INVALID));
            assert_eq!(outcome, none);
            assert_eq!(results, [result_sentinel; 2]);
        }
        items[1] = PresentItem {
            native_flags: 1,
            ..good
        };
        desc.items = items.as_ptr();
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(UNSUPPORTED));
        items[1] = PresentItem {
            retire_fence: ptr::from_ref(&*foreign_fence).cast_mut(),
            ..good
        };
        desc.items = items.as_ptr();
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(INVALID));
        items[1] = good;
        desc.items = items.as_ptr();
        desc.scratch.size = req.size - 1;
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(CAPACITY));
        desc.scratch.size = req.size;
        desc.results = ptr::null_mut();
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(INVALID));
        desc.results = results.as_mut_ptr();
        desc.item_count = 0;
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(INVALID));
        desc.item_count = 2;
        let data = desc.scratch.data;
        desc.scratch.data = data.cast::<u8>().add(1).cast();
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(INVALID));
        desc.scratch.data = data;
        let foreign_wait = [ptr::from_ref(&*foreign_binary).cast_mut()];
        desc.waits = foreign_wait.as_ptr();
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(INVALID));
        desc.waits = waits.as_ptr();
        let foreign_chain = chain(&foreign, domain, 3);
        items[1].frame = foreign_chain.frame(0, &acquire_desc);
        desc.items = items.as_ptr();
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(INVALID));
        let mut wrong_domain = chain(&d, domain, 4);
        wrong_domain.present_domains.clear();
        items[1].frame = wrong_domain.frame(0, &acquire_desc);
        desc.items = items.as_ptr();
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(UNSUPPORTED));
        items[1] = good;
        desc.items = items.as_ptr();
        assert_eq!(outcome, none);
        assert_eq!(results, [result_sentinel; 2]);
        assert_eq!(calls(), 0);
        for native in [
            vk::VkResult_VK_SUCCESS,
            vk::VkResult_VK_SUBOPTIMAL_KHR,
            vk::VkResult_VK_ERROR_OUT_OF_DATE_KHR,
            vk::VkResult_VK_ERROR_SURFACE_LOST_KHR,
            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
            vk::VkResult_VK_ERROR_UNKNOWN,
        ] {
            reset(native);
            TRACE.with_borrow_mut(|t| t.item_results = Some([vk::VkResult_VK_SUCCESS, native]));
            assert_eq!(present_call(queue, &desc, &mut outcome), status(native));
            assert_eq!(
                outcome,
                PresentOutcome {
                    work_effect: batch_effect(native),
                    wait_effect: batch_effect(native)
                }
            );
            assert_eq!(calls(), 1);
            assert_eq!(
                results[0],
                item_result(native, vk::VkResult_VK_SUCCESS, true)
            );
            assert_eq!(results[1], item_result(native, native, false));
            TRACE.with_borrow(|t| {
                assert_eq!(t.chains, [1, 2]);
                assert_eq!(t.indices, [0, 1]);
                assert_eq!(t.waits, [binary.handle as usize]);
                assert_eq!(t.fences, [fence.handle as usize, 0]);
            });
        }
        reset(vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY);
        TRACE.with_borrow_mut(|t| t.item_results = Some([vk::VkResult_VK_ERROR_DEVICE_LOST; 2]));
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(OUT_OF_MEMORY));
        assert!(!d.lost.load(Ordering::Relaxed));
        assert_eq!(results[0].frame_effect, FRAME_RETAINED);
        reset(vk::VkResult_VK_SUCCESS); // Driver leaves every pResults entry untouched.
        desc.wait_count = 0;
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(BACKEND_ERROR));
        assert_eq!(outcome.wait_effect, EFFECT_NONE);
        assert_eq!(results[0].frame_effect, FRAME_UNKNOWN);
        assert_eq!(results[0].fence_effect, EFFECT_UNKNOWN);
        assert_eq!(results[1].fence_effect, EFFECT_NONE);
        reset(vk::VkResult_VK_ERROR_SURFACE_LOST_KHR);
        TRACE.with_borrow_mut(|t| t.item_results = Some([vk::VkResult_VK_SUCCESS; 2]));
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(SURFACE_LOST));
        // Retiring prevents acquisition, but does not revoke acquired images.
        a.state.set(CHAIN_RETIRED);
        reset(vk::VkResult_VK_SUCCESS);
        assert_eq!(acquire_call(&a, &acquire_desc, &mut frame), Err(INVALID));
        assert_eq!(calls(), 0);
        TRACE.with_borrow_mut(|t| t.item_results = Some([vk::VkResult_VK_SUCCESS; 2]));
        assert_eq!(present_call(queue, &desc, &mut outcome), Ok(()));
        let release_frames = [frame, a.frame(1, &acquire_desc)];
        let mut indices = [0u32; 2];
        let release_storage = HostSpan {
            data: indices.as_mut_ptr().cast(),
            size: 8,
        };
        let mut effect = 99;
        reset(vk::VkResult_VK_SUCCESS);
        assert_eq!(
            abandon_call(&a, &[], release_storage, &mut effect),
            Err(INVALID)
        );
        assert_eq!(
            abandon_call(&a, &[frame, good.frame], release_storage, &mut effect),
            Err(INVALID)
        );
        assert_eq!(effect, FRAME_RETAINED);
        assert_eq!(calls(), 0);
        assert_eq!(
            abandon_call(&a, &release_frames, release_storage, &mut effect),
            Ok(())
        );
        assert_eq!(effect, FRAME_RELEASED);
        assert_eq!(calls(), 1);
        TRACE.with_borrow(|t| assert_eq!(t.indices, [0, 1]));
        reset(vk::VkResult_VK_ERROR_UNKNOWN);
        assert_eq!(
            abandon_call(&a, &release_frames, release_storage, &mut effect),
            Err(BACKEND_ERROR)
        );
        assert_eq!(effect, FRAME_UNKNOWN);
        let before = a.info();
        let mut resize = ChainResizeOutcome {
            generation: 99,
            state: 99,
            image_effect: 99,
        };
        assert_eq!(
            a.resize(
                &ChainResizeDesc {
                    format: 0,
                    extent: [0; 2],
                    image_count: 0,
                    chain_flags: 0,
                    reserved: 0
                },
                &mut resize
            ),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            resize,
            ChainResizeOutcome {
                generation: 1,
                state: CHAIN_RETIRED,
                image_effect: IMAGES_UNCHANGED
            }
        );
        assert_eq!(a.info(), before);
        let mut pacing = PacingHandle {
            kind: 99,
            reserved: 99,
            handle: ptr::dangling_mut(),
        };
        let saved = pacing;
        assert_eq!(a.pacing_handle(&mut pacing), Err(UNSUPPORTED));
        assert_eq!(pacing, saved);
        reset(vk::VkResult_VK_ERROR_DEVICE_LOST);
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(DEVICE_LOST));
        assert_eq!(
            outcome,
            PresentOutcome {
                work_effect: EFFECT_UNKNOWN,
                wait_effect: EFFECT_NONE
            }
        );
        assert_eq!(results[0].frame_effect, FRAME_UNKNOWN);
        assert_eq!(results[0].fence_effect, EFFECT_UNKNOWN);
        assert!(d.lost.load(Ordering::Relaxed));
        assert_eq!(calls(), 1);
        results.fill(result_sentinel);
        assert_eq!(present_call(queue, &desc, &mut outcome), Err(DEVICE_LOST));
        assert_eq!(outcome, none);
        assert_eq!(results, [result_sentinel; 2]);
        assert_eq!(calls(), 1);
    }
}

#[test]
#[ignore = "requires modern Vulkan; injected native replacement retirement and adoption failures"]
fn gpu_presentation_replacement() {
    let discovery = rust::discover().unwrap();
    let adapter = rust::adapters(&discovery)
        .iter()
        .find(|a| rust::adapter_features(a).baseline_supported != 0)
        .unwrap();
    let domain = rust::adapter_queues(adapter)
        .iter()
        .find(|q| q.flags & GRAPHICS != 0)
        .unwrap()
        .domain;
    let mut d = Device::create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        FeatureBits::ZERO,
    )
    .unwrap();
    d.snapshot.features.enabled |= PRESENTATION;
    let old = chain(&d, domain, 1);
    let mut out = ChainCreateOutcome {
        chain: ptr::dangling_mut(),
        previous_state: 99,
        reserved: 99,
    };
    unsafe {
        assert_eq!(
            create_call(&d, &Record::new::<Record>(u32::MAX), &mut out),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            out,
            ChainCreateOutcome {
                chain: ptr::null_mut(),
                previous_state: CHAIN_NONE,
                reserved: 0
            }
        );
        assert_eq!(
            create_call(&d, &Record::new::<Record>(SWAPCHAIN_DESC), &mut out),
            Err(INVALID)
        );
        assert_eq!(out.previous_state, CHAIN_NONE);
        let mut desc: SwapchainDesc = std::mem::zeroed();
        desc.header = Record::new::<SwapchainDesc>(SWAPCHAIN_DESC);
        desc.old_swapchain = ptr::from_ref(&old).cast_mut();
        reset(vk::VkResult_VK_SUCCESS);
        assert_eq!(create_call(&d, &desc.header, &mut out), Err(INVALID)); // Null surface.
        assert_eq!(out.previous_state, CHAIN_ACTIVE);
        assert!(out.chain.is_null());
        assert_eq!(old.info().state, CHAIN_ACTIVE);
        assert_eq!(calls(), 0);
        let win32 = Win32SurfaceDesc {
            header: Record::new::<Win32SurfaceDesc>(WIN32_SURFACE_DESC),
            hwnd: ptr::dangling_mut(),
        };
        let mut surface = ptr::dangling_mut();
        assert_eq!(
            api().surface_win32_create.unwrap()(adapter, &win32, &mut surface),
            UNSUPPORTED
        );
        assert!(surface.is_null());
        let mut dxgi: DxgiChainDesc = std::mem::zeroed();
        dxgi.header = Record::new::<DxgiChainDesc>(DXGI_CHAIN_DESC);
        assert_eq!(create_call(&d, &dxgi.header, &mut out), Err(UNSUPPORTED));
        assert_eq!(out, ChainCreateOutcome::default());
        for (native, enumeration, expected, destroys) in [
            (
                vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
                vk::VkResult_VK_SUCCESS,
                Err(OUT_OF_MEMORY),
                0,
            ),
            (
                vk::VkResult_VK_SUCCESS,
                vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
                Err(OUT_OF_MEMORY),
                1,
            ),
            (vk::VkResult_VK_SUCCESS, vk::VkResult_VK_SUCCESS, Ok(()), 1),
        ] {
            old.state.set(CHAIN_ACTIVE);
            let mut new = chain(&d, domain, 0);
            new.images.clear();
            let prepared_desc = ImageDesc {
                header: Record::new::<ImageDesc>(IMAGE_DESC),
                format: 3,
                dimension: 2,
                mip_count: 1,
                layer_count: 1,
                sample_count: 1,
                extent: Extent { x: 8, y: 8, z: 1 },
                usage: 18,
                flags: 0,
                view_formats: ptr::null(),
                view_format_count: 0,
                concurrent_domain_count: 0,
                concurrent_domains: ptr::null(),
            };
            let prepared = d.prepare_image(&prepared_desc).unwrap();
            reset(native);
            TRACE.with_borrow_mut(|t| t.enumerate_result = enumeration);
            let info = vk::VkSwapchainCreateInfoKHR {
                oldSwapchain: old.handle,
                ..Default::default()
            };
            assert_eq!(
                new.create_native(&d, &info, Some(&old), |h| Image::from_swapchain(
                    &d,
                    h,
                    prepared.clone()
                )),
                expected
            );
            assert_eq!(calls(), 1);
            assert_eq!(old.info().state, CHAIN_RETIRED);
            if expected.is_ok() {
                assert_eq!(new.info().image_count, 1);
                assert_eq!(new.info().generation, 1);
            }
            drop(new);
            TRACE.with_borrow(|t| assert_eq!(t.destroyed, destroys));
        }
        // Invalid successful native acquisition cannot publish a bogus image.
        old.state.set(CHAIN_ACTIVE);
        let binary = Binary::create(&d).unwrap();
        let acquire_desc = AcquireDesc {
            timeout_ns: 0,
            binary: ptr::from_ref(&*binary).cast_mut(),
            fence: ptr::null_mut(),
        };
        let mut frame = old.frame(0, &acquire_desc);
        let saved = frame;
        reset(vk::VkResult_VK_SUCCESS);
        TRACE.with_borrow_mut(|t| t.acquire_index = 99);
        assert_eq!(
            acquire_call(&old, &acquire_desc, &mut frame),
            Err(BACKEND_ERROR)
        );
        assert_eq!(frame, saved);
        assert!(d.lost.load(Ordering::Relaxed));
    }
}
