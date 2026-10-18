//! D3D12 descriptor profile: recipe-encoded descriptor bytes (provisional,
//! docs/dx12-backend-d2.md). Descriptor bytes are opaque device-specific
//! recipes living in `DESCRIPTOR_HEAP` memory; `bind_heap` materializes them
//! into the memory's native heaps and binds direct heap indexing (SM 6.6).
use super::*;
use windows::Win32::Graphics::Dxgi::Common as dxgic;

/// Native descriptor slot size of one heap kind.
pub(super) fn native_stride(d: &Device, kind: u32) -> u32 {
    // SAFETY: live device query; returns the native slot size.
    unsafe {
        d.device.GetDescriptorHandleIncrementSize(if kind == HEAP_SAMPLER {
            d3d::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER
        } else {
            d3d::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV
        })
    }
}

/// Resource descriptor recipe size and placement stride (8-aligned record).
pub(super) const RESOURCE_STRIDE: u64 = 40;
/// Sampler descriptor recipe size and placement stride.
pub(super) const SAMPLER_STRIDE: u64 = 80;
/// Native shader-visible heap caps used for honest limits and validation.
pub(super) const MAX_RESOURCE_DESCRIPTORS: u64 = 1 << 20;
pub(super) const MAX_SAMPLER_DESCRIPTORS: u64 = 2048;

/// Descriptor limits of this profile: recipe sizes/alignments and the native
/// heap caps. "Descriptor bytes are opaque and device-specific."
pub(super) fn descriptor_limits() -> DescriptorLimits {
    DescriptorLimits {
        buffer_size: RESOURCE_STRIDE,
        buffer_alignment: 8,
        image_size: RESOURCE_STRIDE,
        image_alignment: 8,
        sampler_size: SAMPLER_STRIDE,
        sampler_alignment: 8,
        resource_heap_alignment: 8,
        resource_heap_max_size: RESOURCE_STRIDE * MAX_RESOURCE_DESCRIPTORS,
        resource_reserved_size: 0,
        resource_reserved_alignment: 8,
        sampler_heap_alignment: 8,
        sampler_heap_max_size: SAMPLER_STRIDE * MAX_SAMPLER_DESCRIPTORS,
        sampler_reserved_size: 0,
        sampler_reserved_alignment: 8,
        uniform_address_alignment: 256,
        storage_address_alignment: 16,
        max_uniform_range: 65536,
        max_storage_range: 1 << 40,
        max_sampler_lod_bias: 15.99,
        max_sampler_anisotropy: 16.0,
    }
}

/// Recipe encoding needs no translation storage on this backend.
pub(in crate::foundation) fn descriptor_scratch(
    _kind: u32,
    _count: u32,
) -> Result<HostRequirements, Status> {
    Ok(HostRequirements {
        size: 0,
        alignment: 1,
    })
}

fn scratch_shape(kind: u32, count: u32) -> Result<(u64, u64), Status> {
    let (stride, maximum) = match kind {
        HEAP_RESOURCE => (RESOURCE_STRIDE, MAX_RESOURCE_DESCRIPTORS),
        HEAP_SAMPLER => (SAMPLER_STRIDE, MAX_SAMPLER_DESCRIPTORS),
        _ => return Err(INVALID),
    };
    if u64::from(count) > maximum {
        return Err(CAPACITY);
    }
    Ok((stride, u64::from(count) * stride))
}

impl Device {
    /// Encode validated resource descriptors into caller host storage. On this
    /// backend the bytes are the opaque recipes `bind_heap` materializes.
    /// # Safety
    /// Live device; inputs and disjoint writable outputs/scratch as declared.
    pub(in crate::foundation) unsafe fn write_resources(
    &self,
    count: u32,
    inputs: *const ResourceDescriptor,
    outputs: *const HostSpan,
    _scratch: HostSpan,
) -> Result<(), Status> {
        self.ready()?;
    // SAFETY: trusted arrays of the declared count.
    let inputs = unsafe { super::slice(inputs, count)? };
    // SAFETY: trusted arrays of the declared count.
    let outputs = unsafe { super::slice(outputs, count)? };
    let (_, total) = scratch_shape(HEAP_RESOURCE, count)?;
    let mut written = 0u64;
    for (input, output) in inputs.iter().zip(outputs.iter()) {
            validate_resource(self, input)?;
        if output.data.is_null() || u64::from(output.size) < RESOURCE_STRIDE {
            return Err(INVALID);
        }
        written += RESOURCE_STRIDE;
        // SAFETY: output spans are writable host storage of the declared size.
        unsafe {
            ptr::copy_nonoverlapping(
                ptr::from_ref(input).cast::<u8>(),
                output.data.cast::<u8>(),
                size_of::<ResourceDescriptor>(),
            );
        }
    }
    debug_assert!(written <= total);
    Ok(())
}

    /// Encode validated sampler descriptors into caller host storage.
    /// # Safety
    /// Live device; inputs and disjoint writable outputs/scratch as declared.
    pub(in crate::foundation) unsafe fn write_samplers(
    &self,
    count: u32,
    inputs: *const SamplerDesc,
    outputs: *const HostSpan,
    _scratch: HostSpan,
) -> Result<(), Status> {
        self.ready()?;
    // SAFETY: trusted arrays of the declared count.
    let inputs = unsafe { super::slice(inputs, count)? };
    // SAFETY: trusted arrays of the declared count.
    let outputs = unsafe { super::slice(outputs, count)? };
    scratch_shape(HEAP_SAMPLER, count)?;
    for (input, output) in inputs.iter().zip(outputs.iter()) {
        input.header.validate::<SamplerDesc>(SAMPLER_DESC)?;
        if input.min_filter > 1
            || input.mag_filter > 1
            || input.mip_filter > 1
            || input.address_mode.iter().any(|m| *m > 3)
            || input.compare_op > 7
            || input.border_color > 4
            || input.unnormalized_coordinates > 1
            || input.max_anisotropy > descriptor_limits().max_sampler_anisotropy
            || input.lod_bias.abs() > descriptor_limits().max_sampler_lod_bias
            || input.min_lod > input.max_lod
        {
            return Err(INVALID);
        }
        if output.data.is_null() || u64::from(output.size) < SAMPLER_STRIDE {
            return Err(INVALID);
        }
        // SAFETY: output spans are writable host storage of the declared size.
        unsafe {
            ptr::copy_nonoverlapping(
                ptr::from_ref(input).cast::<u8>(),
                output.data.cast::<u8>(),
                size_of::<SamplerDesc>(),
            );
        }
    }
    Ok(())
}
}

/// Recipe validation at encode time: kinds, usage consistency and spans.
fn validate_resource(d: &Device, input: &ResourceDescriptor) -> Result<(), Status> {
    match input.kind {
        // Buffer descriptors carry a span; image descriptors carry a view.
        DESCRIPTOR_STORAGE_BUFFER | DESCRIPTOR_UNIFORM_BUFFER => {
            if !input.view.is_null() {
                return Err(INVALID);
            }
            let memory = unsafe { input.buffer.memory.as_ref() }.ok_or(INVALID)?;
            if memory.usage & USAGE_STORAGE == 0 && input.kind == DESCRIPTOR_STORAGE_BUFFER {
                return Err(INVALID);
            }
            if memory.usage & USAGE_UNIFORM == 0 && input.kind == DESCRIPTOR_UNIFORM_BUFFER {
                return Err(INVALID);
            }
            memory.descriptor_range(
                ptr::from_ref(d),
                input.buffer.offset,
                input.buffer.size,
                0,
            )?;
            if input.kind == DESCRIPTOR_UNIFORM_BUFFER {
                if input.buffer.size == 0
                    || input.buffer.size > 65536
                    || input.buffer.offset % 256 != 0
                    || input.buffer.size % 256 != 0
                {
                    return Err(INVALID);
                }
            }
            if input.image_state != STATE_GENERAL {
                return Err(INVALID);
            }
            Ok(())
        }
        DESCRIPTOR_SAMPLED_IMAGE | DESCRIPTOR_STORAGE_IMAGE | DESCRIPTOR_INPUT_ATTACHMENT => {
            // Image descriptors carry a validated view record.
            if !input.buffer.memory.is_null() {
                return Err(INVALID);
            }
            // SAFETY: view handles are trusted live objects by contract.
            let view = unsafe { input.view.as_ref() }.ok_or(INVALID)?;
            if view.image.is_null() {
                return Err(INVALID);
            }
            if input.kind == DESCRIPTOR_STORAGE_IMAGE && input.image_state != STATE_GENERAL {
                return Err(INVALID);
            }
            if input.kind != DESCRIPTOR_STORAGE_IMAGE
                && input.image_state != STATE_GENERAL
                && input.image_state != STATE_SHADER_READ
            {
                return Err(INVALID);
            }
            Ok(())
        }
        _ => Err(INVALID),
    }
}

/// Materialize one recipe into a native descriptor at a heap slot.
/// # Safety
/// The recipe bytes come from live, externally serialized heap memory.
pub(super) unsafe fn materialize(
    d: &Device,
    heap: &d3d::ID3D12DescriptorHeap,
    kind: u32,
    index: u32,
    bytes: &[u8],
) -> Result<(), Status> {
    // SAFETY: the CPU handle targets this live heap's slot.
    let cpu = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
    match kind {
        HEAP_RESOURCE => {
            // SAFETY: bytes hold one validated resource recipe record.
            let recipe: ResourceDescriptor = unsafe { ptr::read(bytes.as_ptr().cast()) };
            // SAFETY: live device query; returns the native slot size.
            let stride = unsafe {
                d.device
                    .GetDescriptorHandleIncrementSize(d3d::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV)
            };
            let handle = d3d::D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: cpu.ptr + stride as usize * index as usize,
            };
            match recipe.kind {
                DESCRIPTOR_STORAGE_BUFFER | DESCRIPTOR_UNIFORM_BUFFER => {
                let memory = unsafe { recipe.buffer.memory.as_ref() }.ok_or(INVALID)?;
                let (resource, offset, address) = memory.descriptor_range(
                    ptr::from_ref(d),
                    recipe.buffer.offset,
                    recipe.buffer.size,
                    0,
                )?;
                match recipe.kind {
                DESCRIPTOR_STORAGE_BUFFER => {
                    let view = d3d::D3D12_UNORDERED_ACCESS_VIEW_DESC {
                        Format: dxgic::DXGI_FORMAT_R32_TYPELESS,
                        ViewDimension: d3d::D3D12_UAV_DIMENSION_BUFFER,
                        Anonymous: d3d::D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                            Buffer: d3d::D3D12_BUFFER_UAV {
                                FirstElement: offset / 4,
                                NumElements: (recipe.buffer.size / 4) as u32,
                                StructureByteStride: 0,
                                CounterOffsetInBytes: 0,
                                Flags: d3d::D3D12_BUFFER_UAV_FLAG_RAW,
                            },
                        },
                    };
                    // SAFETY: live resource and heap slot; no retained state.
                    unsafe {
                        d.device
                            .CreateUnorderedAccessView(resource, None, Some(&view), handle)
                    };
                }
                DESCRIPTOR_UNIFORM_BUFFER => {
                    let view = d3d::D3D12_CONSTANT_BUFFER_VIEW_DESC {
                        BufferLocation: address,
                        SizeInBytes: recipe.buffer.size as u32,
                    };
                    // SAFETY: live resource and heap slot; no retained state.
                    unsafe { d.device.CreateConstantBufferView(Some(&view), handle) };
                }
                DESCRIPTOR_SAMPLED_IMAGE | DESCRIPTOR_INPUT_ATTACHMENT | DESCRIPTOR_STORAGE_IMAGE => {
                    // SAFETY: view handles are trusted live objects by contract.
                    let view = unsafe { recipe.view.as_ref() }.ok_or(INVALID)?;
                    // SAFETY: the view's image is live through every recorded use.
                    let image = unsafe { &*view.image };
                    let resource = image.resource()?;
                    let native = view.native_view_desc()?;
                    if recipe.kind == DESCRIPTOR_STORAGE_IMAGE {
                        // SAFETY: live resource and heap slot; no retained state.
                        unsafe {
                            d.device.CreateUnorderedAccessView(
                                resource,
                                None,
                                Some(&native.unordered_access()),
                                handle,
                            )
                        };
                    } else {
                        // SAFETY: live resource and heap slot; no retained state.
                        unsafe {
                            d.device.CreateShaderResourceView(
                                Some(resource),
                                Some(&native.shader_resource()),
                                handle,
                            )
                        };
                    }
                }
                _ => return Err(INVALID),
                }
                }
                DESCRIPTOR_SAMPLED_IMAGE | DESCRIPTOR_INPUT_ATTACHMENT | DESCRIPTOR_STORAGE_IMAGE => {
                    // SAFETY: view handles are trusted live objects by contract.
                    let view = unsafe { recipe.view.as_ref() }.ok_or(INVALID)?;
                    // SAFETY: the view's image is live through every recorded use.
                    let image = unsafe { &*view.image };
                    let resource = image.resource()?;
                    let native = view.native_view_desc()?;
                    if recipe.kind == DESCRIPTOR_STORAGE_IMAGE {
                        // SAFETY: live resource and heap slot; no retained state.
                        unsafe {
                            d.device.CreateUnorderedAccessView(
                                resource,
                                None,
                                Some(&native.unordered_access()),
                                handle,
                            )
                        };
                    } else {
                        // SAFETY: live resource and heap slot; no retained state.
                        unsafe {
                            d.device.CreateShaderResourceView(
                                Some(resource),
                                Some(&native.shader_resource()),
                                handle,
                            )
                        };
                    }
                }
                _ => return Err(INVALID),
            }
            Ok(())
        }
        HEAP_SAMPLER => {
            // SAFETY: bytes hold one validated sampler recipe record.
            let recipe: SamplerDesc = unsafe { ptr::read(bytes.as_ptr().cast()) };
            // SAFETY: live device query; returns the native slot size.
            let stride = unsafe {
                d.device
                    .GetDescriptorHandleIncrementSize(d3d::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER)
            };
            let handle = d3d::D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: cpu.ptr + stride as usize * index as usize,
            };
            let view = d3d::D3D12_SAMPLER_DESC {
                Filter: sampler_filter(&recipe),
                AddressU: address_mode(recipe.address_mode[0]),
                AddressV: address_mode(recipe.address_mode[1]),
                AddressW: address_mode(recipe.address_mode[2]),
                MipLODBias: recipe.lod_bias,
                MaxAnisotropy: recipe.max_anisotropy as u32,
                ComparisonFunc: comparison(recipe.compare_op),
                BorderColor: match recipe.border_color {
                    1 => [0.0, 0.0, 0.0, 1.0],
                    2 => [1.0, 1.0, 1.0, 1.0],
                    3 => [1.0, 1.0, 1.0, 1.0],
                    _ => [0.0, 0.0, 0.0, 0.0],
                },
                MinLOD: recipe.min_lod,
                MaxLOD: recipe.max_lod,
            };
            // SAFETY: heap slot is live; no retained state.
            unsafe { d.device.CreateSampler(&view, handle) };
            Ok(())
        }
        _ => Err(INVALID),
    }
}

fn sampler_filter(recipe: &SamplerDesc) -> d3d::D3D12_FILTER {
    let anisotropic = recipe.max_anisotropy > 1.0;
    let comparison = recipe.compare_enable != 0;
    let mag = recipe.mag_filter;
    let min = recipe.min_filter;
    let mip = recipe.mip_filter;
    // D3D12 encodes min/mag/mip filtering and comparison into one enum.
    let (min_f, mag_f, mip_f) = if anisotropic {
        (2u32, 2, 1)
    } else {
        (
            u32::from(min != 0),
            u32::from(mag != 0),
            u32::from(mip != 0),
        )
    };
    d3d::D3D12_FILTER(
        (min_f | (mag_f << 4) | (mip_f << 8) | (u32::from(comparison) << 15)) as i32,
    )
}

fn address_mode(mode: u32) -> d3d::D3D12_TEXTURE_ADDRESS_MODE {
    match mode {
        0 => d3d::D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        1 => d3d::D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        2 => d3d::D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        _ => d3d::D3D12_TEXTURE_ADDRESS_MODE_BORDER,
    }
}

fn comparison(op: u32) -> d3d::D3D12_COMPARISON_FUNC {
    match op {
        0 => d3d::D3D12_COMPARISON_FUNC_NEVER,
        1 => d3d::D3D12_COMPARISON_FUNC_LESS,
        2 => d3d::D3D12_COMPARISON_FUNC_EQUAL,
        3 => d3d::D3D12_COMPARISON_FUNC_LESS_EQUAL,
        4 => d3d::D3D12_COMPARISON_FUNC_GREATER,
        5 => d3d::D3D12_COMPARISON_FUNC_NOT_EQUAL,
        6 => d3d::D3D12_COMPARISON_FUNC_GREATER_EQUAL,
        _ => d3d::D3D12_COMPARISON_FUNC_ALWAYS,
    }
}
