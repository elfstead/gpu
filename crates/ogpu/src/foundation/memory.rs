//! Explicit backing and non-owning ranges. No staging, suballocator or retention.
use super::*;
use std::cell::Cell;

struct Prepared {
    buffer: vk::VkBufferCreateInfo,
    size: u64,
    alignment: u64,
    dedicated_required: u32,
    dedicated_preferred: u32,
    types: Vec<u32>,
}
pub struct Memory {
    device: *const Device,
    buffer: vk::VkBuffer,
    memory: vk::VkDeviceMemory,
    size: u64,
    allocated: u64,
    prefix: u64,
    address: u64,
    properties: u32,
    mapped: Cell<*mut u8>,
    usage: u64,
    domains: Vec<u32>,
    memory_type: u32,
    dedicated_image: vk::VkImage,
}

fn buffer_usage(usage: u64) -> Result<u32, Status> {
    if usage & !255 != 0 {
        return Err(UNSUPPORTED);
    }
    let bits = [
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_SRC_BIT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFER_DST_BIT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_INDIRECT_BUFFER_BIT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_INDEX_BUFFER_BIT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_VERTEX_BUFFER_BIT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_STORAGE_BUFFER_BIT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_DESCRIPTOR_HEAP_BIT_EXT,
    ];
    Ok(bits.iter().enumerate().fold(
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT,
        |mask, (i, bit)| {
            if usage & (1 << i) != 0 {
                mask | bit
            } else {
                mask
            }
        },
    ))
}

impl Device {
    fn buffer_requirements(
        &self,
        buffer: &vk::VkBufferCreateInfo,
    ) -> (vk::VkMemoryRequirements, vk::VkMemoryDedicatedRequirements) {
        let query = vk::VkDeviceBufferMemoryRequirements {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_BUFFER_MEMORY_REQUIREMENTS,
            pCreateInfo: buffer,
            ..Default::default()
        };
        let mut dedicated = vk::VkMemoryDedicatedRequirements {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_REQUIREMENTS,
            ..Default::default()
        };
        let mut result = vk::VkMemoryRequirements2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_REQUIREMENTS_2,
            pNext: ptr::from_mut(&mut dedicated).cast(),
            ..Default::default()
        };
        // SAFETY: caller has validated description; no temporary GPU object.
        unsafe {
            (self.f.vkGetDeviceBufferMemoryRequirements.unwrap())(self.handle, &query, &mut result);
        }
        (result.memoryRequirements, dedicated)
    }
    unsafe fn prepare_memory(&self, desc: &MemoryDesc) -> Result<Prepared, Status> {
        self.ready()?;
        desc.header.validate::<MemoryDesc>(MEMORY_DESC)?;
        if desc.size == 0
            || !desc.alignment.is_power_of_two()
            || desc.flags != 0
            || !(1..=2).contains(&desc.kind)
        {
            return Err(INVALID);
        }
        let concurrent = desc.concurrent_domain_count;
        if concurrent == 1
            || concurrent as usize > self.snapshot.queues.len()
            || (concurrent != 0 && desc.concurrent_domains.is_null())
        {
            return Err(INVALID);
        }
        if concurrent != 0 {
            // SAFETY: caller provides the described domain array.
            let domains =
                unsafe { std::slice::from_raw_parts(desc.concurrent_domains, concurrent as usize) };
            for (index, domain) in domains.iter().enumerate() {
                if domains[..index].contains(domain) {
                    return Err(INVALID);
                }
                if !self.snapshot.queues.iter().any(|q| q.domain == *domain) {
                    return Err(UNSUPPORTED);
                }
            }
        }
        if desc.kind == 2 && (desc.usage != 0 || desc.alignment != 1 || concurrent != 0) {
            return Err(INVALID);
        }
        let mut buffer = vk::VkBufferCreateInfo::default();
        let mut requirements = vk::VkMemoryRequirements {
            size: desc.size,
            alignment: 1,
            memoryTypeBits: u32::MAX,
        };
        let mut dedicated = vk::VkMemoryDedicatedRequirements {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_REQUIREMENTS,
            ..Default::default()
        };
        if desc.kind == 1 {
            // Ordinary alignments use the native guarantee, with no padding.
            // Only over-alignment needs a second (allocation-free) query.
            let padded = desc.size.checked_add(desc.alignment - 1).ok_or(INVALID)?;
            if desc.size > self.snapshot.memory_limits.max_buffer_size {
                return Err(UNSUPPORTED);
            }
            buffer = vk::VkBufferCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
                size: desc.size,
                usage: buffer_usage(desc.usage)?,
                sharingMode: if concurrent == 0 {
                    vk::VkSharingMode_VK_SHARING_MODE_EXCLUSIVE
                } else {
                    vk::VkSharingMode_VK_SHARING_MODE_CONCURRENT
                },
                queueFamilyIndexCount: concurrent,
                pQueueFamilyIndices: if concurrent == 0 {
                    ptr::null()
                } else {
                    desc.concurrent_domains
                },
                ..Default::default()
            };
            (requirements, dedicated) = self.buffer_requirements(&buffer);
            if desc.alignment > requirements.alignment {
                if padded > self.snapshot.memory_limits.max_buffer_size {
                    return Err(UNSUPPORTED);
                }
                buffer.size = padded;
                (requirements, dedicated) = self.buffer_requirements(&buffer);
            }
        }
        if requirements.size == 0
            || requirements.alignment == 0
            || requirements.size > self.snapshot.memory_limits.max_allocation_size
        {
            return Err(UNSUPPORTED);
        }
        let types: Vec<_> = self
            .snapshot
            .memory_types
            .iter()
            .filter(|m| {
                // Protected / AMD device-coherent/uncached enabling isn't part of
                // this implemented profile. Physical query records still expose it.
                m.properties & (32 | 64 | 128) == 0
                    && requirements.memoryTypeBits & (1u32 << m.id) != 0
            })
            .map(|m| m.id)
            .collect();
        if types.is_empty() {
            return Err(UNSUPPORTED);
        }
        Ok(Prepared {
            buffer,
            size: requirements.size,
            alignment: requirements.alignment.max(desc.alignment),
            dedicated_required: dedicated.requiresDedicatedAllocation,
            dedicated_preferred: dedicated.prefersDedicatedAllocation,
            types,
        })
    }
    pub(in crate::foundation) unsafe fn memory_requirements(
        &self,
        desc: &MemoryDesc,
        out: &mut Requirements,
    ) -> Result<(), Status> {
        if out.compatible_type_capacity != 0 && out.compatible_types.is_null() {
            return Err(INVALID);
        }
        let p = unsafe { self.prepare_memory(desc)? };
        let capacity = out.compatible_type_capacity;
        out.size = p.size;
        out.alignment = p.alignment;
        out.dedicated_required = p.dedicated_required;
        out.dedicated_preferred = p.dedicated_preferred;
        out.compatible_type_count = p.types.len() as u32;
        if capacity == 0 {
            return Ok(());
        }
        if capacity < out.compatible_type_count {
            return Err(CAPACITY);
        }
        // SAFETY: caller guarantees writable array for capacity elements.
        unsafe {
            ptr::copy_nonoverlapping(p.types.as_ptr(), out.compatible_types, p.types.len());
        }
        Ok(())
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        // SAFETY: caller has retired all uses and kept the device alive. Partial
        // construction adopts only successful native outputs; no implicit wait.
        unsafe {
            let d = &*self.device;
            self.unmap();
            if !self.buffer.is_null() {
                (d.f.vkDestroyBuffer.unwrap())(d.handle, self.buffer, ptr::null());
            }
            if !self.memory.is_null() {
                (d.f.vkFreeMemory.unwrap())(d.handle, self.memory, ptr::null());
            }
        }
    }
}
impl Memory {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &MemoryDesc,
    ) -> Result<Box<Self>, Status> {
        let p = unsafe { d.prepare_memory(desc)? };
        if !p.types.contains(&desc.memory_type) {
            return Err(UNSUPPORTED);
        }
        let properties = d
            .snapshot
            .memory_types
            .iter()
            .find(|m| m.id == desc.memory_type)
            .ok_or(INVALID)?
            .properties;
        let mut result = Box::new(Self {
            device: d,
            buffer: ptr::null_mut(),
            memory: ptr::null_mut(),
            size: desc.size,
            allocated: p.size,
            prefix: 0,
            address: 0,
            properties,
            mapped: Cell::new(ptr::null_mut()),
            usage: desc.usage,
            memory_type: desc.memory_type,
            dedicated_image: ptr::null_mut(),
            domains: if desc.concurrent_domain_count == 0 {
                Vec::new()
            } else {
                unsafe {
                    std::slice::from_raw_parts(
                        desc.concurrent_domains,
                        desc.concurrent_domain_count as usize,
                    )
                }
                .to_vec()
            },
        });
        if desc.kind == 1 {
            let mut buffer = ptr::null_mut();
            unsafe {
                d.result((d.f.vkCreateBuffer.unwrap())(
                    d.handle,
                    &p.buffer,
                    ptr::null(),
                    &mut buffer,
                ))?;
            }
            result.buffer = buffer;
        }
        let dedicated = vk::VkMemoryDedicatedAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
            buffer: result.buffer,
            ..Default::default()
        };
        let flags = vk::VkMemoryAllocateFlagsInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO,
            pNext: ptr::from_ref(&dedicated).cast(),
            flags: vk::VkMemoryAllocateFlagBits_VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT,
            ..Default::default()
        };
        let allocate = vk::VkMemoryAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
            pNext: if desc.kind == 1 {
                ptr::from_ref(&flags).cast()
            } else {
                ptr::null()
            },
            allocationSize: p.size,
            memoryTypeIndex: desc.memory_type,
        };
        let mut memory = ptr::null_mut();
        unsafe {
            d.result((d.f.vkAllocateMemory.unwrap())(
                d.handle,
                &allocate,
                ptr::null(),
                &mut memory,
            ))?;
        }
        result.memory = memory;
        if desc.kind == 1 {
            unsafe {
                d.result((d.f.vkBindBufferMemory.unwrap())(
                    d.handle,
                    result.buffer,
                    memory,
                    0,
                ))?;
            }
            let address = vk::VkBufferDeviceAddressInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_DEVICE_ADDRESS_INFO,
                buffer: result.buffer,
                ..Default::default()
            };
            let base = unsafe { (d.f.vkGetBufferDeviceAddress.unwrap())(d.handle, &address) };
            if base == 0 {
                return Err(BACKEND_ERROR);
            }
            result.prefix = (desc.alignment - base % desc.alignment) % desc.alignment;
            result.address = base.checked_add(result.prefix).ok_or(BACKEND_ERROR)?;
            result.address.checked_add(desc.size).ok_or(BACKEND_ERROR)?;
        }
        Ok(result)
    }
    fn range(&self, offset: u64, size: u64) -> Result<u64, Status> {
        if offset > self.size || size > self.size - offset {
            return Err(INVALID);
        }
        self.prefix.checked_add(offset).ok_or(INVALID)
    }
    pub(super) fn image_placement(
        &self,
        device: *const Device,
        image: vk::VkImage,
        requirements: vk::VkMemoryRequirements,
        dedicated_required: bool,
        offset: u64,
        size: u64,
    ) -> Result<vk::VkDeviceMemory, Status> {
        if self.device != device || !self.buffer.is_null() {
            return Err(INVALID);
        }
        self.range(offset, size)?;
        if size < requirements.size || offset % requirements.alignment != 0 {
            return Err(INVALID);
        }
        if requirements.memoryTypeBits & (1u32 << self.memory_type) == 0 {
            return Err(UNSUPPORTED);
        }
        if (!self.dedicated_image.is_null() && (self.dedicated_image != image || offset != 0))
            || (dedicated_required && self.dedicated_image != image)
        {
            return Err(INVALID);
        }
        Ok(self.memory)
    }
    pub(super) fn dedicated_image(
        d: &Device,
        image: vk::VkImage,
        size: u64,
        memory_type: u32,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        let properties = d
            .snapshot
            .memory_types
            .iter()
            .find(|m| m.id == memory_type && m.properties & (32 | 64 | 128) == 0)
            .ok_or(UNSUPPORTED)?
            .properties;
        let mut result = Box::new(Self {
            device: d,
            buffer: ptr::null_mut(),
            memory: ptr::null_mut(),
            size,
            allocated: size,
            prefix: 0,
            address: 0,
            properties,
            mapped: Cell::new(ptr::null_mut()),
            usage: 0,
            domains: Vec::new(),
            memory_type,
            dedicated_image: image,
        });
        let dedicated = vk::VkMemoryDedicatedAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
            image,
            ..Default::default()
        };
        let info = vk::VkMemoryAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
            pNext: ptr::from_ref(&dedicated).cast(),
            allocationSize: size,
            memoryTypeIndex: memory_type,
        };
        let mut memory = ptr::null_mut();
        unsafe {
            d.result((d.f.vkAllocateMemory.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut memory,
            ))?;
        }
        result.memory = memory;
        Ok(result)
    }
    pub(super) fn command_range(
        &self,
        device: *const Device,
        domain: u32,
        offset: u64,
        size: u64,
        usage: u64,
    ) -> Result<(vk::VkBuffer, u64, u64), Status> {
        if !self.domains.is_empty() && !self.domains.contains(&domain) {
            return Err(INVALID);
        }
        self.descriptor_range(device, offset, size, usage)
    }
    pub(super) fn descriptor_range(
        &self,
        device: *const Device,
        offset: u64,
        size: u64,
        usage: u64,
    ) -> Result<(vk::VkBuffer, u64, u64), Status> {
        if self.device != device || self.buffer.is_null() || self.usage & usage != usage {
            return Err(INVALID);
        }
        Ok((
            self.buffer,
            self.range(offset, size)?,
            self.address.checked_add(offset).ok_or(INVALID)?,
        ))
    }
    pub(super) fn concurrent(&self) -> bool {
        !self.domains.is_empty()
    }
    pub(super) fn address_flags(&self) -> u32 {
        vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR
            | if self.usage & 64 != 0 {
                vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_STORAGE_BUFFER_USAGE_BIT_KHR
            } else {
                0
            }
    }
    pub(in crate::foundation) fn address(&self, offset: u64, size: u64) -> Result<u64, Status> {
        unsafe { &*self.device }.ready()?;
        self.range(offset, size)?;
        if self.address == 0 {
            return Err(UNSUPPORTED);
        }
        self.address.checked_add(offset).ok_or(INVALID)
    }
    pub(in crate::foundation) fn map(&self, offset: u64, size: u64) -> Result<Mapping, Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        let offset = self.range(offset, size)?;
        if size == 0 {
            return Err(INVALID);
        }
        if self.properties & 2 == 0 || self.allocated > isize::MAX as u64 {
            return Err(UNSUPPORTED);
        }
        if self.mapped.get().is_null() {
            let mut pointer = ptr::null_mut();
            unsafe {
                d.result((d.f.vkMapMemory.unwrap())(
                    d.handle,
                    self.memory,
                    0,
                    self.allocated,
                    0,
                    &mut pointer,
                ))?;
            }
            self.mapped.set(pointer.cast());
        }
        let coherent = self.properties & 4 != 0;
        let atom = if coherent {
            1
        } else {
            d.snapshot.memory_limits.cache_atom_size
        };
        Ok(Mapping {
            // SAFETY: validated range in a mapped allocation <= isize::MAX.
            data: unsafe { self.mapped.get().add(offset as usize) }.cast(),
            size,
            cache_atom_size: atom,
            cache_offset: offset % atom,
            coherent: u32::from(coherent),
            reserved: 0,
        })
    }
    pub(in crate::foundation) fn unmap(&self) {
        if !self.mapped.get().is_null() {
            let d = unsafe { &*self.device };
            unsafe {
                (d.f.vkUnmapMemory.unwrap())(d.handle, self.memory);
            }
            self.mapped.set(ptr::null_mut());
        }
    }
    pub(in crate::foundation) fn cache(
        &self,
        offset: u64,
        size: u64,
        flush: bool,
    ) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        let offset = self.range(offset, size)?;
        if self.mapped.get().is_null() {
            return Err(INVALID);
        }
        if size == 0 || self.properties & 4 != 0 {
            return Ok(());
        }
        let (start, length) = cache_range(
            offset,
            size,
            self.allocated,
            d.snapshot.memory_limits.cache_atom_size,
        )?;
        let range = vk::VkMappedMemoryRange {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
            memory: self.memory,
            offset: start,
            size: length,
            ..Default::default()
        };
        let f = if flush {
            d.f.vkFlushMappedMemoryRanges
        } else {
            d.f.vkInvalidateMappedMemoryRanges
        }
        .unwrap();
        unsafe { d.result(f(d.handle, 1, &range)) }
    }
}

fn cache_range(offset: u64, size: u64, allocated: u64, atom: u64) -> Result<(u64, u64), Status> {
    if atom == 0 || size == 0 || offset >= allocated || size > allocated - offset {
        return Err(INVALID);
    }
    let start = offset - offset % atom;
    let end = offset + size;
    let end = if end % atom == 0 {
        end
    } else {
        end + (atom - end % atom).min(allocated - end)
    };
    Ok((start, end - start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::*;
    thread_local! {
        static ALLOCATIONS: Cell<u32> = const { Cell::new(0) };
        static DESTROYS: Cell<u32> = const { Cell::new(0) };
        static FREES: Cell<u32> = const { Cell::new(0) };
        static DESTROY: Cell<vk::PFN_vkDestroyBuffer> = const { Cell::new(None) };
        static CACHE: Cell<(u64, u64)> = const { Cell::new((0, 0)) };
    }
    unsafe extern "C" fn fail_allocate(
        _: vk::VkDevice,
        _: *const vk::VkMemoryAllocateInfo,
        _: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkDeviceMemory,
    ) -> vk::VkResult {
        ALLOCATIONS.set(ALLOCATIONS.get() + 1);
        unsafe {
            out.write(ptr::dangling_mut());
        }
        vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY
    }
    unsafe extern "C" fn destroy(
        d: vk::VkDevice,
        b: vk::VkBuffer,
        a: *const vk::VkAllocationCallbacks,
    ) {
        DESTROYS.set(DESTROYS.get() + 1);
        unsafe {
            DESTROY.get().unwrap()(d, b, a);
        }
    }
    unsafe extern "C" fn free(
        _: vk::VkDevice,
        _: vk::VkDeviceMemory,
        _: *const vk::VkAllocationCallbacks,
    ) {
        FREES.set(FREES.get() + 1);
    }
    unsafe extern "C" fn cache(
        _: vk::VkDevice,
        count: u32,
        ranges: *const vk::VkMappedMemoryRange,
    ) -> vk::VkResult {
        assert_eq!(count, 1);
        let r = unsafe { &*ranges };
        CACHE.set((r.offset, r.size));
        vk::VkResult_VK_SUCCESS
    }
    #[test]
    #[ignore = "requires modern Vulkan; explicit backing, requirements, ranges and allocation cleanup"]
    fn gpu_foundation_memory() {
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
                .find(|q| q.count != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let mut d = Device::create(
                &adapter,
                &[QueueRequest {
                    domain,
                    count: 1,
                    priority: 0.5,
                }],
                0,
            )
            .unwrap();
            let mut desc = MemoryDesc {
                header: Record::new::<MemoryDesc>(MEMORY_DESC),
                size: 1027,
                alignment: 65536,
                usage: 3,
                memory_type: u32::MAX,
                kind: 1,
                concurrent_domains: ptr::null(),
                concurrent_domain_count: 0,
                flags: 0,
            };
            unsafe {
                let mut req = Requirements::default();
                assert_eq!(ogpu_next_memory_requirements(&*d, &desc, &mut req), OK);
                assert!(req.size >= desc.size && req.alignment >= desc.alignment);
                let mut compatible = vec![0; req.compatible_type_count as usize];
                req.compatible_type_capacity = compatible.len() as u32;
                req.compatible_types = compatible.as_mut_ptr();
                assert_eq!(ogpu_next_memory_requirements(&*d, &desc, &mut req), OK);
                if compatible.len() > 1 {
                    let mut unchanged = u32::MAX;
                    let mut short = Requirements {
                        compatible_type_capacity: 1,
                        compatible_types: &mut unchanged,
                        ..Default::default()
                    };
                    assert_eq!(
                        ogpu_next_memory_requirements(&*d, &desc, &mut short),
                        CAPACITY
                    );
                    assert_eq!(short.compatible_type_count, compatible.len() as u32);
                    assert_eq!(unchanged, u32::MAX);
                }
                desc.memory_type = *compatible
                    .iter()
                    .find(|id| {
                        d.snapshot
                            .memory_types
                            .iter()
                            .any(|t| t.id == **id && t.properties & 2 != 0)
                    })
                    .unwrap();
                for bad in [
                    MemoryDesc { size: 0, ..desc },
                    MemoryDesc {
                        alignment: 3,
                        ..desc
                    },
                    MemoryDesc {
                        size: u64::MAX,
                        ..desc
                    },
                    MemoryDesc { flags: 1, ..desc },
                    MemoryDesc {
                        concurrent_domain_count: 1,
                        ..desc
                    },
                ] {
                    let mut out = ptr::dangling_mut();
                    assert_eq!(ogpu_next_memory_create(&mut *d, &bad, &mut out), INVALID);
                    assert!(out.is_null());
                }
                let mut out = ptr::dangling_mut();
                assert_eq!(
                    ogpu_next_memory_create(
                        &mut *d,
                        &MemoryDesc {
                            memory_type: u32::MAX,
                            ..desc
                        },
                        &mut out
                    ),
                    UNSUPPORTED
                );
                assert!(out.is_null());

                let alloc = d.f.vkAllocateMemory;
                let free_native = d.f.vkFreeMemory;
                DESTROY.set(d.f.vkDestroyBuffer);
                d.f.vkAllocateMemory = Some(fail_allocate);
                d.f.vkFreeMemory = Some(free);
                d.f.vkDestroyBuffer = Some(destroy);
                ALLOCATIONS.set(0);
                DESTROYS.set(0);
                FREES.set(0);
                assert_eq!(ogpu_next_memory_requirements(&*d, &desc, &mut req), OK);
                assert_eq!(ALLOCATIONS.get(), 0);
                assert_eq!(
                    ogpu_next_memory_create(&mut *d, &desc, &mut out),
                    OUT_OF_MEMORY
                );
                assert!(out.is_null());
                assert_eq!((ALLOCATIONS.get(), DESTROYS.get(), FREES.get()), (1, 1, 0));
                d.f.vkAllocateMemory = alloc;
                d.f.vkFreeMemory = free_native;
                d.f.vkDestroyBuffer = DESTROY.get();

                let flush = d.f.vkFlushMappedMemoryRanges;
                let invalidate = d.f.vkInvalidateMappedMemoryRanges;
                d.f.vkFlushMappedMemoryRanges = Some(cache);
                d.f.vkInvalidateMappedMemoryRanges = Some(cache);
                let mut memory = Memory::create(&d, &desc).unwrap();
                assert_eq!(memory.address(0, desc.size).unwrap() % desc.alignment, 0);
                assert_eq!(memory.address(u64::MAX, 1), Err(INVALID));
                assert_eq!(memory.address(desc.size, 1), Err(INVALID));
                assert!(memory.address(desc.size, 0).is_ok());
                assert_eq!(memory.cache(0, 1, true), Err(INVALID));
                let view = memory.map(3, 259).unwrap();
                let all = memory.map(0, desc.size).unwrap();
                assert_eq!(view.data, all.data.cast::<u8>().add(3).cast());
                // Model noncoherent native calls on real backing, without passing
                // fabricated memory properties to the actual driver.
                memory.properties &= !4;
                let atom = d.snapshot.memory_limits.cache_atom_size;
                let expected = cache_range(memory.prefix + 3, 259, memory.allocated, atom).unwrap();
                memory.cache(3, 259, true).unwrap();
                assert_eq!(CACHE.get(), expected);
                memory.cache(3, 259, false).unwrap();
                assert_eq!(CACHE.get(), expected);
                assert_eq!(
                    memory.map(3, 259).unwrap().cache_offset,
                    (memory.prefix + 3) % atom
                );
                memory.unmap();
                memory.unmap();
                drop(memory);
                d.f.vkFlushMappedMemoryRanges = flush;
                d.f.vkInvalidateMappedMemoryRanges = invalidate;

                let opaque_desc = MemoryDesc {
                    size: 4096,
                    alignment: 1,
                    usage: 0,
                    kind: 2,
                    ..desc
                };
                let opaque = Memory::create(&d, &opaque_desc).unwrap();
                assert_eq!(opaque.address(0, 1), Err(UNSUPPORTED));
                assert!(opaque.map(0, 4096).is_ok());
                drop(opaque);
                let domains: Vec<_> = d.snapshot.queues.iter().map(|q| q.domain).collect();
                if domains.len() >= 2 {
                    let concurrent_desc = MemoryDesc {
                        concurrent_domains: domains.as_ptr(),
                        concurrent_domain_count: 2,
                        ..desc
                    };
                    drop(Memory::create(&d, &concurrent_desc).unwrap());
                    let duplicate = [domains[0], domains[0]];
                    let invalid = MemoryDesc {
                        concurrent_domains: duplicate.as_ptr(),
                        ..concurrent_desc
                    };
                    assert_eq!(
                        ogpu_next_memory_create(&mut *d, &invalid, &mut out),
                        INVALID
                    );
                }
            }
            tested += 1;
        }
        assert!(tested != 0, "No suitable adapter; not a passing skip");
    }
    #[test]
    fn cache_expansion_is_bounded_and_overflow_safe() {
        assert_eq!(cache_range(259, 3, 1024, 256), Ok((256, 256)));
        assert_eq!(cache_range(259, 5, 264, 256), Ok((256, 8)));
        assert_eq!(
            cache_range(u64::MAX - 1, 1, u64::MAX, 256),
            Ok((u64::MAX - 255, 255))
        );
        assert_eq!(cache_range(u64::MAX - 1, 2, u64::MAX, 256), Err(INVALID));
        assert_eq!(cache_range(0, 1, 16, 0), Err(INVALID));
    }
    #[test]
    fn buffer_usage_never_silently_drops_bits() {
        assert_eq!(buffer_usage(256), Err(UNSUPPORTED));
        assert_eq!(
            buffer_usage(0),
            Ok(vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_SHADER_DEVICE_ADDRESS_BIT)
        );
        assert_ne!(
            buffer_usage(8).unwrap() & vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_INDEX_BUFFER_BIT,
            0
        );
    }
}
