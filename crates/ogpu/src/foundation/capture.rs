//! Native ordered primitive capture; callers own payloads, counters and scheduling.
use super::*;
pub(super) const FEATURES: FeatureBits = TRANSFORM_FEEDBACK | GEOMETRY_STREAMS;
pub(super) fn native(
    enabled: FeatureBits,
    next: *mut std::ffi::c_void,
) -> vk::VkPhysicalDeviceTransformFeedbackFeaturesEXT {
    vk::VkPhysicalDeviceTransformFeedbackFeaturesEXT {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_TRANSFORM_FEEDBACK_FEATURES_EXT,
        pNext: next,
        transformFeedback: u32::from(enabled & TRANSFORM_FEEDBACK != 0),
        geometryStreams: u32::from(enabled & GEOMETRY_STREAMS != 0),
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(FeatureBits, CaptureLimits), Status> {
    if !instance
        .supports_extension(physical, c"VK_EXT_transform_feedback")
        .map_err(loader_error)?
    {
        return Ok((0, CaptureLimits::default()));
    }
    let mut n = native(0, ptr::null_mut());
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: ptr::from_mut(&mut n).cast(),
        ..Default::default()
    };
    let mut p = vk::VkPhysicalDeviceTransformFeedbackPropertiesEXT {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_TRANSFORM_FEEDBACK_PROPERTIES_EXT,
        ..Default::default()
    };
    let mut props = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: ptr::from_mut(&mut p).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut props);
    }
    let bits = if n.transformFeedback != 0 {
        TRANSFORM_FEEDBACK
    } else {
        0
    } | if n.geometryStreams != 0 {
        GEOMETRY_STREAMS
    } else {
        0
    };
    Ok((
        bits,
        CaptureLimits {
            max_streams: p.maxTransformFeedbackStreams,
            max_buffers: p.maxTransformFeedbackBuffers,
            max_buffer_size: p.maxTransformFeedbackBufferSize,
            max_stream_data_size: p.maxTransformFeedbackStreamDataSize,
            max_buffer_data_size: p.maxTransformFeedbackBufferDataSize,
            max_stride: p.maxTransformFeedbackBufferDataStride,
            queries: p.transformFeedbackQueries,
            streams_lines_triangles: p.transformFeedbackStreamsLinesTriangles,
            rasterization_stream_select: p.transformFeedbackRasterizationStreamSelect,
            draw_byte_count: p.transformFeedbackDraw,
            reserved: 0,
        },
    ))
}
pub(super) struct Commands {
    pub bind: vk::PFN_vkCmdBindTransformFeedbackBuffers2EXT,
    pub begin: vk::PFN_vkCmdBeginTransformFeedback2EXT,
    pub end: vk::PFN_vkCmdEndTransformFeedback2EXT,
    pub draw: vk::PFN_vkCmdDrawIndirectByteCount2EXT,
    pub query_begin: vk::PFN_vkCmdBeginQueryIndexedEXT,
    pub query_end: vk::PFN_vkCmdEndQueryIndexedEXT,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        macro_rules! load {
            ($name:literal,$ty:ty) => {{
                if enabled & TRANSFORM_FEEDBACK == 0 {
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
            bind: load!(
                c"vkCmdBindTransformFeedbackBuffers2EXT",
                vk::PFN_vkCmdBindTransformFeedbackBuffers2EXT
            ),
            begin: load!(
                c"vkCmdBeginTransformFeedback2EXT",
                vk::PFN_vkCmdBeginTransformFeedback2EXT
            ),
            end: load!(
                c"vkCmdEndTransformFeedback2EXT",
                vk::PFN_vkCmdEndTransformFeedback2EXT
            ),
            draw: load!(
                c"vkCmdDrawIndirectByteCount2EXT",
                vk::PFN_vkCmdDrawIndirectByteCount2EXT
            ),
            query_begin: load!(
                c"vkCmdBeginQueryIndexedEXT",
                vk::PFN_vkCmdBeginQueryIndexedEXT
            ),
            query_end: load!(c"vkCmdEndQueryIndexedEXT", vk::PFN_vkCmdEndQueryIndexedEXT),
        })
    }
}
pub(super) fn stream(
    value: u32,
    enabled: FeatureBits,
    p: &CaptureLimits,
    raster: bool,
) -> Result<(), Status> {
    if value == 0 {
        return Ok(());
    }
    if enabled & GEOMETRY_STREAMS == 0
        || value >= p.max_streams
        || raster && p.rasterization_stream_select == 0
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
pub(super) fn range(first: u32, count: u32, max: u32) -> Result<(), Status> {
    if first >= max || first.checked_add(count).ok_or(INVALID)? > max {
        return Err(INVALID);
    }
    Ok(())
}
pub(super) fn scratch(count: u32) -> Result<HostRequirements, Status> {
    if count == 0 {
        return Ok(HostRequirements {
            size: 0,
            alignment: 1,
        });
    }
    let l = std::alloc::Layout::array::<vk::VkBindTransformFeedbackBuffer2InfoEXT>(count as usize)
        .map_err(|_| INVALID)?;
    Ok(HostRequirements {
        size: l.size() as u64,
        alignment: l.align() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_stream_selection_and_query_ranges_preserve_independent_capabilities() {
        let mut p = CaptureLimits {
            max_streams: 4,
            max_buffers: 4,
            ..Default::default()
        };
        assert_eq!(stream(0, 0, &p, true), Ok(()));
        assert_eq!(stream(1, TRANSFORM_FEEDBACK, &p, false), Err(UNSUPPORTED));
        assert_eq!(stream(1, GEOMETRY_STREAMS, &p, false), Ok(()));
        assert_eq!(stream(1, GEOMETRY_STREAMS, &p, true), Err(UNSUPPORTED));
        p.rasterization_stream_select = 1;
        assert_eq!(stream(1, GEOMETRY_STREAMS, &p, true), Ok(()));
        assert_eq!(stream(4, GEOMETRY_STREAMS, &p, false), Err(UNSUPPORTED));
        assert_eq!(range(3, 1, 4), Ok(()));
        assert_eq!(range(4, 0, 4), Err(INVALID));
        assert_eq!(range(1, u32::MAX, 4), Err(INVALID));
        assert_eq!(scratch(0).unwrap().size, 0);
        assert!(
            scratch(4).unwrap().size
                >= 4 * std::mem::size_of::<vk::VkBindTransformFeedbackBuffer2InfoEXT>() as u64
        );
        assert_eq!(
            validate_feature_dependencies(TRANSFORM_FEEDBACK),
            Err(INVALID)
        );
        assert_eq!(
            validate_feature_dependencies(TRANSFORM_FEEDBACK | RASTER),
            Ok(())
        );
        let n = native(TRANSFORM_FEEDBACK, ptr::null_mut());
        assert_eq!((n.transformFeedback, n.geometryStreams), (1, 0));
    }
}
