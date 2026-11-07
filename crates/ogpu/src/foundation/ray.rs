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
