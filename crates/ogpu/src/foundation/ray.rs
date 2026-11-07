//! Optional native ray profiles; no scene ownership, traversal or build policy.
use super::*;
pub(super) const PIPELINE_FEATURES: FeatureBits =
    FeatureBits::union(&[RAY_PIPELINE, RAY_INDIRECT, RAY_PRIMITIVE_CULLING]);
pub(super) struct Native {
    acceleration: vk::VkPhysicalDeviceAccelerationStructureFeaturesKHR,
    query: vk::VkPhysicalDeviceRayQueryFeaturesKHR,
    pipeline: vk::VkPhysicalDeviceRayTracingPipelineFeaturesKHR,
    maintenance: vk::VkPhysicalDeviceRayTracingMaintenance1FeaturesKHR,
}
impl Native {
    pub fn new(enabled: FeatureBits) -> Self {
        Self {
            acceleration: vk::VkPhysicalDeviceAccelerationStructureFeaturesKHR {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ACCELERATION_STRUCTURE_FEATURES_KHR,
                accelerationStructure:u32::from(enabled & ACCELERATION_STRUCTURE!=0),
                accelerationStructureIndirectBuild:u32::from(enabled & ACCELERATION_INDIRECT_BUILD!=0),..Default::default()},
            query:vk::VkPhysicalDeviceRayQueryFeaturesKHR {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_RAY_QUERY_FEATURES_KHR,
                rayQuery:u32::from(enabled & RAY_QUERY!=0),..Default::default()},
            pipeline:vk::VkPhysicalDeviceRayTracingPipelineFeaturesKHR {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_RAY_TRACING_PIPELINE_FEATURES_KHR,
                rayTracingPipeline:u32::from(enabled & RAY_PIPELINE!=0),
                rayTracingPipelineTraceRaysIndirect:u32::from(enabled & RAY_INDIRECT!=0),
                rayTraversalPrimitiveCulling:u32::from(enabled & RAY_PRIMITIVE_CULLING!=0),..Default::default()},
            maintenance:vk::VkPhysicalDeviceRayTracingMaintenance1FeaturesKHR {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_RAY_TRACING_MAINTENANCE_1_FEATURES_KHR,
                rayTracingMaintenance1:u32::from(enabled & RAY_MAINTENANCE!=0),
                rayTracingPipelineTraceRaysIndirect2:u32::from(enabled & RAY_INDIRECT_TABLE!=0),..Default::default()},
        }
    }
    pub fn link(
        &mut self,
        mut next: *mut std::ffi::c_void,
        selected: FeatureBits,
    ) -> *mut std::ffi::c_void {
        if selected & RAY_MAINTENANCE != 0 {
            self.maintenance.pNext = next;
            next = ptr::from_mut(&mut self.maintenance).cast();
        }
        if selected & PIPELINE_FEATURES != 0 {
            self.pipeline.pNext = next;
            next = ptr::from_mut(&mut self.pipeline).cast();
        }
        if selected & RAY_QUERY != 0 {
            self.query.pNext = next;
            next = ptr::from_mut(&mut self.query).cast();
        }
        if selected & ACCELERATION_STRUCTURE != 0 {
            self.acceleration.pNext = next;
            next = ptr::from_mut(&mut self.acceleration).cast();
        }
        next
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(FeatureBits, RayLimits), Status> {
    let mut selected = FeatureBits::ZERO;
    for (name, bit) in [
        (c"VK_KHR_acceleration_structure", ACCELERATION_STRUCTURE),
        (c"VK_KHR_ray_query", RAY_QUERY),
        (c"VK_KHR_ray_tracing_pipeline", RAY_PIPELINE),
        (c"VK_KHR_ray_tracing_maintenance1", RAY_MAINTENANCE),
    ] {
        if instance
            .supports_extension(physical, name)
            .map_err(loader_error)?
        {
            selected |= bit;
        }
    }
    if selected & ACCELERATION_STRUCTURE == 0
        || !instance
            .supports_extension(physical, c"VK_KHR_deferred_host_operations")
            .map_err(loader_error)?
    {
        return Ok((FeatureBits::ZERO, RayLimits::default()));
    }
    let mut native = Native::new(FeatureBits::ZERO);
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: native.link(ptr::null_mut(), selected),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
    }
    if native.acceleration.accelerationStructure == 0 {
        return Ok((FeatureBits::ZERO, RayLimits::default()));
    }
    let mut available = ACCELERATION_STRUCTURE;
    for (present, bit) in [
        (
            native.acceleration.accelerationStructureIndirectBuild,
            ACCELERATION_INDIRECT_BUILD,
        ),
        (native.query.rayQuery, RAY_QUERY),
        (native.pipeline.rayTracingPipeline, RAY_PIPELINE),
        (
            native.pipeline.rayTracingPipelineTraceRaysIndirect,
            RAY_INDIRECT,
        ),
        (
            native.pipeline.rayTraversalPrimitiveCulling,
            RAY_PRIMITIVE_CULLING,
        ),
        (native.maintenance.rayTracingMaintenance1, RAY_MAINTENANCE),
        (
            native.maintenance.rayTracingPipelineTraceRaysIndirect2,
            RAY_INDIRECT_TABLE,
        ),
    ] {
        if present != 0 {
            available |= bit;
        }
    }
    let mut rt=vk::VkPhysicalDeviceRayTracingPipelinePropertiesKHR {sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_RAY_TRACING_PIPELINE_PROPERTIES_KHR,..Default::default()};
    let mut acceleration=vk::VkPhysicalDeviceAccelerationStructurePropertiesKHR {
        sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_ACCELERATION_STRUCTURE_PROPERTIES_KHR,
        pNext:if selected & RAY_PIPELINE!=0 {ptr::from_mut(&mut rt).cast()} else {ptr::null_mut()},..Default::default()};
    let mut props = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: ptr::from_mut(&mut acceleration).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut props);
    }
    Ok((
        available,
        RayLimits {
            max_geometries: acceleration.maxGeometryCount,
            max_instances: acceleration.maxInstanceCount,
            max_primitives: acceleration.maxPrimitiveCount,
            storage_alignment: 256,
            scratch_alignment: u64::from(
                acceleration.minAccelerationStructureScratchOffsetAlignment,
            ),
            group_handle_size: rt.shaderGroupHandleSize,
            group_handle_alignment: rt.shaderGroupHandleAlignment,
            table_alignment: rt.shaderGroupBaseAlignment,
            max_table_stride: rt.maxShaderGroupStride,
            max_recursion: rt.maxRayRecursionDepth,
            max_trace_invocations: rt.maxRayDispatchInvocationCount,
            max_hit_attribute_size: rt.maxRayHitAttributeSize,
            reserved: 0,
        },
    ))
}
pub(super) const ARGUMENT_STAGES: u64 = ARGUMENT_RAYGEN
    | ARGUMENT_MISS
    | ARGUMENT_CLOSEST_HIT
    | ARGUMENT_ANY_HIT
    | ARGUMENT_INTERSECTION
    | ARGUMENT_CALLABLE;
pub(super) fn shader_visibility(stage: u32) -> u64 {
    match stage {
        SHADER_RAYGEN => ARGUMENT_RAYGEN,
        SHADER_MISS => ARGUMENT_MISS,
        SHADER_CLOSEST_HIT => ARGUMENT_CLOSEST_HIT,
        SHADER_ANY_HIT => ARGUMENT_ANY_HIT,
        SHADER_INTERSECTION => ARGUMENT_INTERSECTION,
        SHADER_CALLABLE => ARGUMENT_CALLABLE,
        _ => u64::from(stage),
    }
}
pub(super) fn shader_stage(stage: u32) -> Result<u32, Status> {
    match stage {
        SHADER_RAYGEN => Ok(vk::VkShaderStageFlagBits_VK_SHADER_STAGE_RAYGEN_BIT_KHR),
        SHADER_MISS => Ok(vk::VkShaderStageFlagBits_VK_SHADER_STAGE_MISS_BIT_KHR),
        SHADER_CLOSEST_HIT => Ok(vk::VkShaderStageFlagBits_VK_SHADER_STAGE_CLOSEST_HIT_BIT_KHR),
        SHADER_ANY_HIT => Ok(vk::VkShaderStageFlagBits_VK_SHADER_STAGE_ANY_HIT_BIT_KHR),
        SHADER_INTERSECTION => Ok(vk::VkShaderStageFlagBits_VK_SHADER_STAGE_INTERSECTION_BIT_KHR),
        SHADER_CALLABLE => Ok(vk::VkShaderStageFlagBits_VK_SHADER_STAGE_CALLABLE_BIT_KHR),
        _ => Err(UNSUPPORTED),
    }
}
pub(super) struct Commands {
    pub create: vk::PFN_vkCreateRayTracingPipelinesKHR,
    pub handles: vk::PFN_vkGetRayTracingShaderGroupHandlesKHR,
    pub stack: vk::PFN_vkGetRayTracingShaderGroupStackSizeKHR,
    pub set_stack: vk::PFN_vkCmdSetRayTracingPipelineStackSizeKHR,
    pub trace: vk::PFN_vkCmdTraceRaysKHR,
    pub indirect: vk::PFN_vkCmdTraceRaysIndirectKHR,
    pub indirect_table: vk::PFN_vkCmdTraceRaysIndirect2KHR,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        macro_rules! get {
            ($name:literal,$ty:ty,$feature:expr) => {{
                if enabled & $feature == 0 {
                    None
                } else {
                    let f = unsafe {
                        std::mem::transmute::<vk::PFN_vkVoidFunction, $ty>(instance.proc($name))
                    };
                    if f.is_none() {
                        return Err(UNSUPPORTED);
                    }
                    f
                }
            }};
        }
        Ok(Self {
            create: get!(
                c"vkCreateRayTracingPipelinesKHR",
                vk::PFN_vkCreateRayTracingPipelinesKHR,
                RAY_PIPELINE
            ),
            handles: get!(
                c"vkGetRayTracingShaderGroupHandlesKHR",
                vk::PFN_vkGetRayTracingShaderGroupHandlesKHR,
                RAY_PIPELINE
            ),
            stack: get!(
                c"vkGetRayTracingShaderGroupStackSizeKHR",
                vk::PFN_vkGetRayTracingShaderGroupStackSizeKHR,
                RAY_PIPELINE
            ),
            set_stack: get!(
                c"vkCmdSetRayTracingPipelineStackSizeKHR",
                vk::PFN_vkCmdSetRayTracingPipelineStackSizeKHR,
                RAY_PIPELINE
            ),
            trace: get!(
                c"vkCmdTraceRaysKHR",
                vk::PFN_vkCmdTraceRaysKHR,
                RAY_PIPELINE
            ),
            indirect: get!(
                c"vkCmdTraceRaysIndirectKHR",
                vk::PFN_vkCmdTraceRaysIndirectKHR,
                RAY_INDIRECT
            ),
            indirect_table: get!(
                c"vkCmdTraceRaysIndirect2KHR",
                vk::PFN_vkCmdTraceRaysIndirect2KHR,
                RAY_INDIRECT_TABLE
            ),
        })
    }
}
impl Executable {
    pub(in crate::foundation) unsafe fn ray_group_handles(
        &self,
        first: u32,
        count: u32,
        output: HostSpan,
    ) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        if self.stages != STAGE_RAY {
            return Err(INVALID);
        }
        if count == 0 || first.checked_add(count).ok_or(INVALID)? as usize > self.ray_groups.len() {
            return Err(INVALID);
        }
        let size = u64::from(count) * u64::from(d.snapshot.ray_limits.group_handle_size);
        if output.size < size {
            return Err(CAPACITY);
        }
        if output.data.is_null() || size > isize::MAX as u64 {
            return Err(INVALID);
        }
        unsafe {
            d.result((d.ray_commands.handles.ok_or(UNSUPPORTED)?)(
                d.handle,
                self.pipeline,
                first,
                count,
                size as usize,
                output.data,
            ))
        }
    }
    pub(in crate::foundation) fn ray_group_stack_size(
        &self,
        group: u32,
        member: u32,
    ) -> Result<u64, Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        if self.stages != STAGE_RAY
            || member > 3
            || self
                .ray_groups
                .get(group as usize)
                .is_none_or(|m| m & (1 << member) == 0)
        {
            return Err(INVALID);
        }
        Ok(unsafe {
            (d.ray_commands.stack.ok_or(UNSUPPORTED)?)(d.handle, self.pipeline, group, member)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ray_queries_and_high_word_profiles_enable_independently() {
        assert_eq!(
            validate_feature_dependencies(ACCELERATION_STRUCTURE | RAY_QUERY),
            Ok(())
        );
        assert_eq!(validate_feature_dependencies(RAY_QUERY), Err(INVALID));
        assert_eq!(
            validate_feature_dependencies(
                ACCELERATION_STRUCTURE | RAY_QUERY | RAY_PRIMITIVE_CULLING
            ),
            Ok(())
        );
        assert_eq!(
            validate_feature_dependencies(
                ACCELERATION_STRUCTURE | RAY_PIPELINE | RAY_INDIRECT_TABLE
            ),
            Err(INVALID)
        );
        assert_eq!(
            validate_feature_dependencies(
                ACCELERATION_STRUCTURE | RAY_PIPELINE | RAY_MAINTENANCE | RAY_INDIRECT_TABLE
            ),
            Ok(())
        );
        let n = Native::new(ACCELERATION_STRUCTURE | RAY_QUERY | ACCELERATION_INDIRECT_BUILD);
        assert_eq!(n.acceleration.accelerationStructureIndirectBuild, 1);
        assert_eq!(n.query.rayQuery, 1);
        assert_eq!(n.pipeline.rayTracingPipeline, 0);
        assert_eq!(
            features::decode(features::encode(&[124, 125, 129]).unwrap()),
            vec![124, 125, 129]
        );
    }
    #[test]
    #[ignore = "requires Vulkan; emulated ray execution is a separate diagnostic lane"]
    fn gpu_ray_capabilities_and_independent_enabling() {
        let discovery = crate::foundation::rust::discover().unwrap();
        let mut tested = 0;
        for adapter in crate::foundation::rust::adapters(&discovery) {
            if adapter.snapshot.features.baseline_supported == 0 {
                continue;
            }
            let Some(queue) = adapter
                .snapshot
                .queues
                .iter()
                .find(|q| q.flags & COMPUTE != 0 && q.count != 0)
            else {
                continue;
            };
            let request = [QueueRequest {
                domain: queue.domain,
                count: 1,
                priority: 0.5,
            }];
            let available = adapter.snapshot.features.available;
            eprintln!(
                "Ray available: {:?}; limits: {:?}",
                features::decode(
                    available
                        & FeatureBits::union(&[
                            ACCELERATION_STRUCTURE,
                            RAY_QUERY,
                            PIPELINE_FEATURES,
                            RAY_INDIRECT_TABLE,
                            ACCELERATION_INDIRECT_BUILD,
                            RAY_MAINTENANCE
                        ])
                ),
                adapter.snapshot.ray_limits
            );
            for requested in [
                FeatureBits::ZERO,
                ACCELERATION_STRUCTURE,
                ACCELERATION_STRUCTURE | RAY_QUERY,
                ACCELERATION_STRUCTURE | RAY_QUERY | RAY_PRIMITIVE_CULLING,
                ACCELERATION_STRUCTURE | RAY_PIPELINE,
                ACCELERATION_STRUCTURE | RAY_PIPELINE | RAY_INDIRECT,
                ACCELERATION_STRUCTURE | RAY_MAINTENANCE,
                ACCELERATION_STRUCTURE | RAY_PIPELINE | RAY_MAINTENANCE | RAY_INDIRECT_TABLE,
                ACCELERATION_STRUCTURE | ACCELERATION_INDIRECT_BUILD,
            ] {
                let result = crate::foundation::rust::device_create(
                    adapter,
                    &request,
                    &features::decode(requested),
                );
                if requested & !available != 0 {
                    assert_eq!(result.unwrap_err(), UNSUPPORTED);
                } else {
                    let device = result.unwrap();
                    unsafe {
                        crate::foundation::rust::device_destroy(device);
                    }
                }
            }
            tested += 1;
        }
        assert_ne!(tested, 0);
    }
}
