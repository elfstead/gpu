use super::*;
use std::cell::RefCell;

fn empty_storage() -> HostSpan {
    HostSpan {
        data: ptr::null_mut(),
        size: 0,
    }
}
struct Storage {
    words: Vec<u64>,
    size: u64,
}
impl Storage {
    fn new(req: HostRequirements) -> Self {
        assert!(req.alignment <= align_of::<u64>() as u64);
        Self {
            words: vec![0; req.size.div_ceil(8) as usize],
            size: req.size,
        }
    }
    fn span(&mut self) -> HostSpan {
        HostSpan {
            data: self.words.as_mut_ptr().cast(),
            size: self.size,
        }
    }
}
fn global(entries: &[BarrierScope]) -> Dependency {
    Dependency {
        global_count: entries.len() as u32,
        global: entries.as_ptr(),
        ..Default::default()
    }
}
fn range_dep(entries: &[MemoryBarrier]) -> Dependency {
    Dependency {
        memory_count: entries.len() as u32,
        memory: entries.as_ptr(),
        ..Default::default()
    }
}
fn memory_barrier(range: Span, scope: BarrierScope) -> MemoryBarrier {
    MemoryBarrier {
        range,
        scope,
        source_domain: u32::MAX,
        destination_domain: u32::MAX,
    }
}
fn copy_edge() -> BarrierScope {
    BarrierScope {
        stages_before: COPY_SCOPE,
        stages_after: COPY_SCOPE,
        access_before: 8,
        access_after: 4,
    }
}
fn host_edge() -> BarrierScope {
    BarrierScope {
        stages_before: COPY_SCOPE,
        stages_after: 256,
        access_before: 8,
        access_after: 64,
    }
}

#[test]
fn capabilities_follow_native_queue_and_feature_scopes() {
    let transfer = limits(7, TRANSFER, 0);
    assert_eq!(transfer.domain, 7);
    assert_eq!(transfer.flags, DEPENDENCY_OWNERSHIP | DEPENDENCY_BINARY);
    assert_eq!(transfer.memory_scope, MEMORY_SCOPE_BYTES);
    assert_eq!(transfer.wait_stages, 1 | 2 | COPY_SCOPE | CLEAR_SCOPE);
    assert_eq!(transfer.signal_stages, transfer.wait_stages);
    assert_eq!(transfer.split_stages, 0);
    for (flags, enabled, expected) in [
        (COMPUTE, 0, 1 | 2 | 4 | 32 | COPY_SCOPE | CLEAR_SCOPE),
        (
            COMPUTE,
            GENERATED_COMMANDS,
            1 | 2 | 4 | 32 | COPY_SCOPE | CLEAR_SCOPE | PREPROCESS_STAGE,
        ),
        (
            GRAPHICS,
            RASTER,
            KNOWN_SCOPES & !(256 | 4 | MESH_STAGE | TASK_STAGE | PREPROCESS_STAGE),
        ),
        (
            GRAPHICS | COMPUTE,
            RASTER | MESH_SHADER | TASK_SHADER | GENERATED_COMMANDS,
            KNOWN_SCOPES & !256,
        ),
    ] {
        let value = limits(0, flags, enabled);
        assert_eq!(value.flags, 7);
        assert_eq!(value.wait_stages, expected);
        assert_eq!(value.signal_stages, expected);
        assert_eq!(value.split_stages, expected);
    }
}

#[test]
fn scratch_covers_all_arrays_without_reading_nested_inputs() {
    assert_eq!(
        requirements(layout(0, 0, 0).unwrap().0),
        HostRequirements {
            size: 0,
            alignment: 1
        }
    );
    assert_eq!(
        requirements(unsafe { split_layout(&[]) }.unwrap()),
        HostRequirements {
            size: 0,
            alignment: 1
        }
    );
    for (g, m, i) in [(1, 0, 0), (0, 1, 0), (0, 0, 1), (3, 5, 7)] {
        let (l, mo, io) = layout(g, m, i).unwrap();
        assert!(mo >= g as usize * size_of::<vk::VkMemoryBarrier2>());
        assert!(io >= mo + m as usize * size_of::<vk::VkBufferMemoryBarrier2>());
        assert!(l.size() >= io + i as usize * size_of::<vk::VkImageMemoryBarrier2>());
        assert_eq!(mo % align_of::<vk::VkBufferMemoryBarrier2>(), 0);
        assert_eq!(io % align_of::<vk::VkImageMemoryBarrier2>(), 0);
        let dep = Dependency {
            global_count: g,
            memory_count: m,
            image_count: i,
            global: ptr::dangling(),
            memory: ptr::dangling(),
            images: ptr::dangling(),
            flags: 0,
        };
        let waits = [SplitDependency {
            split: ptr::null(),
            dependency: &dep,
        }; 3];
        let all = unsafe { split_layout(&waits) }.unwrap();
        let (headers, _) = split_headers(3).unwrap();
        assert!(all.size() >= headers.size() + 3 * l.size());
        assert_eq!(all.align(), l.align().max(headers.align()));
        let bad = Dependency { flags: 1, ..dep };
        assert!(matches!(
            unsafe {
                split_layout(&[
                    waits[0],
                    SplitDependency {
                        dependency: &bad,
                        ..waits[1]
                    },
                ])
            },
            Err(UNSUPPORTED)
        ));
    }
    assert!(matches!(
        unsafe {
            split_layout(&[SplitDependency {
                split: ptr::null(),
                dependency: ptr::null(),
            }])
        },
        Err(INVALID)
    ));
    // Max u32 counts remain arithmetic-only, never an allocation attempt.
    let huge = layout(u32::MAX, u32::MAX, u32::MAX);
    if usize::BITS == 64 {
        assert!(huge.unwrap().0.size() > u32::MAX as usize);
    } else {
        assert!(huge.is_err());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    globals: Vec<[u64; 4]>,
    memory: Vec<([u64; 4], u64, u64, u32, u32)>,
    images: Vec<([u64; 4], u32, u32)>,
}
unsafe fn capture(info: &vk::VkDependencyInfo) -> Seen {
    let values = |a, b, c, d| [a, b, c, d];
    Seen {
        globals: unsafe { slice(info.pMemoryBarriers, info.memoryBarrierCount) }
            .unwrap()
            .iter()
            .map(|b| {
                values(
                    b.srcStageMask,
                    b.dstStageMask,
                    b.srcAccessMask,
                    b.dstAccessMask,
                )
            })
            .collect(),
        memory: unsafe { slice(info.pBufferMemoryBarriers, info.bufferMemoryBarrierCount) }
            .unwrap()
            .iter()
            .map(|b| {
                (
                    values(
                        b.srcStageMask,
                        b.dstStageMask,
                        b.srcAccessMask,
                        b.dstAccessMask,
                    ),
                    b.offset,
                    b.size,
                    b.srcQueueFamilyIndex,
                    b.dstQueueFamilyIndex,
                )
            })
            .collect(),
        images: unsafe { slice(info.pImageMemoryBarriers, info.imageMemoryBarrierCount) }
            .unwrap()
            .iter()
            .map(|b| {
                (
                    values(
                        b.srcStageMask,
                        b.dstStageMask,
                        b.srcAccessMask,
                        b.dstAccessMask,
                    ),
                    b.oldLayout,
                    b.newLayout,
                )
            })
            .collect(),
    }
}
thread_local! {
    static BARRIER: Cell<vk::PFN_vkCmdPipelineBarrier2> = const { Cell::new(None) };
    static SET: Cell<vk::PFN_vkCmdSetEvent2> = const { Cell::new(None) };
    static WAIT: Cell<vk::PFN_vkCmdWaitEvents2> = const { Cell::new(None) };
    static BARRIERS: RefCell<Vec<Seen>> = const { RefCell::new(Vec::new()) };
    static SETS: RefCell<Vec<Seen>> = const { RefCell::new(Vec::new()) };
    static WAITS: RefCell<Vec<Vec<Seen>>> = const { RefCell::new(Vec::new()) };
}
unsafe extern "C" fn barrier(command: vk::VkCommandBuffer, info: *const vk::VkDependencyInfo) {
    BARRIERS.with_borrow_mut(|v| v.push(unsafe { capture(&*info) }));
    unsafe {
        BARRIER.get().unwrap()(command, info);
    }
}
unsafe extern "C" fn set(
    command: vk::VkCommandBuffer,
    event: vk::VkEvent,
    info: *const vk::VkDependencyInfo,
) {
    SETS.with_borrow_mut(|v| v.push(unsafe { capture(&*info) }));
    unsafe {
        SET.get().unwrap()(command, event, info);
    }
}
unsafe extern "C" fn wait(
    command: vk::VkCommandBuffer,
    count: u32,
    events: *const vk::VkEvent,
    infos: *const vk::VkDependencyInfo,
) {
    WAITS.with_borrow_mut(|v| {
        v.push(
            unsafe { slice(infos, count) }
                .unwrap()
                .iter()
                .map(|i| unsafe { capture(i) })
                .collect(),
        )
    });
    unsafe {
        WAIT.get().unwrap()(command, count, events, infos);
    }
}
fn intercept(d: &mut Device) {
    BARRIER.set(d.f.vkCmdPipelineBarrier2);
    SET.set(d.f.vkCmdSetEvent2);
    WAIT.set(d.f.vkCmdWaitEvents2);
    d.f.vkCmdPipelineBarrier2 = Some(barrier);
    d.f.vkCmdSetEvent2 = Some(set);
    d.f.vkCmdWaitEvents2 = Some(wait);
    BARRIERS.with_borrow_mut(Vec::clear);
    SETS.with_borrow_mut(Vec::clear);
    WAITS.with_borrow_mut(Vec::clear);
}
fn arena(d: &Device, domain: u32, count: u32) -> Box<Arena> {
    Arena::create(
        d,
        &ArenaDesc {
            header: Record::new::<ArenaDesc>(ARENA_DESC),
            domain,
            list_capacity: count,
            level: 0,
            reserved: 0,
        },
    )
    .unwrap()
}
fn begin(arena: &mut Arena) -> *mut List {
    arena
        .begin(&RecordingDesc {
            header: Record::new::<RecordingDesc>(RECORDING_DESC),
            replay_mode: 2,
            level: 0,
            inheritance: ptr::null(),
        })
        .unwrap()
}
fn memory(d: &Device) -> Box<Memory> {
    let mut desc = MemoryDesc {
        header: Record::new::<MemoryDesc>(MEMORY_DESC),
        size: 1024,
        alignment: 256,
        usage: 3 | 64,
        memory_type: u32::MAX,
        kind: 1,
        concurrent_domains: ptr::null(),
        concurrent_domain_count: 0,
        flags: 0,
    };
    let mut req = Requirements::default();
    unsafe { d.memory_requirements(&desc, &mut req) }.unwrap();
    let mut types = vec![0; req.compatible_type_count as usize];
    req.compatible_types = types.as_mut_ptr();
    req.compatible_type_capacity = types.len() as u32;
    unsafe { d.memory_requirements(&desc, &mut req) }.unwrap();
    desc.memory_type = *types
        .iter()
        .find(|id| {
            d.snapshot
                .memory_types
                .iter()
                .any(|t| t.id == **id && t.properties & 2 != 0)
        })
        .unwrap();
    unsafe { Memory::create(d, &desc) }.unwrap()
}
fn span(memory: &Memory, offset: u64, size: u64) -> Span {
    Span {
        memory: ptr::from_ref(memory).cast_mut(),
        offset,
        size,
    }
}

#[test]
#[ignore = "requires modern Vulkan; C2 per-entry scope lowering, whole-call rejection, split matching and GPU replay"]
fn gpu_dependency_entries() {
    use crate::foundation as api;
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let snapshot = snapshot(&instance, physical).unwrap();
        if snapshot.features.baseline_supported == 0 || snapshot.features.available & RASTER == 0 {
            continue;
        }
        let Some(q) = snapshot
            .queues
            .iter()
            .find(|q| q.count > 0 && q.flags & (GRAPHICS | COMPUTE) == GRAPHICS | COMPUTE)
        else {
            continue;
        };
        let domain = q.domain;
        let adapter = Adapter {
            instance: instance.clone(),
            physical,
            snapshot,
        };
        let requests = [QueueRequest {
            domain,
            count: 1,
            priority: 0.5,
        }];
        let mut d = Device::create(&adapter, &requests, RASTER).unwrap();
        intercept(&mut d);
        let mut reported = limits(domain, GRAPHICS | COMPUTE | TRANSFER, RASTER);
        let expected = reported;
        reported.flags = 0;
        let mut q = Query {
            header: Record::new::<Query>(DEPENDENCY_LIMITS),
            data: ptr::from_mut(&mut reported).cast(),
            capacity: 1,
            count: 99,
            element_size: size_of::<DependencyLimits>() as u32,
            reserved: 0,
        };
        assert_eq!(unsafe { api::ogpu_next_device_query(&*d, &mut q) }, OK);
        assert_eq!(q.count, 1);
        assert_eq!(reported, expected);
        let sentinel = HostRequirements {
            size: 991,
            alignment: 32,
        };
        let mut req = sentinel;
        assert_eq!(
            unsafe { api::ogpu_next_barrier_scratch_requirements(ptr::null(), 1, 0, 0, &mut req) },
            INVALID
        );
        assert_eq!(req, sentinel);
        assert_eq!(
            unsafe { api::ogpu_next_split_scratch_requirements(&*d, 1, ptr::null(), &mut req) },
            INVALID
        );
        assert_eq!(req, sentinel);
        assert_eq!(
            unsafe { api::ogpu_next_split_scratch_requirements(&*d, 0, ptr::null(), &mut req) },
            OK
        );
        assert_eq!(
            req,
            HostRequirements {
                size: 0,
                alignment: 1
            }
        );

        let m = memory(&d);
        let other = Device::create(&adapter, &requests, RASTER).unwrap();
        let foreign = memory(&other);
        let im_desc = ImageDesc {
            header: Record::new::<ImageDesc>(IMAGE_DESC),
            format: 3,
            dimension: 2,
            mip_count: 1,
            layer_count: 1,
            sample_count: 1,
            extent: Extent { x: 4, y: 4, z: 1 },
            usage: 3,
            flags: 0,
            view_formats: ptr::null(),
            view_format_count: 0,
            concurrent_domain_count: 0,
            concurrent_domains: ptr::null(),
        };
        let im = unsafe { Image::create(&d, &im_desc) }.unwrap();
        let backing = d
            .snapshot
            .memory_types
            .iter()
            .find_map(|t| im.dedicated_memory(t.id).ok())
            .unwrap();
        // Dedicated backing is bound by the caller just like ordinary image storage.
        let mut req = Requirements::default();
        unsafe { d.image_requirements(&im_desc, &mut req) }.unwrap();
        unsafe { im.bind(span(&backing, 0, req.size)) }.unwrap();
        let image = ImageBarrier {
            scope: BarrierScope {
                stages_after: COPY_SCOPE,
                access_after: 8,
                ..Default::default()
            },
            image: ptr::from_ref(&*im).cast_mut(),
            range: Subresources {
                aspects: 1,
                first_mip: 0,
                mip_count: 1,
                first_layer: 0,
                layer_count: 1,
            },
            old_state: 0,
            new_state: 1,
            source_domain: u32::MAX,
            destination_domain: u32::MAX,
            discard: 1,
            reserved: 0,
        };
        let globals = [
            BarrierScope {
                stages_before: 4,
                stages_after: 32,
                access_before: 1 << 20,
                access_after: 256,
            },
            BarrierScope {
                stages_before: 64,
                stages_after: 16,
                access_before: 1 << 13,
                access_after: 1 << 18,
            },
        ];
        let ranges = [memory_barrier(span(&m, 32, 64), copy_edge())];
        let dep = Dependency {
            global_count: 2,
            global: globals.as_ptr(),
            memory_count: 1,
            memory: ranges.as_ptr(),
            image_count: 1,
            images: &image,
            flags: 0,
        };
        let mut storage = Storage::new(barrier_requirements(&d, 2, 1, 1).unwrap());
        let mut a = arena(&d, domain, 1);
        let p = begin(&mut a);
        let list = unsafe { &*p };
        unsafe { list.barrier(&Dependency::default(), empty_storage()) }.unwrap();
        assert!(BARRIERS.with_borrow(|v| v.is_empty()));
        unsafe {
            api::ogpu_next_barrier(p, &dep, storage.span());
        }
        assert_eq!(list.error.get(), OK);
        let seen = BARRIERS.with_borrow(|v| {
            assert_eq!(v.len(), 1);
            v[0].clone()
        });
        assert_eq!(
            seen.globals,
            vec![
                [
                    vk::VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,
                    vk::VK_PIPELINE_STAGE_2_DRAW_INDIRECT_BIT,
                    vk::VK_ACCESS_2_SHADER_STORAGE_WRITE_BIT,
                    vk::VK_ACCESS_2_INDIRECT_COMMAND_READ_BIT
                ],
                [
                    vk::VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
                    vk::VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT,
                    vk::VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT,
                    vk::VK_ACCESS_2_SHADER_SAMPLED_READ_BIT
                ]
            ]
        );
        assert_eq!(
            seen.memory[0].0,
            [
                vk::VK_PIPELINE_STAGE_2_COPY_BIT,
                vk::VK_PIPELINE_STAGE_2_COPY_BIT,
                vk::VK_ACCESS_2_TRANSFER_WRITE_BIT,
                vk::VK_ACCESS_2_TRANSFER_READ_BIT
            ]
        );
        assert_eq!(
            (seen.memory[0].2, seen.memory[0].3, seen.memory[0].4),
            (64, u32::MAX, u32::MAX)
        );
        assert_eq!(
            seen.images[0],
            (
                [
                    0,
                    vk::VK_PIPELINE_STAGE_2_COPY_BIT,
                    0,
                    vk::VK_ACCESS_2_TRANSFER_WRITE_BIT
                ],
                vk::VkImageLayout_VK_IMAGE_LAYOUT_UNDEFINED,
                vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL
            )
        );
        for bad in [
            ImageBarrier {
                reserved: 1,
                ..image
            },
            ImageBarrier {
                range: Subresources {
                    first_mip: 1,
                    ..image.range
                },
                ..image
            },
            ImageBarrier {
                destination_domain: domain,
                ..image
            },
        ] {
            assert_eq!(
                unsafe {
                    list.barrier(
                        &Dependency {
                            images: &bad,
                            ..dep
                        },
                        storage.span(),
                    )
                },
                Err(INVALID)
            );
        }
        for bad in [
            MemoryBarrier {
                range: span(&m, 1020, 8),
                ..ranges[0]
            },
            MemoryBarrier {
                range: span(&foreign, 0, 64),
                ..ranges[0]
            },
            MemoryBarrier {
                source_domain: domain,
                ..ranges[0]
            },
        ] {
            assert_eq!(
                unsafe {
                    list.barrier(
                        &Dependency {
                            memory: &bad,
                            ..dep
                        },
                        storage.span(),
                    )
                },
                Err(INVALID)
            );
        }
        assert_eq!(
            unsafe {
                list.barrier(
                    &dep,
                    HostSpan {
                        size: storage.size - 1,
                        ..storage.span()
                    },
                )
            },
            Err(CAPACITY)
        );
        assert_eq!(
            unsafe {
                list.barrier(
                    &dep,
                    HostSpan {
                        data: storage.span().data.cast::<u8>().wrapping_add(1).cast(),
                        size: storage.size,
                    },
                )
            },
            Err(INVALID)
        );
        assert_eq!(
            unsafe { list.barrier(&Dependency { flags: 1, ..dep }, storage.span()) },
            Err(UNSUPPORTED)
        );
        assert_eq!(BARRIERS.with_borrow(Vec::len), 1); // No valid prefix leaked.
        let execution = [BarrierScope {
            stages_before: COPY_SCOPE,
            stages_after: 4,
            ..Default::default()
        }];
        unsafe { list.barrier(&global(&execution), storage.span()) }.unwrap();
        assert_eq!(
            BARRIERS.with_borrow(|v| v.last().unwrap().globals.clone()),
            vec![[
                vk::VK_PIPELINE_STAGE_2_COPY_BIT,
                vk::VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,
                0,
                0
            ]]
        );
        list.end().unwrap();
        a.reset(false).unwrap();
        let rejected = begin(&mut a);
        let mut out = ptr::dangling_mut();
        let calls = BARRIERS.with_borrow(Vec::len);
        unsafe {
            api::ogpu_next_barrier(rejected, &Dependency { flags: 1, ..dep }, storage.span());
        }
        assert_eq!(
            unsafe { api::ogpu_next_commands_end(rejected, &mut out) },
            UNSUPPORTED
        );
        assert!(out.is_null());
        assert_eq!(BARRIERS.with_borrow(Vec::len), calls);
        a.reset(false).unwrap();

        // Execute changed-host-data replay with explicit event reset, two split
        // descriptions and independent work between release and acquire. One
        // scratch span owns every native header and nested barrier translation.
        let split_a = Split::create(&d).unwrap();
        let split_b = Split::create(&d).unwrap();
        let foreign_split = Split::create(&other).unwrap();
        let edge_a = [memory_barrier(span(&m, 0, 64), copy_edge())];
        let edge_b = [copy_edge()];
        let deps = [range_dep(&edge_a), global(&edge_b)];
        let waits = [
            SplitDependency {
                split: &*split_a,
                dependency: &deps[0],
            },
            SplitDependency {
                split: &*split_b,
                dependency: &deps[1],
            },
        ];
        let mut wait_storage = Storage::new(unsafe { split_requirements(&d, &waits) }.unwrap());
        let mut producer = arena(&d, domain, 1);
        let mut consumer = arena(&d, domain, 1);
        let lists = [begin(&mut producer), begin(&mut consumer)];
        let p = unsafe { &*lists[0] };
        let c = unsafe { &*lists[1] };
        p.split_reset(&split_a, 1).unwrap();
        p.split_reset(&split_b, 1).unwrap();
        unsafe {
            p.barrier(
                &global(&[BarrierScope {
                    stages_before: 1,
                    stages_after: 1,
                    ..Default::default()
                }]),
                storage.span(),
            )
        }
        .unwrap();
        unsafe { p.fill(span(&m, 0, 64), 0x11223344) }.unwrap();
        unsafe { p.split_release(&split_a, &deps[0], storage.span()) }.unwrap();
        unsafe { p.fill(span(&m, 128, 64), 0x55667788) }.unwrap();
        unsafe { p.split_release(&split_b, &deps[1], storage.span()) }.unwrap();
        unsafe { p.fill(span(&m, 256, 64), 0xaabbccdd) }.unwrap();
        p.end().unwrap();
        unsafe { c.split_acquire(&[], empty_storage()) }.unwrap();
        assert!(WAITS.with_borrow(|v| v.is_empty()));
        let invalid = [
            waits[0],
            SplitDependency {
                split: &*foreign_split,
                ..waits[1]
            },
        ];
        assert_eq!(
            unsafe { c.split_acquire(&invalid, wait_storage.span()) },
            Err(INVALID)
        );
        let host_fields = [host_edge()];
        let host = global(&host_fields);
        assert_eq!(
            unsafe { c.split_release(&split_a, &host, storage.span()) },
            Err(INVALID)
        );
        let invalid = [
            waits[0],
            SplitDependency {
                dependency: &host,
                ..waits[1]
            },
        ];
        assert_eq!(
            unsafe { c.split_acquire(&invalid, wait_storage.span()) },
            Err(INVALID)
        );
        assert_eq!(
            unsafe {
                c.split_acquire(
                    &waits,
                    HostSpan {
                        size: wait_storage.size - 1,
                        ..wait_storage.span()
                    },
                )
            },
            Err(CAPACITY)
        );
        assert!(WAITS.with_borrow(|v| v.is_empty()));
        assert_eq!(SETS.with_borrow(Vec::len), 2);
        unsafe { c.split_acquire(&waits, wait_storage.span()) }.unwrap();
        assert_eq!(
            WAITS.with_borrow(|v| {
                assert_eq!(v.len(), 1);
                v[0].clone()
            }),
            SETS.with_borrow(Clone::clone)
        );
        unsafe { c.copy(span(&m, 512, 64), span(&m, 0, 64)) }.unwrap();
        unsafe { c.copy(span(&m, 640, 64), span(&m, 128, 64)) }.unwrap();
        unsafe { c.copy(span(&m, 768, 64), span(&m, 384, 64)) }.unwrap();
        unsafe { c.barrier(&host, storage.span()) }.unwrap();
        c.end().unwrap();
        let timeline = Timeline::create(&d, 0).unwrap();
        let mut submit_storage = Storage::new(submit_scratch(2, 0, 1).unwrap());
        let mapping = m.map(0, 1024).unwrap();
        for replay in 1..=2u32 {
            let words = unsafe { std::slice::from_raw_parts_mut(mapping.data.cast::<u32>(), 256) };
            let guard = 0xf0f00000 | replay;
            words.fill(guard);
            for (i, word) in words[96..112].iter_mut().enumerate() {
                *word = replay * 100 + i as u32;
            }
            m.cache(0, 1024, true).unwrap();
            let signal = SyncPoint {
                point: Point {
                    timeline: ptr::from_ref(&*timeline).cast_mut(),
                    value: u64::from(replay),
                },
                stages: 1,
            };
            let scratch = submit_storage.span();
            let submit = SubmitDesc {
                header: Record::new::<SubmitDesc>(SUBMIT_DESC),
                list_count: 2,
                wait_count: 0,
                signal_count: 1,
                lists: lists.as_ptr(),
                waits: ptr::null(),
                signals: &signal,
                scratch: scratch.data,
                scratch_size: scratch.size,
            };
            unsafe { d.queue(domain, 0).unwrap().submit(&submit) }.unwrap();
            timeline.wait(u64::from(replay), 10_000_000_000).unwrap();
            m.cache(0, 1024, false).unwrap();
            for (i, &word) in words.iter().enumerate() {
                let expected = match i {
                    0..=15 | 128..=143 => 0x11223344,
                    32..=47 | 160..=175 => 0x55667788,
                    64..=79 => 0xaabbccdd,
                    96..=111 => replay * 100 + (i - 96) as u32,
                    192..=207 => replay * 100 + (i - 192) as u32,
                    _ => guard,
                };
                assert_eq!(word, expected, "replay={replay} word={i}");
            }
        }
        // Discard all recorded uses before releasing bound resources/backing.
        drop(consumer);
        drop(producer);
        drop(a);
        drop(im);
        drop(backing);
        tested += 1;
    }
    assert!(tested > 0, "No suitable device; not a passing skip");
}
