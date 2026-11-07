//! Native raster mechanisms and dynamic dispatch; no pipeline-dependent command state.
use super::*;
pub(super) const STIPPLED: FeatureBits = FeatureBits::union(&[
    STIPPLED_RECTANGULAR_LINES,
    STIPPLED_BRESENHAM_LINES,
    STIPPLED_SMOOTH_LINES,
]);
pub(super) const DYNAMIC_FEATURES: FeatureBits = FeatureBits::union(&[
    DYNAMIC_CONSERVATIVE_MODE,
    DYNAMIC_EXTRA_OVERESTIMATION,
    DYNAMIC_PROVOKING_VERTEX,
    DYNAMIC_DEPTH_CLIP,
    DYNAMIC_DEPTH_CONVENTION,
    DYNAMIC_LINE_MODE,
    DYNAMIC_LINE_STIPPLE_ENABLE,
    DYNAMIC_SAMPLE_LOCATIONS_ENABLE,
    extended_raster::EDS3_FEATURES,
]);
pub(super) const DYNAMIC_STATES: &[(u64, vk::VkDynamicState)] = &[
    (
        DYNAMIC_SAMPLE_LOCATIONS_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_SAMPLE_LOCATIONS_EXT,
    ),
    (
        DYNAMIC_SAMPLE_LOCATIONS_ENABLE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_SAMPLE_LOCATIONS_ENABLE_EXT,
    ),
    (
        DYNAMIC_CONSERVATIVE_MODE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_CONSERVATIVE_RASTERIZATION_MODE_EXT,
    ),
    (
        DYNAMIC_EXTRA_OVERESTIMATION_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_EXTRA_PRIMITIVE_OVERESTIMATION_SIZE_EXT,
    ),
    (
        DYNAMIC_PROVOKING_VERTEX_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_PROVOKING_VERTEX_MODE_EXT,
    ),
    (
        DYNAMIC_DEPTH_CLIP_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_CLIP_ENABLE_EXT,
    ),
    (
        DYNAMIC_DEPTH_CONVENTION_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_DEPTH_CLIP_NEGATIVE_ONE_TO_ONE_EXT,
    ),
    (
        DYNAMIC_LINE_MODE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_LINE_RASTERIZATION_MODE_EXT,
    ),
    (
        DYNAMIC_LINE_STIPPLE_ENABLE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_LINE_STIPPLE_ENABLE_EXT,
    ),
    (
        DYNAMIC_LINE_STIPPLE_STATE,
        vk::VkDynamicState_VK_DYNAMIC_STATE_LINE_STIPPLE,
    ),
];
pub(super) fn dynamic_mask(enabled: FeatureBits) -> u64 {
    let mut mask = if enabled & STIPPLED != 0 {
        DYNAMIC_LINE_STIPPLE_STATE
    } else {
        0
    };
    if enabled & DYNAMIC_CONSERVATIVE_MODE != 0 {
        mask |= DYNAMIC_CONSERVATIVE_MODE_STATE;
    }
    if enabled & DYNAMIC_EXTRA_OVERESTIMATION != 0 {
        mask |= DYNAMIC_EXTRA_OVERESTIMATION_STATE;
    }
    if enabled & DYNAMIC_PROVOKING_VERTEX != 0 {
        mask |= DYNAMIC_PROVOKING_VERTEX_STATE;
    }
    if enabled & DYNAMIC_DEPTH_CLIP != 0 {
        mask |= DYNAMIC_DEPTH_CLIP_STATE;
    }
    if enabled & DYNAMIC_DEPTH_CONVENTION != 0 {
        mask |= DYNAMIC_DEPTH_CONVENTION_STATE;
    }
    if enabled & DYNAMIC_LINE_MODE != 0 {
        mask |= DYNAMIC_LINE_MODE_STATE;
    }
    if enabled & DYNAMIC_LINE_STIPPLE_ENABLE != 0 {
        mask |= DYNAMIC_LINE_STIPPLE_ENABLE_STATE;
    }
    if enabled & SAMPLE_LOCATIONS != 0 {
        mask |= DYNAMIC_SAMPLE_LOCATIONS_STATE;
    }
    if enabled & DYNAMIC_SAMPLE_LOCATIONS_ENABLE != 0 {
        mask |= DYNAMIC_SAMPLE_LOCATIONS_ENABLE_STATE;
    }
    mask
}
pub(super) struct Native {
    provoking: vk::VkPhysicalDeviceProvokingVertexFeaturesEXT,
    convention: vk::VkPhysicalDeviceDepthClipControlFeaturesEXT,
    dynamic: vk::VkPhysicalDeviceExtendedDynamicState3FeaturesEXT,
    dynamic2: vk::VkPhysicalDeviceExtendedDynamicState2FeaturesEXT,
}
impl Native {
    pub fn new(enabled: FeatureBits) -> Self {
        Self {
            provoking: vk::VkPhysicalDeviceProvokingVertexFeaturesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROVOKING_VERTEX_FEATURES_EXT,
                provokingVertexLast: u32::from(enabled & PROVOKING_LAST != 0),
                ..Default::default()
            },
            convention: vk::VkPhysicalDeviceDepthClipControlFeaturesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_DEPTH_CLIP_CONTROL_FEATURES_EXT,
                depthClipControl: u32::from(enabled & DEPTH_NEGATIVE_ONE_TO_ONE != 0),
                ..Default::default()
            },
            dynamic2: vk::VkPhysicalDeviceExtendedDynamicState2FeaturesEXT {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_EXTENDED_DYNAMIC_STATE_2_FEATURES_EXT,
                extendedDynamicState2LogicOp: u32::from(enabled & DYNAMIC_LOGIC_OP != 0),
                extendedDynamicState2PatchControlPoints: u32::from(enabled & DYNAMIC_PATCH_CONTROL_POINTS != 0),
                ..Default::default()
            },
            dynamic: vk::VkPhysicalDeviceExtendedDynamicState3FeaturesEXT {
                extendedDynamicState3RasterizationStream:u32::from(enabled & DYNAMIC_RASTERIZATION_STREAM !=0),
                extendedDynamicState3TessellationDomainOrigin:u32::from(enabled & DYNAMIC_TESSELLATION_ORIGIN !=0),
                extendedDynamicState3DepthClampEnable: u32::from(enabled & DYNAMIC_DEPTH_CLAMP != 0),
                extendedDynamicState3PolygonMode: u32::from(enabled & DYNAMIC_POLYGON_MODE != 0),
                extendedDynamicState3RasterizationSamples: u32::from(enabled & DYNAMIC_RASTERIZATION_SAMPLES != 0),
                extendedDynamicState3SampleMask: u32::from(enabled & DYNAMIC_SAMPLE_MASK != 0),
                extendedDynamicState3AlphaToCoverageEnable: u32::from(enabled & DYNAMIC_ALPHA_TO_COVERAGE != 0),
                extendedDynamicState3AlphaToOneEnable: u32::from(enabled & DYNAMIC_ALPHA_TO_ONE != 0),
                extendedDynamicState3LogicOpEnable: u32::from(enabled & DYNAMIC_LOGIC_OP_ENABLE != 0),
                extendedDynamicState3ColorBlendEnable: u32::from(enabled & DYNAMIC_COLOR_BLEND_ENABLE != 0),
                extendedDynamicState3ColorBlendEquation: u32::from(enabled & DYNAMIC_COLOR_BLEND_EQUATION != 0),
                extendedDynamicState3ColorWriteMask: u32::from(enabled & DYNAMIC_COLOR_WRITE_MASK != 0),

                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_EXTENDED_DYNAMIC_STATE_3_FEATURES_EXT,
                extendedDynamicState3SampleLocationsEnable: u32::from(enabled & DYNAMIC_SAMPLE_LOCATIONS_ENABLE != 0),
                extendedDynamicState3ConservativeRasterizationMode: u32::from(
                    enabled & DYNAMIC_CONSERVATIVE_MODE != 0,
                ),
                extendedDynamicState3ExtraPrimitiveOverestimationSize: u32::from(
                    enabled & DYNAMIC_EXTRA_OVERESTIMATION != 0,
                ),
                extendedDynamicState3ProvokingVertexMode: u32::from(
                    enabled & DYNAMIC_PROVOKING_VERTEX != 0,
                ),
                extendedDynamicState3DepthClipEnable: u32::from(enabled & DYNAMIC_DEPTH_CLIP != 0),
                extendedDynamicState3DepthClipNegativeOneToOne: u32::from(
                    enabled & DYNAMIC_DEPTH_CONVENTION != 0,
                ),
                extendedDynamicState3LineRasterizationMode: u32::from(enabled & DYNAMIC_LINE_MODE != 0),
                extendedDynamicState3LineStippleEnable: u32::from(
                    enabled & DYNAMIC_LINE_STIPPLE_ENABLE != 0,
                ),
                ..Default::default()
            },
        }
    }
    // Link only after placement; keep self at this address until the native call returns.
    pub fn link(
        &mut self,
        mut next: *mut std::ffi::c_void,
        selected: FeatureBits,
    ) -> *mut std::ffi::c_void {
        if selected & PROVOKING_LAST != 0 {
            self.provoking.pNext = next;
            next = ptr::from_mut(&mut self.provoking).cast();
        }
        if selected & DEPTH_NEGATIVE_ONE_TO_ONE != 0 {
            self.convention.pNext = next;
            next = ptr::from_mut(&mut self.convention).cast();
        }
        if selected & (DYNAMIC_LOGIC_OP | DYNAMIC_PATCH_CONTROL_POINTS) != 0 {
            self.dynamic2.pNext = next;
            next = ptr::from_mut(&mut self.dynamic2).cast();
        }
        if selected & DYNAMIC_FEATURES != 0 {
            self.dynamic.pNext = next;
            next = ptr::from_mut(&mut self.dynamic).cast();
        }
        next
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
    v14: &vk::VkPhysicalDeviceVulkan14Properties,
    core: &vk::VkPhysicalDeviceLimits,
) -> Result<(FeatureBits, RasterMechanismLimits), Status> {
    let mut selected = FeatureBits::ZERO;
    for (name, bit) in [
        (c"VK_EXT_conservative_rasterization", CONSERVATIVE_RASTER),
        (c"VK_EXT_provoking_vertex", PROVOKING_LAST),
        (c"VK_EXT_depth_clip_control", DEPTH_NEGATIVE_ONE_TO_ONE),
        (c"VK_EXT_extended_dynamic_state3", DYNAMIC_FEATURES),
        (
            c"VK_EXT_extended_dynamic_state2",
            DYNAMIC_LOGIC_OP | DYNAMIC_PATCH_CONTROL_POINTS,
        ),
    ] {
        if instance
            .supports_extension(physical, name)
            .map_err(loader_error)?
        {
            selected |= bit;
        }
    }
    let mut native = Native::new(FeatureBits::ZERO);
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: native.link(ptr::null_mut(), selected),
        ..Default::default()
    };
    let mut conservative=vk::VkPhysicalDeviceConservativeRasterizationPropertiesEXT {
            sType:vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_CONSERVATIVE_RASTERIZATION_PROPERTIES_EXT,
            ..Default::default() };
    let mut provoking = vk::VkPhysicalDeviceProvokingVertexPropertiesEXT {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROVOKING_VERTEX_PROPERTIES_EXT,
        ..Default::default()
    };
    let mut next = ptr::null_mut();
    if selected & CONSERVATIVE_RASTER != 0 {
        conservative.pNext = next;
        next = ptr::from_mut(&mut conservative).cast();
    }
    if selected & PROVOKING_LAST != 0 {
        provoking.pNext = next;
        next = ptr::from_mut(&mut provoking).cast();
    }
    let mut props = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: next,
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut props);
    }
    let mut available = selected & CONSERVATIVE_RASTER;
    if native.dynamic.extendedDynamicState3RasterizationStream != 0 {
        available |= DYNAMIC_RASTERIZATION_STREAM;
    }
    if native.dynamic2.extendedDynamicState2PatchControlPoints != 0 {
        available |= DYNAMIC_PATCH_CONTROL_POINTS;
    }
    if native.dynamic.extendedDynamicState3TessellationDomainOrigin != 0 {
        available |= DYNAMIC_TESSELLATION_ORIGIN;
    }
    if native.dynamic2.extendedDynamicState2LogicOp != 0 {
        available |= DYNAMIC_LOGIC_OP;
    }
    if native.dynamic.extendedDynamicState3DepthClampEnable != 0 {
        available |= DYNAMIC_DEPTH_CLAMP;
    }
    if native.dynamic.extendedDynamicState3PolygonMode != 0 {
        available |= DYNAMIC_POLYGON_MODE;
    }
    if native.dynamic.extendedDynamicState3RasterizationSamples != 0 {
        available |= DYNAMIC_RASTERIZATION_SAMPLES;
    }
    if native.dynamic.extendedDynamicState3SampleMask != 0 {
        available |= DYNAMIC_SAMPLE_MASK;
    }
    if native.dynamic.extendedDynamicState3AlphaToCoverageEnable != 0 {
        available |= DYNAMIC_ALPHA_TO_COVERAGE;
    }
    if native.dynamic.extendedDynamicState3AlphaToOneEnable != 0 {
        available |= DYNAMIC_ALPHA_TO_ONE;
    }
    if native.dynamic.extendedDynamicState3LogicOpEnable != 0 {
        available |= DYNAMIC_LOGIC_OP_ENABLE;
    }
    if native.dynamic.extendedDynamicState3ColorBlendEnable != 0 {
        available |= DYNAMIC_COLOR_BLEND_ENABLE;
    }
    if native.dynamic.extendedDynamicState3ColorBlendEquation != 0 {
        available |= DYNAMIC_COLOR_BLEND_EQUATION;
    }
    if native.dynamic.extendedDynamicState3ColorWriteMask != 0 {
        available |= DYNAMIC_COLOR_WRITE_MASK;
    }

    if native.dynamic.extendedDynamicState3SampleLocationsEnable != 0 {
        available |= DYNAMIC_SAMPLE_LOCATIONS_ENABLE;
    }
    if native.provoking.provokingVertexLast != 0 {
        available |= PROVOKING_LAST;
    }
    if native.convention.depthClipControl != 0 {
        available |= DEPTH_NEGATIVE_ONE_TO_ONE;
    }
    if native
        .dynamic
        .extendedDynamicState3ConservativeRasterizationMode
        != 0
    {
        available |= DYNAMIC_CONSERVATIVE_MODE;
    }
    if native
        .dynamic
        .extendedDynamicState3ExtraPrimitiveOverestimationSize
        != 0
    {
        available |= DYNAMIC_EXTRA_OVERESTIMATION;
    }
    if native.dynamic.extendedDynamicState3ProvokingVertexMode != 0 {
        available |= DYNAMIC_PROVOKING_VERTEX;
    }
    if native.dynamic.extendedDynamicState3DepthClipEnable != 0 {
        available |= DYNAMIC_DEPTH_CLIP;
    }
    if native
        .dynamic
        .extendedDynamicState3DepthClipNegativeOneToOne
        != 0
    {
        available |= DYNAMIC_DEPTH_CONVENTION;
    }
    if native.dynamic.extendedDynamicState3LineRasterizationMode != 0 {
        available |= DYNAMIC_LINE_MODE;
    }
    if native.dynamic.extendedDynamicState3LineStippleEnable != 0 {
        available |= DYNAMIC_LINE_STIPPLE_ENABLE;
    }
    Ok((
        available,
        RasterMechanismLimits {
            primitive_overestimation: conservative.primitiveOverestimationSize,
            max_extra_overestimation: conservative.maxExtraPrimitiveOverestimationSize,
            extra_overestimation_granularity: conservative
                .extraPrimitiveOverestimationSizeGranularity,
            underestimation: conservative.primitiveUnderestimation,
            conservative_points_lines: conservative.conservativePointAndLineRasterization,
            degenerate_triangles: conservative.degenerateTrianglesRasterized,
            degenerate_lines: conservative.degenerateLinesRasterized,
            fully_covered_input: conservative.fullyCoveredFragmentShaderInputVariable,
            conservative_post_depth_coverage: conservative
                .conservativeRasterizationPostDepthCoverage,
            provoking_per_pipeline: provoking.provokingVertexModePerPipeline,
            line_subpixel_precision: v14.lineSubPixelPrecisionBits,
            strict_lines: core.strictLines,
        },
    ))
}
#[derive(Default)]
pub(super) struct States {
    pub clamp: Option<DepthClampControl>,
    pub bias: Option<DepthBiasRepresentation>,
    pub clip: Option<DepthClipState>,
    pub samples: Option<SampleLocationState>,
    pub conservative: Option<ConservativeState>,
    pub line: Option<LineState>,
    pub provoking: Option<ProvokingState>,
    pub convention: Option<DepthConventionState>,
}
pub(super) unsafe fn parse(mut p: *const Record) -> Result<States, Status> {
    let mut result = States::default();
    let mut count = 0;
    while !p.is_null() {
        count += 1;
        if count > 8 {
            return Err(INVALID);
        }
        let h = unsafe { &*p };
        macro_rules! take {
            ($field:ident,$ty:ty,$kind:ident) => {{
                if result.$field.is_some() {
                    return Err(INVALID);
                }
                let mut value = unsafe { *local_read::parent::<$ty>(p, $kind)? };
                value.header.next = ptr::null();
                result.$field = Some(value);
            }};
        }
        match h.kind {
            SAMPLE_LOCATION_STATE => take!(samples, SampleLocationState, SAMPLE_LOCATION_STATE),
            DEPTH_CLIP_STATE => take!(clip, DepthClipState, DEPTH_CLIP_STATE),
            DEPTH_CLAMP_CONTROL_STATE => take!(clamp, DepthClampControl, DEPTH_CLAMP_CONTROL_STATE),
            DEPTH_BIAS_REPRESENTATION => {
                take!(bias, DepthBiasRepresentation, DEPTH_BIAS_REPRESENTATION)
            }
            CONSERVATIVE_STATE => take!(conservative, ConservativeState, CONSERVATIVE_STATE),
            LINE_STATE => take!(line, LineState, LINE_STATE),
            PROVOKING_STATE => take!(provoking, ProvokingState, PROVOKING_STATE),
            DEPTH_CONVENTION_STATE => {
                take!(convention, DepthConventionState, DEPTH_CONVENTION_STATE)
            }
            _ => return Err(UNSUPPORTED),
        }
        p = h.next;
    }
    Ok(result)
}
pub(super) fn conservative(
    mode: u32,
    enabled: FeatureBits,
    p: &RasterMechanismLimits,
) -> Result<(), Status> {
    if mode > 2 {
        return Err(INVALID);
    }
    if enabled & CONSERVATIVE_RASTER == 0 || mode == 2 && p.underestimation == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn extra(value: f32, p: &RasterMechanismLimits) -> Result<(), Status> {
    if !value.is_finite() || value < 0.0 {
        return Err(INVALID);
    }
    if value > p.max_extra_overestimation {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn line_mode(mode: u32, enabled: FeatureBits) -> Result<(), Status> {
    let bit = match mode {
        0 => FeatureBits::ZERO,
        1 => RECTANGULAR_LINES,
        2 => BRESENHAM_LINES,
        3 => SMOOTH_LINES,
        _ => return Err(INVALID),
    };
    if bit & !enabled != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn line_stipple(
    mode: u32,
    enable: u32,
    enabled: FeatureBits,
    p: &RasterMechanismLimits,
) -> Result<(), Status> {
    if enable > 1 {
        return Err(INVALID);
    }
    if enable == 0 {
        return Ok(());
    }
    let bit = match mode {
        0 | 1 => STIPPLED_RECTANGULAR_LINES,
        2 => STIPPLED_BRESENHAM_LINES,
        3 => STIPPLED_SMOOTH_LINES,
        _ => return Err(INVALID),
    };
    if enabled & bit == 0 || mode == 0 && p.strict_lines == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn stipple(factor: u32, pattern: u32) -> Result<(), Status> {
    if !(1..=256).contains(&factor) || pattern > u16::MAX as u32 {
        Err(INVALID)
    } else {
        Ok(())
    }
}
pub(super) struct Commands {
    pub conservative_mode: vk::PFN_vkCmdSetConservativeRasterizationModeEXT,
    pub extra_overestimation: vk::PFN_vkCmdSetExtraPrimitiveOverestimationSizeEXT,
    pub provoking_vertex: vk::PFN_vkCmdSetProvokingVertexModeEXT,
    pub depth_clip: vk::PFN_vkCmdSetDepthClipEnableEXT,
    pub depth_convention: vk::PFN_vkCmdSetDepthClipNegativeOneToOneEXT,
    pub line_mode: vk::PFN_vkCmdSetLineRasterizationModeEXT,
    pub line_stipple_enable: vk::PFN_vkCmdSetLineStippleEnableEXT,
    pub line_stipple: vk::PFN_vkCmdSetLineStipple,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        macro_rules! load {
            ($name:literal,$ty:ty,$needed:expr) => {{
                if $needed {
                    let f = unsafe {
                        std::mem::transmute::<vk::PFN_vkVoidFunction, $ty>(instance.proc($name))
                    };
                    if f.is_none() {
                        return Err(UNSUPPORTED);
                    }
                    f
                } else {
                    None
                }
            }};
        }
        Ok(Self {
            conservative_mode: load!(
                c"vkCmdSetConservativeRasterizationModeEXT",
                vk::PFN_vkCmdSetConservativeRasterizationModeEXT,
                enabled & DYNAMIC_CONSERVATIVE_MODE != 0
            ),
            extra_overestimation: load!(
                c"vkCmdSetExtraPrimitiveOverestimationSizeEXT",
                vk::PFN_vkCmdSetExtraPrimitiveOverestimationSizeEXT,
                enabled & DYNAMIC_EXTRA_OVERESTIMATION != 0
            ),
            provoking_vertex: load!(
                c"vkCmdSetProvokingVertexModeEXT",
                vk::PFN_vkCmdSetProvokingVertexModeEXT,
                enabled & DYNAMIC_PROVOKING_VERTEX != 0
            ),
            depth_clip: load!(
                c"vkCmdSetDepthClipEnableEXT",
                vk::PFN_vkCmdSetDepthClipEnableEXT,
                enabled & DYNAMIC_DEPTH_CLIP != 0
            ),
            depth_convention: load!(
                c"vkCmdSetDepthClipNegativeOneToOneEXT",
                vk::PFN_vkCmdSetDepthClipNegativeOneToOneEXT,
                enabled & DYNAMIC_DEPTH_CONVENTION != 0
            ),
            line_mode: load!(
                c"vkCmdSetLineRasterizationModeEXT",
                vk::PFN_vkCmdSetLineRasterizationModeEXT,
                enabled & DYNAMIC_LINE_MODE != 0
            ),
            line_stipple_enable: load!(
                c"vkCmdSetLineStippleEnableEXT",
                vk::PFN_vkCmdSetLineStippleEnableEXT,
                enabled & DYNAMIC_LINE_STIPPLE_ENABLE != 0
            ),
            line_stipple: load!(
                c"vkCmdSetLineStipple",
                vk::PFN_vkCmdSetLineStipple,
                enabled & STIPPLED != 0
            ),
        })
    }
}
pub(super) struct Prepared {
    pub clamp: Option<DepthClampControl>,
    bias: Option<vk::VkDepthBiasRepresentationInfoEXT>,
    pub samples: Option<SampleLocationState>,
    clip: Option<vk::VkPipelineRasterizationDepthClipStateCreateInfoEXT>,
    conservative: Option<vk::VkPipelineRasterizationConservativeStateCreateInfoEXT>,
    line: Option<vk::VkPipelineRasterizationLineStateCreateInfo>,
    provoking: Option<vk::VkPipelineRasterizationProvokingVertexStateCreateInfoEXT>,
    pub convention: Option<vk::VkPipelineViewportDepthClipControlCreateInfoEXT>,
}
impl Prepared {
    pub fn raster_chain(&mut self) -> *const std::ffi::c_void {
        let mut next = ptr::null();
        macro_rules! link {
            ($field:ident) => {
                if let Some(v) = self.$field.as_mut() {
                    v.pNext = next;
                    next = ptr::from_ref(v).cast();
                }
            };
        }
        link!(bias);
        link!(clip);
        link!(conservative);
        link!(line);
        link!(provoking);
        next
    }
}
pub(super) unsafe fn prepare(
    header: *const Record,
    enabled: FeatureBits,
    p: &RasterMechanismLimits,
    dynamic: u64,
) -> Result<Prepared, Status> {
    let s = unsafe { parse(header)? };
    if dynamic & DYNAMIC_STATES.iter().fold(0, |mask, (bit, _)| mask | bit) & !dynamic_mask(enabled)
        != 0
    {
        return Err(UNSUPPORTED);
    }
    let bias = if dynamic & DYNAMIC_DEPTH_BIAS_STATE != 0 {
        None
    } else {
        s.bias
            .map(|b| raster_profiles::bias(b.representation, b.exact, enabled))
            .transpose()?
    };
    let mut result = Prepared {
        clamp: s.clamp,
        bias,
        samples: s.samples,
        clip: None,
        conservative: None,
        line: None,
        provoking: None,
        convention: None,
    };
    if s.clip.is_some() || dynamic & DYNAMIC_DEPTH_CLIP_STATE != 0 {
        let mut c = s.clip.unwrap_or(DepthClipState {
            header: Record::new::<DepthClipState>(DEPTH_CLIP_STATE),
            enable: 0,
            reserved: 0,
        });
        if dynamic & DYNAMIC_DEPTH_CLIP_STATE != 0 {
            c.enable = 0;
        }
        result.clip = unsafe { raster_controls::depth_clip(&c.header, enabled)? };
    }
    if s.conservative.is_some()
        || dynamic & (DYNAMIC_CONSERVATIVE_MODE_STATE | DYNAMIC_EXTRA_OVERESTIMATION_STATE) != 0
    {
        let mut c = s.conservative.unwrap_or(ConservativeState {
            header: Record::new::<ConservativeState>(CONSERVATIVE_STATE),
            mode: 0,
            extra_overestimation: 0.0,
        });
        if dynamic & DYNAMIC_CONSERVATIVE_MODE_STATE != 0 {
            c.mode = 0;
        }
        if dynamic & DYNAMIC_EXTRA_OVERESTIMATION_STATE != 0 {
            c.extra_overestimation = 0.0;
        }
        conservative(c.mode, enabled, p)?;
        extra(c.extra_overestimation, p)?;
        result.conservative = Some(vk::VkPipelineRasterizationConservativeStateCreateInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_CONSERVATIVE_STATE_CREATE_INFO_EXT,
            conservativeRasterizationMode: c.mode,
            extraPrimitiveOverestimationSize: c.extra_overestimation,
            ..Default::default()
        });
    }
    if s.line.is_some()
        || dynamic
            & (DYNAMIC_LINE_MODE_STATE
                | DYNAMIC_LINE_STIPPLE_ENABLE_STATE
                | DYNAMIC_LINE_STIPPLE_STATE)
            != 0
    {
        let mut c = s.line.unwrap_or(LineState {
            header: Record::new::<LineState>(LINE_STATE),
            mode: 0,
            stipple_enable: 0,
            factor: 1,
            pattern: 65535,
        });
        if dynamic & DYNAMIC_LINE_MODE_STATE != 0 {
            c.mode = 0;
        }
        if dynamic & DYNAMIC_LINE_STIPPLE_ENABLE_STATE != 0 {
            c.stipple_enable = 0;
        }
        if dynamic & DYNAMIC_LINE_STIPPLE_STATE != 0 {
            c.factor = 1;
            c.pattern = 65535;
        }
        line_mode(c.mode, enabled)?;
        stipple(c.factor, c.pattern)?;
        if dynamic & DYNAMIC_LINE_MODE_STATE == 0 {
            line_stipple(c.mode, c.stipple_enable, enabled, p)?;
        } else if c.stipple_enable > 1 {
            return Err(INVALID);
        } else if c.stipple_enable != 0 && enabled & STIPPLED == 0 {
            return Err(UNSUPPORTED);
        }
        result.line = Some(vk::VkPipelineRasterizationLineStateCreateInfo {
            sType:
                vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_LINE_STATE_CREATE_INFO,
            lineRasterizationMode: c.mode,
            stippledLineEnable: c.stipple_enable,
            lineStippleFactor: c.factor,
            lineStipplePattern: c.pattern as u16,
            ..Default::default()
        });
    }
    if s.provoking.is_some() || dynamic & DYNAMIC_PROVOKING_VERTEX_STATE != 0 {
        let mut c = s.provoking.unwrap_or(ProvokingState {
            header: Record::new::<ProvokingState>(PROVOKING_STATE),
            mode: 0,
            reserved: 0,
        });
        if dynamic & DYNAMIC_PROVOKING_VERTEX_STATE != 0 {
            c.mode = 0;
        }
        if c.mode > 1 || c.reserved != 0 {
            return Err(INVALID);
        }
        if enabled & PROVOKING_LAST == 0 {
            return Err(UNSUPPORTED);
        }
        result.provoking = Some(vk::VkPipelineRasterizationProvokingVertexStateCreateInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_PROVOKING_VERTEX_STATE_CREATE_INFO_EXT,
            provokingVertexMode: c.mode,
            ..Default::default()
        });
    }
    if s.convention.is_some() || dynamic & DYNAMIC_DEPTH_CONVENTION_STATE != 0 {
        let mut c = s.convention.unwrap_or(DepthConventionState {
            header: Record::new::<DepthConventionState>(DEPTH_CONVENTION_STATE),
            negative_one_to_one: 0,
            reserved: 0,
        });
        if dynamic & DYNAMIC_DEPTH_CONVENTION_STATE != 0 {
            c.negative_one_to_one = 0;
        }
        if c.negative_one_to_one > 1 || c.reserved != 0 {
            return Err(INVALID);
        }
        if enabled & DEPTH_NEGATIVE_ONE_TO_ONE == 0 {
            return Err(UNSUPPORTED);
        }
        result.convention = Some(vk::VkPipelineViewportDepthClipControlCreateInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_DEPTH_CLIP_CONTROL_CREATE_INFO_EXT,
            negativeOneToOne: c.negative_one_to_one,
            ..Default::default()
        });
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chain_rejects_duplicates_unknown_and_cycles_without_unbounded_walks() {
        let mut c = ConservativeState {
            header: Record::new::<ConservativeState>(CONSERVATIVE_STATE),
            mode: 1,
            extra_overestimation: 0.0,
        };
        let mut l = LineState {
            header: Record::new::<LineState>(LINE_STATE),
            mode: 1,
            stipple_enable: 0,
            factor: 1,
            pattern: 65535,
        };
        let mut p = ProvokingState {
            header: Record::new::<ProvokingState>(PROVOKING_STATE),
            mode: 1,
            reserved: 0,
        };
        let mut d = DepthConventionState {
            header: Record::new::<DepthConventionState>(DEPTH_CONVENTION_STATE),
            negative_one_to_one: 1,
            reserved: 0,
        };
        let clip = DepthClipState {
            header: Record::new::<DepthClipState>(DEPTH_CLIP_STATE),
            enable: 1,
            reserved: 0,
        };
        d.header.next = &clip.header;
        p.header.next = &d.header;
        l.header.next = &p.header;
        c.header.next = &l.header;
        let states = unsafe { parse(&c.header) }.unwrap();
        assert!(
            states.conservative.is_some() && states.clip.is_some() && states.convention.is_some()
        );
        d.header.next = &c.header;
        assert!(matches!(unsafe { parse(&c.header) }, Err(INVALID)));
        let unknown = Record::new::<Record>(u32::MAX);
        assert!(matches!(unsafe { parse(&unknown) }, Err(UNSUPPORTED)));
        assert!(!d.header.next.is_null());
    }
    #[test]
    fn independently_selected_dynamic_fields_ignore_only_their_static_values() {
        let p = RasterMechanismLimits {
            max_extra_overestimation: 0.5,
            ..Default::default()
        };
        let c = ConservativeState {
            header: Record::new::<ConservativeState>(CONSERVATIVE_STATE),
            mode: u32::MAX,
            extra_overestimation: f32::NAN,
        };
        let features =
            CONSERVATIVE_RASTER | DYNAMIC_CONSERVATIVE_MODE | DYNAMIC_EXTRA_OVERESTIMATION;
        let mask = DYNAMIC_CONSERVATIVE_MODE_STATE | DYNAMIC_EXTRA_OVERESTIMATION_STATE;
        assert!(unsafe { prepare(&c.header, features, &p, mask) }.is_ok());
        assert!(
            unsafe { prepare(&c.header, features, &p, DYNAMIC_CONSERVATIVE_MODE_STATE) }.is_err()
        );
        assert!(matches!(
            unsafe { prepare(&c.header, CONSERVATIVE_RASTER, &p, mask) },
            Err(UNSUPPORTED)
        ));
        let mut provoking = ProvokingState {
            header: Record::new::<ProvokingState>(PROVOKING_STATE),
            mode: u32::MAX,
            reserved: 1,
        };
        let enabled = PROVOKING_LAST | DYNAMIC_PROVOKING_VERTEX;
        assert!(matches!(
            unsafe {
                prepare(
                    &provoking.header,
                    enabled,
                    &p,
                    DYNAMIC_PROVOKING_VERTEX_STATE,
                )
            },
            Err(INVALID)
        ));
        provoking.reserved = 0;
        assert!(unsafe {
            prepare(
                &provoking.header,
                enabled,
                &p,
                DYNAMIC_PROVOKING_VERTEX_STATE,
            )
        }
        .is_ok());
        assert_eq!(extra(0.6, &p), Err(UNSUPPORTED));
        assert_eq!(extra(f32::NAN, &p), Err(INVALID));
        assert_eq!(conservative(2, CONSERVATIVE_RASTER, &p), Err(UNSUPPORTED));
        assert_eq!(
            validate_feature_dependencies(DYNAMIC_CONSERVATIVE_MODE),
            Err(INVALID)
        );
        assert_eq!(
            validate_feature_dependencies(DYNAMIC_CONSERVATIVE_MODE | CONSERVATIVE_RASTER),
            Ok(())
        );
    }
    #[test]
    fn line_modes_and_stipple_are_independent_with_native_default_constraints() {
        let p = RasterMechanismLimits::default();
        assert_eq!(line_mode(2, BRESENHAM_LINES), Ok(()));
        assert_eq!(line_mode(1, BRESENHAM_LINES), Err(UNSUPPORTED));
        assert_eq!(line_stipple(2, 1, BRESENHAM_LINES, &p), Err(UNSUPPORTED));
        assert_eq!(line_stipple(2, 1, STIPPLED_BRESENHAM_LINES, &p), Ok(()));
        assert_eq!(
            line_stipple(0, 1, STIPPLED_RECTANGULAR_LINES, &p),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            line_stipple(
                0,
                1,
                STIPPLED_RECTANGULAR_LINES,
                &RasterMechanismLimits {
                    strict_lines: 1,
                    ..p
                }
            ),
            Ok(())
        );
        assert_eq!(stipple(0, 0), Err(INVALID));
        assert_eq!(stipple(256, 65535), Ok(()));
        assert_eq!(stipple(1, 65536), Err(INVALID));
    }
}
