//! Caller-side shader-table packing. This policy deliberately lives outside OGPU.
use super::*;
pub struct Prepared {
    pub executable: gpu::ExecutableHandle,
    pub layout: gpu::ArgumentLayoutHandle,
    tables: RayTables,
    stack: u32,
    generated: Vec<(
        gpu::GeneratedLayoutHandle,
        GeneratedBatch,
        Option<gpu::MemoryHandle>,
    )>,
}
impl Prepared {
    pub unsafe fn new(
        device: gpu::DeviceHandle,
        memory: gpu::MemoryHandle,
        host: *mut u8,
        base: u64,
        enable_generated: bool,
        memory_type: u32,
    ) -> gpu::Result<Self> {
        unsafe {
            let mut generated = GeneratedLimits::default();
            let mut gq = Query {
                header: Record::new::<Query>(GENERATED_LIMITS),
                data: ptr::from_mut(&mut generated).cast(),
                capacity: 1,
                count: 0,
                element_size: size_of::<GeneratedLimits>() as u32,
                reserved: 0,
            };
            gpu::device_query(device, &mut gq)?;
            println!(
                "RAY_GENERATED_AVAILABLE stages={} binding={} tokens={}",
                generated.stages, generated.pipeline_binding_stages, generated.tokens
            );
            let mut limits = RayLimits::default();
            let mut query = Query {
                header: Record::new::<Query>(RAY_LIMITS),
                data: ptr::from_mut(&mut limits).cast(),
                capacity: 1,
                count: 0,
                element_size: size_of::<RayLimits>() as u32,
                reserved: 0,
            };
            gpu::device_query(device, &mut query)?;
            let field = ArgumentField {
                offset: 0,
                size: 16,
                kind: ARGUMENT_INLINE,
                address_alignment: 0,
                stages: ARGUMENT_RAYGEN,
                space: ARGUMENT_DIRECT,
                binding: ARGUMENT_DIRECT,
            };
            let layout = gpu::argument_layout_create(
                device,
                &ArgumentLayoutDesc {
                    execution_class: ARGUMENT_RAY,
                    byte_size: 16,
                    field_count: 1,
                    flags: 0,
                    fields: &field,
                },
            )?;
            let codes: &[&[u8]] = &[
                include_bytes!("../shaders/foundation-ray-pipeline.rgen.spv"),
                include_bytes!("../shaders/foundation-ray-pipeline.rmiss.spv"),
                include_bytes!("../shaders/foundation-ray-pipeline.rchit.spv"),
                include_bytes!("../shaders/foundation-ray-pipeline.rcall.spv"),
            ];
            let aligned: Vec<Vec<u32>> = codes
                .iter()
                .map(|c| {
                    c.chunks_exact(4)
                        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
                        .collect()
                })
                .collect();
            let shaders: Vec<_> = [
                (SHADER_RAYGEN, c"rayMain"),
                (SHADER_MISS, c"missMain"),
                (SHADER_CLOSEST_HIT, c"hitMain"),
                (SHADER_CALLABLE, c"initialize"),
            ]
            .iter()
            .enumerate()
            .map(|(i, (stage, entry))| Shader {
                stage: *stage,
                format: SHADER_SPIRV,
                code: Bytes {
                    data: aligned[i].as_ptr().cast(),
                    size: codes[i].len(),
                },
                entry: entry.as_ptr(),
                specialization: ptr::null(),
                subgroup: ptr::null(),
            })
            .collect();
            let none = RAY_UNUSED_SHADER;
            let mut groups = [0, 1, 2, 3].map(|i| RayGroup {
                kind: RAY_GROUP_GENERAL,
                general: i,
                closest_hit: none,
                any_hit: none,
                intersection: none,
            });
            groups[2] = RayGroup {
                kind: RAY_GROUP_TRIANGLES,
                general: none,
                closest_hit: 2,
                any_hit: none,
                intersection: none,
            };
            let state = RayState {
                header: Record::new::<RayState>(RAY_STATE),
                group_count: 4,
                max_recursion: 1,
                groups: groups.as_ptr(),
                flags: 0,
                reserved: 0,
            };
            let ids = [feature_ids::RAY_PIPELINE];
            let req = ShaderRequirements {
                header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
                features: Features {
                    ids: ids.as_ptr(),
                    count: 1,
                    reserved: 0,
                },
                local_size: [1, 1, 1],
                shared_memory: 0,
            };
            let executable = gpu::executable_create(
                device,
                &ExecutableDesc {
                    header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                    kind: EXECUTABLE_RAY,
                    shader_count: 4,
                    shaders: shaders.as_ptr(),
                    static_state: &state.header,
                    dynamic_state: DYNAMIC_RAY_STACK,
                    requirements: &req.header,
                    cache: ptr::null_mut(),
                    compile_flags: 0,
                    reserved: 0,
                },
                layout,
            )?;
            let handle = u64::from(limits.group_handle_size);
            let stride = (handle + 4).div_ceil(u64::from(limits.group_handle_alignment))
                * u64::from(limits.group_handle_alignment);
            let spacing = stride.div_ceil(u64::from(limits.table_alignment))
                * u64::from(limits.table_alignment);
            assert!(spacing * 4 <= 4096 && stride <= u64::from(limits.max_table_stride));
            assert_eq!((base + 4096) % u64::from(limits.table_alignment), 0);
            for i in 0..4u32 {
                gpu::ray_group_handles(
                    executable,
                    i,
                    1,
                    HostSpan {
                        data: host.add(4096 + (u64::from(i) * spacing) as usize).cast(),
                        size: handle,
                    },
                )?;
            }
            host.add(4096 + (2 * spacing + handle) as usize)
                .cast::<f32>()
                .write(0.125);
            let table = |i| RayTable {
                range: DeviceRange {
                    address: base + 4096 + i * spacing,
                    size: stride,
                },
                stride,
            };
            let tables = RayTables {
                raygen: table(0),
                miss: table(1),
                hit: RayTable {
                    stride: 0,
                    ..table(2)
                },
                callable: table(3),
            };
            let stack = gpu::ray_group_stack_size(executable, 0, 0)?
                + gpu::ray_group_stack_size(executable, 1, 0)?
                    .max(gpu::ray_group_stack_size(executable, 2, 1)?)
                + 2 * gpu::ray_group_stack_size(executable, 3, 0)?;
            assert_eq!(gpu::ray_group_stack_size(executable, 2, 0), Err(INVALID));
            assert_eq!(
                gpu::ray_group_handles(
                    executable,
                    0,
                    1,
                    HostSpan {
                        data: ptr::null_mut(),
                        size: 0
                    }
                ),
                Err(CAPACITY)
            );
            host.add(8192).cast::<[u32; 3]>().write([4, 1, 1]);
            host.add(8320).cast::<RayIndirect>().write(RayIndirect {
                raygen_address: tables.raygen.range.address,
                raygen_size: tables.raygen.range.size,
                miss_address: tables.miss.range.address,
                miss_size: tables.miss.range.size,
                miss_stride: tables.miss.stride,
                hit_address: tables.hit.range.address,
                hit_size: tables.hit.range.size,
                hit_stride: tables.hit.stride,
                callable_address: tables.callable.range.address,
                callable_size: tables.callable.range.size,
                callable_stride: tables.callable.stride,
                width: 4,
                height: 1,
                depth: 1,
            });
            gpu::memory_flush(memory.span(4096, 4352))?;
            println!("RAY_PIPELINE group_handle={handle} stride={stride} stack={stack} record_bias=0.125");
            let mut generated_batches = Vec::new();
            if enable_generated
                && generated.stages & STAGE_RAY != 0
                && generated.tokens & (1 << GENERATED_TRACE_RAYS) != 0
            {
                for explicit in 0..2u32 {
                    let token = GeneratedToken {
                        kind: GENERATED_TRACE_RAYS,
                        offset: 0,
                        target: 0,
                        size: 0,
                    };
                    let gl = gpu::generated_layout_create(
                        device,
                        &GeneratedLayoutDesc {
                            header: Record::new::<GeneratedLayoutDesc>(GENERATED_LAYOUT_DESC),
                            arguments: layout.as_raw(),
                            stages: STAGE_RAY,
                            flags: explicit,
                            stride: size_of::<RayIndirect>() as u32,
                            token_count: 1,
                            reserved: 0,
                            tokens: &token,
                        },
                    )?;
                    let work = GeneratedWork {
                        layout: gl.as_raw(),
                        set: ptr::null_mut(),
                        executable: executable.as_raw(),
                        max_sequences: 1,
                        max_draws: 0,
                    };
                    let req = gpu::generated_requirements(device, &work)?;
                    let (preprocess, owner) = if req.size == 0 {
                        (
                            Span {
                                memory: ptr::null_mut(),
                                offset: 0,
                                size: 0,
                            },
                            None,
                        )
                    } else {
                        let mut md = MemoryDesc {
                            header: Record::new::<MemoryDesc>(MEMORY_DESC),
                            size: req.size,
                            alignment: req.alignment,
                            usage: PREPROCESS_USAGE,
                            memory_type,
                            kind: MEMORY_LINEAR,
                            concurrent_domains: ptr::null(),
                            concurrent_domain_count: 0,
                            flags: 0,
                        };
                        let mut mr = Requirements::default();
                        gpu::memory_requirements(device, &md, &mut mr)?;
                        let mut types = vec![0; mr.compatible_type_count as usize];
                        mr.compatible_types = types.as_mut_ptr();
                        mr.compatible_type_capacity = types.len() as u32;
                        gpu::memory_requirements(device, &md, &mut mr)?;
                        md.memory_type = *types
                            .iter()
                            .find(|&&t| req.memory_type_bits & (1 << t) != 0)
                            .ok_or(UNSUPPORTED)?;
                        let m = gpu::memory_create(device, &md)?;
                        (m.span(0, req.size), Some(m))
                    };
                    generated_batches.push((
                        gl,
                        GeneratedBatch {
                            work,
                            arguments: memory.span(8320, size_of::<RayIndirect>() as u64),
                            preprocess,
                            count: Span {
                                memory: ptr::null_mut(),
                                offset: 0,
                                size: 0,
                            },
                        },
                        owner,
                    ));
                    println!(
                        "RAY_GENERATED explicit={explicit} scratch={} alignment={}",
                        req.size, req.alignment
                    );
                }
            }
            Ok(Self {
                generated: generated_batches,
                executable,
                layout,
                tables,
                stack: u32::try_from(stack).map_err(|_| UNSUPPORTED)?,
            })
        }
    }
    pub unsafe fn record(
        &self,
        device: gpu::DeviceHandle,
        list: gpu::ListHandle,
        scene: u64,
        base: u64,
        mode: u64,
    ) -> gpu::Result<()> {
        unsafe {
            gpu::bind_executable(list, self.executable);
            gpu::set_ray_stack_size(list, self.stack);
            let arguments = [scene, base + 512];
            gpu::set_arguments(
                list,
                self.layout,
                0,
                std::slice::from_raw_parts(arguments.as_ptr().cast(), 16),
            );
            if mode >= 3 && self.generated.len() == 2 {
                let explicit = (mode - 3) as u32;
                let batch = &self.generated[explicit as usize].1;
                if explicit != 0 {
                    gpu::generated_preprocess(list, batch, list);
                    edge(
                        device,
                        list,
                        PREPROCESS_STAGE,
                        STAGE_INDIRECT,
                        PREPROCESS_WRITE,
                        ACCESS_INDIRECT_READ,
                    )?;
                }
                gpu::generated_execute(list, batch, explicit);
                return Ok(());
            }
            match mode % 3 {
                0 => gpu::trace_rays(list, &self.tables, Extent { x: 4, y: 1, z: 1 }),
                1 => gpu::trace_rays_indirect(
                    list,
                    &self.tables,
                    DeviceRange {
                        address: base + 8192,
                        size: 12,
                    },
                ),
                _ => gpu::trace_rays_indirect_table(
                    list,
                    DeviceRange {
                        address: base + 8320,
                        size: size_of::<RayIndirect>() as u64,
                    },
                ),
            }
        }
        Ok(())
    }
    pub unsafe fn destroy(self) {
        unsafe {
            for (layout, _, memory) in self.generated {
                gpu::generated_layout_destroy(layout);
                if let Some(memory) = memory {
                    gpu::memory_destroy(memory);
                }
            }
            gpu::executable_destroy(self.executable);
            gpu::argument_layout_destroy(self.layout);
        }
    }
}
