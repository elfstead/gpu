//! Native fragment shading rate. No rate selection, view loop or image synthesis.
use super::*;

pub(super) const INPUTS: FeatureBits = FeatureBits::union(&[
    PIPELINE_SHADING_RATE,
    PRIMITIVE_SHADING_RATE,
    ATTACHMENT_SHADING_RATE,
]);
pub(super) fn requested(
    enabled: FeatureBits,
) -> vk::VkPhysicalDeviceFragmentShadingRateFeaturesKHR {
    vk::VkPhysicalDeviceFragmentShadingRateFeaturesKHR {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FRAGMENT_SHADING_RATE_FEATURES_KHR,
        pipelineFragmentShadingRate: u32::from(enabled & PIPELINE_SHADING_RATE != 0),
        primitiveFragmentShadingRate: u32::from(enabled & PRIMITIVE_SHADING_RATE != 0),
        attachmentFragmentShadingRate: u32::from(enabled & ATTACHMENT_SHADING_RATE != 0),
        ..Default::default()
    }
}
pub(super) fn load(instance: &Instance) -> Result<vk::PFN_vkCmdSetFragmentShadingRateKHR, Status> {
    let f = unsafe {
        std::mem::transmute::<vk::PFN_vkVoidFunction, vk::PFN_vkCmdSetFragmentShadingRateKHR>(
            instance.proc(c"vkCmdSetFragmentShadingRateKHR"),
        )
    };
    if f.is_none() {
        return Err(UNSUPPORTED);
    }
    Ok(f)
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(FeatureBits, ShadingRateLimits, Vec<ShadingRate>), Status> {
    if !instance
        .supports_extension(physical, c"VK_KHR_fragment_shading_rate")
        .map_err(loader_error)?
    {
        return Ok((FeatureBits::ZERO, ShadingRateLimits::default(), Vec::new()));
    }
    let mut features = requested(FeatureBits::ZERO);
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: ptr::from_mut(&mut features).cast(),
        ..Default::default()
    };
    let mut p = vk::VkPhysicalDeviceFragmentShadingRatePropertiesKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FRAGMENT_SHADING_RATE_PROPERTIES_KHR, ..Default::default() };
    let mut props = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: ptr::from_mut(&mut p).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut props);
    }
    let available = if features.pipelineFragmentShadingRate != 0 {
        PIPELINE_SHADING_RATE
    } else {
        FeatureBits::ZERO
    } | if features.primitiveFragmentShadingRate != 0 {
        PRIMITIVE_SHADING_RATE
    } else {
        FeatureBits::ZERO
    } | if features.attachmentFragmentShadingRate != 0 {
        ATTACHMENT_SHADING_RATE
    } else {
        FeatureBits::ZERO
    };
    let query = unsafe {
        std::mem::transmute::<
            vk::PFN_vkVoidFunction,
            vk::PFN_vkGetPhysicalDeviceFragmentShadingRatesKHR,
        >(instance.proc(c"vkGetPhysicalDeviceFragmentShadingRatesKHR"))
    }
    .ok_or(UNSUPPORTED)?;
    for _ in 0..4 {
        let mut count = 0;
        unsafe {
            status(query(physical, &mut count, ptr::null_mut()))?;
        }
        let mut native = vec![
            vk::VkPhysicalDeviceFragmentShadingRateKHR {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FRAGMENT_SHADING_RATE_KHR,
                ..Default::default()
            };
            count as usize
        ];
        let result = unsafe { query(physical, &mut count, native.as_mut_ptr()) };
        if result == vk::VkResult_VK_INCOMPLETE {
            continue;
        }
        status(result)?;
        if count as usize > native.len() {
            return Err(BACKEND_ERROR);
        }
        let rates = native[..count as usize]
            .iter()
            .map(|r| ShadingRate {
                fragment_size: [r.fragmentSize.width, r.fragmentSize.height],
                samples: r.sampleCounts,
            })
            .collect();
        return Ok((available, limits(&p), rates));
    }
    Err(BACKEND_ERROR)
}
fn limits(p: &vk::VkPhysicalDeviceFragmentShadingRatePropertiesKHR) -> ShadingRateLimits {
    ShadingRateLimits {
        min_attachment_texel: [
            p.minFragmentShadingRateAttachmentTexelSize.width,
            p.minFragmentShadingRateAttachmentTexelSize.height,
        ],
        max_attachment_texel: [
            p.maxFragmentShadingRateAttachmentTexelSize.width,
            p.maxFragmentShadingRateAttachmentTexelSize.height,
        ],
        max_attachment_texel_aspect_ratio: p.maxFragmentShadingRateAttachmentTexelSizeAspectRatio,
        primitive_with_multiple_viewports: p.primitiveFragmentShadingRateWithMultipleViewports,
        layered_attachments: p.layeredShadingRateAttachments,
        nontrivial_combiners: p.fragmentShadingRateNonTrivialCombinerOps,
        max_fragment_size: [p.maxFragmentSize.width, p.maxFragmentSize.height],
        max_fragment_aspect_ratio: p.maxFragmentSizeAspectRatio,
        max_coverage_samples: p.maxFragmentShadingRateCoverageSamples,
        max_rasterization_samples: p.maxFragmentShadingRateRasterizationSamples,
        with_shader_depth_stencil_writes: p.fragmentShadingRateWithShaderDepthStencilWrites,
        with_sample_mask: p.fragmentShadingRateWithSampleMask,
        with_shader_sample_mask: p.fragmentShadingRateWithShaderSampleMask,
        with_conservative_rasterization: p.fragmentShadingRateWithConservativeRasterization,
        with_fragment_shader_interlock: p.fragmentShadingRateWithFragmentShaderInterlock,
        with_custom_sample_locations: p.fragmentShadingRateWithCustomSampleLocations,
        strict_multiply_combiner: p.fragmentShadingRateStrictMultiplyCombiner,
    }
}
/// Only hard native validity rules. Unsupported final rates are reduced natively;
/// the physical tuple table and interaction properties are not rejection criteria.
pub(super) fn state(
    enabled: FeatureBits,
    limits: &ShadingRateLimits,
    state: &ShadingRateState,
) -> Result<(), Status> {
    if state
        .fragment_size
        .iter()
        .any(|v| !v.is_power_of_two() || *v > 4)
        || state.combiners.iter().any(|v| *v > 4)
    {
        return Err(INVALID);
    }
    if enabled & INPUTS == 0
        || (state.fragment_size != [1, 1] && enabled & PIPELINE_SHADING_RATE == 0)
        || (state.combiners[0] != 0 && enabled & PRIMITIVE_SHADING_RATE == 0)
        || (state.combiners[1] != 0 && enabled & ATTACHMENT_SHADING_RATE == 0)
        || (limits.nontrivial_combiners == 0 && state.combiners.iter().any(|v| *v > 1))
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn texels(limits: &ShadingRateLimits, size: [u32; 2]) -> Result<(), Status> {
    if size.iter().any(|v| !v.is_power_of_two()) {
        return Err(INVALID);
    }
    for i in 0..2 {
        if size[i] < limits.min_attachment_texel[i]
            || size[i] > limits.max_attachment_texel[i]
            || u64::from(size[i])
                > u64::from(size[1 - i]) * u64::from(limits.max_attachment_texel_aspect_ratio)
        {
            return Err(UNSUPPORTED);
        }
    }
    Ok(())
}

/// Bounds of the selected base mip, including render-area offset and sparse views.
pub(super) fn attachment_bounds(
    extent: [u32; 2],
    layers: u32,
    texels: [u32; 2],
    desc: &RenderDesc,
) -> Result<(), Status> {
    let required_layers = features::render_layers(desc.layers, desc.view_mask);
    if desc.x < 0
        || desc.y < 0
        || layers == 0
        || (layers != 1 && layers < required_layers)
        || u64::from(extent[0]) * u64::from(texels[0]) < desc.x as u64 + u64::from(desc.width)
        || u64::from(extent[1]) * u64::from(texels[1]) < desc.y as u64 + u64::from(desc.height)
    {
        return Err(INVALID);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_inputs_and_native_reduction_rules() {
        let limits = ShadingRateLimits {
            nontrivial_combiners: 1,
            max_fragment_size: [1, 1],
            ..Default::default()
        };
        let mut rate = ShadingRateState::default();
        assert_eq!(state(FeatureBits::ZERO, &limits, &rate), Err(UNSUPPORTED));
        for mask in [
            PIPELINE_SHADING_RATE,
            PRIMITIVE_SHADING_RATE,
            ATTACHMENT_SHADING_RATE,
        ] {
            assert_eq!(state(mask, &limits, &rate), Ok(()));
            let f = requested(mask);
            assert_eq!(
                f.pipelineFragmentShadingRate != 0,
                mask == PIPELINE_SHADING_RATE
            );
            assert_eq!(
                f.primitiveFragmentShadingRate != 0,
                mask == PRIMITIVE_SHADING_RATE
            );
            assert_eq!(
                f.attachmentFragmentShadingRate != 0,
                mask == ATTACHMENT_SHADING_RATE
            );
        }
        rate.fragment_size = [4, 4]; // Valid even if native reduction chooses 1x1.
        assert_eq!(state(PIPELINE_SHADING_RATE, &limits, &rate), Ok(()));
        assert_eq!(
            state(PRIMITIVE_SHADING_RATE, &limits, &rate),
            Err(UNSUPPORTED)
        );
        rate.fragment_size = [1, 1];
        rate.combiners = [1, 0];
        assert_eq!(
            state(PIPELINE_SHADING_RATE, &limits, &rate),
            Err(UNSUPPORTED)
        );
        assert_eq!(state(PRIMITIVE_SHADING_RATE, &limits, &rate), Ok(()));
        rate.combiners = [0, 4];
        assert_eq!(state(ATTACHMENT_SHADING_RATE, &limits, &rate), Ok(()));
        assert_eq!(
            state(
                ATTACHMENT_SHADING_RATE,
                &ShadingRateLimits::default(),
                &rate
            ),
            Err(UNSUPPORTED)
        );
        for size in [[0, 1], [3, 2], [8, 1], [u32::MAX, 1]] {
            rate.fragment_size = size;
            assert_eq!(state(INPUTS, &limits, &rate), Err(INVALID));
        }
        rate.fragment_size = [1, 1];
        rate.combiners = [5, 0];
        assert_eq!(state(INPUTS, &limits, &rate), Err(INVALID));
        let all = INPUTS | MESH_PRIMITIVE_SHADING_RATE | MESH_SHADER | RASTER;
        assert_eq!(features::encode(&features::decode(all)), Ok(all));
        assert_eq!(validate_feature_dependencies(all), Ok(()));
        assert_eq!(
            validate_feature_dependencies(all & !PRIMITIVE_SHADING_RATE),
            Err(INVALID)
        );
    }
    #[test]
    fn attachment_texels_obey_both_aspect_directions() {
        let p = ShadingRateLimits {
            min_attachment_texel: [4, 4],
            max_attachment_texel: [32, 32],
            max_attachment_texel_aspect_ratio: 2,
            ..Default::default()
        };
        assert_eq!(texels(&p, [8, 16]), Ok(()));
        assert_eq!(texels(&p, [32, 4]), Err(UNSUPPORTED));
        assert_eq!(texels(&p, [4, 32]), Err(UNSUPPORTED));
        assert_eq!(texels(&p, [0, 4]), Err(INVALID));
        assert_eq!(texels(&p, [6, 4]), Err(INVALID));
        assert_eq!(texels(&p, [2, 4]), Err(UNSUPPORTED));
    }
    #[test]
    fn attachment_coverage_includes_offset_and_sparse_layers() {
        let mut desc = RenderDesc {
            x: 3,
            y: 5,
            width: 14,
            height: 12,
            layers: 4,
            header: Record::new::<RenderDesc>(RENDER_DESC),
            view_mask: 0,
            samples: 1,
            flags: 0,
            color_count: 0,
            colors: ptr::null(),
            depth: ptr::null(),
            stencil: ptr::null(),
            scratch: HostSpan {
                data: ptr::null_mut(),
                size: 0,
            },
            shading_rate: ptr::null(),
        };
        assert_eq!(attachment_bounds([3, 3], 4, [8, 8], &desc), Ok(()));
        assert_eq!(attachment_bounds([2, 3], 4, [8, 8], &desc), Err(INVALID));
        assert_eq!(attachment_bounds([3, 2], 4, [8, 8], &desc), Err(INVALID));
        assert_eq!(attachment_bounds([3, 3], 3, [8, 8], &desc), Err(INVALID));
        assert_eq!(attachment_bounds([3, 3], 1, [8, 8], &desc), Ok(())); // broadcast
        desc.layers = 1;
        desc.view_mask = 0b1001;
        assert_eq!(attachment_bounds([3, 3], 2, [8, 8], &desc), Err(INVALID)); // not popcount
        assert_eq!(attachment_bounds([3, 3], 4, [8, 8], &desc), Ok(()));
        desc.x = i32::MAX;
        desc.width = u32::MAX;
        assert_eq!(attachment_bounds([u32::MAX, 3], 1, [4, 8], &desc), Ok(()));
        assert_eq!(
            attachment_bounds([u32::MAX, 3], 1, [1, 8], &desc),
            Err(INVALID)
        );
        desc.x = -1;
        assert_eq!(
            attachment_bounds([u32::MAX, 3], 1, [4, 8], &desc),
            Err(INVALID)
        );
    }
    thread_local! { static CAPTURE: std::cell::Cell<Option<[u32;4]>> = const { std::cell::Cell::new(None) }; }
    unsafe extern "C" fn capture(
        _command: vk::VkCommandBuffer,
        size: *const vk::VkExtent2D,
        combiners: *const vk::VkFragmentShadingRateCombinerOpKHR,
    ) {
        CAPTURE.set(Some(unsafe {
            [(*size).width, (*size).height, *combiners, *combiners.add(1)]
        }));
    }
    fn record_rate(d: &Device, domain: u32, state: &ShadingRateState) -> Result<(), Status> {
        let mut arena = Arena::create(
            d,
            &ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
                level: 0,
                reserved: 0,
            },
        )
        .unwrap();
        let list = arena
            .begin(&RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 2,
                level: 0,
                inheritance: ptr::null(),
            })
            .unwrap();
        let result = unsafe { &*list }.shading_rate(state);
        unsafe { &*list }.end().unwrap();
        result
    }
    #[test]
    #[ignore = "requires modern Vulkan; independent native shading-rate discovery/enabling"]
    fn gpu_foundation_shading_rate_enables() {
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
            let available = snapshot.features.available;
            eprintln!(
                "SHADING_RATE native pipeline={} primitive={} attachment={} mesh={} tuples={}",
                available & PIPELINE_SHADING_RATE != 0,
                available & PRIMITIVE_SHADING_RATE != 0,
                available & ATTACHMENT_SHADING_RATE != 0,
                available & MESH_PRIMITIVE_SHADING_RATE != 0,
                snapshot.shading_rates.len()
            );
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            for bit in [
                PIPELINE_SHADING_RATE,
                PRIMITIVE_SHADING_RATE,
                ATTACHMENT_SHADING_RATE,
            ] {
                let result = Device::create(&adapter, &request, RASTER | bit);
                if available & bit == 0 {
                    assert!(matches!(result, Err(UNSUPPORTED)));
                } else {
                    let d = result.unwrap();
                    assert_eq!(d.snapshot.features.enabled, RASTER | bit);
                }
            }
            let mut d = Device::create(&adapter, &request, RASTER).unwrap();
            assert_eq!(
                record_rate(&d, request[0].domain, &ShadingRateState::default()),
                Err(UNSUPPORTED)
            );
            // Capture only: no native FSR command or rendering is executed here.
            d.snapshot.features.enabled |= INPUTS;
            d.snapshot.shading_rate_limits.nontrivial_combiners = 1;
            d.shading_rate_function = Some(capture);
            CAPTURE.set(None);
            let rate = ShadingRateState {
                fragment_size: [4, 2],
                combiners: [SHADING_RATE_MIN, SHADING_RATE_MUL],
            };
            assert_eq!(record_rate(&d, request[0].domain, &rate), Ok(()));
            assert_eq!(CAPTURE.get(), Some([4, 2, 2, 4]));
            CAPTURE.set(None);
            assert_eq!(
                record_rate(
                    &d,
                    request[0].domain,
                    &ShadingRateState {
                        fragment_size: [3, 2],
                        ..rate
                    }
                ),
                Err(INVALID)
            );
            assert_eq!(CAPTURE.get(), None);
            checked = true;
        }
        assert!(checked, "no compatible physical device");
    }
}
