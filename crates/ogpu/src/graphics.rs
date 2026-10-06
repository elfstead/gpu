//! Explicit images, attachment scopes and prepared indexed/depth raster execution.
use super::*;

const FORMAT: vk::VkFormat = vk::VkFormat_VK_FORMAT_R8G8B8A8_UNORM;

pub(crate) const SAMPLED: u32 = 1;
pub(crate) const STORAGE: u32 = 2;
pub(crate) const COLOR: u32 = 4;
pub(crate) const COPY_SRC: u32 = 8;
pub(crate) const COPY_DST: u32 = 16;
pub(crate) const DEPTH: u32 = 32;
pub(crate) const D32: u32 = 4;
pub(crate) const NO_FORMAT: u32 = u32::MAX;

pub use crate::api_types::OgpuImageDesc as ImageDesc;

impl ImageDesc {
    #[cfg(test)]
    pub(crate) fn rgba8(width: u32, height: u32) -> Self {
        Self {
            dimension: 2,
            width,
            height,
            format: 0,
            usage: SAMPLED | STORAGE | COLOR | COPY_SRC | COPY_DST,
            reserved: 0,
        }
    }

    fn validate(&self, limits: &vk::VkPhysicalDeviceLimits) -> Result<usize, Error> {
        if !matches!(self.dimension, 1 | 2)
            || self.format > D32
            || (self.format == 3 && self.usage & (STORAGE | COLOR) != 0)
            || (self.format == D32
                && (self.dimension != 2
                    || self.usage & DEPTH == 0
                    || self.usage & !(DEPTH | COPY_SRC | COPY_DST) != 0))
            || (self.format != D32 && self.usage & DEPTH != 0)
            || self.reserved != 0
            || self.usage == 0
            || self.usage & !(SAMPLED | STORAGE | COLOR | COPY_SRC | COPY_DST | DEPTH) != 0
            || self.width == 0
            || self.height == 0
            || (self.dimension == 1
                && (self.height != 1 || self.width > limits.maxImageDimension1D))
            || (self.dimension == 2
                && (self.width > limits.maxImageDimension2D
                    || self.height > limits.maxImageDimension2D))
            || (self.usage & COLOR != 0 && (self.dimension != 2 || self.format == 1))
        {
            return Err(Error::new(INVALID_ARGUMENT, "Invalid image description"));
        }
        if self.usage & (COLOR | DEPTH) != 0 {
            target_size(self.width, self.height, limits)?;
        }
        (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|n| n.checked_mul(self.texel_size()))
            .filter(|&n| n <= isize::MAX as usize)
            .ok_or_else(|| Error::new(INVALID_ARGUMENT, "Image byte size overflow"))
    }

    pub(super) fn vk_format(&self) -> vk::VkFormat {
        match self.format {
            0 => FORMAT,
            1 => vk::VkFormat_VK_FORMAT_R32_SFLOAT,
            2 => vk::VkFormat_VK_FORMAT_R16G16B16A16_SFLOAT,
            3 => vk::VkFormat_VK_FORMAT_R16G16B16A16_UNORM,
            D32 => vk::VkFormat_VK_FORMAT_D32_SFLOAT,
            _ => unreachable!("validated image format"),
        }
    }

    pub(super) fn texel_size(&self) -> usize {
        if matches!(self.format, 2 | 3) {
            8
        } else {
            4
        }
    }

    fn aspect(&self) -> vk::VkImageAspectFlags {
        if self.format == D32 {
            vk::VkImageAspectFlagBits_VK_IMAGE_ASPECT_DEPTH_BIT
        } else {
            vk::VkImageAspectFlagBits_VK_IMAGE_ASPECT_COLOR_BIT
        }
    }

    pub(super) fn view_type(&self) -> vk::VkImageViewType {
        if self.dimension == 1 {
            vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_1D
        } else {
            vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_2D
        }
    }

    fn vk_usage(&self) -> vk::VkImageUsageFlags {
        [
            (SAMPLED, vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_SAMPLED_BIT),
            (
                DEPTH,
                vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT,
            ),
            (STORAGE, vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_STORAGE_BIT),
            (
                COLOR,
                vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
            ),
            (
                COPY_SRC,
                vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_TRANSFER_SRC_BIT,
            ),
            (
                COPY_DST,
                vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_TRANSFER_DST_BIT,
            ),
        ]
        .into_iter()
        .filter(|(bit, _)| self.usage & bit != 0)
        .fold(0, |flags, (_, flag)| flags | flag)
    }
}

fn ready(device: &Device) -> Result<(), Error> {
    device.ready()?;
    if !device.graphics {
        return Err(Error::new(
            UNSUPPORTED,
            "Execution queue does not support graphics",
        ));
    }
    Ok(())
}

fn target_size(
    width: u32,
    height: u32,
    limits: &vk::VkPhysicalDeviceLimits,
) -> Result<usize, Error> {
    if width == 0
        || height == 0
        || width > limits.maxImageDimension2D
        || height > limits.maxImageDimension2D
        || width > limits.maxFramebufferWidth
        || height > limits.maxFramebufferHeight
        || width > limits.maxViewportDimensions[0]
        || height > limits.maxViewportDimensions[1]
        || width as f32 > limits.viewportBoundsRange[1]
        || height as f32 > limits.viewportBoundsRange[1]
    {
        return Err(Error::new(
            INVALID_ARGUMENT,
            "Invalid or unsupported target extent",
        ));
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .filter(|&n| n <= isize::MAX as usize)
        .ok_or_else(|| Error::new(INVALID_ARGUMENT, "Image byte size overflow"))
}

fn image_memory_type(memory: &vk::VkPhysicalDeviceMemoryProperties, mask: u32) -> Option<u32> {
    (0..memory.memoryTypeCount)
        .filter(|&i| {
            let flags = memory.memoryTypes[i as usize].propertyFlags;
            mask & (1 << i) != 0
                && flags
                    & (vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD
                        | vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_PROTECTED_BIT
                        | vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT)
                    == 0
        })
        .max_by_key(|&i| {
            let flags = memory.memoryTypes[i as usize].propertyFlags;
            let local =
                flags & vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT != 0;
            let visible =
                flags & vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT != 0;
            // Images are never mapped by this runtime. Prefer device-only local
            // memory over a potentially small host-visible VRAM heap, but retain
            // visible local memory on UMA and when required by the type mask.
            (local, !visible)
        })
}

pub(crate) struct Image {
    pub(super) device: Rc<Device>,
    pub(super) size: usize,
    pub(super) desc: ImageDesc,
    pub(super) image: vk::VkImage,
    memory: vk::VkDeviceMemory,
    view: vk::VkImageView,
}

impl Image {
    // Shared preflight for allocation-free queries and actual creation.
    pub(crate) fn check_support(device: &Device, desc: ImageDesc) -> Result<usize, Error> {
        device.ready()?;
        let size = desc.validate(&device.limits)?;
        if desc.usage & (COLOR | DEPTH) != 0 {
            ready(device)?;
        }
        let (width, height) = (desc.width, desc.height);
        let usage = desc.vk_usage();
        let image_type = if desc.dimension == 1 {
            vk::VkImageType_VK_IMAGE_TYPE_1D
        } else {
            vk::VkImageType_VK_IMAGE_TYPE_2D
        };
        // Sampled images promise both nearest and linear filtering, independent of
        // which sampler is later bound. Validate that promise on the actual format.
        if desc.usage & SAMPLED != 0 {
            let mut properties = vk::VkFormatProperties::default();
            unsafe {
                (device.f.vkGetPhysicalDeviceFormatProperties.unwrap())(
                    device.physical,
                    desc.vk_format(),
                    &mut properties,
                );
            }
            if properties.optimalTilingFeatures
                & vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT
                == 0
            {
                return Err(Error::new(
                    UNSUPPORTED,
                    "Image format does not support linear sampling",
                ));
            }
        }
        let mut format = vk::VkImageFormatProperties::default();
        // SAFETY: physical device belongs to our retained instance and output is writable.
        let status = unsafe {
            (device.f.vkGetPhysicalDeviceImageFormatProperties.unwrap())(
                device.physical,
                desc.vk_format(),
                image_type,
                vk::VkImageTiling_VK_IMAGE_TILING_OPTIMAL,
                usage,
                0,
                &mut format,
            )
        };
        if status == vk::VkResult_VK_ERROR_FORMAT_NOT_SUPPORTED {
            return Err(Error::new(
                UNSUPPORTED,
                "Image format/dimension/usage combination unsupported",
            ));
        }
        device.result("vkGetPhysicalDeviceImageFormatProperties", status)?;
        if width > format.maxExtent.width
            || height > format.maxExtent.height
            || format.sampleCounts & vk::VkSampleCountFlagBits_VK_SAMPLE_COUNT_1_BIT == 0
        {
            return Err(Error::new(
                UNSUPPORTED,
                "Unsupported image extent or sample count",
            ));
        }
        Ok(size)
    }

    pub(crate) fn new(device: Rc<Device>, desc: ImageDesc) -> Result<Self, Error> {
        let size = Self::check_support(&device, desc)?;
        let (width, height) = (desc.width, desc.height);
        let usage = desc.vk_usage();
        let image_type = if desc.dimension == 1 {
            vk::VkImageType_VK_IMAGE_TYPE_1D
        } else {
            vk::VkImageType_VK_IMAGE_TYPE_2D
        };
        let mut result = Self {
            device,
            size,
            desc,
            image: ptr::null_mut(),
            memory: ptr::null_mut(),
            view: ptr::null_mut(),
        };
        let d = &result.device;
        // SAFETY: result owns handles immediately; every subsequent failure runs ordered cleanup.
        unsafe {
            let create = vk::VkImageCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
                imageType: image_type,
                format: desc.vk_format(),
                extent: vk::VkExtent3D {
                    width,
                    height,
                    depth: 1,
                },
                mipLevels: 1,
                arrayLayers: 1,
                samples: vk::VkSampleCountFlagBits_VK_SAMPLE_COUNT_1_BIT,
                tiling: vk::VkImageTiling_VK_IMAGE_TILING_OPTIMAL,
                usage,
                sharingMode: vk::VkSharingMode_VK_SHARING_MODE_EXCLUSIVE,
                initialLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_UNDEFINED,
                ..Default::default()
            };
            d.result(
                "vkCreateImage",
                (d.f.vkCreateImage.unwrap())(d.handle, &create, ptr::null(), &mut result.image),
            )?;
            let mut requirements = vk::VkMemoryRequirements::default();
            (d.f.vkGetImageMemoryRequirements.unwrap())(d.handle, result.image, &mut requirements);
            let index = image_memory_type(&d.memory, requirements.memoryTypeBits)
                .ok_or_else(|| Error::new(UNSUPPORTED, "No suitable image memory type"))?;
            let dedicated = vk::VkMemoryDedicatedAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO,
                image: result.image,
                ..Default::default()
            };
            let allocate = vk::VkMemoryAllocateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                pNext: (&dedicated as *const vk::VkMemoryDedicatedAllocateInfo).cast(),
                allocationSize: requirements.size,
                memoryTypeIndex: index,
            };
            d.result(
                "vkAllocateMemory(image)",
                (d.f.vkAllocateMemory.unwrap())(
                    d.handle,
                    &allocate,
                    ptr::null(),
                    &mut result.memory,
                ),
            )?;
            d.result(
                "vkBindImageMemory",
                (d.f.vkBindImageMemory.unwrap())(d.handle, result.image, result.memory, 0),
            )?;
            let view = vk::VkImageViewCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
                image: result.image,
                viewType: desc.view_type(),
                format: desc.vk_format(),
                subresourceRange: vk::VkImageSubresourceRange {
                    aspectMask: desc.aspect(),
                    baseMipLevel: 0,
                    levelCount: 1,
                    baseArrayLayer: 0,
                    layerCount: 1,
                },
                ..Default::default()
            };
            if desc.usage & (COLOR | DEPTH) != 0 {
                d.result(
                    "vkCreateImageView",
                    (d.f.vkCreateImageView.unwrap())(
                        d.handle,
                        &view,
                        ptr::null(),
                        &mut result.view,
                    ),
                )?;
            }
        }
        Ok(result)
    }

    pub(super) unsafe fn discard(&self, command: vk::VkCommandBuffer) {
        // Explicit discard needs no tracked previous layout. Order all earlier uses
        // before initialization, then make GENERAL available to graphics or compute.
        let initialize = vk::VkImageMemoryBarrier2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER_2,
            srcStageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
            srcAccessMask: vk::VK_ACCESS_2_MEMORY_READ_BIT | vk::VK_ACCESS_2_MEMORY_WRITE_BIT,
            dstStageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
            dstAccessMask: vk::VK_ACCESS_2_MEMORY_READ_BIT | vk::VK_ACCESS_2_MEMORY_WRITE_BIT,
            oldLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_UNDEFINED,
            newLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL,
            srcQueueFamilyIndex: vk::VK_QUEUE_FAMILY_IGNORED as u32,
            dstQueueFamilyIndex: vk::VK_QUEUE_FAMILY_IGNORED as u32,
            image: self.image,
            subresourceRange: vk::VkImageSubresourceRange {
                aspectMask: self.desc.aspect(),
                levelCount: 1,
                layerCount: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let dependency = vk::VkDependencyInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
            imageMemoryBarrierCount: 1,
            pImageMemoryBarriers: &initialize,
            ..Default::default()
        };
        unsafe {
            (self.device.f.vkCmdPipelineBarrier2.unwrap())(command, &dependency);
        }
    }

    pub(super) unsafe fn copy(
        &self,
        command: vk::VkCommandBuffer,
        buffer: &Buffer,
        offset: u64,
        to_image: bool,
    ) {
        let region = vk::VkDeviceMemoryImageCopyKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_MEMORY_IMAGE_COPY_KHR,
            addressRange: vk::VkDeviceAddressRangeKHR {
                address: buffer.address + offset,
                size: self.size as u64,
            },
            addressFlags: vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
            imageLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL,
            imageSubresource: vk::VkImageSubresourceLayers {
                aspectMask: self.desc.aspect(),
                mipLevel: 0,
                baseArrayLayer: 0,
                layerCount: 1,
            },
            imageExtent: vk::VkExtent3D {
                width: self.desc.width,
                height: self.desc.height,
                depth: 1,
            },
            ..Default::default()
        };
        let copy = vk::VkCopyDeviceMemoryImageInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_IMAGE_INFO_KHR,
            image: self.image,
            regionCount: 1,
            pRegions: &region,
            ..Default::default()
        };
        // SAFETY: caller has initialized GENERAL in this or an earlier submission.
        // Uploads order prior reads too, allowing a previously sampled image to be replaced.
        unsafe {
            batch::barrier(
                &self.device,
                command,
                vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
                vk::VK_PIPELINE_STAGE_2_TRANSFER_BIT,
                vk::VK_ACCESS_2_MEMORY_WRITE_BIT
                    | if to_image {
                        vk::VK_ACCESS_2_MEMORY_READ_BIT
                    } else {
                        0
                    },
                vk::VK_ACCESS_2_TRANSFER_READ_BIT | vk::VK_ACCESS_2_TRANSFER_WRITE_BIT,
            );
            if to_image {
                (self.device.f.vkCmdCopyMemoryToImageKHR.unwrap())(command, &copy);
            } else {
                (self.device.f.vkCmdCopyImageToMemoryKHR.unwrap())(command, &copy);
            }
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        // SAFETY: recorded/submitted uses retain the target until commands are destroyed.
        unsafe {
            let d = &self.device;
            if !self.view.is_null() {
                (d.f.vkDestroyImageView.unwrap())(d.handle, self.view, ptr::null());
            }
            if !self.image.is_null() {
                (d.f.vkDestroyImage.unwrap())(d.handle, self.image, ptr::null());
            }
            if !self.memory.is_null() {
                (d.f.vkFreeMemory.unwrap())(d.handle, self.memory, ptr::null());
            }
        }
    }
}

pub(crate) struct Raster {
    pub(super) device: Rc<Device>,
    pub(super) push_size: u32,
    pub(super) target_format: u32,
    pub(super) depth_format: u32,
    modules: [vk::VkShaderModule; 2],
    pipeline: vk::VkPipeline,
}

impl Raster {
    /// # Safety
    /// Valid matching vertex/fragment SPIR-V main entries, no descriptor-set bindings, only
    /// the enabled baseline (including native heaps), read-only storage, matching
    /// root layout, and valid bound table indices for any heap accesses.
    pub(crate) unsafe fn new(
        device: Rc<Device>,
        vertex: &[u32],
        fragment: &[u32],
        desc: crate::OgpuRasterDesc,
        constants: [&[SpecializationConstant]; 2],
    ) -> Result<Self, Error> {
        ready(&device)?;
        let crate::OgpuRasterDesc {
            push_size_bytes: push_size,
            topology,
            color_format: target_format,
            depth_format,
            depth_test,
            depth_write,
            depth_compare,
            reserved,
        } = desc;
        if !matches!(target_format, 0 | 2)
            || !matches!(depth_format, D32 | NO_FORMAT)
            || depth_test > 1
            || depth_write > 1
            || depth_compare > 7
            || reserved != 0
            || (depth_format == NO_FORMAT
                && (depth_test != 0 || depth_write != 0 || depth_compare != 7))
        {
            return Err(Error::new(INVALID_ARGUMENT, "Invalid raster description"));
        }
        if depth_format == D32 {
            Image::check_support(
                &device,
                ImageDesc {
                    dimension: 2,
                    width: 1,
                    height: 1,
                    format: D32,
                    usage: DEPTH,
                    reserved: 0,
                },
            )?;
        }
        // The executable fixes attachment format, independently of any image.
        Image::check_support(
            &device,
            ImageDesc {
                dimension: 2,
                width: 1,
                height: 1,
                format: target_format,
                usage: COLOR,
                reserved: 0,
            },
        )?;
        let format = ImageDesc {
            dimension: 2,
            width: 1,
            height: 1,
            format: target_format,
            usage: COLOR,
            reserved: 0,
        }
        .vk_format();
        let topology = match topology {
            0 => vk::VkPrimitiveTopology_VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST,
            1 => vk::VkPrimitiveTopology_VK_PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP,
            _ => return Err(Error::new(INVALID_ARGUMENT, "Invalid raster topology")),
        };
        let specialization = [
            Specialization::new(constants[0])?,
            Specialization::new(constants[1])?,
        ];
        let specialization_info = specialization.each_ref().map(Specialization::info);
        if [vertex, fragment]
            .iter()
            .any(|s| s.len() < 5 || s[0] != 0x07230203)
            || push_size % 4 != 0
            || u64::from(push_size) > device.max_push_data
        {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Invalid raster SPIR-V header or root size",
            ));
        }
        let mut result = Self {
            device,
            push_size,
            target_format,
            depth_format,
            modules: [ptr::null_mut(); 2],
            pipeline: ptr::null_mut(),
        };
        let d = &result.device;
        // SAFETY: shader semantics are the caller's contract. All state is initialized
        // locally, retained through creation, and every partial resource is owned.
        unsafe {
            for (index, words) in [vertex, fragment].iter().enumerate() {
                let create = vk::VkShaderModuleCreateInfo {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
                    codeSize: std::mem::size_of_val(*words),
                    pCode: words.as_ptr(),
                    ..Default::default()
                };
                d.result(
                    "vkCreateShaderModule(raster)",
                    (d.f.vkCreateShaderModule.unwrap())(
                        d.handle,
                        &create,
                        ptr::null(),
                        &mut result.modules[index],
                    ),
                )?;
            }
            let stages = [
                vk::VkShaderStageFlagBits_VK_SHADER_STAGE_VERTEX_BIT,
                vk::VkShaderStageFlagBits_VK_SHADER_STAGE_FRAGMENT_BIT,
            ]
            .map(|stage| vk::VkPipelineShaderStageCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                stage,
                module: result.modules
                    [usize::from(stage == vk::VkShaderStageFlagBits_VK_SHADER_STAGE_FRAGMENT_BIT)],
                pName: c"main".as_ptr(),
                pSpecializationInfo: &specialization_info
                    [usize::from(stage == vk::VkShaderStageFlagBits_VK_SHADER_STAGE_FRAGMENT_BIT)],
                ..Default::default()
            });
            let vertex_input = vk::VkPipelineVertexInputStateCreateInfo {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
                ..Default::default()
            };
            let assembly = vk::VkPipelineInputAssemblyStateCreateInfo {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
                topology,
                ..Default::default()
            };
            let viewport = vk::VkPipelineViewportStateCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO,
                viewportCount: 1,
                scissorCount: 1,
                ..Default::default()
            };
            let raster = vk::VkPipelineRasterizationStateCreateInfo {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
                polygonMode: vk::VkPolygonMode_VK_POLYGON_MODE_FILL,
                frontFace: vk::VkFrontFace_VK_FRONT_FACE_COUNTER_CLOCKWISE,
                lineWidth: 1.0,
                ..Default::default()
            };
            let multisample = vk::VkPipelineMultisampleStateCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
                rasterizationSamples: vk::VkSampleCountFlagBits_VK_SAMPLE_COUNT_1_BIT,
                ..Default::default()
            };
            let blend_attachment = vk::VkPipelineColorBlendAttachmentState {
                colorWriteMask: vk::VkColorComponentFlagBits_VK_COLOR_COMPONENT_R_BIT
                    | vk::VkColorComponentFlagBits_VK_COLOR_COMPONENT_G_BIT
                    | vk::VkColorComponentFlagBits_VK_COLOR_COMPONENT_B_BIT
                    | vk::VkColorComponentFlagBits_VK_COLOR_COMPONENT_A_BIT,
                ..Default::default()
            };
            let blend = vk::VkPipelineColorBlendStateCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
                attachmentCount: 1,
                pAttachments: &blend_attachment,
                ..Default::default()
            };
            let states = [
                vk::VkDynamicState_VK_DYNAMIC_STATE_VIEWPORT,
                vk::VkDynamicState_VK_DYNAMIC_STATE_SCISSOR,
            ];
            let dynamic = vk::VkPipelineDynamicStateCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO,
                dynamicStateCount: 2,
                pDynamicStates: states.as_ptr(),
                ..Default::default()
            };
            let flags = vk::VkPipelineCreateFlags2CreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO,
                flags: vk::VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT,
                ..Default::default()
            };
            let depth = vk::VkPipelineDepthStencilStateCreateInfo {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
                depthTestEnable: depth_test,
                depthWriteEnable: depth_write,
                depthCompareOp: depth_compare as vk::VkCompareOp,
                ..Default::default()
            };
            let rendering = vk::VkPipelineRenderingCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RENDERING_CREATE_INFO,
                pNext: (&flags as *const vk::VkPipelineCreateFlags2CreateInfo).cast(),
                colorAttachmentCount: 1,
                pColorAttachmentFormats: &format,
                depthAttachmentFormat: if depth_format == D32 {
                    vk::VkFormat_VK_FORMAT_D32_SFLOAT
                } else {
                    vk::VkFormat_VK_FORMAT_UNDEFINED
                },
                ..Default::default()
            };
            let create = vk::VkGraphicsPipelineCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO,
                stageCount: 2,
                pStages: stages.as_ptr(),
                pVertexInputState: &vertex_input,
                pInputAssemblyState: &assembly,
                pViewportState: &viewport,
                pRasterizationState: &raster,
                pMultisampleState: &multisample,
                pColorBlendState: &blend,
                pDepthStencilState: &depth,
                pDynamicState: &dynamic,
                pNext: (&rendering as *const vk::VkPipelineRenderingCreateInfo).cast(),
                basePipelineIndex: -1,
                ..Default::default()
            };
            d.result(
                "vkCreateGraphicsPipelines",
                (d.f.vkCreateGraphicsPipelines.unwrap())(
                    d.handle,
                    ptr::null_mut(),
                    1,
                    &create,
                    ptr::null(),
                    &mut result.pipeline,
                ),
            )?;
        }
        Ok(result)
    }
}

impl Raster {
    pub(super) unsafe fn draw(
        &self,
        command: vk::VkCommandBuffer,
        indirect: &batch::IndirectBinding,
        indices: Option<&batch::IndexBinding>,
        bind_state: bool,
    ) {
        let d = &self.device;
        if indirect.maximum == 0 {
            return;
        }
        unsafe {
            if bind_state {
                (d.f.vkCmdBindPipeline.unwrap())(
                    command,
                    vk::VkPipelineBindPoint_VK_PIPELINE_BIND_POINT_GRAPHICS,
                    self.pipeline,
                );
            }
            let draw = vk::VkDrawIndirect2InfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DRAW_INDIRECT_2_INFO_KHR,
                addressRange: vk::VkStridedDeviceAddressRangeKHR {
                    address: indirect.buffer.address + indirect.offset,
                    size: u64::from(indirect.maximum - 1) * u64::from(indirect.stride)
                        + if indices.is_some() { 20 } else { 16 },
                    stride: u64::from(indirect.stride),
                },
                addressFlags:
                    vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
                drawCount: indirect.maximum,
                ..Default::default()
            };
            if let Some(i) = indices.filter(|_| bind_state) {
                let binding = vk::VkBindIndexBuffer3InfoKHR {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_INDEX_BUFFER_3_INFO_KHR,
                    addressRange: vk::VkDeviceAddressRangeKHR {
                        address: i.buffer.address + i.offset,
                        size: i.size,
                    },
                    addressFlags:
                        vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
                    indexType: if i.format == 0 {
                        vk::VkIndexType_VK_INDEX_TYPE_UINT16
                    } else {
                        vk::VkIndexType_VK_INDEX_TYPE_UINT32
                    },
                    ..Default::default()
                };
                (d.f.vkCmdBindIndexBuffer3KHR.unwrap())(command, &binding);
            }
            if let Some(count) = &indirect.count {
                let counted = vk::VkDrawIndirectCount2InfoKHR {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DRAW_INDIRECT_COUNT_2_INFO_KHR,
                    addressRange: draw.addressRange,
                    addressFlags: draw.addressFlags,
                    countAddressRange: vk::VkDeviceAddressRangeKHR {
                        address: count.address + indirect.count_offset,
                        size: 4,
                    },
                    countAddressFlags: draw.addressFlags,
                    maxDrawCount: indirect.maximum,
                    ..Default::default()
                };
                if indices.is_some() {
                    (d.f.vkCmdDrawIndexedIndirectCount2KHR.unwrap())(command, &counted);
                } else {
                    (d.f.vkCmdDrawIndirectCount2KHR.unwrap())(command, &counted);
                }
            } else if indices.is_some() {
                (d.f.vkCmdDrawIndexedIndirect2KHR.unwrap())(command, &draw);
            } else {
                (d.f.vkCmdDrawIndirect2KHR.unwrap())(command, &draw);
            }
        }
    }
}

pub(super) struct Rendering {
    pub color: Rc<Image>,
    pub depth: Option<Rc<Image>>,
    pub desc: crate::OgpuRenderingDesc,
}

impl Rendering {
    pub fn new(
        device: &Rc<Device>,
        color: Rc<Image>,
        depth: Option<Rc<Image>>,
        desc: crate::OgpuRenderingDesc,
    ) -> Result<Self, Error> {
        ready(device)?;
        if !Rc::ptr_eq(device, &color.device) || color.desc.usage & COLOR == 0 {
            return Err(Error::new(INVALID_ARGUMENT, "Invalid color attachment"));
        }
        if let Some(d) = &depth {
            if !Rc::ptr_eq(device, &d.device)
                || d.desc.usage & DEPTH == 0
                || d.desc.width != color.desc.width
                || d.desc.height != color.desc.height
            {
                return Err(Error::new(INVALID_ARGUMENT, "Invalid depth attachment"));
            }
        }
        crate::contract::attachments(&desc)?;
        Ok(Self { color, depth, desc })
    }

    pub fn matches(&self, raster: &Raster) -> bool {
        raster.target_format == self.color.desc.format
            && raster.depth_format == self.depth.as_ref().map_or(NO_FORMAT, |d| d.desc.format)
    }

    pub unsafe fn begin(&self, command: vk::VkCommandBuffer) {
        let d = &self.color.device;
        let area = vk::VkRect2D {
            offset: vk::VkOffset2D { x: 0, y: 0 },
            extent: vk::VkExtent2D {
                width: self.color.desc.width,
                height: self.color.desc.height,
            },
        };
        let attachment = |image: &Image, load, store, clear_value| vk::VkRenderingAttachmentInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO,
            imageView: image.view,
            imageLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL,
            loadOp: match load {
                0 => vk::VkAttachmentLoadOp_VK_ATTACHMENT_LOAD_OP_CLEAR,
                1 => vk::VkAttachmentLoadOp_VK_ATTACHMENT_LOAD_OP_LOAD,
                _ => vk::VkAttachmentLoadOp_VK_ATTACHMENT_LOAD_OP_DONT_CARE,
            },
            storeOp: if store == 0 {
                vk::VkAttachmentStoreOp_VK_ATTACHMENT_STORE_OP_STORE
            } else {
                vk::VkAttachmentStoreOp_VK_ATTACHMENT_STORE_OP_DONT_CARE
            },
            clearValue: clear_value,
            ..Default::default()
        };
        let color = attachment(
            &self.color,
            self.desc.color.load,
            self.desc.color.store,
            vk::VkClearValue {
                color: vk::VkClearColorValue {
                    float32: self.desc.color.clear,
                },
            },
        );
        let depth = self.depth.as_ref().map(|image| {
            attachment(
                image,
                self.desc.depth.load,
                self.desc.depth.store,
                vk::VkClearValue {
                    depthStencil: vk::VkClearDepthStencilValue {
                        depth: self.desc.depth.clear,
                        stencil: 0,
                    },
                },
            )
        });
        let begin = vk::VkRenderingInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDERING_INFO,
            renderArea: area,
            layerCount: 1,
            colorAttachmentCount: 1,
            pColorAttachments: &color,
            pDepthAttachment: depth.as_ref().map_or(ptr::null(), |d| d),
            ..Default::default()
        };
        let viewport = vk::VkViewport {
            x: 0.0,
            y: 0.0,
            width: area.extent.width as f32,
            height: area.extent.height as f32,
            minDepth: 0.0,
            maxDepth: 1.0,
        };
        unsafe {
            (d.f.vkCmdBeginRendering.unwrap())(command, &begin);
            (d.f.vkCmdSetViewport.unwrap())(command, 0, 1, &viewport);
            (d.f.vkCmdSetScissor.unwrap())(command, 0, 1, &area);
        }
    }
}

impl Drop for Raster {
    fn drop(&mut self) {
        // SAFETY: recordings/completions retain this executable until GPU use is over.
        unsafe {
            let d = &self.device;
            if !self.pipeline.is_null() {
                (d.f.vkDestroyPipeline.unwrap())(d.handle, self.pipeline, ptr::null());
            }
            for module in self.modules {
                if !module.is_null() {
                    (d.f.vkDestroyShaderModule.unwrap())(d.handle, module, ptr::null());
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "graphics_tests.rs"]
mod tests;
