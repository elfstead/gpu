//! Optional native raster/fetch controls. All translation is cold preparation.
use super::*;
use super::{commands::slice as array, executables::record};

pub(super) struct Native {
    clip: vk::VkPhysicalDeviceDepthClipEnableFeaturesEXT,
    restart: vk::VkPhysicalDevicePrimitiveTopologyListRestartFeaturesEXT,
}
impl Native {
    pub fn new(enabled: FeatureBits) -> Self {
        Self {
            clip: vk::VkPhysicalDeviceDepthClipEnableFeaturesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEPTH_CLIP_ENABLE_FEATURES_EXT,
                depthClipEnable: u32::from(enabled & DEPTH_CLIP != 0), ..Default::default()
            },
            restart: vk::VkPhysicalDevicePrimitiveTopologyListRestartFeaturesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PRIMITIVE_TOPOLOGY_LIST_RESTART_FEATURES_EXT,
                primitiveTopologyListRestart: u32::from(enabled & LIST_RESTART != 0),
                primitiveTopologyPatchListRestart: u32::from(enabled & PATCH_RESTART != 0), ..Default::default()
            },
        }
    }
    /// Do not move self while the returned chain is in use.
    pub fn link(
        &mut self,
        mut next: *mut std::ffi::c_void,
        selected: FeatureBits,
    ) -> *mut std::ffi::c_void {
        if selected & DEPTH_CLIP != 0 {
            self.clip.pNext = next;
            next = ptr::from_mut(&mut self.clip).cast();
        }
        if selected & (LIST_RESTART | PATCH_RESTART) != 0 {
            self.restart.pNext = next;
            next = ptr::from_mut(&mut self.restart).cast();
        }
        next
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<FeatureBits, Status> {
    let mut selected = FeatureBits::ZERO;
    for (name, bits) in [
        (c"VK_EXT_depth_clip_enable", DEPTH_CLIP),
        (
            c"VK_EXT_primitive_topology_list_restart",
            LIST_RESTART | PATCH_RESTART,
        ),
    ] {
        if instance
            .supports_extension(physical, name)
            .map_err(loader_error)?
        {
            selected |= bits;
        }
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
    let mut available = FeatureBits::ZERO;
    for (value, bit) in [
        (native.clip.depthClipEnable, DEPTH_CLIP),
        (native.restart.primitiveTopologyListRestart, LIST_RESTART),
        (
            native.restart.primitiveTopologyPatchListRestart,
            PATCH_RESTART,
        ),
    ] {
        if value != 0 {
            available |= bit;
        }
    }
    Ok(available)
}
pub(super) fn restart(topology: u32, enabled: FeatureBits) -> Result<(), Status> {
    match topology {
        2 | 4 | 5 | 7 | 9 => Ok(()),
        0 | 1 | 3 | 6 | 8 if enabled & LIST_RESTART != 0 => Ok(()),
        PATCHES if enabled & PATCH_RESTART != 0 => Ok(()),
        0 | 1 | 3 | 6 | 8 | PATCHES => Err(UNSUPPORTED),
        _ => Err(INVALID),
    }
}
pub(super) unsafe fn depth_clip(
    header: *const Record,
    enabled: FeatureBits,
) -> Result<Option<vk::VkPipelineRasterizationDepthClipStateCreateInfoEXT>, Status> {
    if header.is_null() {
        return Ok(None);
    }
    let state = unsafe { record::<DepthClipState>(header, DEPTH_CLIP_STATE)? };
    if state.enable > 1 || state.reserved != 0 {
        return Err(INVALID);
    }
    if enabled & DEPTH_CLIP == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(Some(vk::VkPipelineRasterizationDepthClipStateCreateInfoEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_DEPTH_CLIP_STATE_CREATE_INFO_EXT,
        depthClipEnable: state.enable, ..Default::default()
    }))
}
pub(super) unsafe fn divisor_entries<'a>(
    header: *const Record,
    bindings: &[VertexBinding],
    enabled: FeatureBits,
    limits: &RasterControlLimits,
) -> Result<&'a [VertexDivisor], Status> {
    if header.is_null() {
        return Ok(&[]);
    }
    let state = unsafe { record::<VertexDivisors>(header, VERTEX_DIVISORS)? };
    if state.reserved != 0 || state.count as usize > bindings.len() {
        return Err(INVALID);
    }
    let entries = unsafe { array(state.entries, state.count)? };
    for (i, entry) in entries.iter().enumerate() {
        if !bindings
            .iter()
            .any(|b| b.binding == entry.binding && b.rate == 1)
            || entries[..i].iter().any(|e| e.binding == entry.binding)
        {
            return Err(INVALID);
        }
        if entry.divisor > limits.max_vertex_divisor
            || entry.divisor == 0 && enabled & VERTEX_ZERO_DIVISOR == 0
            || entry.divisor > 1 && enabled & VERTEX_DIVISOR == 0
        {
            return Err(UNSUPPORTED);
        }
    }
    Ok(entries)
}
pub(super) unsafe fn divisors(
    header: *const Record,
    bindings: &[VertexBinding],
    enabled: FeatureBits,
    limits: &RasterControlLimits,
) -> Result<Vec<vk::VkVertexInputBindingDivisorDescription>, Status> {
    let entries = unsafe { divisor_entries(header, bindings, enabled, limits)? };
    let mut result = Vec::new();
    result
        .try_reserve_exact(entries.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    for entry in entries {
        result.push(vk::VkVertexInputBindingDivisorDescription {
            binding: entry.binding,
            divisor: entry.divisor,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_features_preserve_independence() {
        for t in [2, 4, 5, 7, 9] {
            assert_eq!(restart(t, FeatureBits::ZERO), Ok(()));
        }
        for t in [0, 1, 3, 6, 8] {
            assert_eq!(restart(t, PATCH_RESTART), Err(UNSUPPORTED));
            assert_eq!(restart(t, LIST_RESTART), Ok(()));
        }
        assert_eq!(restart(PATCHES, LIST_RESTART), Err(UNSUPPORTED));
        assert_eq!(restart(PATCHES, PATCH_RESTART), Ok(()));
        assert_eq!(restart(11, FeatureBits::MAX), Err(INVALID));
    }
    #[test]
    fn clip_is_explicit_and_rejects_malformed_extensions() {
        let mut s = DepthClipState {
            header: Record::new::<DepthClipState>(DEPTH_CLIP_STATE),
            enable: 0,
            reserved: 0,
        };
        assert!(unsafe { depth_clip(ptr::null(), FeatureBits::ZERO) }
            .unwrap()
            .is_none());
        for enable in [0, 1] {
            s.enable = enable;
            assert!(matches!(
                unsafe { depth_clip(&s.header, DEPTH_CLAMP) },
                Err(UNSUPPORTED)
            ));
            assert_eq!(
                unsafe { depth_clip(&s.header, DEPTH_CLIP) }
                    .unwrap()
                    .unwrap()
                    .depthClipEnable,
                enable
            );
        }
        s.enable = 2;
        assert!(matches!(
            unsafe { depth_clip(&s.header, DEPTH_CLIP) },
            Err(INVALID)
        ));
        s.enable = 0;
        s.header.next = &s.header;
        assert!(unsafe { depth_clip(&s.header, DEPTH_CLIP) }.is_err());
    }
    #[test]
    fn divisor_limits_rates_duplicates_and_enables() {
        let bindings = [
            VertexBinding {
                binding: 3,
                stride: 16,
                rate: 1,
            },
            VertexBinding {
                binding: 5,
                stride: 8,
                rate: 0,
            },
        ];
        let limits = RasterControlLimits {
            max_vertex_divisor: 8,
            ..Default::default()
        };
        let mut e = [
            VertexDivisor {
                binding: 3,
                divisor: 2,
            },
            VertexDivisor {
                binding: 3,
                divisor: 2,
            },
        ];
        let mut s = VertexDivisors {
            header: Record::new::<VertexDivisors>(VERTEX_DIVISORS),
            count: 1,
            reserved: 0,
            entries: e.as_ptr(),
        };
        assert!(matches!(
            unsafe { divisors(&s.header, &bindings, VERTEX_ZERO_DIVISOR, &limits) },
            Err(UNSUPPORTED)
        ));
        assert_eq!(
            unsafe { divisors(&s.header, &bindings, VERTEX_DIVISOR, &limits) }.unwrap()[0].divisor,
            2
        );
        e[0].divisor = 0;
        assert!(matches!(
            unsafe { divisors(&s.header, &bindings, VERTEX_DIVISOR, &limits) },
            Err(UNSUPPORTED)
        ));
        assert_eq!(
            unsafe { divisors(&s.header, &bindings, VERTEX_ZERO_DIVISOR, &limits) }.unwrap()[0]
                .divisor,
            0
        );
        e[0].divisor = 9;
        assert!(matches!(
            unsafe { divisors(&s.header, &bindings, FeatureBits::MAX, &limits) },
            Err(UNSUPPORTED)
        ));
        e[0].divisor = 1;
        e[0].binding = 5;
        assert!(matches!(
            unsafe { divisors(&s.header, &bindings, FeatureBits::MAX, &limits) },
            Err(INVALID)
        ));
        e[0].binding = 3;
        s.count = 2;
        assert!(matches!(
            unsafe { divisors(&s.header, &bindings, FeatureBits::MAX, &limits) },
            Err(INVALID)
        ));
        assert_eq!(e[0].binding, 3);
    }
}
