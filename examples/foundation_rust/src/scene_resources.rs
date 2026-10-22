//! Application-owned placement, assets and recording; no private/backend calls.
use ogpu::foundation::{rust as gpu, types::*};
use std::ptr;
mod shaders {
    include!(concat!(env!("OUT_DIR"), "/textured.rs"));
}
pub const VERTEX: u64 = 8;
pub const FRAGMENT: u64 = 16;
pub const COLOR: u64 = 64;
pub const DEPTH: u64 = 128;
pub const COLOR_WRITE: u64 = 8192;
pub const DEPTH_WRITE: u64 = 32768;
pub const SAMPLED_READ: u64 = 262144;
pub const COLOR_STATE: u32 = 5;
pub const DEPTH_STATE: u32 = 6;
pub const SHADER_READ: u32 = 4;

pub struct Scratch(Vec<u64>);
impl Scratch {
    pub fn new(req: HostRequirements) -> Self {
        assert!(req.alignment <= 8);
        Self(vec![0; req.size.div_ceil(8) as usize])
    }
    pub fn span(&mut self) -> HostSpan {
        HostSpan {
            data: self.0.as_mut_ptr().cast(),
            size: std::mem::size_of_val(&*self.0) as u64,
        }
    }
}
pub fn range(aspects: u32, mips: u32) -> Subresources {
    Subresources {
        aspects,
        first_mip: 0,
        mip_count: mips,
        first_layer: 0,
        layer_count: 1,
    }
}
pub fn recording() -> RecordingDesc {
    RecordingDesc {
        header: Record::new::<RecordingDesc>(RECORDING_DESC),
        replay_mode: 1,
        level: 0,
        inheritance: ptr::null(),
    }
}
pub struct Buffer {
    pub memory: gpu::MemoryHandle,
    pub mapping: Mapping,
    pub size: u64,
}
impl Buffer {
    pub unsafe fn new(
        d: gpu::DeviceHandle,
        adapter: &ogpu::foundation::Adapter,
        size: u64,
        usage: u64,
        alignment: u64,
    ) -> gpu::Result<Self> {
        let mut desc = MemoryDesc {
            header: Record::new::<MemoryDesc>(MEMORY_DESC),
            size,
            alignment,
            usage,
            memory_type: 0,
            kind: MEMORY_LINEAR,
            concurrent_domains: ptr::null(),
            concurrent_domain_count: 0,
            flags: 0,
        };
        let mut types = [0; 32];
        let mut req = Requirements {
            compatible_types: types.as_mut_ptr(),
            compatible_type_capacity: 32,
            ..Default::default()
        };
        unsafe {
            gpu::memory_requirements(d, &desc, &mut req)?;
        }
        desc.memory_type = gpu::adapter_memory_types(adapter)
            .iter()
            .find(|m| {
                m.properties & MEMORY_HOST_VISIBLE != 0
                    && types[..req.compatible_type_count as usize].contains(&m.id)
            })
            .ok_or(UNSUPPORTED)?
            .id;
        let memory = unsafe { gpu::memory_create(d, &desc)? };
        let mapping = unsafe { gpu::memory_map(memory.span(0, size))? };
        Ok(Self {
            memory,
            mapping,
            size,
        })
    }
    pub fn span(&self) -> Span {
        self.memory.span(0, self.size)
    }
    pub unsafe fn destroy(self) {
        unsafe {
            gpu::memory_unmap(self.memory);
            gpu::memory_destroy(self.memory);
        }
    }
}
pub struct Image {
    pub image: gpu::ImageHandle,
    pub memory: gpu::MemoryHandle,
    pub view: gpu::ViewHandle,
}
impl Image {
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn new(
        d: gpu::DeviceHandle,
        adapter: &ogpu::foundation::Adapter,
        format: u32,
        size: [u32; 2],
        mips: u32,
        usage: u64,
        view_usage: u32,
        aspects: u32,
    ) -> gpu::Result<Self> {
        let desc = ImageDesc {
            header: Record::new::<ImageDesc>(IMAGE_DESC),
            format,
            dimension: 2,
            mip_count: mips,
            layer_count: 1,
            sample_count: 1,
            extent: Extent {
                x: size[0],
                y: size[1],
                z: 1,
            },
            usage,
            flags: 0,
            view_formats: ptr::null(),
            view_format_count: 0,
            concurrent_domain_count: 0,
            concurrent_domains: ptr::null(),
        };
        let mut types = [0; 32];
        let mut req = Requirements {
            compatible_types: types.as_mut_ptr(),
            compatible_type_capacity: 32,
            ..Default::default()
        };
        unsafe {
            gpu::image_requirements(d, &desc, &mut req)?;
        }
        let ty = gpu::adapter_memory_types(adapter)
            .iter()
            .filter(|m| types[..req.compatible_type_count as usize].contains(&m.id))
            .min_by_key(|m| u32::from(m.properties & 1 == 0))
            .ok_or(UNSUPPORTED)?
            .id;
        unsafe {
            let image = gpu::image_create_unbound(d, &desc)?;
            let memory = gpu::memory_create_dedicated_image(image, ty)?;
            gpu::image_bind(image, memory.span(0, req.size))?;
            let view = gpu::view_create(
                image,
                &ViewDesc {
                    header: Record::new::<ViewDesc>(VIEW_DESC),
                    format,
                    dimension: 2,
                    usage: view_usage,
                    component_mapping: [0; 4],
                    range: range(aspects, mips),
                },
            )?;
            Ok(Self {
                image,
                memory,
                view,
            })
        }
    }
    pub unsafe fn destroy(self) {
        unsafe {
            gpu::view_destroy(self.view);
            gpu::image_destroy(self.image);
            gpu::memory_destroy(self.memory);
        }
    }
}
pub fn dependency(
    before: u64,
    after: u64,
    src: u64,
    dst: u64,
    scratch: &mut Scratch,
) -> Dependency {
    // The contract's barrier scratch covers the native barrier records,
    // including the global one.
    let span = scratch.span();
    Dependency {
        header: Record::new::<Dependency>(DEPENDENCY),
        before,
        after,
        global_before: src,
        global_after: dst,
        memory_count: 0,
        image_count: 0,
        flags: 0,
        memory: ptr::null(),
        images: ptr::null(),
        scratch: span.data,
        scratch_size: span.size,
    }
}
#[allow(clippy::too_many_arguments)]
pub unsafe fn transition(
    list: gpu::ListHandle,
    image: gpu::ImageHandle,
    range: Subresources,
    old: u32,
    new: u32,
    before: u64,
    after: u64,
    src: u64,
    dst: u64,
    scratch: &mut Scratch,
) {
    let barrier = ImageBarrier {
        image: image.as_raw(),
        range,
        before: src,
        after: dst,
        old_state: old,
        new_state: new,
        source_domain: DOMAIN_IGNORED,
        destination_domain: DOMAIN_IGNORED,
        discard: u32::from(old == STATE_UNDEFINED),
    };
    let storage = scratch.span();
    let mut dep = dependency(before, after, 0, 0, scratch);
    dep.image_count = 1;
    dep.images = &barrier;
    dep.scratch = storage.data;
    dep.scratch_size = storage.size;
    unsafe {
        gpu::barrier(list, &dep);
    }
}
pub unsafe fn raster(
    d: gpu::DeviceHandle,
    format: u32,
    overlay: bool,
) -> gpu::Result<gpu::ExecutableHandle> {
    // The D3D12 ROOT_DESCRIPTORS profile binds the inline vertices address as
    // a root descriptor (u0) and consumes the DXIL variant artifacts; the
    // SPIR-V path reads the same address as inline pointer bytes.
    #[cfg(target_os = "windows")]
    let roots = [RootSlot {
        stages: STAGE_VERTEX,
        offset: 0,
        alignment: 8,
    }];
    #[cfg(not(target_os = "windows"))]
    let roots: [RootSlot; 0] = [];
    let abi = ArgumentInterface {
        header: Record::new::<ArgumentInterface>(ARGUMENT_INTERFACE),
        byte_size: 24,
        root_count: roots.len() as u32,
        roots: roots.as_ptr(),
    };
    #[cfg(target_os = "windows")]
    let entries = [
        (VERTEX, shaders::VERTEX_DXIL, c"vsMain"),
        (FRAGMENT, shaders::FRAGMENT_DXIL, c"fsMain"),
    ];
    #[cfg(not(target_os = "windows"))]
    let entries = [
        (
            VERTEX,
            // SAFETY: the generated words are dense u32 storage.
            unsafe {
                std::slice::from_raw_parts(
                    shaders::VERTEX.as_ptr().cast::<u8>(),
                    shaders::VERTEX.len() * 4,
                )
            },
            c"main",
        ),
        (
            FRAGMENT,
            // SAFETY: the generated words are dense u32 storage.
            unsafe {
                std::slice::from_raw_parts(
                    shaders::FRAGMENT.as_ptr().cast::<u8>(),
                    shaders::FRAGMENT.len() * 4,
                )
            },
            c"main",
        ),
    ];
    let shaders = entries.map(|(stage, code, entry)| Shader {
        stage: stage as u32,
        format: if cfg!(target_os = "windows") {
            SHADER_DXIL
        } else {
            SHADER_SPIRV
        },
        code: Bytes {
            data: code.as_ptr().cast(),
            size: code.len(),
        },
        entry: entry.as_ptr(),
        interface_metadata: &abi.header,
        specialization: ptr::null(),
        subgroup: ptr::null(),
    });
    let req = ShaderRequirements {
        header: Record::new::<ShaderRequirements>(SHADER_REQUIREMENTS),
        features: RASTER,
        local_size: [0; 3],
        shared_memory: 0,
    };
    let color = ColorState {
        format,
        write_mask: 15,
        blend: u32::from(overlay),
        src_color: 6,
        dst_color: 7,
        color_op: 0,
        src_alpha: 1,
        dst_alpha: 7,
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
        depth_format: 14,
        depth_test: u32::from(!overlay),
        depth_write: u32::from(!overlay),
        depth_compare: 1,
        blend_constants: [0.; 4],
        stencil_test: 0,
        stencil_front: Default::default(),
        stencil_back: Default::default(),
        vertex_input: ptr::null(),
        raster_options: ptr::null(),
        logic_op_enable: 0,
        logic_op: 0,
    };
    unsafe {
        gpu::executable_create(
            d,
            &ExecutableDesc {
                header: Record::new::<ExecutableDesc>(EXECUTABLE_DESC),
                kind: 2,
                shader_count: 2,
                shaders: shaders.as_ptr(),
                static_state: &state.header,
                dynamic_state: 1,
                requirements: &req.header,
                cache: ptr::null_mut(),
                compile_flags: 0,
                reserved: 0,
            },
        )
    }
}
pub struct Texture {
    pub image: Image,
    pub heaps: [Buffer; 2],
    pub bindings: [HeapBinding; 2],
}
impl Texture {
    pub unsafe fn new(
        d: gpu::DeviceHandle,
        adapter: &ogpu::foundation::Adapter,
        list: gpu::ListHandle,
        scratch: &mut Scratch,
    ) -> gpu::Result<(Self, Buffer)> {
        unsafe {
            let image = Image::new(
                d,
                adapter,
                3,
                [8, 8],
                4,
                IMAGE_COPY_DST | IMAGE_SAMPLED,
                IMAGE_SAMPLED as u32,
                ASPECT_COLOR,
            )?;
            let upload = Buffer::new(d, adapter, 512, USAGE_COPY_SRC, 4)?;
            let mut offset = 0;
            for mip in 0..4 {
                for y in 0..(8 >> mip) {
                    for x in 0..(8 >> mip) {
                        let color = texel(x, y, mip);
                        for c in color {
                            upload.mapping.data.cast::<u8>().add(offset).write(c);
                            offset += 1;
                        }
                    }
                }
            }
            gpu::memory_flush(upload.span())?;
            transition(
                list,
                image.image,
                range(ASPECT_COLOR, 4),
                STATE_UNDEFINED,
                STATE_COPY_DST,
                0,
                STAGE_TRANSFER,
                0,
                ACCESS_COPY_WRITE,
                scratch,
            );
            offset = 0;
            for mip in 0..4 {
                let side = 8 >> mip;
                let bytes = (side * side * 4) as u64;
                gpu::copy_to_image(
                    list,
                    image.image,
                    upload.memory.span(offset as u64, bytes),
                    &ImageCopy {
                        region: ImageRegion {
                            aspect: ASPECT_COLOR,
                            mip,
                            first_layer: 0,
                            layer_count: 1,
                            offset: Offset { x: 0, y: 0, z: 0 },
                            extent: Extent {
                                x: side,
                                y: side,
                                z: 1,
                            },
                        },
                        row_pitch: 0,
                        slice_pitch: 0,
                        state: STATE_COPY_DST,
                        reserved: 0,
                    },
                );
                offset += bytes as usize;
            }
            transition(
                list,
                image.image,
                range(ASPECT_COLOR, 4),
                STATE_COPY_DST,
                SHADER_READ,
                STAGE_TRANSFER,
                FRAGMENT,
                ACCESS_COPY_WRITE,
                SAMPLED_READ,
                scratch,
            );
            let mut limits = DescriptorLimits::default();
            gpu::device_query(
                d,
                &mut Query {
                    header: Record::new::<Query>(DESCRIPTOR_LIMITS),
                    data: ptr::from_mut(&mut limits).cast(),
                    capacity: 1,
                    count: 0,
                    element_size: size_of::<DescriptorLimits>() as u32,
                    reserved: 0,
                },
            )?;
            let mut heaps = Vec::new();
            let mut bindings = Vec::new();
            for i in 0..2 {
                let (payload, reserved_align, tail, align) = if i == 0 {
                    (
                        limits.image_size,
                        limits.resource_reserved_alignment,
                        limits.resource_reserved_size,
                        limits.resource_heap_alignment,
                    )
                } else {
                    (
                        limits.sampler_size,
                        limits.sampler_reserved_alignment,
                        limits.sampler_reserved_size,
                        limits.sampler_heap_alignment,
                    )
                };
                let offset = payload.div_ceil(reserved_align) * reserved_align;
                let heap = Buffer::new(d, adapter, offset + tail, 128, align)?;
                let mut temp = Scratch::new(gpu::descriptor_scratch_requirements(i + 1, 1)?);
                let out = HostSpan {
                    data: heap.mapping.data,
                    size: payload,
                };
                if i == 0 {
                    gpu::write_resource_descriptors(
                        d,
                        &[ResourceDescriptor {
                            kind: 1,
                            image_state: SHADER_READ,
                            view: image.view.as_raw(),
                            buffer: Span {
                                memory: ptr::null_mut(),
                                offset: 0,
                                size: 0,
                            },
                        }],
                        &[out],
                        temp.span(),
                    )?;
                } else {
                    gpu::write_sampler_descriptors(
                        d,
                        &[SamplerDesc {
                            header: Record::new::<SamplerDesc>(SAMPLER_DESC),
                            min_filter: 0,
                            mag_filter: 0,
                            mip_filter: 0,
                            address_mode: [2; 3],
                            compare_enable: 0,
                            compare_op: 0,
                            border_color: 0,
                            unnormalized_coordinates: 0,
                            min_lod: 0.,
                            max_lod: 3.,
                            lod_bias: 0.,
                            max_anisotropy: 1.,
                        }],
                        &[out],
                        temp.span(),
                    )?;
                }
                gpu::memory_flush(heap.memory.span(0, payload))?;
                bindings.push(HeapBinding {
                    header: Record::new::<HeapBinding>(HEAP_BINDING),
                    kind: i + 1,
                    flags: 0,
                    storage: heap.span(),
                    reserved_offset: offset,
                    reserved_size: tail,
                });
                heaps.push(heap);
            }
            Ok((
                Self {
                    image,
                    heaps: heaps.try_into().ok().unwrap(),
                    bindings: bindings.try_into().ok().unwrap(),
                },
                upload,
            ))
        }
    }
    pub unsafe fn destroy(self) {
        unsafe {
            self.image.destroy();
            for heap in self.heaps {
                heap.destroy();
            }
        }
    }
}
pub fn texel(x: u32, y: u32, mip: u32) -> [u8; 4] {
    [
        ((32 + 13 * x + 7 * y + 23 * mip) % 192) as u8,
        ((48 + 5 * x + 19 * y + 31 * mip) % 192) as u8,
        ((64 + 17 * x + 3 * y + 29 * mip) % 192) as u8,
        255,
    ]
}
