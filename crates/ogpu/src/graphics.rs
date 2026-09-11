//! Fixed-state RGBA8 offscreen rendering. Public targets are images, never pointers.
use super::*;

const FORMAT: vk::VkFormat = vk::VkFormat_VK_FORMAT_R8G8B8A8_UNORM;
const STAGES: vk::VkShaderStageFlags = vk::VkShaderStageFlagBits_VK_SHADER_STAGE_VERTEX_BIT
    | vk::VkShaderStageFlagBits_VK_SHADER_STAGE_FRAGMENT_BIT;

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

// All render passes have identical format/subpass/dependency descriptions, so raster
// pipelines can be used with any of our targets, independent of dimensions.
struct RenderPass {
    device: Rc<Device>,
    handle: vk::VkRenderPass,
}
impl RenderPass {
    fn new(device: Rc<Device>) -> Result<Self, Error> {
        let attachment = vk::VkAttachmentDescription {
            format: FORMAT,
            samples: vk::VkSampleCountFlagBits_VK_SAMPLE_COUNT_1_BIT,
            loadOp: vk::VkAttachmentLoadOp_VK_ATTACHMENT_LOAD_OP_CLEAR,
            storeOp: vk::VkAttachmentStoreOp_VK_ATTACHMENT_STORE_OP_STORE,
            stencilLoadOp: vk::VkAttachmentLoadOp_VK_ATTACHMENT_LOAD_OP_DONT_CARE,
            stencilStoreOp: vk::VkAttachmentStoreOp_VK_ATTACHMENT_STORE_OP_DONT_CARE,
            initialLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_UNDEFINED,
            finalLayout: vk::VkImageLayout_VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
            ..Default::default()
        };
        let color = vk::VkAttachmentReference {
            attachment: 0,
            layout: vk::VkImageLayout_VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL,
        };
        let subpass = vk::VkSubpassDescription {
            pipelineBindPoint: vk::VkPipelineBindPoint_VK_PIPELINE_BIND_POINT_GRAPHICS,
            colorAttachmentCount: 1,
            pColorAttachments: &color,
            ..Default::default()
        };
        let dependencies = [
            vk::VkSubpassDependency {
                srcSubpass: vk::VK_SUBPASS_EXTERNAL as u32,
                dstSubpass: 0,
                srcStageMask: vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
                dstStageMask:
                    vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
                srcAccessMask: vk::VkAccessFlagBits_VK_ACCESS_MEMORY_READ_BIT
                    | vk::VkAccessFlagBits_VK_ACCESS_MEMORY_WRITE_BIT,
                dstAccessMask: vk::VkAccessFlagBits_VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT,
                ..Default::default()
            },
            vk::VkSubpassDependency {
                srcSubpass: 0,
                dstSubpass: vk::VK_SUBPASS_EXTERNAL as u32,
                srcStageMask:
                    vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
                dstStageMask: vk::VkPipelineStageFlagBits_VK_PIPELINE_STAGE_TRANSFER_BIT,
                srcAccessMask: vk::VkAccessFlagBits_VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT,
                dstAccessMask: vk::VkAccessFlagBits_VK_ACCESS_TRANSFER_READ_BIT,
                ..Default::default()
            },
        ];
        let create = vk::VkRenderPassCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO,
            attachmentCount: 1,
            pAttachments: &attachment,
            subpassCount: 1,
            pSubpasses: &subpass,
            dependencyCount: 2,
            pDependencies: dependencies.as_ptr(),
            ..Default::default()
        };
        let mut result = Self {
            device,
            handle: ptr::null_mut(),
        };
        // SAFETY: all referenced state is live for creation; result owns partial construction.
        unsafe {
            result.device.result(
                "vkCreateRenderPass",
                (result.device.f.vkCreateRenderPass.unwrap())(
                    result.device.handle,
                    &create,
                    ptr::null(),
                    &mut result.handle,
                ),
            )?;
        }
        Ok(result)
    }
}
impl Drop for RenderPass {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                (self.device.f.vkDestroyRenderPass.unwrap())(
                    self.device.handle,
                    self.handle,
                    ptr::null(),
                );
            }
        }
    }
}

pub(crate) struct Target {
    pub(super) device: Rc<Device>,
    pub(super) size: usize,
    width: u32,
    height: u32,
    image: vk::VkImage,
    memory: vk::VkDeviceMemory,
    view: vk::VkImageView,
    framebuffer: vk::VkFramebuffer,
    pass: RenderPass,
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
        let pass = RenderPass::new(device.clone())?;
        let mut result = Self {
            device,
            size,
            width,
            height,
            pass,
            image: ptr::null_mut(),
            memory: ptr::null_mut(),
            view: ptr::null_mut(),
            framebuffer: ptr::null_mut(),
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
            let framebuffer = vk::VkFramebufferCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO,
                renderPass: result.pass.handle,
                attachmentCount: 1,
                pAttachments: &result.view,
                width,
                height,
                layers: 1,
                ..Default::default()
            };
            d.result(
                "vkCreateFramebuffer",
                (d.f.vkCreateFramebuffer.unwrap())(
                    d.handle,
                    &framebuffer,
                    ptr::null(),
                    &mut result.framebuffer,
                ),
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
        let begin = vk::VkRenderPassBeginInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO,
            renderPass: self.pass.handle,
            framebuffer: self.framebuffer,
            renderArea: area,
            clearValueCount: 1,
            pClearValues: &clear,
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
            (d.f.vkCmdBeginRenderPass.unwrap())(
                command,
                &begin,
                vk::VkSubpassContents_VK_SUBPASS_CONTENTS_INLINE,
            );
            (d.f.vkCmdBindPipeline.unwrap())(
                command,
                vk::VkPipelineBindPoint_VK_PIPELINE_BIND_POINT_GRAPHICS,
                raster.pipeline,
            );
            (d.f.vkCmdSetViewport.unwrap())(command, 0, 1, &viewport);
            (d.f.vkCmdSetScissor.unwrap())(command, 0, 1, &area);
            if raster.push_size != 0 {
                (d.f.vkCmdPushConstants.unwrap())(
                    command,
                    raster.layout,
                    STAGES,
                    0,
                    raster.push_size,
                    root.as_ptr().cast(),
                );
            }
            (d.f.vkCmdDrawIndirect.unwrap())(command, indirect.buffer, offset, 1, 16);
            (d.f.vkCmdEndRenderPass.unwrap())(command);
        }
    }

    pub(super) unsafe fn copy_to(
        &self,
        command: vk::VkCommandBuffer,
        destination: &Buffer,
        offset: u64,
    ) {
        let region = vk::VkBufferImageCopy {
            bufferOffset: offset,
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
        // SAFETY: prior draw established TRANSFER_SRC layout and attachment visibility;
        // recording checks destination bounds/alignment and retains both resources.
        unsafe {
            (self.device.f.vkCmdCopyImageToBuffer.unwrap())(
                command,
                self.image,
                vk::VkImageLayout_VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
                destination.buffer,
                1,
                &region,
            );
        }
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        // SAFETY: recorded/submitted uses retain the target until commands are destroyed.
        unsafe {
            let d = &self.device;
            if !self.framebuffer.is_null() {
                (d.f.vkDestroyFramebuffer.unwrap())(d.handle, self.framebuffer, ptr::null());
            }
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
    layout: vk::VkPipelineLayout,
    pipeline: vk::VkPipeline,
    _pass: RenderPass,
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
            || push_size > device.limits.maxPushConstantsSize
        {
            return Err(Error::new(
                INVALID_ARGUMENT,
                "Invalid raster SPIR-V header or root size",
            ));
        }
        let pass = RenderPass::new(device.clone())?;
        let mut result = Self {
            device,
            push_size,
            modules: [ptr::null_mut(); 2],
            layout: ptr::null_mut(),
            pipeline: ptr::null_mut(),
            _pass: pass,
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
            let range = vk::VkPushConstantRange {
                stageFlags: STAGES,
                offset: 0,
                size: push_size,
            };
            let layout = vk::VkPipelineLayoutCreateInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
                pushConstantRangeCount: u32::from(push_size != 0),
                pPushConstantRanges: if push_size == 0 { ptr::null() } else { &range },
                ..Default::default()
            };
            d.result(
                "vkCreatePipelineLayout(raster)",
                (d.f.vkCreatePipelineLayout.unwrap())(
                    d.handle,
                    &layout,
                    ptr::null(),
                    &mut result.layout,
                ),
            )?;
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
                layout: result.layout,
                renderPass: result._pass.handle,
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
            if !self.layout.is_null() {
                (d.f.vkDestroyPipelineLayout.unwrap())(d.handle, self.layout, ptr::null());
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
