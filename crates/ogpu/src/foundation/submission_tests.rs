use super::*;
use std::cell::RefCell;

const SENTINEL: SubmitOutcome = SubmitOutcome {
    status: 123,
    acceptance: 456,
};
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
            words: vec![0xfeedc0de; req.size.div_ceil(8) as usize + 2],
            size: req.size,
        }
    }
    fn span(&mut self) -> HostSpan {
        HostSpan {
            data: unsafe { self.words.as_mut_ptr().add(1).cast() },
            size: self.size,
        }
    }
    fn guards(&self) {
        assert_eq!(self.words[0], 0xfeedc0de);
        assert_eq!(*self.words.last().unwrap(), 0xfeedc0de);
    }
    fn overwrite(&mut self) {
        let end = self.words.len() - 1;
        self.words[1..end].fill(0xdeadbeef);
    }
}

#[test]
fn scratch_uses_counts_only_and_preserves_batch_storage() {
    let zero = layout(&[]).unwrap();
    assert_eq!((zero.size(), zero.align()), (0, 1));
    // Even a synchronization-only empty batch has its native batch header.
    assert_eq!(
        layout(&[SubmitDesc::default()]).unwrap().size(),
        size_of::<vk::VkSubmitInfo2>()
    );
    let desc = SubmitDesc {
        list_count: 3,
        wait_count: 2,
        signal_count: 4,
        binary_wait_count: 5,
        binary_signal_count: 6,
        lists: ptr::dangling(),
        waits: ptr::dangling(),
        signals: ptr::dangling(),
        binary_waits: ptr::dangling(),
        binary_signals: ptr::dangling(),
        flags: 0,
    };
    let (nested, wo, so) = entry_layout(&desc).unwrap();
    assert!(wo >= 3 * size_of::<vk::VkCommandBufferSubmitInfo>());
    assert!(so >= wo + 7 * size_of::<vk::VkSemaphoreSubmitInfo>());
    assert!(nested.size() >= so + 10 * size_of::<vk::VkSemaphoreSubmitInfo>());
    assert_eq!(wo % align_of::<vk::VkSemaphoreSubmitInfo>(), 0);
    assert_eq!(so % align_of::<vk::VkSemaphoreSubmitInfo>(), 0);
    let all = layout(&[desc; 3]).unwrap();
    assert!(all.size() >= 3 * (size_of::<vk::VkSubmitInfo2>() + nested.size()));
    assert!(matches!(
        layout(&[desc, SubmitDesc { flags: 1, ..desc }]),
        Err(UNSUPPORTED)
    ));
    for bad in [
        SubmitDesc {
            wait_count: u32::MAX,
            binary_wait_count: 1,
            ..desc
        },
        SubmitDesc {
            signal_count: u32::MAX,
            binary_signal_count: 1,
            ..desc
        },
    ] {
        assert!(matches!(layout(&[desc, bad]), Err(INVALID)));
    }
    let huge = layout(
        &[SubmitDesc {
            list_count: u32::MAX,
            wait_count: u32::MAX,
            signal_count: u32::MAX,
            ..Default::default()
        }; 3],
    );
    if usize::BITS == 64 {
        assert!(huge.unwrap().size() > u32::MAX as usize);
    } else {
        assert!(huge.is_err());
    }
}

#[test]
fn native_acceptance_is_not_inferred_from_generic_status() {
    assert_eq!(
        classify(vk::VkResult_VK_SUCCESS),
        SubmitOutcome {
            status: OK,
            acceptance: SUBMIT_ACCEPTED
        }
    );
    for code in [
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
        vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
    ] {
        assert_eq!(classify(code), SubmitOutcome::rejected(OUT_OF_MEMORY));
    }
    for (code, expected) in [
        (vk::VkResult_VK_ERROR_DEVICE_LOST, DEVICE_LOST),
        (vk::VkResult_VK_ERROR_UNKNOWN, BACKEND_ERROR),
        (vk::VkResult_VK_ERROR_VALIDATION_FAILED, BACKEND_ERROR),
        // Unexpected native returns are also uncertain, not local rejection.
        (vk::VkResult_VK_ERROR_FEATURE_NOT_PRESENT, UNSUPPORTED),
        (vk::VkResult_VK_NOT_READY, NOT_READY),
    ] {
        assert_eq!(
            classify(code),
            SubmitOutcome {
                status: expected,
                acceptance: SUBMIT_UNKNOWN
            }
        );
    }
}

#[test]
fn unbound_queue_rejects_and_writes_every_outcome() {
    let queue = Queue {
        handle: ptr::null_mut(),
        device: AtomicPtr::new(ptr::null_mut()),
        domain: 0,
        index: 0,
    };
    let batches = [SubmitDesc::default(); 3];
    let mut outcomes = [SENTINEL; 3];
    assert_eq!(
        unsafe { queue.submit_descriptions(&batches, &mut outcomes, empty_storage()) },
        Err(INVALID)
    );
    all(&outcomes, INVALID, SUBMIT_REJECTED);
    assert_eq!(
        unsafe { queue.submit_descriptions(&[], &mut [], empty_storage()) },
        Err(INVALID)
    );
}

#[test]
fn c_submission_accepts_uninitialized_outcomes() {
    let batches = [SubmitDesc::default(); 3];
    let mut outputs = [std::mem::MaybeUninit::<SubmitOutcome>::uninit(); 3];
    let status = unsafe {
        crate::foundation::ogpu_next_queue_submit(
            ptr::null_mut(),
            3,
            batches.as_ptr(),
            outputs.as_mut_ptr().cast(),
            empty_storage(),
        )
    };
    assert_eq!(status, INVALID);
    for output in outputs {
        assert_eq!(
            unsafe { output.assume_init() },
            SubmitOutcome::rejected(INVALID)
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    lists: Vec<usize>,
    waits: Vec<(usize, u64, u64)>,
    signals: Vec<(usize, u64, u64)>,
}
thread_local! {
    static NATIVE: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    static FAILURE: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_SUCCESS) };
    static CALLS: RefCell<Vec<Vec<Seen>>> = const { RefCell::new(Vec::new()) };
    static BOUNDS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}
unsafe fn captured<T>(data: *const T, count: u32) -> &'static [T] {
    if count == 0 {
        return &[];
    }
    let (base, size) = BOUNDS.get();
    let start = data as usize;
    assert!(start >= base);
    assert!(start + count as usize * size_of::<T>() <= base + size);
    assert_eq!(start % align_of::<T>(), 0);
    unsafe { std::slice::from_raw_parts(data, count as usize) }
}
unsafe extern "C" fn submit(
    queue: vk::VkQueue,
    count: u32,
    infos: *const vk::VkSubmitInfo2,
    fence: vk::VkFence,
) -> vk::VkResult {
    assert!(fence.is_null());
    let mut batch = Vec::new();
    for info in unsafe { captured(infos, count) } {
        assert_eq!(
            info.sType,
            vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO_2
        );
        assert!(info.pNext.is_null());
        assert_eq!(info.flags, 0);
        let lists = unsafe { captured(info.pCommandBufferInfos, info.commandBufferInfoCount) }
            .iter()
            .map(|l| {
                assert_eq!(
                    l.sType,
                    vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO
                );
                assert!(l.pNext.is_null());
                assert_eq!(l.deviceMask, 0);
                l.commandBuffer as usize
            })
            .collect();
        let points = |ptr, n| {
            unsafe { captured::<vk::VkSemaphoreSubmitInfo>(ptr, n) }
                .iter()
                .map(|p| {
                    assert_eq!(
                        p.sType,
                        vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO
                    );
                    assert!(p.pNext.is_null());
                    assert_eq!(p.deviceIndex, 0);
                    (p.semaphore as usize, p.value, p.stageMask)
                })
                .collect()
        };
        batch.push(Seen {
            lists,
            waits: points(info.pWaitSemaphoreInfos, info.waitSemaphoreInfoCount),
            signals: points(info.pSignalSemaphoreInfos, info.signalSemaphoreInfoCount),
        });
    }
    CALLS.with_borrow_mut(|v| v.push(batch));
    if FAILURE.get() != vk::VkResult_VK_SUCCESS {
        return FAILURE.get();
    }
    unsafe { NATIVE.get().unwrap()(queue, count, infos, fence) }
}
fn intercept(d: &mut Device) {
    NATIVE.set(d.f.vkQueueSubmit2);
    d.f.vkQueueSubmit2 = Some(submit);
    FAILURE.set(vk::VkResult_VK_SUCCESS);
    CALLS.with_borrow_mut(Vec::clear);
}
fn arena(d: &Device, domain: u32, level: u32) -> Box<Arena> {
    Arena::create(
        d,
        &ArenaDesc {
            header: Record::new::<ArenaDesc>(ARENA_DESC),
            domain,
            list_capacity: 3,
            level,
            reserved: 0,
        },
    )
    .unwrap()
}
fn begin(arena: &mut Arena, replay_mode: u32, level: u32) -> *mut List {
    arena
        .begin(&RecordingDesc {
            header: Record::new::<RecordingDesc>(RECORDING_DESC),
            replay_mode,
            level,
            inheritance: ptr::null(),
        })
        .unwrap()
}
fn point(t: &Timeline, value: u64, stages: u64) -> SyncPoint {
    SyncPoint {
        point: Point {
            timeline: ptr::from_ref(t).cast_mut(),
            value,
        },
        stages,
    }
}
fn memory(d: &Device) -> Box<Memory> {
    let mut desc = MemoryDesc {
        header: Record::new::<MemoryDesc>(MEMORY_DESC),
        size: 1024,
        alignment: 256,
        usage: 3,
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
fn span(m: &Memory, offset: u64, size: u64) -> Span {
    Span {
        memory: ptr::from_ref(m).cast_mut(),
        offset,
        size,
    }
}
fn all(outcomes: &[SubmitOutcome], status: Status, acceptance: u32) {
    assert!(!outcomes.is_empty());
    for result in outcomes {
        assert_eq!(*result, SubmitOutcome { status, acceptance });
    }
}
unsafe fn reject_last(
    queue: &Queue,
    batches: &[SubmitDesc; 3],
    last: SubmitDesc,
    storage: HostSpan,
    error: Status,
) {
    let mut bad = *batches;
    bad[2] = last;
    let mut results = [SENTINEL; 3];
    assert_eq!(
        unsafe { queue.submit_descriptions(&bad, &mut results, storage) },
        Err(error)
    );
    all(&results, error, SUBMIT_REJECTED);
    assert!(CALLS.with_borrow(Vec::is_empty));
}

#[test]
#[ignore = "requires modern Vulkan; unified scratch, exact mixed-batch lowering, atomic rejection, OOM retry and uncertain acceptance"]
fn gpu_unified_submission() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let snapshot = snapshot(&instance, physical).unwrap();
        if snapshot.features.baseline_supported == 0 {
            continue;
        }
        let Some(q) = snapshot
            .queues
            .iter()
            .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
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
        let mut d = Device::create(&adapter, &requests, 0).unwrap();
        intercept(&mut d);
        let m = memory(&d);
        let mapping = m.map(0, 1024).unwrap();
        let words = unsafe { std::slice::from_raw_parts_mut(mapping.data.cast::<u32>(), 256) };
        words.fill(0xaabbccdd);
        m.cache(0, 1024, true).unwrap();
        let gate = Timeline::create(&d, 7).unwrap();
        let bridge = Timeline::create(&d, 0).unwrap();
        let done = Timeline::create(&d, 0).unwrap();
        let complete = Timeline::create(&d, 0).unwrap();
        let binary = Binary::create(&d).unwrap();
        let mut pool = arena(&d, domain, 0);
        let producer = begin(&mut pool, 1, 0);
        unsafe { (&*producer).fill(span(&m, 16, 16), 0x12345678) }.unwrap();
        unsafe { &*producer }.end().unwrap();
        let consumer = begin(&mut pool, 2, 0);
        unsafe { (&*consumer).copy(span(&m, 64, 16), span(&m, 16, 16)) }.unwrap();
        let host = BarrierScope {
            stages_before: COPY_SCOPE,
            stages_after: STAGE_HOST,
            access_before: ACCESS_COPY_WRITE,
            access_after: ACCESS_HOST_READ,
        };
        let dep = Dependency {
            global_count: 1,
            global: &host,
            ..Default::default()
        };
        let mut barrier_storage = Storage::new(barrier_scratch(&d, 1, 0, 0).unwrap());
        unsafe { (&*consumer).barrier(&dep, barrier_storage.span()) }.unwrap();
        unsafe { &*consumer }.end().unwrap();
        let gate_wait = point(&gate, 7, STAGE_COMPUTE);
        let bridge_signal = point(&bridge, 1, STAGE_ALL);
        let middle_waits = [point(&bridge, 1, COPY_SCOPE), gate_wait];
        let done_signal = point(&done, 1, STAGE_ALL);
        let done_wait = point(&done, 1, STAGE_ALL);
        let complete_signal = point(&complete, 1, STAGE_ALL);
        let binary_signal = BinaryPoint {
            binary: ptr::from_ref(&*binary).cast_mut(),
            stages: STAGE_ALL,
        };
        let binary_wait = BinaryPoint {
            stages: COPY_SCOPE,
            ..binary_signal
        };
        let batches = [
            SubmitDesc {
                list_count: 1,
                lists: &producer,
                wait_count: 1,
                waits: &gate_wait,
                signal_count: 1,
                signals: &bridge_signal,
                binary_signal_count: 1,
                binary_signals: &binary_signal,
                ..Default::default()
            },
            SubmitDesc {
                list_count: 1,
                lists: &consumer,
                wait_count: 2,
                waits: middle_waits.as_ptr(),
                binary_wait_count: 1,
                binary_waits: &binary_wait,
                signal_count: 1,
                signals: &done_signal,
                ..Default::default()
            },
            SubmitDesc {
                wait_count: 1,
                waits: &done_wait,
                signal_count: 1,
                signals: &complete_signal,
                ..Default::default()
            },
        ];
        let mut storage = Storage::new(requirements(&d, &batches).unwrap());
        let scratch = storage.span();
        BOUNDS.set((scratch.data as usize, scratch.size as usize));
        let queue = d.queue(domain, 0).unwrap();
        assert_eq!(
            requirements(&d, &[]),
            Ok(HostRequirements {
                size: 0,
                alignment: 1
            })
        );
        unsafe { queue.submit_descriptions(&[], &mut [], empty_storage()) }.unwrap();
        let mut wrong_length = [SENTINEL; 2];
        assert_eq!(
            unsafe { queue.submit_descriptions(&batches, &mut wrong_length, scratch) },
            Err(INVALID)
        );
        all(&wrong_length, INVALID, SUBMIT_REJECTED);
        for (bad_storage, expected) in [
            (
                HostSpan {
                    size: scratch.size - 1,
                    ..scratch
                },
                CAPACITY,
            ),
            (
                HostSpan {
                    data: ptr::null_mut(),
                    ..scratch
                },
                INVALID,
            ),
            (
                HostSpan {
                    data: unsafe { scratch.data.cast::<u8>().add(1).cast() },
                    ..scratch
                },
                INVALID,
            ),
        ] {
            unsafe { reject_last(queue, &batches, batches[2], bad_storage, expected) };
        }
        // All of these fail in the final batch, after earlier valid translation.
        for (bad, expected) in [
            (
                SubmitDesc {
                    flags: 1,
                    ..batches[2]
                },
                UNSUPPORTED,
            ),
            (
                SubmitDesc {
                    binary_wait_count: u32::MAX,
                    ..batches[2]
                },
                INVALID,
            ),
            (
                SubmitDesc {
                    waits: ptr::null(),
                    ..batches[2]
                },
                INVALID,
            ),
            (
                SubmitDesc {
                    lists: ptr::null(),
                    list_count: 1,
                    ..batches[2]
                },
                INVALID,
            ),
            (
                SubmitDesc {
                    binary_waits: ptr::null(),
                    binary_wait_count: 1,
                    ..batches[2]
                },
                INVALID,
            ),
        ] {
            // This storage covers any valid enlarged batch, so pointer validation
            // is reached rather than rejected early by capacity checks.
            let mut extra = Storage::new(HostRequirements {
                size: scratch.size + 1024,
                alignment: 8,
            });
            unsafe { reject_last(queue, &batches, bad, extra.span(), expected) };
            extra.guards();
        }
        let other = Device::create(&adapter, &requests, 0).unwrap();
        let foreign_t = Timeline::create(&other, 0).unwrap();
        let foreign_b = Binary::create(&other).unwrap();
        let mut foreign_pool = arena(&other, domain, 0);
        let foreign_list = begin(&mut foreign_pool, 1, 0);
        unsafe { &*foreign_list }.end().unwrap();
        let mut secondary_pool = arena(&d, domain, 1);
        let secondary = begin(&mut secondary_pool, 1, 1);
        unsafe { &*secondary }.end().unwrap();
        let recording = begin(&mut pool, 1, 0);
        let mut extra = Storage::new(HostRequirements {
            size: scratch.size + 1024,
            alignment: 8,
        });
        for list in [foreign_list, secondary, recording, ptr::null_mut()] {
            unsafe {
                reject_last(
                    queue,
                    &batches,
                    SubmitDesc {
                        list_count: 1,
                        lists: &list,
                        ..batches[2]
                    },
                    extra.span(),
                    INVALID,
                )
            };
        }
        unsafe { &*recording }.end().unwrap();
        for bad in [
            point(&foreign_t, 0, STAGE_ALL),
            point(&done, 1, STAGE_HOST),
            SyncPoint {
                point: Point {
                    timeline: ptr::null_mut(),
                    value: 1,
                },
                stages: STAGE_ALL,
            },
        ] {
            unsafe {
                reject_last(
                    queue,
                    &batches,
                    SubmitDesc {
                        waits: &bad,
                        ..batches[2]
                    },
                    scratch,
                    INVALID,
                )
            };
            unsafe {
                reject_last(
                    queue,
                    &batches,
                    SubmitDesc {
                        signals: &bad,
                        ..batches[2]
                    },
                    scratch,
                    INVALID,
                )
            };
        }
        let unsupported = point(&done, 1, MESH_STAGE);
        unsafe {
            reject_last(
                queue,
                &batches,
                SubmitDesc {
                    waits: &unsupported,
                    ..batches[2]
                },
                scratch,
                UNSUPPORTED,
            )
        };
        for bad in [
            BinaryPoint {
                binary: ptr::from_ref(&*foreign_b).cast_mut(),
                stages: STAGE_ALL,
            },
            BinaryPoint {
                binary: ptr::null_mut(),
                stages: STAGE_ALL,
            },
            BinaryPoint {
                stages: STAGE_HOST,
                ..binary_wait
            },
        ] {
            unsafe {
                reject_last(
                    queue,
                    &batches,
                    SubmitDesc {
                        binary_wait_count: 1,
                        binary_waits: &bad,
                        ..batches[2]
                    },
                    extra.span(),
                    INVALID,
                )
            };
            unsafe {
                reject_last(
                    queue,
                    &batches,
                    SubmitDesc {
                        binary_signal_count: 1,
                        binary_signals: &bad,
                        ..batches[2]
                    },
                    extra.span(),
                    INVALID,
                )
            };
        }
        assert_eq!(unsafe { &*producer }.state.get(), 2);
        assert_eq!(complete.poll().unwrap(), 0);
        for code in [
            vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY,
        ] {
            FAILURE.set(code);
            let mut results = [SENTINEL; 3];
            assert_eq!(
                unsafe { queue.submit_descriptions(&batches, &mut results, scratch) },
                Err(OUT_OF_MEMORY)
            );
            all(&results, OUT_OF_MEMORY, SUBMIT_REJECTED);
            assert_eq!(d.ready(), Ok(()));
            assert_eq!(unsafe { &*producer }.state.get(), 2);
            assert_eq!(complete.poll().unwrap(), 0);
            m.cache(0, 1024, false).unwrap();
            assert!(words.iter().all(|w| *w == 0xaabbccdd));
        }
        FAILURE.set(vk::VkResult_VK_SUCCESS);
        let mut fresh = [std::mem::MaybeUninit::<SubmitOutcome>::uninit(); 3];
        assert_eq!(
            unsafe {
                crate::foundation::ogpu_next_queue_submit(
                    ptr::from_ref(queue).cast_mut(),
                    batches.len() as u32,
                    batches.as_ptr(),
                    fresh.as_mut_ptr().cast(),
                    scratch,
                )
            },
            OK
        );
        let mut results = fresh.map(|outcome| unsafe { outcome.assume_init() });
        all(&results, OK, SUBMIT_ACCEPTED);
        storage.guards();
        storage.overwrite();
        complete.wait(1, 10_000_000_000).unwrap();
        m.cache(0, 1024, false).unwrap();
        for (i, word) in words.iter().enumerate() {
            assert_eq!(
                *word,
                if (4..8).contains(&i) || (16..20).contains(&i) {
                    0x12345678
                } else {
                    0xaabbccdd
                }
            );
        }
        assert_eq!(unsafe { &*producer }.state.get(), 3);
        assert_eq!(unsafe { &*consumer }.state.get(), 2);
        let native_stage = |s| stages(&d, domain, s, true).unwrap();
        let expected = vec![
            Seen {
                lists: vec![unsafe { &*producer }.command as usize],
                waits: vec![(gate.handle as usize, 7, native_stage(STAGE_COMPUTE))],
                signals: vec![
                    (bridge.handle as usize, 1, native_stage(STAGE_ALL)),
                    (binary.handle as usize, 0, native_stage(STAGE_ALL)),
                ],
            },
            Seen {
                lists: vec![unsafe { &*consumer }.command as usize],
                waits: vec![
                    (bridge.handle as usize, 1, native_stage(COPY_SCOPE)),
                    (gate.handle as usize, 7, native_stage(STAGE_COMPUTE)),
                    (binary.handle as usize, 0, native_stage(COPY_SCOPE)),
                ],
                signals: vec![(done.handle as usize, 1, native_stage(STAGE_ALL))],
            },
            Seen {
                lists: vec![],
                waits: vec![(done.handle as usize, 1, native_stage(STAGE_ALL))],
                signals: vec![(complete.handle as usize, 1, native_stage(STAGE_ALL))],
            },
        ];
        CALLS.with_borrow(|calls| {
            assert_eq!(calls.len(), 3);
            for call in calls {
                assert_eq!(*call, expected);
            }
        });
        let count = CALLS.with_borrow(Vec::len);
        assert_eq!(
            unsafe { queue.submit_descriptions(&batches, &mut results, scratch) },
            Err(INVALID)
        );
        all(&results, INVALID, SUBMIT_REJECTED);
        assert_eq!(CALLS.with_borrow(Vec::len), count);
        // Replay the reusable list with changed input and no one-shot producer.
        words[4..8].fill(0x87654321);
        m.cache(0, 1024, true).unwrap();
        let replay_signal = point(&complete, 2, STAGE_ALL);
        let replay = SubmitDesc {
            list_count: 1,
            lists: &consumer,
            signal_count: 1,
            signals: &replay_signal,
            ..Default::default()
        };
        unsafe { queue.submit_descriptions(&[replay], &mut results[..1], scratch) }.unwrap();
        all(&results[..1], OK, SUBMIT_ACCEPTED);
        storage.overwrite();
        complete.wait(2, 10_000_000_000).unwrap();
        m.cache(0, 1024, false).unwrap();
        assert!(words[16..20].iter().all(|w| *w == 0x87654321));
        // No commands or timeline arrays are required in an individual batch.
        let final_signal = point(&complete, 3, STAGE_ALL);
        let binary_only = [
            SubmitDesc::default(),
            SubmitDesc {
                binary_signal_count: 1,
                binary_signals: &binary_signal,
                ..Default::default()
            },
            SubmitDesc {
                binary_wait_count: 1,
                binary_waits: &binary_wait,
                signal_count: 1,
                signals: &final_signal,
                ..Default::default()
            },
        ];
        unsafe { queue.submit_descriptions(&binary_only, &mut results, scratch) }.unwrap();
        all(&results, OK, SUBMIT_ACCEPTED);
        storage.overwrite();
        complete.wait(3, 10_000_000_000).unwrap();
        CALLS.with_borrow(|calls| {
            let last = calls.last().unwrap();
            assert_eq!(last.len(), 3);
            assert!(last.iter().all(|batch| batch.lists.is_empty()));
            assert!(last[0].waits.is_empty() && last[0].signals.is_empty());
            assert_eq!(
                last[1].signals,
                vec![(binary.handle as usize, 0, native_stage(STAGE_ALL))]
            );
            assert_eq!(
                last[2].waits,
                vec![(binary.handle as usize, 0, native_stage(COPY_SCOPE))]
            );
        });
        storage.guards();
        extra.guards();
        drop(pool);
        drop(secondary_pool);
        drop(foreign_pool);
        tested += 1;
    }
    assert!(tested > 0, "No suitable device; not a passing skip");
}

#[test]
#[ignore = "requires modern Vulkan; unknown acceptance protects all batches and poisons retries"]
fn gpu_unified_submission_unknown() {
    let instance = Arc::new(Instance::new().unwrap());
    let mut tested = 0;
    for physical in instance.physical_devices().unwrap() {
        let snapshot = snapshot(&instance, physical).unwrap();
        if snapshot.features.baseline_supported == 0 {
            continue;
        }
        let domain = snapshot
            .queues
            .iter()
            .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
            .unwrap()
            .domain;
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
        for (code, expected) in [
            (vk::VkResult_VK_ERROR_DEVICE_LOST, DEVICE_LOST),
            (vk::VkResult_VK_ERROR_UNKNOWN, BACKEND_ERROR),
            (vk::VkResult_VK_ERROR_VALIDATION_FAILED, BACKEND_ERROR),
        ] {
            let mut d = Device::create(&adapter, &requests, 0).unwrap();
            intercept(&mut d);
            let mut pool = arena(&d, domain, 0);
            let one = begin(&mut pool, 1, 0);
            unsafe { &*one }.end().unwrap();
            let reusable = begin(&mut pool, 2, 0);
            unsafe { &*reusable }.end().unwrap();
            let batches = [
                SubmitDesc {
                    list_count: 1,
                    lists: &one,
                    ..Default::default()
                },
                SubmitDesc {
                    list_count: 1,
                    lists: &reusable,
                    ..Default::default()
                },
                SubmitDesc::default(),
            ];
            let mut storage = Storage::new(requirements(&d, &batches).unwrap());
            let scratch = storage.span();
            BOUNDS.set((scratch.data as usize, scratch.size as usize));
            let queue = d.queue(domain, 0).unwrap();
            let mut outcomes = [SENTINEL; 3];
            FAILURE.set(code);
            assert_eq!(
                unsafe {
                    crate::foundation::ogpu_next_queue_submit(
                        ptr::from_ref(queue).cast_mut(),
                        batches.len() as u32,
                        batches.as_ptr(),
                        outcomes.as_mut_ptr(),
                        scratch,
                    )
                },
                expected
            );
            all(&outcomes, expected, SUBMIT_UNKNOWN);
            assert_eq!(unsafe { &*one }.state.get(), 3);
            assert_eq!(d.ready(), Err(DEVICE_LOST));
            assert_eq!(
                unsafe { queue.submit_descriptions(&batches, &mut outcomes, scratch) },
                Err(DEVICE_LOST)
            );
            all(&outcomes, DEVICE_LOST, SUBMIT_REJECTED);
            assert_eq!(
                unsafe { queue.submit_descriptions(&[], &mut [], empty_storage()) },
                Err(DEVICE_LOST)
            );
            assert_eq!(requirements(&d, &[]), Err(DEVICE_LOST));
            assert_eq!(CALLS.with_borrow(Vec::len), 1);
            storage.guards();
            // The interceptor returned failure without executing. Real device is
            // idle; this is not permission to release real UNKNOWN uses early.
            drop(pool);
        }
        tested += 1;
    }
    assert!(tested > 0, "No suitable device; not a passing skip");
}
