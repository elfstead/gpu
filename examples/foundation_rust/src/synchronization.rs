//! Headless mixed timeline/binary handoffs; all storage and reuse are caller-owned.
use super::{dependency, gpu};
use ogpu::foundation::types::*;
use std::ptr;

/// On an uncertain error, propagate without destroying potentially pending GPU
/// objects. This executable exits; it is not a general resource-recovery runtime.
pub unsafe fn run(
    device: gpu::DeviceHandle,
    queue: gpu::QueueHandle,
    domain: u32,
    memory: gpu::MemoryHandle,
    mapping: Mapping,
) -> gpu::Result<()> {
    unsafe {
        for signaled in [false, true] {
            let fence = gpu::fence_create(device, signaled)?;
            assert_eq!(gpu::fence_poll(fence)?, signaled);
            assert_eq!(
                gpu::fence_wait(fence, 0),
                if signaled { Ok(()) } else { Err(TIMEOUT) }
            );
            gpu::fence_reset(fence)?;
            assert!(!gpu::fence_poll(fence)?);
            assert_eq!(gpu::fence_wait(fence, 0), Err(TIMEOUT));
            gpu::fence_reset(fence)?; // Idle unsignaled reset is valid.
            gpu::fence_destroy(fence);
        }
        let binary = gpu::binary_create(device)?;
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
        let req = gpu::submit_scratch_requirements(1, 2, 2)?;
        let batch_req = gpu::submit_batch_scratch_requirements(2)?;
        assert!(req.alignment <= 8 && batch_req.alignment <= 8);
        let mut first_storage = vec![0u64; req.size.div_ceil(8) as usize];
        let mut second_storage = first_storage.clone();
        let mut batch_storage = vec![0u64; batch_req.size.div_ceil(8) as usize];
        let first = HostSpan {
            data: first_storage.as_mut_ptr().cast(),
            size: req.size,
        };
        let second = HostSpan {
            data: second_storage.as_mut_ptr().cast(),
            size: req.size,
        };
        let batch = HostSpan {
            data: batch_storage.as_mut_ptr().cast(),
            size: batch_req.size,
        };
        let binary_points = [binary.at(STAGE_TRANSFER)];
        for (phase, pattern) in [0x3157_9bdf, 0x2468_ace0, 0x3157_9bdf]
            .into_iter()
            .enumerate()
        {
            let producer = gpu::commands_begin(arena, &recording)?;
            gpu::fill_memory(producer, memory.span(0, 256), pattern);
            gpu::commands_end(producer)?;
            let consumer = gpu::commands_begin(arena, &recording)?;
            // Binary semaphore dependency supplies the inter-submit memory order.
            gpu::copy_memory(consumer, memory.span(256, 256), memory.span(0, 256));
            gpu::barrier(
                consumer,
                &dependency(
                    STAGE_TRANSFER,
                    STAGE_HOST,
                    ACCESS_COPY_WRITE,
                    ACCESS_HOST_READ,
                ),
            );
            gpu::commands_end(consumer)?;
            let point = |value, stages| SyncPoint {
                point: Point {
                    timeline: timeline.as_raw(),
                    value,
                },
                stages,
            };
            let before = [point(phase as u64 * 2, STAGE_TRANSFER)];
            let produced = [point(phase as u64 * 2 + 1, STAGE_ALL)];
            let wait_produced = [point(phase as u64 * 2 + 1, STAGE_TRANSFER)];
            let finished = [point(phase as u64 * 2 + 2, STAGE_ALL)];
            if phase != 1 {
                assert_eq!(
                    gpu::queue_submit_with_binary(
                        queue,
                        &[producer],
                        &before,
                        &produced,
                        &[],
                        &binary_points,
                        HostSpan {
                            data: ptr::null_mut(),
                            size: 0
                        }
                    ),
                    Err(CAPACITY)
                );
                assert_eq!(
                    gpu::queue_submit_with_binary(
                        queue,
                        &[producer],
                        &before,
                        &produced,
                        &[],
                        &[binary.at(STAGE_HOST)],
                        first
                    ),
                    Err(INVALID)
                );
                gpu::queue_submit_with_binary(
                    queue,
                    &[producer],
                    &before,
                    &produced,
                    &[],
                    &binary_points,
                    first,
                )?;
                gpu::queue_submit_with_binary(
                    queue,
                    &[consumer],
                    &wait_produced,
                    &finished,
                    &binary_points,
                    &[],
                    second,
                )?;
            } else {
                // One native call, two preserved batches. No timeline producer
                // signal/wait in this case: the binary carries the handoff.
                let signal_binary = BinarySubmitDesc {
                    header: Record::new::<BinarySubmitDesc>(BINARY_SUBMIT_DESC),
                    wait_count: 0,
                    signal_count: 1,
                    waits: ptr::null(),
                    signals: binary_points.as_ptr(),
                };
                let wait_binary = BinarySubmitDesc {
                    header: Record::new::<BinarySubmitDesc>(BINARY_SUBMIT_DESC),
                    wait_count: 1,
                    signal_count: 0,
                    waits: binary_points.as_ptr(),
                    signals: ptr::null(),
                };
                let bad_point = BinaryPoint {
                    binary: ptr::null_mut(),
                    stages: STAGE_TRANSFER,
                };
                let bad_binary = BinarySubmitDesc {
                    waits: &bad_point,
                    ..wait_binary
                };
                let producer = producer.as_raw();
                let consumer = consumer.as_raw();
                let mut header1 = Record::new::<SubmitDesc>(SUBMIT_DESC);
                header1.next = &signal_binary.header;
                let mut header2 = Record::new::<SubmitDesc>(SUBMIT_DESC);
                header2.next = &bad_binary.header;
                let mut descriptions = [
                    SubmitDesc {
                        header: header1,
                        list_count: 1,
                        wait_count: 0,
                        signal_count: 0,
                        lists: &producer,
                        waits: ptr::null(),
                        signals: ptr::null(),
                        scratch: first.data,
                        scratch_size: first.size,
                    },
                    SubmitDesc {
                        header: header2,
                        list_count: 1,
                        wait_count: 0,
                        signal_count: 1,
                        lists: &consumer,
                        waits: ptr::null(),
                        signals: finished.as_ptr(),
                        scratch: second.data,
                        scratch_size: second.size,
                    },
                ];
                assert_eq!(
                    gpu::queue_submit_batch(queue, &descriptions, batch),
                    Err(INVALID)
                );
                descriptions[1].header.next = &wait_binary.header;
                gpu::queue_submit_batch(queue, &descriptions, batch)?;
            }
            gpu::timeline_wait(timeline, phase as u64 * 2 + 2, 10_000_000_000)?;
            gpu::memory_invalidate(memory.span(0, 512))?;
            for word in 0..128 {
                assert_eq!(mapping.data.cast::<u32>().add(word).read(), pattern);
            }
            gpu::arena_reset(arena)?;
        }
        gpu::arena_destroy(arena);
        gpu::timeline_destroy(timeline);
        gpu::binary_destroy(binary);
        println!(
            "NATIVE_RUST_SYNC_PASS phases=3 batch_handoff=1 fence_lifecycle=2 checked_words=384"
        );
        Ok(())
    }
}
