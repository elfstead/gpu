//! Native Rust shader-profile consumer, with exact integer/derivative references.
use ogpu::foundation::{rust as gpu, types::*};
use std::{ffi::CStr, ptr};
#[path = "../submission.rs"]
mod submission;
struct Profile {
    name: &'static CStr,
    code: &'static [u8],
    features: &'static [u32],
    local: [u32; 3],
    words: usize,
}
const PROFILES: [Profile; 4] = [
    Profile {
        name: c"rotateMain",
        code: include_bytes!("../../../shaders/foundation-shader-profiles.rotate.comp.spv"),
        features: &[
            feature_ids::SUBGROUP_ROTATE,
            feature_ids::SUBGROUP_ROTATE_CLUSTERED,
        ],
        local: [64, 1, 1],
        words: 256,
    },
    Profile {
        name: c"derivativeLinear",
        code: include_bytes!("../../../shaders/foundation-shader-profiles.linear.comp.spv"),
        features: &[feature_ids::COMPUTE_DERIVATIVE_LINEAR],
        local: [64, 1, 1],
        words: 128,
    },
    Profile {
        name: c"derivativeQuad",
        code: include_bytes!("../../../shaders/foundation-shader-profiles.quad.comp.spv"),
        features: &[feature_ids::COMPUTE_DERIVATIVE_QUADS],
        local: [8, 8, 1],
        words: 128,
    },
    Profile {
        name: c"narrowPush",
        code: include_bytes!("../../../shaders/foundation-shader-profiles.narrow.comp.spv"),
        features: &[
            feature_ids::PUSH_STORAGE8,
            feature_ids::PUSH_STORAGE16,
            feature_ids::INT8,
            feature_ids::INT16,
        ],
        local: [64, 1, 1],
        words: 64,
    },
];
pub fn run() -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let available = gpu::adapter_features(adapter).available;
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
    let mut executed = 0;
    for (kind, p) in PROFILES.iter().enumerate() {
        if p.features.iter().any(|f| !available.contains(f)) {
            println!("SHADER_PROFILE_UNAVAILABLE={:?}", p.name);
            continue;
        }
        let device = gpu::device_create(adapter, &requests, p.features)?;
        // Caller owns handles and retains all GPU objects through timeline
        // retirement. Any uncertain failure exits without early destruction.
        unsafe {
            let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
            let mut md = MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: 4096,
                alignment: 8,
                usage: USAGE_STORAGE,
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
            let map = gpu::memory_map(memory.span(0, 4096))?;
            let address = gpu::memory_address(memory.span(256, p.words as u64 * 4))?;
            let code: Vec<u32> = p
                .code
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            let shader = Shader {
                stage: 4,
                format: SHADER_SPIRV,
                code: Bytes {
                    data: code.as_ptr().cast(),
                    size: p.code.len(),
                },
                entry: p.name.as_ptr(),
                specialization: ptr::null(),
                subgroup: ptr::null(),
            };
            let field = ArgumentField {
                offset: 0,
                size: 16,
                kind: ARGUMENT_INLINE,
                address_alignment: 0,
                stages: STAGE_COMPUTE,
                space: ARGUMENT_DIRECT,
                binding: ARGUMENT_DIRECT,
            };
            let ld = ArgumentLayoutDesc {
                execution_class: ARGUMENT_COMPUTE,
                byte_size: 16,
                field_count: 1,
                flags: 0,
                fields: &field,
            };
            let arguments = gpu::argument_layout_create(device, &ld)?;
            let requirements = ShaderRequirements {
                header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                features: Features {
                    ids: p.features.as_ptr(),
                    count: p.features.len() as u32,
                    reserved: 0,
                },
                local_size: p.local,
                shared_memory: 0,
            };
            let ed = ExecutableDesc {
                header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                kind: EXECUTABLE_COMPUTE,
                shader_count: 1,
                shaders: &shader,
                static_state: ptr::null(),
                dynamic_state: 0,
                requirements: &requirements.header,
                cache: ptr::null_mut(),
                compile_flags: 0,
                reserved: 0,
            };
            // The same declaration must fail on a device which did not enable it.
            let base_device = gpu::device_create(adapter, &requests, &[])?;
            let base_arguments = gpu::argument_layout_create(base_device, &ld)?;
            assert_eq!(
                gpu::executable_create(base_device, &ed, base_arguments),
                Err(UNSUPPORTED)
            );
            gpu::argument_layout_destroy(base_arguments);
            gpu::device_destroy(base_device);
            let pipeline = gpu::executable_create(device, &ed, arguments)?;
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
            for (phase, seed) in [17u32, 37].into_iter().enumerate() {
                ptr::write_bytes(map.data.cast::<u8>(), 0xcd, 4096);
                gpu::memory_flush(memory.span(0, 4096))?;
                let mut root = [0u8; 16];
                root[..8].copy_from_slice(&address.to_ne_bytes());
                root[8..12].copy_from_slice(&seed.to_ne_bytes());
                if kind == 3 {
                    root[10..12].copy_from_slice(&(0x1234u16 + phase as u16).to_ne_bytes());
                }
                let list = gpu::commands_begin(arena, &rd)?;
                gpu::bind_executable(list, pipeline);
                gpu::set_arguments(list, arguments, 0, &root);
                gpu::dispatch(
                    list,
                    &Launch {
                        groups: Extent { x: 1, y: 1, z: 1 },
                        dynamic_shared_bytes: 0,
                        extensions: ptr::null(),
                    },
                );
                let scope = BarrierScope {
                    stages_before: STAGE_COMPUTE,
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
                gpu::memory_invalidate(memory.span(0, 4096))?;
                let values = std::slice::from_raw_parts(
                    map.data.cast::<u8>().add(256).cast::<u32>(),
                    p.words,
                );
                for i in 0..64 {
                    match kind {
                        0 => {
                            let lane = values[i * 4 + 2];
                            let width = values[i * 4 + 3];
                            assert!((4..=64).contains(&width) && width.is_power_of_two());
                            assert!(lane < width);
                            assert_eq!(values[i * 4], (lane + 1) % width + seed);
                            assert_eq!(values[i * 4 + 1], (lane / 4) * 4 + (lane + 1) % 4 + seed);
                        }
                        1 | 2 => {
                            assert_eq!(f32::from_bits(values[i * 2]), 1.0);
                            assert_eq!(
                                f32::from_bits(values[i * 2 + 1]),
                                if kind == 1 { 2.0 } else { 8.0 }
                            );
                        }
                        _ => assert_eq!(values[i], seed + (0x1234 + phase as u32) * 256 + i as u32),
                    }
                }
                let bytes = std::slice::from_raw_parts(map.data.cast::<u8>(), 4096);
                assert!(bytes[..256]
                    .iter()
                    .chain(&bytes[256 + p.words * 4..])
                    .all(|b| *b == 0xcd));
                gpu::arena_reset(arena)?;
            }
            gpu::arena_destroy(arena);
            gpu::timeline_destroy(done);
            gpu::executable_destroy(pipeline);
            gpu::argument_layout_destroy(arguments);
            gpu::memory_unmap(memory);
            gpu::memory_destroy(memory);
            gpu::device_destroy(device);
            executed += 1;
            println!("SHADER_PROFILE_PASS={:?} invocations=128 guards=pass disabled-requirements=rejected",p.name);
        }
    }
    if executed == 0 {
        return Err(UNSUPPORTED);
    }
    println!(
        "SHADER_PROFILES_PASS executed={executed} unavailable={}",
        PROFILES.len() - executed
    );
    Ok(())
}
#[allow(dead_code)]
fn main() {
    if let Err(e) = run() {
        eprintln!("SHADER_PROFILES_FAILURE={e}");
        std::process::exit(if e == UNSUPPORTED { 77 } else { 1 });
    }
}
