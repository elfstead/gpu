//! Optional native extended dynamic raster state. No current-state cache.
use super::*;
pub(super) const EDS3_FEATURES: FeatureBits = DYNAMIC_TESSELLATION_ORIGIN
    | DYNAMIC_DEPTH_CLAMP
    | DYNAMIC_POLYGON_MODE
    | DYNAMIC_RASTERIZATION_SAMPLES
    | DYNAMIC_SAMPLE_MASK
    | DYNAMIC_ALPHA_TO_COVERAGE
    | DYNAMIC_ALPHA_TO_ONE
    | DYNAMIC_LOGIC_OP_ENABLE
    | DYNAMIC_COLOR_BLEND_ENABLE
    | DYNAMIC_COLOR_BLEND_EQUATION
    | DYNAMIC_COLOR_WRITE_MASK;
pub(super) const STATES: &[(u64, vk::VkDynamicState)] = &[
    (
        DYNAMIC_PATCH_CONTROL_POINTS_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_PATCH_CONTROL_POINTS_EXT,
    ),
    (
        DYNAMIC_TESSELLATION_ORIGIN_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_TESSELLATION_DOMAIN_ORIGIN_EXT,
    ),
    (
        DYNAMIC_DEPTH_CLAMP_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_CLAMP_ENABLE_EXT,
    ),
    (
        DYNAMIC_POLYGON_MODE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_POLYGON_MODE_EXT,
    ),
    (
        DYNAMIC_RASTERIZATION_SAMPLES_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_RASTERIZATION_SAMPLES_EXT,
    ),
    (
        DYNAMIC_SAMPLE_MASK_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_SAMPLE_MASK_EXT,
    ),
    (
        DYNAMIC_ALPHA_TO_COVERAGE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_ALPHA_TO_COVERAGE_ENABLE_EXT,
    ),
    (
        DYNAMIC_ALPHA_TO_ONE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_ALPHA_TO_ONE_ENABLE_EXT,
    ),
    (
        DYNAMIC_LOGIC_OP_ENABLE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_LOGIC_OP_ENABLE_EXT,
    ),
    (
        DYNAMIC_COLOR_BLEND_ENABLE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_COLOR_BLEND_ENABLE_EXT,
    ),
    (
        DYNAMIC_COLOR_BLEND_EQUATION_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_COLOR_BLEND_EQUATION_EXT,
    ),
    (
        DYNAMIC_COLOR_WRITE_MASK_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_COLOR_WRITE_MASK_EXT,
    ),
    (
        DYNAMIC_LOGIC_OP_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_LOGIC_OP_EXT,
    ),
];
pub(super) fn dynamic_mask(enabled: FeatureBits) -> u64 {
    let mut mask = 0;
    if enabled & DYNAMIC_PATCH_CONTROL_POINTS != 0 {
        mask |= DYNAMIC_PATCH_CONTROL_POINTS_STATE;
    }
    if enabled & DYNAMIC_TESSELLATION_ORIGIN != 0 {
        mask |= DYNAMIC_TESSELLATION_ORIGIN_STATE;
    }
    if enabled & DYNAMIC_DEPTH_CLAMP != 0 {
        mask |= DYNAMIC_DEPTH_CLAMP_STATE;
    }
    if enabled & DYNAMIC_POLYGON_MODE != 0 {
        mask |= DYNAMIC_POLYGON_MODE_STATE;
    }
    if enabled & DYNAMIC_RASTERIZATION_SAMPLES != 0 {
        mask |= DYNAMIC_RASTERIZATION_SAMPLES_STATE;
    }
    if enabled & DYNAMIC_SAMPLE_MASK != 0 {
        mask |= DYNAMIC_SAMPLE_MASK_STATE;
    }
    if enabled & DYNAMIC_ALPHA_TO_COVERAGE != 0 {
        mask |= DYNAMIC_ALPHA_TO_COVERAGE_STATE;
    }
    if enabled & DYNAMIC_ALPHA_TO_ONE != 0 {
        mask |= DYNAMIC_ALPHA_TO_ONE_STATE;
    }
    if enabled & DYNAMIC_LOGIC_OP_ENABLE != 0 {
        mask |= DYNAMIC_LOGIC_OP_ENABLE_STATE;
    }
    if enabled & DYNAMIC_COLOR_BLEND_ENABLE != 0 {
        mask |= DYNAMIC_COLOR_BLEND_ENABLE_STATE;
    }
    if enabled & DYNAMIC_COLOR_BLEND_EQUATION != 0 {
        mask |= DYNAMIC_COLOR_BLEND_EQUATION_STATE;
    }
    if enabled & DYNAMIC_COLOR_WRITE_MASK != 0 {
        mask |= DYNAMIC_COLOR_WRITE_MASK_STATE;
    }
    if enabled & DYNAMIC_LOGIC_OP != 0 {
        mask |= DYNAMIC_LOGIC_OP_STATE;
    }
    mask
}
pub(super) struct Commands {
    pub patch_control_points: vk::PFN_vkCmdSetPatchControlPointsEXT,
    pub tessellation_origin: vk::PFN_vkCmdSetTessellationDomainOriginEXT,
    pub depth_clamp: vk::PFN_vkCmdSetDepthClampEnableEXT,
    pub polygon_mode: vk::PFN_vkCmdSetPolygonModeEXT,
    pub rasterization_samples: vk::PFN_vkCmdSetRasterizationSamplesEXT,
    pub sample_mask: vk::PFN_vkCmdSetSampleMaskEXT,
    pub alpha_to_coverage: vk::PFN_vkCmdSetAlphaToCoverageEnableEXT,
    pub alpha_to_one: vk::PFN_vkCmdSetAlphaToOneEnableEXT,
    pub logic_op_enable: vk::PFN_vkCmdSetLogicOpEnableEXT,
    pub color_blend_enable: vk::PFN_vkCmdSetColorBlendEnableEXT,
    pub color_blend_equation: vk::PFN_vkCmdSetColorBlendEquationEXT,
    pub color_write_mask: vk::PFN_vkCmdSetColorWriteMaskEXT,
    pub logic_op: vk::PFN_vkCmdSetLogicOpEXT,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        macro_rules! load {
            ($bit:ident, $name:literal, $ty:ty) => {
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
            };
        }
        Ok(Self {
            patch_control_points: load!(
                DYNAMIC_PATCH_CONTROL_POINTS,
                c"vkCmdSetPatchControlPointsEXT",
                vk::PFN_vkCmdSetPatchControlPointsEXT
            ),
            tessellation_origin: load!(
                DYNAMIC_TESSELLATION_ORIGIN,
                c"vkCmdSetTessellationDomainOriginEXT",
                vk::PFN_vkCmdSetTessellationDomainOriginEXT
            ),
            depth_clamp: load!(
                DYNAMIC_DEPTH_CLAMP,
                c"vkCmdSetDepthClampEnableEXT",
                vk::PFN_vkCmdSetDepthClampEnableEXT
            ),
            polygon_mode: load!(
                DYNAMIC_POLYGON_MODE,
                c"vkCmdSetPolygonModeEXT",
                vk::PFN_vkCmdSetPolygonModeEXT
            ),
            rasterization_samples: load!(
                DYNAMIC_RASTERIZATION_SAMPLES,
                c"vkCmdSetRasterizationSamplesEXT",
                vk::PFN_vkCmdSetRasterizationSamplesEXT
            ),
            sample_mask: load!(
                DYNAMIC_SAMPLE_MASK,
                c"vkCmdSetSampleMaskEXT",
                vk::PFN_vkCmdSetSampleMaskEXT
            ),
            alpha_to_coverage: load!(
                DYNAMIC_ALPHA_TO_COVERAGE,
                c"vkCmdSetAlphaToCoverageEnableEXT",
                vk::PFN_vkCmdSetAlphaToCoverageEnableEXT
            ),
            alpha_to_one: load!(
                DYNAMIC_ALPHA_TO_ONE,
                c"vkCmdSetAlphaToOneEnableEXT",
                vk::PFN_vkCmdSetAlphaToOneEnableEXT
            ),
            logic_op_enable: load!(
                DYNAMIC_LOGIC_OP_ENABLE,
                c"vkCmdSetLogicOpEnableEXT",
                vk::PFN_vkCmdSetLogicOpEnableEXT
            ),
            color_blend_enable: load!(
                DYNAMIC_COLOR_BLEND_ENABLE,
                c"vkCmdSetColorBlendEnableEXT",
                vk::PFN_vkCmdSetColorBlendEnableEXT
            ),
            color_blend_equation: load!(
                DYNAMIC_COLOR_BLEND_EQUATION,
                c"vkCmdSetColorBlendEquationEXT",
                vk::PFN_vkCmdSetColorBlendEquationEXT
            ),
            color_write_mask: load!(
                DYNAMIC_COLOR_WRITE_MASK,
                c"vkCmdSetColorWriteMaskEXT",
                vk::PFN_vkCmdSetColorWriteMaskEXT
            ),
            logic_op: load!(
                DYNAMIC_LOGIC_OP,
                c"vkCmdSetLogicOpEXT",
                vk::PFN_vkCmdSetLogicOpEXT
            ),
        })
    }
}
pub(super) fn sanitize(s: &mut GraphicsState, o: &mut RasterOptions, dynamic: u64) {
    for (bit, field) in [
        (DYNAMIC_DEPTH_CLAMP_STATE, &mut o.depth_clamp),
        (DYNAMIC_POLYGON_MODE_STATE, &mut o.polygon_mode),
        (DYNAMIC_ALPHA_TO_COVERAGE_STATE, &mut o.alpha_to_coverage),
        (DYNAMIC_ALPHA_TO_ONE_STATE, &mut o.alpha_to_one),
        (DYNAMIC_LOGIC_OP_ENABLE_STATE, &mut s.logic_op_enable),
        (DYNAMIC_LOGIC_OP_STATE, &mut s.logic_op),
    ] {
        if dynamic & bit != 0 {
            *field = 0;
        }
    }
    if dynamic & DYNAMIC_SAMPLE_MASK_STATE != 0 {
        o.sample_mask = u64::MAX;
    }
    if dynamic & DYNAMIC_RASTERIZATION_SAMPLES_STATE != 0 {
        // Native static rasterizationSamples still sizes a static mask. Preserve
        // both words of OGPU's u64, irrespective of the eventual draw sample count.
        s.samples = if dynamic & DYNAMIC_SAMPLE_MASK_STATE == 0 {
            64
        } else {
            1
        };
    }
}
pub(super) fn color(mut c: ColorState, dynamic: u64) -> ColorState {
    if dynamic & DYNAMIC_COLOR_BLEND_ENABLE_STATE != 0 {
        c.blend = 0;
    }
    if dynamic & DYNAMIC_COLOR_WRITE_MASK_STATE != 0 {
        c.write_mask = 0;
    }
    if dynamic & DYNAMIC_COLOR_BLEND_EQUATION_STATE != 0 {
        c.src_color = 0;
        c.dst_color = 0;
        c.color_op = 0;
        c.src_alpha = 0;
        c.dst_alpha = 0;
        c.alpha_op = 0;
    }
    c
}
pub(super) fn equation(e: &BlendEquation, enabled: FeatureBits) -> Result<(), Status> {
    let factors = [e.src_color, e.dst_color, e.src_alpha, e.dst_alpha];
    if factors.iter().any(|&f| f > 18) || e.color_op > 4 || e.alpha_op > 4 {
        return Err(INVALID);
    }
    if factors.iter().any(|&f| f >= 15) && enabled & DUAL_SOURCE_BLEND == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn scalar(field: u64, value: u32, enabled: FeatureBits) -> Result<(), Status> {
    if dynamic_mask(enabled) & field == 0 {
        return Err(UNSUPPORTED);
    }
    let max = match field {
        DYNAMIC_PATCH_CONTROL_POINTS_STATE => {
            if value == 0 {
                return Err(INVALID);
            }
            u32::MAX
        }
        DYNAMIC_TESSELLATION_ORIGIN_STATE => 1,
        DYNAMIC_POLYGON_MODE_STATE => 2,
        DYNAMIC_LOGIC_OP_STATE => 15,
        DYNAMIC_RASTERIZATION_SAMPLES_STATE => {
            return samples(value, 127);
        }
        DYNAMIC_DEPTH_CLAMP_STATE
        | DYNAMIC_ALPHA_TO_COVERAGE_STATE
        | DYNAMIC_ALPHA_TO_ONE_STATE
        | DYNAMIC_LOGIC_OP_ENABLE_STATE => 1,
        _ => return Err(INVALID),
    };
    if value > max {
        return Err(INVALID);
    }
    let base = match field {
        DYNAMIC_DEPTH_CLAMP_STATE => DEPTH_CLAMP,
        DYNAMIC_POLYGON_MODE_STATE => NON_SOLID_FILL,
        DYNAMIC_ALPHA_TO_ONE_STATE => ALPHA_TO_ONE,
        DYNAMIC_LOGIC_OP_ENABLE_STATE => LOGIC_OP,
        _ => 0,
    };
    if value != 0 && base & !enabled != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn attachment_range(first: u32, count: u32, max: u32) -> Result<(), Status> {
    if first.checked_add(count).ok_or(INVALID)? > max {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
// The per-attachment command borrows this exact native layout, without packing.
const _: () = {
    assert!(size_of::<BlendEquation>() == size_of::<vk::VkColorBlendEquationEXT>());
    assert!(align_of::<BlendEquation>() == align_of::<vk::VkColorBlendEquationEXT>());
    assert!(
        std::mem::offset_of!(BlendEquation, src_color)
            == std::mem::offset_of!(vk::VkColorBlendEquationEXT, srcColorBlendFactor)
    );
    assert!(
        std::mem::offset_of!(BlendEquation, dst_color)
            == std::mem::offset_of!(vk::VkColorBlendEquationEXT, dstColorBlendFactor)
    );
    assert!(
        std::mem::offset_of!(BlendEquation, color_op)
            == std::mem::offset_of!(vk::VkColorBlendEquationEXT, colorBlendOp)
    );
    assert!(
        std::mem::offset_of!(BlendEquation, src_alpha)
            == std::mem::offset_of!(vk::VkColorBlendEquationEXT, srcAlphaBlendFactor)
    );
    assert!(
        std::mem::offset_of!(BlendEquation, dst_alpha)
            == std::mem::offset_of!(vk::VkColorBlendEquationEXT, dstAlphaBlendFactor)
    );
    assert!(
        std::mem::offset_of!(BlendEquation, alpha_op)
            == std::mem::offset_of!(vk::VkColorBlendEquationEXT, alphaBlendOp)
    );
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dynamic_values_do_not_enable_base_capabilities_or_neighbor_states() {
        for (field, feature, base) in [
            (DYNAMIC_DEPTH_CLAMP_STATE, DYNAMIC_DEPTH_CLAMP, DEPTH_CLAMP),
            (
                DYNAMIC_POLYGON_MODE_STATE,
                DYNAMIC_POLYGON_MODE,
                NON_SOLID_FILL,
            ),
            (
                DYNAMIC_ALPHA_TO_ONE_STATE,
                DYNAMIC_ALPHA_TO_ONE,
                ALPHA_TO_ONE,
            ),
            (
                DYNAMIC_LOGIC_OP_ENABLE_STATE,
                DYNAMIC_LOGIC_OP_ENABLE,
                LOGIC_OP,
            ),
        ] {
            assert_eq!(scalar(field, 0, base), Err(UNSUPPORTED));
            assert_eq!(scalar(field, 0, feature), Ok(()));
            assert_eq!(scalar(field, 1, feature), Err(UNSUPPORTED));
            assert_eq!(scalar(field, 1, feature | base), Ok(()));
            assert_eq!(scalar(field, 99, feature | base), Err(INVALID));
            assert_eq!(dynamic_mask(feature), field);
        }
        assert_eq!(
            scalar(
                DYNAMIC_RASTERIZATION_SAMPLES_STATE,
                64,
                DYNAMIC_RASTERIZATION_SAMPLES
            ),
            Ok(())
        );
        assert_eq!(
            scalar(
                DYNAMIC_RASTERIZATION_SAMPLES_STATE,
                3,
                DYNAMIC_RASTERIZATION_SAMPLES
            ),
            Err(INVALID)
        );
        assert_eq!(
            dynamic_mask(FeatureBits::MAX),
            STATES.iter().fold(0, |m, (s, _)| m | s)
        );
    }
    #[test]
    fn blend_equations_and_ranges_keep_native_values() {
        for field in 0..4 {
            let mut e = BlendEquation::default();
            for factor in 0..=19 {
                [
                    &mut e.src_color,
                    &mut e.dst_color,
                    &mut e.src_alpha,
                    &mut e.dst_alpha,
                ][field]
                    .clone_from(&factor);
                assert_eq!(
                    equation(&e, 0),
                    if factor > 18 {
                        Err(INVALID)
                    } else if factor >= 15 {
                        Err(UNSUPPORTED)
                    } else {
                        Ok(())
                    }
                );
                assert_eq!(
                    equation(&e, DUAL_SOURCE_BLEND),
                    if factor > 18 { Err(INVALID) } else { Ok(()) }
                );
            }
        }
        assert_eq!(
            equation(
                &BlendEquation {
                    alpha_op: 5,
                    ..Default::default()
                },
                DUAL_SOURCE_BLEND
            ),
            Err(INVALID)
        );
        assert_eq!(attachment_range(8, 0, 8), Ok(()));
        assert_eq!(attachment_range(8, 1, 8), Err(UNSUPPORTED));
        assert_eq!(attachment_range(u32::MAX, 1, u32::MAX), Err(INVALID));
    }
    #[test]
    fn sample_mask_width_is_preserved_and_dynamic_fields_alone_are_ignored() {
        // All fields are scalars or raw pointers; zero is a valid Rust representation.
        let mut s: GraphicsState = unsafe { std::mem::zeroed() };
        s.samples = u32::MAX;
        s.logic_op = 99;
        s.logic_op_enable = 99;
        let mut o = RasterOptions {
            depth_clamp: 99,
            polygon_mode: 99,
            alpha_to_one: 99,
            alpha_to_coverage: 99,
            sample_mask: 1 << 63,
            ..Default::default()
        };
        sanitize(&mut s, &mut o, DYNAMIC_RASTERIZATION_SAMPLES_STATE);
        assert_eq!(s.samples, 64);
        assert_eq!(o.sample_mask, 1 << 63);
        assert_eq!((s.logic_op, o.polygon_mode), (99, 99));
        sanitize(&mut s, &mut o, dynamic_mask(FeatureBits::MAX));
        assert_eq!((s.samples, s.logic_op, s.logic_op_enable), (1, 0, 0));
        assert_eq!(
            (
                o.depth_clamp,
                o.polygon_mode,
                o.alpha_to_one,
                o.alpha_to_coverage
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(o.sample_mask, u64::MAX);
        let c = ColorState {
            format: 37,
            write_mask: 99,
            blend: 99,
            src_color: 99,
            dst_color: 99,
            color_op: 99,
            src_alpha: 99,
            dst_alpha: 99,
            alpha_op: 99,
        };
        let v = color(c, DYNAMIC_COLOR_BLEND_EQUATION_STATE);
        assert_eq!(
            (v.format, v.write_mask, v.blend, v.src_color, v.alpha_op),
            (37, 99, 99, 0, 0)
        );
        let v = color(
            c,
            DYNAMIC_COLOR_WRITE_MASK_STATE | DYNAMIC_COLOR_BLEND_ENABLE_STATE,
        );
        assert_eq!(
            (v.format, v.write_mask, v.blend, v.src_color),
            (37, 0, 0, 99)
        );
    }
}
