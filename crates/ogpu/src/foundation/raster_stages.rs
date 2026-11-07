//! Native pre-raster stages and cold extension parsing. No generated geometry,
//! patch expansion, shader reflection or extra draw path.
use super::*;
pub(super) const TESS_STAGES: u64 = TESS_CONTROL_STAGE | TESS_EVALUATION_STAGE;
const TRADITIONAL: u64 = 8 | 16 | TESS_STAGES | GEOMETRY_STAGE;
pub(super) fn traditional(stages: u64) -> bool {
    stages & 8 != 0 && stages & !TRADITIONAL == 0 && matches!(stages & TESS_STAGES, 0 | TESS_STAGES)
}
pub(super) fn enables(enabled: FeatureBits, stages: u64, views: u32) -> Result<(), Status> {
    if stages & TESS_STAGES != 0 && enabled & TESSELLATION == 0
        || stages & GEOMETRY_STAGE != 0 && enabled & GEOMETRY == 0
        || views != 0 && stages & TESS_STAGES != 0 && enabled & TESSELLATION_MULTIVIEW == 0
        || views != 0 && stages & GEOMETRY_STAGE != 0 && enabled & GEOMETRY_MULTIVIEW == 0
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
#[derive(Default)]
pub(super) struct Extensions {
    pub mapping: Option<RenderMapping>,
    pub tessellation: Option<TessellationState>,
    pub viewports: Option<ViewportCounts>,
    pub color_write: Option<ColorWriteState>,
    pub stream: Option<RasterizationStream>,
}
/// At most one of each known extension, in either order. Parent validation
/// preserves size/version/flags rules while parsing each next pointer here.
pub(super) unsafe fn extensions(mut p: *const Record) -> Result<Extensions, Status> {
    let mut result = Extensions::default();
    let mut count = 0;
    while !p.is_null() {
        count += 1;
        if count > 5 {
            return Err(INVALID);
        }
        let header = unsafe { &*p };
        match header.kind {
            RASTERIZATION_STREAM_STATE => {
                if result.stream.is_some() {
                    return Err(INVALID);
                }
                result.stream = Some(unsafe {
                    *local_read::parent::<RasterizationStream>(p, RASTERIZATION_STREAM_STATE)?
                });
            }
            RENDER_MAPPING => {
                if result.mapping.is_some() {
                    return Err(INVALID);
                }
                let mut mapping =
                    unsafe { *local_read::parent::<RenderMapping>(p, RENDER_MAPPING)? };
                mapping.header.next = ptr::null();
                result.mapping = Some(mapping);
            }
            COLOR_WRITE_STATE => {
                if result.color_write.is_some() {
                    return Err(INVALID);
                }
                result.color_write =
                    Some(unsafe { *local_read::parent::<ColorWriteState>(p, COLOR_WRITE_STATE)? });
            }
            VIEWPORT_COUNTS => {
                if result.viewports.is_some() {
                    return Err(INVALID);
                }
                result.viewports =
                    Some(unsafe { *local_read::parent::<ViewportCounts>(p, VIEWPORT_COUNTS)? });
            }
            TESSELLATION_STATE => {
                if result.tessellation.is_some() {
                    return Err(INVALID);
                }
                result.tessellation = Some(unsafe {
                    *local_read::parent::<TessellationState>(p, TESSELLATION_STATE)?
                });
            }
            _ => return Err(UNSUPPORTED),
        }
        p = header.next;
    }
    Ok(result)
}
pub(super) fn dynamic_tessellation(
    mut state: Option<TessellationState>,
    dynamic: u64,
    enabled: FeatureBits,
) -> Result<Option<TessellationState>, Status> {
    let mask = DYNAMIC_PATCH_CONTROL_POINTS_STATE | DYNAMIC_TESSELLATION_ORIGIN_STATE;
    if dynamic & mask & !extended_raster::dynamic_mask(enabled) != 0 {
        return Err(UNSUPPORTED);
    }
    if let Some(s) = &mut state {
        if dynamic & DYNAMIC_PATCH_CONTROL_POINTS_STATE != 0 {
            s.patch_control_points = 1;
        }
        if dynamic & DYNAMIC_TESSELLATION_ORIGIN_STATE != 0 {
            s.domain_origin = 0;
        }
    }
    Ok(state)
}
pub(super) fn tessellation(
    stages: u64,
    topology: u32,
    state: Option<TessellationState>,
    limits: &RasterStageLimits,
) -> Result<Option<TessellationState>, Status> {
    let has_tess = stages & TESS_STAGES != 0;
    if has_tess != (topology == PATCHES) || has_tess != state.is_some() {
        return Err(INVALID);
    }
    if let Some(s) = state {
        if s.patch_control_points == 0 || s.domain_origin > 1 {
            return Err(INVALID);
        }
        if s.patch_control_points > limits.max_patch_size {
            return Err(UNSUPPORTED);
        }
    }
    Ok(state)
}
pub(super) fn limits(p: &vk::VkPhysicalDeviceLimits) -> RasterStageLimits {
    RasterStageLimits {
        max_generation_level: p.maxTessellationGenerationLevel,
        max_patch_size: p.maxTessellationPatchSize,
        control_input_components: p.maxTessellationControlPerVertexInputComponents,
        control_output_components: p.maxTessellationControlPerVertexOutputComponents,
        patch_output_components: p.maxTessellationControlPerPatchOutputComponents,
        control_total_output_components: p.maxTessellationControlTotalOutputComponents,
        evaluation_input_components: p.maxTessellationEvaluationInputComponents,
        evaluation_output_components: p.maxTessellationEvaluationOutputComponents,
        geometry_invocations: p.maxGeometryShaderInvocations,
        geometry_input_components: p.maxGeometryInputComponents,
        geometry_output_components: p.maxGeometryOutputComponents,
        geometry_output_vertices: p.maxGeometryOutputVertices,
        geometry_total_output_components: p.maxGeometryTotalOutputComponents,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stage_sets_preserve_pairing_and_independent_enables() {
        for stages in [
            8,
            24,
            8 | GEOMETRY_STAGE,
            24 | GEOMETRY_STAGE,
            8 | TESS_STAGES,
            24 | TESS_STAGES,
            24 | TESS_STAGES | GEOMETRY_STAGE,
        ] {
            assert!(traditional(stages));
        }
        for stages in [
            0,
            16,
            4,
            8 | TESS_CONTROL_STAGE,
            8 | TESS_EVALUATION_STAGE,
            8 | MESH_STAGE,
        ] {
            assert!(!traditional(stages));
        }
        assert_eq!(enables(GEOMETRY, 8 | GEOMETRY_STAGE, 0), Ok(()));
        assert_eq!(enables(TESSELLATION, 8 | TESS_STAGES, 0), Ok(()));
        assert_eq!(enables(GEOMETRY, 8 | TESS_STAGES, 0), Err(UNSUPPORTED));
        assert_eq!(
            enables(TESSELLATION, 8 | GEOMETRY_STAGE, 0),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            enables(GEOMETRY | MULTIVIEW, 8 | GEOMETRY_STAGE, 1),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            enables(
                GEOMETRY | MULTIVIEW | GEOMETRY_MULTIVIEW,
                8 | GEOMETRY_STAGE,
                1
            ),
            Ok(())
        );
    }
    #[test]
    fn dynamic_patch_fields_ignore_only_selected_static_values() {
        let s = TessellationState {
            header: Record::new::<TessellationState>(TESSELLATION_STATE),
            patch_control_points: 0,
            domain_origin: 99,
        };
        let both = DYNAMIC_PATCH_CONTROL_POINTS_STATE | DYNAMIC_TESSELLATION_ORIGIN_STATE;
        let enabled = DYNAMIC_PATCH_CONTROL_POINTS | DYNAMIC_TESSELLATION_ORIGIN;
        assert!(dynamic_tessellation(Some(s), both, DYNAMIC_PATCH_CONTROL_POINTS).is_err());
        let p = RasterStageLimits {
            max_patch_size: 32,
            ..Default::default()
        };
        let state = dynamic_tessellation(Some(s), both, enabled).unwrap();
        assert!(tessellation(8 | TESS_STAGES, PATCHES, state, &p).is_ok());
        let state =
            dynamic_tessellation(Some(s), DYNAMIC_PATCH_CONTROL_POINTS_STATE, enabled).unwrap();
        assert!(tessellation(8 | TESS_STAGES, PATCHES, state, &p).is_err());
        assert_eq!(
            extended_raster::scalar(DYNAMIC_PATCH_CONTROL_POINTS_STATE, 0, enabled),
            Err(INVALID)
        );
        assert_eq!(
            extended_raster::scalar(DYNAMIC_TESSELLATION_ORIGIN_STATE, 2, enabled),
            Err(INVALID)
        );
    }
    #[test]
    fn patch_state_is_explicit_and_bounded() {
        let p = RasterStageLimits {
            max_patch_size: 32,
            ..Default::default()
        };
        let s = TessellationState {
            header: Record::new::<TessellationState>(TESSELLATION_STATE),
            patch_control_points: 3,
            domain_origin: 0,
        };
        assert!(tessellation(8 | TESS_STAGES, PATCHES, Some(s), &p).is_ok());
        for (stages, topology, state) in [
            (8, PATCHES, Some(s)),
            (8 | TESS_STAGES, 3, Some(s)),
            (8 | TESS_STAGES, PATCHES, None),
            (8, 3, Some(s)),
        ] {
            assert!(matches!(
                tessellation(stages, topology, state, &p),
                Err(INVALID)
            ));
        }
        assert!(matches!(
            tessellation(
                8 | TESS_STAGES,
                PATCHES,
                Some(TessellationState {
                    patch_control_points: 33,
                    ..s
                }),
                &p
            ),
            Err(UNSUPPORTED)
        ));
    }
    #[test]
    fn extension_chain_accepts_both_orders_rejects_duplicates_cycles_and_unknown() {
        let mut t = TessellationState {
            header: Record::new::<TessellationState>(TESSELLATION_STATE),
            patch_control_points: 3,
            domain_origin: 1,
        };
        let mut m = RenderMapping {
            header: Record::new::<RenderMapping>(RENDER_MAPPING),
            locations: ptr::null(),
            inputs: ptr::null(),
            binding_count: 0,
            reserved: 0,
            bindings: ptr::null(),
        };
        t.header.next = &m.header;
        assert!(unsafe { extensions(&t.header) }
            .unwrap()
            .mapping
            .unwrap()
            .header
            .next
            .is_null());
        t.header.next = ptr::null();
        m.header.next = &t.header;
        assert!(unsafe { extensions(&m.header) }
            .unwrap()
            .tessellation
            .is_some());
        t.header.next = &m.header;
        assert!(!t.header.next.is_null());
        assert!(matches!(unsafe { extensions(&m.header) }, Err(INVALID)));
        let unknown = Record::new::<Record>(u32::MAX);
        assert!(matches!(unsafe { extensions(&unknown) }, Err(UNSUPPORTED)));
    }
    #[test]
    fn multiview_enables_do_not_imply_their_prerequisites() {
        for (sub, base) in [
            (TESSELLATION_MULTIVIEW, TESSELLATION),
            (GEOMETRY_MULTIVIEW, GEOMETRY),
        ] {
            assert_eq!(validate_feature_dependencies(sub), Err(INVALID));
            assert_eq!(validate_feature_dependencies(sub | base), Err(INVALID));
            assert_eq!(
                validate_feature_dependencies(sub | base | MULTIVIEW | RASTER),
                Ok(())
            );
        }
    }
    #[test]
    #[ignore = "requires modern Vulkan; independent tessellation/geometry feature enables and argument visibility"]
    fn gpu_raster_stage_feature_contract() {
        let instance = Arc::new(Instance::new().unwrap());
        let mut checked = false;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = super::super::snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let Some(q) = snapshot
                .queues
                .iter()
                .find(|q| q.flags & GRAPHICS != 0 && q.count != 0)
            else {
                continue;
            };
            let request = [QueueRequest {
                domain: q.domain,
                count: 1,
                priority: 0.5,
            }];
            eprintln!("Sample locations: {:?}", snapshot.sample_location_limits);
            let available = snapshot.features.available;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            for bit in [
                0,
                UNRESTRICTED_DEPTH_RANGE,
                DEPTH_CLAMP_CONTROL,
                DEPTH_BIAS_CONTROL,
                DEPTH_BIAS_CONTROL | DEPTH_BIAS_FLOAT,
                DEPTH_BIAS_CONTROL | DEPTH_BIAS_UNORM,
                DEPTH_BIAS_CONTROL | DEPTH_BIAS_EXACT,
                TRANSFORM_FEEDBACK,
                GEOMETRY | GEOMETRY_STREAMS,
                DYNAMIC_RASTERIZATION_STREAM,
                COLOR_WRITE_ENABLE,
                DYNAMIC_PATCH_CONTROL_POINTS,
                DYNAMIC_TESSELLATION_ORIGIN,
                MULTI_VIEWPORT,
                SHADER_OUTPUT_VIEWPORT_INDEX,
                MULTI_VIEWPORT | SHADER_OUTPUT_VIEWPORT_INDEX,
                DYNAMIC_DEPTH_CLAMP,
                DYNAMIC_POLYGON_MODE,
                DYNAMIC_RASTERIZATION_SAMPLES,
                DYNAMIC_ALPHA_TO_COVERAGE,
                DYNAMIC_ALPHA_TO_ONE,
                DYNAMIC_LOGIC_OP_ENABLE,
                DYNAMIC_LOGIC_OP,
                DYNAMIC_SAMPLE_MASK,
                DYNAMIC_COLOR_BLEND_ENABLE,
                DYNAMIC_COLOR_BLEND_EQUATION,
                DYNAMIC_COLOR_WRITE_MASK,
                DYNAMIC_VERTEX_INPUT,
                DYNAMIC_VERTEX_INPUT | VERTEX_DIVISOR,
                DYNAMIC_VERTEX_INPUT | VERTEX_ZERO_DIVISOR,
                SAMPLE_LOCATIONS,
                SAMPLE_LOCATIONS | DYNAMIC_SAMPLE_LOCATIONS_ENABLE,
                CONSERVATIVE_RASTER,
                PROVOKING_LAST,
                DEPTH_NEGATIVE_ONE_TO_ONE,
                RECTANGULAR_LINES,
                BRESENHAM_LINES,
                SMOOTH_LINES,
                STIPPLED_RECTANGULAR_LINES,
                STIPPLED_BRESENHAM_LINES,
                STIPPLED_SMOOTH_LINES,
                DYNAMIC_CONSERVATIVE_MODE | CONSERVATIVE_RASTER,
                DYNAMIC_EXTRA_OVERESTIMATION | CONSERVATIVE_RASTER,
                DYNAMIC_PROVOKING_VERTEX | PROVOKING_LAST,
                DYNAMIC_DEPTH_CLIP | DEPTH_CLIP,
                DYNAMIC_DEPTH_CONVENTION | DEPTH_NEGATIVE_ONE_TO_ONE,
                DYNAMIC_LINE_MODE,
                DYNAMIC_LINE_STIPPLE_ENABLE,
                LARGE_POINTS,
                DEPTH_CLIP,
                VERTEX_DIVISOR,
                VERTEX_ZERO_DIVISOR,
                LIST_RESTART,
                PATCH_RESTART,
                TESSELLATION,
                GEOMETRY,
                TESSELLATION_GEOMETRY_POINT_SIZE,
                TESSELLATION | MULTIVIEW | TESSELLATION_MULTIVIEW,
                GEOMETRY | MULTIVIEW | GEOMETRY_MULTIVIEW,
            ] {
                let required = RASTER | bit;
                let result = Device::create(&adapter, &request, required);
                if required & !available != 0 {
                    assert!(matches!(result, Err(UNSUPPORTED)));
                    continue;
                }
                let d = result.unwrap();
                assert_eq!(d.snapshot.features.enabled, required);
                if bit == 0
                    || bit
                        & (extended_raster::EDS3_FEATURES
                            | DYNAMIC_LOGIC_OP
                            | MULTI_VIEWPORT
                            | SHADER_OUTPUT_VIEWPORT_INDEX
                            | UNRESTRICTED_DEPTH_RANGE
                            | DEPTH_CLAMP_CONTROL
                            | raster_profiles::BIAS_FEATURES
                            | COLOR_WRITE_ENABLE
                            | TRANSFORM_FEEDBACK
                            | DYNAMIC_RASTERIZATION_STREAM
                            | DYNAMIC_PATCH_CONTROL_POINTS
                            | DYNAMIC_TESSELLATION_ORIGIN)
                        != 0
                {
                    let ad = ArenaDesc {
                        header: Record::new::<ArenaDesc>(ARENA_DESC),
                        domain: request[0].domain,
                        list_capacity: 1,
                        level: 0,
                        reserved: 0,
                    };
                    let mut arena = Arena::create(&d, &ad).unwrap();
                    let rd = RecordingDesc {
                        header: Record::new::<RecordingDesc>(RECORDING_DESC),
                        replay_mode: 1,
                        level: 0,
                        inheritance: ptr::null(),
                    };
                    let list = unsafe { &*arena.begin(&rd).unwrap() };
                    assert_eq!(list.line_width(1.0), Ok(()));
                    assert_eq!(list.line_width(2.0), Err(UNSUPPORTED));
                    assert_eq!(list.depth_bias(-1.0, 0.0, 2.0), Ok(()));
                    assert_eq!(list.depth_bias(0.0, 1.0, 0.0), Err(UNSUPPORTED));
                    assert_eq!(list.depth_bounds(0.2, 0.8), Ok(()));
                    assert_eq!(
                        list.dynamic_raster_value(DYNAMIC_DEPTH_BOUNDS_ENABLE_STATE, 0),
                        Ok(())
                    );
                    assert_eq!(
                        list.dynamic_raster_value(DYNAMIC_DEPTH_BOUNDS_ENABLE_STATE, 1),
                        Err(UNSUPPORTED)
                    );
                    assert_eq!(list.dynamic_raster_value(DYNAMIC_TOPOLOGY_STATE, 4), Ok(()));
                    assert_eq!(
                        list.dynamic_raster_value(DYNAMIC_TOPOLOGY_STATE, PATCHES),
                        Err(UNSUPPORTED)
                    );
                    assert_eq!(
                        list.dynamic_raster_value(DYNAMIC_TOPOLOGY_STATE, 6),
                        Err(UNSUPPORTED)
                    );
                    assert_eq!(
                        unsafe {
                            list.bind_capture(
                                0,
                                0,
                                ptr::null(),
                                HostSpan {
                                    data: ptr::null_mut(),
                                    size: 0,
                                },
                            )
                        },
                        if required & TRANSFORM_FEEDBACK != 0 {
                            Ok(())
                        } else {
                            Err(UNSUPPORTED)
                        }
                    );
                    for (state, feature, value) in [
                        (
                            DYNAMIC_RASTERIZATION_STREAM_STATE,
                            DYNAMIC_RASTERIZATION_STREAM,
                            0,
                        ),
                        (
                            DYNAMIC_PATCH_CONTROL_POINTS_STATE,
                            DYNAMIC_PATCH_CONTROL_POINTS,
                            3,
                        ),
                        (
                            DYNAMIC_TESSELLATION_ORIGIN_STATE,
                            DYNAMIC_TESSELLATION_ORIGIN,
                            1,
                        ),
                        (DYNAMIC_DEPTH_CLAMP_STATE, DYNAMIC_DEPTH_CLAMP, 0),
                        (DYNAMIC_POLYGON_MODE_STATE, DYNAMIC_POLYGON_MODE, 0),
                        (
                            DYNAMIC_RASTERIZATION_SAMPLES_STATE,
                            DYNAMIC_RASTERIZATION_SAMPLES,
                            4,
                        ),
                        (
                            DYNAMIC_ALPHA_TO_COVERAGE_STATE,
                            DYNAMIC_ALPHA_TO_COVERAGE,
                            1,
                        ),
                        (DYNAMIC_ALPHA_TO_ONE_STATE, DYNAMIC_ALPHA_TO_ONE, 0),
                        (DYNAMIC_LOGIC_OP_ENABLE_STATE, DYNAMIC_LOGIC_OP_ENABLE, 0),
                        (DYNAMIC_LOGIC_OP_STATE, DYNAMIC_LOGIC_OP, 6),
                    ] {
                        assert_eq!(
                            list.extended_raster_value(state, value),
                            if bit & feature == 0 {
                                Err(UNSUPPORTED)
                            } else {
                                Ok(())
                            }
                        );
                    }
                    for (state, feature) in [
                        (DYNAMIC_DEPTH_CLAMP_STATE, DYNAMIC_DEPTH_CLAMP),
                        (DYNAMIC_POLYGON_MODE_STATE, DYNAMIC_POLYGON_MODE),
                        (DYNAMIC_ALPHA_TO_ONE_STATE, DYNAMIC_ALPHA_TO_ONE),
                        (DYNAMIC_LOGIC_OP_ENABLE_STATE, DYNAMIC_LOGIC_OP_ENABLE),
                    ] {
                        if bit & feature != 0 {
                            assert_eq!(list.extended_raster_value(state, 1), Err(UNSUPPORTED));
                        }
                    }
                    assert_eq!(
                        list.sample_mask(64, 1 << 63),
                        if bit & DYNAMIC_SAMPLE_MASK == 0 {
                            Err(UNSUPPORTED)
                        } else {
                            Ok(())
                        }
                    );
                    for (state, feature) in [
                        (DYNAMIC_COLOR_BLEND_ENABLE_STATE, DYNAMIC_COLOR_BLEND_ENABLE),
                        (DYNAMIC_COLOR_WRITE_MASK_STATE, DYNAMIC_COLOR_WRITE_MASK),
                    ] {
                        assert_eq!(
                            unsafe { list.color_values(state, 0, 0, ptr::null()) },
                            if bit & feature == 0 {
                                Err(UNSUPPORTED)
                            } else {
                                Ok(())
                            }
                        );
                        if bit & feature != 0 {
                            assert_eq!(unsafe { list.color_values(state, 0, 1, &0) }, Ok(()));
                            assert_eq!(
                                unsafe { list.color_values(state, 0, 1, &99) },
                                Err(INVALID)
                            );
                        }
                    }
                    let e = BlendEquation::default();
                    assert_eq!(
                        unsafe { list.color_equations(0, 0, ptr::null()) },
                        if bit & DYNAMIC_COLOR_BLEND_EQUATION == 0 {
                            Err(UNSUPPORTED)
                        } else {
                            Ok(())
                        }
                    );
                    if bit & DYNAMIC_COLOR_BLEND_EQUATION != 0 {
                        assert_eq!(unsafe { list.color_equations(0, 1, &e) }, Ok(()));
                        assert_eq!(
                            unsafe {
                                list.color_equations(0, 1, &BlendEquation { src_color: 15, ..e })
                            },
                            Err(UNSUPPORTED)
                        );
                    }
                    let v = Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: 8.0,
                        height: 8.0,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    };
                    let sc = Scissor {
                        x: 0,
                        y: 0,
                        width: 8,
                        height: 8,
                    };
                    assert_eq!(unsafe { list.viewports(0, 1, &v, SET_COUNT) }, Ok(()));
                    assert_eq!(unsafe { list.scissors(0, 1, &sc, SET_COUNT) }, Ok(()));
                    assert_eq!(
                        unsafe { list.viewports(1, 1, &v, 0) },
                        if bit & MULTI_VIEWPORT != 0 {
                            Ok(())
                        } else {
                            Err(UNSUPPORTED)
                        }
                    );
                    assert_eq!(
                        unsafe { list.scissors(1, 1, &sc, 0) },
                        if bit & MULTI_VIEWPORT != 0 {
                            Ok(())
                        } else {
                            Err(UNSUPPORTED)
                        }
                    );
                    assert_eq!(
                        list.depth_clamp_range(0, f32::NAN, f32::NAN),
                        if bit & DEPTH_CLAMP_CONTROL != 0 {
                            Ok(())
                        } else {
                            Err(UNSUPPORTED)
                        }
                    );
                    assert_eq!(
                        list.depth_bias_control(1.0, 0.0, 0.0, 0, 0),
                        if bit & DEPTH_BIAS_CONTROL != 0 {
                            Ok(())
                        } else {
                            Err(UNSUPPORTED)
                        }
                    );
                    assert_eq!(
                        unsafe { list.color_write_enable(0, ptr::null()) },
                        if bit & COLOR_WRITE_ENABLE != 0 {
                            Ok(())
                        } else {
                            Err(UNSUPPORTED)
                        }
                    );
                    if bit & COLOR_WRITE_ENABLE != 0 {
                        assert_eq!(
                            unsafe { list.color_write_enable(2, [0, 1].as_ptr()) },
                            Ok(())
                        );
                    }
                    assert_eq!(
                        list.depth_bounds(-1.0, 2.0),
                        if bit & UNRESTRICTED_DEPTH_RANGE != 0 {
                            Ok(())
                        } else {
                            Err(INVALID)
                        }
                    );
                    let wide = Viewport {
                        min_depth: -1.0,
                        max_depth: 2.0,
                        ..v
                    };
                    assert_eq!(
                        unsafe { list.viewports(0, 1, &wide, SET_COUNT) },
                        if bit & UNRESTRICTED_DEPTH_RANGE != 0 {
                            Ok(())
                        } else {
                            Err(INVALID)
                        }
                    );
                    list.end().unwrap();
                }

                assert_eq!(
                    d.vertex_input_command.is_some(),
                    required & DYNAMIC_VERTEX_INPUT != 0
                );
                assert_eq!(
                    d.snapshot.raster_stage_limits,
                    adapter.snapshot.raster_stage_limits
                );
                for (stage, feature) in [
                    (TESS_CONTROL_STAGE, TESSELLATION),
                    (TESS_EVALUATION_STAGE, TESSELLATION),
                    (GEOMETRY_STAGE, GEOMETRY),
                ] {
                    let field = ArgumentField {
                        offset: 0,
                        size: 8,
                        kind: ARGUMENT_INLINE,
                        address_alignment: 0,
                        stages: stage,
                        space: ARGUMENT_DIRECT,
                        binding: ARGUMENT_DIRECT,
                    };
                    let desc = ArgumentLayoutDesc {
                        execution_class: ARGUMENT_GRAPHICS,
                        byte_size: 8,
                        field_count: 1,
                        flags: 0,
                        fields: &field,
                    };
                    let result = unsafe { ArgumentLayout::create(&d, &desc) };
                    if required & feature == 0 {
                        assert!(matches!(result, Err(UNSUPPORTED)));
                    } else {
                        assert!(result.is_ok());
                    }
                }
            }
            checked = true;
        }
        assert!(checked, "no compatible physical device");
    }
}
