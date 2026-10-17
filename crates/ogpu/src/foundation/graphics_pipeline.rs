//! Cold graphics preparation; no attachment owners or command-time state compilation.
use super::*;

struct Module<'a> {
    d: &'a Device,
    handle: vk::VkShaderModule,
}
impl Default for RasterOptions {
    fn default() -> Self {
        Self {
            header: Record::new::<Self>(RASTER_OPTIONS),
            primitive_restart: 0,
            depth_clamp: 0,
            discard: 0,
            polygon_mode: 0,
            depth_bias: 0,
            depth_bias_constant: 0.0,
            depth_bias_clamp: 0.0,
            depth_bias_slope: 0.0,
            line_width: 1.0,
            depth_bounds: 0,
            min_depth_bounds: 0.0,
            max_depth_bounds: 1.0,
            sample_mask: u64::MAX,
            sample_shading: 0,
            min_sample_shading: 0.0,
            alpha_to_coverage: 0,
            alpha_to_one: 0,
        }
    }
}
fn raster_options(
    o: &RasterOptions,
    features: u64,
    p: &GraphicsLimits,
    topology: u32,
) -> Result<(), Status> {
    if [
        o.primitive_restart,
        o.depth_clamp,
        o.discard,
        o.depth_bias,
        o.depth_bounds,
        o.sample_shading,
        o.alpha_to_coverage,
        o.alpha_to_one,
    ]
    .iter()
    .any(|&v| v > 1)
        || o.polygon_mode > 2
        || !o.line_width.is_finite()
        || o.line_width <= 0.0
        || ![
            o.depth_bias_constant,
            o.depth_bias_clamp,
            o.depth_bias_slope,
        ]
        .iter()
        .all(|v| v.is_finite())
        || ![o.min_depth_bounds, o.max_depth_bounds, o.min_sample_shading]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        || o.min_depth_bounds > o.max_depth_bounds
    {
        return Err(INVALID);
    }
    // List restart and independent depth clipping are separate, unimplemented profiles.
    if o.primitive_restart != 0 && !matches!(topology, 2 | 4) {
        return Err(UNSUPPORTED);
    }
    for (needed, bit) in [
        (o.depth_clamp != 0, DEPTH_CLAMP),
        (o.depth_bias_clamp != 0.0, DEPTH_BIAS_CLAMP),
        (o.polygon_mode != 0, NON_SOLID_FILL),
        (o.line_width != 1.0, WIDE_LINES),
        (o.depth_bounds != 0, DEPTH_BOUNDS),
        (o.sample_shading != 0, SAMPLE_SHADING),
        (o.alpha_to_one != 0, ALPHA_TO_ONE),
    ] {
        if needed && features & bit == 0 {
            return Err(UNSUPPORTED);
        }
    }
    if o.line_width != 1.0
        && !(p.line_width_range[0]..=p.line_width_range[1]).contains(&o.line_width)
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
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
fn graphics_stages(shaders: &[Shader]) -> Result<u64, Status> {
    let mut stages = [0; 3];
    if shaders.len() > stages.len() {
        return Err(UNSUPPORTED);
    }
    for (stage, shader) in stages.iter_mut().zip(shaders) {
        *stage = u64::from(shader.stage);
    }
    let stages = &stages[..shaders.len()];
    match stages {
        [8] | [8, 16] => Ok(stages.iter().copied().fold(0, |a, b| a | b)),
        [MESH_STAGE]
        | [MESH_STAGE, 16]
        | [TASK_STAGE, MESH_STAGE]
        | [TASK_STAGE, MESH_STAGE, 16] => Ok(stages.iter().copied().fold(0, |a, b| a | b)),
        _ => Err(UNSUPPORTED),
    }
}
pub(super) unsafe fn prepare(
    d: &Device,
    desc: &ExecutableDesc,
    pipeline_flags: u64,
) -> Result<Box<Executable>, Status> {
    if d.snapshot.features.enabled & RASTER == 0
        || desc.dynamic_state & 1 == 0
        || desc.dynamic_state & !d.snapshot.graphics_limits.dynamic_states != 0
    {
        return Err(UNSUPPORTED);
    }
    let cache = unsafe { ExecutableCache::optional(d, desc.cache)? };
    let mut state = unsafe { *record::<GraphicsState>(desc.static_state, GRAPHICS_STATE)? };
    // Native dynamic fields ignore their static counterparts. Sanitize only
    // those ignored cold-path fields before ordinary validation/native creation.
    for (bit, field) in [
        (32, &mut state.depth_test),
        (64, &mut state.depth_write),
        (128, &mut state.depth_compare),
        (256, &mut state.stencil_test),
        (1024, &mut state.cull),
        (2048, &mut state.front_face),
    ] {
        if desc.dynamic_state & bit != 0 {
            *field = 0;
        }
    }
    if desc.dynamic_state & 512 != 0 {
        for stencil in [&mut state.stencil_front, &mut state.stencil_back] {
            stencil.fail = 0;
            stencil.pass = 0;
            stencil.depth_fail = 0;
            stencil.compare = 0;
        }
    }
    if !(1..=3).contains(&desc.shader_count) {
        return Err(UNSUPPORTED);
    }
    let shaders = unsafe { array(desc.shaders, desc.shader_count)? };
    let stage_mask = graphics_stages(shaders)?;
    if desc.compile_flags & 2 != 0
        && (stage_mask & !d.snapshot.generated_limits.pipeline_binding_stages != 0
            || d.snapshot.generated_limits.pipeline_binding_stages & 16 == 0)
    {
        return Err(UNSUPPORTED);
    }
    let mesh_pipeline = stage_mask & MESH_STAGE != 0;
    let mesh_req = if mesh_pipeline {
        let req = unsafe { record::<MeshRequirements>(desc.requirements, MESH_REQUIREMENTS)? };
        mesh::requirements(
            &d.snapshot.mesh_limits,
            d.snapshot.features.enabled,
            stage_mask,
            req,
        )?;
        if !state.vertex_input.is_null() {
            return Err(INVALID);
        }
        // Mesh output topology is part of the artifact, not input assembly state.
        state.topology = 0;
        Some(req)
    } else {
        let req = unsafe { record::<ShaderRequirements>(desc.requirements, SHADER_REQUIREMENTS)? };
        if req.features & !d.snapshot.features.enabled != 0 {
            return Err(UNSUPPORTED);
        }
        if req.local_size != [0; 3] || req.shared_memory != 0 {
            return Err(INVALID);
        }
        None
    };
    let p = &d.snapshot.graphics_limits;
    let options = if state.raster_options.is_null() {
        RasterOptions::default()
    } else {
        unsafe { *record::<RasterOptions>(state.raster_options, RASTER_OPTIONS)? }
    };
    if mesh_pipeline && options.primitive_restart != 0 {
        return Err(INVALID);
    }
    raster_options(&options, d.snapshot.features.enabled, p, state.topology)?;
    let shader_count = desc.shader_count;
    if options.discard != 0 && stage_mask & 16 != 0 {
        return Err(UNSUPPORTED);
    }
    // Static discard has no fragment-output interface. Attached execution uses
    // UNUSED_ATTACHMENTS with this same zero-format pipeline, not invented formats.
    if options.discard != 0 && (state.color_count != 0 || state.depth_format != 0) {
        return Err(UNSUPPORTED);
    }
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
    if state.logic_op_enable > 1 || state.logic_op > 15 {
        return Err(INVALID);
    }
    if state.logic_op_enable != 0 && d.snapshot.features.enabled & LOGIC_OP == 0 {
        return Err(UNSUPPORTED);
    }
    if state.depth_format == 0
        && (state.depth_test != 0 || state.depth_write != 0 || state.stencil_test != 0)
    {
        return Err(INVALID);
    }
    if desc.dynamic_state & 2 == 0 && !state.blend_constants.iter().all(|f| f.is_finite()) {
        return Err(INVALID);
    }
    let colors = unsafe { array(state.colors, state.color_count)? };
    let mut sample_mask = if colors.iter().all(|c| c.format == 0) && state.depth_format == 0 {
        p.no_attachment_samples
    } else {
        u32::MAX
    };
    for c in colors {
        if c.format == 0 {
            continue;
        }
        sample_mask &= if images::format(c.format)?.integer_color() {
            p.integer_color_samples
        } else {
            p.color_samples
        };
    }
    let aspects = if state.depth_format == 0 {
        0
    } else {
        images::format(state.depth_format)?.aspects
    };
    let has_depth = aspects & 2 != 0;
    let has_stencil = aspects & 4 != 0;
    if (state.depth_test != 0 || state.depth_write != 0) && !has_depth {
        return Err(INVALID);
    }
    if has_depth {
        sample_mask &= p.depth_samples;
    }
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
        if c.write_mask & !15 != 0
            || c.blend > 1
            || c.src_color > 18
            || c.dst_color > 18
            || c.src_alpha > 18
            || c.dst_alpha > 18
            || c.color_op > 4
            || c.alpha_op > 4
        {
            return Err(INVALID);
        }
        if [c.src_color, c.dst_color, c.src_alpha, c.dst_alpha]
            .iter()
            .any(|&f| f >= 15)
            && d.snapshot.features.enabled & DUAL_SOURCE_BLEND == 0
        {
            return Err(UNSUPPORTED);
        }
        let native_format = if c.format == 0 {
            0
        } else {
            let f = images::format(c.format)?;
            if f.aspects != 1 {
                return Err(INVALID);
            }
            let mut props = vk::VkFormatProperties::default();
            unsafe {
                (d.f.vkGetPhysicalDeviceFormatProperties.unwrap())(
                    d.physical, f.native, &mut props,
                );
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
            f.native
        };
        formats.push(native_format);
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
    // Without explicit enabling, native attachment states must match. Never normalize.
    if d.snapshot.features.enabled & INDEPENDENT_BLEND == 0
        && colors.iter().skip(1).any(|c| {
            let first = colors[0];
            ColorState {
                format: first.format,
                ..*c
            } != first
        })
    {
        return Err(UNSUPPORTED);
    }
    let depth_format = if state.depth_format == 0 {
        0
    } else {
        let f = images::format(state.depth_format)?;
        if f.aspects & 6 == 0 {
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
    let mut modules = Vec::new();
    let mut stages = Vec::new();
    let mut specialization_entries = Vec::new();
    let mut specialization_data = Vec::new();
    let mut owned_roots = Vec::new();
    let mut byte_size = 0;
    let mut subgroup_states = Vec::new();
    subgroup_states
        .try_reserve_exact(shader_count as usize)
        .map_err(|_| OUT_OF_MEMORY)?;
    for (i, shader) in shaders.iter().enumerate() {
        if shader.format != 0 {
            return Err(UNSUPPORTED);
        }
        let local = mesh_req.map_or([0; 3], |r| match u64::from(shader.stage) {
            MESH_STAGE => r.mesh_local,
            TASK_STAGE => r.task_local,
            _ => [0; 3],
        });
        subgroup_states.push(unsafe { subgroups::prepare(d, shader, local)? });
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
        if roots
            .iter()
            .any(|r| r.stages & !(if mesh_pipeline { stage_mask | 16 } else { 24 }) != 0)
        {
            return Err(UNSUPPORTED);
        }
        // One shared argument namespace; all supplied stages use the same slot table.
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
            stage: match u64::from(shader.stage) {
                8 => vk::VkShaderStageFlagBits_VK_SHADER_STAGE_VERTEX_BIT,
                16 => vk::VkShaderStageFlagBits_VK_SHADER_STAGE_FRAGMENT_BIT,
                MESH_STAGE => vk::VkShaderStageFlagBits_VK_SHADER_STAGE_MESH_BIT_EXT,
                TASK_STAGE => vk::VkShaderStageFlagBits_VK_SHADER_STAGE_TASK_BIT_EXT,
                _ => unreachable!(),
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
        primitiveRestartEnable: options.primitive_restart,
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
        depthClampEnable: options.depth_clamp,
        rasterizerDiscardEnable: options.discard,
        polygonMode: options.polygon_mode,
        cullMode: state.cull,
        frontFace: state.front_face,
        depthBiasEnable: options.depth_bias,
        depthBiasConstantFactor: options.depth_bias_constant,
        depthBiasClamp: options.depth_bias_clamp,
        depthBiasSlopeFactor: options.depth_bias_slope,
        lineWidth: options.line_width,
        ..Default::default()
    };
    let mask_words = [
        options.sample_mask as u32,
        (options.sample_mask >> 32) as u32,
    ];
    let multi = vk::VkPipelineMultisampleStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
        rasterizationSamples: state.samples,
        sampleShadingEnable: options.sample_shading,
        minSampleShading: options.min_sample_shading,
        pSampleMask: mask_words.as_ptr(),
        alphaToCoverageEnable: options.alpha_to_coverage,
        alphaToOneEnable: options.alpha_to_one,
        ..Default::default()
    };
    let depth = vk::VkPipelineDepthStencilStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
        depthTestEnable: state.depth_test,
        depthWriteEnable: state.depth_write,
        depthCompareOp: state.depth_compare,
        depthBoundsTestEnable: options.depth_bounds,
        minDepthBounds: options.min_depth_bounds,
        maxDepthBounds: options.max_depth_bounds,
        stencilTestEnable: state.stencil_test,
        front,
        back,
        ..Default::default()
    };
    let blend = vk::VkPipelineColorBlendStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
        logicOpEnable: state.logic_op_enable,
        logicOp: state.logic_op,
        attachmentCount: blends.len() as u32,
        pAttachments: blends.as_ptr(),
        blendConstants: state.blend_constants,
        ..Default::default()
    };
    let mut dynamic_states = [0; 13];
    dynamic_states[0] = vk::VkDynamicState_VK_DYNAMIC_STATE_VIEWPORT;
    dynamic_states[1] = vk::VkDynamicState_VK_DYNAMIC_STATE_SCISSOR;
    let mut dynamic_count = 2;
    for (mask, state) in [
        (2, vk::VkDynamicState_VK_DYNAMIC_STATE_BLEND_CONSTANTS),
        (4, vk::VkDynamicState_VK_DYNAMIC_STATE_STENCIL_REFERENCE),
        (8, vk::VkDynamicState_VK_DYNAMIC_STATE_STENCIL_COMPARE_MASK),
        (16, vk::VkDynamicState_VK_DYNAMIC_STATE_STENCIL_WRITE_MASK),
        (32, vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_TEST_ENABLE),
        (64, vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_WRITE_ENABLE),
        (128, vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_COMPARE_OP),
        (256, vk::VkDynamicState_VK_DYNAMIC_STATE_STENCIL_TEST_ENABLE),
        (512, vk::VkDynamicState_VK_DYNAMIC_STATE_STENCIL_OP),
        (1024, vk::VkDynamicState_VK_DYNAMIC_STATE_CULL_MODE),
        (2048, vk::VkDynamicState_VK_DYNAMIC_STATE_FRONT_FACE),
    ] {
        if desc.dynamic_state & mask != 0 {
            dynamic_states[dynamic_count] = state;
            dynamic_count += 1;
        }
    }
    let dynamic = vk::VkPipelineDynamicStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO,
        dynamicStateCount: dynamic_count as u32,
        pDynamicStates: dynamic_states.as_ptr(),
        ..Default::default()
    };
    let flags = vk::VkPipelineCreateFlags2CreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO,
        flags: pipeline_flags,
        ..Default::default()
    };
    let rendering = vk::VkPipelineRenderingCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RENDERING_CREATE_INFO,
        pNext: ptr::from_ref(&flags).cast(),
        colorAttachmentCount: formats.len() as u32,
        pColorAttachmentFormats: formats.as_ptr(),
        depthAttachmentFormat: if has_depth { depth_format } else { 0 },
        stencilAttachmentFormat: if has_stencil { depth_format } else { 0 },
        ..Default::default()
    };
    let info = vk::VkGraphicsPipelineCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO,
        pNext: ptr::from_ref(&rendering).cast(),
        stageCount: shader_count,
        pStages: stages.as_ptr(),
        pVertexInputState: if mesh_pipeline { ptr::null() } else { &vertex },
        pInputAssemblyState: if mesh_pipeline {
            ptr::null()
        } else {
            &assembly
        },
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
        stages: stage_mask,
        indirect_bindable: desc.compile_flags & 2 != 0,
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
    fn graphics_stage_families_do_not_mix_or_reorder() {
        for stages in [
            vec![8],
            vec![8, 16],
            vec![MESH_STAGE],
            vec![MESH_STAGE, 16],
            vec![TASK_STAGE, MESH_STAGE],
            vec![TASK_STAGE, MESH_STAGE, 16],
            vec![],
            vec![TASK_STAGE],
            vec![16],
            vec![8, MESH_STAGE],
            vec![MESH_STAGE, TASK_STAGE],
            vec![TASK_STAGE, MESH_STAGE, 8],
            vec![8, 16, 16],
            vec![TASK_STAGE, MESH_STAGE, 16, 16],
        ] {
            let shaders: Vec<_> = stages
                .iter()
                .map(|&stage| Shader {
                    stage: stage as u32,
                    format: 0,
                    code: Bytes {
                        data: ptr::null(),
                        size: 0,
                    },
                    entry: ptr::null(),
                    interface_metadata: ptr::null(),
                    specialization: ptr::null(),
                    subgroup: ptr::null(),
                })
                .collect();
            let valid = matches!(
                stages.as_slice(),
                [8] | [8, 16]
                    | [MESH_STAGE]
                    | [MESH_STAGE, 16]
                    | [TASK_STAGE, MESH_STAGE]
                    | [TASK_STAGE, MESH_STAGE, 16]
            );
            assert_eq!(
                graphics_stages(&shaders),
                if valid {
                    Ok(stages.iter().fold(0, |a, b| a | b))
                } else {
                    Err(UNSUPPORTED)
                }
            );
        }
    }
    #[test]
    fn raster_options_validate_without_normalizing_or_enabling_features() {
        let p = GraphicsLimits {
            line_width_range: [1.0, 8.0],
            ..Default::default()
        };
        let base = RasterOptions::default();
        assert_eq!(base.sample_mask, u64::MAX);
        assert_eq!(raster_options(&base, 0, &p, 3), Ok(()));
        for (bit, options) in [
            (
                DEPTH_CLAMP,
                RasterOptions {
                    depth_clamp: 1,
                    ..base
                },
            ),
            (
                DEPTH_BIAS_CLAMP,
                RasterOptions {
                    depth_bias_clamp: -1.0,
                    ..base
                },
            ),
            (
                NON_SOLID_FILL,
                RasterOptions {
                    polygon_mode: 2,
                    ..base
                },
            ),
            (
                WIDE_LINES,
                RasterOptions {
                    line_width: 2.0,
                    ..base
                },
            ),
            (
                DEPTH_BOUNDS,
                RasterOptions {
                    depth_bounds: 1,
                    min_depth_bounds: 0.25,
                    max_depth_bounds: 0.75,
                    ..base
                },
            ),
            (
                SAMPLE_SHADING,
                RasterOptions {
                    sample_shading: 1,
                    min_sample_shading: 0.5,
                    ..base
                },
            ),
            (
                ALPHA_TO_ONE,
                RasterOptions {
                    alpha_to_one: 1,
                    ..base
                },
            ),
        ] {
            assert_eq!(raster_options(&options, 0, &p, 3), Err(UNSUPPORTED));
            assert_eq!(raster_options(&options, !bit, &p, 3), Err(UNSUPPORTED));
            assert_eq!(raster_options(&options, bit, &p, 3), Ok(()));
        }
        for options in [
            RasterOptions { discard: 1, ..base },
            RasterOptions {
                sample_mask: 0,
                ..base
            },
            RasterOptions {
                sample_mask: 1 << 63,
                ..base
            },
            RasterOptions {
                depth_bias: 1,
                depth_bias_constant: -2.0,
                depth_bias_slope: 3.0,
                ..base
            },
            RasterOptions {
                alpha_to_coverage: 1,
                ..base
            },
        ] {
            assert_eq!(raster_options(&options, 0, &p, 3), Ok(()));
        }
        let restart = RasterOptions {
            primitive_restart: 1,
            ..base
        };
        for topology in 0..=4 {
            assert_eq!(
                raster_options(&restart, u64::MAX, &p, topology),
                if matches!(topology, 2 | 4) {
                    Ok(())
                } else {
                    Err(UNSUPPORTED)
                }
            );
        }
        for options in [
            RasterOptions { discard: 2, ..base },
            RasterOptions {
                primitive_restart: 2,
                ..base
            },
            RasterOptions {
                depth_clamp: 2,
                ..base
            },
            RasterOptions {
                depth_bias: 2,
                ..base
            },
            RasterOptions {
                depth_bounds: 2,
                ..base
            },
            RasterOptions {
                sample_shading: 2,
                ..base
            },
            RasterOptions {
                alpha_to_coverage: 2,
                ..base
            },
            RasterOptions {
                alpha_to_one: 2,
                ..base
            },
            RasterOptions {
                polygon_mode: 3,
                ..base
            },
            RasterOptions {
                line_width: 0.0,
                ..base
            },
            RasterOptions {
                line_width: f32::NAN,
                ..base
            },
            RasterOptions {
                depth_bias_constant: f32::INFINITY,
                ..base
            },
            RasterOptions {
                depth_bias_clamp: f32::NAN,
                ..base
            },
            RasterOptions {
                depth_bias_slope: f32::NEG_INFINITY,
                ..base
            },
            RasterOptions {
                min_depth_bounds: 0.75,
                max_depth_bounds: 0.25,
                ..base
            },
            RasterOptions {
                min_depth_bounds: -0.1,
                ..base
            },
            RasterOptions {
                max_depth_bounds: 1.1,
                ..base
            },
            RasterOptions {
                min_sample_shading: 1.1,
                ..base
            },
        ] {
            assert_eq!(raster_options(&options, u64::MAX, &p, 3), Err(INVALID));
        }
        assert_eq!(
            raster_options(
                &RasterOptions {
                    line_width: 9.0,
                    ..base
                },
                WIDE_LINES,
                &p,
                3
            ),
            Err(UNSUPPORTED)
        );
    }
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
