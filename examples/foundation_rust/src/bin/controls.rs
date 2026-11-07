//! Public Rust consumer: GPU predicates, inherited dispatch and native primitive counts.
use ogpu::foundation::{rust as gpu, types::*};
use std::ptr;
#[path = "../submission.rs"]
mod submission;
fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
        .collect()
}
unsafe fn barrier(list: gpu::ListHandle, before: u64, after: u64, source: u64, target: u64) {
    let scope = BarrierScope {
        stages_before: before,
        stages_after: after,
        access_before: source,
        access_after: target,
    };
    let mut scratch = [0u64; 64];
    unsafe {
        gpu::barrier(
            list,
            &Dependency {
                global_count: 1,
                global: &scope,
                ..Default::default()
            },
            HostSpan {
                data: scratch.as_mut_ptr().cast(),
                size: 512,
            },
        );
    }
}
unsafe fn dispatch(
    list: gpu::ListHandle,
    executable: gpu::ExecutableHandle,
    arguments: gpu::ArgumentLayoutHandle,
    address: u64,
    index: u64,
) {
    let control = [address, address + 16 + index * 16, 3];
    unsafe {
        gpu::bind_executable(list, executable);
        gpu::set_arguments(
            list,
            arguments,
            0,
            std::slice::from_raw_parts(control.as_ptr().cast(), 24),
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
}
pub fn run() -> gpu::Result<()> {
    let discovery = gpu::discover()?;
    let adapter = gpu::adapters(&discovery)
        .iter()
        .find(|a| gpu::adapter_features(a).baseline_supported != 0)
        .ok_or(UNSUPPORTED)?;
    let available = gpu::adapter_features(adapter).available;
    let mut features = vec![
        feature_ids::RASTER,
        feature_ids::CONDITIONAL_RENDERING,
        feature_ids::PRIMITIVES_GENERATED_QUERY,
        feature_ids::PRIMITIVES_GENERATED_DISCARD,
    ];
    if features.iter().any(|f| !available.contains(f)) {
        return Err(UNSUPPORTED);
    }
    let inherited = available.contains(&feature_ids::INHERITED_CONDITIONAL_RENDERING);
    if inherited {
        features.push(feature_ids::INHERITED_CONDITIONAL_RENDERING);
    }
    let domain = gpu::adapter_queues(adapter)
        .iter()
        .find(|q| q.count > 0 && q.flags & (GRAPHICS | COMPUTE) == GRAPHICS | COMPUTE)
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
    println!("CONTROL_FEATURES={features:?}; transform feedback deliberately not enabled");
    // This process owns all handles and keeps resources, queues, lists and parents
    // live through explicit retirement. No cleanup on uncertain native failure.
    unsafe {
        let queue = gpu::device_queue(device, domain, 0).ok_or(INVALID)?;
        let mut md = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size: 4096,
            alignment: 8,
            usage: USAGE_STORAGE | USAGE_COPY_DST | CONDITIONAL_USAGE,
            memory_type: 0,
            kind: MEMORY_LINEAR,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let mut req = Requirements::default();
        gpu::memory_requirements(device, &md, &mut req)?;
        let mut compatible = vec![0u32; req.compatible_type_count as usize];
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
        let base = gpu::memory_address(memory.span(0, 4096))?;
        let data = map.data.cast::<u8>();
        ptr::write_bytes(data, 0xcd, 4096);
        data.cast::<u64>().write(base + 256);
        data.add(8).cast::<u32>().write(1);
        data.add(12).cast::<u32>().write(5);
        for i in 0..8usize {
            data.add(16 + i * 16)
                .cast::<u64>()
                .write(base + 512 + i as u64 * 4);
            data.add(24 + i * 16).cast::<u32>().write(1);
            data.add(28 + i * 16).cast::<u32>().write(7);
        }
        data.add(256).cast::<u32>().write(11);
        // Graphics root at 320: color, depth, right edge, height, padding.
        for (i, v) in [1.0f32, 0.0, 0.0, 1.0, 0.5, 1.0].iter().enumerate() {
            data.add(320 + i * 4).cast::<f32>().write(*v);
        }
        data.add(344).cast::<u32>().write(1);
        data.add(348).cast::<u32>().write(0);
        gpu::memory_flush(memory.span(0, 4096))?;
        let field = ArgumentField {
            offset: 0,
            size: 24,
            kind: ARGUMENT_INLINE,
            address_alignment: 0,
            stages: STAGE_COMPUTE,
            space: ARGUMENT_DIRECT,
            binding: ARGUMENT_DIRECT,
        };
        let compute_arguments = gpu::argument_layout_create(
            device,
            &ArgumentLayoutDesc {
                execution_class: ARGUMENT_COMPUTE,
                byte_size: 24,
                field_count: 1,
                flags: 0,
                fields: &field,
            },
        )?;
        let field = ArgumentField {
            size: 8,
            stages: 8,
            ..field
        };
        let graphics_arguments = gpu::argument_layout_create(
            device,
            &ArgumentLayoutDesc {
                execution_class: ARGUMENT_GRAPHICS,
                byte_size: 8,
                field_count: 1,
                flags: 0,
                fields: &field,
            },
        )?;
        let code = words(include_bytes!(
            "../../../shaders/foundation-compute.comp.spv"
        ));
        let shader = Shader {
            stage: 4,
            format: SHADER_SPIRV,
            code: Bytes {
                data: code.as_ptr().cast(),
                size: code.len() * 4,
            },
            entry: c"transform".as_ptr(),
            specialization: ptr::null(),
            subgroup: ptr::null(),
        };
        let requirements = ShaderRequirements {
            header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
            features: Features::default(),
            local_size: [64, 1, 1],
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
        let compute = gpu::executable_create(device, &ed, compute_arguments)?;
        let vertex = words(include_bytes!(
            "../../../shaders/foundation-graphics.vert.spv"
        ));
        let shader = Shader {
            stage: 8,
            code: Bytes {
                data: vertex.as_ptr().cast(),
                size: vertex.len() * 4,
            },
            entry: c"vertexMain".as_ptr(),
            ..shader
        };
        let mut raster: RasterOptions = std::mem::zeroed();
        raster.header = Record::new::<RasterOptions>(RASTER_OPTIONS);
        raster.discard = 1;
        raster.line_width = 1.0;
        let mut graphics: GraphicsState = std::mem::zeroed();
        graphics.header = Record::new::<GraphicsState>(GRAPHICS_STATE);
        graphics.topology = 3;
        graphics.samples = 1;
        graphics.raster_options = &raster.header;
        let requirements = ShaderRequirements {
            local_size: [0; 3],
            ..requirements
        };
        let ed = ExecutableDesc {
            kind: 2,
            shaders: &shader,
            static_state: &graphics.header,
            dynamic_state: 1,
            requirements: &requirements.header,
            ..ed
        };
        let graphics = gpu::executable_create(device, &ed, graphics_arguments)?;
        let query = gpu::query_pool_create(
            device,
            &QueryPoolDesc {
                header: Record::new::<QueryPoolDesc>(QUERY_POOL_DESC),
                kind: QUERY_PRIMITIVES_GENERATED,
                count: 8,
                statistics: 0,
            },
        )?;
        let ad = ArenaDesc {
            header: Record::new::<ArenaDesc>(ARENA_DESC),
            domain,
            list_capacity: 8,
            level: 0,
            reserved: 0,
        };
        let arena = gpu::arena_create(device, &ad)?;
        let children = gpu::arena_create(device, &ArenaDesc { level: 1, ..ad })?;
        let rd = RecordingDesc {
            header: Record::new::<RecordingDesc>(RECORDING_DESC),
            replay_mode: 2,
            level: 0,
            inheritance: ptr::null(),
        };
        let conditional = ConditionalInheritance {
            header: Record::new::<ConditionalInheritance>(CONDITIONAL_INHERITANCE),
            enable: 1,
            reserved: 0,
        };
        let inheritance = Inheritance {
            header: Record {
                next: &conditional.header,
                ..Record::new::<Inheritance>(INHERITANCE)
            },
            occlusion: 0,
            query_flags: 0,
            statistics: 0,
            rendering: ptr::null(),
            resource_heap: ptr::null_mut(),
            sampler_heap: ptr::null_mut(),
        };
        let child_rd = RecordingDesc {
            level: 1,
            inheritance: &inheritance.header,
            ..rd
        };
        let cases = if inherited { 8usize } else { 4 };
        let mut child_lists = Vec::new();
        for i in 4..cases {
            let list = gpu::commands_begin(children, &child_rd)?;
            dispatch(list, compute, compute_arguments, base, i as u64);
            child_lists.push(gpu::commands_end(list)?);
        }
        let mut render: RenderDesc = std::mem::zeroed();
        render.header = Record::new::<RenderDesc>(RENDER_DESC);
        render.width = 1;
        render.height = 1;
        render.layers = 1;
        render.samples = 1;
        let viewport = ViewportState {
            header: Record::new::<ViewportState>(VIEWPORT_STATE),
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            min_depth: 0.0,
            max_depth: 1.0,
            scissor_x: 0,
            scissor_y: 0,
            scissor_width: 1,
            scissor_height: 1,
        };
        let list = gpu::commands_begin(arena, &rd)?;
        gpu::queries_reset(list, query, 0, cases as u32);
        let mut execute_scratch = [0u64; 16];
        for i in 0..cases {
            let predicate = memory.span(160 + i as u64 * 4, 4);
            gpu::fill_memory(list, predicate, (i % 2) as u32);
            barrier(
                list,
                STAGE_TRANSFER,
                CONDITIONAL_STAGE,
                ACCESS_COPY_WRITE,
                CONDITIONAL_READ,
            );
            gpu::conditional_begin(
                list,
                predicate,
                if i % 4 >= 2 { CONDITIONAL_INVERTED } else { 0 },
            );
            gpu::query_begin(list, query, i as u32, 0, 0);
            gpu::render_begin(list, &render);
            gpu::bind_executable(list, graphics);
            gpu::set_graphics_state(list, &viewport);
            gpu::set_arguments(list, graphics_arguments, 0, &(base + 320).to_ne_bytes());
            gpu::draw(
                list,
                &DrawDesc {
                    count: 3,
                    instances: 1,
                    first: 0,
                    first_instance: 0,
                    vertex_offset: 0,
                },
            );
            gpu::render_end(list);
            gpu::query_end(list, query, i as u32, 0);
            if i < 4 {
                dispatch(list, compute, compute_arguments, base, i as u64);
            } else {
                gpu::execute_lists(
                    list,
                    &[child_lists[i - 4]],
                    HostSpan {
                        data: execute_scratch.as_mut_ptr().cast(),
                        size: 128,
                    },
                );
            }
            // Native fill is unconditional even when the draw/dispatch is discarded.
            gpu::fill_memory(list, memory.span(224 + i as u64 * 4, 4), 0x12345678);
            gpu::conditional_end(list);
        }
        gpu::queries_resolve(
            list,
            query,
            0,
            cases as u32,
            memory.span(1024, cases as u64 * 8),
            8,
            5,
        );
        barrier(
            list,
            STAGE_ALL,
            STAGE_HOST,
            ACCESS_COPY_WRITE | ACCESS_SHADER_WRITE,
            ACCESS_HOST_READ,
        );
        let list = gpu::commands_end(list)?;
        let timeline = gpu::timeline_create(device, 0)?;
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
        let signal = SyncPoint {
            point: Point {
                timeline: timeline.as_raw(),
                value: 1,
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
        gpu::timeline_wait(timeline, 1, 10_000_000_000)?;
        gpu::memory_invalidate(memory.span(0, 4096))?;
        for i in 0..cases {
            let enabled = (i % 2 != 0) ^ (i % 4 >= 2);
            assert_eq!(
                data.add(512 + i * 4).cast::<u32>().read(),
                if enabled { 46 } else { 0xcdcdcdcd },
                "dispatch case {i}"
            );
            assert_eq!(
                data.add(1024 + i * 8).cast::<u64>().read(),
                u64::from(enabled),
                "primitive case {i}"
            );
            assert_eq!(data.add(224 + i * 4).cast::<u32>().read(), 0x12345678);
        }
        for i in 0..4096 {
            let used = i < 144
                || (160..160 + cases * 4).contains(&i)
                || (224..224 + cases * 4).contains(&i)
                || (256..260).contains(&i)
                || (320..352).contains(&i)
                || (512..512 + cases * 4).contains(&i)
                || (1024..1024 + cases * 8).contains(&i);
            if !used {
                assert_eq!(data.add(i).read(), 0xcd, "guard {i}");
            }
        }
        gpu::arena_reset(arena)?;
        // Keep malformed scopes in distinct native slots. VVL 1.4.357 leaves its
        // conditional-active flag set on reset of an abandoned scope. Opt into
        // same-slot reuse to reproduce that diagnostic; no library workaround.
        let reset_diagnostic =
            std::env::var("OGPU_CONDITIONAL_RESET_DIAGNOSTIC").as_deref() == Ok("1");
        println!("CONTROL_RESET_DIAGNOSTIC={reset_diagnostic}");
        for bad in 0..5 {
            let list = gpu::commands_begin(arena, &rd)?;
            match bad {
                0 => gpu::conditional_end(list),
                1 => gpu::conditional_begin(list, memory.span(161, 4), 0),
                2 => gpu::conditional_begin(list, memory.span(160, 3), 0),
                3 => {
                    gpu::conditional_begin(list, memory.span(160, 4), 0);
                    gpu::conditional_begin(list, memory.span(160, 4), 0);
                }
                _ => gpu::conditional_begin(list, memory.span(160, 4), 0),
            }
            assert_eq!(gpu::commands_end(list), Err(INVALID));
            if reset_diagnostic {
                gpu::arena_reset(arena)?;
            }
        }
        gpu::arena_destroy(arena);
        gpu::arena_destroy(children);
        gpu::query_pool_destroy(query);
        gpu::executable_destroy(compute);
        gpu::executable_destroy(graphics);
        gpu::argument_layout_destroy(compute_arguments);
        gpu::argument_layout_destroy(graphics_arguments);
        gpu::memory_unmap(memory);
        gpu::memory_destroy(memory);
        gpu::timeline_destroy(timeline);
        gpu::device_destroy(device);
        println!("CONTROL_ACCEPTANCE cases={cases} primary=4 inherited={} primitive_counts={cases} unconditional_fills={cases} rejected=5 guards=pass",cases-4);
    }
    Ok(())
}
#[allow(dead_code)]
fn main() {
    if let Err(e) = run() {
        eprintln!("CONTROL_FAILURE={e}");
        std::process::exit(if e == UNSUPPORTED { 77 } else { 1 });
    }
}
