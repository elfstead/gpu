//! Standalone native Rust consumer: no C entry-point calls or private backend API.
use ogpu::foundation::{rust as gpu, types::*};
use std::{ffi::CStr, ptr};
mod extended;

fn dependency(before: u64, after: u64, read: u64, write: u64) -> Dependency {
    Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before,
        after,
        global_before: read,
        global_after: write,
        memory_count: 0,
        image_count: 0,
        flags: 0,
        memory: ptr::null(),
        images: ptr::null(),
        scratch: ptr::null_mut(),
        scratch_size: 0,
    }
}

/// No automatic resource teardown: on uncertain submission/wait failure, the
/// executable reports failure and exits without prematurely freeing GPU objects.
pub fn run() -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    // SAFETY: adapter name is a NUL-terminated foundation info record.
    let name = unsafe { CStr::from_ptr(gpu::adapter_info(adapter).name.as_ptr()) };
    println!("NATIVE_RUST_DEVICE={}", name.to_string_lossy());
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
        .ok_or(UNSUPPORTED)?
        .domain;
    let requests = [QueueRequest {
        domain,
        count: 1,
        priority: 1.0,
    }];
    assert_eq!(gpu::device_create(adapter, &[], 0), Err(INVALID));
    let device = gpu::device_create(adapter, &requests, 0)?;

    // SAFETY: this single-threaded consumer owns every handle, keeps all parents
    // live, and retains every resource/list until an ALL-stage timeline wait.
    // Mapped host reads occur only after a transfer-to-host barrier, completion
    // and explicit invalidation. No Rust references alias GPU-writable storage.
    unsafe {
        let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
        let mut desc = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: 4096,
            alignment: 4,
            usage: USAGE_COPY_SRC | USAGE_COPY_DST | USAGE_STORAGE | USAGE_INDIRECT,
            memory_type: 0,
            kind: MEMORY_LINEAR,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let mut requirements = Requirements::default();
        gpu::memory_requirements(device, &desc, &mut requirements)?;
        let mut compatible = vec![0; requirements.compatible_type_count as usize];
        requirements.compatible_types = compatible.as_mut_ptr();
        requirements.compatible_type_capacity = compatible.len() as u32;
        gpu::memory_requirements(device, &desc, &mut requirements)?;
        desc.memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| m.properties & MEMORY_HOST_VISIBLE != 0 && compatible.contains(&m.id))
            .ok_or(UNSUPPORTED)?
            .id;
        let mut malformed = desc;
        malformed.alignment = 3;
        assert_eq!(gpu::memory_create(device, &malformed), Err(INVALID));
        let memory = gpu::memory_create(device, &desc)?;
        let mapping = gpu::memory_map(memory.span(0, 4096))?;
        assert_ne!(gpu::memory_address(memory.span(0, 512))?, 0);
        let timeline = gpu::timeline_create(device, 0)?;
        assert_eq!(gpu::timeline_wait(timeline, 1, 0), Err(TIMEOUT));
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
        let recording = RecordingDesc {
            header: Record::new::<RecordingDesc>(RECORDING_DESC),
            replay_mode: 1,
            level: 0,
            inheritance: ptr::null(),
        };
        // Invalid recording is poisoned and rejected at end; reset permits retry.
        let invalid = gpu::commands_begin(arena, &recording)?;
        gpu::fill_memory(invalid, memory.span(1, 256), 0);
        assert_eq!(gpu::commands_end(invalid), Err(INVALID));
        gpu::arena_reset(arena)?;

        let scratch_req = gpu::submit_scratch_requirements(1, 0, 1)?;
        // Caller-owned, once-allocated, suitably aligned translation storage.
        assert!(scratch_req.alignment <= align_of::<u64>() as u64);
        let mut scratch = vec![0u64; scratch_req.size.div_ceil(8) as usize];
        for (index, pattern) in [0x1234_abcd, 0x8765_4321, 0x1234_abcd]
            .into_iter()
            .enumerate()
        {
            let list = gpu::commands_begin(arena, &recording)?;
            gpu::fill_memory(list, memory.span(0, 256), pattern);
            gpu::barrier(
                list,
                &dependency(
                    STAGE_TRANSFER,
                    STAGE_TRANSFER,
                    ACCESS_COPY_WRITE,
                    ACCESS_COPY_READ,
                ),
            );
            gpu::copy_memory(list, memory.span(256, 256), memory.span(0, 256));
            gpu::barrier(
                list,
                &dependency(
                    STAGE_TRANSFER,
                    STAGE_HOST,
                    ACCESS_COPY_WRITE,
                    ACCESS_HOST_READ,
                ),
            );
            let list = gpu::commands_end(list)?;
            let value = index as u64 + 1;
            let signal = SyncPoint {
                point: Point {
                    timeline: timeline.as_raw(),
                    value,
                },
                stages: STAGE_ALL,
            };
            // Insufficient scratch rejects before submission, preserving retry.
            assert_eq!(
                gpu::queue_submit(
                    queue,
                    &[list],
                    &[],
                    &[signal],
                    HostSpan {
                        data: scratch.as_mut_ptr().cast(),
                        size: 0,
                    }
                ),
                Err(CAPACITY)
            );
            gpu::queue_submit(
                queue,
                &[list],
                &[],
                &[signal],
                HostSpan {
                    data: scratch.as_mut_ptr().cast(),
                    size: scratch_req.size,
                },
            )?;
            gpu::timeline_wait(timeline, value, 10_000_000_000)?;
            assert!(gpu::timeline_poll(timeline)? >= value);
            gpu::memory_invalidate(memory.span(0, 512))?;
            for word in 0..128 {
                assert_eq!(mapping.data.cast::<u32>().add(word).read(), pattern);
            }
            gpu::arena_reset(arena)?;
        }
        extended::run(extended::Context {
            device,
            queue,
            arena,
            timeline,
            memory,
            mapping,
        })?;
        gpu::arena_destroy(arena);
        gpu::timeline_destroy(timeline);
        gpu::memory_unmap(memory);
        gpu::memory_destroy(memory);
        gpu::device_destroy(device);
    }
    println!("NATIVE_RUST_PASS phases=3 checked_words=384 poisoned_recording=1 submit_retries=3");
    Ok(())
}

#[allow(dead_code)] // Also compiled as the integration-test consumer module.
fn main() {
    if let Err(status) = run() {
        eprintln!("NATIVE_RUST_FAIL status={status}; no unsupported-device passing skip");
        std::process::exit(1);
    }
}
