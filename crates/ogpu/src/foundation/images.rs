//! Image interpretations of explicit backing, with independent borrowed views.
//! No allocator, placement policy, layout tracker, ownership retention or waits.
use super::*;
use std::cell::Cell;

#[derive(Clone, Copy)]
struct Format {
    native: vk::VkFormat,
    aspects: u32,
    class: u32,
}
fn format(id: u32) -> Result<Format, Status> {
    let (native, aspects, class) = match id {
        1 => (vk::VkFormat_VK_FORMAT_R8_UNORM, 1, 8),
        2 => (vk::VkFormat_VK_FORMAT_R8G8_UNORM, 1, 16),
        3 => (vk::VkFormat_VK_FORMAT_R8G8B8A8_UNORM, 1, 32),
        4 => (vk::VkFormat_VK_FORMAT_R8G8B8A8_SRGB, 1, 32),
        5 => (vk::VkFormat_VK_FORMAT_B8G8R8A8_UNORM, 1, 32),
        6 => (vk::VkFormat_VK_FORMAT_B8G8R8A8_SRGB, 1, 32),
        7 => (vk::VkFormat_VK_FORMAT_R16_SFLOAT, 1, 16),
        8 => (vk::VkFormat_VK_FORMAT_R16G16_SFLOAT, 1, 32),
        9 => (vk::VkFormat_VK_FORMAT_R16G16B16A16_SFLOAT, 1, 64),
        10 => (vk::VkFormat_VK_FORMAT_R32_SFLOAT, 1, 32),
        11 => (vk::VkFormat_VK_FORMAT_R32_UINT, 1, 32),
        12 => (vk::VkFormat_VK_FORMAT_R32G32B32A32_SFLOAT, 1, 128),
        13 => (vk::VkFormat_VK_FORMAT_D16_UNORM, 2, 1001),
        14 => (vk::VkFormat_VK_FORMAT_D32_SFLOAT, 2, 1002),
        15 => (vk::VkFormat_VK_FORMAT_D24_UNORM_S8_UINT, 6, 1003),
        16 => (vk::VkFormat_VK_FORMAT_D32_SFLOAT_S8_UINT, 6, 1004),
        _ => return Err(UNSUPPORTED),
    };
    Ok(Format {
        native,
        aspects,
        class,
    })
}
fn usage(value: u64, aspects: u32) -> Result<u32, Status> {
    if value & !255 != 0 {
        return Err(UNSUPPORTED);
    }
    if value == 0
        || (value & 16 != 0 && aspects != 1)
        || (value & 32 != 0 && aspects == 1)
        || (value & 64 != 0 && (value & !(16 | 32 | 64 | 128) != 0 || value & (16 | 32 | 128) == 0))
    {
        return Err(INVALID);
    }
    // These eight OGPU usage bits deliberately map to the eight core image uses.
    Ok(value as u32)
}
fn dimensions(desc: &ImageDesc) -> Result<vk::VkImageType, Status> {
    let e = desc.extent;
    if e.x == 0
        || e.y == 0
        || e.z == 0
        || desc.layer_count == 0
        || desc.mip_count == 0
        || desc.mip_count > 32 - e.x.max(e.y).max(e.z).leading_zeros()
        || !desc.sample_count.is_power_of_two()
        || desc.sample_count > 64
    {
        return Err(INVALID);
    }
    let ty = match desc.dimension {
        1 if e.y == 1 && e.z == 1 => vk::VkImageType_VK_IMAGE_TYPE_1D,
        2 if e.z == 1 => vk::VkImageType_VK_IMAGE_TYPE_2D,
        3 if desc.layer_count == 1 => vk::VkImageType_VK_IMAGE_TYPE_3D,
        _ => return Err(INVALID),
    };
    if desc.sample_count != 1 && (desc.dimension != 2 || desc.mip_count != 1 || desc.flags & 2 != 0)
    {
        return Err(INVALID);
    }
    if desc.flags & 2 != 0 && (desc.dimension != 2 || e.x != e.y || desc.layer_count < 6) {
        return Err(INVALID);
    }
    Ok(ty)
}

fn copy_pitches(
    e: Extent,
    layers: u32,
    texel: u64,
    row: u64,
    slice: u64,
) -> Result<(u32, u32, u64), Status> {
    if e.x == 0 || e.y == 0 || e.z == 0 || layers == 0 || texel == 0 {
        return Err(INVALID);
    }
    let tight_row = u64::from(e.x).checked_mul(texel).ok_or(INVALID)?;
    let row = if row == 0 { tight_row } else { row };
    if row % texel != 0 || row < tight_row {
        return Err(INVALID);
    }
    if row > i32::MAX as u64 {
        return Err(UNSUPPORTED);
    }
    let tight_slice = row.checked_mul(u64::from(e.y)).ok_or(INVALID)?;
    let slice = if slice == 0 { tight_slice } else { slice };
    if slice % row != 0 || slice < tight_slice {
        return Err(INVALID);
    }
    let slices = u64::from(layers) * u64::from(e.z);
    let size = (slices - 1)
        .checked_mul(slice)
        .and_then(|v| v.checked_add(tight_slice - row))
        .and_then(|v| v.checked_add(tight_row))
        .ok_or(INVALID)?;
    Ok((
        u32::try_from(row / texel).map_err(|_| UNSUPPORTED)?,
        u32::try_from(slice / row).map_err(|_| UNSUPPORTED)?,
        size,
    ))
}

struct Prepared {
    info: vk::VkImageCreateInfo,
    formats: Vec<vk::VkFormat>,
    domains: Vec<u32>,
    desc: ImageDesc,
    max_resource_size: u64,
}
impl Prepared {
    fn with_info<T>(&self, f: impl FnOnce(&vk::VkImageCreateInfo) -> T) -> T {
        let formats = vk::VkImageFormatListCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_FORMAT_LIST_CREATE_INFO,
            viewFormatCount: self.formats.len() as u32,
            pViewFormats: self.formats.as_ptr(),
            ..Default::default()
        };
        let mut info = self.info;
        info.pNext = if self.formats.is_empty() {
            ptr::null()
        } else {
            ptr::from_ref(&formats).cast()
        };
        info.pQueueFamilyIndices = self.domains.as_ptr();
        f(&info)
    }
}
pub struct Image {
    device: *const Device,
    handle: vk::VkImage,
    prepared: Prepared,
    requirements: vk::VkMemoryRequirements,
    dedicated: vk::VkMemoryDedicatedRequirements,
    bound: Cell<bool>,
}
pub struct View {
    device: *const Device,
    handle: vk::VkImageView,
}

impl Device {
    unsafe fn prepare_image(&self, desc: &ImageDesc) -> Result<Prepared, Status> {
        self.ready()?;
        desc.header.validate::<ImageDesc>(IMAGE_DESC)?;
        if desc.flags & !7 != 0 {
            return Err(UNSUPPORTED);
        }
        let base = format(desc.format)?;
        let native_usage = usage(desc.usage, base.aspects)?;
        let ty = dimensions(desc)?;
        // These require separately enabled features, not silently requested ones.
        if desc.sample_count > 1 && desc.usage & 8 != 0 {
            return Err(UNSUPPORTED);
        }
        if base.aspects != 1 && desc.dimension != 2 {
            return Err(UNSUPPORTED);
        }
        let mut formats = Vec::new();
        if desc.view_format_count != 0 {
            if desc.view_formats.is_null() {
                return Err(INVALID);
            }
            formats
                .try_reserve_exact(desc.view_format_count as usize)
                .map_err(|_| OUT_OF_MEMORY)?;
            for id in unsafe {
                std::slice::from_raw_parts(desc.view_formats, desc.view_format_count as usize)
            } {
                let f = format(*id)?;
                if f.class != base.class || (*id != desc.format && desc.flags & 1 == 0) {
                    return Err(INVALID);
                }
                if !formats.contains(&f.native) {
                    formats.push(f.native);
                }
            }
        }
        if desc.concurrent_domain_count == 1
            || desc.concurrent_domain_count as usize > self.snapshot.queues.len()
        {
            return Err(INVALID);
        }
        let domains = if desc.concurrent_domain_count == 0 {
            Vec::new()
        } else {
            if desc.concurrent_domains.is_null() {
                return Err(INVALID);
            }
            let domains = unsafe {
                std::slice::from_raw_parts(
                    desc.concurrent_domains,
                    desc.concurrent_domain_count as usize,
                )
            };
            for (i, domain) in domains.iter().enumerate() {
                if domains[..i].contains(domain) {
                    return Err(INVALID);
                }
                if !self.snapshot.queues.iter().any(|q| q.domain == *domain) {
                    return Err(UNSUPPORTED);
                }
            }
            domains.to_vec()
        };
        let flags = if desc.flags & 1 != 0 {
            vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_MUTABLE_FORMAT_BIT
        } else {
            0
        } | if desc.flags & 2 != 0 {
            vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_CUBE_COMPATIBLE_BIT
        } else {
            0
        } | if desc.flags & 4 != 0 {
            vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_ALIAS_BIT
        } else {
            0
        };
        let info = vk::VkImageCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
            flags,
            imageType: ty,
            format: base.native,
            extent: vk::VkExtent3D {
                width: desc.extent.x,
                height: desc.extent.y,
                depth: desc.extent.z,
            },
            mipLevels: desc.mip_count,
            arrayLayers: desc.layer_count,
            samples: desc.sample_count,
            tiling: vk::VkImageTiling_VK_IMAGE_TILING_OPTIMAL,
            usage: native_usage,
            sharingMode: if domains.is_empty() {
                vk::VkSharingMode_VK_SHARING_MODE_EXCLUSIVE
            } else {
                vk::VkSharingMode_VK_SHARING_MODE_CONCURRENT
            },
            queueFamilyIndexCount: domains.len() as u32,
            initialLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_UNDEFINED,
            ..Default::default()
        };
        let max_resource_size = self.check_image_support(&info)?;
        // No borrowed description-array pointers survive the synchronous call.
        let mut desc = *desc;
        desc.view_formats = ptr::null();
        desc.concurrent_domains = ptr::null();
        Ok(Prepared {
            info,
            formats,
            domains,
            desc,
            max_resource_size,
        })
    }
    fn check_image_support(&self, info: &vk::VkImageCreateInfo) -> Result<u64, Status> {
        let mut limits = vk::VkImageFormatProperties::default();
        let result = unsafe {
            (self.f.vkGetPhysicalDeviceImageFormatProperties.unwrap())(
                self.physical,
                info.format,
                info.imageType,
                info.tiling,
                info.usage,
                info.flags,
                &mut limits,
            )
        };
        if result == vk::VkResult_VK_ERROR_FORMAT_NOT_SUPPORTED {
            return Err(UNSUPPORTED);
        }
        self.result(result)?;
        if info.extent.width > limits.maxExtent.width
            || info.extent.height > limits.maxExtent.height
            || info.extent.depth > limits.maxExtent.depth
            || info.mipLevels > limits.maxMipLevels
            || info.arrayLayers > limits.maxArrayLayers
            || info.samples & limits.sampleCounts == 0
        {
            return Err(UNSUPPORTED);
        }
        Ok(limits.maxResourceSize)
    }
    fn image_native_requirements(
        &self,
        p: &Prepared,
    ) -> Result<(vk::VkMemoryRequirements, vk::VkMemoryDedicatedRequirements), Status> {
        let mut dedicated = vk::VkMemoryDedicatedRequirements {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_REQUIREMENTS,
            ..Default::default()
        };
        let mut req = vk::VkMemoryRequirements2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_REQUIREMENTS_2,
            pNext: ptr::from_mut(&mut dedicated).cast(),
            ..Default::default()
        };
        p.with_info(|info| {
            let query = vk::VkDeviceImageMemoryRequirements {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_IMAGE_MEMORY_REQUIREMENTS,
                pCreateInfo: info,
                ..Default::default()
            };
            unsafe {
                (self.f.vkGetDeviceImageMemoryRequirements.unwrap())(self.handle, &query, &mut req);
            }
        });
        if req.memoryRequirements.size == 0
            || !req.memoryRequirements.alignment.is_power_of_two()
            || req.memoryRequirements.size > self.snapshot.memory_limits.max_allocation_size
            || req.memoryRequirements.size > p.max_resource_size
        {
            return Err(UNSUPPORTED);
        }
        Ok((req.memoryRequirements, dedicated))
    }
    pub(in crate::foundation) unsafe fn image_requirements(
        &self,
        desc: &ImageDesc,
        out: &mut Requirements,
    ) -> Result<(), Status> {
        if out.compatible_type_capacity != 0 && out.compatible_types.is_null() {
            return Err(INVALID);
        }
        let p = unsafe { self.prepare_image(desc)? };
        let (req, dedicated) = self.image_native_requirements(&p)?;
        let mut types = [0u32; 32];
        let mut count = 0;
        for m in &self.snapshot.memory_types {
            if req.memoryTypeBits & (1u32 << m.id) != 0 && m.properties & (32 | 64 | 128) == 0 {
                types[count] = m.id;
                count += 1;
            }
        }
        if count == 0 {
            return Err(UNSUPPORTED);
        }
        out.size = req.size;
        out.alignment = req.alignment;
        out.dedicated_required = dedicated.requiresDedicatedAllocation;
        out.dedicated_preferred = dedicated.prefersDedicatedAllocation;
        out.compatible_type_count = count as u32;
        if out.compatible_type_capacity == 0 {
            return Ok(());
        }
        if out.compatible_type_capacity < count as u32 {
            return Err(CAPACITY);
        }
        unsafe {
            ptr::copy_nonoverlapping(types.as_ptr(), out.compatible_types, count);
        }
        Ok(())
    }
}
impl Image {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &ImageDesc,
    ) -> Result<Box<Self>, Status> {
        let prepared = unsafe { d.prepare_image(desc)? };
        let (requirements, dedicated) = d.image_native_requirements(&prepared)?;
        let mut handle = ptr::null_mut();
        prepared.with_info(|info| unsafe {
            d.result((d.f.vkCreateImage.unwrap())(
                d.handle,
                info,
                ptr::null(),
                &mut handle,
            ))
        })?;
        Ok(Box::new(Self {
            device: d,
            handle,
            prepared,
            requirements,
            dedicated,
            bound: Cell::new(false),
        }))
    }
    pub(in crate::foundation) unsafe fn bind(&self, placement: Span) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        if self.bound.get() {
            return Err(INVALID);
        }
        let memory = unsafe { placement.memory.as_ref() }.ok_or(INVALID)?;
        let handle = memory.image_placement(
            self.device,
            self.handle,
            self.requirements,
            self.dedicated.requiresDedicatedAllocation != 0,
            placement.offset,
            placement.size,
        )?;
        unsafe {
            d.result((d.f.vkBindImageMemory.unwrap())(
                d.handle,
                self.handle,
                handle,
                placement.offset,
            ))?;
        }
        self.bound.set(true);
        Ok(())
    }
    pub(in crate::foundation) fn dedicated_memory(
        &self,
        memory_type: u32,
    ) -> Result<Box<Memory>, Status> {
        if self.bound.get() {
            return Err(INVALID);
        }
        if memory_type >= 32 || self.requirements.memoryTypeBits & (1u32 << memory_type) == 0 {
            return Err(UNSUPPORTED);
        }
        Memory::dedicated_image(
            unsafe { &*self.device },
            self.handle,
            self.requirements.size,
            memory_type,
        )
    }
    pub(super) fn subresources(
        &self,
        range: Subresources,
    ) -> Result<vk::VkImageSubresourceRange, Status> {
        let desc = &self.prepared.desc;
        let aspects = format(desc.format)?.aspects;
        if range.aspects == 0
            || range.aspects & !aspects != 0
            || range.mip_count == 0
            || range.layer_count == 0
            || range.first_mip >= desc.mip_count
            || range.mip_count > desc.mip_count - range.first_mip
            || range.first_layer >= desc.layer_count
            || range.layer_count > desc.layer_count - range.first_layer
        {
            return Err(INVALID);
        }
        Ok(vk::VkImageSubresourceRange {
            aspectMask: range.aspects,
            baseMipLevel: range.first_mip,
            levelCount: range.mip_count,
            baseArrayLayer: range.first_layer,
            layerCount: range.layer_count,
        })
    }
    pub(super) fn command_handle(
        &self,
        device: *const Device,
        domain: u32,
        usage: u64,
    ) -> Result<vk::VkImage, Status> {
        if self.device != device
            || !self.bound.get()
            || self.prepared.desc.usage & usage != usage
            || (!self.prepared.domains.is_empty() && !self.prepared.domains.contains(&domain))
        {
            return Err(INVALID);
        }
        Ok(self.handle)
    }
    pub(super) fn layout(&self, state: u32) -> Result<vk::VkImageLayout, Status> {
        let usage = self.prepared.desc.usage;
        let aspects = format(self.prepared.desc.format)?.aspects;
        match state {
            0 => Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_UNDEFINED),
            1 => Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL),
            2 if usage & 1 != 0 => Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL),
            3 if usage & 2 != 0 => Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL),
            4 if usage & (4 | 128) != 0 => {
                Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL)
            }
            5 if usage & 16 != 0 && aspects == 1 => {
                Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL)
            }
            6 if usage & 32 != 0 && aspects != 1 => {
                Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            }
            7 if usage & (4 | 32 | 128) != 0 && aspects != 1 => {
                Ok(vk::VkImageLayout_VK_IMAGE_LAYOUT_DEPTH_STENCIL_READ_ONLY_OPTIMAL)
            }
            _ => Err(if state <= 7 { INVALID } else { UNSUPPORTED }),
        }
    }
    pub(super) fn barrier(
        &self,
        device: *const Device,
        domain: u32,
        b: &ImageBarrier,
    ) -> Result<vk::VkImageMemoryBarrier2, Status> {
        let handle = self.command_handle(device, domain, 0)?;
        if b.discard > 1 || b.new_state == 0 {
            return Err(INVALID);
        }
        let range = self.subresources(b.range)?;
        // Separate depth/stencil layout transitions need separateDepthStencilLayouts.
        if format(self.prepared.desc.format)?.aspects == 6 && range.aspectMask != 6 {
            return Err(UNSUPPORTED);
        }
        let d = unsafe { &*self.device };
        let (src, dst) = (b.source_domain, b.destination_domain);
        if (src != u32::MAX || dst != u32::MAX)
            && (src == u32::MAX
                || dst == u32::MAX
                || !self.prepared.domains.is_empty()
                || !d
                    .snapshot
                    .queues
                    .iter()
                    .any(|q| q.domain == src && q.count > 0)
                || !d
                    .snapshot
                    .queues
                    .iter()
                    .any(|q| q.domain == dst && q.count > 0)
                || (domain != src && domain != dst))
        {
            return Err(INVALID);
        }
        Ok(vk::VkImageMemoryBarrier2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER_2,
            oldLayout: self.layout(if b.discard != 0 { 0 } else { b.old_state })?,
            newLayout: self.layout(b.new_state)?,
            srcQueueFamilyIndex: src,
            dstQueueFamilyIndex: dst,
            image: handle,
            subresourceRange: range,
            ..Default::default()
        })
    }
    pub(super) fn record_clear(
        &self,
        device: *const Device,
        domain: u32,
        command: vk::VkCommandBuffer,
        state: u32,
        range: Subresources,
        value: ClearValue,
    ) -> Result<(), Status> {
        let handle = self.command_handle(device, domain, 2)?;
        if state != 1 && state != 3 {
            return Err(INVALID);
        }
        let range = self.subresources(range)?;
        let layout = self.layout(state)?;
        let d = unsafe { &*self.device };
        let flags = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == domain)
            .ok_or(INVALID)?
            .flags;
        if range.aspectMask == 1 {
            if flags & (GRAPHICS | COMPUTE) == 0 {
                return Err(UNSUPPORTED);
            }
            let color = vk::VkClearColorValue {
                uint32: unsafe { value.u32 },
            };
            unsafe {
                (d.f.vkCmdClearColorImage.unwrap())(command, handle, layout, &color, 1, &range);
            }
        } else {
            if flags & GRAPHICS == 0 {
                return Err(UNSUPPORTED);
            }
            let value = unsafe { value.depth_stencil };
            if !value.depth.is_finite() || !(0.0..=1.0).contains(&value.depth) {
                return Err(INVALID);
            }
            let clear = vk::VkClearDepthStencilValue {
                depth: value.depth,
                stencil: value.stencil,
            };
            unsafe {
                (d.f.vkCmdClearDepthStencilImage.unwrap())(
                    command, handle, layout, &clear, 1, &range,
                );
            }
        }
        Ok(())
    }
    pub(super) unsafe fn record_copy(
        &self,
        device: *const Device,
        domain: u32,
        command: vk::VkCommandBuffer,
        span: Span,
        copy: &ImageCopy,
        to_image: bool,
    ) -> Result<(), Status> {
        let image = self.command_handle(device, domain, if to_image { 2 } else { 1 })?;
        let state = if to_image { 3 } else { 2 };
        if copy.reserved != 0 || (copy.state != 1 && copy.state != state) {
            return Err(INVALID);
        }
        let desc = &self.prepared.desc;
        if desc.sample_count != 1 {
            return Err(INVALID);
        }
        let r = copy.region;
        self.subresources(Subresources {
            aspects: r.aspect,
            first_mip: r.mip,
            mip_count: 1,
            first_layer: r.first_layer,
            layer_count: r.layer_count,
        })?;
        if !r.aspect.is_power_of_two() {
            return Err(INVALID);
        }
        let d = unsafe { &*self.device };
        let queue = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == domain)
            .ok_or(INVALID)?;
        if r.aspect != 1 && queue.flags & GRAPHICS == 0 {
            return Err(UNSUPPORTED);
        }
        let texel = if r.aspect == 1 {
            u64::from(format(desc.format)?.class / 8)
        } else if r.aspect == 4 {
            1
        } else if desc.format == 13 {
            2
        } else {
            4
        };
        let extent = [r.extent.x, r.extent.y, r.extent.z];
        let offset = [r.offset.x, r.offset.y, r.offset.z];
        let full = [desc.extent.x, desc.extent.y, desc.extent.z].map(|v| (v >> r.mip).max(1));
        for i in 0..3 {
            if offset[i] < 0
                || extent[i] == 0
                || offset[i] as u32 >= full[i]
                || extent[i] > full[i] - offset[i] as u32
            {
                return Err(INVALID);
            }
            let granularity = queue.copy_granularity[i];
            if granularity == 0 {
                if offset[i] != 0 || extent[i] != full[i] {
                    return Err(UNSUPPORTED);
                }
            } else if offset[i] as u32 % granularity != 0
                || (extent[i] % granularity != 0 && offset[i] as u32 + extent[i] != full[i])
            {
                return Err(INVALID);
            }
        }
        let (row_texels, slice_rows, size) = copy_pitches(
            r.extent,
            r.layer_count,
            texel,
            copy.row_pitch,
            copy.slice_pitch,
        )?;
        if span.size < size {
            return Err(INVALID);
        }
        let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, address) = memory.command_range(
            device,
            domain,
            span.offset,
            span.size,
            if to_image { 1 } else { 2 },
        )?;
        // Address copies obey the same texel-block / depth-stencil alignment rules.
        let alignment = if r.aspect != 1 || queue.flags & (GRAPHICS | COMPUTE) == 0 {
            texel.max(4)
        } else {
            texel
        };
        if address % alignment != 0 {
            return Err(INVALID);
        }
        let region = vk::VkDeviceMemoryImageCopyKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_MEMORY_IMAGE_COPY_KHR,
            addressRange: vk::VkDeviceAddressRangeKHR {
                address,
                size: span.size,
            },
            addressFlags: memory.address_flags(),
            addressRowLength: row_texels,
            addressImageHeight: slice_rows,
            imageLayout: self.layout(copy.state)?,
            imageSubresource: vk::VkImageSubresourceLayers {
                aspectMask: r.aspect,
                mipLevel: r.mip,
                baseArrayLayer: r.first_layer,
                layerCount: r.layer_count,
            },
            imageOffset: vk::VkOffset3D {
                x: r.offset.x,
                y: r.offset.y,
                z: r.offset.z,
            },
            imageExtent: vk::VkExtent3D {
                width: r.extent.x,
                height: r.extent.y,
                depth: r.extent.z,
            },
            ..Default::default()
        };
        let info = vk::VkCopyDeviceMemoryImageInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_IMAGE_INFO_KHR,
            image,
            regionCount: 1,
            pRegions: &region,
            ..Default::default()
        };
        unsafe {
            (if to_image {
                d.f.vkCmdCopyMemoryToImageKHR
            } else {
                d.f.vkCmdCopyImageToMemoryKHR
            })
            .unwrap()(command, &info);
        }
        Ok(())
    }
}
impl Drop for Image {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.f.vkDestroyImage.unwrap())(d.handle, self.handle, ptr::null());
        }
    }
}
impl View {
    pub(in crate::foundation) fn create(
        image: &Image,
        desc: &ViewDesc,
    ) -> Result<Box<Self>, Status> {
        let d = unsafe { &*image.device };
        d.ready()?;
        desc.header.validate::<ViewDesc>(VIEW_DESC)?;
        if !image.bound.get() {
            return Err(INVALID);
        }
        let base = &image.prepared.desc;
        let f = format(desc.format)?;
        if desc.format != base.format
            && (base.flags & 1 == 0 || f.class != format(base.format)?.class)
        {
            return Err(INVALID);
        }
        if !image.prepared.formats.is_empty() && !image.prepared.formats.contains(&f.native) {
            return Err(INVALID);
        }
        if desc.usage & !188 != 0 || desc.usage == 0 || u64::from(desc.usage) & !base.usage != 0 {
            return Err(INVALID);
        }
        let native_usage = usage(desc.usage.into(), f.aspects)?;
        let range = image.subresources(desc.range)?;
        let native_type = match desc.dimension {
            1 if base.dimension == 1 && range.layerCount == 1 => {
                vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_1D
            }
            2 if base.dimension == 2 && range.layerCount == 1 => {
                vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_2D
            }
            3 if base.dimension == 3 => vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_3D,
            4 if base.flags & 2 != 0 && range.layerCount == 6 => {
                vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_CUBE
            }
            5 if base.dimension == 1 => vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_1D_ARRAY,
            6 if base.dimension == 2 => vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_2D_ARRAY,
            7 => return Err(UNSUPPORTED), // imageCubeArray is not yet an enabled capability.
            _ => return Err(INVALID),
        };
        if desc.component_mapping.iter().any(|c| *c > 6) {
            return Err(INVALID);
        }
        // Storage/attachment views require identity swizzles, not silently reset ones.
        if desc.usage & (8 | 16 | 32 | 128) != 0 && desc.component_mapping != [0; 4] {
            return Err(INVALID);
        }
        let mut support = image.prepared.info;
        support.format = f.native;
        support.usage = native_usage;
        d.check_image_support(&support)?;
        let view_usage = vk::VkImageViewUsageCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_VIEW_USAGE_CREATE_INFO,
            usage: native_usage,
            ..Default::default()
        };
        let c = desc.component_mapping;
        let info = vk::VkImageViewCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
            pNext: ptr::from_ref(&view_usage).cast(),
            image: image.handle,
            viewType: native_type,
            format: f.native,
            components: vk::VkComponentMapping {
                r: c[0],
                g: c[1],
                b: c[2],
                a: c[3],
            },
            subresourceRange: range,
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            d.result((d.f.vkCreateImageView.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Box::new(Self {
            device: image.device,
            handle,
        }))
    }
}
impl Drop for View {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.f.vkDestroyImageView.unwrap())(d.handle, self.handle, ptr::null());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::*;
    fn desc() -> ImageDesc {
        ImageDesc {
            header: Record::new::<ImageDesc>(IMAGE_DESC),
            format: 3,
            dimension: 2,
            mip_count: 1,
            layer_count: 1,
            sample_count: 1,
            extent: Extent { x: 16, y: 8, z: 1 },
            usage: 7,
            flags: 0,
            view_formats: ptr::null(),
            view_format_count: 0,
            concurrent_domain_count: 0,
            concurrent_domains: ptr::null(),
        }
    }
    #[test]
    fn shapes_formats_and_usage_are_exact() {
        let good = desc();
        assert_eq!(dimensions(&good), Ok(vk::VkImageType_VK_IMAGE_TYPE_2D));
        for bad in [
            ImageDesc {
                mip_count: 6,
                ..good
            },
            ImageDesc {
                mip_count: 0,
                ..good
            },
            ImageDesc {
                sample_count: 3,
                ..good
            },
            ImageDesc {
                dimension: 1,
                ..good
            },
            ImageDesc {
                dimension: 3,
                layer_count: 2,
                ..good
            },
            ImageDesc {
                flags: 2,
                layer_count: 6,
                ..good
            },
            ImageDesc {
                sample_count: 4,
                mip_count: 2,
                ..good
            },
        ] {
            assert_eq!(dimensions(&bad), Err(INVALID));
        }
        assert_eq!(format(3).unwrap().class, format(4).unwrap().class);
        assert_ne!(format(14).unwrap().class, format(11).unwrap().class);
        assert!(matches!(format(u32::MAX), Err(UNSUPPORTED)));
        assert_eq!(usage(16, 2), Err(INVALID));
        assert_eq!(usage(32, 1), Err(INVALID));
        assert_eq!(usage(64 | 4, 1), Err(INVALID));
        assert_eq!(usage(64 | 16, 1), Ok(80));
        assert_eq!(usage(256, 1), Err(UNSUPPORTED));
    }
    #[test]
    fn pitched_copy_extent_and_overflow() {
        let e = Extent { x: 3, y: 2, z: 1 };
        assert_eq!(copy_pitches(e, 2, 4, 20, 80), Ok((5, 4, 112)));
        assert_eq!(copy_pitches(e, 2, 4, 0, 0), Ok((3, 2, 48)));
        assert_eq!(copy_pitches(e, 2, 4, 13, 80), Err(INVALID));
        assert_eq!(copy_pitches(e, 2, 4, 20, 79), Err(INVALID));
        assert_eq!(copy_pitches(e, 2, 4, 20, 20), Err(INVALID));
        assert_eq!(copy_pitches(e, 2, 4, 1u64 << 31, 0), Err(UNSUPPORTED));
        assert_eq!(copy_pitches(e, 0, 4, 0, 0), Err(INVALID));
        let one = Extent { x: 1, y: 1, z: 1 };
        assert_eq!(
            copy_pitches(one, 1, 1, 1, u64::from(u32::MAX) + 1),
            Err(UNSUPPORTED)
        );
        assert_eq!(copy_pitches(one, 3, 1, 1, u64::MAX), Err(INVALID));
        assert_eq!(
            copy_pitches(Extent { z: 3, ..e }, 1, 4, 20, 80),
            Ok((5, 4, 192))
        );
    }
    thread_local! {
        static FAILURE: Cell<u32> = const { Cell::new(0) };
        static CREATES: Cell<u32> = const { Cell::new(0) };
        static ALLOCATIONS: Cell<u32> = const { Cell::new(0) };
        static CREATE: Cell<vk::PFN_vkCreateImage> = const { Cell::new(None) };
        static BIND: Cell<vk::PFN_vkBindImageMemory> = const { Cell::new(None) };
        static VIEW: Cell<vk::PFN_vkCreateImageView> = const { Cell::new(None) };
        static ALLOCATE: Cell<vk::PFN_vkAllocateMemory> = const { Cell::new(None) };
    }
    unsafe extern "C" fn create(
        d: vk::VkDevice,
        info: *const vk::VkImageCreateInfo,
        allocator: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkImage,
    ) -> vk::VkResult {
        CREATES.set(CREATES.get() + 1);
        if FAILURE.get() == 1 {
            unsafe {
                out.write(ptr::dangling_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { CREATE.get().unwrap()(d, info, allocator, out) }
    }
    unsafe extern "C" fn bind(
        d: vk::VkDevice,
        image: vk::VkImage,
        memory: vk::VkDeviceMemory,
        offset: u64,
    ) -> vk::VkResult {
        if FAILURE.get() == 2 {
            return vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY;
        }
        unsafe { BIND.get().unwrap()(d, image, memory, offset) }
    }
    unsafe extern "C" fn view(
        d: vk::VkDevice,
        info: *const vk::VkImageViewCreateInfo,
        allocator: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkImageView,
    ) -> vk::VkResult {
        if FAILURE.get() == 3 {
            unsafe {
                out.write(ptr::dangling_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { VIEW.get().unwrap()(d, info, allocator, out) }
    }
    unsafe extern "C" fn allocate(
        d: vk::VkDevice,
        info: *const vk::VkMemoryAllocateInfo,
        allocator: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkDeviceMemory,
    ) -> vk::VkResult {
        ALLOCATIONS.set(ALLOCATIONS.get() + 1);
        if FAILURE.get() == 4 {
            unsafe {
                out.write(ptr::dangling_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY;
        }
        unsafe { ALLOCATE.get().unwrap()(d, info, allocator, out) }
    }
    #[test]
    #[ignore = "requires modern Vulkan; image/view shapes, exact placement and failed-output cleanup"]
    fn gpu_foundation_images() {
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
                .find(|q| q.count > 0 && q.flags & (GRAPHICS | COMPUTE) != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let mut device = Device::create(
                &adapter,
                &[QueueRequest {
                    domain,
                    count: 1,
                    priority: 0.5,
                }],
                0,
            )
            .unwrap();
            CREATE.set(device.f.vkCreateImage);
            device.f.vkCreateImage = Some(create);
            BIND.set(device.f.vkBindImageMemory);
            device.f.vkBindImageMemory = Some(bind);
            VIEW.set(device.f.vkCreateImageView);
            device.f.vkCreateImageView = Some(view);
            ALLOCATE.set(device.f.vkAllocateMemory);
            device.f.vkAllocateMemory = Some(allocate);
            let d = ptr::from_ref(&*device).cast_mut();
            let desc = desc();
            unsafe {
                let mut req = Requirements::default();
                CREATES.set(0);
                ALLOCATIONS.set(0);
                FAILURE.set(0);
                assert_eq!(ogpu_next_image_requirements(d, &desc, &mut req), OK);
                assert_eq!((CREATES.get(), ALLOCATIONS.get()), (0, 0));
                let mut types = vec![0; req.compatible_type_count as usize];
                req.compatible_type_capacity = types.len() as u32;
                req.compatible_types = types.as_mut_ptr();
                assert_eq!(ogpu_next_image_requirements(d, &desc, &mut req), OK);
                if types.len() > 1 {
                    let mut sentinel = u32::MAX;
                    let mut short = Requirements {
                        compatible_type_capacity: 1,
                        compatible_types: &mut sentinel,
                        ..Default::default()
                    };
                    assert_eq!(ogpu_next_image_requirements(d, &desc, &mut short), CAPACITY);
                    assert_eq!(sentinel, u32::MAX);
                    assert_eq!(short.size, req.size);
                }
                let mut output = ptr::dangling_mut();
                FAILURE.set(1);
                assert_eq!(
                    ogpu_next_image_create_unbound(d, &desc, &mut output),
                    OUT_OF_MEMORY
                );
                assert!(output.is_null());
                FAILURE.set(0);
                let image = Image::create(&device, &desc).unwrap();
                FAILURE.set(4);
                let mut memory = ptr::dangling_mut();
                assert_eq!(
                    ogpu_next_memory_create_dedicated_image(
                        ptr::from_ref(&*image).cast_mut(),
                        types[0],
                        &mut memory
                    ),
                    OUT_OF_MEMORY
                );
                assert!(memory.is_null());
                FAILURE.set(0);
                let memory = image.dedicated_memory(types[0]).unwrap();
                let span = Span {
                    memory: ptr::from_ref(&*memory).cast_mut(),
                    offset: 0,
                    size: req.size,
                };
                let other = Image::create(&device, &desc).unwrap();
                assert_eq!(other.bind(span), Err(INVALID));
                FAILURE.set(2);
                assert_eq!(image.bind(span), Err(OUT_OF_MEMORY));
                assert!(!image.bound.get());
                FAILURE.set(0);
                image.bind(span).unwrap();
                assert_eq!(image.bind(span), Err(INVALID));
                let vd = ViewDesc {
                    header: Record::new::<ViewDesc>(VIEW_DESC),
                    format: 3,
                    dimension: 2,
                    usage: 4,
                    component_mapping: [0; 4],
                    range: Subresources {
                        aspects: 1,
                        first_mip: 0,
                        mip_count: 1,
                        first_layer: 0,
                        layer_count: 1,
                    },
                };
                FAILURE.set(3);
                let mut output = ptr::dangling_mut();
                assert_eq!(
                    ogpu_next_view_create(ptr::from_ref(&*image).cast_mut(), &vd, &mut output),
                    OUT_OF_MEMORY
                );
                assert!(output.is_null());
                FAILURE.set(0);
                drop(View::create(&image, &vd).unwrap());
                drop(other);
                drop(image);
                drop(memory);
            }
            // Native shape/interpretation coverage, not shader-sampling claims.
            for (dimension, extent, layers, mips, flags, format, usage, view_type, samples) in [
                (1, [32, 1, 1], 3, 6, 0, 3, 7, 5, 1),
                (3, [16, 8, 4], 1, 5, 0, 3, 7, 3, 1),
                (2, [16, 16, 1], 12, 5, 2, 3, 7, 4, 1),
                (2, [16, 8, 1], 2, 1, 0, 3, 16, 6, 4),
                (2, [16, 8, 1], 1, 1, 0, 13, 39, 2, 1),
                (2, [16, 8, 1], 1, 1, 0, 14, 39, 2, 1),
                (2, [16, 8, 1], 1, 1, 0, 15, 39, 2, 1),
                (2, [16, 8, 1], 1, 1, 0, 16, 39, 2, 1),
            ] {
                let desc = ImageDesc {
                    dimension,
                    extent: Extent {
                        x: extent[0],
                        y: extent[1],
                        z: extent[2],
                    },
                    layer_count: layers,
                    mip_count: mips,
                    flags,
                    format,
                    usage,
                    sample_count: samples,
                    ..desc
                };
                let image = match unsafe { Image::create(&device, &desc) } {
                    Ok(image) => image,
                    Err(UNSUPPORTED) => {
                        eprintln!("Image shape not supported: format={format} dim={dimension} samples={samples}");
                        continue;
                    }
                    Err(e) => panic!("image creation: {e}"),
                };
                let ty = device
                    .snapshot
                    .memory_types
                    .iter()
                    .find(|m| {
                        image.requirements.memoryTypeBits & (1 << m.id) != 0
                            && m.properties & (32 | 64 | 128) == 0
                    })
                    .unwrap()
                    .id;
                let memory = image.dedicated_memory(ty).unwrap();
                unsafe {
                    image
                        .bind(Span {
                            memory: ptr::from_ref(&*memory).cast_mut(),
                            offset: 0,
                            size: image.requirements.size,
                        })
                        .unwrap();
                }
                let vd = ViewDesc {
                    header: Record::new::<ViewDesc>(VIEW_DESC),
                    format,
                    dimension: view_type,
                    usage: if usage == 16 { 16 } else { 4 },
                    component_mapping: [0; 4],
                    range: Subresources {
                        aspects: if format >= 13 { 2 } else { 1 },
                        first_mip: 0,
                        mip_count: mips,
                        first_layer: if flags == 2 { 6 } else { 0 },
                        layer_count: if flags == 2 { 6 } else { layers },
                    },
                };
                drop(View::create(&image, &vd).unwrap());
                drop(image);
                drop(memory);
            }
            tested += 1;
        }
        assert!(tested != 0, "No suitable GPU, not a passing skip");
    }
}
