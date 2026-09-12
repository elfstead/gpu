//! Fixed-state RGBA8 offscreen rendering. Public targets are images, never pointers.
use super::*;

const FORMAT: vk::VkFormat = vk::VkFormat_VK_FORMAT_R8G8B8A8_UNORM;

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
        .ok_or_else(|| Error::new(INVALID_ARGUMENT, "Target byte size overflow"))
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
            memory.memoryTypes[i as usize].propertyFlags
                & vk::VkMemoryPropertyFlagBits_VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                != 0
        })
}

pub(crate) struct Target {
    pub(super) device: Rc<Device>,
    pub(super) size: usize,
    width: u32,
    height: u32,
    image: vk::VkImage,
    memory: vk::VkDeviceMemory,
    view: vk::VkImageView,
}

impl Target {
    pub(crate) fn new(device: Rc<Device>, width: u32, height: u32) -> Result<Self, Error> {
        ready(&device)?;
        let size = target_size(width, height, &device.limits)?;
        let usage = vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT
            | vk::VkImageUsageFlagBits_VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
        let mut format = vk::VkImageFormatProperties::default();
        // SAFETY: physical device belongs to our retained instance and output is writable.
        let status = unsafe {
            (device.f.vkGetPhysicalDeviceImageFormatProperties.unwrap())(
                device.physical,
                FORMAT,
                vk::VkImageType_VK_IMAGE_TYPE_2D,
                vk::VkImageTiling_VK_IMAGE_TILING_OPTIMAL,
                usage,
                0,
                &mut format,
            )
        };
        if status == vk::VkResult_VK_ERROR_FORMAT_NOT_SUPPORTED {
            return Err(Error::new(
                UNSUPPORTED,
                "RGBA8 attachment/readback format unsupported",
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
        let mut result = Self {
            device,
            size,
            width,
            height,
            image: ptr::null_mut(),
            memory: ptr::null_mut(),
            view: ptr::null_mut(),
        };
        let d = &result.device;
        // SAFETY: result owns handles immediately; every subsequent failure runs ordered cleanup.
        unsafe {
            let create = vk::VkImageCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
                imageType: vk::VkImageType_VK_IMAGE_TYPE_2D,
                format: FORMAT,
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
                viewType: vk::VkImageViewType_VK_IMAGE_VIEW_TYPE_2D,
                format: FORMAT,
                subresourceRange: vk::VkImageSubresourceRange {
                    aspectMask: vk::VkImageAspectFlagBits_VK_IMAGE_ASPECT_COLOR_BIT,
                    baseMipLevel: 0,
                    levelCount: 1,
                    baseArrayLayer: 0,
                    layerCount: 1,
                },
                ..Default::default()
            };
            d.result(
                "vkCreateImageView",
                (d.f.vkCreateImageView.unwrap())(d.handle, &view, ptr::null(), &mut result.view),
            )?;
        }
        Ok(result)
    }

    pub(super) unsafe fn draw(
        &self,
        command: vk::VkCommandBuffer,
        raster: &Raster,
        indirect: &Buffer,
        offset: u64,
        root: &[u8],
    ) {
        let d = &self.device;
        let area = vk::VkRect2D {
            offset: vk::VkOffset2D { x: 0, y: 0 },
            extent: vk::VkExtent2D {
                width: self.width,
                height: self.height,
            },
        };
        let clear = vk::VkClearValue {
            color: vk::VkClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        };
        let attachment = vk::VkRenderingAttachmentInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO,
            imageView: self.view,
            imageLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL,
            loadOp: vk::VkAttachmentLoadOp_VK_ATTACHMENT_LOAD_OP_CLEAR,
            storeOp: vk::VkAttachmentStoreOp_VK_ATTACHMENT_STORE_OP_STORE,
            clearValue: clear,
            ..Default::default()
        };
        let begin = vk::VkRenderingInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDERING_INFO,
            renderArea: area,
            layerCount: 1,
            colorAttachmentCount: 1,
            pColorAttachments: &attachment,
            ..Default::default()
        };
        // This fixed-profile draw discards contents every time. UNDEFINED handles
        // initialization too, without mutable per-image layout bookkeeping.
        let initialize = vk::VkImageMemoryBarrier2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER_2,
            srcStageMask: vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
            srcAccessMask: vk::VK_ACCESS_2_MEMORY_READ_BIT | vk::VK_ACCESS_2_MEMORY_WRITE_BIT,
            dstStageMask: vk::VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
            dstAccessMask: vk::VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT,
            oldLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_UNDEFINED,
            newLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL,
            srcQueueFamilyIndex: vk::VK_QUEUE_FAMILY_IGNORED as u32,
            dstQueueFamilyIndex: vk::VK_QUEUE_FAMILY_IGNORED as u32,
            image: self.image,
            subresourceRange: vk::VkImageSubresourceRange {
                aspectMask: vk::VkImageAspectFlagBits_VK_IMAGE_ASPECT_COLOR_BIT,
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
        let viewport = vk::VkViewport {
            x: 0.0,
            y: 0.0,
            width: self.width as f32,
            height: self.height as f32,
            minDepth: 0.0,
            maxDepth: 1.0,
        };
        // SAFETY: validated same-device objects are retained by the recording; caller
        // guarantees indirect contents and reachable shader memory. Commands are recording.
        unsafe {
            (d.f.vkCmdPipelineBarrier2.unwrap())(command, &dependency);
            (d.f.vkCmdBeginRendering.unwrap())(command, &begin);
            (d.f.vkCmdBindPipeline.unwrap())(
                command,
                vk::VkPipelineBindPoint_VK_PIPELINE_BIND_POINT_GRAPHICS,
                raster.pipeline,
            );
            (d.f.vkCmdSetViewport.unwrap())(command, 0, 1, &viewport);
            (d.f.vkCmdSetScissor.unwrap())(command, 0, 1, &area);
            batch::push_data(d, command, root);
            let draw = vk::VkDrawIndirect2InfoKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DRAW_INDIRECT_2_INFO_KHR,
                addressRange: vk::VkStridedDeviceAddressRangeKHR {
                    address: indirect.address + offset,
                    size: 16,
                    stride: 16,
                },
                addressFlags:
                    vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
                drawCount: 1,
                ..Default::default()
            };
            (d.f.vkCmdDrawIndirect2KHR.unwrap())(command, &draw);
            (d.f.vkCmdEndRendering.unwrap())(command);
        }
    }

    pub(super) unsafe fn copy_to(
        &self,
        command: vk::VkCommandBuffer,
        destination: &Buffer,
        offset: u64,
    ) {
        let region = vk::VkDeviceMemoryImageCopyKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_MEMORY_IMAGE_COPY_KHR,
            addressRange: vk::VkDeviceAddressRangeKHR {
                address: destination.address + offset,
                size: self.size as u64,
            },
            addressFlags: vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
            imageLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL,
            imageSubresource: vk::VkImageSubresourceLayers {
                aspectMask: vk::VkImageAspectFlagBits_VK_IMAGE_ASPECT_COLOR_BIT,
                mipLevel: 0,
                baseArrayLayer: 0,
                layerCount: 1,
            },
            imageExtent: vk::VkExtent3D {
                width: self.width,
                height: self.height,
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
        // SAFETY: the earlier same-batch draw initialized GENERAL. Preserve the
        // existing profile's implicit attachment-to-readback dependency.
        unsafe {
            batch::barrier(
                &self.device,
                command,
                vk::VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
                vk::VK_PIPELINE_STAGE_2_TRANSFER_BIT,
                vk::VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT,
                vk::VK_ACCESS_2_TRANSFER_READ_BIT,
            );
            (self.device.f.vkCmdCopyImageToMemoryKHR.unwrap())(command, &copy);
        }
    }
}

impl Drop for Target {
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
    modules: [vk::VkShaderModule; 2],
    pipeline: vk::VkPipeline,
}

impl Raster {
    /// # Safety
    /// Valid matching vertex/fragment SPIR-V main entries, descriptor-free, only
    /// core-required capabilities plus BDA, read-only storage, and matching root layout.
    pub(crate) unsafe fn new(
        device: Rc<Device>,
        vertex: &[u32],
        fragment: &[u32],
        push_size: u32,
    ) -> Result<Self, Error> {
        ready(&device)?;
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
                topology: vk::VkPrimitiveTopology_VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST,
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
            let rendering = vk::VkPipelineRenderingCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RENDERING_CREATE_INFO,
                pNext: (&flags as *const vk::VkPipelineCreateFlags2CreateInfo).cast(),
                colorAttachmentCount: 1,
                pColorAttachmentFormats: &FORMAT,
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
