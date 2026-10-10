//! Immutable format capabilities and allocation-free image profile queries.
use super::*;

fn image_features(native: u32) -> u64 {
    let bits = [
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_SAMPLED_IMAGE_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_STORAGE_IMAGE_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_STORAGE_IMAGE_ATOMIC_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BLEND_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_DEPTH_STENCIL_ATTACHMENT_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_SRC_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_DST_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_TRANSFER_SRC_BIT,
        vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_TRANSFER_DST_BIT,
    ];
    remap(native, &bits)
}
fn buffer_features(native: u32) -> u64 {
    remap(
        native,
        &[
            vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_VERTEX_BUFFER_BIT,
            vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_UNIFORM_TEXEL_BUFFER_BIT,
            vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_STORAGE_TEXEL_BUFFER_BIT,
            vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_STORAGE_TEXEL_BUFFER_ATOMIC_BIT,
        ],
    )
}
fn remap(native: u32, bits: &[u32]) -> u64 {
    bits.iter().enumerate().fold(0, |mask, (i, bit)| {
        mask | if native & bit != 0 { 1 << i } else { 0 }
    })
}
pub(in crate::foundation::native) fn format_snapshot(
    f: &Functions,
    physical: vk::VkPhysicalDevice,
) -> Result<Vec<FormatInfo>, Status> {
    let mut result = Vec::new();
    result.try_reserve_exact(18).map_err(|_| OUT_OF_MEMORY)?;
    for id in 1..=18 {
        let format = format(id)?;
        let mut p = vk::VkFormatProperties::default();
        unsafe {
            (f.vkGetPhysicalDeviceFormatProperties.unwrap())(physical, format.native, &mut p);
        }
        result.push(FormatInfo {
            format: id,
            aspects: format.aspects,
            optimal_features: image_features(p.optimalTilingFeatures),
            linear_features: image_features(p.linearTilingFeatures),
            buffer_features: buffer_features(p.bufferFeatures),
        });
    }
    Ok(result)
}
pub(super) fn image_flags(flags: u32) -> Result<u32, Status> {
    if flags & !7 != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(if flags & 1 != 0 {
        vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_MUTABLE_FORMAT_BIT
    } else {
        0
    } | if flags & 2 != 0 {
        vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_CUBE_COMPATIBLE_BIT
    } else {
        0
    } | if flags & 4 != 0 {
        vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_ALIAS_BIT
    } else {
        0
    })
}
impl Device {
    pub(in crate::foundation) fn image_limits(
        &self,
        profile: &ImageProfile,
    ) -> Result<ImageLimits, Status> {
        self.ready()?;
        profile.header.validate::<ImageProfile>(IMAGE_PROFILE)?;
        if profile.reserved != 0 {
            return Err(UNSUPPORTED);
        }
        let format = format(profile.format)?;
        let native_usage = usage(profile.usage, format.aspects)?;
        let flags = image_flags(profile.image_flags)?;
        let ty = match profile.dimension {
            1 => vk::VkImageType_VK_IMAGE_TYPE_1D,
            2 => vk::VkImageType_VK_IMAGE_TYPE_2D,
            3 => vk::VkImageType_VK_IMAGE_TYPE_3D,
            _ => return Err(INVALID),
        };
        if profile.image_flags & 2 != 0 && profile.dimension != 2 {
            return Err(INVALID);
        }
        if format.aspects != 1 && profile.dimension != 2 {
            return Err(UNSUPPORTED);
        }
        let mut p = vk::VkImageFormatProperties::default();
        let result = unsafe {
            (self.f.vkGetPhysicalDeviceImageFormatProperties.unwrap())(
                self.physical,
                format.native,
                ty,
                vk::VkImageTiling_VK_IMAGE_TILING_OPTIMAL,
                native_usage,
                flags,
                &mut p,
            )
        };
        if result == vk::VkResult_VK_ERROR_FORMAT_NOT_SUPPORTED {
            return Err(UNSUPPORTED);
        }
        self.result(result)?;
        let samples =
            if profile.usage & 8 != 0 || profile.dimension != 2 || profile.image_flags & 2 != 0 {
                p.sampleCounts & 1
            } else {
                p.sampleCounts
            };
        if samples == 0 {
            return Err(UNSUPPORTED);
        }
        Ok(ImageLimits {
            max_extent: [p.maxExtent.width, p.maxExtent.height, p.maxExtent.depth],
            max_mips: p.maxMipLevels,
            max_layers: p.maxArrayLayers,
            sample_counts: samples,
            max_resource_size: p.maxResourceSize,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn format_capabilities_keep_tiling_and_buffer_domains_distinct() {
        assert_eq!(image_features(0), 0);
        assert_eq!(buffer_features(0), 0);
        assert_eq!(image_features(u32::MAX), 2047);
        assert_eq!(buffer_features(u32::MAX), 15);
        assert_eq!(
            image_features(vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_DST_BIT),
            128
        );
        assert_eq!(
            image_features(vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_VERTEX_BUFFER_BIT),
            0
        );
        assert_eq!(
            buffer_features(vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_VERTEX_BUFFER_BIT),
            1
        );
        assert_eq!(
            buffer_features(vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_SAMPLED_IMAGE_BIT),
            0
        );
        assert_eq!(
            image_flags(7),
            Ok(vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_MUTABLE_FORMAT_BIT
                | vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_CUBE_COMPATIBLE_BIT
                | vk::VkImageCreateFlagBits_VK_IMAGE_CREATE_ALIAS_BIT)
        );
        assert_eq!(image_flags(8), Err(UNSUPPORTED));
    }
}
