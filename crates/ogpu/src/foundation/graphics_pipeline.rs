//! Cold graphics preparation; no attachment owners or command-time state compilation.
use super::*;

struct Module<'a> {
    d: &'a Device,
    handle: vk::VkShaderModule,
}
fn stencil(s: &StencilState) -> Result<vk::VkStencilOpState, Status> {
    if s.fail > 7 || s.pass > 7 || s.depth_fail > 7 || s.compare > 7 {
        return Err(INVALID);
    }
    Ok(vk::VkStencilOpState {
        failOp: s.fail,
        passOp: s.pass,
        depthFailOp: s.depth_fail,
        compareOp: s.compare,
        compareMask: s.compare_mask,
        writeMask: s.write_mask,
        reference: s.reference,
    })
}
impl Drop for Module<'_> {
    fn drop(&mut self) {
        unsafe {
            (self.d.f.vkDestroyShaderModule.unwrap())(self.d.handle, self.handle, ptr::null());
        }
    }
}
pub(super) unsafe fn prepare(d: &Device, desc: &ExecutableDesc) -> Result<Box<Executable>, Status> {
    if d.snapshot.features.enabled & RASTER == 0
        || desc.shader_count != 2
        || desc.dynamic_state != 1
    {
        return Err(UNSUPPORTED);
    }
    let cache = unsafe { ExecutableCache::optional(d, desc.cache)? };
    let state = unsafe { record::<GraphicsState>(desc.static_state, GRAPHICS_STATE)? };
    let req = unsafe { record::<ShaderRequirements>(desc.requirements, SHADER_REQUIREMENTS)? };
    if req.features & !d.snapshot.features.enabled != 0 {
        return Err(UNSUPPORTED);
    }
    if req.local_size != [0; 3] || req.shared_memory != 0 {
        return Err(INVALID);
    }
    let p = &d.snapshot.graphics_limits;
    if state.topology > 4
        || state.cull > 3
        || state.front_face > 1
        || state.depth_test > 1
        || state.depth_write > 1
        || state.depth_compare > 7
        || state.stencil_test > 1
    {
        return Err(UNSUPPORTED);
    }
    if state.color_count > p.max_colors {
        return Err(UNSUPPORTED);
    }
    if state.depth_format == 0
        && (state.depth_test != 0 || state.depth_write != 0 || state.stencil_test != 0)
    {
        return Err(INVALID);
    }
    if !state.blend_constants.iter().all(|f| f.is_finite()) {
        return Err(INVALID);
    }
    let colors = unsafe { array(state.colors, state.color_count)? };
    let mut sample_mask = if colors.is_empty() && state.depth_format == 0 {
        p.no_attachment_samples
    } else {
        u32::MAX
    };
    for c in colors {
        sample_mask &= if c.format == 11 {
            p.integer_color_samples
        } else {
            p.color_samples
        };
    }
    if state.depth_format != 0 {
        sample_mask &= p.depth_samples;
    }
    let has_stencil = matches!(state.depth_format, 15 | 16);
    if has_stencil {
        sample_mask &= p.stencil_samples;
    }
    if state.stencil_test != 0 && !has_stencil {
        return Err(INVALID);
    }
    let front = stencil(&state.stencil_front)?;
    let back = stencil(&state.stencil_back)?;
    samples(state.samples, sample_mask)?;
    let (vertex_bindings, vertex_attributes) = if state.vertex_input.is_null() {
        (Vec::new(), Vec::new())
    } else {
        let input = unsafe { record::<VertexInput>(state.vertex_input, VERTEX_INPUT)? };
        if input.binding_count > p.max_vertex_bindings
            || input.attribute_count > p.max_vertex_attributes
        {
            return Err(UNSUPPORTED);
        }
        let bindings = unsafe { array(input.bindings, input.binding_count)? };
        let attributes = unsafe { array(input.attributes, input.attribute_count)? };
        vertex_layout(p, bindings, attributes)?;
        let mut native_bindings = Vec::new();
        let mut native_attributes = Vec::new();
        native_bindings
            .try_reserve_exact(bindings.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        native_attributes
            .try_reserve_exact(attributes.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        for binding in bindings {
            native_bindings.push(vk::VkVertexInputBindingDescription {
                binding: binding.binding,
                stride: binding.stride,
                inputRate: binding.rate,
            });
        }
        for attribute in attributes {
            let format = images::format(attribute.format)?;
            if format.aspects != 1 {
                return Err(INVALID);
            }
            let mut properties = vk::VkFormatProperties::default();
            unsafe {
                (d.f.vkGetPhysicalDeviceFormatProperties.unwrap())(
                    d.physical,
                    format.native,
                    &mut properties,
                );
            }
            if properties.bufferFeatures
                & vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_VERTEX_BUFFER_BIT
                == 0
            {
                return Err(UNSUPPORTED);
            }
            native_attributes.push(vk::VkVertexInputAttributeDescription {
                location: attribute.location,
                binding: attribute.binding,
                format: format.native,
                offset: attribute.offset,
            });
        }
        (native_bindings, native_attributes)
    };
    let mut formats = Vec::new();
    let mut blends = Vec::new();
    formats
        .try_reserve_exact(colors.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    blends
        .try_reserve_exact(colors.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    for c in colors {
        let f = images::format(c.format)?;
        if f.aspects != 1
            || c.write_mask & !15 != 0
            || c.blend > 1
            || c.src_color > 14
            || c.dst_color > 14
            || c.src_alpha > 14
            || c.dst_alpha > 14
            || c.color_op > 4
            || c.alpha_op > 4
        {
            return Err(INVALID);
        }
        let mut props = vk::VkFormatProperties::default();
        unsafe {
            (d.f.vkGetPhysicalDeviceFormatProperties.unwrap())(d.physical, f.native, &mut props);
        }
        let flags = vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BIT
            | if c.blend != 0 {
                vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BLEND_BIT
            } else {
                0
            };
        if props.optimalTilingFeatures & flags != flags {
            return Err(UNSUPPORTED);
        }
        formats.push(f.native);
        blends.push(vk::VkPipelineColorBlendAttachmentState {
            blendEnable: c.blend,
            srcColorBlendFactor: c.src_color,
            dstColorBlendFactor: c.dst_color,
            colorBlendOp: c.color_op,
            srcAlphaBlendFactor: c.src_alpha,
            dstAlphaBlendFactor: c.dst_alpha,
            alphaBlendOp: c.alpha_op,
            colorWriteMask: c.write_mask,
        });
    }
    // independentBlend is not enabled in this profile. Do not normalize differing states.
    if colors.iter().skip(1).any(|c| {
        let first = colors[0];
        ColorState {
            format: first.format,
            ..*c
        } != first
    }) {
        return Err(UNSUPPORTED);
    }
    let depth_format = if state.depth_format == 0 {
        0
    } else {
        let f = images::format(state.depth_format)?;
        if f.aspects & 2 == 0 {
            return Err(UNSUPPORTED);
        }
        let mut props = vk::VkFormatProperties::default();
        unsafe {
            (d.f.vkGetPhysicalDeviceFormatProperties.unwrap())(d.physical, f.native, &mut props);
        }
        if props.optimalTilingFeatures
            & vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_DEPTH_STENCIL_ATTACHMENT_BIT
            == 0
        {
            return Err(UNSUPPORTED);
        }
        f.native
    };
    let shaders = unsafe { array(desc.shaders, 2)? };
    let mut modules = Vec::new();
    let mut stages = Vec::new();
    let mut specialization_entries = Vec::new();
    let mut specialization_data = Vec::new();
    let mut owned_roots = Vec::new();
    let mut byte_size = 0;
    let mut subgroup_states = Vec::new();
    subgroup_states
        .try_reserve_exact(2)
        .map_err(|_| OUT_OF_MEMORY)?;
    for (i, shader) in shaders.iter().enumerate() {
        if shader.stage != [8, 16][i] || shader.format != 0 {
            return Err(UNSUPPORTED);
        }
        subgroup_states.push(unsafe { subgroups::prepare(d, shader, [0; 3])? });
        if shader.code.data.is_null()
            || shader.code.data as usize % 4 != 0
            || shader.code.size < 20
            || shader.code.size % 4 != 0
            || shader.code.size > isize::MAX as usize
            || shader.entry.is_null()
        {
            return Err(INVALID);
        }
        if unsafe { shader.code.data.cast::<u32>().read() } != 0x07230203 {
            return Err(INVALID);
        }
        let entry = unsafe { CStr::from_ptr(shader.entry) };
        if entry.to_bytes().is_empty() || entry.to_str().is_err() {
            return Err(INVALID);
        }
        let abi =
            unsafe { record::<ArgumentInterface>(shader.interface_metadata, ARGUMENT_INTERFACE)? };
        let roots = unsafe { array(abi.roots, abi.root_count)? };
        interface(&d.snapshot.execution_limits, abi, roots)?;
        if roots.iter().any(|r| r.stages & !24 != 0) {
            return Err(UNSUPPORTED);
        }
        // One shared argument namespace; the complete slot table is identical for both stages.
        if i == 0 {
            byte_size = abi.byte_size;
            owned_roots
                .try_reserve_exact(roots.len())
                .map_err(|_| OUT_OF_MEMORY)?;
            owned_roots.extend_from_slice(roots);
        } else if abi.byte_size != byte_size
            || roots.len() != owned_roots.len()
            || roots.iter().zip(&owned_roots).any(|(a, b)| {
                a.offset != b.offset || a.alignment != b.alignment || a.stages != b.stages
            })
        {
            return Err(INVALID);
        }
        let mut entries = Vec::new();
        let mut data = Bytes {
            data: ptr::null(),
            size: 0,
        };
        if !shader.specialization.is_null() {
            let s = unsafe { record::<Specialization>(shader.specialization, SPECIALIZATION)? };
            if s.reserved != 0 {
                return Err(UNSUPPORTED);
            }
            if s.data.size > isize::MAX as usize || (s.data.size != 0 && s.data.data.is_null()) {
                return Err(INVALID);
            }
            let values = unsafe { array(s.entries, s.count)? };
            entries
                .try_reserve_exact(values.len())
                .map_err(|_| OUT_OF_MEMORY)?;
            for (j, v) in values.iter().enumerate() {
                if v.size == 0
                    || v.size > s.data.size as u64
                    || u64::from(v.offset) > s.data.size as u64 - v.size
                    || values[..j].iter().any(|p| p.id == v.id)
                {
                    return Err(INVALID);
                }
                entries.push(vk::VkSpecializationMapEntry {
                    constantID: v.id,
                    offset: v.offset,
                    size: v.size as usize,
                });
            }
            data = s.data;
        }
        specialization_entries.push(entries);
        specialization_data.push(data);
        let info = vk::VkShaderModuleCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
            codeSize: shader.code.size,
            pCode: shader.code.data.cast(),
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            d.result((d.f.vkCreateShaderModule.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        modules.push(Module { d, handle });
    }
    let specs: Vec<_> = specialization_entries
        .iter()
        .zip(&specialization_data)
        .map(|(e, b)| vk::VkSpecializationInfo {
            mapEntryCount: e.len() as u32,
            pMapEntries: e.as_ptr(),
            dataSize: b.size,
            pData: b.data,
        })
        .collect();
    for (i, shader) in shaders.iter().enumerate() {
        stages.push(vk::VkPipelineShaderStageCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
            flags: subgroup_states[i].0,
            pNext: if subgroup_states[i].1.requiredSubgroupSize == 0 {
                ptr::null()
            } else {
                ptr::from_ref(&subgroup_states[i].1).cast()
            },
            stage: if i == 0 {
                vk::VkShaderStageFlagBits_VK_SHADER_STAGE_VERTEX_BIT
            } else {
                vk::VkShaderStageFlagBits_VK_SHADER_STAGE_FRAGMENT_BIT
            },
            module: modules[i].handle,
            pName: shader.entry,
            pSpecializationInfo: &specs[i],
        });
    }
    let vertex = vk::VkPipelineVertexInputStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
        vertexBindingDescriptionCount: vertex_bindings.len() as u32,
        pVertexBindingDescriptions: vertex_bindings.as_ptr(),
        vertexAttributeDescriptionCount: vertex_attributes.len() as u32,
        pVertexAttributeDescriptions: vertex_attributes.as_ptr(),
        ..Default::default()
    };
    let assembly = vk::VkPipelineInputAssemblyStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
        topology: state.topology,
        ..Default::default()
    };
    let viewport = vk::VkPipelineViewportStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO,
        viewportCount: 1,
        scissorCount: 1,
        ..Default::default()
    };
    let raster = vk::VkPipelineRasterizationStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
        polygonMode: vk::VkPolygonMode_VK_POLYGON_MODE_FILL,
        cullMode: state.cull,
        frontFace: state.front_face,
        lineWidth: 1.0,
        ..Default::default()
    };
    let multi = vk::VkPipelineMultisampleStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
        rasterizationSamples: state.samples,
        ..Default::default()
    };
    let depth = vk::VkPipelineDepthStencilStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
        depthTestEnable: state.depth_test,
        depthWriteEnable: state.depth_write,
        depthCompareOp: state.depth_compare,
        stencilTestEnable: state.stencil_test,
        front,
        back,
        ..Default::default()
    };
    let blend = vk::VkPipelineColorBlendStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
        attachmentCount: blends.len() as u32,
        pAttachments: blends.as_ptr(),
        blendConstants: state.blend_constants,
        ..Default::default()
    };
    let dynamic_states = [
        vk::VkDynamicState_VK_DYNAMIC_STATE_VIEWPORT,
        vk::VkDynamicState_VK_DYNAMIC_STATE_SCISSOR,
    ];
    let dynamic = vk::VkPipelineDynamicStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO,
        dynamicStateCount: 2,
        pDynamicStates: dynamic_states.as_ptr(),
        ..Default::default()
    };
    let flags = vk::VkPipelineCreateFlags2CreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO,
        flags: vk::VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT,
        ..Default::default()
    };
    let rendering = vk::VkPipelineRenderingCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RENDERING_CREATE_INFO,
        pNext: ptr::from_ref(&flags).cast(),
        colorAttachmentCount: formats.len() as u32,
        pColorAttachmentFormats: formats.as_ptr(),
        depthAttachmentFormat: depth_format,
        stencilAttachmentFormat: if has_stencil { depth_format } else { 0 },
        ..Default::default()
    };
    let info = vk::VkGraphicsPipelineCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO,
        pNext: ptr::from_ref(&rendering).cast(),
        stageCount: 2,
        pStages: stages.as_ptr(),
        pVertexInputState: &vertex,
        pInputAssemblyState: &assembly,
        pViewportState: &viewport,
        pRasterizationState: &raster,
        pMultisampleState: &multi,
        pDepthStencilState: &depth,
        pColorBlendState: &blend,
        pDynamicState: &dynamic,
        basePipelineIndex: -1,
        ..Default::default()
    };
    let mut result = Box::new(Executable {
        device: d,
        pipeline: ptr::null_mut(),
        byte_size,
        roots: owned_roots,
        stages: 24,
    });
    unsafe {
        d.result((d.f.vkCreateGraphicsPipelines.unwrap())(
            d.handle,
            cache,
            1,
            &info,
            ptr::null(),
            &mut result.pipeline,
        ))?;
    }
    Ok(result)
}

fn vertex_layout(
    limits: &GraphicsLimits,
    bindings: &[VertexBinding],
    attributes: &[VertexAttribute],
) -> Result<(), Status> {
    for (index, binding) in bindings.iter().enumerate() {
        if binding.rate > 1
            || bindings[..index]
                .iter()
                .any(|b| b.binding == binding.binding)
        {
            return Err(INVALID);
        }
        if binding.binding >= limits.max_vertex_bindings
            || binding.stride > limits.max_vertex_stride
        {
            return Err(UNSUPPORTED);
        }
    }
    for (index, attribute) in attributes.iter().enumerate() {
        if !bindings.iter().any(|b| b.binding == attribute.binding)
            || attributes[..index]
                .iter()
                .any(|a| a.location == attribute.location)
        {
            return Err(INVALID);
        }
        if attribute.location >= limits.max_vertex_attributes
            || attribute.offset > limits.max_vertex_attribute_offset
        {
            return Err(UNSUPPORTED);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vertex_layout_preserves_zero_stride_sparse_and_overlapping_fetch() {
        let limits = GraphicsLimits {
            max_vertex_bindings: 8,
            max_vertex_attributes: 16,
            max_vertex_stride: 2048,
            max_vertex_attribute_offset: 2047,
            ..Default::default()
        };
        let mut bindings = [
            VertexBinding {
                binding: 7,
                stride: 0,
                rate: 1,
            },
            VertexBinding {
                binding: 3,
                stride: 4,
                rate: 0,
            },
        ];
        let mut attributes = [
            VertexAttribute {
                location: 15,
                binding: 7,
                format: 17,
                offset: 2047,
            },
            VertexAttribute {
                location: 3,
                binding: 3,
                format: 18,
                offset: 8,
            },
        ];
        assert_eq!(vertex_layout(&limits, &bindings, &attributes), Ok(()));
        bindings[1].binding = 7;
        assert_eq!(vertex_layout(&limits, &bindings, &attributes), Err(INVALID));
        bindings[1].binding = 3;
        bindings[1].stride = 2049;
        assert_eq!(
            vertex_layout(&limits, &bindings, &attributes),
            Err(UNSUPPORTED)
        );
        bindings[1].stride = 4;
        bindings[1].rate = 2;
        assert_eq!(vertex_layout(&limits, &bindings, &attributes), Err(INVALID));
        bindings[1].rate = 0;
        attributes[1].location = 15;
        assert_eq!(vertex_layout(&limits, &bindings, &attributes), Err(INVALID));
        attributes[1].location = 16;
        assert_eq!(
            vertex_layout(&limits, &bindings, &attributes),
            Err(UNSUPPORTED)
        );
        attributes[1].location = 3;
        attributes[1].binding = 0;
        assert_eq!(vertex_layout(&limits, &bindings, &attributes), Err(INVALID));
        attributes[1].binding = 3;
        attributes[1].offset = 2048;
        assert_eq!(
            vertex_layout(&limits, &bindings, &attributes),
            Err(UNSUPPORTED)
        );
    }
    #[test]
    fn stencil_fields_are_not_normalized() {
        let s = StencilState {
            fail: 7,
            pass: 6,
            depth_fail: 5,
            compare: 4,
            compare_mask: 0x12345678,
            write_mask: 0xfedcba98,
            reference: u32::MAX,
        };
        let native = stencil(&s).unwrap();
        assert_eq!(
            (
                native.failOp,
                native.passOp,
                native.depthFailOp,
                native.compareOp
            ),
            (7, 6, 5, 4)
        );
        assert_eq!(
            (native.compareMask, native.writeMask, native.reference),
            (s.compare_mask, s.write_mask, s.reference)
        );
        for bad in [
            StencilState { fail: 8, ..s },
            StencilState { pass: 8, ..s },
            StencilState { depth_fail: 8, ..s },
            StencilState { compare: 8, ..s },
        ] {
            assert!(matches!(stencil(&bad), Err(INVALID)));
        }
    }
}
