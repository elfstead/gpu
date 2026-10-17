//! Immutable format capabilities and allocation-free image profile queries.
use super::*;

fn image_features(native: u64) -> u64 {
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
        | if native & vk::VK_FORMAT_FEATURE_2_STORAGE_READ_WITHOUT_FORMAT_BIT != 0 {
            format_features::STORAGE_READ_WITHOUT_FORMAT
        } else {
            0
        }
        | if native & vk::VK_FORMAT_FEATURE_2_STORAGE_WRITE_WITHOUT_FORMAT_BIT != 0 {
            format_features::STORAGE_WRITE_WITHOUT_FORMAT
        } else {
            0
        }
}
fn buffer_features(native: u64) -> u64 {
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
fn remap(native: u64, bits: &[u32]) -> u64 {
    bits.iter().enumerate().fold(0, |mask, (i, bit)| {
        mask | if native & u64::from(*bit) != 0 {
            1 << i
        } else {
            0
        }
    })
}
pub(in crate::foundation::native) fn format_snapshot(
    f: &Functions,
    physical: vk::VkPhysicalDevice,
) -> Result<Vec<FormatInfo>, Status> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(FORMATS.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    for (index, format) in FORMATS.iter().enumerate() {
        let mut p = vk::VkFormatProperties3 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_FORMAT_PROPERTIES_3,
            ..Default::default()
        };
        let mut query = vk::VkFormatProperties2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_FORMAT_PROPERTIES_2,
            pNext: (&mut p as *mut vk::VkFormatProperties3).cast(),
            ..Default::default()
        };
        unsafe {
            (f.vkGetPhysicalDeviceFormatProperties2.unwrap())(physical, format.native, &mut query);
        }
        result.push(FormatInfo {
            format: index as u32 + 1,
            aspects: format.aspects,
            optimal_features: image_features(p.optimalTilingFeatures),
            linear_features: image_features(p.linearTilingFeatures),
            buffer_features: buffer_features(p.bufferFeatures),
            copy_block_extent: format.block_extent,
            copy_block_bytes: [1, 2, 4].map(|aspect| format.copy_bytes(aspect).unwrap_or(0) as u32),
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
    fn c_and_rust_image_feature_bits_match() {
        use format_features::*;
        let header = include_str!("../../../../include/ogpu_next.h");
        for (name, value) in [
            ("SAMPLED", SAMPLED),
            ("STORAGE", STORAGE),
            ("STORAGE_ATOMIC", STORAGE_ATOMIC),
            ("COLOR", COLOR),
            ("BLEND", BLEND),
            ("DEPTH_STENCIL", DEPTH_STENCIL),
            ("BLIT_SRC", BLIT_SRC),
            ("BLIT_DST", BLIT_DST),
            ("FILTER_LINEAR", FILTER_LINEAR),
            ("COPY_SRC", COPY_SRC),
            ("COPY_DST", COPY_DST),
            ("STORAGE_READ_WITHOUT_FORMAT", STORAGE_READ_WITHOUT_FORMAT),
            ("STORAGE_WRITE_WITHOUT_FORMAT", STORAGE_WRITE_WITHOUT_FORMAT),
        ] {
            assert!(header.contains(&format!(
                "#define OGPU_NEXT_FORMAT_{name} UINT64_C({value})"
            )));
        }
    }
    #[test]
    fn catalog_metadata_and_native_roundtrips() {
        use image_formats::*;
        let mut natives = std::collections::BTreeSet::new();
        for (index, f) in FORMATS.iter().enumerate() {
            assert!(natives.insert(f.native));
            assert_eq!(
                crate::foundation::native::image_format_id(f.native),
                index as u32 + 1
            );
            if f.aspects == 1 {
                if f.compressed() {
                    assert!(f.class >= 2001);
                    assert!(matches!(f.color_bytes, 8 | 16));
                } else {
                    assert!(matches!(f.class, 8 | 16 | 24 | 32 | 48 | 64 | 96 | 128));
                    assert_eq!(f.copy_bytes(1), Ok(u64::from(f.class / 8)));
                }
            } else {
                assert_eq!(f.numeric, Numeric::DepthStencil);
            }
            assert_eq!(f.copy_bytes(0), Err(INVALID));
            assert_eq!(f.copy_bytes(3), Err(INVALID));
        }
        assert_eq!(format(RGB8_UNORM).unwrap().copy_bytes(1), Ok(3));
        assert_eq!(format(RGB16_FLOAT).unwrap().copy_bytes(1), Ok(6));
        assert_eq!(format(D16_UNORM_S8_UINT).unwrap().copy_bytes(2), Ok(2));
        assert_eq!(format(D16_UNORM_S8_UINT).unwrap().copy_bytes(4), Ok(1));
        assert_eq!(format(X8_D24_UNORM_PACK32).unwrap().copy_bytes(2), Ok(4));
        assert_eq!(format(S8_UINT).unwrap().copy_bytes(2), Err(INVALID));
        assert_eq!(format(S8_UINT).unwrap().copy_bytes(4), Ok(1));
        assert!(format(R32_SINT).unwrap().integer_color());
        assert!(!format(R8_SSCALED).unwrap().integer_color());
        assert!(!format(S8_UINT).unwrap().integer_color());
        assert_eq!(
            format(A2R10G10B10_UNORM_PACK32).unwrap().class,
            format(RGBA8_UNORM).unwrap().class
        );
        assert!(format(0).is_err());
        assert!(format(FORMATS.len() as u32 + 1).is_err());
    }
    #[test]
    fn rgb_address_alignment_intersects_transfer_queue_rules() {
        for bytes in [3, 6, 12] {
            assert!(copy_address_aligned(12, 1, bytes, 0));
            assert!(!copy_address_aligned(16, 1, bytes, 0));
            assert!(!copy_address_aligned(6, 1, bytes, 0));
        }
        assert!(copy_address_aligned(3, 1, 3, GRAPHICS));
        assert!(copy_address_aligned(6, 1, 6, COMPUTE));
        assert!(!copy_address_aligned(2, 2, 2, GRAPHICS));
        assert!(copy_address_aligned(4, 4, 1, GRAPHICS));
    }
    #[test]
    fn compressed_block_pitches_edges_and_view_classes() {
        use image_formats::*;
        let bc = format(BC1_RGBA_UNORM_BLOCK).unwrap();
        assert_eq!(bc.block_extent, [4, 4, 1]);
        assert_eq!(bc.class, format(BC1_RGBA_SRGB_BLOCK).unwrap().class);
        assert_ne!(bc.class, format(BC1_RGB_UNORM_BLOCK).unwrap().class);
        assert_ne!(bc.class, format(BC4_UNORM_BLOCK).unwrap().class);
        let e = Extent { x: 7, y: 5, z: 1 };
        assert_eq!(block_copy_pitches(e, 2, bc, 1, 0, 0), Ok((8, 8, 64)));
        assert_eq!(block_copy_pitches(e, 2, bc, 1, 24, 72), Ok((12, 12, 112)));
        assert_eq!(block_copy_pitches(e, 1, bc, 1, 17, 0), Err(INVALID));
        assert_eq!(block_copy_pitches(e, 1, bc, 1, 8, 0), Err(INVALID));
        assert_eq!(block_copy_pitches(e, 1, bc, 1, 24, 25), Err(INVALID));
        assert_eq!(block_copy_pitches(e, 1, bc, 2, 0, 0), Err(INVALID));
        let one = Extent { x: 1, y: 1, z: 1 };
        assert_eq!(block_copy_pitches(one, 1, bc, 1, 0, 0), Ok((4, 4, 8)));
        let astc = format(ASTC_5X4_UNORM_BLOCK).unwrap();
        assert_eq!(block_copy_pitches(e, 2, astc, 1, 0, 0), Ok((10, 8, 128)));
        // Conversion back to native texel units must not wrap.
        assert_eq!(
            block_copy_pitches(one, 1, astc, 1, 16, u64::from(u32::MAX) * 16),
            Err(UNSUPPORTED)
        );
        assert_eq!(block_copy_granularity(astc, [2, 4, 1]), Ok([10, 16, 1]));
        assert_eq!(block_copy_granularity(astc, [0; 3]), Ok([0; 3]));
        assert_eq!(
            block_copy_granularity(astc, [u32::MAX; 3]),
            Err(UNSUPPORTED)
        );
        let edge = Offset { x: 4, y: 4, z: 0 };
        assert_eq!(
            copy_bounds(
                [7, 5, 1],
                edge,
                Extent { x: 3, y: 1, z: 1 },
                bc.block_extent
            ),
            Ok(())
        );
        assert_eq!(
            copy_bounds(
                [7, 5, 1],
                edge,
                Extent { x: 2, y: 1, z: 1 },
                bc.block_extent
            ),
            Err(INVALID)
        );
        assert_eq!(
            copy_bounds([7, 5, 1], Offset { x: 1, ..edge }, one, bc.block_extent),
            Err(INVALID)
        );
        assert_eq!(
            copy_bounds([1; 3], Offset { x: 0, y: 0, z: 0 }, one, bc.block_extent),
            Ok(())
        );
    }
    #[test]
    fn format_capabilities_keep_tiling_and_buffer_domains_distinct() {
        assert_eq!(image_features(0), 0);
        assert_eq!(buffer_features(0), 0);
        assert_eq!(image_features(u64::MAX), 8191);
        assert_eq!(buffer_features(u64::MAX), 15);
        assert_eq!(image_features(1 << 31), 2048);
        assert_eq!(image_features(1 << 32), 4096);
        assert_eq!(buffer_features((1 << 31) | (1 << 32)), 0);
        assert_eq!(
            image_features(vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_DST_BIT.into()),
            128
        );
        assert_eq!(
            image_features(vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_VERTEX_BUFFER_BIT.into()),
            0
        );
        assert_eq!(
            buffer_features(vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_VERTEX_BUFFER_BIT.into()),
            1
        );
        assert_eq!(
            buffer_features(vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_SAMPLED_IMAGE_BIT.into()),
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
