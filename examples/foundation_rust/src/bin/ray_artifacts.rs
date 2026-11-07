//! Library linkage and capture/replay witness. Never enables driver emulation.
use ogpu::foundation::{rust as gpu, types::*};
use std::{mem, ptr};
#[path = "../submission.rs"]
mod submission;
const CODE: &[u8] = include_bytes!("../../../shaders/foundation-ray-native.rgen.spv");
unsafe fn query<T: Default>(device: gpu::DeviceHandle, kind: u32) -> gpu::Result<T> {
    let mut out = T::default();
    let mut q = Query {
        header: Record::new::<Query>(kind),
        data: ptr::from_mut(&mut out).cast(),
        capacity: 1,
        count: 0,
        element_size: mem::size_of::<T>() as u32,
        reserved: 0,
    };
    unsafe {
        gpu::device_query(device, &mut q)?;
    }
    Ok(out)
}
unsafe fn layout(device: gpu::DeviceHandle) -> gpu::Result<gpu::ArgumentLayoutHandle> {
    let field = ArgumentField {
        offset: 0,
        size: 8,
        kind: ARGUMENT_INLINE,
        address_alignment: 0,
        stages: ARGUMENT_RAYGEN,
        space: ARGUMENT_DIRECT,
        binding: ARGUMENT_DIRECT,
    };
    unsafe {
        gpu::argument_layout_create(
            device,
            &ArgumentLayoutDesc {
                execution_class: ARGUMENT_RAY,
                byte_size: 8,
                field_count: 1,
                flags: 0,
                fields: &field,
            },
        )
    }
}
#[allow(clippy::too_many_arguments)] // Keep independent native linkage inputs visible.
unsafe fn build(
    device: gpu::DeviceHandle,
    layout: gpu::ArgumentLayoutHandle,
    flags: u32,
    library: bool,
    shaders: &[Shader],
    groups: &[RayGroup],
    libraries: &[gpu::ExecutableHandle],
    replay: Option<&[u8]>,
) -> gpu::Result<gpu::ExecutableHandle> {
    let handles = Bytes {
        data: replay.map_or(ptr::null(), |p| p.as_ptr().cast()),
        size: replay.map_or(0, |p| p.len()),
    };
    let replay_info = RayReplayInfo {
        header: Record::new::<RayReplayInfo>(RAY_REPLAY_INFO),
        count: 1,
        reserved: 0,
        handles: &handles,
    };
    let libs: Vec<_> = libraries.iter().map(|l| l.as_raw()).collect();
    let info = RayLibrariesInfo {
        header: Record {
            next: if replay.is_some() {
                &replay_info.header
            } else {
                ptr::null()
            },
            ..Record::new::<RayLibrariesInfo>(RAY_LIBRARIES_INFO)
        },
        count: libs.len() as u32,
        max_payload_size: 0,
        max_hit_attribute_size: 0,
        reserved: 0,
        libraries: libs.as_ptr(),
    };
    let state = RayState {
        header: Record {
            next: &info.header,
            ..Record::new::<RayState>(RAY_STATE)
        },
        group_count: groups.len() as u32,
        max_recursion: 1,
        groups: groups.as_ptr(),
        flags,
        reserved: 0,
    };
    let requirements = ShaderRequirements {
        header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
        features: Features::from_slice(&[feature_ids::RAY_PIPELINE]),
        local_size: [1, 1, 1],
        shared_memory: 0,
    };
    unsafe {
        gpu::executable_create(
            device,
            &ExecutableDesc {
                header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                kind: EXECUTABLE_RAY,
                shader_count: shaders.len() as u32,
                shaders: shaders.as_ptr(),
                static_state: &state.header,
                dynamic_state: 0,
                requirements: &requirements.header,
                cache: ptr::null_mut(),
                compile_flags: if library { COMPILE_LIBRARY } else { 0 },
                reserved: 0,
            },
            layout,
        )
    }
}
unsafe fn group(e: gpu::ExecutableHandle, index: u32, size: u32) -> gpu::Result<Vec<u8>> {
    let mut bytes = vec![0; size as usize];
    unsafe {
        gpu::ray_group_handles(
            e,
            index,
            1,
            HostSpan {
                data: bytes.as_mut_ptr().cast(),
                size: size as u64,
            },
        )?;
    }
    Ok(bytes)
}
pub fn run() -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let required = [
        feature_ids::ACCELERATION_STRUCTURE,
        feature_ids::RAY_PIPELINE,
        feature_ids::RAY_LIBRARIES,
    ];
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| {
            gpu::adapter_features(a).baseline_supported != 0
                && required
                    .iter()
                    .all(|f| gpu::adapter_features(a).available.contains(f))
        })
        .ok_or(UNSUPPORTED)?;
    let available = gpu::adapter_features(adapter).available;
    let stable = available.contains(&feature_ids::RAY_LIBRARY_HANDLES);
    let capture = available.contains(&feature_ids::RAY_CAPTURE_REPLAY);
    // Deliberately exclude MIXED: replay must be the first pipeline creation on a
    // fresh device. This consumer proves that the base replay profile is useful.
    let mut features = required.to_vec();
    if stable {
        features.push(feature_ids::RAY_LIBRARY_HANDLES);
    }
    if capture {
        features.push(feature_ids::RAY_CAPTURE_REPLAY);
    }
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
        .ok_or(UNSUPPORTED)?
        .domain;
    let mut saved: Option<(Vec<u8>, Vec<u8>)> = None;
    for replay_phase in [false, true] {
        if replay_phase && saved.is_none() {
            println!("RAY_REPLAY_UNAVAILABLE stable={stable} capture={capture}");
            break;
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
            let limits: RayLimits = query(device, RAY_LIMITS)?;
            let artifact: RayArtifactLimits = query(device, RAY_ARTIFACT_LIMITS)?;
            let library_layout = layout(device)?;
            let nested_layout = layout(device)?;
            let final_layout = layout(device)?;
            let code: Vec<u32> = CODE
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let shader = Shader {
                stage: SHADER_RAYGEN,
                format: SHADER_SPIRV,
                code: Bytes {
                    data: code.as_ptr().cast(),
                    size: CODE.len(),
                },
                entry: c"rayMain".as_ptr(),
                specialization: ptr::null(),
                subgroup: ptr::null(),
            };
            let group0 = RayGroup {
                kind: RAY_GROUP_GENERAL,
                general: 0,
                closest_hit: RAY_UNUSED_SHADER,
                any_hit: RAY_UNUSED_SHADER,
                intersection: RAY_UNUSED_SHADER,
            };
            let flags = if capture { RAY_CAPTURE_HANDLES } else { 0 };
            if replay_phase {
                assert_eq!(
                    build(
                        device,
                        library_layout,
                        flags,
                        true,
                        std::slice::from_ref(&shader),
                        &[group0],
                        &[],
                        Some(&[0u8])
                    ),
                    Err(INVALID)
                );
            }
            let library = build(
                device,
                library_layout,
                flags,
                true,
                std::slice::from_ref(&shader),
                &[group0],
                &[],
                saved.as_ref().map(|p| p.0.as_slice()),
            )?;
            let nested = build(
                device,
                nested_layout,
                flags,
                true,
                &[],
                &[],
                &[library],
                None,
            )?;
            let linked = build(
                device,
                final_layout,
                flags,
                false,
                &[],
                &[],
                &[nested],
                None,
            )?;
            // Zero local shaders; local group explicitly references the second
            // imported shader. Imported groups follow the local group in order.
            let indexed = build(
                device,
                final_layout,
                flags,
                false,
                &[],
                &[RayGroup {
                    general: 1,
                    ..group0
                }],
                &[library, library],
                None,
            )?;
            let live = group(linked, 0, limits.group_handle_size)?;
            if stable {
                assert_eq!(group(library, 0, limits.group_handle_size)?, live);
                assert_eq!(group(nested, 0, limits.group_handle_size)?, live);
                assert_eq!(group(indexed, 1, limits.group_handle_size)?, live);
                assert_eq!(group(indexed, 2, limits.group_handle_size)?, live);
            } else {
                assert_eq!(
                    group(library, 0, limits.group_handle_size),
                    Err(UNSUPPORTED)
                );
            }
            if capture && stable {
                let mut opaque = vec![0; artifact.capture_handle_size as usize];
                gpu::ray_capture_handles(library, 0, 1, &mut opaque)?;
                assert_eq!(
                    gpu::ray_capture_handles(
                        library,
                        0,
                        1,
                        &mut opaque[..artifact.capture_handle_size as usize - 1]
                    ),
                    Err(CAPACITY)
                );
                if let Some((old_opaque, old_live)) = &saved {
                    assert_eq!(&opaque, old_opaque);
                    assert_eq!(&live, old_live);
                } else {
                    saved = Some((opaque, live));
                }
            }
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
            let rd = RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: ptr::null(),
            };
            let bad = gpu::commands_begin(arena, &rd)?;
            gpu::bind_executable(bad, library);
            assert_eq!(gpu::commands_end(bad), Err(INVALID));
            gpu::arena_reset(arena)?;
            // Caller retains every linked library and its argument layout until
            // all derived executables are retired and destroyed, as native linkage requires.
            if limits.table_alignment > 8192 {
                return Err(UNSUPPORTED);
            }
            let mut md = MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: 16384,
                alignment: u64::from(limits.table_alignment.max(8)),
                usage: USAGE_STORAGE | USAGE_RAY_TABLE,
                memory_type: 0,
                kind: MEMORY_LINEAR,
                concurrent_domains: ptr::null(),
                concurrent_domain_count: 0,
                flags: 0,
            };
            let mut req = Requirements::default();
            gpu::memory_requirements(device, &md, &mut req)?;
            let mut compatible = vec![0; req.compatible_type_count as usize];
            req.compatible_types = compatible.as_mut_ptr();
            req.compatible_type_capacity = compatible.len() as u32;
            gpu::memory_requirements(device, &md, &mut req)?;
            md.memory_type = gpu::adapter_memory_types(adapter)
                .iter()
                .find(|m| m.properties & MEMORY_HOST_VISIBLE != 0 && compatible.contains(&m.id))
                .ok_or(UNSUPPORTED)?
                .id;
            let memory = gpu::memory_create(device, &md)?;
            let all = memory.span(0, 16384);
            let map = gpu::memory_map(all)?;
            let address = gpu::memory_address(all)?;
            let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
            let done = gpu::timeline_create(device, 0)?;
            let sr = gpu::submit_scratch_requirements(
                device,
                &[SubmitDesc {
                    list_count: 1,
                    signal_count: 1,
                    ..Default::default()
                }],
            )?;
            assert!(sr.alignment <= 8);
            let mut scratch = vec![0u64; sr.size.div_ceil(8) as usize];
            let br = gpu::barrier_scratch_requirements(device, 1, 0, 0)?;
            assert!(br.alignment <= 8);
            let mut barriers = vec![0u64; br.size.div_ceil(8) as usize];
            let stride = u64::from(limits.group_handle_size)
                .div_ceil(u64::from(limits.group_handle_alignment))
                * u64::from(limits.group_handle_alignment);
            for (phase, (e, index)) in [(linked, 0), (indexed, 0), (indexed, 2)]
                .into_iter()
                .enumerate()
            {
                let handle = group(e, index, limits.group_handle_size)?;
                let output = 256 + phase * 128;
                let bytes = std::slice::from_raw_parts_mut(map.data.cast::<u8>(), 16384);
                bytes.fill(0xcd);
                bytes[8192..8192 + handle.len()].copy_from_slice(&handle);
                let before = bytes.to_vec();
                gpu::memory_flush(all)?;
                let list = gpu::commands_begin(arena, &rd)?;
                gpu::bind_executable(list, e);
                gpu::set_arguments(
                    list,
                    final_layout,
                    0,
                    &(address + output as u64).to_ne_bytes(),
                );
                let empty = RayTable {
                    range: DeviceRange {
                        address: 0,
                        size: 0,
                    },
                    stride: 0,
                };
                gpu::trace_rays(
                    list,
                    &RayTables {
                        raygen: RayTable {
                            range: DeviceRange {
                                address: address + 8192,
                                size: stride,
                            },
                            stride,
                        },
                        miss: empty,
                        hit: empty,
                        callable: empty,
                    },
                    Extent { x: 16, y: 1, z: 1 },
                );
                let scope = BarrierScope {
                    stages_before: STAGE_RAY,
                    stages_after: STAGE_HOST,
                    access_before: ACCESS_SHADER_WRITE,
                    access_after: ACCESS_HOST_READ,
                };
                gpu::barrier(
                    list,
                    &Dependency {
                        global_count: 1,
                        global: &scope,
                        ..Default::default()
                    },
                    HostSpan {
                        data: barriers.as_mut_ptr().cast(),
                        size: br.size,
                    },
                );
                let list = gpu::commands_end(list)?;
                let signal = SyncPoint {
                    point: Point {
                        timeline: done.as_raw(),
                        value: phase as u64 + 1,
                    },
                    stages: STAGE_ALL,
                };
                submission::one(
                    queue,
                    &[list],
                    &[],
                    &[signal],
                    &[],
                    &[],
                    HostSpan {
                        data: scratch.as_mut_ptr().cast(),
                        size: sr.size,
                    },
                )?;
                gpu::timeline_wait(done, phase as u64 + 1, 10_000_000_000)?;
                gpu::memory_invalidate(all)?;
                let bytes = std::slice::from_raw_parts(map.data.cast::<u8>(), 16384);
                for i in 0..16 {
                    assert_eq!(
                        u32::from_ne_bytes(
                            bytes[output + i * 4..output + i * 4 + 4]
                                .try_into()
                                .unwrap()
                        ),
                        0x12340000 + i as u32
                    );
                }
                for i in 0..16384 {
                    if !(output..output + 64).contains(&i) {
                        assert_eq!(bytes[i], before[i], "guard/table byte {i}");
                    }
                }
                gpu::arena_reset(arena)?;
            }
            gpu::timeline_destroy(done);
            gpu::arena_destroy(arena);
            gpu::memory_unmap(memory);
            gpu::memory_destroy(memory);
            gpu::executable_destroy(linked);
            gpu::executable_destroy(indexed);
            gpu::argument_layout_destroy(final_layout);
            gpu::executable_destroy(nested);
            gpu::executable_destroy(library);
            gpu::argument_layout_destroy(nested_layout);
            gpu::argument_layout_destroy(library_layout);
            gpu::device_destroy(device);
            println!("RAY_LIBRARY_PASS replay={replay_phase} nested=true imported_shader_indices=true stable={stable} library_lifetimes=pass outputs=exact guards=pass");
        }
    }
    Ok(())
}
#[allow(dead_code)]
fn main() {
    if let Err(e) = run() {
        eprintln!("RAY_LIBRARY_STATUS={e}");
        std::process::exit(if e == UNSUPPORTED { 77 } else { 1 });
    }
}
