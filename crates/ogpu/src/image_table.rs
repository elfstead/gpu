//! Bounded immutable image-table experiment. No descriptor sets or mutable slots.
use super::*;

pub(crate) const SAMPLED: u32 = 0;
pub(crate) const STORAGE: u32 = 1;

fn align_up(value: u64, alignment: u64) -> Result<u64, Error> {
    if !alignment.is_power_of_two() {
        return Err(Error::new(UNSUPPORTED, "Invalid heap alignment"));
    }
    value
        .checked_add(alignment - 1)
        .map(|v| v & !(alignment - 1))
        .ok_or_else(|| Error::new(OUT_OF_RANGE, "Descriptor heap size overflow"))
}

struct Heap {
    buffer: Buffer,
    info: vk::VkBindHeapInfoEXT,
    offset: usize,
    stride: usize,
}

impl Heap {
    fn new(device: Rc<Device>, count: u32, sampler: bool) -> Result<Self, Error> {
        let p = &device.heap_limits;
        let (stride, alignment, reserved_alignment, reserved, limit) = if sampler {
            (
                p.samplerDescriptorSize,
                p.samplerHeapAlignment,
                p.samplerDescriptorAlignment,
                p.minSamplerHeapReservedRange,
                p.maxSamplerHeapSize,
            )
        } else {
            (
                p.imageDescriptorSize,
                p.resourceHeapAlignment,
                p.imageDescriptorAlignment.max(p.bufferDescriptorAlignment),
                p.minResourceHeapReservedRange,
                p.maxResourceHeapSize,
            )
        };
        let bytes = stride
            .checked_mul(u64::from(count))
            .ok_or_else(|| Error::new(OUT_OF_RANGE, "Descriptor count overflow"))?;
        let reserved_offset = align_up(bytes, reserved_alignment)?;
        let size = reserved_offset
            .checked_add(reserved)
            .ok_or_else(|| Error::new(OUT_OF_RANGE, "Descriptor reservation overflow"))?;
        if stride == 0 || count == 0 || size > limit {
            return Err(Error::new(
                OUT_OF_RANGE,
                "Descriptor heap exceeds device limit",
            ));
        }
        let backing_size = align_up(size, alignment)?
            .checked_add(alignment - 1)
            .filter(|&n| n <= isize::MAX as u64)
            .ok_or_else(|| Error::new(OUT_OF_RANGE, "Descriptor allocation overflow"))?;
        let buffer = Buffer::with_usage(
            device,
            backing_size as usize,
            vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_DESCRIPTOR_HEAP_BIT_EXT,
        )?;
        let address = align_up(buffer.address, alignment)?;
        let offset = (address - buffer.address) as usize;
        Ok(Self {
            buffer,
            offset,
            stride: stride as usize,
            info: vk::VkBindHeapInfoEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_HEAP_INFO_EXT,
                heapRange: vk::VkDeviceAddressRangeEXT { address, size },
                reservedRangeOffset: reserved_offset,
                reservedRangeSize: reserved,
                ..Default::default()
            },
        })
    }

    fn destination(&self, index: usize) -> vk::VkHostAddressRangeEXT {
        // Construction-only access: after binding, neither descriptor bytes nor the
        // implementation reservation are touched by the host, including cache ops.
        vk::VkHostAddressRangeEXT {
            address: unsafe {
                self.buffer
                    .mapped
                    .add(self.offset + index * self.stride)
                    .cast()
            },
            size: self.stride,
        }
    }
}

pub(crate) struct ImageTable {
    pub(super) device: Rc<Device>,
    resources: Heap,
    samplers: Heap,
    _entries: Vec<(Rc<Target>, u32)>,
}

impl ImageTable {
    pub(crate) fn new(device: Rc<Device>, entries: Vec<(Rc<Target>, u32)>) -> Result<Self, Error> {
        device.ready()?;
        if !device.graphics {
            return Err(Error::new(
                UNSUPPORTED,
                "Image tables currently require the graphics image profile",
            ));
        }
        if entries.is_empty() || entries.len() > u32::MAX as usize {
            return Err(Error::new(INVALID_ARGUMENT, "Image table must be nonempty"));
        }
        for (target, kind) in &entries {
            if !Rc::ptr_eq(&device, &target.device) || !matches!(*kind, SAMPLED | STORAGE) {
                return Err(Error::new(
                    INVALID_ARGUMENT,
                    "Invalid image-table device or descriptor kind",
                ));
            }
        }
        let resources = Heap::new(device.clone(), entries.len() as u32, false)?;
        let samplers = Heap::new(device.clone(), 1, true)?;
        let views: Vec<_> = entries
            .iter()
            .map(|(target, _)| vk::VkImageViewCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
                image: target.image,
                viewType: vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_2D,
                format: vk::VkFormat_VK_FORMAT_R8G8B8A8_UNORM,
                subresourceRange: vk::VkImageSubresourceRange {
                    aspectMask: vk::VkImageAspectFlagBits_VK_IMAGE_ASPECT_COLOR_BIT,
                    levelCount: 1,
                    layerCount: 1,
                    ..Default::default()
                },
                ..Default::default()
            })
            .collect();
        let images: Vec<_> = views
            .iter()
            .map(|view| vk::VkImageDescriptorInfoEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_DESCRIPTOR_INFO_EXT,
                pView: view,
                layout: vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL,
                ..Default::default()
            })
            .collect();
        let descriptors: Vec<_> = images
            .iter()
            .zip(&entries)
            .map(|(image, (_, kind))| vk::VkResourceDescriptorInfoEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RESOURCE_DESCRIPTOR_INFO_EXT,
                type_: if *kind == SAMPLED {
                    vk::VkDescriptorType_VK_DESCRIPTOR_TYPE_SAMPLED_IMAGE
                } else {
                    vk::VkDescriptorType_VK_DESCRIPTOR_TYPE_STORAGE_IMAGE
                },
                data: vk::VkResourceDescriptorDataEXT { pImage: image },
                ..Default::default()
            })
            .collect();
        let destinations: Vec<_> = (0..entries.len())
            .map(|i| resources.destination(i))
            .collect();
        let sampler = vk::VkSamplerCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,
            magFilter: vk::VkFilter_VK_FILTER_NEAREST,
            minFilter: vk::VkFilter_VK_FILTER_NEAREST,
            mipmapMode: vk::VkSamplerMipmapMode_VK_SAMPLER_MIPMAP_MODE_NEAREST,
            addressModeU: vk::VkSamplerAddressMode_VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
            addressModeV: vk::VkSamplerAddressMode_VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
            addressModeW: vk::VkSamplerAddressMode_VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
            maxLod: 0.0,
            ..Default::default()
        };
        // SAFETY: stable local arrays, live images, disjoint writable heap slots.
        unsafe {
            device.result(
                "vkWriteResourceDescriptorsEXT",
                (device.f.vkWriteResourceDescriptorsEXT.unwrap())(
                    device.handle,
                    descriptors.len() as u32,
                    descriptors.as_ptr(),
                    destinations.as_ptr(),
                ),
            )?;
            device.result(
                "vkWriteSamplerDescriptorsEXT",
                (device.f.vkWriteSamplerDescriptorsEXT.unwrap())(
                    device.handle,
                    1,
                    &sampler,
                    &samplers.destination(0),
                ),
            )?;
        }
        resources.buffer.cache(true)?;
        samplers.buffer.cache(true)?;
        Ok(Self {
            device,
            resources,
            samplers,
            _entries: entries,
        })
    }

    pub(super) unsafe fn bind(&self, command: vk::VkCommandBuffer) {
        // The immutable table and its exact reserved ranges live until command-pool
        // destruction; every binding step retains this object and all of its images.
        unsafe {
            (self.device.f.vkCmdBindResourceHeapEXT.unwrap())(command, &self.resources.info);
            (self.device.f.vkCmdBindSamplerHeapEXT.unwrap())(command, &self.samplers.info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heap_alignment_checks_overflow_and_invalid_properties() {
        assert_eq!(align_up(65, 64).unwrap(), 128);
        assert_eq!(align_up(64, 64).unwrap(), 64);
        assert!(align_up(u64::MAX, 64).is_err());
        assert!(align_up(0, 0).is_err());
        assert!(align_up(8, 3).is_err());
    }

    thread_local! {
        static DESTROY: Cell<vk::PFN_vkDestroyBuffer> = const { Cell::new(None) };
        static DESTROYS: Cell<u32> = const { Cell::new(0) };
    }
    unsafe extern "C" fn count_destroy(
        d: vk::VkDevice,
        b: vk::VkBuffer,
        a: *const vk::VkAllocationCallbacks,
    ) {
        DESTROYS.set(DESTROYS.get() + 1);
        unsafe {
            (DESTROY.get().unwrap())(d, b, a);
        }
    }
    unsafe extern "C" fn fail_resource(
        _: vk::VkDevice,
        _: u32,
        _: *const vk::VkResourceDescriptorInfoEXT,
        _: *const vk::VkHostAddressRangeEXT,
    ) -> vk::VkResult {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    }
    unsafe extern "C" fn fail_sampler(
        _: vk::VkDevice,
        _: u32,
        _: *const vk::VkSamplerCreateInfo,
        _: *const vk::VkHostAddressRangeEXT,
    ) -> vk::VkResult {
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY
    }

    #[test]
    #[ignore = "requires modern graphics Vulkan; table retention and descriptor-write cleanup"]
    fn gpu_image_tables() {
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let device = match Device::new_graphics(instance.clone(), physical) {
                Ok(d) => d,
                Err(e) if e.status == UNSUPPORTED => continue,
                Err(e) => panic!("{e:?}"),
            };
            let target = Rc::new(Target::new(device.clone(), 2, 3).unwrap());
            assert!(ImageTable::new(device.clone(), vec![]).is_err());
            assert!(ImageTable::new(device.clone(), vec![(target.clone(), 99)]).is_err());
            let foreign = Device::new_graphics(instance.clone(), physical).unwrap();
            assert!(ImageTable::new(foreign.clone(), vec![(target.clone(), SAMPLED)]).is_err());
            let table = Rc::new(
                ImageTable::new(
                    device.clone(),
                    vec![(target.clone(), SAMPLED), (target.clone(), STORAGE)],
                )
                .unwrap(),
            );
            assert!(Batch::new(foreign.clone())
                .unwrap()
                .bind_images(table.clone())
                .is_err());
            assert!(Batch::new(foreign)
                .unwrap()
                .discard_target(target.clone())
                .is_err());
            let weak = Rc::downgrade(&table);
            let target_weak = Rc::downgrade(&target);
            let mut batch = Batch::new(device.clone()).unwrap();
            batch.bind_images(table.clone()).unwrap();
            batch.discard_target(target.clone()).unwrap();
            drop(target);
            drop(table);
            assert!(weak.upgrade().is_some());
            drop(batch);
            assert!(weak.upgrade().is_none() && target_weak.upgrade().is_none());
            let target = Rc::new(Target::new(device.clone(), 2, 3).unwrap());
            let table =
                Rc::new(ImageTable::new(device.clone(), vec![(target.clone(), SAMPLED)]).unwrap());
            let weak = Rc::downgrade(&table);
            let mut batch = Batch::new(device.clone()).unwrap();
            batch.bind_images(table.clone()).unwrap();
            batch.discard_target(target.clone()).unwrap();
            let mut completion = unsafe { batch.submit().unwrap() };
            assert!(batch.bind_images(table.clone()).is_err());
            assert!(batch.discard_target(target.clone()).is_err());
            drop(table);
            drop(target);
            drop(batch);
            completion.wait().unwrap();
            // Heap reservations remain owned even AFTER wait, until pool destruction.
            assert!(weak.upgrade().is_some());
            drop(completion);
            assert!(weak.upgrade().is_none());
            for sampler_failure in [false, true] {
                let d = Device::create_configured(instance.clone(), physical, true, |f| {
                    DESTROY.set(f.vkDestroyBuffer);
                    f.vkDestroyBuffer = Some(count_destroy);
                    if sampler_failure {
                        f.vkWriteSamplerDescriptorsEXT = Some(fail_sampler);
                    } else {
                        f.vkWriteResourceDescriptorsEXT = Some(fail_resource);
                    }
                })
                .unwrap();
                let target = Rc::new(Target::new(d.clone(), 1, 1).unwrap());
                DESTROYS.set(0);
                let error = ImageTable::new(d, vec![(target, SAMPLED)]).err().unwrap();
                assert_eq!(error.vk, vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY);
                assert_eq!(
                    DESTROYS.get(),
                    2,
                    "both partially built heaps must be freed"
                );
            }
            tested += 1;
        }
        assert!(tested > 0, "No image-capable device");
    }
}
