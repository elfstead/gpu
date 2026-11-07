//! Cold native ray pipeline preparation; no shader-table allocation or stack policy.
use super::*;
struct Module<'a> {
    d: &'a Device,
    handle: vk::VkShaderModule,
}
impl Drop for Module<'_> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                (self.d.f.vkDestroyShaderModule.unwrap())(self.d.handle, self.handle, ptr::null());
            }
        }
    }
}
fn groups(
    shaders: &[Shader],
    input: &[RayGroup],
    flags: u32,
) -> Result<(Vec<vk::VkRayTracingShaderGroupCreateInfoKHR>, Box<[u8]>), Status> {
    if flags & 3 == 3 {
        return Err(INVALID);
    }
    let mut native = Vec::new();
    native
        .try_reserve_exact(input.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    let mut members = Vec::new();
    members
        .try_reserve_exact(input.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    for g in input {
        let mut mask = 0u8;
        for (i, index) in [g.general, g.closest_hit, g.any_hit, g.intersection]
            .into_iter()
            .enumerate()
        {
            if index == RAY_UNUSED_SHADER {
                continue;
            }
            let s = shaders.get(index as usize).ok_or(INVALID)?;
            let valid = match (g.kind, i) {
                (0, 0) => matches!(s.stage, SHADER_RAYGEN | SHADER_MISS | SHADER_CALLABLE),
                (1 | 2, 1) => s.stage == SHADER_CLOSEST_HIT,
                (1 | 2, 2) => s.stage == SHADER_ANY_HIT,
                (2, 3) => s.stage == SHADER_INTERSECTION,
                _ => false,
            };
            if !valid {
                return Err(INVALID);
            }
            mask |= 1 << i;
        }
        if g.kind > 2 || g.kind == 0 && mask != 1 || g.kind == 2 && mask & 8 == 0 {
            return Err(INVALID);
        }
        if g.kind != 0
            && ((flags & RAY_NO_NULL_ANY_HIT != 0 && mask & 4 == 0)
                || (flags & RAY_NO_NULL_CLOSEST_HIT != 0 && mask & 2 == 0))
        {
            return Err(INVALID);
        }
        native.push(vk::VkRayTracingShaderGroupCreateInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RAY_TRACING_SHADER_GROUP_CREATE_INFO_KHR,
            type_: g.kind,
            generalShader: g.general,
            closestHitShader: g.closest_hit,
            anyHitShader: g.any_hit,
            intersectionShader: g.intersection,
            ..Default::default()
        });
        members.push(mask);
    }
    Ok((native, members.into_boxed_slice()))
}
pub(super) unsafe fn prepare(
    d: &Device,
    desc: &ExecutableDesc,
    pipeline_flags: u64,
    layout: &ArgumentLayout,
    binaries: &pipeline_binaries::Input,
    mut action: pipeline_binaries::Action<'_>,
) -> Result<Box<Executable>, Status> {
    if d.snapshot.features.enabled & RAY_PIPELINE == 0 {
        return Err(UNSUPPORTED);
    }
    if desc.compile_flags & 4 != 0 || desc.dynamic_state & !DYNAMIC_RAY_STACK != 0 {
        return Err(INVALID);
    }
    if desc.compile_flags & 2 != 0
        && d.snapshot.generated_limits.pipeline_binding_stages & STAGE_RAY == 0
    {
        return Err(UNSUPPORTED);
    }
    let state = unsafe { record::<RayState>(desc.static_state, RAY_STATE)? };
    if state.flags & !63 != 0 {
        return Err(UNSUPPORTED);
    }
    if state.reserved != 0 || state.group_count == 0 || desc.shader_count == 0 {
        return Err(INVALID);
    }
    if state.max_recursion > d.snapshot.ray_limits.max_recursion {
        return Err(UNSUPPORTED);
    }
    if state.flags & 3 != 0 && d.snapshot.features.enabled & RAY_PRIMITIVE_CULLING == 0 {
        return Err(UNSUPPORTED);
    }
    let req = unsafe { record::<ShaderRequirements>(desc.requirements, SHADER_REQUIREMENTS)? };
    if unsafe { features::required(req.features)? } & !d.snapshot.features.enabled != 0 {
        return Err(UNSUPPORTED);
    }
    if req.local_size != [1, 1, 1] || req.shared_memory != 0 {
        return Err(INVALID);
    }
    let shaders = unsafe { array(desc.shaders, desc.shader_count)? };
    if !shaders.iter().any(|s| s.stage == SHADER_RAYGEN) {
        return Err(INVALID);
    }
    let (groups, members) = groups(
        shaders,
        unsafe { array(state.groups, state.group_count)? },
        state.flags,
    )?;
    let cache = unsafe { ExecutableCache::optional(d, desc.cache)? };
    let mut modules = Vec::new();
    modules
        .try_reserve_exact(shaders.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    let mut mappings = Vec::new();
    mappings
        .try_reserve_exact(shaders.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(shaders.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    let mut specs = Vec::new();
    specs
        .try_reserve_exact(shaders.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    for shader in shaders {
        ray::shader_stage(shader.stage)?;
        if shader.format != 0 || !shader.subgroup.is_null() {
            return Err(UNSUPPORTED);
        }
        unsafe {
            pipeline_binaries::shader_code(shader, binaries.used())?;
        }
        let name = unsafe { CStr::from_ptr(shader.entry) };
        if name.to_bytes().is_empty() || name.to_str().is_err() {
            return Err(INVALID);
        }
        let mut e = Vec::new();
        let mut spec = vk::VkSpecializationInfo::default();
        if !shader.specialization.is_null() {
            let s = unsafe { record::<Specialization>(shader.specialization, SPECIALIZATION)? };
            if s.reserved != 0
                || s.data.size > isize::MAX as usize
                || s.data.size != 0 && s.data.data.is_null()
            {
                return Err(INVALID);
            }
            let values = unsafe { array(s.entries, s.count)? };
            e.try_reserve_exact(values.len())
                .map_err(|_| OUT_OF_MEMORY)?;
            for (i, v) in values.iter().enumerate() {
                if v.size == 0
                    || v.size > s.data.size as u64
                    || u64::from(v.offset) > s.data.size as u64 - v.size
                    || values[..i].iter().any(|p| p.id == v.id)
                {
                    return Err(INVALID);
                }
                e.push(vk::VkSpecializationMapEntry {
                    constantID: v.id,
                    offset: v.offset,
                    size: v.size as usize,
                });
            }
            spec = vk::VkSpecializationInfo {
                mapEntryCount: s.count,
                pMapEntries: e.as_ptr(),
                dataSize: s.data.size,
                pData: s.data.data,
            };
        }
        entries.push(e);
        specs.push(spec);
        mappings.push(layout.mapping_info(shader.stage, ptr::null())?);
        let info = vk::VkShaderModuleCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
            codeSize: shader.code.size,
            pCode: shader.code.data.cast(),
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        if !binaries.used() {
            unsafe {
                d.result((d.f.vkCreateShaderModule.unwrap())(
                    d.handle,
                    &info,
                    ptr::null(),
                    &mut handle,
                ))?;
            }
        }
        modules.push(Module { d, handle });
    }
    let mut stages = Vec::new();
    stages
        .try_reserve_exact(shaders.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    for (i, s) in shaders.iter().enumerate() {
        stages.push(vk::VkPipelineShaderStageCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
            pNext: ptr::from_ref(&mappings[i]).cast(),
            stage: ray::shader_stage(s.stage)?,
            module: modules[i].handle,
            pName: s.entry,
            pSpecializationInfo: &specs[i],
            ..Default::default()
        });
    }
    let bits = [
        vk::VK_PIPELINE_CREATE_2_RAY_TRACING_SKIP_TRIANGLES_BIT_KHR,
        vk::VK_PIPELINE_CREATE_2_RAY_TRACING_SKIP_AABBS_BIT_KHR,
        vk::VK_PIPELINE_CREATE_2_RAY_TRACING_NO_NULL_ANY_HIT_SHADERS_BIT_KHR,
        vk::VK_PIPELINE_CREATE_2_RAY_TRACING_NO_NULL_CLOSEST_HIT_SHADERS_BIT_KHR,
        vk::VK_PIPELINE_CREATE_2_RAY_TRACING_NO_NULL_MISS_SHADERS_BIT_KHR,
        vk::VK_PIPELINE_CREATE_2_RAY_TRACING_NO_NULL_INTERSECTION_SHADERS_BIT_KHR,
    ];
    let flags = vk::VkPipelineCreateFlags2CreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO,
        flags: bits.iter().enumerate().fold(pipeline_flags, |v, (i, b)| {
            v | if state.flags & (1 << i) != 0 { *b } else { 0 }
        }),
        ..Default::default()
    };
    let stack_state = vk::VkDynamicState_VK_DYNAMIC_STATE_RAY_TRACING_PIPELINE_STACK_SIZE_KHR;
    let dynamic = vk::VkPipelineDynamicStateCreateInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO,
        dynamicStateCount: 1,
        pDynamicStates: &stack_state,
        ..Default::default()
    };
    let binary_info = binaries.native(ptr::from_ref(&flags).cast());
    let info = vk::VkRayTracingPipelineCreateInfoKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RAY_TRACING_PIPELINE_CREATE_INFO_KHR,
        pNext: binaries.link(&binary_info, ptr::from_ref(&flags).cast()),
        stageCount: desc.shader_count,
        pStages: stages.as_ptr(),
        groupCount: state.group_count,
        pGroups: groups.as_ptr(),
        maxPipelineRayRecursionDepth: state.max_recursion,
        pDynamicState: if desc.dynamic_state != 0 {
            &dynamic
        } else {
            ptr::null()
        },
        basePipelineIndex: -1,
        ..Default::default()
    };
    let mut result = Box::new(Executable {
        device: d,
        pipeline: ptr::null_mut(),
        arguments: layout,
        stages: STAGE_RAY,
        view_mask: 0,
        indirect_bindable: desc.compile_flags & 2 != 0,
        dispatch_origin: false,
        capture_data: pipeline_binaries::capture_state(desc.compile_flags),
        dynamic_state: desc.dynamic_state,
        vertex_bindings: Box::new([]),
        ray_groups: members,
    });
    if unsafe { action.resolve(d, ptr::from_ref(&info).cast())? } {
        return Ok(result);
    }
    unsafe {
        d.result((d.ray_commands.create.ok_or(UNSUPPORTED)?)(
            d.handle,
            ptr::null_mut(),
            cache,
            1,
            &info,
            ptr::null(),
            &mut result.pipeline,
        ))?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_groups_preserve_empty_hits_but_reject_wrong_members_and_flag_promises() {
        let shader = |stage| Shader {
            stage,
            format: 0,
            code: Bytes {
                data: ptr::null(),
                size: 0,
            },
            entry: ptr::null(),
            specialization: ptr::null(),
            subgroup: ptr::null(),
        };
        let shaders = [
            shader(SHADER_RAYGEN),
            shader(SHADER_MISS),
            shader(SHADER_CLOSEST_HIT),
            shader(SHADER_ANY_HIT),
            shader(SHADER_INTERSECTION),
        ];
        let empty = RayGroup {
            kind: RAY_GROUP_TRIANGLES,
            general: RAY_UNUSED_SHADER,
            closest_hit: RAY_UNUSED_SHADER,
            any_hit: RAY_UNUSED_SHADER,
            intersection: RAY_UNUSED_SHADER,
        };
        let general = RayGroup {
            kind: RAY_GROUP_GENERAL,
            general: 0,
            ..empty
        };
        let procedural = RayGroup {
            kind: RAY_GROUP_PROCEDURAL,
            intersection: 4,
            closest_hit: 2,
            any_hit: 3,
            ..empty
        };
        let (_, members) = groups(&shaders, &[general, empty, procedural], 0).unwrap();
        assert_eq!(&*members, &[1, 0, 14]);
        for (g, flags) in [
            (
                RayGroup {
                    general: 2,
                    ..general
                },
                0,
            ),
            (
                RayGroup {
                    intersection: 4,
                    ..empty
                },
                0,
            ),
            (
                RayGroup {
                    intersection: RAY_UNUSED_SHADER,
                    ..procedural
                },
                0,
            ),
            (empty, RAY_NO_NULL_CLOSEST_HIT),
            (empty, RAY_NO_NULL_ANY_HIT),
            (empty, RAY_SKIP_TRIANGLES | RAY_SKIP_AABBS),
        ] {
            assert!(matches!(groups(&shaders, &[g], flags), Err(INVALID)));
        }
        assert!(groups(
            &shaders,
            &[procedural],
            RAY_NO_NULL_ANY_HIT | RAY_NO_NULL_CLOSEST_HIT
        )
        .is_ok());
    }
}
