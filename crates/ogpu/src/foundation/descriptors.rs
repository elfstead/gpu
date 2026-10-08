//! Opaque descriptor bytes and borrowed GPU heap ranges. No owned slot table,
//! retained resources, implicit allocation/flush/copy or heap-wide mutation lock.
use super::*;
use std::alloc::Layout;

#[derive(Default)]
struct ResourceData {
    view: vk::VkImageViewCreateInfo,
    image: vk::VkImageDescriptorInfoEXT,
    range: vk::VkDeviceAddressRangeKHR,
}
fn array<T>(count: u32) -> Result<Layout, Status> {
    if count == 0 {
        return Ok(Layout::from_size_align(0, 1).unwrap());
    }
    Layout::array::<T>(count as usize).map_err(|_| INVALID)
}
fn layout(kind: u32, count: u32) -> Result<(Layout, usize, usize), Status> {
    let first = match kind {
        1 => array::<ResourceData>(count)?,
        2 => array::<vk::VkSamplerCreateInfo>(count)?,
        _ => return Err(UNSUPPORTED),
    };
    let (combined, resources) = first
        .extend(array::<vk::VkResourceDescriptorInfoEXT>(if kind == 1 {
            count
        } else {
            0
        })?)
        .map_err(|_| INVALID)?;
    let (combined, outputs) = combined
        .extend(array::<vk::VkHostAddressRangeEXT>(count)?)
        .map_err(|_| INVALID)?;
    Ok((combined.pad_to_align(), resources, outputs))
}
pub(in crate::foundation) fn descriptor_scratch(
    kind: u32,
    count: u32,
) -> Result<HostRequirements, Status> {
    let (layout, _, _) = layout(kind, count)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
fn scratch(span: HostSpan, kind: u32, count: u32) -> Result<(*mut u8, usize, usize), Status> {
    let (layout, r, o) = layout(kind, count)?;
    if span.size < layout.size() as u64 {
        return Err(CAPACITY);
    }
    if layout.size() != 0 && (span.data.is_null() || span.data as usize % layout.align() != 0) {
        return Err(INVALID);
    }
    Ok((span.data.cast(), r, o))
}
fn destination(span: HostSpan, size: u64) -> Result<vk::VkHostAddressRangeEXT, Status> {
    if size == 0 || size > isize::MAX as u64 {
        return Err(UNSUPPORTED);
    }
    if span.size < size {
        return Err(CAPACITY);
    }
    if span.data.is_null() {
        return Err(INVALID);
    }
    // Limit the driver's destination range to descriptor bytes, not caller padding.
    Ok(vk::VkHostAddressRangeEXT {
        address: span.data,
        size: size as usize,
    })
}
fn sampler(
    desc: &SamplerDesc,
    limits: &DescriptorLimits,
    enabled: u64,
) -> Result<vk::VkSamplerCreateInfo, Status> {
    desc.header.validate::<SamplerDesc>(SAMPLER_DESC)?;
    if desc.min_filter > 1
        || desc.mag_filter > 1
        || desc.mip_filter > 1
        || desc.address_mode.iter().any(|v| *v > 3)
        || desc.border_color > 5
    {
        return Err(UNSUPPORTED);
    }
    if desc.compare_enable > 1
        || desc.compare_op > 7
        || desc.unnormalized_coordinates > 1
        || ![
            desc.min_lod,
            desc.max_lod,
            desc.lod_bias,
            desc.max_anisotropy,
        ]
        .iter()
        .all(|v| v.is_finite())
        || desc.min_lod > desc.max_lod
        || desc.max_anisotropy < 1.0
    {
        return Err(INVALID);
    }
    if desc.lod_bias.abs() > limits.max_sampler_lod_bias
        || desc.max_anisotropy > limits.max_sampler_anisotropy
        || (desc.max_anisotropy > 1.0 && enabled & SAMPLER_ANISOTROPY == 0)
    {
        return Err(UNSUPPORTED);
    }
    if desc.unnormalized_coordinates != 0
        && (desc.min_filter != desc.mag_filter
            || desc.mip_filter != 0
            || desc.min_lod != 0.0
            || desc.max_lod != 0.0
            || desc.compare_enable != 0
            || desc.max_anisotropy != 1.0
            || !matches!(desc.address_mode[0], 2 | 3)
            || !matches!(desc.address_mode[1], 2 | 3))
    {
        return Err(INVALID);
    }
    Ok(vk::VkSamplerCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,
        magFilter: desc.mag_filter,
        minFilter: desc.min_filter,
        mipmapMode: desc.mip_filter,
        addressModeU: desc.address_mode[0],
        addressModeV: desc.address_mode[1],
        addressModeW: desc.address_mode[2],
        mipLodBias: desc.lod_bias,
        anisotropyEnable: u32::from(desc.max_anisotropy > 1.0),
        maxAnisotropy: desc.max_anisotropy,
        compareEnable: desc.compare_enable,
        compareOp: desc.compare_op,
        minLod: desc.min_lod,
        maxLod: desc.max_lod,
        borderColor: desc.border_color,
        unnormalizedCoordinates: desc.unnormalized_coordinates,
        ..Default::default()
    })
}

impl Device {
    pub(in crate::foundation) unsafe fn write_resources(
        &self,
        count: u32,
        inputs: *const ResourceDescriptor,
        outputs: *const HostSpan,
        temporary: HostSpan,
    ) -> Result<(), Status> {
        self.ready()?;
        array::<ResourceDescriptor>(count)?;
        array::<HostSpan>(count)?;
        let (bytes, resource_offset, output_offset) = scratch(temporary, 1, count)?;
        if count == 0 {
            return Ok(());
        }
        if inputs.is_null() || outputs.is_null() {
            return Err(INVALID);
        }
        let data = bytes.cast::<ResourceData>();
        let resources = bytes
            .wrapping_add(resource_offset)
            .cast::<vk::VkResourceDescriptorInfoEXT>();
        let destinations = bytes
            .wrapping_add(output_offset)
            .cast::<vk::VkHostAddressRangeEXT>();
        let limits = &self.snapshot.descriptor_limits;
        for i in 0..count as usize {
            // SAFETY: trusted input/output arrays and disjoint caller scratch.
            let input = unsafe { &*inputs.add(i) };
            let mut info = vk::VkResourceDescriptorInfoEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RESOURCE_DESCRIPTOR_INFO_EXT,
                ..Default::default()
            };
            let mut entry = ResourceData::default();
            let size;
            match input.kind {
                1 | 2 | 5 => {
                    if !input.buffer.memory.is_null()
                        || input.buffer.offset != 0
                        || input.buffer.size != 0
                    {
                        return Err(INVALID);
                    }
                    let view = unsafe { input.view.as_ref() }.ok_or(INVALID)?;
                    let (native_view, image_layout) =
                        view.descriptor(self, input.kind, input.image_state)?;
                    entry.view = native_view;
                    entry.image = vk::VkImageDescriptorInfoEXT {
                        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_DESCRIPTOR_INFO_EXT,
                        layout: image_layout,
                        pView: unsafe { ptr::addr_of!((*data.add(i)).view) },
                        ..Default::default()
                    };
                    info.type_ = match input.kind {
                        1 => vk::VkDescriptorType_VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE,
                        2 => vk::VkDescriptorType_VK_DESCRIPTOR_TYPE_STORAGE_IMAGE,
                        _ => vk::VkDescriptorType_VK_DESCRIPTOR_TYPE_INPUT_ATTACHMENT,
                    };
                    info.data = vk::VkResourceDescriptorDataEXT {
                        pImage: unsafe { ptr::addr_of!((*data.add(i)).image) },
                    };
                    size = limits.image_size;
                }
                3 | 4 => {
                    if !input.view.is_null() || input.image_state != 0 || input.buffer.size == 0 {
                        return Err(INVALID);
                    }
                    let memory = unsafe { input.buffer.memory.as_ref() }.ok_or(INVALID)?;
                    let uniform = input.kind == 4;
                    let (_, _, address) = memory.descriptor_range(
                        self,
                        input.buffer.offset,
                        input.buffer.size,
                        if uniform { 32 } else { 64 },
                    )?;
                    let (alignment, maximum) = if uniform {
                        (limits.uniform_address_alignment, limits.max_uniform_range)
                    } else {
                        (limits.storage_address_alignment, limits.max_storage_range)
                    };
                    if alignment == 0 || address % alignment != 0 {
                        return Err(INVALID);
                    }
                    if input.buffer.size > maximum {
                        return Err(UNSUPPORTED);
                    }
                    entry.range = vk::VkDeviceAddressRangeKHR {
                        address,
                        size: input.buffer.size,
                    };
                    info.type_ = if uniform {
                        vk::VkDescriptorType_VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER
                    } else {
                        vk::VkDescriptorType_VK_DESCRIPTOR_TYPE_STORAGE_BUFFER
                    };
                    info.data = vk::VkResourceDescriptorDataEXT {
                        pAddressRange: unsafe { ptr::addr_of!((*data.add(i)).range) },
                    };
                    size = limits.buffer_size;
                }
                _ => return Err(UNSUPPORTED),
            }
            let dest = destination(unsafe { outputs.add(i).read() }, size)?;
            unsafe {
                data.add(i).write(entry);
                resources.add(i).write(info);
                destinations.add(i).write(dest);
            }
        }
        // All local validation precedes the only native write. Native failures may
        // leave destination bytes partially written; no rollback or hidden staging.
        unsafe {
            self.result((self.f.vkWriteResourceDescriptorsEXT.unwrap())(
                self.handle,
                count,
                resources,
                destinations,
            ))
        }
    }
    pub(in crate::foundation) unsafe fn write_samplers(
        &self,
        count: u32,
        inputs: *const SamplerDesc,
        outputs: *const HostSpan,
        temporary: HostSpan,
    ) -> Result<(), Status> {
        self.ready()?;
        array::<SamplerDesc>(count)?;
        array::<HostSpan>(count)?;
        let (bytes, _, output_offset) = scratch(temporary, 2, count)?;
        if count == 0 {
            return Ok(());
        }
        if inputs.is_null() || outputs.is_null() {
            return Err(INVALID);
        }
        let samplers = bytes.cast::<vk::VkSamplerCreateInfo>();
        let destinations = bytes
            .wrapping_add(output_offset)
            .cast::<vk::VkHostAddressRangeEXT>();
        let limits = &self.snapshot.descriptor_limits;
        for i in 0..count as usize {
            let pointer = unsafe { inputs.add(i) };
            unsafe { pointer.cast::<Record>().read() }.validate::<SamplerDesc>(SAMPLER_DESC)?;
            let info = sampler(unsafe { &*pointer }, limits, self.snapshot.features.enabled)?;
            let dest = destination(unsafe { outputs.add(i).read() }, limits.sampler_size)?;
            unsafe {
                samplers.add(i).write(info);
                destinations.add(i).write(dest);
            }
        }
        unsafe {
            self.result((self.f.vkWriteSamplerDescriptorsEXT.unwrap())(
                self.handle,
                count,
                samplers,
                destinations,
            ))
        }
    }
    pub(super) unsafe fn bind_heap(
        &self,
        command: vk::VkCommandBuffer,
        domain: u32,
        binding: &HeapBinding,
    ) -> Result<(), Status> {
        binding.header.validate::<HeapBinding>(HEAP_BINDING)?;
        if binding.flags != 0 {
            return Err(UNSUPPORTED);
        }
        let queue = self
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == domain)
            .ok_or(INVALID)?;
        if queue.flags & (GRAPHICS | COMPUTE) == 0 {
            return Err(UNSUPPORTED);
        }
        let p = &self.snapshot.descriptor_limits;
        let (alignment, maximum, reserved, offset_alignment, f) = match binding.kind {
            1 => (
                p.resource_heap_alignment,
                p.resource_heap_max_size,
                p.resource_reserved_size,
                p.resource_reserved_alignment,
                self.f.vkCmdBindResourceHeapEXT,
            ),
            2 => (
                p.sampler_heap_alignment,
                p.sampler_heap_max_size,
                p.sampler_reserved_size,
                p.sampler_reserved_alignment,
                self.f.vkCmdBindSamplerHeapEXT,
            ),
            _ => return Err(UNSUPPORTED),
        };
        let span = binding.storage;
        let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, address) = memory.command_range(self, domain, span.offset, span.size, 128)?;
        if alignment == 0 || offset_alignment == 0 {
            return Err(UNSUPPORTED);
        }
        if span.size == 0
            || address % alignment != 0
            || binding.reserved_offset % offset_alignment != 0
            || binding.reserved_offset > span.size
            || binding.reserved_size > span.size - binding.reserved_offset
        {
            return Err(INVALID);
        }
        if span.size > maximum || binding.reserved_size < reserved {
            return Err(UNSUPPORTED);
        }
        let info = vk::VkBindHeapInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_HEAP_INFO_EXT,
            heapRange: vk::VkDeviceAddressRangeKHR {
                address,
                size: span.size,
            },
            reservedRangeOffset: binding.reserved_offset,
            reservedRangeSize: binding.reserved_size,
            ..Default::default()
        };
        unsafe {
            f.unwrap()(command, &info);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    thread_local! {
        static CALLS: Cell<u32> = const { Cell::new(0) };
        static FAILURE: Cell<vk::VkResult> = const { Cell::new(vk::VkResult_VK_SUCCESS) };
    }
    unsafe extern "C" fn write_failure(
        _: vk::VkDevice,
        count: u32,
        inputs: *const vk::VkSamplerCreateInfo,
        outputs: *const vk::VkHostAddressRangeEXT,
    ) -> vk::VkResult {
        CALLS.set(CALLS.get() + 1);
        assert_eq!(count, 2);
        unsafe {
            assert_eq!((*inputs).anisotropyEnable, 0);
            assert!((*outputs).size > 0);
            // Model a driver writing one destination before failing the batch.
            (*outputs).address.cast::<u8>().write(0x42);
        }
        FAILURE.get()
    }
    fn basic_sampler() -> SamplerDesc {
        SamplerDesc {
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
        }
    }
    #[test]
    #[ignore = "requires modern Vulkan; descriptor failure contracts and concurrent native encoding"]
    fn gpu_foundation_descriptors() {
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let domain = snapshot
                .queues
                .iter()
                .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let requests = [QueueRequest {
                domain,
                count: 1,
                priority: 0.5,
            }];
            let mut device = Device::create(&adapter, &requests, 0).unwrap();
            // Real native writers on four independent caller buffers/scratch areas.
            std::thread::scope(|scope| {
                for _ in 0..4 {
                    let device = &*device;
                    scope.spawn(move || {
                        let req = descriptor_scratch(2, 1).unwrap();
                        let mut scratch = vec![0u64; req.size.div_ceil(8) as usize];
                        assert!(req.alignment <= align_of::<u64>() as u64);
                        let mut output =
                            vec![0u8; device.snapshot.descriptor_limits.sampler_size as usize];
                        let dest = HostSpan {
                            data: output.as_mut_ptr().cast(),
                            size: output.len() as u64,
                        };
                        assert_eq!(
                            unsafe {
                                device.write_samplers(
                                    1,
                                    &basic_sampler(),
                                    &dest,
                                    HostSpan {
                                        data: scratch.as_mut_ptr().cast(),
                                        size: req.size,
                                    },
                                )
                            },
                            Ok(())
                        );
                    });
                }
            });
            // No children exist when replacing the dispatch pointer.
            device.f.vkWriteSamplerDescriptorsEXT = Some(write_failure);
            CALLS.set(0);
            let req = descriptor_scratch(2, 2).unwrap();
            let mut scratch = vec![0u64; req.size.div_ceil(8) as usize];
            let temporary = HostSpan {
                data: scratch.as_mut_ptr().cast(),
                size: req.size,
            };
            let size = device.snapshot.descriptor_limits.sampler_size as usize;
            let mut output = vec![0xa5u8; size * 2];
            let outputs = [
                HostSpan {
                    data: output.as_mut_ptr().cast(),
                    size: size as u64,
                },
                HostSpan {
                    data: output.as_mut_ptr().wrapping_add(size).cast(),
                    size: size as u64,
                },
            ];
            let mut inputs = [basic_sampler(), basic_sampler()];
            inputs[1].max_anisotropy = f32::NAN;
            assert_eq!(
                unsafe { device.write_samplers(2, inputs.as_ptr(), outputs.as_ptr(), temporary) },
                Err(INVALID)
            );
            assert_eq!(CALLS.get(), 0);
            assert!(output.iter().all(|v| *v == 0xa5));
            inputs[1] = basic_sampler();
            FAILURE.set(vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY);
            assert_eq!(
                unsafe { device.write_samplers(2, inputs.as_ptr(), outputs.as_ptr(), temporary) },
                Err(OUT_OF_MEMORY)
            );
            assert_eq!(CALLS.get(), 1);
            assert_eq!(output[0], 0x42);
            assert!(output[1..].iter().all(|v| *v == 0xa5));
            assert_eq!(device.ready(), Ok(()));
            FAILURE.set(vk::VkResult_VK_ERROR_DEVICE_LOST);
            assert_eq!(
                unsafe { device.write_samplers(2, inputs.as_ptr(), outputs.as_ptr(), temporary) },
                Err(DEVICE_LOST)
            );
            assert_eq!(CALLS.get(), 2);
            assert_eq!(
                unsafe { device.write_samplers(2, inputs.as_ptr(), outputs.as_ptr(), temporary) },
                Err(DEVICE_LOST)
            );
            assert_eq!(CALLS.get(), 2);
            tested += 1;
        }
        assert!(tested > 0, "No suitable device; not a passing skip");
    }
    #[test]
    fn sampler_constraints_and_scratch_layout() {
        let limits = DescriptorLimits {
            max_sampler_anisotropy: 16.0,
            max_sampler_lod_bias: 2.0,
            ..Default::default()
        };
        let mut desc = SamplerDesc {
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
        assert!(sampler(&desc, &limits, 0).is_ok());
        desc.max_anisotropy = 2.0;
        assert!(matches!(sampler(&desc, &limits, 0), Err(UNSUPPORTED)));
        assert_eq!(
            sampler(&desc, &limits, SAMPLER_ANISOTROPY)
                .unwrap()
                .anisotropyEnable,
            1
        );
        desc.max_anisotropy = 1.0;
        desc.unnormalized_coordinates = 1;
        desc.max_lod = 1.0;
        assert!(matches!(sampler(&desc, &limits, 0), Err(INVALID)));
        desc.max_lod = 0.0;
        assert!(sampler(&desc, &limits, 0).is_ok());
        desc.lod_bias = f32::NAN;
        assert!(matches!(sampler(&desc, &limits, 0), Err(INVALID)));
        for kind in [1, 2] {
            assert_eq!(
                descriptor_scratch(kind, 0),
                Ok(HostRequirements {
                    size: 0,
                    alignment: 1
                })
            );
            let req = descriptor_scratch(kind, 7).unwrap();
            assert!(req.size > 0 && req.alignment.is_power_of_two());
            assert_eq!(
                scratch(
                    HostSpan {
                        data: ptr::null_mut(),
                        size: 0
                    },
                    kind,
                    7
                ),
                Err(CAPACITY)
            );
            assert_eq!(
                scratch(
                    HostSpan {
                        data: ptr::null_mut(),
                        size: req.size
                    },
                    kind,
                    7
                ),
                Err(INVALID)
            );
        }
    }
}
