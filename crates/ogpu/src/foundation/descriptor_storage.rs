//! C2 explicit descriptor storage. One backing, no descriptor recipes or mirror.
//! Public locations and C/Rust operations share this native implementation.
use super::*;

const CPU_ACCESS: u32 = HEAP_CPU_WRITE | HEAP_CPU_COPY_SOURCE | HEAP_CPU_COPY_DESTINATION;

pub struct DescriptorHeap {
    device: *const Device,
    desc: HeapDesc,
    requirements: HeapRequirements,
    binding: vk::VkBindHeapInfoEXT,
}

fn requirements(
    p: &NativeDescriptorProperties,
    request: &HeapRequest,
) -> Result<HeapRequirements, Status> {
    if request.flags != 0 || request.usage & !15 != 0 {
        return Err(UNSUPPORTED);
    }
    if request.capacity == 0 {
        return Err(INVALID);
    }
    let (alignment, maximum, reserved, reserved_alignment) = match request.kind {
        1 => (
            p.resource_heap_alignment,
            p.resource_heap_max_size,
            p.resource_reserved_size,
            p.resource_reserved_alignment,
        ),
        2 => (
            p.sampler_heap_alignment,
            p.sampler_heap_max_size,
            p.sampler_reserved_size,
            p.sampler_reserved_alignment,
        ),
        _ => return Err(UNSUPPORTED),
    };
    match request.storage {
        HEAP_HOST if request.usage & HEAP_SHADER_VISIBLE != 0 => return Err(INVALID),
        HEAP_HOST | HEAP_MEMORY => (),
        // No convenience Vulkan allocation behind the native-heap alternative.
        _ => return Err(UNSUPPORTED),
    }
    let visible = request.usage & HEAP_SHADER_VISIBLE != 0;
    if visible && (request.capacity > maximum || request.capacity < reserved) {
        return Err(UNSUPPORTED);
    }
    let host_access = request.storage == HEAP_HOST || request.usage & CPU_ACCESS != 0;
    if host_access && request.capacity > isize::MAX as u64 {
        return Err(UNSUPPORTED);
    }
    if !alignment.is_power_of_two() || !reserved_alignment.is_power_of_two() {
        return Err(UNSUPPORTED);
    }
    Ok(HeapRequirements {
        unit_bytes: 1,
        backing_size: request.capacity,
        backing_alignment: if request.storage == HEAP_MEMORY {
            alignment
        } else {
            1
        },
        reserved_size: if visible { reserved } else { 0 },
        reserved_alignment: if visible { reserved_alignment } else { 1 },
        memory_usage: if request.storage == HEAP_MEMORY {
            128
        } else {
            0
        },
    })
}
fn shape(size: u64, alignment: u64) -> Result<DescriptorShape, Status> {
    if size == 0 || !alignment.is_power_of_two() || size % alignment != 0 {
        return Err(UNSUPPORTED);
    }
    // Vulkan typed arrays use the native descriptor size, not a common slot.
    Ok(DescriptorShape {
        size,
        alignment,
        array_stride: size,
    })
}
fn resource_shape(p: &NativeDescriptorProperties, kind: u32) -> Result<DescriptorShape, Status> {
    match kind {
        1 | 2 | 5 => shape(p.image_size, p.image_alignment),
        3 | 4 => shape(p.buffer_size, p.buffer_alignment),
        _ => Err(UNSUPPORTED),
    }
}
fn empty_host(host: HostSpan) -> bool {
    host.data.is_null() && host.size == 0
}
fn empty_memory(memory: Span) -> bool {
    memory.memory.is_null() && memory.offset == 0 && memory.size == 0
}
fn host_extent(host: HostSpan, size: u64) -> Result<(), Status> {
    if host.data.is_null()
        || size > isize::MAX as u64
        || (host.data as usize).checked_add(size as usize).is_none()
    {
        return Err(INVALID);
    }
    if host.size < size {
        return Err(CAPACITY);
    }
    Ok(())
}
fn range(desc: &HeapDesc, offset: u64, size: u64) -> Result<(), Status> {
    let end = offset.checked_add(size).ok_or(INVALID)?;
    if end > desc.request.capacity {
        return Err(CAPACITY);
    }
    if size != 0
        && desc.reserved_size != 0
        && offset < desc.reserved_offset + desc.reserved_size
        && desc.reserved_offset < end
    {
        return Err(INVALID);
    }
    Ok(())
}
impl DescriptorHeap {
    /// All backing is borrowed through destruction and recorded/future/pending uses.
    pub(in crate::foundation) unsafe fn create(
        device: &Device,
        desc: &HeapDesc,
    ) -> Result<Box<Self>, Status> {
        device.ready()?;
        let req = requirements(&device.snapshot.descriptor_properties, &desc.request)?;
        if desc.reserved_size < req.reserved_size
            || desc.reserved_offset % req.reserved_alignment != 0
            || desc.reserved_offset > desc.request.capacity
            || desc.reserved_size > desc.request.capacity - desc.reserved_offset
        {
            return Err(INVALID);
        }
        let mut binding = vk::VkBindHeapInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_HEAP_INFO_EXT,
            reservedRangeOffset: desc.reserved_offset,
            reservedRangeSize: desc.reserved_size,
            ..Default::default()
        };
        match desc.request.storage {
            HEAP_HOST => {
                if !empty_memory(desc.memory) {
                    return Err(INVALID);
                }
                host_extent(desc.host, req.backing_size)?;
            }
            HEAP_MEMORY => {
                let span = desc.memory;
                let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
                // Validate the supplied extent, but bind only requested capacity.
                let (_, _, address) =
                    memory.descriptor_range(device, span.offset, span.size, req.memory_usage)?;
                if span.size < req.backing_size {
                    return Err(CAPACITY);
                }
                if address % req.backing_alignment != 0 {
                    return Err(INVALID);
                }
                if desc.request.usage & CPU_ACCESS != 0 || !empty_host(desc.host) {
                    if desc.host.size > span.size {
                        return Err(INVALID);
                    }
                    host_extent(desc.host, req.backing_size)?;
                    memory.matches_mapping(span.offset, req.backing_size, desc.host)?;
                }
                binding.heapRange = vk::VkDeviceAddressRangeKHR {
                    address,
                    size: req.backing_size,
                };
            }
            _ => unreachable!(),
        }
        Ok(Box::new(Self {
            device,
            desc: *desc,
            requirements: req,
            binding,
        }))
    }
    pub(in crate::foundation) fn kind(&self) -> u32 {
        self.desc.request.kind
    }
    pub(in crate::foundation) fn info(&self) -> (HeapDesc, HeapRequirements) {
        (self.desc, self.requirements)
    }
    fn host_range(
        &self,
        device: &Device,
        kind: u32,
        usage: u32,
        offset: u64,
        size: u64,
    ) -> Result<HostSpan, Status> {
        if !ptr::eq(self.device, device)
            || self.desc.request.kind != kind
            || self.desc.request.usage & usage != usage
        {
            return Err(INVALID);
        }
        range(&self.desc, offset, size)?;
        if size == 0 {
            return Ok(HostSpan {
                data: ptr::null_mut(),
                size: 0,
            });
        }
        host_extent(self.desc.host, self.requirements.backing_size)?;
        // The immutable descriptor borrows a live mapping for its whole lifetime.
        Ok(HostSpan {
            data: self
                .desc
                .host
                .data
                .cast::<u8>()
                .wrapping_add(offset as usize)
                .cast(),
            size,
        })
    }
    unsafe fn destination(
        device: &Device,
        kind: u32,
        location: HeapLocation,
        shape: DescriptorShape,
    ) -> Result<vk::VkHostAddressRangeEXT, Status> {
        let heap = unsafe { location.heap.as_ref() }.ok_or(INVALID)?;
        if location.offset % shape.alignment != 0 {
            return Err(INVALID);
        }
        destination(
            heap.host_range(device, kind, HEAP_CPU_WRITE, location.offset, shape.size)?,
            shape.size,
        )
    }
    pub(in crate::foundation) unsafe fn bind_info(
        &self,
        device: &Device,
        domain: u32,
    ) -> Result<vk::VkBindHeapInfoEXT, Status> {
        device.ready()?;
        if !ptr::eq(self.device, device) || self.desc.request.usage & HEAP_SHADER_VISIBLE == 0 {
            return Err(INVALID);
        }
        if !device
            .snapshot
            .queues
            .iter()
            .any(|q| q.domain == domain && q.flags & (COMPUTE | GRAPHICS) != 0)
        {
            return Err(UNSUPPORTED);
        }
        let memory = unsafe { &*self.desc.memory.memory };
        memory.command_range(
            device,
            domain,
            self.desc.memory.offset,
            self.requirements.backing_size,
            self.requirements.memory_usage,
        )?;
        Ok(self.binding)
    }
    pub(in crate::foundation::native) unsafe fn bind(
        &self,
        device: &Device,
        command: vk::VkCommandBuffer,
        domain: u32,
    ) -> Result<(), Status> {
        let info = unsafe { self.bind_info(device, domain)? };
        let function = if self.desc.request.kind == 1 {
            device.f.vkCmdBindResourceHeapEXT
        } else {
            device.f.vkCmdBindSamplerHeapEXT
        };
        unsafe { function.unwrap()(command, &info) };
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct CopyRange {
    source: *const u8,
    destination: *mut u8,
    size: usize,
}
#[derive(Clone, Copy)]
struct Interval {
    start: usize,
    end: usize,
}
fn copy_layout(count: u32) -> Result<(Layout, usize), Status> {
    let intervals = count.checked_mul(2).ok_or(INVALID)?;
    let (layout, offset) = array::<CopyRange>(count)?
        .extend(array::<Interval>(intervals)?)
        .map_err(|_| INVALID)?;
    Ok((layout.pad_to_align(), offset))
}
impl Device {
    pub(in crate::foundation) fn heap_requirements(
        &self,
        request: &HeapRequest,
    ) -> Result<HeapRequirements, Status> {
        self.ready()?;
        requirements(&self.snapshot.descriptor_properties, request)
    }
    pub(in crate::foundation) unsafe fn resource_descriptor_shape(
        &self,
        input: &ResourceDescriptor,
    ) -> Result<DescriptorShape, Status> {
        self.ready()?;
        let mut data = ResourceData::default();
        unsafe { self.resource_data(input, &mut data)? };
        resource_shape(&self.snapshot.descriptor_properties, input.kind)
    }
    pub(in crate::foundation) fn sampler_descriptor_shape(
        &self,
        input: &SamplerDesc,
    ) -> Result<DescriptorShape, Status> {
        self.ready()?;
        let p = &self.snapshot.descriptor_properties;
        sampler(input, p, self.snapshot.features.enabled)?;
        shape(p.sampler_size, p.sampler_alignment)
    }
    pub(in crate::foundation) fn heap_descriptor_scratch(
        &self,
        operation: u32,
        count: u32,
    ) -> Result<HostRequirements, Status> {
        self.ready()?;
        match operation {
            DESCRIPTOR_WRITE_RESOURCE => descriptor_scratch(1, count),
            DESCRIPTOR_WRITE_SAMPLER => descriptor_scratch(2, count),
            DESCRIPTOR_COPY => {
                let (layout, _) = copy_layout(count)?;
                Ok(HostRequirements {
                    size: layout.size() as u64,
                    alignment: layout.align() as u64,
                })
            }
            _ => Err(UNSUPPORTED),
        }
    }
    pub(in crate::foundation) unsafe fn write_resource_locations(
        &self,
        count: u32,
        inputs: *const ResourceDescriptor,
        locations: *const HeapLocation,
        scratch: HostSpan,
    ) -> Result<(), Status> {
        array::<HeapLocation>(count)?;
        if count != 0 && locations.is_null() {
            return Err(INVALID);
        }
        unsafe {
            self.write_resources_to(count, inputs, scratch, |i, _| {
                DescriptorHeap::destination(
                    self,
                    1,
                    locations.add(i).read(),
                    resource_shape(&self.snapshot.descriptor_properties, (*inputs.add(i)).kind)?,
                )
            })
        }
    }
    pub(in crate::foundation) unsafe fn write_sampler_locations(
        &self,
        count: u32,
        inputs: *const SamplerDesc,
        locations: *const HeapLocation,
        scratch: HostSpan,
    ) -> Result<(), Status> {
        array::<HeapLocation>(count)?;
        if count != 0 && locations.is_null() {
            return Err(INVALID);
        }
        unsafe {
            self.write_samplers_to(count, inputs, scratch, |i, _| {
                let p = &self.snapshot.descriptor_properties;
                DescriptorHeap::destination(
                    self,
                    2,
                    locations.add(i).read(),
                    shape(p.sampler_size, p.sampler_alignment)?,
                )
            })
        }
    }
    pub(in crate::foundation) unsafe fn copy_descriptors(
        &self,
        count: u32,
        copies: *const DescriptorCopy,
        scratch: HostSpan,
    ) -> Result<(), Status> {
        self.ready()?;
        array::<DescriptorCopy>(count)?;
        let (layout, offset) = copy_layout(count)?;
        if scratch.size < layout.size() as u64 {
            return Err(CAPACITY);
        }
        if count == 0 {
            return Ok(());
        }
        if copies.is_null() || scratch.data.is_null() || scratch.data as usize % layout.align() != 0
        {
            return Err(INVALID);
        }
        let records = scratch.data.cast::<CopyRange>();
        let intervals = scratch
            .data
            .cast::<u8>()
            .wrapping_add(offset)
            .cast::<Interval>();
        let mut active = 0;
        for i in 0..count as usize {
            let copy = unsafe { &*copies.add(i) };
            let source = unsafe { copy.source.heap.as_ref() }.ok_or(INVALID)?;
            let dest = unsafe { copy.destination.heap.as_ref() }.ok_or(INVALID)?;
            let kind = source.desc.request.kind;
            let src = source.host_range(
                self,
                kind,
                HEAP_CPU_COPY_SOURCE,
                copy.source.offset,
                copy.size,
            )?;
            let dst = dest.host_range(
                self,
                kind,
                HEAP_CPU_COPY_DESTINATION,
                copy.destination.offset,
                copy.size,
            )?;
            if copy.size == 0 {
                continue;
            }
            unsafe {
                records.add(active).write(CopyRange {
                    source: src.data.cast(),
                    destination: dst.data.cast(),
                    size: copy.size as usize,
                });
                intervals.add(active * 2).write(Interval {
                    start: src.data as usize,
                    end: src.data as usize + copy.size as usize,
                });
                intervals.add(active * 2 + 1).write(Interval {
                    start: dst.data as usize,
                    end: dst.data as usize + copy.size as usize,
                });
            }
            active += 1;
        }
        // Check the whole batch, including aliases via different wrappers. The
        // caller proves aliases not visible through these host mappings. Sorting
        // uses only the supplied scratch; no allocation or temporary contents.
        let ranges = unsafe { std::slice::from_raw_parts_mut(intervals, active * 2) };
        ranges.sort_unstable_by_key(|v| v.start);
        if ranges.windows(2).any(|v| v[0].end > v[1].start) {
            return Err(INVALID);
        }
        for i in 0..active {
            let copy = unsafe { records.add(i).read() };
            unsafe { ptr::copy_nonoverlapping(copy.source, copy.destination, copy.size) };
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "descriptor_storage_tests.rs"]
mod tests;
