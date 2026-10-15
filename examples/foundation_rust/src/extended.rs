//! Public native Rust compute/image checks; no C entry points or native GPU API.
use super::{dependency, gpu};
use ogpu::foundation::types::*;
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    ptr::{self, NonNull},
};

pub struct Context {
    pub device: gpu::DeviceHandle,
    pub queue: gpu::QueueHandle,
    pub arena: gpu::ArenaHandle,
    pub timeline: gpu::TimelineHandle,
    pub memory: gpu::MemoryHandle,
    pub mapping: Mapping,
}

// Consumer-owned CPU storage only; dropping it never waits or destroys GPU objects.
struct Storage {
    pointer: NonNull<u8>,
    layout: Layout,
}
impl Storage {
    fn new(requirements: HostRequirements) -> gpu::Result<Self> {
        let size = usize::try_from(requirements.size)
            .map_err(|_| CAPACITY)?
            .max(1);
        let alignment = usize::try_from(requirements.alignment).map_err(|_| INVALID)?;
        let layout = Layout::from_size_align(size, alignment).map_err(|_| INVALID)?;
        let pointer = NonNull::new(unsafe { alloc_zeroed(layout) }).ok_or(OUT_OF_MEMORY)?;
        Ok(Self { pointer, layout })
    }
    fn span(&mut self) -> HostSpan {
        HostSpan {
            data: self.pointer.as_ptr().cast(),
            size: self.layout.size() as u64,
        }
    }
}
impl Drop for Storage {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.pointer.as_ptr(), self.layout);
        }
    }
}

fn recording() -> RecordingDesc {
    RecordingDesc {
        header: Record::new::<RecordingDesc>(RECORDING_DESC),
        replay_mode: 1,
        level: 0,
        inheritance: ptr::null(),
    }
}
unsafe fn submit(
    c: &Context,
    list: gpu::ListHandle,
    value: u64,
    storage: &mut Storage,
) -> gpu::Result<()> {
    let signal = SyncPoint {
        point: Point {
            timeline: c.timeline.as_raw(),
            value,
        },
        stages: STAGE_ALL,
    };
    unsafe {
        gpu::queue_submit(c.queue, &[list], &[], &[signal], storage.span())?;
        gpu::timeline_wait(c.timeline, value, 10_000_000_000)?;
        gpu::memory_invalidate(c.memory.span(0, 4096))?;
        gpu::arena_reset(c.arena)?;
    }
    Ok(())
}

#[repr(C)]
struct Root {
    address: u64,
    count: u32,
    add: u32,
}
const _: () = assert!(size_of::<Root>() == 16 && std::mem::offset_of!(Root, count) == 8);

/// # Safety
/// Context objects are live, exclusive, idle, and have 4096 bytes of mapped backing
/// with copy/storage/indirect usage. Timeline starts completed at 3. Caller retains
/// everything on error until completion is known (the executable exits instead).
pub unsafe fn run(c: Context) -> gpu::Result<()> {
    let mut submit_storage = Storage::new(gpu::submit_scratch_requirements(1, 0, 1)?)?;
    unsafe {
        compute(&c, &mut submit_storage)?;
        images(&c, &mut submit_storage)?;
    }
    println!("NATIVE_RUST_EXTENDED_PASS compute_phases=3 image_phases=3 cache_roundtrip=1 descriptor_batches=2");
    Ok(())
}

unsafe fn compute(c: &Context, storage: &mut Storage) -> gpu::Result<()> {
    // Reuse the existing trusted artifact; copying into words makes alignment explicit.
    let bytes = include_bytes!("../../shaders/foundation-compute.comp.spv");
    let code: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let cache_desc = ExecutableCacheDesc {
        header: Record::new::<ExecutableCacheDesc>(EXECUTABLE_CACHE_DESC),
        synchronization: 0,
        reserved: 0,
        initial: Bytes {
            data: ptr::null(),
            size: 0,
        },
    };
    unsafe {
        let cache = gpu::executable_cache_create(c.device, &cache_desc)?;
        let slots = [
            RootSlot {
                stages: STAGE_COMPUTE,
                offset: 0,
                alignment: 8,
            },
            RootSlot {
                stages: STAGE_COMPUTE,
                offset: 8,
                alignment: 8,
            },
        ];
        let abi = ArgumentInterface {
            header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
            byte_size: 24,
            root_count: 2,
            roots: slots.as_ptr(),
        };
        let mut requirements = ShaderRequirements {
            header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
            features: 0,
            local_size: [64, 1, 1],
            shared_memory: 0,
        };
        let extra = 13u32;
        let entry = SpecializationEntry {
            id: 7,
            offset: 0,
            size: 4,
        };
        let spec = Specialization {
            header: Record::new::<Specialization>(SPECIALIZATION),
            count: 1,
            reserved: 0,
            entries: &entry,
            data: Bytes {
                data: ptr::from_ref(&extra).cast(),
                size: 4,
            },
        };
        let shader = Shader {
            stage: STAGE_COMPUTE as u32,
            format: SHADER_SPIRV,
            code: Bytes {
                data: code.as_ptr().cast(),
                size: code.len() * 4,
            },
            entry: c"transform".as_ptr(),
            interface_metadata: &abi.header,
            specialization: &spec.header,
            subgroup: ptr::null(),
        };
        let mut desc = ExecutableDesc {
            header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
            kind: EXECUTABLE_COMPUTE,
            shader_count: 1,
            shaders: &shader,
            static_state: ptr::null(),
            dynamic_state: 0,
            requirements: &requirements.header,
            cache: cache.as_raw(),
            compile_flags: 0,
            reserved: 0,
        };
        requirements.local_size[0] = 0;
        desc.requirements = &requirements.header;
        assert_eq!(gpu::executable_create(c.device, &desc), Err(INVALID));
        requirements.local_size[0] = 64;
        desc.requirements = &requirements.header;
        let executable = gpu::executable_create(c.device, &desc)?;
        drop(code); // Prepared executable does not borrow artifact storage.
        let mut size = 0;
        gpu::executable_cache_data(cache, &mut size, None)?;
        assert!(size > 0);
        let mut data = vec![0u8; size];
        gpu::executable_cache_data(cache, &mut size, Some(&mut data))?;
        data.truncate(size);
        let imported = gpu::executable_cache_create(
            c.device,
            &ExecutableCacheDesc {
                initial: Bytes {
                    data: data.as_ptr().cast(),
                    size: data.len(),
                },
                ..cache_desc
            },
        )?;
        gpu::executable_cache_merge(cache, &[imported])?;
        gpu::executable_cache_destroy(imported);
        gpu::executable_cache_destroy(cache);

        let base = gpu::memory_address(c.memory.span(0, 4096))?;
        for (phase, seed) in [0u32, 9, 0].into_iter().enumerate() {
            ptr::write_bytes(c.mapping.data, 0xa5, 4096);
            let words = c.mapping.data.cast::<u32>();
            for i in 0..65 {
                words.add(i).write(i as u32 + seed);
            }
            let roots = c.mapping.data.cast::<u8>().add(1024).cast::<Root>();
            roots.write(Root {
                address: base,
                count: 65,
                add: seed,
            });
            roots.add(1).write(Root {
                address: base + 512,
                count: 65,
                add: 7,
            });
            roots.add(2).write(Root {
                address: base + 2048,
                count: 65,
                add: 11,
            });
            gpu::memory_flush(c.memory.span(0, 4096))?;
            let list = gpu::commands_begin(c.arena, &recording())?;
            gpu::bind_executable(list, executable);
            gpu::set_root(list, STAGE_COMPUTE, 0, base + 1024);
            gpu::set_root(list, STAGE_COMPUTE, 1, base + 1040);
            let mut control = [0u8; 8];
            control[..4].copy_from_slice(&3u32.to_ne_bytes());
            gpu::set_inline(list, STAGE_COMPUTE, 16, &control);
            gpu::dispatch(
                list,
                &Launch {
                    groups: Extent { x: 2, y: 1, z: 1 },
                    dynamic_shared_bytes: 0,
                    extensions: ptr::null(),
                },
            );
            gpu::set_root(list, STAGE_COMPUTE, 1, base + 1056);
            gpu::set_inline(list, STAGE_COMPUTE, 16, &5u32.to_ne_bytes());
            gpu::fill_memory(list, c.memory.span(1536, 4), 2);
            gpu::fill_memory(list, c.memory.span(1540, 8), 1);
            gpu::barrier(
                list,
                &dependency(
                    STAGE_TRANSFER,
                    STAGE_INDIRECT,
                    ACCESS_COPY_WRITE,
                    ACCESS_INDIRECT_READ,
                ),
            );
            gpu::dispatch_indirect(list, c.memory.span(1536, 12), 0);
            gpu::barrier(
                list,
                &dependency(
                    STAGE_COMPUTE,
                    STAGE_HOST,
                    ACCESS_SHADER_WRITE,
                    ACCESS_HOST_READ,
                ),
            );
            let list = gpu::commands_end(list)?;
            submit(c, list, phase as u64 + 4, storage)?;
            for i in 0..65 {
                assert_eq!(words.add(i).read(), i as u32 + seed);
                assert_eq!(
                    words.add(128 + i).read(),
                    (i as u32 + seed) * 3 + seed + 7 + extra
                );
                assert_eq!(
                    words.add(512 + i).read(),
                    (i as u32 + seed) * 5 + seed + 11 + extra
                );
            }
            assert_eq!(words.add(128 + 65).read(), 0xa5a5_a5a5);
            assert_eq!(words.add(512 + 65).read(), 0xa5a5_a5a5);
        }
        gpu::executable_destroy(executable);
    }
    Ok(())
}

unsafe fn images(c: &Context, storage: &mut Storage) -> gpu::Result<()> {
    let desc = ImageDesc {
        header: Record::new::<ImageDesc>(IMAGE_DESC),
        format: RGBA8_UNORM,
        dimension: IMAGE_2D,
        mip_count: 1,
        layer_count: 1,
        sample_count: 1,
        extent: Extent { x: 8, y: 8, z: 1 },
        usage: IMAGE_COPY_SRC | IMAGE_COPY_DST | IMAGE_SAMPLED,
        flags: 0,
        view_formats: ptr::null(),
        view_format_count: 0,
        concurrent_domains: ptr::null(),
        concurrent_domain_count: 0,
    };
    let full = Subresources {
        aspects: ASPECT_COLOR,
        first_mip: 0,
        mip_count: 1,
        first_layer: 0,
        layer_count: 1,
    };
    unsafe {
        let limits = gpu::image_limits_query(
            c.device,
            &ImageProfile {
                header: Record::new::<ImageProfile>(IMAGE_PROFILE),
                format: desc.format,
                dimension: desc.dimension,
                usage: desc.usage,
                image_flags: 0,
                reserved: 0,
            },
        )?;
        assert!(limits.max_extent[0] >= 8 && limits.max_extent[1] >= 8);
        let mut req = Requirements::default();
        gpu::image_requirements(c.device, &desc, &mut req)?;
        let mut types = vec![0u32; req.compatible_type_count as usize];
        req.compatible_type_capacity = types.len() as u32;
        req.compatible_types = types.as_mut_ptr();
        gpu::image_requirements(c.device, &desc, &mut req)?;
        let image = gpu::image_create_unbound(c.device, &desc)?;
        let view_desc = ViewDesc {
            header: Record::new::<ViewDesc>(VIEW_DESC),
            format: RGBA8_UNORM,
            dimension: VIEW_2D,
            usage: IMAGE_SAMPLED as u32,
            component_mapping: [0; 4],
            range: full,
        };
        assert_eq!(gpu::view_create(image, &view_desc), Err(INVALID));
        let backing = gpu::memory_create_dedicated_image(image, types[0])?;
        gpu::image_bind(image, backing.span(0, req.size))?;
        let view = gpu::view_create(image, &view_desc)?;
        let mut descriptor_limits = DescriptorLimits::default();
        gpu::device_query(
            c.device,
            &mut Query {
                header: Record::new::<Query>(DESCRIPTOR_LIMITS),
                data: ptr::from_mut(&mut descriptor_limits).cast(),
                capacity: 1,
                count: 0,
                element_size: size_of::<DescriptorLimits>() as u32,
                reserved: 0,
            },
        )?;
        let mut output = Storage::new(HostRequirements {
            size: descriptor_limits.image_size,
            alignment: descriptor_limits.image_alignment,
        })?;
        let mut ds = Storage::new(gpu::descriptor_scratch_requirements(1, 1)?)?;
        let input = ResourceDescriptor {
            kind: 1,
            image_state: STATE_GENERAL,
            view: view.as_raw(),
            buffer: Span {
                memory: ptr::null_mut(),
                offset: 0,
                size: 0,
            },
        };
        assert_eq!(
            gpu::write_resource_descriptors(c.device, std::slice::from_ref(&input), &[], ds.span()),
            Err(INVALID)
        );
        gpu::write_resource_descriptors(c.device, &[input], &[output.span()], ds.span())?;
        let sampler = SamplerDesc {
            header: Record::new::<SamplerDesc>(SAMPLER_DESC),
            min_filter: 0,
            mag_filter: 0,
            mip_filter: 0,
            address_mode: [2; 3],
            compare_enable: 0,
            compare_op: 0,
            border_color: 0,
            unnormalized_coordinates: 0,
            min_lod: 0.0,
            max_lod: 0.0,
            lod_bias: 0.0,
            max_anisotropy: 1.0,
        };
        let mut sampler_output = Storage::new(HostRequirements {
            size: descriptor_limits.sampler_size,
            alignment: descriptor_limits.sampler_alignment,
        })?;
        let mut ss = Storage::new(gpu::descriptor_scratch_requirements(2, 1)?)?;
        gpu::write_sampler_descriptors(c.device, &[sampler], &[sampler_output.span()], ss.span())?;
        let mut bs = Storage::new(gpu::barrier_scratch_requirements(0, 1)?)?;
        for (phase, green) in [false, true, false].into_iter().enumerate() {
            ptr::write_bytes(c.mapping.data, 0xa5, 4096);
            gpu::memory_flush(c.memory.span(0, 4096))?;
            let list = gpu::commands_begin(c.arena, &recording())?;
            let mut barrier = ImageBarrier {
                image: image.as_raw(),
                range: full,
                before: 0,
                after: ACCESS_COPY_WRITE,
                old_state: STATE_UNDEFINED,
                new_state: STATE_COPY_DST,
                source_domain: DOMAIN_IGNORED,
                destination_domain: DOMAIN_IGNORED,
                discard: 1,
            };
            let scratch = bs.span();
            let mut dep = dependency(0, STAGE_TRANSFER, 0, 0);
            dep.image_count = 1;
            dep.images = &barrier;
            dep.scratch = scratch.data;
            dep.scratch_size = scratch.size;
            gpu::barrier(list, &dep);
            let color = if green {
                [0.0, 1.0, 0.0, 1.0]
            } else {
                [1.0, 0.0, 0.0, 1.0]
            };
            gpu::clear_image(list, image, STATE_COPY_DST, full, ClearValue { f32: color });
            barrier.before = ACCESS_COPY_WRITE;
            barrier.after = ACCESS_COPY_READ;
            barrier.old_state = STATE_COPY_DST;
            barrier.new_state = STATE_COPY_SRC;
            barrier.discard = 0;
            dep.images = &barrier;
            dep.before = STAGE_TRANSFER;
            gpu::barrier(list, &dep);
            gpu::copy_from_image(
                list,
                c.memory.span(256, 256),
                image,
                &ImageCopy {
                    region: ImageRegion {
                        aspect: ASPECT_COLOR,
                        mip: 0,
                        first_layer: 0,
                        layer_count: 1,
                        offset: Offset { x: 0, y: 0, z: 0 },
                        extent: desc.extent,
                    },
                    row_pitch: 0,
                    slice_pitch: 0,
                    state: STATE_COPY_SRC,
                    reserved: 0,
                },
            );
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
            submit(c, list, phase as u64 + 7, storage)?;
            let expected = if green {
                [0, 255, 0, 255]
            } else {
                [255, 0, 0, 255]
            };
            let bytes = c.mapping.data.cast::<u8>();
            for pixel in 0..64 {
                for (channel, value) in expected.into_iter().enumerate() {
                    assert_eq!(bytes.add(256 + pixel * 4 + channel).read(), value);
                }
            }
            assert_eq!(bytes.add(255).read(), 0xa5);
            assert_eq!(bytes.add(512).read(), 0xa5);
        }
        gpu::view_destroy(view);
        gpu::image_destroy(image);
        gpu::memory_destroy(backing);
    }
    Ok(())
}
