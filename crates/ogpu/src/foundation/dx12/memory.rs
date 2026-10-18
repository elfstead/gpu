//! Explicit D3D12 backing and non-owning ranges. No staging, suballocator or
//! retention. One linear memory is one addressable placed buffer over its heap;
//! opaque memory reserves its heap for placed resources only.
use super::*;
use std::cell::Cell;

/// Native facts gathered before any heap creation. Preparation may allocate on
/// the native side only where the API requires a queryable description.
struct Prepared {
    desc: d3d::D3D12_RESOURCE_DESC,
    size: u64,
    alignment: u64,
    types: Vec<u32>,
}

pub struct Memory {
    device: *const Device,
    /// Placement heap. Dedicated committed backing has no separate heap.
    heap: Option<d3d::ID3D12Heap>,
    /// Linear kind: the whole-allocation buffer that makes this backing addressable.
    buffer: Option<d3d::ID3D12Resource>,
    /// Dedicated image backing: a committed resource adopted by exactly one image.
    committed: Option<d3d::ID3D12Resource>,
    size: u64,
    allocated: u64,
    prefix: u64,
    address: u64,
    properties: u32,
    mapped: Cell<*mut u8>,
    usage: u64,
    dedicated_image: *const Image,
    domains: Vec<u32>,
}

impl Drop for Memory {
    fn drop(&mut self) {
        // COM references drop with the fields; the caller retired all uses and
        // kept the device alive. No implicit wait.
    }
}

/// The D3D12 heap class flags of a backing. Tier-2+ heaps may mix classes;
/// tier-1 heaps keep RT/DS textures out (those images report dedicated
/// requirements and use the committed path).
fn heap_flags(device: &Device, opaque: bool) -> d3d::D3D12_HEAP_FLAGS {
    if !opaque {
        d3d::D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS
    } else if device.heap_tier >= 2 {
        d3d::D3D12_HEAP_FLAG_ALLOW_ALL_BUFFERS_AND_TEXTURES
    } else {
        d3d::D3D12_HEAP_FLAG_ALLOW_ONLY_BUFFERS | d3d::D3D12_HEAP_FLAG_ALLOW_ONLY_NON_RT_DS_TEXTURES
    }
}

fn heap_type(memory_type: u32) -> Result<d3d::D3D12_HEAP_TYPE, Status> {
    match memory_type {
        0 => Ok(d3d::D3D12_HEAP_TYPE_DEFAULT),
        1 => Ok(d3d::D3D12_HEAP_TYPE_UPLOAD),
        2 => Ok(d3d::D3D12_HEAP_TYPE_READBACK),
        _ => Err(INVALID),
    }
}

impl Device {
    /// Validate a memory description and compute exact native requirements.
    /// No GPU object or allocation is created here.
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
        out.dedicated_required = 0;
        out.dedicated_preferred = 0;
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
        let usage = if desc.kind == 1 {
            buffer_usage(desc.usage)?
        } else {
            d3d::D3D12_RESOURCE_FLAG_NONE
        };
        // Storage usage carries the native unordered-access flag, which the
        // UPLOAD and READBACK heap types reject: storage buffers report only
        // the DEFAULT heap as compatible instead of failing later.
        let storage = desc.usage & USAGE_STORAGE != 0;
        let types: Vec<u32> = self
            .snapshot
            .memory_types
            .iter()
            .filter(|m| m.properties & (32 | 64 | 128) == 0 && (!storage || m.id == 0))
            .map(|m| m.id)
            .collect();
        if types.is_empty() {
            return Err(UNSUPPORTED);
        }
        if desc.kind == 2 {
            // Opaque backing is reserved for placed resources: the caller sizes
            // it from image requirements and placement alignment is checked at
            // bind. No buffer description exists for this kind.
            return Ok(Prepared {
                desc: d3d::D3D12_RESOURCE_DESC::default(),
                size: desc.size,
                alignment: 1,
                types,
            });
        }
        let mut resource = d3d::D3D12_RESOURCE_DESC {
            Dimension: d3d::D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: desc.size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: dxgi::Common::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: d3d::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: usage,
            Alignment: 0,
        };
        let mut info = self.allocation_info(&resource);
        if desc.kind == 1 && desc.alignment > info.Alignment {
            // Ordinary alignments use the native guarantee. Only over-alignment
            // needs bounded leading padding inside a larger backing.
            let padded = desc.size.checked_add(desc.alignment - 1).ok_or(INVALID)?;
            resource.Width = padded;
            info = self.allocation_info(&resource);
        }
        if info.SizeInBytes == 0
            || info.SizeInBytes == u64::MAX
            || info.Alignment == 0
            || info.SizeInBytes > self.snapshot.memory_limits.max_allocation_size
        {
            return Err(UNSUPPORTED);
        }
        Ok(Prepared {
            desc: resource,
            size: info.SizeInBytes,
            alignment: info.Alignment.max(desc.alignment),
            types,
        })
    }

    pub(super) fn allocation_info(&self, desc: &d3d::D3D12_RESOURCE_DESC) -> d3d::D3D12_RESOURCE_ALLOCATION_INFO {
        // SAFETY: single well-formed description; query creates no object.
        unsafe { self.device.GetResourceAllocationInfo(0, std::slice::from_ref(desc)) }
    }
}

/// Translate foundation buffer usage bits to native resource flags. Unknown bits
/// are rejected, never silently dropped.
fn buffer_usage(usage: u64) -> Result<d3d::D3D12_RESOURCE_FLAGS, Status> {
    if usage & !511 != 0 {
        return Err(UNSUPPORTED);
    }
    // USAGE_STORAGE (64) is the only buffer usage with a native resource flag.
    let mut flags = d3d::D3D12_RESOURCE_FLAG_NONE;
    if usage & 64 != 0 {
        flags |= d3d::D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }
    Ok(flags)
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
        let ty = heap_type(desc.memory_type)?;
        let heap_desc = d3d::D3D12_HEAP_DESC {
            SizeInBytes: p.size,
            Properties: d3d::D3D12_HEAP_PROPERTIES {
                Type: ty,
                CPUPageProperty: d3d::D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
                MemoryPoolPreference: d3d::D3D12_MEMORY_POOL_UNKNOWN,
                CreationNodeMask: 1,
                VisibleNodeMask: 1,
            },
            Flags: heap_flags(d, desc.kind == 2),
            // Opaque heaps use the native default; the caller aligns placements.
            Alignment: if desc.kind == 1 { p.alignment } else { 0 },
        };
        // SAFETY: validated description; the output heap reference is adopted.
        let mut heap: Option<d3d::ID3D12Heap> = None;
        unsafe {
            d.device
                .CreateHeap::<d3d::ID3D12Heap>(&heap_desc, &mut heap)
                .map_err(map_error)?
        };
        let mut result = Box::new(Self {
            device: d,
            heap: Some(heap.ok_or(BACKEND_ERROR)?),
            buffer: None,
            committed: None,
            size: desc.size,
            allocated: p.size,
            prefix: 0,
            address: 0,
            properties,
            mapped: Cell::new(ptr::null_mut()),
            usage: desc.usage,
            dedicated_image: ptr::null(),
            domains: if desc.concurrent_domain_count == 0 {
                Vec::new()
            } else {
                // SAFETY: caller provides the described domain array.
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
            // One whole-allocation placed buffer makes the backing addressable;
            // spans are caller-partitioned ranges of it.
            let mut buffer: Option<d3d::ID3D12Resource> = None;
            // SAFETY: heap offset zero and the probed allocation size fit exactly.
            unsafe {
                d.device
                    .CreatePlacedResource(
                        result.heap.as_ref().ok_or(BACKEND_ERROR)?,
                        0,
                        ptr::from_ref(&p.desc),
                        d3d::D3D12_RESOURCE_STATE_COMMON,
                        None,
                        &mut buffer,
                    )
                    .map_err(map_error)?
            };
            let buffer = buffer.ok_or(BACKEND_ERROR)?;
            // SAFETY: buffer stays live for the query.
            let base = unsafe { buffer.GetGPUVirtualAddress() };
            if base == 0 {
                return Err(BACKEND_ERROR);
            }
            result.prefix = (desc.alignment - base % desc.alignment) % desc.alignment;
            result.address = base.checked_add(result.prefix).ok_or(BACKEND_ERROR)?;
            result.address.checked_add(desc.size).ok_or(BACKEND_ERROR)?;
            result.buffer = Some(buffer);
        }
        Ok(result)
    }

    /// Create backing dedicated to one unbound image: a committed resource that
    /// exactly one image may bind, at offset zero only.
    pub(super) fn dedicated_image(
        d: &Device,
        image: &Image,
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
        let ty = heap_type(memory_type)?;
        let heap_properties = d3d::D3D12_HEAP_PROPERTIES {
            Type: ty,
            CPUPageProperty: d3d::D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: d3d::D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 1,
            VisibleNodeMask: 1,
        };
        let mut committed: Option<d3d::ID3D12Resource> = None;
        // SAFETY: well-formed committed description; the output reference is adopted.
        unsafe {
            d.device
                .CreateCommittedResource::<d3d::ID3D12Resource>(
                    &heap_properties,
                    d3d::D3D12_HEAP_FLAG_NONE,
                    ptr::from_ref(&image.native_desc()),
                    d3d::D3D12_RESOURCE_STATE_COMMON,
                    None,
                    &mut committed,
                )
                .map_err(map_error)?
        };
        let requirements = image.requirements();
        Ok(Box::new(Self {
            device: d,
            // Committed resources own their heap; no placement is possible.
            heap: None,
            buffer: None,
            committed: Some(committed.ok_or(BACKEND_ERROR)?),
            size: requirements.0,
            allocated: requirements.0,
            prefix: 0,
            address: 0,
            properties,
            mapped: Cell::new(ptr::null_mut()),
            usage: 0,
            dedicated_image: image,
            domains: Vec::new(),
        }))
    }

    fn range(&self, offset: u64, size: u64) -> Result<u64, Status> {
        if offset > self.size || size > self.size - offset {
            return Err(INVALID);
        }
        self.prefix.checked_add(offset).ok_or(INVALID)
    }

    /// Validate placement of one image and return this backing for binding.
    pub(super) fn image_placement(
        &self,
        device: *const Device,
        image: *const Image,
        size: u64,
        alignment: u64,
        dedicated_required: bool,
        offset: u64,
        span_size: u64,
    ) -> Result<(), Status> {
        if self.device != device || self.buffer.is_some() || self.committed.is_some() {
            return Err(INVALID);
        }
        self.range(offset, span_size)?;
        if span_size < size || offset % alignment != 0 {
            return Err(INVALID);
        }
        if (!self.dedicated_image.is_null() && (self.dedicated_image != image || offset != 0))
            || (dedicated_required && self.dedicated_image != image)
        {
            return Err(INVALID);
        }
        Ok(())
    }

    pub(super) fn heap(&self) -> Option<&d3d::ID3D12Heap> {
        self.heap.as_ref()
    }

    /// The dedicated committed backing, cloned with an owned reference for the
    /// adopting image. Exactly one image may hold this backing.
    pub(super) fn committed(&self) -> Option<d3d::ID3D12Resource> {
        self.committed.clone()
    }

    pub(super) fn command_range(
        &self,
        device: *const Device,
        domain: u32,
        offset: u64,
        size: u64,
        usage: u64,
    ) -> Result<(&d3d::ID3D12Resource, u64, u64), Status> {
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
    ) -> Result<(&d3d::ID3D12Resource, u64, u64), Status> {
        if self.device != device || self.usage & usage != usage {
            return Err(INVALID);
        }
        let buffer = self.buffer.as_ref().ok_or(INVALID)?;
        Ok((
            buffer,
            self.range(offset, size)?,
            self.address.checked_add(offset).ok_or(INVALID)?,
        ))
    }

    pub(in crate::foundation) fn address(&self, offset: u64, size: u64) -> Result<u64, Status> {
        // SAFETY: device pointer is live for every memory operation.
        unsafe { &*self.device }.ready()?;
        self.range(offset, size)?;
        if self.address == 0 {
            return Err(UNSUPPORTED);
        }
        self.address.checked_add(offset).ok_or(INVALID)
    }

    pub(in crate::foundation) fn map(&self, offset: u64, size: u64) -> Result<Mapping, Status> {
        // SAFETY: device pointer is live for every memory operation.
        let d = unsafe { &*self.device };
        d.ready()?;
        let offset = self.range(offset, size)?;
        if size == 0 {
            return Err(INVALID);
        }
        if self.properties & MEMORY_HOST_VISIBLE == 0 || self.allocated > isize::MAX as u64 {
            return Err(UNSUPPORTED);
        }
        let buffer = self.buffer.as_ref().ok_or(UNSUPPORTED)?;
        if self.mapped.get().is_null() {
            let mut pointer: *mut std::ffi::c_void = ptr::null_mut();
            // SAFETY: whole subresource mapping of a live upload/readback buffer.
            unsafe {
                buffer
                    .Map(
                        0,
                        Some(ptr::from_ref(&d3d::D3D12_RANGE {
                            Begin: 0,
                            End: 0,
                        })),
                        Some(ptr::from_mut(&mut pointer).cast()),
                    )
                    .map_err(map_error)?
            };
            self.mapped.set(pointer.cast());
        }
        Ok(Mapping {
            // SAFETY: validated range in a mapped allocation <= isize::MAX.
            data: unsafe { self.mapped.get().add(offset as usize) }.cast(),
            size,
            cache_atom_size: 1,
            cache_offset: 0,
            coherent: 1,
            reserved: 0,
        })
    }

    pub(in crate::foundation) fn unmap(&self) {
        if !self.mapped.get().is_null() {
            if let Some(buffer) = self.buffer.as_ref() {
                // SAFETY: external serialization of map/unmap is a host contract.
                unsafe {
                    buffer.Unmap(0, None);
                }
            }
            self.mapped.set(ptr::null_mut());
        }
    }

    /// All CPU-visible D3D12 memory is coherent: flush/invalidate are ordered
    /// no-ops that still enforce the mapped-state and range contract.
    pub(in crate::foundation) fn cache(
        &self,
        offset: u64,
        size: u64,
        _flush: bool,
    ) -> Result<(), Status> {
        // SAFETY: device pointer is live for every memory operation.
        let d = unsafe { &*self.device };
        d.ready()?;
        self.range(offset, size)?;
        if self.mapped.get().is_null() {
            return Err(INVALID);
        }
        if size == 0 || self.properties & MEMORY_COHERENT != 0 {
            return Ok(());
        }
        Ok(())
    }
}
