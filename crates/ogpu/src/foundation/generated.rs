//! Native generated-command layouts and executable tables. No command expansion.
use super::*;

#[allow(non_snake_case)]
pub(super) struct GeneratedFunctions {
    pub(super) vkCreateIndirectCommandsLayoutEXT: vk::PFN_vkCreateIndirectCommandsLayoutEXT,
    pub(super) vkDestroyIndirectCommandsLayoutEXT: vk::PFN_vkDestroyIndirectCommandsLayoutEXT,
    pub(super) vkCreateIndirectExecutionSetEXT: vk::PFN_vkCreateIndirectExecutionSetEXT,
    pub(super) vkDestroyIndirectExecutionSetEXT: vk::PFN_vkDestroyIndirectExecutionSetEXT,
    pub(super) vkUpdateIndirectExecutionSetPipelineEXT:
        vk::PFN_vkUpdateIndirectExecutionSetPipelineEXT,
    pub(super) vkGetGeneratedCommandsMemoryRequirementsEXT:
        vk::PFN_vkGetGeneratedCommandsMemoryRequirementsEXT,
    pub(super) vkCmdPreprocessGeneratedCommandsEXT: vk::PFN_vkCmdPreprocessGeneratedCommandsEXT,
    pub(super) vkCmdExecuteGeneratedCommandsEXT: vk::PFN_vkCmdExecuteGeneratedCommandsEXT,
}
impl GeneratedFunctions {
    pub(super) fn load(instance: &Instance) -> Result<Self, Status> {
        let f = Self {
            vkCreateIndirectCommandsLayoutEXT: unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkCreateIndirectCommandsLayoutEXT,
                >(instance.proc(c"vkCreateIndirectCommandsLayoutEXT"))
            },
            vkDestroyIndirectCommandsLayoutEXT: unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkDestroyIndirectCommandsLayoutEXT,
                >(instance.proc(c"vkDestroyIndirectCommandsLayoutEXT"))
            },
            vkCreateIndirectExecutionSetEXT: unsafe {
                std::mem::transmute::<vk::PFN_vkVoidFunction, vk::PFN_vkCreateIndirectExecutionSetEXT>(
                    instance.proc(c"vkCreateIndirectExecutionSetEXT"),
                )
            },
            vkDestroyIndirectExecutionSetEXT: unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkDestroyIndirectExecutionSetEXT,
                >(instance.proc(c"vkDestroyIndirectExecutionSetEXT"))
            },
            vkUpdateIndirectExecutionSetPipelineEXT: unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkUpdateIndirectExecutionSetPipelineEXT,
                >(instance.proc(c"vkUpdateIndirectExecutionSetPipelineEXT"))
            },
            vkGetGeneratedCommandsMemoryRequirementsEXT: unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkGetGeneratedCommandsMemoryRequirementsEXT,
                >(instance.proc(c"vkGetGeneratedCommandsMemoryRequirementsEXT"))
            },
            vkCmdPreprocessGeneratedCommandsEXT: unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkCmdPreprocessGeneratedCommandsEXT,
                >(instance.proc(c"vkCmdPreprocessGeneratedCommandsEXT"))
            },
            vkCmdExecuteGeneratedCommandsEXT: unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkCmdExecuteGeneratedCommandsEXT,
                >(instance.proc(c"vkCmdExecuteGeneratedCommandsEXT"))
            },
        };
        if f.vkCreateIndirectCommandsLayoutEXT.is_none()
            || f.vkDestroyIndirectCommandsLayoutEXT.is_none()
            || f.vkCreateIndirectExecutionSetEXT.is_none()
            || f.vkDestroyIndirectExecutionSetEXT.is_none()
            || f.vkUpdateIndirectExecutionSetPipelineEXT.is_none()
            || f.vkGetGeneratedCommandsMemoryRequirementsEXT.is_none()
            || f.vkCmdPreprocessGeneratedCommandsEXT.is_none()
            || f.vkCmdExecuteGeneratedCommandsEXT.is_none()
        {
            return Err(UNSUPPORTED);
        }
        Ok(f)
    }
}
pub(super) fn functions(d: &Device) -> Result<&GeneratedFunctions, Status> {
    d.ready()?;
    d.generated_functions.as_ref().ok_or(UNSUPPORTED)
}
pub(super) fn native_stages(stages: u64) -> u32 {
    [
        (
            STAGE_RAY,
            vk::VkShaderStageFlagBits_VK_SHADER_STAGE_RAYGEN_BIT_KHR
                | vk::VkShaderStageFlagBits_VK_SHADER_STAGE_MISS_BIT_KHR
                | vk::VkShaderStageFlagBits_VK_SHADER_STAGE_CLOSEST_HIT_BIT_KHR
                | vk::VkShaderStageFlagBits_VK_SHADER_STAGE_ANY_HIT_BIT_KHR
                | vk::VkShaderStageFlagBits_VK_SHADER_STAGE_INTERSECTION_BIT_KHR
                | vk::VkShaderStageFlagBits_VK_SHADER_STAGE_CALLABLE_BIT_KHR,
        ),
        (4, vk::VkShaderStageFlagBits_VK_SHADER_STAGE_COMPUTE_BIT),
        (8, vk::VkShaderStageFlagBits_VK_SHADER_STAGE_VERTEX_BIT),
        (16, vk::VkShaderStageFlagBits_VK_SHADER_STAGE_FRAGMENT_BIT),
        (
            TESS_CONTROL_STAGE,
            vk::VkShaderStageFlagBits_VK_SHADER_STAGE_TESSELLATION_CONTROL_BIT,
        ),
        (
            TESS_EVALUATION_STAGE,
            vk::VkShaderStageFlagBits_VK_SHADER_STAGE_TESSELLATION_EVALUATION_BIT,
        ),
        (
            GEOMETRY_STAGE,
            vk::VkShaderStageFlagBits_VK_SHADER_STAGE_GEOMETRY_BIT,
        ),
        (
            MESH_STAGE,
            vk::VkShaderStageFlagBits_VK_SHADER_STAGE_MESH_BIT_EXT,
        ),
        (
            TASK_STAGE,
            vk::VkShaderStageFlagBits_VK_SHADER_STAGE_TASK_BIT_EXT,
        ),
    ]
    .into_iter()
    .fold(0, |r, (bit, native)| {
        r | if stages & bit != 0 { native } else { 0 }
    })
}
pub(super) fn public_stages(stages: u32) -> u64 {
    [
        STAGE_RAY,
        4,
        8,
        16,
        MESH_STAGE,
        TASK_STAGE,
        TESS_CONTROL_STAGE,
        TESS_EVALUATION_STAGE,
        GEOMETRY_STAGE,
    ]
    .into_iter()
    .fold(0, |r, bit| {
        r | if stages & native_stages(bit) == native_stages(bit) {
            bit
        } else {
            0
        }
    })
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(FeatureBits, GeneratedLimits), Status> {
    if !instance
        .supports_extension(physical, c"VK_EXT_device_generated_commands")
        .map_err(loader_error)?
    {
        return Ok((FeatureBits::ZERO, GeneratedLimits::default()));
    }
    let mut features = vk::VkPhysicalDeviceDeviceGeneratedCommandsFeaturesEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEVICE_GENERATED_COMMANDS_FEATURES_EXT, ..Default::default()
    };
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: ptr::from_mut(&mut features).cast(),
        ..Default::default()
    };
    let mut p = vk::VkPhysicalDeviceDeviceGeneratedCommandsPropertiesEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEVICE_GENERATED_COMMANDS_PROPERTIES_EXT, ..Default::default()
    };
    let mut properties = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: ptr::from_mut(&mut p).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut properties);
    }
    let input_modes = p.supportedIndirectCommandsInputModes;
    let vulkan_indices = input_modes & vk::VkIndirectCommandsInputModeFlagBitsEXT_VK_INDIRECT_COMMANDS_INPUT_MODE_VULKAN_INDEX_BUFFER_EXT != 0;
    let dxgi_indices = input_modes & vk::VkIndirectCommandsInputModeFlagBitsEXT_VK_INDIRECT_COMMANDS_INPUT_MODE_DXGI_INDEX_BUFFER_EXT != 0;
    let mut tokens = ((1u64 << 8) - 2) | (1 << GENERATED_VERTEX_BUFFER); // Base action/argument tokens plus vertex binding.
    if p.deviceGeneratedCommandsMultiDrawIndirectCount != 0 {
        tokens |= 7 << 8;
    }
    if public_stages(p.supportedIndirectCommandsShaderStages) & STAGE_RAY != 0 {
        tokens |= 1 << GENERATED_TRACE_RAYS;
    }
    if vulkan_indices || dxgi_indices {
        tokens |= 1 << GENERATED_INDEX_BUFFER;
    }
    Ok((
        if features.deviceGeneratedCommands != 0 {
            GENERATED_COMMANDS
        } else {
            FeatureBits::ZERO
        },
        GeneratedLimits {
            stages: public_stages(p.supportedIndirectCommandsShaderStages),
            pipeline_binding_stages: public_stages(
                p.supportedIndirectCommandsShaderStagesPipelineBinding,
            ),
            tokens,
            max_pipelines: p.maxIndirectPipelineCount,
            max_sequences: p.maxIndirectSequenceCount,
            max_tokens: p.maxIndirectCommandsTokenCount,
            max_token_offset: p.maxIndirectCommandsTokenOffset,
            max_stride: p.maxIndirectCommandsIndirectStride,
            reserved: 0,
            index_u16: if vulkan_indices {
                vk::VkIndexType_VK_INDEX_TYPE_UINT16
            } else {
                57
            },
            index_u32: if vulkan_indices {
                vk::VkIndexType_VK_INDEX_TYPE_UINT32
            } else {
                42
            },
        },
    ))
}

pub struct GeneratedLayout {
    arguments: *const ArgumentLayout,
    pub(super) device: *const Device,
    pub(super) handle: vk::VkIndirectCommandsLayoutEXT,
    pub(super) stages: u64,
    pub(super) flags: u32,
    pub(super) stride: u32,
    pub(super) extent: u64,
    pub(super) argument_end: u32,
    pub(super) action: u32,
    pub(super) selection: bool,
    pub(super) index_binding: bool,
    vertex_bindings: Box<[u32]>,
}
fn layout_contract(
    p: &GeneratedLimits,
    enabled: FeatureBits,
    argument_bytes: u64,
    desc: &GeneratedLayoutDesc,
    tokens: &[GeneratedToken],
) -> Result<(u64, u32), Status> {
    if desc.reserved != 0 || desc.flags & !3 != 0 {
        return Err(INVALID);
    }
    if tokens.is_empty() || desc.stride % 4 != 0 {
        return Err(INVALID);
    }
    if tokens.len() > p.max_tokens as usize
        || desc.stride > p.max_stride
        || desc.stages & !p.stages != 0
    {
        return Err(UNSUPPORTED);
    }
    let compute = desc.stages == 4;
    let ray = desc.stages == STAGE_RAY;
    let vertex = raster_stages::traditional(desc.stages);
    let mesh = matches!(desc.stages, MESH_STAGE)
        || desc.stages == MESH_STAGE | 16
        || desc.stages == MESH_STAGE | TASK_STAGE
        || desc.stages == MESH_STAGE | TASK_STAGE | 16;
    if !compute && !vertex && !mesh && !ray {
        return Err(UNSUPPORTED);
    }
    if !compute && !ray && enabled & RASTER == 0
        || ray && enabled & (RAY_PIPELINE | RAY_MAINTENANCE) != RAY_PIPELINE | RAY_MAINTENANCE
        // Native generated mesh tokens require both enables (VUID 11126), even
        // when the executable itself has no task stage. Ordinary mesh draws do not.
        || mesh && enabled & (MESH_SHADER | TASK_SHADER) != MESH_SHADER | TASK_SHADER
        || desc.stages & TASK_STAGE != 0 && enabled & TASK_SHADER == 0
    {
        return Err(UNSUPPORTED);
    }
    raster_stages::enables(enabled, desc.stages, 0)?;
    let action = tokens.last().unwrap().kind;
    if !(matches!(action, 4..=10) || action == GENERATED_TRACE_RAYS)
        || ray && action != GENERATED_TRACE_RAYS
        || compute && action != 4
        || vertex && !matches!(action, 5 | 6 | 8 | 9)
        || mesh && !matches!(action, 7 | 10)
    {
        return Err(INVALID);
    }
    let mut extent = 0u64;
    let mut argument_end = 0;
    let mut sequence = false;
    for (i, t) in tokens.iter().enumerate() {
        if t.kind > GENERATED_TRACE_RAYS || t.kind == 0 || p.tokens & (1 << t.kind) == 0 {
            return Err(UNSUPPORTED);
        }
        if t.offset % 4 != 0 || i != 0 && t.offset < tokens[i - 1].offset {
            return Err(INVALID);
        }
        if t.offset > p.max_token_offset {
            return Err(UNSUPPORTED);
        }
        if t.kind == 1 && (i != 0 || desc.stages & !p.pipeline_binding_stages != 0) {
            return Err(UNSUPPORTED);
        }
        if (matches!(t.kind, 4..=10) || t.kind == GENERATED_TRACE_RAYS) && i != tokens.len() - 1 {
            return Err(INVALID);
        }
        let size = if matches!(t.kind, 2 | 3) {
            if t.target % 4 != 0
                || t.size == 0
                || t.size % 4 != 0
                || t.kind == 3 && (sequence || t.size != 4)
            {
                return Err(INVALID);
            }
            let end = t.target.checked_add(t.size).ok_or(INVALID)?;
            if u64::from(end) > argument_bytes {
                return Err(INVALID);
            }
            for prev in &tokens[..i] {
                if matches!(prev.kind, 2 | 3)
                    && u64::from(prev.target) < u64::from(end)
                    && u64::from(t.target) < u64::from(prev.target) + u64::from(prev.size)
                {
                    return Err(INVALID);
                }
            }
            argument_end = argument_end.max(end);
            if t.kind == 3 {
                sequence = true;
                0
            } else {
                t.size
            }
        } else if matches!(t.kind, GENERATED_VERTEX_BUFFER | GENERATED_INDEX_BUFFER) {
            if !vertex
                || t.size != 0
                || t.kind == GENERATED_INDEX_BUFFER && t.target != 0
                || tokens[..i].iter().any(|p| {
                    p.kind == t.kind && (t.kind == GENERATED_INDEX_BUFFER || p.target == t.target)
                })
            {
                return Err(INVALID);
            }
            16
        } else {
            if t.target != 0 || t.size != 0 {
                return Err(INVALID);
            }
            match t.kind {
                1 => 4,
                4 | 7 => 12,
                5 | 8..=10 => 16,
                6 => 20,
                GENERATED_TRACE_RAYS => size_of::<RayIndirect>() as u32,
                _ => unreachable!(),
            }
        };
        extent = extent.max(u64::from(t.offset) + u64::from(size));
    }
    Ok((extent, argument_end))
}
impl GeneratedLayout {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &GeneratedLayoutDesc,
    ) -> Result<Box<Self>, Status> {
        let f = functions(d)?;
        desc.header
            .validate::<GeneratedLayoutDesc>(GENERATED_LAYOUT_DESC)?;
        let arguments = unsafe { desc.arguments.as_ref() }.ok_or(INVALID)?;
        let class = if desc.stages == STAGE_RAY {
            ARGUMENT_RAY
        } else if desc.stages == 4 {
            ARGUMENT_COMPUTE
        } else {
            ARGUMENT_GRAPHICS
        };
        if !ptr::eq(arguments.device, d) || arguments.execution_class != class {
            return Err(INVALID);
        }
        if desc.token_count == 0 || desc.tokens.is_null() {
            return Err(INVALID);
        }
        if desc.token_count > d.snapshot.generated_limits.max_tokens {
            return Err(UNSUPPORTED);
        }
        let tokens = unsafe { std::slice::from_raw_parts(desc.tokens, desc.token_count as usize) };
        let (extent, argument_end) = layout_contract(
            &d.snapshot.generated_limits,
            d.snapshot.features.enabled,
            u64::from(arguments.byte_size()),
            desc,
            tokens,
        )?;
        for t in tokens.iter().filter(|t| t.kind == GENERATED_VERTEX_BUFFER) {
            if t.target >= d.snapshot.graphics_limits.max_vertex_bindings {
                return Err(UNSUPPORTED);
            }
        }
        for t in tokens.iter().filter(|t| matches!(t.kind, 2 | 3)) {
            arguments.generated_range(t.target, t.size, t.kind == 3)?;
        }
        let push: Vec<_> = tokens
            .iter()
            .map(|t| vk::VkIndirectCommandsPushConstantTokenEXT {
                updateRange: vk::VkPushConstantRange {
                    stageFlags: vk::VkShaderStageFlagBits_VK_SHADER_STAGE_ALL,
                    offset: t.target,
                    size: t.size,
                },
            })
            .collect();
        let vertex: Vec<_> = tokens
            .iter()
            .map(|t| vk::VkIndirectCommandsVertexBufferTokenEXT {
                vertexBindingUnit: t.target,
            })
            .collect();
        let index = vk::VkIndirectCommandsIndexBufferTokenEXT {
            mode: if d.snapshot.generated_limits.index_u16 == vk::VkIndexType_VK_INDEX_TYPE_UINT16 {
                vk::VkIndirectCommandsInputModeFlagBitsEXT_VK_INDIRECT_COMMANDS_INPUT_MODE_VULKAN_INDEX_BUFFER_EXT
            } else {
                vk::VkIndirectCommandsInputModeFlagBitsEXT_VK_INDIRECT_COMMANDS_INPUT_MODE_DXGI_INDEX_BUFFER_EXT
            },
        };
        let execution = vk::VkIndirectCommandsExecutionSetTokenEXT { type_: vk::VkIndirectExecutionSetInfoTypeEXT_VK_INDIRECT_EXECUTION_SET_INFO_TYPE_PIPELINES_EXT,
            shaderStages: native_stages(desc.stages) };
        let native: Vec<_> = tokens.iter().zip(&push).zip(&vertex).map(|((t, p), v)| vk::VkIndirectCommandsLayoutTokenEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_INDIRECT_COMMANDS_LAYOUT_TOKEN_EXT,
            type_: match t.kind {
                1 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_EXECUTION_SET_EXT,
                2 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_PUSH_DATA_EXT,
                3 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_PUSH_DATA_SEQUENCE_INDEX_EXT,
                4 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_DISPATCH_EXT,
                5 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_DRAW_EXT,
                6 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_DRAW_INDEXED_EXT,
                7 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_DRAW_MESH_TASKS_EXT,
                8 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_DRAW_COUNT_EXT,
                9 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_DRAW_INDEXED_COUNT_EXT,
                10 => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_DRAW_MESH_TASKS_COUNT_EXT,
                GENERATED_TRACE_RAYS => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_TRACE_RAYS2_EXT,
                GENERATED_VERTEX_BUFFER => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_VERTEX_BUFFER_EXT,
                GENERATED_INDEX_BUFFER => vk::VkIndirectCommandsTokenTypeEXT_VK_INDIRECT_COMMANDS_TOKEN_TYPE_INDEX_BUFFER_EXT,
                _ => unreachable!(),
            },
            data: if t.kind == 1 { vk::VkIndirectCommandsTokenDataEXT { pExecutionSet: &execution } }
                else if matches!(t.kind, 2 | 3) { vk::VkIndirectCommandsTokenDataEXT { pPushConstant: p } }
                else if t.kind == GENERATED_VERTEX_BUFFER { vk::VkIndirectCommandsTokenDataEXT { pVertexBuffer: v } }
                else if t.kind == GENERATED_INDEX_BUFFER { vk::VkIndirectCommandsTokenDataEXT { pIndexBuffer: &index } }
                else { Default::default() }, offset: t.offset, ..Default::default()
        }).collect();
        let info = vk::VkIndirectCommandsLayoutCreateInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_INDIRECT_COMMANDS_LAYOUT_CREATE_INFO_EXT,
            flags: desc.flags,
            shaderStages: native_stages(desc.stages),
            indirectStride: desc.stride,
            tokenCount: desc.token_count,
            pTokens: native.as_ptr(),
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            d.result((f.vkCreateIndirectCommandsLayoutEXT.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Box::new(Self {
            device: d,
            handle,
            arguments,
            stages: desc.stages,
            flags: desc.flags,
            stride: desc.stride,
            extent,
            argument_end,
            action: tokens.last().unwrap().kind,
            selection: tokens[0].kind == 1,
            index_binding: tokens.iter().any(|t| t.kind == GENERATED_INDEX_BUFFER),
            vertex_bindings: tokens
                .iter()
                .filter(|t| t.kind == GENERATED_VERTEX_BUFFER)
                .map(|t| t.target)
                .collect(),
        }))
    }
}
impl Drop for GeneratedLayout {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.generated_functions
                .as_ref()
                .unwrap()
                .vkDestroyIndirectCommandsLayoutEXT
                .unwrap())(d.handle, self.handle, ptr::null())
        };
    }
}

pub struct GeneratedSet {
    pub(super) device: *const Device,
    pub(super) handle: vk::VkIndirectExecutionSetEXT,
    pub(super) initial: *const Executable,
    capacity: u32,
}
impl GeneratedSet {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &GeneratedSetDesc,
    ) -> Result<Box<Self>, Status> {
        let f = functions(d)?;
        desc.header
            .validate::<GeneratedSetDesc>(GENERATED_SET_DESC)?;
        if desc.reserved != 0 || desc.capacity == 0 {
            return Err(INVALID);
        }
        if desc.capacity > d.snapshot.generated_limits.max_pipelines {
            return Err(UNSUPPORTED);
        }
        let initial = unsafe { desc.initial.as_ref() }.ok_or(INVALID)?;
        if !ptr::eq(initial.device, d) || !initial.indirect_bindable || initial.library {
            return Err(INVALID);
        }
        let pipeline = vk::VkIndirectExecutionSetPipelineInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_INDIRECT_EXECUTION_SET_PIPELINE_INFO_EXT,
            initialPipeline: initial.pipeline,
            maxPipelineCount: desc.capacity,
            ..Default::default()
        };
        let info = vk::VkIndirectExecutionSetCreateInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_INDIRECT_EXECUTION_SET_CREATE_INFO_EXT,
            type_: vk::VkIndirectExecutionSetInfoTypeEXT_VK_INDIRECT_EXECUTION_SET_INFO_TYPE_PIPELINES_EXT,
            info: vk::VkIndirectExecutionSetInfoEXT { pPipelineInfo: &pipeline }, ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            d.result((f.vkCreateIndirectExecutionSetEXT.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Box::new(Self {
            device: d,
            handle,
            initial,
            capacity: desc.capacity,
        }))
    }
    pub(in crate::foundation) unsafe fn update(
        &self,
        writes: &[GeneratedSetWrite],
    ) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        let f = functions(d)?;
        let initial = unsafe { &*self.initial };
        if writes.len() > self.capacity as usize {
            return Err(INVALID);
        }
        let mut native = Vec::new();
        native
            .try_reserve_exact(writes.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        for (i, w) in writes.iter().enumerate() {
            let e = unsafe { w.executable.as_ref() }.ok_or(INVALID)?;
            if w.reserved != 0
                || w.index >= self.capacity
                || !ptr::eq(e.device, d)
                || !e.indirect_bindable
                || e.library
                || e.stages != initial.stages
                || e.view_mask != initial.view_mask
                || !e.arguments().generated_compatible(initial.arguments())
                || writes[..i].iter().any(|p| p.index == w.index)
            {
                return Err(INVALID);
            }
            native.push(vk::VkWriteIndirectExecutionSetPipelineEXT {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_WRITE_INDIRECT_EXECUTION_SET_PIPELINE_EXT,
                index: w.index,
                pipeline: e.pipeline,
                ..Default::default()
            });
        }
        if !native.is_empty() {
            unsafe {
                (f.vkUpdateIndirectExecutionSetPipelineEXT.unwrap())(
                    d.handle,
                    self.handle,
                    native.len() as u32,
                    native.as_ptr(),
                )
            };
        }
        Ok(())
    }
}
impl Drop for GeneratedSet {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.generated_functions
                .as_ref()
                .unwrap()
                .vkDestroyIndirectExecutionSetEXT
                .unwrap())(d.handle, self.handle, ptr::null())
        };
    }
}

pub(super) unsafe fn work<'a>(
    d: &Device,
    work: &GeneratedWork,
) -> Result<
    (
        &'a GeneratedLayout,
        &'a Executable,
        vk::VkIndirectExecutionSetEXT,
    ),
    Status,
> {
    functions(d)?;
    let layout = unsafe { work.layout.as_ref() }.ok_or(INVALID)?;
    if !ptr::eq(layout.device, d) || work.max_sequences == 0 {
        return Err(INVALID);
    }
    if work.max_sequences > d.snapshot.generated_limits.max_sequences {
        return Err(UNSUPPORTED);
    }
    if matches!(layout.action, 8..=10)
        && u64::from(work.max_sequences) * u64::from(work.max_draws) >= (1 << 24)
    {
        return Err(INVALID);
    }
    let (e, set) = if layout.selection {
        if !work.executable.is_null() {
            return Err(INVALID);
        }
        let set = unsafe { work.set.as_ref() }.ok_or(INVALID)?;
        if !ptr::eq(set.device, d) {
            return Err(INVALID);
        }
        (unsafe { &*set.initial }, set.handle)
    } else {
        if !work.set.is_null() {
            return Err(INVALID);
        }
        (
            unsafe { work.executable.as_ref() }.ok_or(INVALID)?,
            ptr::null_mut(),
        )
    };
    // Mapping compatibility is checked by the mandatory cold requirements query
    // and by set updates. Recording trusts that pairing, just as it trusts the
    // native scratch requirements, without walking argument fields here.
    if !ptr::eq(e.device, d)
        || layout.stages != e.stages
        || layout.argument_end > e.arguments().byte_size()
    {
        return Err(INVALID);
    }
    Ok((layout, e, set))
}
impl Device {
    pub(in crate::foundation) unsafe fn generated_requirements(
        &self,
        input: &GeneratedWork,
    ) -> Result<GeneratedRequirements, Status> {
        let (layout, e, set) = unsafe { work(self, input)? };
        if !unsafe { &*layout.arguments }.generated_compatible(e.arguments()) {
            return Err(INVALID);
        }
        // Native state compatibility is a cold pairing check, never a per-draw scan.
        if !layout.vertex_bindings.is_empty()
            && (e.dynamic_state & DYNAMIC_VERTEX_STRIDE == 0
                || e.dynamic_state & DYNAMIC_VERTEX_INPUT_STATE == 0
                    && layout
                        .vertex_bindings
                        .iter()
                        .any(|binding| !e.vertex_bindings.contains(binding)))
        {
            return Err(INVALID);
        }
        let pipeline = vk::VkGeneratedCommandsPipelineInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_GENERATED_COMMANDS_PIPELINE_INFO_EXT,
            pipeline: e.pipeline,
            ..Default::default()
        };
        let info = vk::VkGeneratedCommandsMemoryRequirementsInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_GENERATED_COMMANDS_MEMORY_REQUIREMENTS_INFO_EXT,
            pNext: if set.is_null() { ptr::from_ref(&pipeline).cast() } else { ptr::null() },
            indirectExecutionSet: set, indirectCommandsLayout: layout.handle,
            maxSequenceCount: input.max_sequences, maxDrawCount: input.max_draws,
        };
        let mut out = vk::VkMemoryRequirements2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_REQUIREMENTS_2,
            ..Default::default()
        };
        unsafe {
            (functions(self)?
                .vkGetGeneratedCommandsMemoryRequirementsEXT
                .unwrap())(self.handle, &info, &mut out)
        };
        Ok(GeneratedRequirements {
            size: out.memoryRequirements.size,
            alignment: out.memoryRequirements.alignment,
            memory_type_bits: out.memoryRequirements.memoryTypeBits,
            reserved: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffer_state_tokens_reject_ambiguous_or_wrong_stage_layouts() {
        let limits = GeneratedLimits {
            stages: 28,
            tokens: (1 << 13) - 2,
            max_tokens: 8,
            max_token_offset: 256,
            max_stride: 256,
            ..Default::default()
        };
        let mut tokens = [
            GeneratedToken {
                kind: GENERATED_VERTEX_BUFFER,
                offset: 0,
                target: 3,
                size: 0,
            },
            GeneratedToken {
                kind: GENERATED_INDEX_BUFFER,
                offset: 16,
                target: 0,
                size: 0,
            },
            GeneratedToken {
                kind: GENERATED_DRAW_INDEXED,
                offset: 32,
                target: 0,
                size: 0,
            },
        ];
        let mut desc = GeneratedLayoutDesc {
            header: Record::new::<GeneratedLayoutDesc>(GENERATED_LAYOUT_DESC),
            arguments: ptr::null_mut(),
            stages: 24,
            flags: 0,
            stride: 64,
            token_count: 3,
            reserved: 0,
            tokens: tokens.as_ptr(),
        };
        let check = |d: &GeneratedLayoutDesc, t: &[GeneratedToken]| {
            layout_contract(&limits, GENERATED_COMMANDS | RASTER, 0, d, t)
        };
        assert_eq!(check(&desc, &tokens), Ok((52, 0)));
        tokens[0].size = 4;
        assert_eq!(check(&desc, &tokens), Err(INVALID));
        tokens[0].size = 0;
        tokens[1].target = 1;
        assert_eq!(check(&desc, &tokens), Err(INVALID));
        tokens[1] = GeneratedToken {
            offset: 16,
            ..tokens[0]
        };
        assert_eq!(check(&desc, &tokens), Err(INVALID)); // Duplicate vertex destination.
        tokens[1].target = 4;
        assert_eq!(check(&desc, &tokens), Ok((52, 0)));
        for token in &mut tokens[..2] {
            token.kind = GENERATED_INDEX_BUFFER;
            token.target = 0;
        }
        assert_eq!(check(&desc, &tokens), Err(INVALID)); // Duplicate index binding.
        tokens[0].kind = GENERATED_VERTEX_BUFFER;
        desc.stages = 4;
        tokens[2].kind = GENERATED_DISPATCH;
        assert_eq!(check(&desc, &tokens), Err(INVALID));
    }
    #[test]
    fn layout_contract_preserves_explicit_wire_and_argument_ranges() {
        let limits = GeneratedLimits {
            stages: 28 | MESH_STAGE | TASK_STAGE,
            pipeline_binding_stages: 28 | MESH_STAGE | TASK_STAGE,
            tokens: (1 << 11) - 2,
            max_tokens: 8,
            max_token_offset: 128,
            max_stride: 128,
            ..Default::default()
        };
        let mut tokens = [
            GeneratedToken {
                kind: 1,
                ..Default::default()
            },
            GeneratedToken {
                kind: 2,
                offset: 4,
                target: 8,
                size: 8,
            },
            GeneratedToken {
                kind: 3,
                offset: 12,
                target: 0,
                size: 4,
            },
            GeneratedToken {
                kind: 4,
                offset: 12,
                ..Default::default()
            },
        ];
        let mut desc = GeneratedLayoutDesc {
            header: Record::new::<GeneratedLayoutDesc>(GENERATED_LAYOUT_DESC),
            arguments: ptr::null_mut(),
            stages: 4,
            flags: 0,
            stride: 24,
            token_count: 4,
            reserved: 0,
            tokens: tokens.as_ptr(),
        };
        let check = |desc: &GeneratedLayoutDesc, tokens: &[GeneratedToken]| {
            layout_contract(&limits, GENERATED_COMMANDS, 128, desc, tokens)
        };
        assert_eq!(check(&desc, &tokens), Ok((24, 16)));
        desc.stride = 0;
        assert_eq!(check(&desc, &tokens), Ok((24, 16)));
        desc.stride = 20;
        assert_eq!(check(&desc, &tokens), Ok((24, 16)));
        desc.stride = 25;
        assert_eq!(check(&desc, &tokens), Err(INVALID));
        desc.stride = 24;
        tokens[2].target = 12;
        assert_eq!(check(&desc, &tokens), Err(INVALID));
        tokens[2].target = 0;
        tokens[3].kind = 5;
        assert_eq!(check(&desc, &tokens), Err(INVALID));
        tokens[3].kind = 4;
        tokens[1].offset = 5;
        assert_eq!(check(&desc, &tokens), Err(INVALID));
        tokens[1].offset = 4;
        tokens[1].target = u32::MAX - 3;
        assert_eq!(check(&desc, &tokens), Err(INVALID));
        tokens[1].target = 8;
        tokens[1].kind = 1;
        assert_eq!(check(&desc, &tokens), Err(UNSUPPORTED));
        assert_eq!(
            public_stages(native_stages(
                28 | MESH_STAGE | TASK_STAGE | raster_stages::TESS_STAGES | GEOMETRY_STAGE
            )),
            28 | MESH_STAGE | TASK_STAGE | raster_stages::TESS_STAGES | GEOMETRY_STAGE
        );
    }
    #[test]
    fn ray_tokens_require_maintenance_and_preserve_native_record_shape() {
        let limits = GeneratedLimits {
            stages: STAGE_RAY,
            tokens: 1 << GENERATED_TRACE_RAYS,
            max_tokens: 1,
            max_stride: 104,
            ..Default::default()
        };
        let token = [GeneratedToken {
            kind: GENERATED_TRACE_RAYS,
            ..Default::default()
        }];
        let desc = GeneratedLayoutDesc {
            header: Record::new::<GeneratedLayoutDesc>(GENERATED_LAYOUT_DESC),
            arguments: ptr::null_mut(),
            stages: STAGE_RAY,
            flags: 0,
            stride: 104,
            token_count: 1,
            reserved: 0,
            tokens: token.as_ptr(),
        };
        let enabled = GENERATED_COMMANDS | RAY_PIPELINE | RAY_MAINTENANCE;
        assert_eq!(
            layout_contract(&limits, enabled, 0, &desc, &token),
            Ok((104, 0))
        );
        assert_eq!(
            layout_contract(&limits, enabled & !RAY_MAINTENANCE, 0, &desc, &token),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            layout_contract(&limits, enabled & !RAY_PIPELINE, 0, &desc, &token),
            Err(UNSUPPORTED)
        );
        assert_eq!(public_stages(native_stages(STAGE_RAY)), STAGE_RAY);
        assert_eq!(
            public_stages(vk::VkShaderStageFlagBits_VK_SHADER_STAGE_RAYGEN_BIT_KHR),
            0
        );
    }
    #[test]
    fn action_families_and_optional_enables_are_explicit() {
        let limits = GeneratedLimits {
            stages: 28 | MESH_STAGE | TASK_STAGE,
            tokens: (1 << 11) - 2,
            max_tokens: 1,
            max_stride: 32,
            ..Default::default()
        };
        let all = GENERATED_COMMANDS | RASTER | MESH_SHADER | TASK_SHADER;
        for (stage, actions) in [
            (4, &[4][..]),
            (24, &[5, 6, 8, 9][..]),
            (MESH_STAGE | 16, &[7, 10][..]),
            (MESH_STAGE | TASK_STAGE | 16, &[7, 10][..]),
        ] {
            let desc = GeneratedLayoutDesc {
                header: Record::new::<GeneratedLayoutDesc>(GENERATED_LAYOUT_DESC),
                arguments: ptr::null_mut(),
                stages: stage,
                flags: 0,
                stride: 32,
                token_count: 1,
                reserved: 0,
                tokens: ptr::null(),
            };
            for kind in 4..=10 {
                let token = [GeneratedToken {
                    kind,
                    ..Default::default()
                }];
                let check = |enabled| layout_contract(&limits, enabled, 128, &desc, &token);
                if actions.contains(&kind) {
                    assert!(check(all).is_ok());
                    if stage != 4 {
                        assert_eq!(check(all & !RASTER), Err(UNSUPPORTED));
                    }
                    if stage & MESH_STAGE != 0 {
                        assert_eq!(check(all & !MESH_SHADER), Err(UNSUPPORTED));
                        assert_eq!(check(all & !TASK_SHADER), Err(UNSUPPORTED));
                    }
                    let absent = GeneratedLimits {
                        tokens: limits.tokens & !(1 << kind),
                        ..limits
                    };
                    assert_eq!(
                        layout_contract(&absent, all, 128, &desc, &token),
                        Err(UNSUPPORTED)
                    );
                } else {
                    assert_eq!(check(all), Err(INVALID));
                }
            }
        }
    }
    #[test]
    #[ignore = "requires Vulkan; generated command discovery, optional enabling and native layout creation"]
    fn gpu_generated_setup() {
        use crate::foundation::{ogpu_next_generated_layout_create, rust};
        let discovery = rust::discover().unwrap();
        let mut tested = 0;
        for a in rust::adapters(&discovery) {
            if a.snapshot.features.baseline_supported == 0 {
                continue;
            }
            let Some(q) = a
                .snapshot
                .queues
                .iter()
                .find(|q| q.flags & COMPUTE != 0 && q.count > 0)
            else {
                continue;
            };
            eprintln!(
                "generated available={} limits={:?}",
                a.snapshot.features.available & GENERATED_COMMANDS != 0,
                a.snapshot.generated_limits
            );
            let request = [QueueRequest {
                domain: q.domain,
                count: 1,
                priority: 0.5,
            }];
            let token = GeneratedToken {
                kind: 4,
                ..Default::default()
            };
            let desc = GeneratedLayoutDesc {
                header: Record::new::<GeneratedLayoutDesc>(GENERATED_LAYOUT_DESC),
                arguments: ptr::null_mut(),
                stages: 4,
                flags: 0,
                stride: 12,
                token_count: 1,
                reserved: 0,
                tokens: &token,
            };
            let d = rust::device_create(a, &request, &features::decode(FeatureBits::ZERO)).unwrap();
            let mut rejected = ptr::dangling_mut();
            assert_eq!(
                unsafe { ogpu_next_generated_layout_create(d.as_raw(), &desc, &mut rejected) },
                UNSUPPORTED
            );
            assert!(rejected.is_null());
            unsafe { rust::device_destroy(d) };
            if a.snapshot.features.available & GENERATED_COMMANDS != 0 {
                let d = rust::device_create(a, &request, &features::decode(GENERATED_COMMANDS))
                    .unwrap();
                let arguments = unsafe {
                    ArgumentLayout::create(
                        &*d.as_raw(),
                        &ArgumentLayoutDesc {
                            execution_class: ARGUMENT_COMPUTE,
                            byte_size: 0,
                            field_count: 0,
                            flags: 0,
                            fields: ptr::null(),
                        },
                    )
                }
                .unwrap();
                if a.snapshot.generated_limits.stages & 4 != 0 {
                    for flags in 0..4 {
                        let desc = GeneratedLayoutDesc {
                            flags,
                            arguments: ptr::from_ref(&*arguments).cast_mut(),
                            ..desc
                        };
                        let layout = unsafe { rust::generated_layout_create(d, &desc) }.unwrap();
                        unsafe { rust::generated_layout_destroy(layout) };
                    }
                }
                drop(arguments);
                unsafe { rust::device_destroy(d) };
                // Native layout creation for other advertised action families is
                // distinct from execution evidence (the C consumer is compute).
                let enabled = a.snapshot.features.available
                    & (GENERATED_COMMANDS | RASTER | MESH_SHADER | TASK_SHADER);
                let d = rust::device_create(a, &request, &features::decode(enabled)).unwrap();
                if enabled & RASTER == 0 {
                    unsafe { rust::device_destroy(d) };
                    tested += 1;
                    continue;
                }
                let arguments = unsafe {
                    ArgumentLayout::create(
                        &*d.as_raw(),
                        &ArgumentLayoutDesc {
                            execution_class: ARGUMENT_GRAPHICS,
                            byte_size: 0,
                            field_count: 0,
                            flags: 0,
                            fields: ptr::null(),
                        },
                    )
                }
                .unwrap();
                for (stage, kind) in [
                    (24, 5),
                    (24, 6),
                    (24, 8),
                    (24, 9),
                    (MESH_STAGE | 16, 7),
                    (MESH_STAGE | 16, 10),
                    (MESH_STAGE | TASK_STAGE | 16, 7),
                ] {
                    let token = GeneratedToken {
                        kind,
                        ..Default::default()
                    };
                    let desc = GeneratedLayoutDesc {
                        arguments: ptr::from_ref(&*arguments).cast_mut(),
                        stages: stage,
                        stride: 32,
                        tokens: &token,
                        ..desc
                    };
                    if layout_contract(
                        &a.snapshot.generated_limits,
                        enabled,
                        128,
                        &desc,
                        std::slice::from_ref(&token),
                    )
                    .is_ok()
                    {
                        let layout = unsafe { rust::generated_layout_create(d, &desc) }.unwrap();
                        unsafe { rust::generated_layout_destroy(layout) };
                    }
                }
                drop(arguments);
                unsafe { rust::device_destroy(d) };
            } else {
                assert!(matches!(
                    rust::device_create(a, &request, &features::decode(GENERATED_COMMANDS)),
                    Err(UNSUPPORTED)
                ));
            }
            tested += 1;
        }
        assert!(tested > 0);
    }
}
