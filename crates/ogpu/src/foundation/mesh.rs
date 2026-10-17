//! Optional native mesh shading. No vertex emulation or command-time compilation.
use super::*;

pub(super) struct MeshFunctions {
    pub(super) direct: vk::PFN_vkCmdDrawMeshTasksEXT,
    pub(super) indirect: vk::PFN_vkCmdDrawMeshTasksIndirect2EXT,
    pub(super) count: vk::PFN_vkCmdDrawMeshTasksIndirectCount2EXT,
}
impl MeshFunctions {
    pub(super) fn load(instance: &Instance) -> Result<Self, Status> {
        let f = Self {
            // SAFETY: native names match the generated function pointer types.
            direct: unsafe {
                std::mem::transmute::<vk::PFN_vkVoidFunction, vk::PFN_vkCmdDrawMeshTasksEXT>(
                    instance.proc(c"vkCmdDrawMeshTasksEXT"),
                )
            },
            indirect: unsafe {
                std::mem::transmute::<vk::PFN_vkVoidFunction, vk::PFN_vkCmdDrawMeshTasksIndirect2EXT>(
                    instance.proc(c"vkCmdDrawMeshTasksIndirect2EXT"),
                )
            },
            count: unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkCmdDrawMeshTasksIndirectCount2EXT,
                >(instance.proc(c"vkCmdDrawMeshTasksIndirectCount2EXT"))
            },
        };
        if f.direct.is_none() || f.indirect.is_none() || f.count.is_none() {
            return Err(UNSUPPORTED);
        }
        Ok(f)
    }
}

pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(u64, MeshLimits), Status> {
    if !instance
        .supports_extension(physical, c"VK_EXT_mesh_shader")
        .map_err(loader_error)?
    {
        return Ok((0, MeshLimits::default()));
    }
    let mut mesh = vk::VkPhysicalDeviceMeshShaderFeaturesEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MESH_SHADER_FEATURES_EXT,
        ..Default::default()
    };
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: ptr::from_mut(&mut mesh).cast(),
        ..Default::default()
    };
    let mut p = vk::VkPhysicalDeviceMeshShaderPropertiesEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_MESH_SHADER_PROPERTIES_EXT,
        ..Default::default()
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
    let features = if mesh.meshShader != 0 {
        MESH_SHADER | if mesh.taskShader != 0 { TASK_SHADER } else { 0 }
    } else {
        0
    };
    Ok((features, limits(&p)))
}

fn limits(p: &vk::VkPhysicalDeviceMeshShaderPropertiesEXT) -> MeshLimits {
    MeshLimits {
        task: MeshWorkgroupLimits {
            max_groups: p.maxTaskWorkGroupCount,
            max_total_groups: p.maxTaskWorkGroupTotalCount,
            max_local: p.maxTaskWorkGroupSize,
            max_invocations: p.maxTaskWorkGroupInvocations,
            max_shared_memory: p.maxTaskSharedMemorySize,
        },
        mesh: MeshWorkgroupLimits {
            max_groups: p.maxMeshWorkGroupCount,
            max_total_groups: p.maxMeshWorkGroupTotalCount,
            max_local: p.maxMeshWorkGroupSize,
            max_invocations: p.maxMeshWorkGroupInvocations,
            max_shared_memory: p.maxMeshSharedMemorySize,
        },
        max_task_payload: p.maxTaskPayloadSize,
        max_task_payload_shared: p.maxTaskPayloadAndSharedMemorySize,
        max_mesh_payload_shared: p.maxMeshPayloadAndSharedMemorySize,
        max_mesh_output_memory: p.maxMeshOutputMemorySize,
        max_mesh_payload_output: p.maxMeshPayloadAndOutputMemorySize,
        max_mesh_output_components: p.maxMeshOutputComponents,
        max_mesh_output_vertices: p.maxMeshOutputVertices,
        max_mesh_output_primitives: p.maxMeshOutputPrimitives,
        max_mesh_output_layers: p.maxMeshOutputLayers,
        max_mesh_multiview_views: p.maxMeshMultiviewViewCount,
        output_vertex_granularity: p.meshOutputPerVertexGranularity,
        output_primitive_granularity: p.meshOutputPerPrimitiveGranularity,
        preferred_task_invocations: p.maxPreferredTaskWorkGroupInvocations,
        preferred_mesh_invocations: p.maxPreferredMeshWorkGroupInvocations,
        preference_flags: u32::from(p.prefersLocalInvocationVertexOutput != 0)
            | (u32::from(p.prefersLocalInvocationPrimitiveOutput != 0) << 1)
            | (u32::from(p.prefersCompactVertexOutput != 0) << 2)
            | (u32::from(p.prefersCompactPrimitiveOutput != 0) << 3),
        reserved: 0,
    }
}

pub(super) fn groups(limits: &MeshWorkgroupLimits, groups: [u32; 3]) -> Result<(), Status> {
    if groups
        .iter()
        .zip(limits.max_groups)
        .any(|(&n, max)| n > max)
    {
        return Err(INVALID);
    }
    let total = groups
        .into_iter()
        .try_fold(1u64, |n, v| n.checked_mul(u64::from(v)))
        .ok_or(INVALID)?;
    if total > u64::from(limits.max_total_groups) {
        return Err(INVALID);
    }
    Ok(())
}

pub(super) fn requirements(
    p: &MeshLimits,
    enabled: u64,
    stages: u64,
    r: &MeshRequirements,
) -> Result<(), Status> {
    if enabled & MESH_SHADER == 0
        || r.features & !enabled != 0
        || stages & TASK_STAGE != 0 && enabled & TASK_SHADER == 0
    {
        return Err(UNSUPPORTED);
    }
    if stages & TASK_STAGE == 0
        && (r.task_local != [0; 3] || r.task_shared != 0 || r.payload_size != 0)
    {
        return Err(INVALID);
    }
    for (local, shared, l, present) in [
        (r.mesh_local, r.mesh_shared, &p.mesh, true),
        (
            r.task_local,
            r.task_shared,
            &p.task,
            stages & TASK_STAGE != 0,
        ),
    ] {
        if !present {
            continue;
        }
        if local.contains(&0) {
            return Err(INVALID);
        }
        let total = local
            .into_iter()
            .try_fold(1u64, |n, v| n.checked_mul(u64::from(v)))
            .ok_or(UNSUPPORTED)?;
        if local.iter().zip(l.max_local).any(|(&n, max)| n > max)
            || total > u64::from(l.max_invocations)
            || shared > l.max_shared_memory
        {
            return Err(UNSUPPORTED);
        }
    }
    for (size, max) in [
        (u64::from(r.payload_size), p.max_task_payload),
        (
            u64::from(r.payload_size) + u64::from(r.task_shared),
            p.max_task_payload_shared,
        ),
        (
            u64::from(r.payload_size) + u64::from(r.mesh_shared),
            p.max_mesh_payload_shared,
        ),
        (u64::from(r.output_memory), p.max_mesh_output_memory),
        (
            u64::from(r.payload_size) + u64::from(r.output_memory),
            p.max_mesh_payload_output,
        ),
        (u64::from(r.output_vertices), p.max_mesh_output_vertices),
        (u64::from(r.output_primitives), p.max_mesh_output_primitives),
        (u64::from(r.output_components), p.max_mesh_output_components),
        (u64::from(r.output_layers), p.max_mesh_output_layers),
    ] {
        if size > u64::from(max) {
            return Err(UNSUPPORTED);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{ogpu_next_device_create, rust};
    #[test]
    fn dimensions_and_feature_dependencies_are_explicit() {
        let limits = MeshWorkgroupLimits {
            max_groups: [32; 3],
            max_total_groups: 64,
            ..Default::default()
        };
        assert_eq!(groups(&limits, [4, 4, 4]), Ok(()));
        assert_eq!(groups(&limits, [0, 32, 32]), Ok(()));
        assert_eq!(groups(&limits, [0, 33, 1]), Err(INVALID));
        assert_eq!(groups(&limits, [4, 4, 5]), Err(INVALID));
        let huge = MeshWorkgroupLimits {
            max_groups: [u32::MAX; 3],
            max_total_groups: u32::MAX,
            ..limits
        };
        assert_eq!(groups(&huge, [u32::MAX; 3]), Err(INVALID));
        assert_eq!(validate_feature_dependencies(MESH_SHADER), Err(INVALID));
        assert_eq!(
            validate_feature_dependencies(TASK_SHADER | RASTER),
            Err(INVALID)
        );
        assert_eq!(validate_feature_dependencies(MESH_SHADER | RASTER), Ok(()));
        assert_eq!(
            validate_feature_dependencies(MESH_SHADER | TASK_SHADER | RASTER),
            Ok(())
        );
    }
    #[test]
    fn artifact_requirements_check_separate_and_combined_limits() {
        let work = MeshWorkgroupLimits {
            max_local: [32; 3],
            max_invocations: 32,
            max_shared_memory: 1024,
            ..Default::default()
        };
        let p = MeshLimits {
            task: work,
            mesh: work,
            max_task_payload: 512,
            max_task_payload_shared: 1024,
            max_mesh_payload_shared: 1024,
            max_mesh_output_memory: 2048,
            max_mesh_payload_output: 2048,
            max_mesh_output_components: 128,
            max_mesh_output_vertices: 256,
            max_mesh_output_primitives: 256,
            max_mesh_output_layers: 8,
            ..Default::default()
        };
        let mut r = MeshRequirements {
            header: Record::new::<MeshRequirements>(MESH_REQUIREMENTS),
            features: 0,
            task_local: [0; 3],
            task_shared: 0,
            mesh_local: [32, 1, 1],
            mesh_shared: 0,
            payload_size: 0,
            output_memory: 64,
            output_vertices: 3,
            output_primitives: 1,
            output_components: 4,
            output_layers: 1,
        };
        let enabled = MESH_SHADER | TASK_SHADER | RASTER;
        assert_eq!(requirements(&p, enabled, MESH_STAGE, &r), Ok(()));
        assert_eq!(requirements(&p, RASTER, MESH_STAGE, &r), Err(UNSUPPORTED));
        r.task_local = [1; 3];
        assert_eq!(requirements(&p, enabled, MESH_STAGE, &r), Err(INVALID));
        assert_eq!(
            requirements(&p, enabled, MESH_STAGE | TASK_STAGE, &r),
            Ok(())
        );
        assert_eq!(
            requirements(&p, MESH_SHADER | RASTER, MESH_STAGE | TASK_STAGE, &r),
            Err(UNSUPPORTED)
        );
        r.payload_size = 512;
        r.task_shared = 513;
        assert_eq!(
            requirements(&p, enabled, MESH_STAGE | TASK_STAGE, &r),
            Err(UNSUPPORTED)
        );
        r.task_shared = 512;
        r.mesh_shared = 513;
        assert_eq!(
            requirements(&p, enabled, MESH_STAGE | TASK_STAGE, &r),
            Err(UNSUPPORTED)
        );
        r.mesh_shared = 512;
        r.output_memory = 1537;
        assert_eq!(
            requirements(&p, enabled, MESH_STAGE | TASK_STAGE, &r),
            Err(UNSUPPORTED)
        );
        r.output_memory = 1536;
        assert_eq!(
            requirements(&p, enabled, MESH_STAGE | TASK_STAGE, &r),
            Ok(())
        );
        r.mesh_local = [32, 2, 1];
        assert_eq!(
            requirements(&p, enabled, MESH_STAGE | TASK_STAGE, &r),
            Err(UNSUPPORTED)
        );
        r.mesh_local = [0, 1, 1];
        assert_eq!(
            requirements(&p, enabled, MESH_STAGE | TASK_STAGE, &r),
            Err(INVALID)
        );
    }
    #[test]
    #[ignore = "requires Vulkan; mesh discovery and explicit native feature enabling"]
    fn gpu_mesh_discovery() {
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = super::super::snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let available = snapshot.features.available & (MESH_SHADER | TASK_SHADER);
            eprintln!(
                "mesh available={available:#x}, limits={:?}",
                snapshot.mesh_limits
            );
            let Some(domain) = snapshot
                .queues
                .iter()
                .find(|q| q.flags & GRAPHICS != 0 && q.count > 0)
                .map(|q| q.domain)
            else {
                continue;
            };
            let adapter = Adapter {
                state: AdapterState {
                    instance: instance.clone(),
                    physical,
                },
                snapshot,
            };
            let requests = [QueueRequest {
                domain,
                count: 1,
                priority: 0.5,
            }];
            for required_features in [MESH_SHADER, RASTER | TASK_SHADER] {
                let desc = DeviceDesc {
                    header: Record::new::<DeviceDesc>(DEVICE_DESC),
                    queues: requests.as_ptr(),
                    queue_request_count: 1,
                    reserved: 0,
                    required_features,
                };
                let mut output = ptr::dangling_mut();
                assert_eq!(
                    unsafe { ogpu_next_device_create(&adapter, &desc, &mut output) },
                    INVALID
                );
                assert!(output.is_null());
            }
            if available == 0 {
                assert!(matches!(
                    rust::device_create(&adapter, &requests, RASTER | MESH_SHADER),
                    Err(UNSUPPORTED)
                ));
            } else {
                assert!(adapter.snapshot.mesh_limits.mesh.max_invocations > 0);
                for enabled in [RASTER, RASTER | MESH_SHADER, RASTER | available] {
                    let handle = rust::device_create(&adapter, &requests, enabled).unwrap();
                    let d = unsafe { &*handle.as_raw() };
                    assert_eq!(d.mesh_functions.is_some(), enabled & MESH_SHADER != 0);
                    assert_eq!(d.snapshot.features.enabled, enabled);
                    assert_eq!(d.snapshot.mesh_limits, adapter.snapshot.mesh_limits);
                    unsafe { rust::device_destroy(handle) };
                }
            }
            tested += 1;
        }
        assert!(tested > 0);
    }
}
