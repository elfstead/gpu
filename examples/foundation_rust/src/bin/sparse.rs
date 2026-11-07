//! Independent application policy: reserve, bind, use, remap, use, then unmap.
use ogpu::foundation::{rust as gpu, types::*};
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    ptr::{self, NonNull},
};
#[path = "../submission.rs"]
mod submission;
struct Storage {
    pointer: NonNull<u8>,
    layout: Layout,
}
impl Storage {
    fn new(r: HostRequirements) -> gpu::Result<Self> {
        let layout = Layout::from_size_align(
            usize::try_from(r.size).map_err(|_| CAPACITY)?.max(1),
            r.alignment as usize,
        )
        .map_err(|_| INVALID)?;
        let pointer = NonNull::new(unsafe { alloc_zeroed(layout) }).ok_or(OUT_OF_MEMORY)?;
        Ok(Self { pointer, layout })
    }
    fn span(&mut self) -> HostSpan {
        HostSpan {
            data: self.pointer.as_ptr().cast(),
            size: self.layout.size() as u64,
        }
    }
}
impl Drop for Storage {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.pointer.as_ptr(), self.layout);
        }
    }
}
unsafe fn bind(
    device: gpu::DeviceHandle,
    queue: gpu::QueueHandle,
    descriptions: &[SparseSubmit],
) -> gpu::Result<()> {
    let req = unsafe { gpu::sparse_scratch_requirements(device, descriptions)? };
    let mut scratch = Storage::new(req)?;
    let mut outcomes = vec![
        SubmitOutcome {
            status: INVALID,
            acceptance: SUBMIT_REJECTED
        };
        descriptions.len()
    ];
    unsafe {
        gpu::queue_bind_sparse(queue, descriptions, &mut outcomes, scratch.span())?;
    }
    assert!(outcomes
        .iter()
        .all(|o| o.status == OK && o.acceptance == SUBMIT_ACCEPTED));
    Ok(())
}
unsafe fn requirements(
    device: gpu::DeviceHandle,
    desc: &MemoryDesc,
) -> gpu::Result<(Requirements, Vec<u32>)> {
    let mut req = Requirements::default();
    unsafe {
        gpu::memory_requirements(device, desc, &mut req)?;
    }
    let mut types = vec![0; req.compatible_type_count as usize];
    req.compatible_types = types.as_mut_ptr();
    req.compatible_type_capacity = types.len() as u32;
    unsafe {
        gpu::memory_requirements(device, desc, &mut req)?;
    }
    Ok((req, types))
}
fn run_buffer(residency: bool) -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let available = gpu::adapter_features(adapter).available;
    println!(
        "SPARSE_FEATURES={:?}",
        available
            .iter()
            .filter(|v| **v >= 132 && **v <= 142)
            .collect::<Vec<_>>()
    );
    if !available.contains(&feature_ids::SPARSE_BINDING)
        || (residency && !available.contains(&feature_ids::SPARSE_BUFFER))
    {
        return Err(UNSUPPORTED);
    }
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & (COMPUTE | QUEUE_SPARSE) == COMPUTE | QUEUE_SPARSE)
        .ok_or(UNSUPPORTED)?
        .domain;
    let mut features = vec![feature_ids::SPARSE_BINDING];
    for f in [feature_ids::SPARSE_BUFFER, feature_ids::SPARSE_IMAGE_2D] {
        if available.contains(&f) {
            features.push(f);
        }
    }
    let device = gpu::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        &features,
    )?;
    unsafe {
        let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
        let timeline = gpu::timeline_create(device, 0)?;
        let arena = gpu::arena_create(
            device,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 2,
                level: 0,
                reserved: 0,
            },
        )?;
        let recording = RecordingDesc {
            header: Record::new::<RecordingDesc>(RECORDING_DESC),
            replay_mode: 1,
            level: 0,
            inheritance: ptr::null(),
        };
        let mut vd = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: 4096,
            alignment: 1,
            usage: USAGE_COPY_SRC | USAGE_COPY_DST,
            memory_type: u32::MAX,
            kind: MEMORY_VIRTUAL,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: if residency {
                MEMORY_SPARSE_RESIDENCY
            } else {
                0
            },
        };
        let (probe, _) = requirements(device, &vd)?;
        let page = probe.alignment;
        vd.size = page.checked_mul(3).ok_or(INVALID)?;
        let (req, types) = requirements(device, &vd)?;
        assert_eq!(req.size, vd.size);
        let virtual_memory = gpu::memory_create(device, &vd)?;
        let address = gpu::memory_address(virtual_memory.span(0, vd.size))?;
        let info = gpu::memory_info_query(virtual_memory)?;
        assert_eq!((info.allocation_size, info.memory_type), (0, u32::MAX));
        assert!(gpu::memory_map(virtual_memory.span(0, vd.size)).is_err());
        let physical_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| {
                types.contains(&m.id)
                    && m.properties & MEMORY_HOST_VISIBLE != 0
                    && m.properties & (16 | 32 | 64 | 128) == 0
            })
            .ok_or(UNSUPPORTED)?
            .id;
        let physical = gpu::memory_create(
            device,
            &MemoryDesc {
                kind: MEMORY_OPAQUE,
                flags: MEMORY_ADDRESSABLE,
                usage: 0,
                memory_type: physical_type,
                ..vd
            },
        )?;
        let mapping = gpu::memory_map(physical.span(0, vd.size))?;
        for i in 0..3 {
            ptr::write_bytes(
                mapping.data.cast::<u8>().add(i * page as usize),
                (i as u8 + 1) * 0x11,
                page as usize,
            );
        }
        gpu::memory_flush(physical.span(0, vd.size))?;
        let mut rd = MemoryDesc {
            kind: MEMORY_LINEAR,
            flags: 0,
            size: vd.size * 2 + 256,
            alignment: 4,
            usage: USAGE_COPY_DST,
            ..vd
        };
        let (_, rt) = requirements(device, &rd)?;
        rd.memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| rt.contains(&m.id) && m.properties & MEMORY_HOST_VISIBLE != 0)
            .ok_or(UNSUPPORTED)?
            .id;
        let readback = gpu::memory_create(device, &rd)?;
        let mapped = gpu::memory_map(readback.span(0, rd.size))?;
        ptr::write_bytes(mapped.data.cast::<u8>(), 0xcd, rd.size as usize);
        gpu::memory_flush(readback.span(0, rd.size))?;
        let point = |value| Point {
            timeline: timeline.as_raw(),
            value,
        };
        let mut binds: Vec<_> = (0..3)
            .map(|i| SparseMemoryBind {
                offset: i * page,
                size: page,
                physical: physical.span(i * page, page),
                flags: 0,
                reserved: 0,
            })
            .collect();
        let mut group = SparseBufferBinds {
            memory: virtual_memory.as_raw(),
            count: binds.len() as u32,
            reserved: 0,
            binds: binds.as_ptr(),
        };
        let signal = point(1);
        let desc = SparseSubmit {
            buffer_count: 1,
            buffers: &group,
            signal_count: 1,
            signals: &signal,
            ..Default::default()
        };
        let mut outcome = [SubmitOutcome {
            status: OK,
            acceptance: SUBMIT_ACCEPTED,
        }];
        assert_eq!(
            gpu::queue_bind_sparse(
                queue,
                &[desc],
                &mut outcome,
                HostSpan {
                    data: ptr::null_mut(),
                    size: 0
                }
            ),
            Err(CAPACITY)
        );
        assert_eq!(outcome[0].acceptance, SUBMIT_REJECTED);
        // A malformed last batch must reject the whole call, leaving signal 1 unused.
        let malformed = SparseSubmit {
            flags: 1,
            ..Default::default()
        };
        assert_eq!(bind(device, queue, &[desc, malformed]), Err(UNSUPPORTED));
        let invalid_bind = SparseMemoryBind {
            flags: 1,
            ..binds[0]
        };
        let invalid_group = SparseBufferBinds {
            binds: &invalid_bind,
            count: 1,
            ..group
        };
        let invalid_desc = SparseSubmit {
            buffers: &invalid_group,
            signal_count: 0,
            signals: ptr::null(),
            ..desc
        };
        assert_eq!(bind(device, queue, &[desc, invalid_desc]), Err(INVALID));
        assert_eq!(gpu::timeline_poll(timeline)?, 0);
        bind(device, queue, &[desc])?;
        let mut command_storage = Storage::new(gpu::submit_scratch_requirements(
            device,
            &[SubmitDesc {
                list_count: 1,
                wait_count: 1,
                signal_count: 1,
                ..Default::default()
            }],
        )?)?;
        for pass in 0..2u64 {
            let list = gpu::commands_begin(arena, &recording)?;
            gpu::copy_memory(
                list,
                readback.span(128 + pass * vd.size, vd.size),
                virtual_memory.span(0, vd.size),
            );
            let scope = BarrierScope {
                stages_before: STAGE_TRANSFER,
                stages_after: STAGE_HOST,
                access_before: ACCESS_COPY_WRITE,
                access_after: ACCESS_HOST_READ,
            };
            let mut bytes = [0u64; 64];
            gpu::barrier(
                list,
                &Dependency {
                    global_count: 1,
                    global: &scope,
                    ..Default::default()
                },
                HostSpan {
                    data: bytes.as_mut_ptr().cast(),
                    size: std::mem::size_of_val(&bytes) as u64,
                },
            );
            let list = gpu::commands_end(list)?;
            submission::one(
                queue,
                &[list],
                &[SyncPoint {
                    point: point(1 + pass * 2),
                    stages: STAGE_ALL,
                }],
                &[SyncPoint {
                    point: point(2 + pass * 2),
                    stages: STAGE_ALL,
                }],
                &[],
                &[],
                command_storage.span(),
            )?;
            if pass == 0 {
                binds[0].physical = physical.span(2 * page, page);
                binds[2].physical = physical.span(0, page);
                group.binds = binds.as_ptr();
                let wait = point(2);
                let signal = point(3);
                bind(
                    device,
                    queue,
                    &[
                        SparseSubmit::default(),
                        SparseSubmit {
                            buffer_count: 1,
                            buffers: &group,
                            wait_count: 1,
                            waits: &wait,
                            signal_count: 1,
                            signals: &signal,
                            ..Default::default()
                        },
                    ],
                )?;
            }
        }
        gpu::timeline_wait(timeline, 4, 10_000_000_000)?;
        assert_eq!(
            gpu::memory_address(virtual_memory.span(0, vd.size))?,
            address
        );
        gpu::memory_invalidate(readback.span(0, rd.size))?;
        let bytes = std::slice::from_raw_parts(mapped.data.cast::<u8>(), rd.size as usize);
        assert!(bytes[..128]
            .iter()
            .chain(&bytes[128 + 2 * vd.size as usize..])
            .all(|&b| b == 0xcd));
        for (i, expected) in [0x11, 0x22, 0x33, 0x33, 0x22, 0x11].into_iter().enumerate() {
            assert!(
                bytes[128 + i * page as usize..128 + (i + 1) * page as usize]
                    .iter()
                    .all(|&b| b == expected)
            );
        }
        let mut retired = 4;
        if residency {
            // Evict only the middle page. Copy two still-resident ranges using
            // the same reservation/address while a genuine hole remains.
            let hole = SparseMemoryBind {
                offset: page,
                size: page,
                physical: Span {
                    memory: ptr::null_mut(),
                    offset: 0,
                    size: 0,
                },
                flags: 0,
                reserved: 0,
            };
            let holes = SparseBufferBinds {
                memory: virtual_memory.as_raw(),
                count: 1,
                reserved: 0,
                binds: &hole,
            };
            let wait = point(4);
            let signal = point(5);
            bind(
                device,
                queue,
                &[SparseSubmit {
                    buffer_count: 1,
                    buffers: &holes,
                    wait_count: 1,
                    waits: &wait,
                    signal_count: 1,
                    signals: &signal,
                    ..Default::default()
                }],
            )?;
            gpu::arena_reset(arena)?;
            let list = gpu::commands_begin(arena, &recording)?;
            gpu::copy_memory(list, readback.span(128, page), virtual_memory.span(0, page));
            gpu::copy_memory(
                list,
                readback.span(128 + page, page),
                virtual_memory.span(2 * page, page),
            );
            let host = BarrierScope {
                stages_before: STAGE_TRANSFER,
                stages_after: STAGE_HOST,
                access_before: ACCESS_COPY_WRITE,
                access_after: ACCESS_HOST_READ,
            };
            let mut bs = Storage::new(gpu::barrier_scratch_requirements(device, 1, 0, 0)?)?;
            gpu::barrier(
                list,
                &Dependency {
                    global_count: 1,
                    global: &host,
                    ..Default::default()
                },
                bs.span(),
            );
            let list = gpu::commands_end(list)?;
            submission::one(
                queue,
                &[list],
                &[SyncPoint {
                    point: point(5),
                    stages: STAGE_ALL,
                }],
                &[SyncPoint {
                    point: point(6),
                    stages: STAGE_ALL,
                }],
                &[],
                &[],
                command_storage.span(),
            )?;
            gpu::timeline_wait(timeline, 6, 10_000_000_000)?;
            gpu::memory_invalidate(readback.span(128, 2 * page))?;
            let bytes =
                std::slice::from_raw_parts(mapped.data.cast::<u8>().add(128), (2 * page) as usize);
            assert!(bytes[..page as usize].iter().all(|&b| b == 0x33));
            assert!(bytes[page as usize..].iter().all(|&b| b == 0x11));
            retired = 6;
            println!("SPARSE_PARTIAL=pass resident_pages=2 unbound_pages=1");
        }
        let unmap = SparseMemoryBind {
            offset: 0,
            size: req.size,
            physical: Span {
                memory: ptr::null_mut(),
                offset: 0,
                size: 0,
            },
            flags: 0,
            reserved: 0,
        };
        group.count = 1;
        group.binds = &unmap;
        let wait = point(retired);
        let signal = point(retired + 1);
        bind(
            device,
            queue,
            &[SparseSubmit {
                buffer_count: 1,
                buffers: &group,
                wait_count: 1,
                waits: &wait,
                signal_count: 1,
                signals: &signal,
                ..Default::default()
            }],
        )?;
        gpu::timeline_wait(timeline, retired + 1, 10_000_000_000)?;
        gpu::arena_destroy(arena);
        gpu::memory_destroy(readback);
        gpu::memory_destroy(virtual_memory);
        gpu::memory_destroy(physical);
        gpu::timeline_destroy(timeline);
        println!("SPARSE_BUFFER=pass page_bytes={page} verified_bytes={} stable_address=true residency={residency}", vd.size * 2);
        gpu::device_destroy(device);
    }
    Ok(())
}
fn run_image(residency: bool) -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let available = gpu::adapter_features(adapter).available;
    let mut features = vec![feature_ids::SPARSE_BINDING];
    if residency {
        features.push(feature_ids::SPARSE_IMAGE_2D);
    }
    if features.iter().any(|f| !available.contains(f)) {
        return Err(UNSUPPORTED);
    }
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & (COMPUTE | QUEUE_SPARSE) == COMPUTE | QUEUE_SPARSE)
        .ok_or(UNSUPPORTED)?
        .domain;
    let device = gpu::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        &features,
    )?;
    unsafe {
        let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
        let timeline = gpu::timeline_create(device, 0)?;
        let desc = ImageDesc {
            header: Record::new::<ImageDesc>(IMAGE_DESC),
            format: RGBA8_UNORM,
            dimension: IMAGE_2D,
            mip_count: 9,
            layer_count: 2,
            sample_count: 1,
            extent: Extent {
                x: 256,
                y: 256,
                z: 1,
            },
            usage: IMAGE_COPY_SRC | IMAGE_COPY_DST,
            flags: IMAGE_SPARSE | if residency { IMAGE_SPARSE_RESIDENCY } else { 0 },
            view_formats: ptr::null(),
            view_format_count: 0,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
        };
        let mut req = Requirements::default();
        gpu::image_requirements(device, &desc, &mut req)?;
        let mut types = vec![0; req.compatible_type_count as usize];
        req.compatible_types = types.as_mut_ptr();
        req.compatible_type_capacity = types.len() as u32;
        gpu::image_requirements(device, &desc, &mut req)?;
        let mut query = Query {
            header: Record::new::<Query>(SPARSE_IMAGE_REQUIREMENTS),
            data: ptr::null_mut(),
            capacity: 0,
            count: 0,
            element_size: std::mem::size_of::<SparseImageRequirements>() as u32,
            reserved: 0,
        };
        gpu::image_sparse_requirements(device, &desc, &mut query)?;
        let mut sparse = vec![SparseImageRequirements::default(); query.count as usize];
        query.capacity = query.count;
        query.data = sparse.as_mut_ptr().cast();
        gpu::image_sparse_requirements(device, &desc, &mut query)?;
        let image = gpu::image_create_unbound(device, &desc)?;
        let null = Span {
            memory: ptr::null_mut(),
            offset: 0,
            size: 0,
        };
        assert_eq!(gpu::image_bind(image, null), Err(INVALID));
        let mut tiled = Vec::new();
        let mut opaque = Vec::new();
        let mut allocated = 0u64;
        let page = req.alignment;
        // Consumer allocator: disjoint physical extents for every tile/tail. All
        // shape information comes from the public query; no hardcoded tile size.
        if residency {
            assert!(sparse.iter().any(|r| r.aspects == ASPECT_COLOR));
            for r in &sparse {
                println!("SPARSE_IMAGE_REQUIREMENT aspect={} granularity={:?} tail_lod={} tail_size={} tail_offset={} tail_stride={} flags={}", r.aspects, r.granularity, r.mip_tail_first_lod, r.mip_tail_size, r.mip_tail_offset, r.mip_tail_stride, r.flags);
                if r.aspects != ASPECT_METADATA {
                    for layer in 0..desc.layer_count {
                        for mip in 0..desc.mip_count.min(r.mip_tail_first_lod) {
                            let extent = Extent {
                                x: (desc.extent.x >> mip).max(1),
                                y: (desc.extent.y >> mip).max(1),
                                z: 1,
                            };
                            let bytes = u64::from(extent.x.div_ceil(r.granularity[0]))
                                * u64::from(extent.y.div_ceil(r.granularity[1]))
                                * page;
                            tiled.push(SparseImageBind {
                                aspect: r.aspects,
                                mip,
                                layer,
                                reserved: 0,
                                offset: Offset { x: 0, y: 0, z: 0 },
                                extent,
                                physical: Span {
                                    offset: allocated,
                                    size: bytes,
                                    ..null
                                },
                            });
                            allocated += bytes;
                        }
                    }
                }
                if r.mip_tail_size != 0 {
                    let layers = if r.flags & SPARSE_SINGLE_MIPTAIL != 0 {
                        1
                    } else {
                        desc.layer_count
                    };
                    for layer in 0..layers {
                        opaque.push(SparseMemoryBind {
                            offset: r.mip_tail_offset + u64::from(layer) * r.mip_tail_stride,
                            size: r.mip_tail_size,
                            physical: Span {
                                offset: allocated,
                                size: r.mip_tail_size,
                                ..null
                            },
                            flags: if r.aspects == ASPECT_METADATA {
                                SPARSE_BIND_METADATA
                            } else {
                                0
                            },
                            reserved: 0,
                        });
                        allocated += r.mip_tail_size.div_ceil(page) * page;
                    }
                }
            }
        } else {
            allocated = req.size;
            opaque.push(SparseMemoryBind {
                offset: 0,
                size: req.size,
                physical: Span {
                    size: req.size,
                    ..null
                },
                flags: 0,
                reserved: 0,
            });
        }
        let memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| types.contains(&m.id) && m.properties & (16 | 32 | 64 | 128) == 0)
            .ok_or(UNSUPPORTED)?
            .id;
        let md = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: allocated,
            alignment: 1,
            usage: 0,
            memory_type,
            kind: MEMORY_OPAQUE,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let backing = gpu::memory_create(device, &md)?;
        for b in &mut tiled {
            b.physical.memory = backing.as_raw();
        }
        for b in &mut opaque {
            b.physical.memory = backing.as_raw();
        }
        let group = SparseImageBinds {
            image: image.as_raw(),
            count: tiled.len() as u32,
            reserved: 0,
            binds: tiled.as_ptr(),
        };
        let ogroup = SparseOpaqueImageBinds {
            image: image.as_raw(),
            count: opaque.len() as u32,
            reserved: 0,
            binds: opaque.as_ptr(),
        };
        let point = |value| Point {
            timeline: timeline.as_raw(),
            value,
        };
        let signal = point(1);
        bind(
            device,
            queue,
            &[SparseSubmit {
                image_count: u32::from(!tiled.is_empty()),
                images: &group,
                opaque_image_count: u32::from(!opaque.is_empty()),
                opaque_images: &ogroup,
                signal_count: 1,
                signals: &signal,
                ..Default::default()
            }],
        )?;
        let mut rd = MemoryDesc {
            size: 4096,
            alignment: 4,
            usage: USAGE_COPY_DST,
            kind: MEMORY_LINEAR,
            ..md
        };
        let (_, types) = requirements(device, &rd)?;
        rd.memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| types.contains(&m.id) && m.properties & MEMORY_HOST_VISIBLE != 0)
            .ok_or(UNSUPPORTED)?
            .id;
        let readback = gpu::memory_create(device, &rd)?;
        let mapping = gpu::memory_map(readback.span(0, 4096))?;
        ptr::write_bytes(mapping.data.cast::<u8>(), 0xcd, 4096);
        gpu::memory_flush(readback.span(0, 4096))?;
        let arena = gpu::arena_create(
            device,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            },
        )?;
        let list = gpu::commands_begin(
            arena,
            &RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: ptr::null(),
            },
        )?;
        let full = Subresources {
            aspects: ASPECT_COLOR,
            first_mip: 0,
            mip_count: desc.mip_count,
            first_layer: 0,
            layer_count: desc.layer_count,
        };
        let mut barrier_storage =
            Storage::new(gpu::barrier_scratch_requirements(device, 1, 0, 1)?)?;
        let mut barrier = ImageBarrier {
            next: ptr::null(),
            scope: BarrierScope {
                stages_before: 0,
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
        gpu::barrier(
            list,
            &Dependency {
                image_count: 1,
                images: &barrier,
                ..Default::default()
            },
            barrier_storage.span(),
        );
        gpu::clear_image(
            list,
            image,
            STATE_COPY_DST,
            full,
            ClearValue {
                f32: [0.0, 1.0, 0.0, 1.0],
            },
        );
        barrier.scope = BarrierScope {
            stages_before: STAGE_TRANSFER,
            stages_after: STAGE_TRANSFER,
            access_before: ACCESS_COPY_WRITE,
            access_after: ACCESS_COPY_READ,
        };
        barrier.old_state = STATE_COPY_DST;
        barrier.new_state = STATE_COPY_SRC;
        barrier.discard = 0;
        gpu::barrier(
            list,
            &Dependency {
                image_count: 1,
                images: &barrier,
                ..Default::default()
            },
            barrier_storage.span(),
        );
        for layer in 0..2 {
            for (mip, side, offset) in [
                (0, 8, 256 + u64::from(layer) * 512),
                (8, 1, 1536 + u64::from(layer) * 16),
            ] {
                gpu::copy_from_image(
                    list,
                    readback.span(offset, u64::from(side * side * 4)),
                    image,
                    &ImageCopy {
                        region: ImageRegion {
                            aspect: ASPECT_COLOR,
                            mip,
                            first_layer: layer,
                            layer_count: 1,
                            offset: Offset { x: 0, y: 0, z: 0 },
                            extent: Extent {
                                x: side,
                                y: side,
                                z: 1,
                            },
                        },
                        row_pitch: 0,
                        slice_pitch: 0,
                        state: STATE_COPY_SRC,
                        reserved: 0,
                    },
                );
            }
        }
        let host = BarrierScope {
            stages_before: STAGE_TRANSFER,
            stages_after: STAGE_HOST,
            access_before: ACCESS_COPY_WRITE,
            access_after: ACCESS_HOST_READ,
        };
        gpu::barrier(
            list,
            &Dependency {
                global_count: 1,
                global: &host,
                ..Default::default()
            },
            barrier_storage.span(),
        );
        let list = gpu::commands_end(list)?;
        let mut scratch = Storage::new(gpu::submit_scratch_requirements(
            device,
            &[SubmitDesc {
                list_count: 1,
                wait_count: 1,
                signal_count: 1,
                ..Default::default()
            }],
        )?)?;
        submission::one(
            queue,
            &[list],
            &[SyncPoint {
                point: point(1),
                stages: STAGE_ALL,
            }],
            &[SyncPoint {
                point: point(2),
                stages: STAGE_ALL,
            }],
            &[],
            &[],
            scratch.span(),
        )?;
        gpu::timeline_wait(timeline, 2, 10_000_000_000)?;
        gpu::memory_invalidate(readback.span(0, 4096))?;
        let bytes = std::slice::from_raw_parts(mapping.data.cast::<u8>(), 4096);
        let mut expected = [0xcd; 4096];
        for layer in 0..2usize {
            for (offset, pixels) in [(256 + layer * 512, 64), (1536 + layer * 16, 1)] {
                for pixel in 0..pixels {
                    expected[offset + pixel * 4..offset + pixel * 4 + 4]
                        .copy_from_slice(&[0, 255, 0, 255]);
                }
            }
        }
        assert_eq!(bytes, &expected);
        for b in &mut tiled {
            b.physical = null;
        }
        for b in &mut opaque {
            b.physical = null;
        }
        let wait = point(2);
        let signal = point(3);
        bind(
            device,
            queue,
            &[SparseSubmit {
                image_count: u32::from(!tiled.is_empty()),
                images: &group,
                opaque_image_count: u32::from(!opaque.is_empty()),
                opaque_images: &ogroup,
                wait_count: 1,
                waits: &wait,
                signal_count: 1,
                signals: &signal,
                ..Default::default()
            }],
        )?;
        gpu::timeline_wait(timeline, 3, 10_000_000_000)?;
        gpu::arena_destroy(arena);
        gpu::image_destroy(image);
        gpu::memory_destroy(backing);
        gpu::memory_destroy(readback);
        gpu::timeline_destroy(timeline);
        gpu::device_destroy(device);
        println!("SPARSE_IMAGE=pass residency={residency} tiled_regions={} opaque_regions={} verified_pixels=130", tiled.len(), opaque.len());
    }
    Ok(())
}

pub fn run() -> gpu::Result<()> {
    run_buffer(false)?;
    run_image(false)?;
    match run_image(true) {
        Err(UNSUPPORTED) => println!("SPARSE_IMAGE_RESIDENCY=unsupported"),
        result => result?,
    }
    match run_buffer(true) {
        Err(UNSUPPORTED) => println!("SPARSE_RESIDENCY=unsupported"),
        result => result?,
    }
    println!("SPARSE_PASS");
    Ok(())
}
#[allow(dead_code)]
fn main() {
    if let Err(status) = run() {
        eprintln!("SPARSE_STATUS={status}");
        std::process::exit(if status == UNSUPPORTED { 77 } else { 1 });
    }
}
