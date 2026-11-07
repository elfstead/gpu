//! Programmable positions. Patterns remain caller-owned; no historical depth state.
use super::*;

pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(FeatureBits, SampleLocationLimits), Status> {
    if !instance
        .supports_extension(physical, c"VK_EXT_sample_locations")
        .map_err(loader_error)?
    {
        return Ok((0, SampleLocationLimits::default()));
    }
    let mut p = vk::VkPhysicalDeviceSampleLocationsPropertiesEXT {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_SAMPLE_LOCATIONS_PROPERTIES_EXT,
        ..Default::default()
    };
    let mut root = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: ptr::from_mut(&mut p).cast(),
        ..Default::default()
    };
    let query = unsafe {
        std::mem::transmute::<
            vk::PFN_vkVoidFunction,
            vk::PFN_vkGetPhysicalDeviceMultisamplePropertiesEXT,
        >(instance.proc(c"vkGetPhysicalDeviceMultisamplePropertiesEXT"))
    }
    .ok_or(UNSUPPORTED)?;
    unsafe {
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut root);
    }
    let mut limits = SampleLocationLimits {
        samples: p.sampleLocationSampleCounts,
        max_grid: [
            p.maxSampleLocationGridSize.width,
            p.maxSampleLocationGridSize.height,
        ],
        coordinate_range: p.sampleLocationCoordinateRange,
        subpixel_bits: p.sampleLocationSubPixelBits,
        variable: p.variableSampleLocations,
        grids: [[0; 2]; 7],
    };
    for (index, grid) in limits.grids.iter_mut().enumerate() {
        if limits.samples & (1 << index) == 0 {
            continue;
        }
        let mut result = vk::VkMultisamplePropertiesEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MULTISAMPLE_PROPERTIES_EXT,
            ..Default::default()
        };
        unsafe {
            query(physical, 1 << index, &mut result);
        }
        *grid = [
            result.maxSampleLocationGridSize.width,
            result.maxSampleLocationGridSize.height,
        ];
    }
    Ok((SAMPLE_LOCATIONS, limits))
}
pub(super) fn image_flags(flags: u32, aspects: u32, enabled: FeatureBits) -> Result<(), Status> {
    if flags & IMAGE_SAMPLE_LOCATIONS_DEPTH != 0 {
        if aspects & 2 == 0 {
            return Err(INVALID);
        }
        if enabled & SAMPLE_LOCATIONS == 0 {
            return Err(UNSUPPORTED);
        }
    }
    Ok(())
}
pub(super) struct Commands {
    pub positions: vk::PFN_vkCmdSetSampleLocationsEXT,
    pub enable: vk::PFN_vkCmdSetSampleLocationsEnableEXT,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        let positions = if enabled & SAMPLE_LOCATIONS != 0 {
            unsafe {
                std::mem::transmute::<vk::PFN_vkVoidFunction, vk::PFN_vkCmdSetSampleLocationsEXT>(
                    instance.proc(c"vkCmdSetSampleLocationsEXT"),
                )
            }
            .ok_or(UNSUPPORTED)?
            .into()
        } else {
            None
        };
        let enable = if enabled & DYNAMIC_SAMPLE_LOCATIONS_ENABLE != 0 {
            unsafe {
                std::mem::transmute::<
                    vk::PFN_vkVoidFunction,
                    vk::PFN_vkCmdSetSampleLocationsEnableEXT,
                >(instance.proc(c"vkCmdSetSampleLocationsEnableEXT"))
            }
            .ok_or(UNSUPPORTED)?
            .into()
        } else {
            None
        };
        Ok(Self { positions, enable })
    }
}
// C/Rust/native coordinates have identical representation; no packing/copy pass.
const _: () = {
    assert!(
        std::mem::size_of::<SampleLocation>() == std::mem::size_of::<vk::VkSampleLocationEXT>()
    );
    assert!(
        std::mem::align_of::<SampleLocation>() == std::mem::align_of::<vk::VkSampleLocationEXT>()
    );
};
pub(super) unsafe fn pattern(
    p: &SampleLocations,
    enabled: FeatureBits,
    limits: &SampleLocationLimits,
) -> Result<vk::VkSampleLocationsInfoEXT, Status> {
    p.header
        .validate::<SampleLocations>(SAMPLE_LOCATIONS_RECORD)?;
    if !p.samples.is_power_of_two() || p.samples > 64 || p.grid.contains(&0) {
        return Err(INVALID);
    }
    if enabled & SAMPLE_LOCATIONS == 0 || limits.samples & p.samples == 0 {
        return Err(UNSUPPORTED);
    }
    let max = limits.grids[p.samples.trailing_zeros() as usize];
    if (0..2).any(|i| max[i] == 0 || max[i] % p.grid[i] != 0) {
        return Err(UNSUPPORTED);
    }
    let count = p
        .samples
        .checked_mul(p.grid[0])
        .and_then(|n| n.checked_mul(p.grid[1]))
        .ok_or(INVALID)?;
    if p.count != count {
        return Err(INVALID);
    }
    let values = unsafe { commands::slice(p.locations, p.count)? };
    if values.iter().any(|v| !v.x.is_finite() || !v.y.is_finite()) {
        return Err(INVALID);
    }
    Ok(vk::VkSampleLocationsInfoEXT {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SAMPLE_LOCATIONS_INFO_EXT,
        sampleLocationsPerPixel: p.samples,
        sampleLocationGridSize: vk::VkExtent2D {
            width: p.grid[0],
            height: p.grid[1],
        },
        sampleLocationsCount: p.count,
        pSampleLocations: p.locations.cast(),
        ..Default::default()
    })
}
pub(super) unsafe fn prepare(
    state: Option<SampleLocationState>,
    enabled: FeatureBits,
    limits: &SampleLocationLimits,
    dynamic: u64,
    samples: u32,
) -> Result<Option<vk::VkPipelineSampleLocationsStateCreateInfoEXT>, Status> {
    let dp = dynamic & DYNAMIC_SAMPLE_LOCATIONS_STATE != 0;
    let de = dynamic & DYNAMIC_SAMPLE_LOCATIONS_ENABLE_STATE != 0;
    if state.is_none() && !dp && !de {
        return Ok(None);
    }
    if enabled & SAMPLE_LOCATIONS == 0 || de && enabled & DYNAMIC_SAMPLE_LOCATIONS_ENABLE == 0 {
        return Err(UNSUPPORTED);
    }
    let state = state.unwrap_or(SampleLocationState {
        header: Record::new::<SampleLocationState>(SAMPLE_LOCATION_STATE),
        enable: 0,
        reserved: 0,
        locations: ptr::null(),
    });
    if state.reserved != 0 || !de && state.enable > 1 {
        return Err(INVALID);
    }
    if !de && state.enable == 0 && !dp {
        return Ok(None);
    }
    let info = if dp || !de && state.enable == 0 {
        vk::VkSampleLocationsInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SAMPLE_LOCATIONS_INFO_EXT,
            sampleLocationsPerPixel: samples,
            ..Default::default()
        }
    } else {
        let p = unsafe { state.locations.as_ref() }.ok_or(INVALID)?;
        let result = unsafe { pattern(p, enabled, limits)? };
        if p.samples != samples {
            return Err(INVALID);
        }
        result
    };
    Ok(Some(vk::VkPipelineSampleLocationsStateCreateInfoEXT {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_SAMPLE_LOCATIONS_STATE_CREATE_INFO_EXT,
        sampleLocationsEnable: if de { 0 } else { state.enable },
        sampleLocationsInfo: info,
        ..Default::default()
    }))
}
/// Transition metadata describes existing depth, never a pattern inferred from a pipeline.
pub(super) unsafe fn transition(
    p: *const Record,
    enabled: FeatureBits,
    limits: &SampleLocationLimits,
    image_flags: u32,
    samples: u32,
) -> Result<vk::VkSampleLocationsInfoEXT, Status> {
    if image_flags & IMAGE_SAMPLE_LOCATIONS_DEPTH == 0 {
        return Err(INVALID);
    }
    let p = unsafe { executables::record::<SampleLocations>(p, SAMPLE_LOCATIONS_RECORD)? };
    let result = unsafe { pattern(p, enabled, limits)? };
    if p.samples != samples {
        return Err(INVALID);
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> SampleLocationLimits {
        let mut p = SampleLocationLimits {
            samples: 4,
            coordinate_range: [0.0, 0.9375],
            ..Default::default()
        };
        p.grids[2] = [2, 2];
        p
    }
    #[test]
    fn pattern_checks_shape_count_and_capability_but_preserves_native_clamping() {
        let coords = [SampleLocation { x: -1.0, y: 2.0 }; 4];
        let mut p = SampleLocations {
            header: Record::new::<SampleLocations>(SAMPLE_LOCATIONS_RECORD),
            samples: 4,
            grid: [1, 1],
            count: 4,
            locations: coords.as_ptr(),
        };
        let native = unsafe { pattern(&p, SAMPLE_LOCATIONS, &limits()) }.unwrap();
        assert_eq!(native.pSampleLocations, coords.as_ptr().cast());
        assert!(matches!(
            unsafe { pattern(&p, 0, &limits()) },
            Err(UNSUPPORTED)
        ));
        p.grid = [3, 1];
        assert!(matches!(
            unsafe { pattern(&p, SAMPLE_LOCATIONS, &limits()) },
            Err(UNSUPPORTED)
        ));
        p.grid = [1, 1];
        p.count = 3;
        assert!(matches!(
            unsafe { pattern(&p, SAMPLE_LOCATIONS, &limits()) },
            Err(INVALID)
        ));
        p.count = 4;
        p.samples = 3;
        assert!(matches!(
            unsafe { pattern(&p, SAMPLE_LOCATIONS, &limits()) },
            Err(INVALID)
        ));
        p.samples = 4;
        p.header.next = &p.header;
        assert!(unsafe { pattern(&p, SAMPLE_LOCATIONS, &limits()) }.is_err());
    }
    #[test]
    fn dynamic_pattern_and_enable_are_independent_and_reserved_still_checked() {
        let mut s = SampleLocationState {
            header: Record::new::<SampleLocationState>(SAMPLE_LOCATION_STATE),
            enable: 1,
            reserved: 0,
            locations: ptr::null(),
        };
        assert!(unsafe {
            prepare(
                Some(s),
                SAMPLE_LOCATIONS,
                &limits(),
                DYNAMIC_SAMPLE_LOCATIONS_STATE,
                4,
            )
        }
        .is_ok());
        assert!(unsafe { prepare(Some(s), SAMPLE_LOCATIONS, &limits(), 0, 4) }.is_err());
        s.enable = u32::MAX;
        let both = DYNAMIC_SAMPLE_LOCATIONS_STATE | DYNAMIC_SAMPLE_LOCATIONS_ENABLE_STATE;
        let enabled = SAMPLE_LOCATIONS | DYNAMIC_SAMPLE_LOCATIONS_ENABLE;
        assert!(unsafe { prepare(Some(s), enabled, &limits(), both, 4) }.is_ok());
        assert!(unsafe { prepare(Some(s), SAMPLE_LOCATIONS, &limits(), both, 4) }.is_err());
        s.reserved = 1;
        assert!(unsafe { prepare(Some(s), enabled, &limits(), both, 4) }.is_err());
        assert_eq!(
            validate_feature_dependencies(DYNAMIC_SAMPLE_LOCATIONS_ENABLE),
            Err(INVALID)
        );
    }
    #[test]
    fn depth_metadata_requires_compatible_image_and_matching_sample_count() {
        let coords = [SampleLocation { x: 0.25, y: 0.75 }; 4];
        let p = SampleLocations {
            header: Record::new::<SampleLocations>(SAMPLE_LOCATIONS_RECORD),
            samples: 4,
            grid: [1, 1],
            count: 4,
            locations: coords.as_ptr(),
        };
        assert!(unsafe { transition(&p.header, SAMPLE_LOCATIONS, &limits(), 8, 4) }.is_ok());
        assert!(unsafe { transition(&p.header, SAMPLE_LOCATIONS, &limits(), 0, 4) }.is_err());
        assert!(unsafe { transition(&p.header, SAMPLE_LOCATIONS, &limits(), 8, 2) }.is_err());
    }
}
