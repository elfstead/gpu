//! Core dynamic raster state. Native commands, no inferred pipeline state.
use super::*;
pub(super) const MASK: u64 = 0xff << 25;
pub(super) const STATES: &[(u64, vk::VkDynamicState)] = &[
    (
        DYNAMIC_TOPOLOGY_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_PRIMITIVE_TOPOLOGY,
    ),
    (
        DYNAMIC_PRIMITIVE_RESTART_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_PRIMITIVE_RESTART_ENABLE,
    ),
    (
        DYNAMIC_RASTERIZER_DISCARD_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_RASTERIZER_DISCARD_ENABLE,
    ),
    (
        DYNAMIC_LINE_WIDTH_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_LINE_WIDTH,
    ),
    (
        DYNAMIC_DEPTH_BIAS_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_BIAS,
    ),
    (
        DYNAMIC_DEPTH_BIAS_ENABLE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_BIAS_ENABLE,
    ),
    (
        DYNAMIC_DEPTH_BOUNDS_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_BOUNDS,
    ),
    (
        DYNAMIC_DEPTH_BOUNDS_ENABLE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_BOUNDS_TEST_ENABLE,
    ),
];
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<DynamicRasterLimits, Status> {
    if !instance
        .supports_extension(physical, c"VK_EXT_extended_dynamic_state3")
        .map_err(loader_error)?
    {
        return Ok(DynamicRasterLimits::default());
    }
    let mut p = vk::VkPhysicalDeviceExtendedDynamicState3PropertiesEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_EXTENDED_DYNAMIC_STATE_3_PROPERTIES_EXT,
        ..Default::default()
    };
    let mut root = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: ptr::from_mut(&mut p).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut root);
    }
    Ok(DynamicRasterLimits {
        unrestricted_topology: p.dynamicPrimitiveTopologyUnrestricted,
    })
}
pub(super) fn topology(value: u32, enabled: FeatureBits) -> Result<(), Status> {
    if value > PATCHES {
        return Err(INVALID);
    }
    if value == PATCHES && enabled & TESSELLATION == 0
        || matches!(value, 6..=9) && enabled & GEOMETRY == 0
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn topology_static(value: &mut u32, dynamic: u64, unrestricted: bool, stages: u64) {
    if dynamic & DYNAMIC_TOPOLOGY_STATE != 0 && unrestricted {
        // Legal placeholders for ignored static state. A strip also permits a
        // static restart enable without unnecessarily requiring list restart.
        *value = if stages & raster_stages::TESS_STAGES != 0 {
            PATCHES
        } else {
            4
        };
    }
}
pub(super) fn sanitize(o: &mut RasterOptions, dynamic: u64) {
    for (mask, field) in [
        (DYNAMIC_PRIMITIVE_RESTART_STATE, &mut o.primitive_restart),
        (DYNAMIC_RASTERIZER_DISCARD_STATE, &mut o.discard),
        (DYNAMIC_DEPTH_BIAS_ENABLE_STATE, &mut o.depth_bias),
        (DYNAMIC_DEPTH_BOUNDS_ENABLE_STATE, &mut o.depth_bounds),
    ] {
        if dynamic & mask != 0 {
            *field = 0;
        }
    }
    if dynamic & DYNAMIC_LINE_WIDTH_STATE != 0 {
        o.line_width = 1.0;
    }
    if dynamic & DYNAMIC_DEPTH_BIAS_STATE != 0 {
        o.depth_bias_constant = 0.0;
        o.depth_bias_clamp = 0.0;
        o.depth_bias_slope = 0.0;
    }
    if dynamic & DYNAMIC_DEPTH_BOUNDS_STATE != 0 {
        o.min_depth_bounds = 0.0;
        o.max_depth_bounds = 1.0;
    }
}
pub(super) fn line_width(width: f32, enabled: FeatureBits) -> Result<(), Status> {
    if !width.is_finite() || width <= 0.0 {
        return Err(INVALID);
    }
    // The native rasterizer chooses a supported width. The queried range is not
    // a command validity constraint; do not reject or normalize finite inputs.
    if width != 1.0 && enabled & WIDE_LINES == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn depth_bias(
    constant: f32,
    clamp: f32,
    slope: f32,
    enabled: FeatureBits,
) -> Result<(), Status> {
    if ![constant, clamp, slope].iter().all(|v| v.is_finite()) {
        return Err(INVALID);
    }
    if clamp != 0.0 && enabled & DEPTH_BIAS_CLAMP == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn depth_bounds(min: f32, max: f32, enabled: FeatureBits) -> Result<(), Status> {
    if ![min, max].iter().all(|v| {
        v.is_finite() && (enabled & UNRESTRICTED_DEPTH_RANGE != 0 || (0.0..=1.0).contains(v))
    }) || min > max
    {
        return Err(INVALID);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignored_static_values_are_independent_of_other_fields_and_topology_class() {
        let mut o = RasterOptions {
            primitive_restart: 9,
            discard: 9,
            depth_bias: 9,
            depth_bounds: 9,
            line_width: f32::NAN,
            depth_bias_constant: f32::NAN,
            depth_bias_slope: f32::NAN,
            depth_bias_clamp: f32::NAN,
            min_depth_bounds: f32::NAN,
            max_depth_bounds: f32::NAN,
            polygon_mode: 2,
            ..Default::default()
        };
        sanitize(&mut o, DYNAMIC_LINE_WIDTH_STATE);
        assert_eq!(o.line_width, 1.0);
        assert_eq!(o.primitive_restart, 9);
        assert!(o.depth_bias_slope.is_nan());
        sanitize(&mut o, MASK);
        assert_eq!(
            (
                o.primitive_restart,
                o.discard,
                o.depth_bias,
                o.depth_bounds,
                o.polygon_mode
            ),
            (0, 0, 0, 0, 2)
        );
        assert_eq!(
            (
                o.depth_bias_constant,
                o.depth_bias_slope,
                o.depth_bias_clamp,
                o.min_depth_bounds,
                o.max_depth_bounds
            ),
            (0.0, 0.0, 0.0, 0.0, 1.0)
        );
        let mut t = u32::MAX;
        topology_static(&mut t, DYNAMIC_TOPOLOGY_STATE, false, 8);
        assert_eq!(t, u32::MAX);
        topology_static(&mut t, DYNAMIC_TOPOLOGY_STATE, true, 8);
        assert_eq!(t, 4);
        topology_static(
            &mut t,
            DYNAMIC_TOPOLOGY_STATE,
            true,
            raster_stages::TESS_STAGES,
        );
        assert_eq!(t, PATCHES);
        assert_eq!(STATES.iter().fold(0, |m, (s, _)| m | s), MASK);
    }
    #[test]
    fn values_preserve_native_limits_and_independent_capabilities() {
        assert_eq!(line_width(1.0, FeatureBits::ZERO), Ok(()));
        assert_eq!(line_width(2.0, FeatureBits::ZERO), Err(UNSUPPORTED));
        assert_eq!(line_width(2.0, WIDE_LINES), Ok(()));
        assert_eq!(line_width(9.0, WIDE_LINES), Ok(()));
        assert_eq!(line_width(f32::NAN, WIDE_LINES), Err(INVALID));
        assert_eq!(line_width(0.0, WIDE_LINES), Err(INVALID));
        assert_eq!(depth_bias(-2.0, 0.0, 3.0, FeatureBits::ZERO), Ok(()));
        assert_eq!(
            depth_bias(0.0, -1.0, 0.0, FeatureBits::ZERO),
            Err(UNSUPPORTED)
        );
        assert_eq!(depth_bias(0.0, -1.0, 0.0, DEPTH_BIAS_CLAMP), Ok(()));
        assert_eq!(
            depth_bias(f32::INFINITY, 0.0, 0.0, DEPTH_BIAS_CLAMP),
            Err(INVALID)
        );
        assert_eq!(depth_bounds(0.0, 1.0, FeatureBits::ZERO), Ok(()));
        for (a, b) in [(0.7, 0.3), (-0.1, 1.0), (0.0, 1.1), (f32::NAN, 1.0)] {
            assert_eq!(depth_bounds(a, b, FeatureBits::ZERO), Err(INVALID));
        }
        assert_eq!(topology(4, FeatureBits::ZERO), Ok(()));
        assert_eq!(topology(11, FeatureBits::MAX), Err(INVALID));
        assert_eq!(topology(PATCHES, FeatureBits::ZERO), Err(UNSUPPORTED));
        assert_eq!(topology(PATCHES, TESSELLATION), Ok(()));
        assert_eq!(topology(6, TESSELLATION), Err(UNSUPPORTED));
        assert_eq!(topology(6, GEOMETRY), Ok(()));
    }
}
