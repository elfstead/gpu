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
    buffer_offset: u64,
    buffer_size: u64,
    ownership: u32,
    address: u64,
    properties: u32,
    mapped: Cell<*mut u8>,
    usage: u64,
    domains: Vec<u32>,
    memory_type: u32,
    dedicated_image: vk::VkImage,
    allow_images: bool,
    kind: u32,
    flags: u32,
    page_size: u64,
    page_types: u32,
    sparse_size: u64,
}

fn buffer_usage(usage: u64) -> Result<u32, Status> {
    if usage & !16383 != 0 {
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
        vk::VK_BUFFER_USAGE_2_PREPROCESS_BUFFER_BIT_EXT as u32,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFORM_FEEDBACK_BUFFER_BIT_EXT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_TRANSFORM_FEEDBACK_COUNTER_BUFFER_BIT_EXT,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_BIT_KHR,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_ACCELERATION_STRUCTURE_STORAGE_BIT_KHR,
        vk::VkBufferUsageFlagBits_VK_BUFFER_USAGE_SHADER_BINDING_TABLE_BIT_KHR,
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
        let mut buffer = *buffer;
        let usage = vk::VkBufferUsageFlags2CreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_USAGE_FLAGS_2_CREATE_INFO,
            usage: u64::from(buffer.usage),
            ..Default::default()
        };
        if usage.usage & vk::VK_BUFFER_USAGE_2_PREPROCESS_BUFFER_BIT_EXT != 0 {
            buffer.pNext = ptr::from_ref(&usage).cast();
            buffer.usage = 0;
        }
        let query = vk::VkDeviceBufferMemoryRequirements {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_BUFFER_MEMORY_REQUIREMENTS,
            pCreateInfo: &buffer,
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
    unsafe fn validate_memory(&self, desc: &MemoryDesc) -> Result<(), Status> {
        self.ready()?;
        desc.header.validate::<MemoryDesc>(MEMORY_DESC)?;
        if desc.usage & USAGE_RAY_TABLE != 0 && self.snapshot.features.enabled & RAY_PIPELINE == 0 {
            return Err(UNSUPPORTED);
        }
        if desc.usage & (ACCELERATION_INPUT_USAGE | ACCELERATION_STORAGE_USAGE) != 0
            && self.snapshot.features.enabled & ACCELERATION_STRUCTURE == 0
        {
            return Err(UNSUPPORTED);
        }
        if desc.usage & (CAPTURE_USAGE | CAPTURE_COUNTER_USAGE) != 0
            && self.snapshot.features.enabled & TRANSFORM_FEEDBACK == 0
        {
            return Err(UNSUPPORTED);
        }
        if desc.usage & PREPROCESS_USAGE != 0
            && self.snapshot.features.enabled & GENERATED_COMMANDS == 0
        {
            return Err(UNSUPPORTED);
        }
        if desc.size == 0 || !desc.alignment.is_power_of_two() || !(1..=3).contains(&desc.kind) {
            return Err(INVALID);
        }
        let allowed = match desc.kind {
            MEMORY_LINEAR => MEMORY_ALLOW_IMAGES,
            MEMORY_OPAQUE => MEMORY_ADDRESSABLE,
            MEMORY_VIRTUAL => MEMORY_SPARSE_RESIDENCY | MEMORY_SPARSE_ALIAS | MEMORY_FULLY_BOUND,
            _ => return Err(INVALID),
        };
        if desc.flags & !allowed != 0 {
            return Err(INVALID);
        }
        if desc.kind == MEMORY_VIRTUAL {
            if desc.usage & 128 != 0 && self.snapshot.sparse_limits.descriptor_heaps == 0 {
                return Err(UNSUPPORTED);
            }
            let mut required = SPARSE_BINDING;
            if desc.flags & MEMORY_SPARSE_RESIDENCY != 0 {
                required |= SPARSE_BUFFER;
            }
            if desc.flags & MEMORY_SPARSE_ALIAS != 0 {
                required |= SPARSE_ALIAS;
            }
            if required & !self.snapshot.features.enabled != 0 {
                return Err(UNSUPPORTED);
            }
            if desc.size > self.snapshot.sparse_limits.address_space_size {
                return Err(UNSUPPORTED);
            }
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
        if desc.kind != MEMORY_OPAQUE {
            buffer_usage(desc.usage)?;
        }
        Ok(())
    }
    unsafe fn prepare_memory(&self, desc: &MemoryDesc) -> Result<Prepared, Status> {
        unsafe {
            self.validate_memory(desc)?;
        }
        let concurrent = desc.concurrent_domain_count;
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
        if desc.kind != MEMORY_OPAQUE {
            // Ordinary alignments use the native guarantee, with no padding.
            // Only over-alignment needs a second (allocation-free) query.
            let padded = desc.size.checked_add(desc.alignment - 1).ok_or(INVALID)?;
            if desc.size > self.snapshot.memory_limits.max_buffer_size {
                return Err(UNSUPPORTED);
            }
            buffer = vk::VkBufferCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,
                flags: if desc.kind == MEMORY_VIRTUAL {
                    vk::VkBufferCreateFlagBits_VK_BUFFER_CREATE_SPARSE_BINDING_BIT
                        | if desc.flags & MEMORY_SPARSE_RESIDENCY != 0 {
                            vk::VkBufferCreateFlagBits_VK_BUFFER_CREATE_SPARSE_RESIDENCY_BIT
                        } else {
                            0
                        }
                        | if desc.flags & MEMORY_SPARSE_ALIAS != 0 {
                            vk::VkBufferCreateFlagBits_VK_BUFFER_CREATE_SPARSE_ALIASED_BIT
                        } else {
                            0
                        }
                } else {
                    0
                },
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
            if desc.kind == MEMORY_VIRTUAL
                && (desc.alignment > requirements.alignment
                    || dedicated.requiresDedicatedAllocation != 0)
            {
                return Err(UNSUPPORTED);
            }
            if desc.alignment > requirements.alignment {
                if padded > self.snapshot.memory_limits.max_buffer_size {
                    return Err(UNSUPPORTED);
                }
                buffer.size = padded;
                (requirements, dedicated) = self.buffer_requirements(&buffer);
            }
        }
        if desc.flags & MEMORY_ALLOW_IMAGES != 0 && dedicated.requiresDedicatedAllocation != 0 {
            return Err(UNSUPPORTED);
        }
        if requirements.size == 0
            || requirements.alignment == 0
            || (desc.kind != MEMORY_VIRTUAL
                && requirements.size > self.snapshot.memory_limits.max_allocation_size)
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
            if self.ownership & NATIVE_OWN_BUFFER != 0 && !self.buffer.is_null() {
                (d.f.vkDestroyBuffer.unwrap())(d.handle, self.buffer, ptr::null());
            }
            if self.ownership & NATIVE_OWN != 0 && !self.memory.is_null() {
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
        if desc.kind != MEMORY_VIRTUAL && !p.types.contains(&desc.memory_type) {
            return Err(UNSUPPORTED);
        }
        let properties = if desc.kind == MEMORY_VIRTUAL {
            0
        } else {
            d.snapshot
                .memory_types
                .iter()
                .find(|m| m.id == desc.memory_type)
                .ok_or(INVALID)?
                .properties
        };
        let mut result = Box::new(Self {
            device: d,
            buffer: ptr::null_mut(),
            memory: ptr::null_mut(),
            size: desc.size,
            allocated: if desc.kind == MEMORY_VIRTUAL {
                0
            } else {
                p.size
            },
            prefix: 0,
            buffer_offset: 0,
            buffer_size: p.buffer.size,
            ownership: NATIVE_OWN | NATIVE_OWN_BUFFER,
            address: 0,
            properties,
            mapped: Cell::new(ptr::null_mut()),
            usage: desc.usage,
            memory_type: if desc.kind == MEMORY_VIRTUAL {
                u32::MAX
            } else {
                desc.memory_type
            },
            dedicated_image: ptr::null_mut(),
            allow_images: desc.kind == MEMORY_OPAQUE || desc.flags & MEMORY_ALLOW_IMAGES != 0,
            kind: desc.kind,
            flags: desc.flags,
            page_size: p.alignment,
            page_types: p.types.iter().fold(0, |mask, &id| mask | (1 << id)),
            sparse_size: p.size,
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
        if desc.kind != MEMORY_OPAQUE {
            let mut buffer = ptr::null_mut();
            let mut info = p.buffer;
            let usage = vk::VkBufferUsageFlags2CreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_USAGE_FLAGS_2_CREATE_INFO,
                usage: u64::from(info.usage),
                ..Default::default()
            };
            if usage.usage & vk::VK_BUFFER_USAGE_2_PREPROCESS_BUFFER_BIT_EXT != 0 {
                info.pNext = ptr::from_ref(&usage).cast();
                info.usage = 0;
            }
            unsafe {
                d.result((d.f.vkCreateBuffer.unwrap())(
                    d.handle,
                    &info,
                    ptr::null(),
                    &mut buffer,
                ))?;
            }
            result.buffer = buffer;
        }
        if desc.kind == MEMORY_VIRTUAL {
            let query = vk::VkBufferDeviceAddressInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_DEVICE_ADDRESS_INFO,
                buffer: result.buffer,
                ..Default::default()
            };
            result.address = unsafe { (d.f.vkGetBufferDeviceAddress.unwrap())(d.handle, &query) };
            if result.address == 0
                || result.address % desc.alignment != 0
                || result.address.checked_add(desc.size).is_none()
            {
                return Err(BACKEND_ERROR);
            }
            return Ok(result);
        }
        let dedicated = vk::VkMemoryDedicatedAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
            buffer: result.buffer,
            ..Default::default()
        };
        let flags = vk::VkMemoryAllocateFlagsInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO,
            pNext: if desc.kind == MEMORY_LINEAR && desc.flags & MEMORY_ALLOW_IMAGES == 0 {
                ptr::from_ref(&dedicated).cast()
            } else {
                ptr::null()
            },
            flags: vk::VkMemoryAllocateFlagBits_VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT,
            ..Default::default()
        };
        let allocate = vk::VkMemoryAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
            pNext: if desc.kind == MEMORY_LINEAR || desc.flags & MEMORY_ADDRESSABLE != 0 {
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
            result.buffer_offset = result.prefix;
            result.address = base.checked_add(result.prefix).ok_or(BACKEND_ERROR)?;
            result.address.checked_add(desc.size).ok_or(BACKEND_ERROR)?;
        }
        Ok(result)
    }
    pub(in crate::foundation) fn native_vulkan(&self) -> Result<VulkanMemory, Status> {
        unsafe { &*self.device }.ready()?;
        Ok(VulkanMemory {
            memory: self.memory as u64,
            buffer: self.buffer as u64,
            allocation_size: self.allocated,
            allocation_offset: self.prefix,
            buffer_size: self.buffer_size,
            buffer_offset: self.buffer_offset,
        })
    }
    pub(in crate::foundation) unsafe fn import_vulkan(
        d: &Device,
        desc: &MemoryDesc,
        value: VulkanMemory,
        ownership: u32,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        // Validate the same creation/usage/domain contract, without allocating
        // or looking up any GPU address in a runtime registry.
        unsafe {
            d.validate_memory(desc)?;
        }
        if ownership & !(NATIVE_OWN | NATIVE_OWN_BUFFER) != 0
            || (value.memory == 0 && ownership & NATIVE_OWN != 0)
            || (value.buffer == 0 && ownership & NATIVE_OWN_BUFFER != 0)
            || value.allocation_offset > value.allocation_size
            || (value.memory != 0 && desc.size > value.allocation_size - value.allocation_offset)
            || (value.memory == 0 && (value.allocation_size != 0 || value.allocation_offset != 0))
        {
            return Err(INVALID);
        }
        if desc.kind == MEMORY_OPAQUE {
            if value.memory == 0
                || value.buffer != 0
                || value.buffer_size != 0
                || value.buffer_offset != 0
            {
                return Err(INVALID);
            }
        } else if value.buffer == 0
            || value.buffer_offset > value.buffer_size
            || desc.size > value.buffer_size - value.buffer_offset
        {
            return Err(INVALID);
        }
        let properties = if desc.kind == MEMORY_VIRTUAL {
            0
        } else {
            d.snapshot
                .memory_types
                .iter()
                .find(|m| m.id == desc.memory_type && m.properties & (32 | 64 | 128) == 0)
                .ok_or(UNSUPPORTED)?
                .properties
        };
        let sparse = if desc.kind == MEMORY_VIRTUAL {
            if value.memory != 0 || value.buffer_offset != 0 || value.buffer_size != desc.size {
                return Err(INVALID);
            }
            Some(unsafe { d.prepare_memory(desc)? })
        } else {
            None
        };
        let address = if value.buffer != 0 {
            let query = vk::VkBufferDeviceAddressInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_DEVICE_ADDRESS_INFO,
                buffer: value.buffer as vk::VkBuffer,
                ..Default::default()
            };
            let base = unsafe { (d.f.vkGetBufferDeviceAddress.unwrap())(d.handle, &query) };
            if base == 0 {
                return Err(BACKEND_ERROR);
            }
            let address = base.checked_add(value.buffer_offset).ok_or(INVALID)?;
            if address % desc.alignment != 0 {
                return Err(INVALID);
            }
            address.checked_add(desc.size).ok_or(INVALID)?;
            address
        } else {
            0
        };
        Ok(Box::new(Self {
            device: d,
            buffer: value.buffer as vk::VkBuffer,
            memory: value.memory as vk::VkDeviceMemory,
            size: desc.size,
            allocated: value.allocation_size,
            prefix: value.allocation_offset,
            buffer_size: value.buffer_size,
            buffer_offset: value.buffer_offset,
            ownership,
            address,
            properties,
            mapped: Cell::new(ptr::null_mut()),
            usage: desc.usage,
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
            memory_type: if desc.kind == MEMORY_VIRTUAL {
                u32::MAX
            } else {
                desc.memory_type
            },
            kind: desc.kind,
            flags: desc.flags,
            page_size: sparse.as_ref().map_or(0, |p| p.alignment),
            page_types: sparse
                .as_ref()
                .map_or(0, |p| p.types.iter().fold(0, |mask, &id| mask | (1 << id))),
            sparse_size: sparse.as_ref().map_or(0, |p| p.size),
            dedicated_image: ptr::null_mut(),
            allow_images: value.memory != 0
                && (desc.kind == MEMORY_OPAQUE || desc.flags & MEMORY_ALLOW_IMAGES != 0),
        }))
    }
    pub(super) unsafe fn sparse_physical(
        d: &Device,
        span: Span,
        bytes: u64,
        alignment: u64,
        types: u32,
        addressable: bool,
    ) -> Result<(vk::VkDeviceMemory, u64), Status> {
        if span.memory.is_null() {
            if span.offset != 0 || span.size != 0 {
                return Err(INVALID);
            }
            return Ok((ptr::null_mut(), 0));
        }
        let m = unsafe { &*span.memory };
        if !ptr::eq(m.device, d)
            || m.memory.is_null()
            || m.kind == MEMORY_VIRTUAL
            || !m.allow_images
            || !m.dedicated_image.is_null()
            || span.size < bytes
            || m.properties & 16 != 0
        {
            return Err(INVALID);
        }
        let offset = m.range(span.offset, span.size)?;
        if offset % alignment != 0 {
            return Err(INVALID);
        }
        if types & (1 << m.memory_type) == 0
            || (addressable && m.kind == MEMORY_OPAQUE && m.flags & MEMORY_ADDRESSABLE == 0)
        {
            return Err(UNSUPPORTED);
        }
        Ok((m.memory, offset))
    }
    pub(super) fn sparse_handle(&self, d: &Device) -> Result<vk::VkBuffer, Status> {
        if !ptr::eq(self.device, d) || self.kind != MEMORY_VIRTUAL {
            return Err(INVALID);
        }
        Ok(self.buffer)
    }
    pub(super) unsafe fn sparse_bind(
        &self,
        d: &Device,
        b: &SparseMemoryBind,
    ) -> Result<vk::VkSparseMemoryBind, Status> {
        self.sparse_handle(d)?;
        if b.reserved != 0
            || b.flags != 0
            || b.size == 0
            || b.offset >= self.sparse_size
            || b.size > self.sparse_size - b.offset
            || b.offset % self.page_size != 0
            || b.size % self.page_size != 0
        {
            return Err(INVALID);
        }
        let (memory, offset) = unsafe {
            Self::sparse_physical(d, b.physical, b.size, self.page_size, self.page_types, true)?
        };
        Ok(vk::VkSparseMemoryBind {
            resourceOffset: b.offset,
            size: b.size,
            memory,
            memoryOffset: offset,
            flags: 0,
        })
    }
    pub(in crate::foundation) fn info(&self) -> Result<MemoryInfo, Status> {
        unsafe { &*self.device }.ready()?;
        Ok(MemoryInfo {
            size: self.size,
            allocation_size: self.allocated,
            allocation_offset: self.prefix,
            memory_type: self.memory_type,
            flags: self.flags,
        })
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
    ) -> Result<(vk::VkDeviceMemory, u64), Status> {
        if self.device != device || !self.allow_images || self.memory.is_null() {
            return Err(INVALID);
        }
        let native_offset = self.range(offset, size)?;
        if size < requirements.size || native_offset % requirements.alignment != 0 {
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
        Ok((self.memory, native_offset))
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
            buffer_offset: 0,
            buffer_size: 0,
            ownership: NATIVE_OWN,
            address: 0,
            properties,
            mapped: Cell::new(ptr::null_mut()),
            usage: 0,
            domains: Vec::new(),
            memory_type,
            dedicated_image: image,
            allow_images: true,
            kind: MEMORY_OPAQUE,
            flags: 0,
            page_size: 0,
            page_types: 0,
            sparse_size: 0,
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
        self.range(offset, size)?;
        Ok((
            self.buffer,
            self.buffer_offset.checked_add(offset).ok_or(INVALID)?,
            self.address.checked_add(offset).ok_or(INVALID)?,
        ))
    }
    // Check supplied heap host bytes without mapping or retaining them.
    pub(super) fn matches_mapping(
        &self,
        offset: u64,
        size: u64,
        host: HostSpan,
    ) -> Result<(), Status> {
        let native_offset = self.range(offset, host.size)?;
        if self.mapped.get().is_null()
            || host.size < size
            || host.data
                != self
                    .mapped
                    .get()
                    .wrapping_add(native_offset as usize)
                    .cast()
        {
            return Err(INVALID);
        }
        Ok(())
    }
    pub(super) fn concurrent(&self) -> bool {
        !self.domains.is_empty()
    }
    pub(super) fn capture_address_flags(&self) -> u32 {
        self.address_flags()
            | if self.usage & CAPTURE_USAGE != 0 {
                vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_TRANSFORM_FEEDBACK_BUFFER_USAGE_BIT_KHR
            } else {
                0
            }
    }
    pub(super) fn address_flags(&self) -> u32 {
        (if self.kind != MEMORY_VIRTUAL || self.flags & MEMORY_FULLY_BOUND != 0 {
            vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR
        } else {
            0
        }) | if self.usage & 64 != 0 {
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
        if self.memory.is_null() || self.properties & 2 == 0 || self.allocated > isize::MAX as u64 {
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
                FeatureBits::ZERO,
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
                    MemoryDesc { flags: 2, ..desc },
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
    fn image_placement_uses_native_origin_and_preserves_dedication() {
        // No native object is created or destroyed. Exercise a deliberately
        // nonzero prefix independently of driver allocation-address choices.
        let mut memory = std::mem::ManuallyDrop::new(Memory {
            device: ptr::null(),
            buffer: ptr::dangling_mut(),
            memory: ptr::dangling_mut(),
            size: 256,
            allocated: 512,
            prefix: 5,
            buffer_offset: 5,
            buffer_size: 512,
            ownership: 0,
            address: 0,
            properties: 0,
            mapped: Cell::new(ptr::null_mut()),
            usage: 3,
            domains: Vec::new(),
            memory_type: 0,
            dedicated_image: ptr::null_mut(),
            allow_images: true,
            kind: MEMORY_LINEAR,
            flags: MEMORY_ALLOW_IMAGES,
            page_size: 0,
            page_types: 0,
            sparse_size: 0,
        });
        let req = vk::VkMemoryRequirements {
            size: 64,
            alignment: 16,
            memoryTypeBits: 1,
        };
        let image = ptr::dangling_mut();
        assert_eq!(
            memory.image_placement(ptr::null(), image, req, false, 11, 64),
            Ok((memory.memory, 16))
        );
        assert_eq!(
            memory.image_placement(ptr::null(), image, req, false, 16, 64),
            Err(INVALID)
        );
        assert_eq!(
            memory.image_placement(ptr::null(), image, req, false, 251, 64),
            Err(INVALID)
        );
        assert_eq!(
            memory.image_placement(ptr::null(), image, req, true, 11, 64),
            Err(INVALID)
        );
        memory.allow_images = false;
        assert_eq!(
            memory.image_placement(ptr::null(), image, req, false, 11, 64),
            Err(INVALID)
        );
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
        assert_eq!(buffer_usage(16384), Err(UNSUPPORTED));
        assert_ne!(
            buffer_usage(PREPROCESS_USAGE).unwrap()
                & vk::VK_BUFFER_USAGE_2_PREPROCESS_BUFFER_BIT_EXT as u32,
            0
        );
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
