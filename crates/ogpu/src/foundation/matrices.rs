//! Immutable native matrix tuples, not a tensor allocator or launch policy.
use super::*;

fn tuple(p: &vk::VkCooperativeMatrixPropertiesKHR, stages: u32) -> Option<MatrixTuple> {
    // This profile enables KHR subgroup scope and core component types only.
    // Workgroup/flexible-dimension and newer low-bit formats need their own enabling.
    if p.scope != vk::VkScopeKHR_VK_SCOPE_SUBGROUP_KHR
        || [p.AType, p.BType, p.CType, p.ResultType]
            .iter()
            .any(|t| !(0..=10).contains(t))
        || stages & vk::VkShaderStageFlagBits_VK_SHADER_STAGE_COMPUTE_BIT == 0
    {
        return None;
    }
    Some(MatrixTuple {
        m: p.MSize,
        n: p.NSize,
        k: p.KSize,
        a: p.AType,
        b: p.BType,
        c: p.CType,
        result: p.ResultType,
        saturating: u32::from(p.saturatingAccumulation != 0),
        scope: 3,
        reserved: 0,
        stages: 4,
    })
}

fn enumerate(
    mut call: impl FnMut(&mut u32, *mut vk::VkCooperativeMatrixPropertiesKHR) -> vk::VkResult,
) -> Result<Vec<vk::VkCooperativeMatrixPropertiesKHR>, Status> {
    for _ in 0..8 {
        let mut count = 0;
        let result = call(&mut count, ptr::null_mut());
        if result == vk::VkResult_VK_INCOMPLETE {
            continue;
        }
        status(result)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(count as usize)
            .map_err(|_| OUT_OF_MEMORY)?;
        values.resize(
            count as usize,
            vk::VkCooperativeMatrixPropertiesKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COOPERATIVE_MATRIX_PROPERTIES_KHR,
                ..Default::default()
            },
        );
        let result = call(&mut count, values.as_mut_ptr());
        if result == vk::VkResult_VK_INCOMPLETE {
            continue;
        }
        status(result)?;
        if count as usize > values.len() {
            return Err(BACKEND_ERROR);
        }
        values.truncate(count as usize);
        return Ok(values);
    }
    Err(BACKEND_ERROR)
}

pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(u64, Vec<MatrixTuple>), Status> {
    if !instance
        .supports_extension(physical, c"VK_KHR_cooperative_matrix")
        .map_err(loader_error)?
    {
        return Ok((0, Vec::new()));
    }
    let mut matrix = vk::VkPhysicalDeviceCooperativeMatrixFeaturesKHR {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COOPERATIVE_MATRIX_FEATURES_KHR,
        ..Default::default()
    };
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: ptr::from_mut(&mut matrix).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
    }
    if matrix.cooperativeMatrix == 0 {
        return Ok((0, Vec::new()));
    }
    let mut matrix_props = vk::VkPhysicalDeviceCooperativeMatrixPropertiesKHR {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COOPERATIVE_MATRIX_PROPERTIES_KHR,
        ..Default::default()
    };
    let mut props = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: ptr::from_mut(&mut matrix_props).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut props);
    }
    // Optional physical-device function is loaded only when the extension exists.
    let get: vk::PFN_vkGetPhysicalDeviceCooperativeMatrixPropertiesKHR = unsafe {
        std::mem::transmute(instance.proc(c"vkGetPhysicalDeviceCooperativeMatrixPropertiesKHR"))
    };
    let get = get.ok_or(BACKEND_ERROR)?;
    let native = enumerate(|count, out| unsafe { get(physical, count, out) })?;
    Ok((
        COOPERATIVE_MATRIX,
        native
            .iter()
            .filter_map(|p| tuple(p, matrix_props.cooperativeMatrixSupportedStages))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matrix_tuples_keep_shape_types_scope_and_saturation_exact() {
        let p = vk::VkCooperativeMatrixPropertiesKHR {
            MSize: 8,
            NSize: 32,
            KSize: 16,
            AType: 0,
            BType: 0,
            CType: 1,
            ResultType: 1,
            scope: 3,
            ..Default::default()
        };
        assert_eq!(
            tuple(&p, 32),
            Some(MatrixTuple {
                m: 8,
                n: 32,
                k: 16,
                a: 0,
                b: 0,
                c: 1,
                result: 1,
                scope: 3,
                stages: 4,
                ..Default::default()
            })
        );
        for t in 0..=10 {
            let p = vk::VkCooperativeMatrixPropertiesKHR {
                AType: t,
                BType: t,
                CType: t,
                ResultType: t,
                saturatingAccumulation: 1,
                ..p
            };
            let got = tuple(&p, 32).unwrap();
            assert_eq!([got.a, got.b, got.c, got.result], [t; 4]);
            assert_eq!(got.saturating, 1);
        }
        assert!(tuple(&p, 16).is_none());
        assert!(tuple(&vk::VkCooperativeMatrixPropertiesKHR { scope: 2, ..p }, 32).is_none());
        assert!(tuple(
            &vk::VkCooperativeMatrixPropertiesKHR {
                AType: 1000491000,
                ..p
            },
            32
        )
        .is_none());
    }
    #[test]
    fn matrix_enumeration_initializes_headers_retries_and_bounds_counts() {
        let mut calls = 0;
        let result = enumerate(|count, out| {
            calls += 1;
            if out.is_null() {
                *count = 2;
                return vk::VkResult_VK_SUCCESS;
            }
            assert_eq!(*count, 2);
            for i in 0..2 {
                let p = unsafe { &*out.add(i) };
                assert_eq!(
                    p.sType,
                    vk::VkStructureType_VK_STRUCTURE_TYPE_COOPERATIVE_MATRIX_PROPERTIES_KHR
                );
                assert!(p.pNext.is_null());
            }
            if calls == 2 {
                return vk::VkResult_VK_INCOMPLETE;
            }
            unsafe {
                (*out).MSize = 16;
            }
            *count = 1;
            vk::VkResult_VK_SUCCESS
        })
        .unwrap();
        assert_eq!(calls, 4);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].MSize, 16);
        assert!(matches!(
            enumerate(|_, _| vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY),
            Err(OUT_OF_MEMORY)
        ));
        assert!(matches!(
            enumerate(|_, _| vk::VkResult_VK_INCOMPLETE),
            Err(BACKEND_ERROR)
        ));
        assert!(matches!(
            enumerate(|n, p| {
                *n = if p.is_null() { 1 } else { 2 };
                vk::VkResult_VK_SUCCESS
            }),
            Err(BACKEND_ERROR)
        ));
        assert!(enumerate(|_, _| vk::VkResult_VK_SUCCESS)
            .unwrap()
            .is_empty());
    }
}
