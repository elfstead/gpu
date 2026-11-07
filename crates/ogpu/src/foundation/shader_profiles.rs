//! Explicit shader capabilities. Artifact bytes own execution modes and layouts;
//! this module only queries and enables exact native features on the cold path.
use super::*;
const VARIABLE: FeatureBits = FeatureBits::union(&[VARIABLE_POINTERS_STORAGE, VARIABLE_POINTERS]);
const UNIFORM: FeatureBits = FeatureBits::union(&[SUBGROUP_UNIFORM_CONTROL_FLOW]);
const RECONVERGENCE: FeatureBits = FeatureBits::union(&[MAXIMAL_RECONVERGENCE]);
const QUAD: FeatureBits = FeatureBits::union(&[QUAD_CONTROL]);
const WORKGROUP: FeatureBits = FeatureBits::union(&[
    WORKGROUP_EXPLICIT_LAYOUT,
    WORKGROUP_SCALAR_LAYOUT,
    WORKGROUP_STORAGE8,
    WORKGROUP_STORAGE16,
]);
const DERIVATIVES: FeatureBits =
    FeatureBits::union(&[COMPUTE_DERIVATIVE_QUADS, COMPUTE_DERIVATIVE_LINEAR]);
pub(super) struct Native {
    variable: vk::VkPhysicalDeviceVariablePointersFeatures,
    uniform: vk::VkPhysicalDeviceShaderSubgroupUniformControlFlowFeaturesKHR,
    reconvergence: vk::VkPhysicalDeviceShaderMaximalReconvergenceFeaturesKHR,
    quad: vk::VkPhysicalDeviceShaderQuadControlFeaturesKHR,
    workgroup: vk::VkPhysicalDeviceWorkgroupMemoryExplicitLayoutFeaturesKHR,
    derivatives: vk::VkPhysicalDeviceComputeShaderDerivativesFeaturesKHR,
}
impl Native {
    pub fn new(enabled: FeatureBits) -> Self {
        Self {
            variable: vk::VkPhysicalDeviceVariablePointersFeatures { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VARIABLE_POINTERS_FEATURES,
                variablePointersStorageBuffer: u32::from(enabled & VARIABLE_POINTERS_STORAGE != 0),
                variablePointers: u32::from(enabled & VARIABLE_POINTERS != 0),
                ..Default::default() },
            uniform: vk::VkPhysicalDeviceShaderSubgroupUniformControlFlowFeaturesKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_SUBGROUP_UNIFORM_CONTROL_FLOW_FEATURES_KHR,
                shaderSubgroupUniformControlFlow: u32::from(enabled & SUBGROUP_UNIFORM_CONTROL_FLOW != 0),
                ..Default::default() },
            reconvergence: vk::VkPhysicalDeviceShaderMaximalReconvergenceFeaturesKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_MAXIMAL_RECONVERGENCE_FEATURES_KHR,
                shaderMaximalReconvergence: u32::from(enabled & MAXIMAL_RECONVERGENCE != 0),
                ..Default::default() },
            quad: vk::VkPhysicalDeviceShaderQuadControlFeaturesKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_QUAD_CONTROL_FEATURES_KHR,
                shaderQuadControl: u32::from(enabled & QUAD_CONTROL != 0),
                ..Default::default() },
            workgroup: vk::VkPhysicalDeviceWorkgroupMemoryExplicitLayoutFeaturesKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_WORKGROUP_MEMORY_EXPLICIT_LAYOUT_FEATURES_KHR,
                workgroupMemoryExplicitLayout: u32::from(enabled & WORKGROUP_EXPLICIT_LAYOUT != 0),
                workgroupMemoryExplicitLayoutScalarBlockLayout: u32::from(enabled & WORKGROUP_SCALAR_LAYOUT != 0),
                workgroupMemoryExplicitLayout8BitAccess: u32::from(enabled & WORKGROUP_STORAGE8 != 0),
                workgroupMemoryExplicitLayout16BitAccess: u32::from(enabled & WORKGROUP_STORAGE16 != 0),
                ..Default::default() },
            derivatives: vk::VkPhysicalDeviceComputeShaderDerivativesFeaturesKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COMPUTE_SHADER_DERIVATIVES_FEATURES_KHR,
                computeDerivativeGroupQuads: u32::from(enabled & COMPUTE_DERIVATIVE_QUADS != 0),
                computeDerivativeGroupLinear: u32::from(enabled & COMPUTE_DERIVATIVE_LINEAR != 0),
                ..Default::default() },
        }
    }
    pub fn link(
        &mut self,
        mut next: *mut std::ffi::c_void,
        enabled: FeatureBits,
    ) -> *mut std::ffi::c_void {
        if enabled & VARIABLE != 0 {
            self.variable.pNext = next;
            next = ptr::from_mut(&mut self.variable).cast();
        }
        if enabled & UNIFORM != 0 {
            self.uniform.pNext = next;
            next = ptr::from_mut(&mut self.uniform).cast();
        }
        if enabled & RECONVERGENCE != 0 {
            self.reconvergence.pNext = next;
            next = ptr::from_mut(&mut self.reconvergence).cast();
        }
        if enabled & QUAD != 0 {
            self.quad.pNext = next;
            next = ptr::from_mut(&mut self.quad).cast();
        }
        if enabled & WORKGROUP != 0 {
            self.workgroup.pNext = next;
            next = ptr::from_mut(&mut self.workgroup).cast();
        }
        if enabled & DERIVATIVES != 0 {
            self.derivatives.pNext = next;
            next = ptr::from_mut(&mut self.derivatives).cast();
        }
        next
    }
    fn available(&self) -> FeatureBits {
        let mut result = FeatureBits::ZERO;
        if self.variable.variablePointersStorageBuffer != 0 {
            result |= VARIABLE_POINTERS_STORAGE;
        }
        if self.variable.variablePointers != 0 {
            result |= VARIABLE_POINTERS;
        }
        if self.uniform.shaderSubgroupUniformControlFlow != 0 {
            result |= SUBGROUP_UNIFORM_CONTROL_FLOW;
        }
        if self.reconvergence.shaderMaximalReconvergence != 0 {
            result |= MAXIMAL_RECONVERGENCE;
        }
        if self.quad.shaderQuadControl != 0 {
            result |= QUAD_CONTROL;
        }
        if self.workgroup.workgroupMemoryExplicitLayout != 0 {
            result |= WORKGROUP_EXPLICIT_LAYOUT;
        }
        if self
            .workgroup
            .workgroupMemoryExplicitLayoutScalarBlockLayout
            != 0
        {
            result |= WORKGROUP_SCALAR_LAYOUT;
        }
        if self.workgroup.workgroupMemoryExplicitLayout8BitAccess != 0 {
            result |= WORKGROUP_STORAGE8;
        }
        if self.workgroup.workgroupMemoryExplicitLayout16BitAccess != 0 {
            result |= WORKGROUP_STORAGE16;
        }
        if self.derivatives.computeDerivativeGroupQuads != 0 {
            result |= COMPUTE_DERIVATIVE_QUADS;
        }
        if self.derivatives.computeDerivativeGroupLinear != 0 {
            result |= COMPUTE_DERIVATIVE_LINEAR;
        }
        result
    }
}
pub(super) fn extensions(enabled: FeatureBits, names: &mut Vec<*const std::ffi::c_char>) {
    if enabled & (UNIFORM) != 0 {
        names.push(c"VK_KHR_shader_subgroup_uniform_control_flow".as_ptr());
    }
    if enabled & (RECONVERGENCE | QUAD) != 0 {
        names.push(c"VK_KHR_shader_maximal_reconvergence".as_ptr());
    }
    if enabled & (QUAD) != 0 {
        names.push(c"VK_KHR_shader_quad_control".as_ptr());
    }
    if enabled & (WORKGROUP) != 0 {
        names.push(c"VK_KHR_workgroup_memory_explicit_layout".as_ptr());
    }
    if enabled & (DERIVATIVES) != 0 {
        names.push(c"VK_KHR_compute_shader_derivatives".as_ptr());
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(FeatureBits, ShaderProfileLimits), Status> {
    let mut present = VARIABLE;
    if instance
        .supports_extension(physical, c"VK_KHR_shader_subgroup_uniform_control_flow")
        .map_err(loader_error)?
    {
        present |= UNIFORM;
    }
    if instance
        .supports_extension(physical, c"VK_KHR_shader_maximal_reconvergence")
        .map_err(loader_error)?
    {
        present |= RECONVERGENCE;
    }
    if instance
        .supports_extension(physical, c"VK_KHR_shader_quad_control")
        .map_err(loader_error)?
    {
        present |= QUAD;
    }
    if instance
        .supports_extension(physical, c"VK_KHR_workgroup_memory_explicit_layout")
        .map_err(loader_error)?
    {
        present |= WORKGROUP;
    }
    if instance
        .supports_extension(physical, c"VK_KHR_compute_shader_derivatives")
        .map_err(loader_error)?
    {
        present |= DERIVATIVES;
    }
    let mut native = Native::new(FeatureBits::ZERO);
    let mut storage = vk::VkPhysicalDevice16BitStorageFeatures {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_16BIT_STORAGE_FEATURES,
        pNext: native.link(ptr::null_mut(), present),
        ..Default::default()
    };
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: ptr::from_mut(&mut storage).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
    }
    let mut available = native.available();
    if storage.uniformAndStorageBuffer16BitAccess != 0 {
        available |= UNIFORM_STORAGE16;
    }
    if storage.storagePushConstant16 != 0 {
        available |= PUSH_STORAGE16;
    }
    if storage.storageInputOutput16 != 0 {
        available |= INPUT_OUTPUT16;
    }
    let mut limits = ShaderProfileLimits::default();
    if available & DERIVATIVES != 0 {
        let mut p = vk::VkPhysicalDeviceComputeShaderDerivativesPropertiesKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COMPUTE_SHADER_DERIVATIVES_PROPERTIES_KHR, ..Default::default()
        };
        let mut root = vk::VkPhysicalDeviceProperties2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
            pNext: ptr::from_mut(&mut p).cast(),
            ..Default::default()
        };
        unsafe {
            (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut root);
        }
        limits.derivative_stages = STAGE_COMPUTE
            | if p.meshAndTaskShaderDerivatives != 0 {
                MESH_STAGE | TASK_STAGE
            } else {
                0
            };
    }
    Ok((available, limits))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shader_features_are_individual_native_permissions() {
        let all = VARIABLE | UNIFORM | RECONVERGENCE | QUAD | WORKGROUP | DERIVATIVES;
        for id in features::decode(all) {
            let requested = FeatureBits::bit(id - 1);
            let mut n = Native::new(requested);
            assert_eq!(n.available(), requested);
            assert!(!n.link(ptr::null_mut(), requested).is_null());
            assert!(n.link(ptr::null_mut(), FeatureBits::ZERO).is_null());
        }
        assert_eq!(
            validate_feature_dependencies(VARIABLE_POINTERS),
            Err(INVALID)
        );
        assert_eq!(validate_feature_dependencies(VARIABLE), Ok(()));
        // Extension dependency is not an implicit shader feature enable.
        let mut names = Vec::new();
        extensions(QUAD_CONTROL, &mut names);
        let names: Vec<_> = names
            .iter()
            .map(|n| unsafe { std::ffi::CStr::from_ptr(*n) })
            .collect();
        assert_eq!(
            names,
            [
                c"VK_KHR_shader_maximal_reconvergence",
                c"VK_KHR_shader_quad_control"
            ]
        );
        assert_eq!(Native::new(QUAD_CONTROL).available(), QUAD_CONTROL);
    }
    #[test]
    #[ignore = "requires Vulkan; verifies each newly exposed native device feature independently"]
    fn gpu_shader_profile_enablement() {
        let instance = Arc::new(Instance::new().unwrap());
        let physical = instance
            .physical_devices()
            .unwrap()
            .into_iter()
            .find(|p| snapshot_baseline(&instance, *p))
            .expect("no baseline adapter");
        let snap = super::super::snapshot(&instance, physical).unwrap();
        let domain = snap
            .queues
            .iter()
            .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
            .unwrap()
            .domain;
        let adapter = Adapter {
            instance,
            physical,
            snapshot: snap,
        };
        let queues = [QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }];
        let mut accepted = Vec::new();
        let mut excluded = Vec::new();
        for id in feature_ids::UNIFORM_STORAGE8..=feature_ids::COMPUTE_DERIVATIVE_LINEAR {
            let mut bits = FeatureBits::bit(id - 1);
            if bits == VARIABLE_POINTERS {
                bits |= VARIABLE_POINTERS_STORAGE;
            }
            let result = Device::create(&adapter, &queues, bits);
            if bits & !adapter.snapshot.features.available != 0 {
                assert!(matches!(result, Err(UNSUPPORTED)), "feature {id}");
                excluded.push(id);
            } else {
                let d = result.unwrap_or_else(|status| panic!("feature {id}: {status}"));
                assert_eq!(d.snapshot.features.enabled, bits);
                accepted.push(id);
            }
        }
        println!(
            "SHADER_PROFILE_ENABLED={accepted:?}; UNSUPPORTED={excluded:?}; derivative_stages={}",
            adapter.snapshot.shader_profile_limits.derivative_stages
        );
        assert!(!accepted.is_empty());
    }
    fn snapshot_baseline(instance: &Instance, physical: vk::VkPhysicalDevice) -> bool {
        super::super::snapshot(instance, physical).is_ok_and(|s| s.features.baseline_supported != 0)
    }
}
