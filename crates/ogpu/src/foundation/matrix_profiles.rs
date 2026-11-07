//! Exact shader permissions; no implicit precision, shape, or workspace policy.
use super::*;
const BASE: FeatureBits = FeatureBits::union(&[COOPERATIVE_MATRIX, MATRIX_ROBUST]);
const EXTENDED: FeatureBits = FeatureBits::union(&[
    MATRIX_WORKGROUP,
    MATRIX_FLEXIBLE,
    MATRIX_REDUCTIONS,
    MATRIX_CONVERSIONS,
    MATRIX_PER_ELEMENT,
    MATRIX_TENSOR_ADDRESSING,
    MATRIX_BLOCK_LOADS,
]);
const BFLOAT: FeatureBits = FeatureBits::union(&[BFLOAT16_TYPE, BFLOAT16_DOT, BFLOAT16_MATRIX]);
const FLOAT8: FeatureBits = FeatureBits::union(&[FLOAT8_TYPE, FLOAT8_MATRIX]);
pub(super) struct Native {
    base: vk::VkPhysicalDeviceCooperativeMatrixFeaturesKHR,
    extended: vk::VkPhysicalDeviceCooperativeMatrix2FeaturesNV,
    bfloat: vk::VkPhysicalDeviceShaderBfloat16FeaturesKHR,
    float8: vk::VkPhysicalDeviceShaderFloat8FeaturesEXT,
}
impl Native {
    pub fn new(enabled: FeatureBits) -> Self {
        Self {
            base: vk::VkPhysicalDeviceCooperativeMatrixFeaturesKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COOPERATIVE_MATRIX_FEATURES_KHR,
                cooperativeMatrix: u32::from(enabled & COOPERATIVE_MATRIX != 0),
                cooperativeMatrixRobustBufferAccess: u32::from(enabled & MATRIX_ROBUST != 0),
                ..Default::default() },
            extended: vk::VkPhysicalDeviceCooperativeMatrix2FeaturesNV { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COOPERATIVE_MATRIX_2_FEATURES_NV,
                cooperativeMatrixWorkgroupScope: u32::from(enabled & MATRIX_WORKGROUP != 0),
                cooperativeMatrixFlexibleDimensions: u32::from(enabled & MATRIX_FLEXIBLE != 0),
                cooperativeMatrixReductions: u32::from(enabled & MATRIX_REDUCTIONS != 0),
                cooperativeMatrixConversions: u32::from(enabled & MATRIX_CONVERSIONS != 0),
                cooperativeMatrixPerElementOperations: u32::from(enabled & MATRIX_PER_ELEMENT != 0),
                cooperativeMatrixTensorAddressing: u32::from(enabled & MATRIX_TENSOR_ADDRESSING != 0),
                cooperativeMatrixBlockLoads: u32::from(enabled & MATRIX_BLOCK_LOADS != 0),
                ..Default::default() },
            bfloat: vk::VkPhysicalDeviceShaderBfloat16FeaturesKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_BFLOAT16_FEATURES_KHR,
                shaderBFloat16Type: u32::from(enabled & BFLOAT16_TYPE != 0),
                shaderBFloat16DotProduct: u32::from(enabled & BFLOAT16_DOT != 0),
                shaderBFloat16CooperativeMatrix: u32::from(enabled & BFLOAT16_MATRIX != 0),
                ..Default::default() },
            float8: vk::VkPhysicalDeviceShaderFloat8FeaturesEXT { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SHADER_FLOAT8_FEATURES_EXT,
                shaderFloat8: u32::from(enabled & FLOAT8_TYPE != 0),
                shaderFloat8CooperativeMatrix: u32::from(enabled & FLOAT8_MATRIX != 0),
                ..Default::default() },
        }
    }
    pub fn link(
        &mut self,
        mut next: *mut std::ffi::c_void,
        enabled: FeatureBits,
    ) -> *mut std::ffi::c_void {
        if enabled & BASE != 0 {
            self.base.pNext = next;
            next = ptr::from_mut(&mut self.base).cast();
        }
        if enabled & EXTENDED != 0 {
            self.extended.pNext = next;
            next = ptr::from_mut(&mut self.extended).cast();
        }
        if enabled & BFLOAT != 0 {
            self.bfloat.pNext = next;
            next = ptr::from_mut(&mut self.bfloat).cast();
        }
        if enabled & FLOAT8 != 0 {
            self.float8.pNext = next;
            next = ptr::from_mut(&mut self.float8).cast();
        }
        next
    }
    fn available(&self) -> FeatureBits {
        let mut bits = FeatureBits::ZERO;
        if self.base.cooperativeMatrix != 0 {
            bits |= COOPERATIVE_MATRIX;
        }
        if self.base.cooperativeMatrixRobustBufferAccess != 0 {
            bits |= MATRIX_ROBUST;
        }
        if self.extended.cooperativeMatrixWorkgroupScope != 0 {
            bits |= MATRIX_WORKGROUP;
        }
        if self.extended.cooperativeMatrixFlexibleDimensions != 0 {
            bits |= MATRIX_FLEXIBLE;
        }
        if self.extended.cooperativeMatrixReductions != 0 {
            bits |= MATRIX_REDUCTIONS;
        }
        if self.extended.cooperativeMatrixConversions != 0 {
            bits |= MATRIX_CONVERSIONS;
        }
        if self.extended.cooperativeMatrixPerElementOperations != 0 {
            bits |= MATRIX_PER_ELEMENT;
        }
        if self.extended.cooperativeMatrixTensorAddressing != 0 {
            bits |= MATRIX_TENSOR_ADDRESSING;
        }
        if self.extended.cooperativeMatrixBlockLoads != 0 {
            bits |= MATRIX_BLOCK_LOADS;
        }
        if self.bfloat.shaderBFloat16Type != 0 {
            bits |= BFLOAT16_TYPE;
        }
        if self.bfloat.shaderBFloat16DotProduct != 0 {
            bits |= BFLOAT16_DOT;
        }
        if self.bfloat.shaderBFloat16CooperativeMatrix != 0 {
            bits |= BFLOAT16_MATRIX;
        }
        if self.float8.shaderFloat8 != 0 {
            bits |= FLOAT8_TYPE;
        }
        if self.float8.shaderFloat8CooperativeMatrix != 0 {
            bits |= FLOAT8_MATRIX;
        }
        bits
    }
}
pub(super) fn extensions(enabled: FeatureBits, names: &mut Vec<*const std::ffi::c_char>) {
    if enabled & (BASE | EXTENDED) != 0 {
        names.push(c"VK_KHR_cooperative_matrix".as_ptr());
    }
    if enabled & (EXTENDED) != 0 {
        names.push(c"VK_NV_cooperative_matrix2".as_ptr());
    }
    if enabled & (BFLOAT) != 0 {
        names.push(c"VK_KHR_shader_bfloat16".as_ptr());
    }
    if enabled & (FLOAT8) != 0 {
        names.push(c"VK_EXT_shader_float8".as_ptr());
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(FeatureBits, MatrixProfileLimits, Vec<MatrixFlexibleTuple>), Status> {
    let mut present = FeatureBits::ZERO;
    if instance
        .supports_extension(physical, c"VK_KHR_cooperative_matrix")
        .map_err(loader_error)?
    {
        present |= BASE;
    }
    if instance
        .supports_extension(physical, c"VK_NV_cooperative_matrix2")
        .map_err(loader_error)?
    {
        present |= EXTENDED;
    }
    if instance
        .supports_extension(physical, c"VK_KHR_shader_bfloat16")
        .map_err(loader_error)?
    {
        present |= BFLOAT;
    }
    if instance
        .supports_extension(physical, c"VK_EXT_shader_float8")
        .map_err(loader_error)?
    {
        present |= FLOAT8;
    }
    let mut native = Native::new(FeatureBits::ZERO);
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: native.link(ptr::null_mut(), present),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
    }
    let available = native.available();
    let mut limits = MatrixProfileLimits::default();
    let mut tuples = Vec::new();
    if present & EXTENDED != 0 {
        let mut p = vk::VkPhysicalDeviceCooperativeMatrix2PropertiesNV { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COOPERATIVE_MATRIX_2_PROPERTIES_NV, ..Default::default() };
        let mut root = vk::VkPhysicalDeviceProperties2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
            pNext: ptr::from_mut(&mut p).cast(),
            ..Default::default()
        };
        unsafe {
            (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut root);
        }
        limits = MatrixProfileLimits {
            workgroup_max_invocations: p.cooperativeMatrixWorkgroupScopeMaxWorkgroupSize,
            max_dimension: p.cooperativeMatrixFlexibleDimensionsMaxDimension,
            workgroup_reserved_shared_bytes: p.cooperativeMatrixWorkgroupScopeReservedSharedMemory,
            reserved: 0,
        };
        if available & MATRIX_FLEXIBLE != 0 {
            let get: vk::PFN_vkGetPhysicalDeviceCooperativeMatrixFlexibleDimensionsPropertiesNV =
                unsafe {
                    std::mem::transmute(instance.proc(
                        c"vkGetPhysicalDeviceCooperativeMatrixFlexibleDimensionsPropertiesNV",
                    ))
                };
            let get = get.ok_or(BACKEND_ERROR)?;
            let stages = matrices::supported_stages(physical, f);
            let native = matrices::enumerate(vk::VkCooperativeMatrixFlexibleDimensionsPropertiesNV { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COOPERATIVE_MATRIX_FLEXIBLE_DIMENSIONS_PROPERTIES_NV, ..Default::default() }, |count, out| unsafe { get(physical, count, out) })?;
            tuples = native.iter().filter_map(|p| flexible(p, stages)).collect();
        }
    }
    Ok((available, limits, tuples))
}
fn flexible(
    p: &vk::VkCooperativeMatrixFlexibleDimensionsPropertiesNV,
    stages: u32,
) -> Option<MatrixFlexibleTuple> {
    let t = matrices::tuple(
        &vk::VkCooperativeMatrixPropertiesKHR {
            MSize: p.MGranularity,
            NSize: p.NGranularity,
            KSize: p.KGranularity,
            AType: p.AType,
            BType: p.BType,
            CType: p.CType,
            ResultType: p.ResultType,
            saturatingAccumulation: p.saturatingAccumulation,
            scope: p.scope,
            ..Default::default()
        },
        stages,
    )?;
    Some(MatrixFlexibleTuple {
        m_granularity: t.m,
        n_granularity: t.n,
        k_granularity: t.k,
        a: t.a,
        b: t.b,
        c: t.c,
        result: t.result,
        saturating: t.saturating,
        scope: t.scope,
        workgroup_invocations: p.workgroupInvocations,
        stages: t.stages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matrix_permissions_do_not_invent_arithmetic_or_implicit_robustness() {
        let all = BASE | EXTENDED | BFLOAT | FLOAT8;
        for id in features::decode(all) {
            let bit = FeatureBits::bit(id - 1);
            let mut n = Native::new(bit);
            assert_eq!(n.available(), bit);
            assert!(!n.link(ptr::null_mut(), bit).is_null());
            assert!(n.link(ptr::null_mut(), FeatureBits::ZERO).is_null());
        }
        assert_eq!(
            Native::new(COOPERATIVE_MATRIX)
                .base
                .cooperativeMatrixRobustBufferAccess,
            0
        );
        let mut names = Vec::new();
        extensions(MATRIX_TENSOR_ADDRESSING, &mut names);
        let names: Vec<_> = names
            .iter()
            .map(|n| unsafe { std::ffi::CStr::from_ptr(*n) })
            .collect();
        assert_eq!(
            names,
            [c"VK_KHR_cooperative_matrix", c"VK_NV_cooperative_matrix2"]
        );
    }
    #[test]
    fn flexible_tuples_preserve_joint_constraints_and_low_precision_types() {
        let p = vk::VkCooperativeMatrixFlexibleDimensionsPropertiesNV {
            MGranularity: 16,
            NGranularity: 32,
            KGranularity: 8,
            AType: vk::VkComponentTypeKHR_VK_COMPONENT_TYPE_BFLOAT16_KHR,
            BType: vk::VkComponentTypeKHR_VK_COMPONENT_TYPE_FLOAT8_E4M3_EXT,
            CType: vk::VkComponentTypeKHR_VK_COMPONENT_TYPE_FLOAT8_E5M2_EXT,
            ResultType: vk::VkComponentTypeKHR_VK_COMPONENT_TYPE_FLOAT32_KHR,
            scope: vk::VkScopeKHR_VK_SCOPE_WORKGROUP_KHR,
            saturatingAccumulation: 1,
            workgroupInvocations: 128,
            ..Default::default()
        };
        assert_eq!(
            flexible(&p, 32),
            Some(MatrixFlexibleTuple {
                m_granularity: 16,
                n_granularity: 32,
                k_granularity: 8,
                a: COMPONENT_BFLOAT16,
                b: COMPONENT_FLOAT8_E4M3,
                c: COMPONENT_FLOAT8_E5M2,
                result: COMPONENT_FLOAT32,
                scope: MATRIX_SCOPE_WORKGROUP,
                saturating: 1,
                workgroup_invocations: 128,
                stages: STAGE_COMPUTE,
            })
        );
        assert!(flexible(
            &vk::VkCooperativeMatrixFlexibleDimensionsPropertiesNV {
                AType: u32::MAX,
                ..p
            },
            32
        )
        .is_none());
        let initial = vk::VkCooperativeMatrixFlexibleDimensionsPropertiesNV {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COOPERATIVE_MATRIX_FLEXIBLE_DIMENSIONS_PROPERTIES_NV,
            ..Default::default()
        };
        let tuples = matrices::enumerate(initial, |n, out| {
            *n = 1;
            if !out.is_null() {
                unsafe {
                    assert_eq!((*out).sType, initial.sType);
                    assert!((*out).pNext.is_null());
                    *out = p;
                }
            }
            vk::VkResult_VK_SUCCESS
        })
        .unwrap();
        assert_eq!(flexible(&tuples[0], 32), flexible(&p, 32));
    }
    #[test]
    #[ignore = "requires Vulkan; tests independent native matrix feature enables or unsupported rejection"]
    fn gpu_matrix_profile_enablement() {
        let instance = Arc::new(Instance::new().unwrap());
        let physical = instance
            .physical_devices()
            .unwrap()
            .into_iter()
            .find(|p| {
                super::super::snapshot(&instance, *p)
                    .is_ok_and(|s| s.features.baseline_supported != 0)
            })
            .expect("no baseline adapter");
        let snapshot = super::super::snapshot(&instance, physical).unwrap();
        let domain = snapshot
            .queues
            .iter()
            .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
            .unwrap()
            .domain;
        println!(
            "MATRIX_LIMITS={:?}; FIXED={:?}; FLEXIBLE={:?}",
            snapshot.matrix_profile_limits, snapshot.matrix_tuples, snapshot.matrix_flexible_tuples
        );
        let adapter = Adapter {
            instance,
            physical,
            snapshot,
        };
        let queues = [QueueRequest {
            domain,
            count: 1,
            priority: 1.0,
        }];
        let mut accepted = Vec::new();
        let mut excluded = Vec::new();
        for id in feature_ids::MATRIX_WORKGROUP..=feature_ids::MATRIX_ROBUST {
            let bits = FeatureBits::bit(id - 1);
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
        println!("MATRIX_PROFILE_ENABLED={accepted:?}; UNSUPPORTED={excluded:?}");
    }
}
