//! Explicit depth units/ranges and independent native color-write control.
use super::*;
pub(super) const BIAS_FEATURES: FeatureBits = FeatureBits::union(&[
    DEPTH_BIAS_CONTROL,
    DEPTH_BIAS_UNORM,
    DEPTH_BIAS_FLOAT,
    DEPTH_BIAS_EXACT,
]);
pub(super) const STATES: &[(u64, vk::VkDynamicState)] = &[
    (
        DYNAMIC_DEPTH_CLAMP_RANGE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_CLAMP_RANGE_EXT,
    ),
    (
        DYNAMIC_COLOR_WRITE_ENABLE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_COLOR_WRITE_ENABLE_EXT,
    ),
];
pub(super) fn dynamic_mask(enabled: FeatureBits) -> u64 {
    (if enabled & DEPTH_CLAMP_CONTROL != 0 {
        DYNAMIC_DEPTH_CLAMP_RANGE_STATE
    } else {
        0
    }) | (if enabled & COLOR_WRITE_ENABLE != 0 {
        DYNAMIC_COLOR_WRITE_ENABLE_STATE
    } else {
        0
    })
}
pub(super) struct Native {
    clamp: vk::VkPhysicalDeviceDepthClampControlFeaturesEXT,
    bias: vk::VkPhysicalDeviceDepthBiasControlFeaturesEXT,
    color: vk::VkPhysicalDeviceColorWriteEnableFeaturesEXT,
}
impl Native {
    pub fn new(enabled: FeatureBits) -> Self {
        Self {
            clamp:vk::VkPhysicalDeviceDepthClampControlFeaturesEXT {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEPTH_CLAMP_CONTROL_FEATURES_EXT,
                depthClampControl:u32::from(enabled & DEPTH_CLAMP_CONTROL !=0),..Default::default()
            },
            bias:vk::VkPhysicalDeviceDepthBiasControlFeaturesEXT {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEPTH_BIAS_CONTROL_FEATURES_EXT,
                depthBiasControl:u32::from(enabled & DEPTH_BIAS_CONTROL !=0),
                leastRepresentableValueForceUnormRepresentation:u32::from(enabled & DEPTH_BIAS_UNORM !=0),
                floatRepresentation:u32::from(enabled & DEPTH_BIAS_FLOAT !=0),
                depthBiasExact:u32::from(enabled & DEPTH_BIAS_EXACT !=0),..Default::default()
            },
            color:vk::VkPhysicalDeviceColorWriteEnableFeaturesEXT {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_COLOR_WRITE_ENABLE_FEATURES_EXT,
                colorWriteEnable:u32::from(enabled & COLOR_WRITE_ENABLE !=0),..Default::default()
            },
        }
    }
    pub fn link(
        &mut self,
        mut next: *mut std::ffi::c_void,
        selected: FeatureBits,
    ) -> *mut std::ffi::c_void {
        if selected & DEPTH_CLAMP_CONTROL != 0 {
            self.clamp.pNext = next;
            next = ptr::from_mut(&mut self.clamp).cast();
        }
        if selected & BIAS_FEATURES != 0 {
            self.bias.pNext = next;
            next = ptr::from_mut(&mut self.bias).cast();
        }
        if selected & COLOR_WRITE_ENABLE != 0 {
            self.color.pNext = next;
            next = ptr::from_mut(&mut self.color).cast();
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
        (c"VK_EXT_depth_range_unrestricted", UNRESTRICTED_DEPTH_RANGE),
        (c"VK_EXT_depth_clamp_control", DEPTH_CLAMP_CONTROL),
        (c"VK_EXT_depth_bias_control", BIAS_FEATURES),
        (c"VK_EXT_color_write_enable", COLOR_WRITE_ENABLE),
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
    let mut available = selected & UNRESTRICTED_DEPTH_RANGE;
    for (value, bit) in [
        (native.clamp.depthClampControl, DEPTH_CLAMP_CONTROL),
        (native.bias.depthBiasControl, DEPTH_BIAS_CONTROL),
        (
            native.bias.leastRepresentableValueForceUnormRepresentation,
            DEPTH_BIAS_UNORM,
        ),
        (native.bias.floatRepresentation, DEPTH_BIAS_FLOAT),
        (native.bias.depthBiasExact, DEPTH_BIAS_EXACT),
        (native.color.colorWriteEnable, COLOR_WRITE_ENABLE),
    ] {
        if value != 0 {
            available |= bit;
        }
    }
    if available & DEPTH_BIAS_CONTROL == 0 {
        available &= !BIAS_FEATURES;
    }
    Ok(available)
}
pub(super) struct Commands {
    pub clamp: vk::PFN_vkCmdSetDepthClampRangeEXT,
    pub bias: vk::PFN_vkCmdSetDepthBias2EXT,
    pub color: vk::PFN_vkCmdSetColorWriteEnableEXT,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        macro_rules! load {
            ($bit:ident,$name:literal,$ty:ty) => {{
                if enabled & $bit == 0 {
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
            clamp: load!(
                DEPTH_CLAMP_CONTROL,
                c"vkCmdSetDepthClampRangeEXT",
                vk::PFN_vkCmdSetDepthClampRangeEXT
            ),
            bias: load!(
                DEPTH_BIAS_CONTROL,
                c"vkCmdSetDepthBias2EXT",
                vk::PFN_vkCmdSetDepthBias2EXT
            ),
            color: load!(
                COLOR_WRITE_ENABLE,
                c"vkCmdSetColorWriteEnableEXT",
                vk::PFN_vkCmdSetColorWriteEnableEXT
            ),
        })
    }
}
pub(super) fn clamp(mode: u32, min: f32, max: f32, enabled: FeatureBits) -> Result<(), Status> {
    if enabled & DEPTH_CLAMP_CONTROL == 0 {
        return Err(UNSUPPORTED);
    }
    match mode {
        0 => Ok(()),
        1 => core_raster::depth_bounds(min, max, enabled),
        _ => Err(INVALID),
    }
}
pub(super) fn bias(
    representation: u32,
    exact: u32,
    enabled: FeatureBits,
) -> Result<vk::VkDepthBiasRepresentationInfoEXT, Status> {
    if representation > 2 || exact > 1 {
        return Err(INVALID);
    }
    let bits = DEPTH_BIAS_CONTROL
        | match representation {
            1 => DEPTH_BIAS_UNORM,
            2 => DEPTH_BIAS_FLOAT,
            _ => FeatureBits::ZERO,
        }
        | if exact != 0 {
            DEPTH_BIAS_EXACT
        } else {
            FeatureBits::ZERO
        };
    if bits & !enabled != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(vk::VkDepthBiasRepresentationInfoEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPTH_BIAS_REPRESENTATION_INFO_EXT,
        depthBiasRepresentation: representation,
        depthBiasExact: exact,
        ..Default::default()
    })
}
pub(super) fn prepare_clamp(
    state: Option<DepthClampControl>,
    dynamic: u64,
    enabled: FeatureBits,
) -> Result<Option<DepthClampControl>, Status> {
    let dyn_clamp = dynamic & DYNAMIC_DEPTH_CLAMP_RANGE_STATE != 0;
    if state.is_none() && !dyn_clamp {
        return Ok(None);
    }
    let mut c = state.unwrap_or(DepthClampControl {
        header: Record::new::<DepthClampControl>(DEPTH_CLAMP_CONTROL_STATE),
        mode: 0,
        reserved: 0,
        min: 0.0,
        max: 1.0,
    });
    if c.reserved != 0 {
        return Err(INVALID);
    }
    if dyn_clamp {
        c.mode = 0;
    }
    clamp(c.mode, c.min, c.max, enabled)?;
    Ok(Some(c))
}
pub(super) unsafe fn prepare_color(
    state: Option<ColorWriteState>,
    count: u32,
    dynamic: u64,
    enabled: FeatureBits,
) -> Result<Option<vk::VkPipelineColorWriteCreateInfoEXT>, Status> {
    if dynamic & DYNAMIC_COLOR_WRITE_ENABLE_STATE != 0 {
        if enabled & COLOR_WRITE_ENABLE == 0 {
            return Err(UNSUPPORTED);
        }
        if state.is_some_and(|s| s.reserved != 0) {
            return Err(INVALID);
        }
        // Omitting the native static record is equivalent to an all-true array.
        return Ok(None);
    }
    let Some(s) = state else {
        return Ok(None);
    };
    if s.reserved != 0 || s.count != count {
        return Err(INVALID);
    }
    if enabled & COLOR_WRITE_ENABLE == 0 {
        return Err(UNSUPPORTED);
    }
    let enables = unsafe { commands::slice(s.enables, s.count)? };
    if enables.iter().any(|&v| v > 1) {
        return Err(INVALID);
    }
    Ok(Some(vk::VkPipelineColorWriteCreateInfoEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_COLOR_WRITE_CREATE_INFO_EXT,
        attachmentCount: s.count,
        pColorWriteEnables: s.enables,
        ..Default::default()
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_units_exactness_and_ranges_are_independently_enabled() {
        assert_eq!(clamp(0, f32::NAN, f32::NAN, DEPTH_CLAMP_CONTROL), Ok(()));
        assert_eq!(clamp(1, -2.0, 3.0, DEPTH_CLAMP_CONTROL), Err(INVALID));
        assert_eq!(
            clamp(1, -2.0, 3.0, DEPTH_CLAMP_CONTROL | UNRESTRICTED_DEPTH_RANGE),
            Ok(())
        );
        assert_eq!(clamp(1, 1.0, 0.0, FeatureBits::MAX), Err(INVALID));
        assert_eq!(
            core_raster::depth_bounds(-2.0, 3.0, UNRESTRICTED_DEPTH_RANGE),
            Ok(())
        );
        for (r, bit) in [
            (0, FeatureBits::ZERO),
            (1, DEPTH_BIAS_UNORM),
            (2, DEPTH_BIAS_FLOAT),
        ] {
            assert!(bias(r, 0, DEPTH_BIAS_CONTROL | bit).is_ok());
            assert!(bias(r, 1, DEPTH_BIAS_CONTROL | bit).is_err());
            assert!(bias(r, 1, DEPTH_BIAS_CONTROL | bit | DEPTH_BIAS_EXACT).is_ok());
        }
        assert!(bias(1, 0, DEPTH_BIAS_CONTROL).is_err());
        assert!(bias(2, 0, DEPTH_BIAS_FLOAT).is_err());
        assert_eq!(
            validate_feature_dependencies(DEPTH_BIAS_EXACT),
            Err(INVALID)
        );
    }
    #[test]
    fn dynamic_records_ignore_static_values_but_keep_reserved_validation() {
        let c = DepthClampControl {
            header: Record::new::<DepthClampControl>(DEPTH_CLAMP_CONTROL_STATE),
            mode: 99,
            reserved: 0,
            min: f32::NAN,
            max: f32::NAN,
        };
        assert!(prepare_clamp(
            Some(c),
            DYNAMIC_DEPTH_CLAMP_RANGE_STATE,
            DEPTH_CLAMP_CONTROL
        )
        .is_ok());
        assert!(prepare_clamp(Some(c), 0, DEPTH_CLAMP_CONTROL).is_err());
        let s = ColorWriteState {
            header: Record::new::<ColorWriteState>(COLOR_WRITE_STATE),
            count: u32::MAX,
            reserved: 0,
            enables: ptr::null(),
        };
        assert!(unsafe {
            prepare_color(
                Some(s),
                2,
                DYNAMIC_COLOR_WRITE_ENABLE_STATE,
                COLOR_WRITE_ENABLE,
            )
        }
        .is_ok());
        assert!(unsafe { prepare_color(Some(s), 2, 0, COLOR_WRITE_ENABLE) }.is_err());
        let s = ColorWriteState { reserved: 1, ..s };
        assert!(unsafe {
            prepare_color(
                Some(s),
                2,
                DYNAMIC_COLOR_WRITE_ENABLE_STATE,
                COLOR_WRITE_ENABLE,
            )
        }
        .is_err());
    }
}
