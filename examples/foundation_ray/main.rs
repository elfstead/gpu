//! Explicit AS + inline query consumer. No emulation is enabled by this program.
use ogpu::foundation::{rust as gpu, types::*};
use std::ptr;
mod pipeline;

struct Scratch(Vec<u64>);
impl Scratch {
    fn new(r: HostRequirements) -> Self {
        assert!(r.alignment <= 8);
        Self(vec![0; r.size.div_ceil(8) as usize])
    }
    fn span(&mut self) -> HostSpan {
        HostSpan {
            data: self.0.as_mut_ptr().cast(),
            size: (self.0.len() * 8) as u64,
        }
    }
}
unsafe fn edge(
    device: gpu::DeviceHandle,
    list: gpu::ListHandle,
    before: u64,
    after: u64,
    src: u64,
    dst: u64,
) -> gpu::Result<()> {
    let scope = BarrierScope {
        stages_before: before,
        stages_after: after,
        access_before: src,
        access_after: dst,
    };
    let dep = Dependency {
        global_count: 1,
        global: &scope,
        ..Default::default()
    };
    let mut scratch = Scratch::new(unsafe { gpu::barrier_scratch_requirements(device, 1, 0, 0)? });
    unsafe {
        gpu::barrier(list, &dep, scratch.span());
    }
    Ok(())
}
unsafe fn finish(
    device: gpu::DeviceHandle,
    queue: gpu::QueueHandle,
    timeline: gpu::TimelineHandle,
    list: gpu::ListHandle,
    value: u64,
) -> gpu::Result<()> {
    let list = unsafe { gpu::commands_end(list)? };
    let raw = list.as_raw();
    let signal = SyncPoint {
        point: Point {
            timeline: timeline.as_raw(),
            value,
        },
        stages: STAGE_ALL,
    };
    let desc = SubmitDesc {
        list_count: 1,
        signal_count: 1,
        lists: &raw,
        signals: &signal,
        ..Default::default()
    };
    let mut scratch = Scratch::new(unsafe { gpu::submit_scratch_requirements(device, &[desc])? });
    let mut outcome = [SubmitOutcome {
        status: INVALID,
        acceptance: SUBMIT_REJECTED,
    }];
    unsafe {
        gpu::queue_submit(queue, &[desc], &mut outcome, scratch.span())?;
        gpu::timeline_wait(timeline, value, 30_000_000_000)?;
    }
    assert_eq!(outcome[0].acceptance, SUBMIT_ACCEPTED);
    Ok(())
}

#[allow(dead_code)]
pub fn run() -> gpu::Result<()> {
    run_profile(false)
}
#[allow(dead_code)]
pub fn run_profile(ray_pipeline: bool) -> gpu::Result<()> {
    run_geometry(ray_pipeline, false)
}
pub fn run_geometry(ray_pipeline: bool, procedural: bool) -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let Some(adapter) = gpu::adapters(&discovery).iter().find(|a| {
        gpu::adapter_features(a).baseline_supported != 0
            && gpu::adapter_features(a)
                .available
                .contains(&feature_ids::RAY_QUERY)
    }) else {
        println!("RAY_QUERY_UNSUPPORTED: no advertised native profile; no fallback");
        return Err(UNSUPPORTED);
    };
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
        .ok_or(UNSUPPORTED)?
        .domain;
    let mut requested = vec![feature_ids::ACCELERATION_STRUCTURE, feature_ids::RAY_QUERY];
    if ray_pipeline {
        requested.extend([
            feature_ids::RAY_PIPELINE,
            feature_ids::RAY_INDIRECT,
            feature_ids::RAY_MAINTENANCE,
            feature_ids::RAY_INDIRECT_TABLE,
        ]);
    }
    let generated = ray_pipeline
        && std::env::var_os("OGPU_RAY_GENERATED").is_some()
        && gpu::adapter_features(adapter)
            .available
            .contains(&feature_ids::GENERATED_COMMANDS);
    if generated {
        requested.push(feature_ids::GENERATED_COMMANDS);
    }
    let generated_work =
        generated && std::env::var("OGPU_RAY_GENERATED").ok().as_deref() != Some("feature-only");
    let device = gpu::device_create(
        adapter,
        &[QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }],
        &requested,
    )?;
    // SAFETY: one host thread, explicit barriers, waits before host access and teardown.
    // On uncertain submission/error, exit without destroying possibly pending owners.
    unsafe {
        let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
        let timeline = gpu::timeline_create(device, 0)?;
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
        let mut md = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: 1 << 20,
            alignment: 256,
            usage: USAGE_COPY_SRC
                | USAGE_COPY_DST
                | USAGE_STORAGE
                | ACCELERATION_INPUT_USAGE
                | ACCELERATION_STORAGE_USAGE
                | if ray_pipeline {
                    USAGE_RAY_TABLE | USAGE_INDIRECT
                } else {
                    0
                },
            memory_type: 0,
            kind: MEMORY_LINEAR,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let mut mr = Requirements::default();
        gpu::memory_requirements(device, &md, &mut mr)?;
        let mut compatible = vec![0; mr.compatible_type_count as usize];
        mr.compatible_types = compatible.as_mut_ptr();
        mr.compatible_type_capacity = compatible.len() as u32;
        gpu::memory_requirements(device, &md, &mut mr)?;
        md.memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| m.properties & MEMORY_HOST_VISIBLE != 0 && compatible.contains(&m.id))
            .ok_or(UNSUPPORTED)?
            .id;
        let memory = gpu::memory_create(device, &md)?;
        let base = gpu::memory_address(memory.span(0, md.size))?;
        let map = gpu::memory_map(memory.span(0, md.size))?;
        let host = map.data.cast::<u8>();
        host.write_bytes(0xcd, md.size as usize);
        let vertices = [
            0f32, 0., 0., 1., 0., 0., 0., 1., 0., 2., 0., 0., 3., 0., 0., 2., 1., 0.,
        ];
        ptr::copy_nonoverlapping(vertices.as_ptr(), host.cast::<f32>(), vertices.len());
        let triangle = AccelerationGeometry {
            kind: GEOMETRY_TRIANGLES,
            flags: GEOMETRY_OPAQUE,
            data: AccelerationGeometryData {
                triangles: AccelerationTriangles {
                    vertices: base,
                    stride: 12,
                    indices: 0,
                    transform: 0,
                    format: image_formats::RGB32_FLOAT,
                    max_vertex: 2,
                    index_type: 0,
                    reserved: 0,
                },
            },
        };
        let mut geometries = [
            triangle,
            AccelerationGeometry {
                data: AccelerationGeometryData {
                    triangles: AccelerationTriangles {
                        vertices: base + 36,
                        ..triangle.data.triangles
                    },
                },
                ..triangle
            },
        ];
        if procedural {
            let boxes = [0f32, 0., 0., 1., 1., 0.25, 2., 0., 0., 3., 1., 0.25];
            ptr::copy_nonoverlapping(boxes.as_ptr(), host.cast::<f32>(), boxes.len());
            geometries = [0, 24].map(|offset| AccelerationGeometry {
                kind: GEOMETRY_AABBS,
                flags: 0,
                data: AccelerationGeometryData {
                    aabbs: AccelerationAabbs {
                        data: base + offset,
                        stride: 24,
                    },
                },
            });
        }
        let maxima = [1u32, 1];
        let max = 1;
        let bottom = AccelerationBuildDesc {
            header: Record::new::<AccelerationBuildDesc>(ACCELERATION_BUILD_DESC),
            kind: ACCELERATION_BOTTOM,
            flags: BUILD_ALLOW_UPDATE | BUILD_ALLOW_COMPACTION,
            geometry_count: 2,
            reserved: 0,
            geometries: geometries.as_ptr(),
            max_primitives: maxima.as_ptr(),
        };
        let instance_geom = AccelerationGeometry {
            kind: GEOMETRY_INSTANCES,
            flags: 0,
            data: AccelerationGeometryData {
                instances: AccelerationInstances {
                    data: base + 256,
                    pointers: 0,
                    reserved: 0,
                },
            },
        };
        let top = AccelerationBuildDesc {
            header: Record::new::<AccelerationBuildDesc>(ACCELERATION_BUILD_DESC),
            kind: ACCELERATION_TOP,
            flags: 0,
            geometry_count: 1,
            reserved: 0,
            geometries: &instance_geom,
            max_primitives: &max,
        };
        let mut scratch = Scratch::new(gpu::acceleration_scratch_requirements(2, 4)?);
        let br = gpu::acceleration_requirements(device, &bottom, scratch.span())?;
        let tr = gpu::acceleration_requirements(device, &top, scratch.span())?;
        println!(
            "AS_REQUIREMENTS bottom={} top={}",
            br.storage_size, tr.storage_size
        );
        assert!(br.storage_size <= 65536 && tr.storage_size <= 65536);
        let scratch_size = br
            .build_scratch_size
            .max(br.update_scratch_size)
            .max(tr.build_scratch_size);
        println!("AS_SCRATCH={scratch_size}");
        assert!(scratch_size <= md.size - 262144);
        assert_eq!((base + 262144) % br.scratch_alignment, 0);
        let make = |kind, offset, size| {
            gpu::acceleration_create(
                device,
                &AccelerationDesc {
                    header: Record::new::<AccelerationDesc>(ACCELERATION_DESC),
                    kind,
                    reserved: 0,
                    backing: memory.span(offset, size),
                },
            )
        };
        let blas = make(ACCELERATION_BOTTOM, 65536, br.storage_size)?;
        let tlas = make(ACCELERATION_TOP, 131072, tr.storage_size)?;
        let clone = make(ACCELERATION_BOTTOM, 196608, br.storage_size)?;
        let batched = make(ACCELERATION_BOTTOM, 458752, br.storage_size)?;
        assert!(br.build_scratch_size <= md.size - 786432);
        let instance = AccelerationInstance {
            transform: [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.],
            custom_index_and_mask: 37 | (255 << 24),
            table_offset_and_flags: INSTANCE_DISABLE_CULL << 24,
            reference: gpu::acceleration_address(blas)?,
        };
        host.add(256).cast::<AccelerationInstance>().write(instance);
        gpu::memory_flush(memory.span(0, md.size))?;
        let range = AccelerationRange {
            primitive_count: 1,
            primitive_offset: 0,
            first_vertex: 0,
            transform_offset: u32::from(procedural),
        };
        let ranges = [range, range];
        let work = |description, destination, size| AccelerationBuild {
            description,
            source: ptr::null_mut(),
            destination,
            update: 0,
            indirect_stride: 0,
            scratch: DeviceRange {
                address: base + 262144,
                size,
            },
            ranges: ranges.as_ptr(),
            indirect: DeviceRange {
                address: 0,
                size: 0,
            },
        };
        let pools = [
            QUERY_ACCELERATION_COMPACTED_SIZE,
            QUERY_ACCELERATION_SERIALIZED_SIZE,
        ]
        .map(|kind| {
            gpu::query_pool_create(
                device,
                &QueryPoolDesc {
                    header: Record::new::<QueryPoolDesc>(QUERY_POOL_DESC),
                    kind,
                    count: 1,
                    statistics: 0,
                },
            )
            .unwrap()
        });
        let field = ArgumentField {
            offset: 0,
            size: 24,
            kind: ARGUMENT_INLINE,
            address_alignment: 0,
            stages: STAGE_COMPUTE,
            space: ARGUMENT_DIRECT,
            binding: ARGUMENT_DIRECT,
        };
        let layout = gpu::argument_layout_create(
            device,
            &ArgumentLayoutDesc {
                execution_class: ARGUMENT_COMPUTE,
                byte_size: 24,
                field_count: 1,
                flags: 0,
                fields: &field,
            },
        )?;
        let code = include_bytes!("../shaders/foundation-ray-query.comp.spv");
        let aligned: Vec<u32> = code
            .chunks_exact(4)
            .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
            .collect();
        let shader = Shader {
            stage: 4,
            format: SHADER_SPIRV,
            code: Bytes {
                data: aligned.as_ptr().cast(),
                size: code.len(),
            },
            entry: c"queryMain".as_ptr(),
            specialization: ptr::null(),
            subgroup: ptr::null(),
        };
        let features = [feature_ids::RAY_QUERY];
        let req = ShaderRequirements {
            header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
            features: Features {
                ids: features.as_ptr(),
                count: 1,
                reserved: 0,
            },
            local_size: [4, 1, 1],
            shared_memory: 0,
        };
        let executable = gpu::executable_create(
            device,
            &ExecutableDesc {
                header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                kind: EXECUTABLE_COMPUTE,
                shader_count: 1,
                shaders: &shader,
                static_state: ptr::null(),
                dynamic_state: 0,
                requirements: &req.header,
                cache: ptr::null_mut(),
                compile_flags: 0,
                reserved: 0,
            },
            layout,
        )?;
        let ray = if ray_pipeline {
            Some(pipeline::Prepared::new(
                device,
                memory,
                host,
                base,
                generated_work,
                md.memory_type,
                procedural,
            )?)
        } else {
            None
        };
        // Rejected recordings must not reach a native build or leave a reusable slot poisoned.
        for case in 0..5 {
            let list = gpu::commands_begin(arena, &recording)?;
            let mut bad = work(&bottom, blas.as_raw(), br.build_scratch_size);
            let too_many = [
                AccelerationRange {
                    primitive_count: 2,
                    ..range
                },
                range,
            ];
            let mut span = scratch.span();
            match case {
                0 => bad.scratch.address += 1,
                1 => bad.ranges = ptr::null(),
                2 => bad.ranges = too_many.as_ptr(),
                3 => bad.update = 1,
                _ => span.size = 0,
            }
            gpu::build_accelerations(list, &[bad], span);
            assert_eq!(
                gpu::commands_end(list),
                Err(if case == 4 { CAPACITY } else { INVALID })
            );
            gpu::arena_reset(arena)?;
        }
        let mut compact = None;
        let mut restored = None;
        for iteration in 0..6u64 {
            if iteration != 0 {
                gpu::arena_reset(arena)?;
            }
            // Update geometry from z=0 to z=2; the clone retains the old geometry.
            if iteration == 1 {
                if procedural {
                    for (i, z) in [(2, 2.), (5, 2.25), (8, 2.), (11, 2.25)] {
                        host.cast::<f32>().add(i).write(z);
                    }
                } else {
                    for i in [2, 5, 8, 11, 14, 17] {
                        host.cast::<f32>().add(i).write(2.);
                    }
                }
                gpu::memory_flush(memory.span(0, 256))?;
            }
            host.add(512).write_bytes(0xcd, 64);
            gpu::memory_flush(memory.span(512, 64))?;
            let list = gpu::commands_begin(arena, &recording)?;
            if iteration < 2 {
                let mut b = work(
                    &bottom,
                    blas.as_raw(),
                    if iteration == 0 {
                        br.build_scratch_size
                    } else {
                        br.update_scratch_size
                    },
                );
                if iteration == 1 {
                    b.source = blas.as_raw();
                    b.update = 1;
                }
                if iteration == 0 {
                    let mut second = work(&bottom, batched.as_raw(), br.build_scratch_size);
                    second.scratch.address = base + 786432;
                    gpu::build_accelerations(list, &[b, second], scratch.span());
                } else {
                    gpu::build_accelerations(list, &[b], scratch.span());
                }
                edge(
                    device,
                    list,
                    ACCELERATION_BUILD_STAGE,
                    ACCELERATION_BUILD_STAGE,
                    ACCELERATION_WRITE,
                    ACCELERATION_READ | ACCELERATION_WRITE,
                )?;
                if iteration == 0 {
                    gpu::copy_acceleration(list, clone, blas, ACCELERATION_CLONE);
                    edge(
                        device,
                        list,
                        ACCELERATION_BUILD_STAGE,
                        ACCELERATION_BUILD_STAGE,
                        ACCELERATION_WRITE,
                        ACCELERATION_READ,
                    )?;
                    for (i, pool) in pools.iter().enumerate() {
                        gpu::queries_reset(list, *pool, 0, 1);
                        gpu::acceleration_properties(list, &[blas], *pool, 0, scratch.span());
                        gpu::queries_resolve(
                            list,
                            *pool,
                            0,
                            1,
                            memory.span(1024 + i as u64 * 8, 8),
                            8,
                            5,
                        );
                    }
                }
            }
            if iteration == 3 {
                let size = host.add(1024).cast::<u64>().read();
                assert!(size > 0 && size <= 65536);
                let object = make(ACCELERATION_BOTTOM, 524288, size)?;
                gpu::copy_acceleration(list, object, clone, ACCELERATION_COMPACT);
                edge(
                    device,
                    list,
                    ACCELERATION_BUILD_STAGE,
                    ACCELERATION_BUILD_STAGE,
                    ACCELERATION_WRITE,
                    ACCELERATION_READ,
                )?;
                compact = Some(object);
            }
            if iteration == 4 {
                let size = host.add(1032).cast::<u64>().read();
                assert!(size <= 65536);
                let bytes = DeviceRange {
                    address: base + 655360,
                    size,
                };
                gpu::serialize_acceleration(list, bytes, clone);
                edge(
                    device,
                    list,
                    ACCELERATION_BUILD_STAGE,
                    ACCELERATION_BUILD_STAGE,
                    ACCESS_COPY_WRITE,
                    ACCESS_COPY_READ,
                )?;
                let object = make(ACCELERATION_BOTTOM, 589824, br.storage_size)?;
                gpu::deserialize_acceleration(list, object, bytes);
                edge(
                    device,
                    list,
                    ACCELERATION_BUILD_STAGE,
                    ACCELERATION_BUILD_STAGE,
                    ACCELERATION_WRITE,
                    ACCELERATION_READ,
                )?;
                restored = Some(object);
            }
            if iteration >= 2 {
                let reference = if iteration == 5 {
                    batched
                } else if iteration == 3 {
                    compact.unwrap()
                } else if iteration == 4 {
                    restored.unwrap()
                } else {
                    clone
                };
                host.add(256)
                    .cast::<AccelerationInstance>()
                    .write(AccelerationInstance {
                        reference: gpu::acceleration_address(reference)?,
                        ..instance
                    });
                gpu::memory_flush(memory.span(256, 64))?;
            }
            gpu::build_accelerations(
                list,
                &[work(&top, tlas.as_raw(), tr.build_scratch_size)],
                scratch.span(),
            );
            edge(
                device,
                list,
                ACCELERATION_BUILD_STAGE,
                if ray_pipeline {
                    STAGE_RAY
                } else {
                    STAGE_COMPUTE
                },
                ACCELERATION_WRITE,
                ACCELERATION_READ,
            )?;
            let arguments = [
                gpu::acceleration_address(tlas)?,
                base + 512,
                u64::from((if iteration == 1 { 2f32 } else { 0f32 }).to_bits())
                    | (u64::from(procedural) << 32),
            ];
            if let Some(ray) = &ray {
                ray.record(device, list, base, iteration, &arguments)?;
            } else {
                gpu::bind_executable(list, executable);
                gpu::set_arguments(
                    list,
                    layout,
                    0,
                    std::slice::from_raw_parts(arguments.as_ptr().cast(), 24),
                );
                gpu::dispatch(
                    list,
                    &Launch {
                        groups: Extent { x: 1, y: 1, z: 1 },
                        dynamic_shared_bytes: 0,
                        extensions: ptr::null(),
                    },
                );
            }
            edge(
                device,
                list,
                STAGE_ALL,
                STAGE_HOST,
                ACCESS_SHADER_WRITE | ACCESS_COPY_WRITE,
                ACCESS_HOST_READ,
            )?;
            finish(device, queue, timeline, list, iteration + 1)?;
            gpu::memory_invalidate(memory.span(0, md.size))?;
            for i in 0..4 {
                let p = host.add(512 + i * 16).cast::<u32>();
                let hit = i == 0 || i == 3;
                assert_eq!(p.read(), u32::from(hit));
                if hit {
                    assert!(
                        (f32::from_bits(p.add(1).read())
                            - (if iteration == 1 { 3. } else { 1. })
                            - if ray_pipeline { 0.125 } else { 0.0 })
                        .abs()
                            < 1e-5
                    );
                    assert_eq!(p.add(2).read(), 37);
                    assert_eq!(
                        p.add(3).read(),
                        (if i == 3 { 1 << 16 } else { 0 })
                            | (if procedural && ray_pipeline {
                                1 << 31
                            } else {
                                0
                            })
                    );
                } else {
                    assert_eq!(p.add(1).read(), (-1f32).to_bits());
                    assert_eq!(p.add(2).read(), u32::MAX);
                    assert_eq!(p.add(3).read(), u32::MAX);
                }
            }
            assert!(std::slice::from_raw_parts(host.add(576), 448)
                .iter()
                .all(|&b| b == 0xcd));
            println!("RAY_SCENE pipeline={ray_pipeline} procedural={procedural} iteration={iteration} hits=2 misses=2 instance=37 geometries=0,1 guards=ok");
        }
        let compacted = host.add(1024).cast::<u64>().read();
        let serialized = host.add(1032).cast::<u64>().read();
        assert!(compacted > 0 && compacted <= br.storage_size);
        assert!(serialized > 0);
        println!("AS_PROPERTIES compacted={compacted} serialized={serialized}");
        let version = std::slice::from_raw_parts(host.add(655360), 32);
        assert!(gpu::acceleration_compatibility(device, version)?);
        assert_eq!(
            gpu::acceleration_compatibility(device, &version[..31]),
            Err(INVALID)
        );
        println!(
            "AS_SERIALIZATION native-version-compatible=true cloned-compacted-restored-hits=ok"
        );
        for object in [compact.unwrap(), restored.unwrap()] {
            gpu::acceleration_destroy(object);
        }
        if let Some(ray) = ray {
            ray.destroy();
        }
        gpu::executable_destroy(executable);
        gpu::argument_layout_destroy(layout);
        for pool in pools {
            gpu::query_pool_destroy(pool);
        }
        for object in [tlas, clone, blas, batched] {
            gpu::acceleration_destroy(object);
        }
        gpu::arena_destroy(arena);
        gpu::timeline_destroy(timeline);
        gpu::memory_unmap(memory);
        gpu::memory_destroy(memory);
        gpu::device_destroy(device);
    }
    Ok(())
}
#[allow(dead_code)]
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|s| s != "--pipeline" && s != "--procedural")
    {
        eprintln!("usage: ray [--pipeline] [--procedural]");
        std::process::exit(2);
    }
    match run_geometry(
        args.iter().any(|s| s == "--pipeline"),
        args.iter().any(|s| s == "--procedural"),
    ) {
        Ok(()) => println!("RAY_CONSUMER_PASS"),
        Err(e) => {
            eprintln!("ray consumer status={e}");
            std::process::exit(if e == UNSUPPORTED { 77 } else { 1 });
        }
    }
}
