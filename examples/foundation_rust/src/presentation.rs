//! Bounded real Wayland presentation, using only the public native Rust interface.
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
        gpu::swapchain_images(chain, &mut [], &mut count)?;
    }
    let mut pointers = vec![ptr::null_mut(); count as usize];
    unsafe {
        gpu::swapchain_images(chain, &mut pointers, &mut count)?;
    }
    Ok(pointers
        .into_iter()
        .map(|p| unsafe { gpu::ImageHandle::from_raw(p).unwrap() })
        .collect())
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
    let unified = gpu::adapter_features(adapter).available & UNIFIED_IMAGES;
    let device = gpu::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        PRESENTATION | unified,
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
        assert_eq!(unsafe { gpu::swapchain_create(device, &bad) }, Err(INVALID));
        chains.push(unsafe { gpu::swapchain_create(device, &desc)? });
        descs.push(desc);
    }
    let mut borrowed = [unsafe { images(chains[0])? }, unsafe { images(chains[1])? }];
    let mut submit_storage = storage(gpu::submit_scratch_requirements(1, 2, 2)?);
    let mut present_storage = storage(gpu::present_scratch_requirements(1, 2)?);
    let mut barrier_storage = storage(gpu::barrier_scratch_requirements(0, 1)?);
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
            gpu::swapchain_images(chains[0], &mut short, &mut count),
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
                    let (index, _) =
                        gpu::acquire(chains[i], 1_000_000_000, Some(acquired[i]), None)?;
                    let image = borrowed[i][index as usize];
                    let mut barrier = ImageBarrier {
                        image: image.as_raw(),
                        range: full,
                        before: 0,
                        after: ACCESS_COPY_WRITE,
                        old_state: STATE_UNDEFINED,
                        new_state: STATE_COPY_DST,
                        source_domain: DOMAIN_IGNORED,
                        destination_domain: DOMAIN_IGNORED,
                        discard: 1,
                    };
                    let scratch = span(&mut barrier_storage);
                    let mut dep = Dependency {
                        header: Record::new::<Dependency>(DEPENDENCY),
                        before: STAGE_TRANSFER,
                        after: STAGE_TRANSFER,
                        global_before: 0,
                        global_after: 0,
                        memory_count: 0,
                        image_count: 1,
                        flags: 0,
                        memory: ptr::null(),
                        images: &barrier,
                        scratch: scratch.data,
                        scratch_size: scratch.size,
                    };
                    gpu::barrier(list, &dep);
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
                    barrier.before = ACCESS_COPY_WRITE;
                    barrier.after = 0;
                    barrier.old_state = STATE_COPY_DST;
                    barrier.new_state = STATE_PRESENT;
                    barrier.discard = 0;
                    dep.images = &barrier;
                    dep.after = 0;
                    gpu::barrier(list, &dep);
                    entries.push(PresentImage {
                        swapchain: chains[i].as_raw(),
                        index,
                        reserved: 0,
                        fence: fences[i].as_raw(),
                    });
                }
                gpu::commands_end(list)?;
                value += 1;
                gpu::queue_submit_with_binary(
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
                let mut results = [99; 2];
                let mut present = PresentDesc {
                    header: Record::new::<PresentDesc>(PRESENT_DESC),
                    wait_count: 1,
                    image_count: 2,
                    waits: waits.as_ptr(),
                    images: entries.as_ptr(),
                    results: results.as_mut_ptr(),
                    scratch: HostSpan {
                        data: ptr::null_mut(),
                        size: 0,
                    },
                };
                assert_eq!(gpu::queue_present(queue, &present), Err(CAPACITY));
                assert_eq!(results, [99; 2]);
                present.scratch = span(&mut present_storage);
                gpu::queue_present(queue, &present)?;
                assert!(results.iter().all(|s| [OK, SUBOPTIMAL].contains(s)));
                gpu::timeline_wait(timeline, value, 10_000_000_000)?;
                for fence in fences {
                    gpu::fence_wait(fence, 10_000_000_000)?;
                    gpu::fence_reset(fence)?;
                }
                gpu::arena_reset(arena)?;
            }
            // Unused acquisition: fence proves the signal completed; consume binary
            // separately, retire that wait, then release. No implicit reset in release.
            let (index, _) = gpu::acquire(
                chains[0],
                1_000_000_000,
                Some(acquired[0]),
                Some(acquisition_fence),
            )?;
            gpu::fence_wait(acquisition_fence, 10_000_000_000)?;
            gpu::fence_reset(acquisition_fence)?;
            value += 1;
            gpu::queue_submit_with_binary(
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
            assert_eq!(gpu::release(chains[0], &[u32::MAX]), Err(INVALID));
            gpu::release(chains[0], &[index])?;
            if phase == 0 {
                for i in 0..2 {
                    descs[i].old_swapchain = chains[i].as_raw();
                    let replacement = gpu::swapchain_create(device, &descs[i])?;
                    gpu::swapchain_destroy(chains[i]);
                    chains[i] = replacement;
                    descs[i].old_swapchain = ptr::null_mut();
                    borrowed[i] = images(replacement)?;
                }
            }
        }
        for chain in chains {
            gpu::swapchain_destroy(chain);
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
