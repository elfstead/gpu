//! Bounded real Wayland presentation, using only the public native Rust interface.
use super::submission;
use super::{query, query_mode, Window};
use ogpu::foundation::{rust as gpu, types::*};
use std::{mem::ManuallyDrop, ptr};

fn storage(req: HostRequirements) -> Vec<u64> {
    assert!(req.alignment <= 8);
    vec![0; req.size.div_ceil(8) as usize]
}
fn span(storage: &mut [u64]) -> HostSpan {
    HostSpan {
        data: storage.as_mut_ptr().cast(),
        size: std::mem::size_of_val(storage) as u64,
    }
}
unsafe fn images(chain: gpu::SwapchainHandle) -> gpu::Result<Vec<gpu::ImageHandle>> {
    let mut count = 0;
    unsafe {
        gpu::chain_images(chain, &mut [], &mut count)?;
    }
    let mut pointers = vec![ptr::null_mut(); count as usize];
    unsafe {
        gpu::chain_images(chain, &mut pointers, &mut count)?;
    }
    Ok(pointers
        .into_iter()
        .map(|p| unsafe { gpu::ImageHandle::from_raw(p).unwrap() })
        .collect())
}

// Application helper: inspect replacement state even on failure. This sample
// stops on creation failure; it never assumes the old chain can still acquire.
pub(super) unsafe fn create_chain(
    device: gpu::DeviceHandle,
    desc: &SwapchainDesc,
) -> gpu::Result<gpu::SwapchainHandle> {
    let mut out = ChainCreateOutcome::default();
    let result = unsafe { gpu::chain_create(device, &desc.header, &mut out) };
    if let Some(old) = unsafe { gpu::SwapchainHandle::from_raw(desc.old_swapchain) } {
        assert_eq!(
            out.previous_state,
            unsafe { gpu::chain_info_query(old) }.state
        );
        if result.is_ok() {
            assert_eq!(out.previous_state, CHAIN_RETIRED);
        }
    } else {
        assert_eq!(out.previous_state, CHAIN_NONE);
    }
    result?;
    unsafe { gpu::SwapchainHandle::from_raw(out.chain) }.ok_or(BACKEND_ERROR)
}

pub unsafe fn run() -> gpu::Result<()> {
    let discovery = gpu::discover_presentation(PRESENTATION_WAYLAND)?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    // Keep native windows alive on uncertain failure, just like GPU objects.
    // GLFW is process-global: destroy both windows before terminating either owner.
    let mut windows = [
        ManuallyDrop::new(Window::create()),
        ManuallyDrop::new(Window::create()),
    ];
    for window in &windows {
        window.show_and_poll();
    }
    let mut surfaces = Vec::new();
    for window in &windows {
        surfaces.push(unsafe {
            gpu::surface_wayland_create(
                adapter,
                &WaylandSurfaceDesc {
                    header: Record::new::<WaylandSurfaceDesc>(WAYLAND_SURFACE_DESC),
                    display: window.display,
                    surface: window.surface,
                },
            )?
        });
    }
    for surface in surfaces.iter().copied() {
        let contract =
            unsafe { gpu::presentation_contract_query(adapter, surface, PRESENT_VULKAN)? };
        assert_eq!(contract.availability, AVAILABILITY_SIGNALS);
        assert_eq!(
            contract.capabilities,
            PRESENT_REPLACE | PRESENT_RETIRE_FENCE | PRESENT_BINARY_WAIT
        );
        assert!(contract.max_present_count >= 2);
        assert_eq!(
            unsafe { gpu::presentation_contract_query(adapter, surface, PRESENT_DXGI) },
            Err(UNSUPPORTED)
        );
    }
    let queue_support: Vec<SurfaceQueue> = query(adapter, surfaces[0], SURFACE_QUEUES);
    let second_support: Vec<SurfaceQueue> = query(adapter, surfaces[1], SURFACE_QUEUES);
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| {
            q.flags & GRAPHICS != 0
                && queue_support
                    .iter()
                    .any(|p| p.domain == q.domain && p.supported != 0)
                && second_support
                    .iter()
                    .any(|p| p.domain == q.domain && p.supported != 0)
        })
        .ok_or(UNSUPPORTED)?
        .domain;
    let mut required = vec![feature_ids::PRESENTATION];
    if gpu::adapter_features(adapter).available.contains(&feature_ids::UNIFIED_IMAGES) { required.push(feature_ids::UNIFIED_IMAGES); }
    let device = gpu::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        &required,
    )?;
    let queue = unsafe { gpu::device_queue(device, domain, 0).ok_or(INVALID)? };
    let timeline = unsafe { gpu::timeline_create(device, 0)? };
    let arena = unsafe {
        gpu::arena_create(
            device,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            },
        )?
    };
    let acquired = [unsafe { gpu::binary_create(device)? }, unsafe {
        gpu::binary_create(device)?
    }];
    let ready = unsafe { gpu::binary_create(device)? };
    let fences = [unsafe { gpu::fence_create(device, false)? }, unsafe {
        gpu::fence_create(device, false)?
    }];
    let acquisition_fence = unsafe { gpu::fence_create(device, false)? };
    // This bounded two-window probe deliberately selects MAILBOX: a compositor
    // may throttle an occluded FIFO window indefinitely. OGPU has no mode policy.
    let modes = [1];
    let mut descs = Vec::new();
    let mut chains = Vec::new();
    for &surface in &surfaces {
        let caps: Vec<SurfaceCapabilities> =
            query_mode(adapter, surface, Some(modes[0]), SURFACE_CAPABILITIES);
        let caps = caps[0];
        let formats: Vec<SurfaceFormat> = query(adapter, surface, SURFACE_FORMATS);
        let format = formats
            .iter()
            .find(|f| [3, 5].contains(&f.format) && f.color_space == 0)
            .ok_or(UNSUPPORTED)?;
        let extent = [0, 1].map(|i| {
            if caps.current_extent[i] == u32::MAX {
                [320, 240][i].clamp(caps.min_extent[i], caps.max_extent[i])
            } else {
                caps.current_extent[i]
            }
        });
        let desc = SwapchainDesc {
            header: Record::new::<SwapchainDesc>(SWAPCHAIN_DESC),
            surface: surface.as_raw(),
            old_swapchain: ptr::null_mut(),
            format: format.format,
            color_space: format.color_space,
            extent,
            layer_count: 1,
            min_image_count: caps.min_image_count,
            usage: IMAGE_COPY_DST | IMAGE_COLOR_ATTACHMENT,
            transform: caps.current_transform,
            alpha: caps.supported_alpha & caps.supported_alpha.wrapping_neg(),
            present_mode: modes[0],
            clipped: 0,
            domain_count: 0,
            domains: ptr::null(),
            mode_count: 1,
            modes: modes.as_ptr(),
        };
        let mut bad = desc;
        bad.extent[0] = 0;
        assert_eq!(unsafe { create_chain(device, &bad) }, Err(INVALID));
        chains.push(unsafe { create_chain(device, &desc)? });
        descs.push(desc);
    }
    let mut borrowed = [unsafe { images(chains[0])? }, unsafe { images(chains[1])? }];
    let mut submit_storage = storage(gpu::submit_scratch_requirements(
        device,
        &[SubmitDesc {
            list_count: 1,
            wait_count: 0,
            signal_count: 1,
            binary_wait_count: 2,
            binary_signal_count: 1,
            ..Default::default()
        }],
    )?);
    let mut abandon_storage = storage(gpu::abandon_scratch_requirements(device, 1)?);
    let mut present_storage = storage(gpu::present_scratch_requirements(device, 1, 2)?);
    let mut barrier_storage = storage(gpu::barrier_scratch_requirements(device, 0, 0, 1)?);
    let mut value = 0;
    let full = Subresources {
        aspects: ASPECT_COLOR,
        first_mip: 0,
        mip_count: 1,
        first_layer: 0,
        layer_count: 1,
    };
    unsafe {
        // Borrow protection and stable query output, before acquisition/use.
        let mut count = 0;
        let mut short = [ptr::dangling_mut()];
        assert_eq!(
            gpu::chain_images(chains[0], &mut short, &mut count),
            Err(CAPACITY)
        );
        assert_eq!(short[0], ptr::dangling_mut());
        for (i, views) in borrowed.iter().enumerate() {
            for &image in views {
                gpu::image_destroy(image); // Explicit no-op on a swapchain borrow.
                assert_eq!(
                    gpu::image_bind(
                        image,
                        Span {
                            memory: ptr::null_mut(),
                            offset: 0,
                            size: 0
                        }
                    ),
                    Err(INVALID)
                );
                assert_eq!(gpu::memory_create_dedicated_image(image, 0), Err(INVALID));
                let view = gpu::view_create(
                    image,
                    &ViewDesc {
                        header: Record::new::<ViewDesc>(VIEW_DESC),
                        format: 0,
                        dimension: 2,
                        range: full,
                        usage: IMAGE_COLOR_ATTACHMENT as u32,
                        component_mapping: [0; 4],
                    },
                );
                // View format must be explicit, including on borrowed images.
                assert_eq!(view, Err(UNSUPPORTED));
                let view = gpu::view_create(
                    image,
                    &ViewDesc {
                        header: Record::new::<ViewDesc>(VIEW_DESC),
                        format: descs[i].format,
                        dimension: 2,
                        range: full,
                        usage: IMAGE_COLOR_ATTACHMENT as u32,
                        component_mapping: [0; 4],
                    },
                )?;
                gpu::view_destroy(view);
            }
        }
        let mut entries = Vec::with_capacity(2);
        for phase in 0..2 {
            for frame in 0..4 {
                windows[0].poll();
                entries.clear();
                let list = gpu::commands_begin(
                    arena,
                    &RecordingDesc {
                        header: Record::new::<RecordingDesc>(RECORDING_DESC),
                        replay_mode: 1,
                        level: 0,
                        inheritance: ptr::null(),
                    },
                )?;
                for i in 0..2 {
                    let mut acquired_frame = Frame::default();
                    match gpu::acquire(
                        chains[i],
                        &AcquireDesc {
                            timeout_ns: 1_000_000_000,
                            binary: acquired[i].as_raw(),
                            fence: ptr::null_mut(),
                        },
                        &mut acquired_frame,
                    ) {
                        Ok(()) | Err(SUBOPTIMAL) => (),
                        Err(e) => return Err(e),
                    }
                    assert_eq!(acquired_frame.availability, AVAILABILITY_SIGNALS);
                    let image =
                        gpu::ImageHandle::from_raw(acquired_frame.image).ok_or(BACKEND_ERROR)?;
                    assert_eq!(image, borrowed[i][acquired_frame.index as usize]);
                    let mut barrier = ImageBarrier {
                        scope: BarrierScope {
                            stages_before: STAGE_TRANSFER,
                            stages_after: STAGE_TRANSFER,
                            access_before: 0,
                            access_after: ACCESS_COPY_WRITE,
                        },
                        reserved: 0,
                        image: image.as_raw(),
                        range: full,
                        old_state: STATE_UNDEFINED,
                        new_state: STATE_COPY_DST,
                        source_domain: DOMAIN_IGNORED,
                        destination_domain: DOMAIN_IGNORED,
                        discard: 1,
                    };
                    let scratch = span(&mut barrier_storage);
                    let mut dep = Dependency {
                        image_count: 1,
                        images: &barrier,
                        ..Default::default()
                    };
                    gpu::barrier(list, &dep, scratch);
                    gpu::clear_image(
                        list,
                        image,
                        STATE_COPY_DST,
                        full,
                        ClearValue {
                            f32: if (frame + i) % 2 == 0 {
                                [0.1, 0.6, 0.2, 1.0]
                            } else {
                                [0.4, 0.1, 0.7, 1.0]
                            },
                        },
                    );
                    barrier.scope.access_before = ACCESS_COPY_WRITE;
                    barrier.scope.access_after = 0;
                    barrier.old_state = STATE_COPY_DST;
                    barrier.new_state = STATE_PRESENT;
                    barrier.discard = 0;
                    barrier.scope.stages_after = 0;
                    dep.images = &barrier;
                    gpu::barrier(list, &dep, scratch);
                    entries.push(PresentItem {
                        frame: acquired_frame,
                        retire_fence: fences[i].as_raw(),
                        sync_interval: 0,
                        native_flags: 0,
                    });
                }
                gpu::commands_end(list)?;
                value += 1;
                submission::one(
                    queue,
                    &[list],
                    &[],
                    &[SyncPoint {
                        point: Point {
                            timeline: timeline.as_raw(),
                            value,
                        },
                        stages: STAGE_ALL,
                    }],
                    &[
                        acquired[0].at(STAGE_TRANSFER),
                        acquired[1].at(STAGE_TRANSFER),
                    ],
                    &[ready.at(STAGE_ALL)],
                    span(&mut submit_storage),
                )?;
                let waits = [ready.as_raw()];
                let sentinel = PresentResult {
                    status: 99,
                    frame_effect: 99,
                    fence_effect: 99,
                    reserved: 99,
                };
                let mut results = [sentinel; 2];
                let mut outcome = PresentOutcome::default();
                let mut present = PresentDesc {
                    wait_count: 1,
                    item_count: 2,
                    waits: waits.as_ptr(),
                    items: entries.as_ptr(),
                    results: results.as_mut_ptr(),
                    scratch: HostSpan {
                        data: ptr::null_mut(),
                        size: 0,
                    },
                };
                assert_eq!(
                    gpu::queue_present(queue, &present, &mut outcome),
                    Err(CAPACITY)
                );
                assert_eq!(results, [sentinel; 2]);
                assert_eq!(outcome, PresentOutcome::default());
                present.scratch = span(&mut present_storage);
                match gpu::queue_present(queue, &present, &mut outcome) {
                    Ok(()) | Err(SUBOPTIMAL) => (),
                    Err(e) => return Err(e),
                }
                assert_eq!(
                    outcome,
                    PresentOutcome {
                        work_effect: EFFECT_ENQUEUED,
                        wait_effect: EFFECT_ENQUEUED
                    }
                );
                assert!(results.iter().all(|r| [OK, SUBOPTIMAL].contains(&r.status)
                    && r.frame_effect == FRAME_RELEASED
                    && r.fence_effect == EFFECT_ENQUEUED));
                gpu::timeline_wait(timeline, value, 10_000_000_000)?;
                for fence in fences {
                    gpu::fence_wait(fence, 10_000_000_000)?;
                    gpu::fence_reset(fence)?;
                }
                gpu::arena_reset(arena)?;
            }
            // Unused acquisition: fence proves the signal completed; consume binary
            // separately, retire that wait, then release. No implicit reset in release.
            let mut unused = Frame::default();
            match gpu::acquire(
                chains[0],
                &AcquireDesc {
                    timeout_ns: 1_000_000_000,
                    binary: acquired[0].as_raw(),
                    fence: acquisition_fence.as_raw(),
                },
                &mut unused,
            ) {
                Ok(()) | Err(SUBOPTIMAL) => (),
                Err(e) => return Err(e),
            }
            gpu::fence_wait(acquisition_fence, 10_000_000_000)?;
            gpu::fence_reset(acquisition_fence)?;
            value += 1;
            submission::one(
                queue,
                &[],
                &[],
                &[SyncPoint {
                    point: Point {
                        timeline: timeline.as_raw(),
                        value,
                    },
                    stages: STAGE_ALL,
                }],
                &[acquired[0].at(STAGE_ALL)],
                &[],
                span(&mut submit_storage),
            )?;
            gpu::timeline_wait(timeline, value, 10_000_000_000)?;
            let mut effect = FRAME_UNKNOWN;
            assert_eq!(
                gpu::abandon(
                    chains[0],
                    &[Frame {
                        index: u32::MAX,
                        ..unused
                    }],
                    span(&mut abandon_storage),
                    &mut effect
                ),
                Err(INVALID)
            );
            assert_eq!(effect, FRAME_RETAINED);
            gpu::abandon(
                chains[0],
                &[unused],
                span(&mut abandon_storage),
                &mut effect,
            )?;
            assert_eq!(effect, FRAME_RELEASED);
            if phase == 0 {
                for i in 0..2 {
                    descs[i].old_swapchain = chains[i].as_raw();
                    let replacement = create_chain(device, &descs[i])?;
                    gpu::chain_destroy(chains[i]);
                    chains[i] = replacement;
                    descs[i].old_swapchain = ptr::null_mut();
                    borrowed[i] = images(replacement)?;
                }
            }
        }
        for chain in chains {
            gpu::chain_destroy(chain);
        }
        for fence in fences {
            gpu::fence_destroy(fence);
        }
        gpu::fence_destroy(acquisition_fence);
        for binary in acquired {
            gpu::binary_destroy(binary);
        }
        gpu::binary_destroy(ready);
        gpu::arena_destroy(arena);
        gpu::timeline_destroy(timeline);
        gpu::device_destroy(device);
        for surface in surfaces {
            gpu::surface_destroy(surface);
        }
        for window in &mut windows {
            (window.destroy)(window.handle);
            window.handle = ptr::null_mut();
        }
        for window in &mut windows {
            ManuallyDrop::drop(window);
        }
    }
    println!(
        "PRESENTATION_PASS frames=8 swapchains=2 replacement=2 unused_release=2 unified={unified}"
    );
    Ok(())
}
