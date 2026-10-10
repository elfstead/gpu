//! Native blits: explicit endpoints, format/filter capability, no shader fallback.
use super::*;

fn endpoints(
    dimension: u32,
    full: [u32; 3],
    offsets: [Offset; 2],
    destination: bool,
) -> Result<[vk::VkOffset3D; 2], Status> {
    let a = [offsets[0].x, offsets[0].y, offsets[0].z];
    let b = [offsets[1].x, offsets[1].y, offsets[1].z];
    for i in 0..3 {
        if a[i] < 0
            || b[i] < 0
            || a[i] as u32 > full[i]
            || b[i] as u32 > full[i]
            || destination && a[i] == b[i]
            || i >= dimension as usize && (a[i] != 0 || b[i] != 1)
        {
            return Err(INVALID);
        }
    }
    Ok(offsets.map(|p| vk::VkOffset3D {
        x: p.x,
        y: p.y,
        z: p.z,
    }))
}

fn filtering(src: u32, dst: u32, sf: u32, df: u32, filter: u32) -> Result<vk::VkFilter, Status> {
    if filter > 1 {
        return Err(UNSUPPORTED);
    }
    let source = format(src)?;
    let destination = format(dst)?;
    // R32_UINT is currently the only integer color format. Add future integer
    // formats to a shared numeric-class helper when extending the format table.
    if (src == 11) != (dst == 11)
        || (source.aspects != 1 || destination.aspects != 1) && (src != dst || filter != 0)
    {
        return Err(INVALID);
    }
    if sf & vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_SRC_BIT == 0
        || df & vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_DST_BIT == 0
        || filter == 1
            && sf & vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT
                == 0
    {
        return Err(UNSUPPORTED);
    }
    Ok(filter)
}

impl Image {
    fn blit_region(
        &self,
        region: BlitRegion,
        destination: bool,
    ) -> Result<(vk::VkImageSubresourceLayers, [vk::VkOffset3D; 2]), Status> {
        self.subresources(Subresources {
            aspects: region.aspect,
            first_mip: region.mip,
            mip_count: 1,
            first_layer: region.first_layer,
            layer_count: region.layer_count,
        })?;
        let desc = &self.prepared.desc;
        let full = [desc.extent.x, desc.extent.y, desc.extent.z].map(|v| (v >> region.mip).max(1));
        Ok((
            vk::VkImageSubresourceLayers {
                aspectMask: region.aspect,
                mipLevel: region.mip,
                baseArrayLayer: region.first_layer,
                layerCount: region.layer_count,
            },
            endpoints(desc.dimension, full, region.offsets, destination)?,
        ))
    }

    pub(in crate::foundation::native) fn record_blit(
        &self,
        d: &Device,
        domain: u32,
        command: vk::VkCommandBuffer,
        src: &Image,
        blit: &ImageBlit,
    ) -> Result<(), Status> {
        let dst_image = self.command_handle(d, domain, 2)?;
        let src_image = src.command_handle(d, domain, 1)?;
        if blit.reserved != 0 {
            return Err(UNSUPPORTED);
        }
        if !matches!(blit.source_state, 1 | 2)
            || !matches!(blit.destination_state, 1 | 3)
            || blit.source.aspect != blit.destination.aspect
            || blit.source.layer_count != blit.destination.layer_count
        {
            return Err(INVALID);
        }
        let a = &src.prepared.desc;
        let b = &self.prepared.desc;
        if a.sample_count != 1
            || b.sample_count != 1
            || (a.dimension == 3 || b.dimension == 3)
                && (blit.source.first_layer != 0
                    || blit.destination.first_layer != 0
                    || blit.source.layer_count != 1)
        {
            return Err(INVALID);
        }
        let filter = filtering(
            a.format,
            b.format,
            src.prepared.format_features,
            self.prepared.format_features,
            blit.filter,
        )?;
        let (src_subresource, src_offsets) = src.blit_region(blit.source, false)?;
        let (dst_subresource, dst_offsets) = self.blit_region(blit.destination, true)?;
        let region = vk::VkImageBlit2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_IMAGE_BLIT_2,
            srcSubresource: src_subresource,
            srcOffsets: src_offsets,
            dstSubresource: dst_subresource,
            dstOffsets: dst_offsets,
            ..Default::default()
        };
        let info = vk::VkBlitImageInfo2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BLIT_IMAGE_INFO_2,
            srcImage: src_image,
            srcImageLayout: src.layout(blit.source_state)?,
            dstImage: dst_image,
            dstImageLayout: self.layout(blit.destination_state)?,
            regionCount: 1,
            pRegions: &region,
            filter,
            ..Default::default()
        };
        unsafe {
            (d.f.vkCmdBlitImage2.unwrap())(command, &info);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blit_bounds_preserve_reversal_and_source_degeneracy() {
        let mut offsets = [Offset { x: 8, y: 4, z: 0 }, Offset { x: 0, y: 0, z: 1 }];
        let native = endpoints(2, [8, 4, 1], offsets, true).unwrap();
        assert_eq!(native[0].x, 8);
        assert_eq!(native[1].x, 0);
        offsets[1].x = 8;
        assert!(endpoints(2, [8, 4, 1], offsets, false).is_ok());
        assert!(matches!(
            endpoints(2, [8, 4, 1], offsets, true),
            Err(INVALID)
        ));
        offsets[1].x = -1;
        assert!(matches!(
            endpoints(2, [8, 4, 1], offsets, false),
            Err(INVALID)
        ));
        offsets[1].x = 9;
        assert!(matches!(
            endpoints(2, [8, 4, 1], offsets, false),
            Err(INVALID)
        ));
        offsets[1].x = 0;
        offsets[0].z = 1;
        offsets[1].z = 0;
        assert!(matches!(
            endpoints(2, [8, 4, 1], offsets, false),
            Err(INVALID)
        ));
        assert!(endpoints(3, [8, 4, 1], offsets, true).is_ok());
    }
    #[test]
    fn blit_format_classes_and_filter_capabilities_are_exact() {
        let all = vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_SRC_BIT
            | vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_DST_BIT
            | vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT;
        assert_eq!(filtering(3, 9, all, all, 1), Ok(1));
        assert_eq!(filtering(4, 5, all, all, 0), Ok(0));
        assert_eq!(filtering(3, 11, all, all, 0), Err(INVALID));
        assert_eq!(filtering(11, 3, all, all, 0), Err(INVALID));
        assert_eq!(filtering(11, 11, all, all, 0), Ok(0));
        assert_eq!(filtering(14, 14, all, all, 0), Ok(0));
        assert_eq!(filtering(14, 13, all, all, 0), Err(INVALID));
        assert_eq!(filtering(14, 14, all, all, 1), Err(INVALID));
        assert_eq!(filtering(3, 3, all, all, 2), Err(UNSUPPORTED));
        assert_eq!(filtering(3, 3, 0, all, 0), Err(UNSUPPORTED));
        assert_eq!(filtering(3, 3, all, 0, 0), Err(UNSUPPORTED));
        assert_eq!(
            filtering(
                3,
                3,
                vk::VkFormatFeatureFlagBits_VK_FORMAT_FEATURE_BLIT_SRC_BIT,
                all,
                1
            ),
            Err(UNSUPPORTED)
        );
    }
}
