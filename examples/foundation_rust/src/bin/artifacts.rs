//! Native binary persistence and source-free recreation, with GPU equivalence checks.
use ogpu::foundation::{rust as gpu, types::*};
use std::ptr;
#[path = "../submission.rs"]
mod submission;
const CODE: &[u8] = include_bytes!("../../../shaders/foundation-shader-profiles.origin.comp.spv");
pub fn run() -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let available = gpu::adapter_features(adapter).available;
    if !available.contains(&feature_ids::PIPELINE_BINARIES) {
        println!("PIPELINE_BINARIES_UNAVAILABLE");
        return Err(UNSUPPORTED);
    }
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
    for disable in [false, true] {
        if disable && !available.contains(&feature_ids::DISABLE_BINARY_INTERNAL_CACHE) {
            println!("BINARY_CACHE_CONTROL_UNAVAILABLE");
            continue;
        }
        let mut features = vec![feature_ids::PIPELINE_BINARIES];
        if available.contains(&feature_ids::RASTER) {
            features.push(feature_ids::RASTER);
        }
        if disable {
            features.push(feature_ids::DISABLE_BINARY_INTERNAL_CACHE);
        }
        let device = gpu::device_create(adapter, &requests, &features)?;
        // Retain every object through explicit completion; failures do not destroy
        // uncertain pending resources. The executable does not retain its binaries.
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
            let address = gpu::memory_address(memory.span(256, 256))?;
            let code: Vec<u32> = CODE
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            let shader = Shader {
                stage: 4,
                format: SHADER_SPIRV,
                code: Bytes {
                    data: code.as_ptr().cast(),
                    size: CODE.len(),
                },
                entry: c"originMain".as_ptr(),
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
                    ids: ptr::null(),
                    count: 0,
                    reserved: 0,
                },
                local_size: [1, 1, 1],
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
                compile_flags: COMPILE_CAPTURE_BINARY,
                reserved: 0,
            };

            let (pipeline, restored) = recreate(device, arguments, &ed, disable)?;
            if available.contains(&feature_ids::RASTER) {
                graphics(device, disable)?;
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

            for (phase, (selected, seed)) in [
                (pipeline, 17u32),
                (restored, 17),
                (pipeline, 37),
                (restored, 37),
            ]
            .into_iter()
            .enumerate()
            {
                ptr::write_bytes(map.data.cast::<u8>(), 0xcd, 4096);
                gpu::memory_flush(memory.span(0, 4096))?;
                let mut root = [0u8; 16];
                root[..8].copy_from_slice(&address.to_ne_bytes());
                root[8..12].copy_from_slice(&seed.to_ne_bytes());
                let list = gpu::commands_begin(arena, &rd)?;
                gpu::bind_executable(list, selected);
                gpu::set_arguments(list, arguments, 0, &root);
                gpu::dispatch(
                    list,
                    &Launch {
                        groups: Extent { x: 8, y: 4, z: 2 },
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
                let values =
                    std::slice::from_raw_parts(map.data.cast::<u8>().add(256).cast::<u32>(), 64);
                for (i, v) in values.iter().enumerate() {
                    assert_eq!(
                        *v,
                        seed + (i % 8) as u32
                            + ((i / 8) % 4) as u32 * 256
                            + (i / 32) as u32 * 65536
                    );
                }
                let bytes = std::slice::from_raw_parts(map.data.cast::<u8>(), 4096);
                assert!(bytes[..256].iter().chain(&bytes[512..]).all(|b| *b == 0xcd));
                gpu::arena_reset(arena)?;
            }
            gpu::arena_destroy(arena);
            gpu::timeline_destroy(done);
            gpu::executable_destroy(restored);
            gpu::executable_destroy(pipeline);
            gpu::argument_layout_destroy(arguments);
            gpu::memory_unmap(memory);
            gpu::memory_destroy(memory);
            gpu::device_destroy(device);
            println!("PIPELINE_BINARY_PASS cache_disabled={disable} identity_rejection=pass source_free=pass numerical_equivalence=pass guards=pass");
        }
    }
    Ok(())
}
unsafe fn recreate(
    device: gpu::DeviceHandle,
    arguments: gpu::ArgumentLayoutHandle,
    ed: &ExecutableDesc,
    disable: bool,
) -> gpu::Result<(gpu::ExecutableHandle, gpu::ExecutableHandle)> {
    unsafe {
        let identity = gpu::pipeline_binary_identity(device)?;
        let source_key = gpu::executable_key(device, ed, arguments)?;
        let plain_desc = ExecutableDesc {
            compile_flags: 0,
            ..*ed
        };
        assert_eq!(
            gpu::executable_key(device, &plain_desc, arguments)?,
            source_key
        );
        let mut lookup_count = 0;
        let lookup = gpu::executable_binary_lookup(device, ed, arguments, None, &mut lookup_count);
        if disable {
            assert_eq!(lookup, Err(UNSUPPORTED));
        } else {
            match lookup {
                Ok(()) => {
                    let mut hits = vec![ptr::null_mut(); lookup_count as usize];
                    if !hits.is_empty() {
                        match gpu::executable_binary_lookup(
                            device,
                            ed,
                            arguments,
                            Some(&mut hits),
                            &mut lookup_count,
                        ) {
                            Ok(()) => {
                                for b in hits {
                                    gpu::pipeline_binary_destroy(
                                        gpu::PipelineBinaryHandle::from_raw(b).ok_or(INVALID)?,
                                    );
                                }
                            }
                            Err(BINARY_MISSING | CAPACITY) => {
                                assert!(hits.iter().all(|p| p.is_null()));
                                println!("BINARY_CACHE_LOOKUP=changed-after-query no-compilation");
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    println!("BINARY_CACHE_QUERY=hit latest_count={lookup_count}");
                }
                Err(BINARY_MISSING) => println!("BINARY_CACHE_LOOKUP=miss no-compilation"),
                Err(e) => return Err(e),
            }
        }
        let pipeline = gpu::executable_create(device, ed, arguments)?;
        let mut total = 0;
        gpu::executable_binary_capture(pipeline, None, &mut total)?;
        assert!(total > 0);
        if total > 1 && std::env::var_os("OGPU_BINARY_CAPACITY_DIAGNOSTIC").is_some() {
            let mut short = vec![ptr::dangling_mut(); total as usize - 1];
            let mut written = 0;
            assert_eq!(
                gpu::executable_binary_capture(pipeline, Some(&mut short), &mut written),
                Err(CAPACITY)
            );
            assert!(short.iter().all(|p| p.is_null()));
        }

        let mut captured = vec![ptr::null_mut(); total as usize];
        gpu::executable_binary_capture(pipeline, Some(&mut captured), &mut total)?;
        let mut imported = Vec::new();
        for raw in captured {
            let binary = gpu::PipelineBinaryHandle::from_raw(raw).ok_or(INVALID)?;
            let mut key = ArtifactKey::default();
            let mut size = 0;
            gpu::pipeline_binary_export(binary, &mut key, &mut size, None)?;
            assert!(size > 0);
            let expected_key = key;
            let mut short = vec![0x5au8; size - 1];
            let mut needed = short.len();
            assert_eq!(
                gpu::pipeline_binary_export(binary, &mut key, &mut needed, Some(&mut short)),
                Err(CAPACITY)
            );
            assert_eq!(needed, size);
            assert!(short.iter().all(|b| *b == 0x5a));
            let mut bytes = vec![0; size];
            gpu::pipeline_binary_export(binary, &mut key, &mut size, Some(&mut bytes))?;
            assert_eq!(key, expected_key);
            let mut wrong = identity;
            wrong.data[0] ^= 1;
            assert_eq!(
                gpu::pipeline_binary_import(device, &wrong, &key, &bytes),
                Err(UNSUPPORTED)
            );
            imported.push(gpu::pipeline_binary_import(
                device, &identity, &key, &bytes,
            )?);
            gpu::pipeline_binary_destroy(binary);
        }
        gpu::executable_capture_release(pipeline)?;
        assert_eq!(gpu::executable_capture_release(pipeline), Err(INVALID));
        assert_eq!(
            gpu::executable_binary_capture(pipeline, None, &mut total),
            Err(INVALID)
        );
        let native: Vec<_> = imported.iter().map(|h| h.as_raw()).collect();
        let binary_input = ExecutableBinaries {
            header: Record::new::<ExecutableBinaries>(EXECUTABLE_BINARIES),
            count: native.len() as u32,
            reserved: 0,
            binaries: native.as_ptr(),
        };
        let binary_shaders: Vec<_> =
            std::slice::from_raw_parts(ed.shaders, ed.shader_count as usize)
                .iter()
                .map(|s| Shader {
                    code: Bytes {
                        data: ptr::null(),
                        size: 0,
                    },
                    ..*s
                })
                .collect();
        let restored_desc = ExecutableDesc {
            header: Record {
                next: &binary_input.header,
                ..ed.header
            },
            shaders: binary_shaders.as_ptr(),
            compile_flags: 0,
            ..*ed
        };
        let restored = gpu::executable_create(device, &restored_desc, arguments)?;
        assert_eq!(
            gpu::executable_key(device, &restored_desc, arguments),
            Err(INVALID)
        );
        for b in imported {
            gpu::pipeline_binary_destroy(b);
        }
        Ok((pipeline, restored))
    }
}
#[allow(dead_code)]
fn main() {
    if let Err(e) = run() {
        eprintln!("PIPELINE_BINARY_FAILURE={e}");
        std::process::exit(if e == UNSUPPORTED { 77 } else { 1 });
    }
}
unsafe fn graphics(device: gpu::DeviceHandle, disable: bool) -> gpu::Result<()> {
    unsafe {
        let code: Vec<Vec<u32>> = [
            include_bytes!("../../../shaders/foundation-graphics.vert.spv").as_slice(),
            include_bytes!("../../../shaders/foundation-graphics.frag.spv").as_slice(),
        ]
        .into_iter()
        .map(|b| {
            b.chunks_exact(4)
                .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
                .collect()
        })
        .collect();
        let shaders: Vec<_> = [(8, c"vertexMain"), (16, c"fragmentMain")]
            .into_iter()
            .enumerate()
            .map(|(i, (stage, entry))| Shader {
                stage,
                format: SHADER_SPIRV,
                code: Bytes {
                    data: code[i].as_ptr().cast(),
                    size: code[i].len() * 4,
                },
                entry: entry.as_ptr(),
                specialization: ptr::null(),
                subgroup: ptr::null(),
            })
            .collect();
        let field = ArgumentField {
            offset: 0,
            size: 8,
            kind: ARGUMENT_INLINE,
            address_alignment: 0,
            stages: 24,
            space: ARGUMENT_DIRECT,
            binding: ARGUMENT_DIRECT,
        };
        let arguments = gpu::argument_layout_create(
            device,
            &ArgumentLayoutDesc {
                execution_class: ARGUMENT_GRAPHICS,
                byte_size: 8,
                field_count: 1,
                flags: 0,
                fields: &field,
            },
        )?;
        let color = ColorState {
            format: RGBA8_UNORM,
            write_mask: 15,
            blend: 0,
            src_color: 1,
            dst_color: 0,
            color_op: 0,
            src_alpha: 1,
            dst_alpha: 0,
            alpha_op: 0,
        };
        let state = GraphicsState {
            header: Record::new::<GraphicsState>(GRAPHICS_STATE),
            topology: 3,
            cull: 0,
            front_face: 0,
            samples: 1,
            color_count: 1,
            colors: &color,
            depth_format: 0,
            depth_test: 0,
            depth_write: 0,
            depth_compare: 0,
            blend_constants: [0.; 4],
            stencil_test: 0,
            stencil_front: StencilState::default(),
            stencil_back: StencilState::default(),
            vertex_input: ptr::null(),
            raster_options: ptr::null(),
            logic_op_enable: 0,
            logic_op: 0,
            view_mask: 0,
            shading_rate: ptr::null(),
        };
        let requirements = ShaderRequirements {
            header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
            features: Features::from_slice(&[feature_ids::RASTER]),
            local_size: [0; 3],
            shared_memory: 0,
        };
        let desc = ExecutableDesc {
            header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
            kind: 2,
            shader_count: 2,
            shaders: shaders.as_ptr(),
            static_state: &state.header,
            dynamic_state: 1,
            requirements: &requirements.header,
            cache: ptr::null_mut(),
            compile_flags: COMPILE_CAPTURE_BINARY,
            reserved: 0,
        };
        let (original, restored) = recreate(device, arguments, &desc, disable)?;
        gpu::executable_destroy(original);
        gpu::executable_destroy(restored);
        gpu::argument_layout_destroy(arguments);
        println!("GRAPHICS_BINARY_PREPARATION_PASS cache_disabled={disable} source_free=true raster_execution=not-tested");
        Ok(())
    }
}
