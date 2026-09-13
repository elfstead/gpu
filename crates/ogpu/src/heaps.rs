//! Independent checked descriptor heaps. Mutation requires exclusive ownership.
use super::*;

pub(crate) const SAMPLED: u32 = 0;
pub(crate) const STORAGE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SamplerDesc {
    pub min_filter: u32,
    pub mag_filter: u32,
    pub address_u: u32,
    pub address_v: u32,
}

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
    capacity: usize,
    poisoned: bool,
}

impl Heap {
    fn new(device: Rc<Device>, count: u32, sampler: bool) -> Result<Self, Error> {
        device.ready()?;
        if !device.graphics {
            return Err(Error::new(
                UNSUPPORTED,
                "Descriptor heaps currently require the graphics image profile",
            ));
        }
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
        // Validate alignment and reserve enough padding to align the device address.
        align_up(0, alignment)?;
        let backing_size = size
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
            capacity: count as usize,
            poisoned: false,
            info: vk::VkBindHeapInfoEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_HEAP_INFO_EXT,
                heapRange: vk::VkDeviceAddressRangeEXT { address, size },
                reservedRangeOffset: reserved_offset,
                reservedRangeSize: reserved,
                ..Default::default()
            },
        })
    }

    fn ready(&self) -> Result<(), Error> {
        self.buffer.device.ready()?;
        if self.poisoned {
            Err(Error::new(
                INVALID_ARGUMENT,
                "Descriptor heap is poisoned after a flush failure; destroy it",
            ))
        } else {
            Ok(())
        }
    }

    fn range(&self, first: u32, count: usize) -> Result<std::ops::Range<usize>, Error> {
        self.ready()?;
        let first = first as usize;
        if first > self.capacity || count > self.capacity - first {
            Err(Error::new(
                OUT_OF_RANGE,
                "Descriptor range exceeds heap capacity",
            ))
        } else {
            Ok(first..first + count)
        }
    }

    fn staging(&self, count: usize) -> Vec<u8> {
        vec![0; count * self.stride]
    }

    fn destinations(&self, bytes: &mut [u8]) -> Vec<vk::VkHostAddressRangeEXT> {
        bytes
            .chunks_exact_mut(self.stride)
            .map(|slot| vk::VkHostAddressRangeEXT {
                address: slot.as_mut_ptr().cast(),
                size: slot.len(),
            })
            .collect()
    }

    fn commit(&mut self, first: u32, bytes: &[u8]) -> Result<(), Error> {
        if bytes.is_empty() {
            return Ok(());
        }
        // No recorded references exist: the public boundary obtains &mut through
        // Rc::get_mut. All native pools bound to our reservation have been destroyed.
        // Generate descriptors separately so a driver error cannot corrupt live slots.
        unsafe {
            ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.buffer
                    .mapped
                    .add(self.offset + first as usize * self.stride),
                bytes.len(),
            );
        }
        if let Err(e) = self.buffer.cache(true) {
            self.poisoned = true;
            return Err(e);
        }
        Ok(())
    }
}

pub(crate) struct ImageHeap {
    pub(super) device: Rc<Device>,
    heap: Heap,
    entries: Vec<Option<Rc<Image>>>,
}

impl ImageHeap {
    pub(crate) fn new(device: Rc<Device>, capacity: u32) -> Result<Self, Error> {
        let heap = Heap::new(device.clone(), capacity, false)?;
        Ok(Self {
            device,
            heap,
            entries: vec![None; capacity as usize],
        })
    }
    pub(crate) fn ready(&self) -> Result<(), Error> {
        self.heap.ready()
    }

    pub(crate) fn write(
        &mut self,
        first: u32,
        entries: Vec<(Rc<Image>, u32)>,
    ) -> Result<(), Error> {
        let range = self.heap.range(first, entries.len())?;
        for (target, kind) in &entries {
            let usage = match *kind {
                SAMPLED => graphics::SAMPLED,
                STORAGE => graphics::STORAGE,
                _ => 0,
            };
            if !Rc::ptr_eq(&self.device, &target.device)
                || usage == 0
                || target.desc.usage & usage == 0
            {
                return Err(Error::new(
                    INVALID_ARGUMENT,
                    "Invalid image-heap device, descriptor kind or image usage",
                ));
            }
        }
        if entries.is_empty() {
            return Ok(());
        }
        let views: Vec<_> = entries
            .iter()
            .map(|(target, _)| vk::VkImageViewCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
                image: target.image,
                viewType: target.desc.view_type(),
                format: target.desc.vk_format(),
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
        let mut bytes = self.heap.staging(entries.len());
        let destinations = self.heap.destinations(&mut bytes);
        // SAFETY: stable input arrays, live images and independent staging slots.
        self.device
            .result("vkWriteResourceDescriptorsEXT", unsafe {
                (self.device.f.vkWriteResourceDescriptorsEXT.unwrap())(
                    self.device.handle,
                    descriptors.len() as u32,
                    descriptors.as_ptr(),
                    destinations.as_ptr(),
                )
            })?;
        // Whole-allocation maintenance must preserve bytes outside this edit,
        // including driver-written reservation bytes from retired command pools.
        self.heap.buffer.cache(false)?;
        // Replace ownership before committing bytes. On flush failure the heap is
        // terminal, but any descriptors now present still have live backing images.
        for (slot, (target, _)) in self.entries[range].iter_mut().zip(entries) {
            *slot = Some(target);
        }
        self.heap.commit(first, &bytes)
    }

    pub(crate) fn clear(&mut self, first: u32, count: u32) -> Result<(), Error> {
        let range = self.heap.range(first, count as usize)?;
        // Cleared slots are invalid, not null descriptors. No GPU access is allowed;
        // leave opaque bytes alone and release the explicitly retained allocations.
        self.entries[range].fill(None);
        Ok(())
    }

    pub(super) unsafe fn bind(&self, command: vk::VkCommandBuffer) {
        unsafe {
            (self.device.f.vkCmdBindResourceHeapEXT.unwrap())(command, &self.heap.info);
        }
    }
}

pub(crate) struct SamplerHeap {
    pub(super) device: Rc<Device>,
    heap: Heap,
}

impl SamplerHeap {
    pub(crate) fn new(device: Rc<Device>, capacity: u32) -> Result<Self, Error> {
        Ok(Self {
            heap: Heap::new(device.clone(), capacity, true)?,
            device,
        })
    }
    pub(crate) fn ready(&self) -> Result<(), Error> {
        self.heap.ready()
    }

    pub(crate) fn write(&mut self, first: u32, entries: &[SamplerDesc]) -> Result<(), Error> {
        self.heap.range(first, entries.len())?;
        for s in entries {
            if [s.min_filter, s.mag_filter, s.address_u, s.address_v]
                .iter()
                .any(|&v| v > 1)
            {
                return Err(Error::new(
                    INVALID_ARGUMENT,
                    "Invalid sampler filter/address mode",
                ));
            }
        }
        if entries.is_empty() {
            return Ok(());
        }
        let address = |v| {
            if v == 0 {
                vk::VkSamplerAddressMode_VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE
            } else {
                vk::VkSamplerAddressMode_VK_SAMPLER_ADDRESS_MODE_REPEAT
            }
        };
        let samplers: Vec<_> = entries
            .iter()
            .map(|s| vk::VkSamplerCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO,
                magFilter: s.mag_filter as vk::VkFilter,
                minFilter: s.min_filter as vk::VkFilter,
                mipmapMode: vk::VkSamplerMipmapMode_VK_SAMPLER_MIPMAP_MODE_NEAREST,
                addressModeU: address(s.address_u),
                addressModeV: address(s.address_v),
                addressModeW: vk::VkSamplerAddressMode_VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE,
                maxLod: 0.0,
                ..Default::default()
            })
            .collect();
        let mut bytes = self.heap.staging(entries.len());
        let destinations = self.heap.destinations(&mut bytes);
        self.device.result("vkWriteSamplerDescriptorsEXT", unsafe {
            (self.device.f.vkWriteSamplerDescriptorsEXT.unwrap())(
                self.device.handle,
                samplers.len() as u32,
                samplers.as_ptr(),
                destinations.as_ptr(),
            )
        })?;
        self.heap.buffer.cache(false)?;
        self.heap.commit(first, &bytes)
    }
    pub(super) unsafe fn bind(&self, command: vk::VkCommandBuffer) {
        unsafe {
            (self.device.f.vkCmdBindSamplerHeapEXT.unwrap())(command, &self.heap.info);
        }
    }
}

#[cfg(test)]
#[path = "heap_tests.rs"]
mod tests;
