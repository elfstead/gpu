//! Scratch-backed render scopes. No resource retention or implicit layouts/state.
use super::*;
use std::alloc::Layout;

pub(super) fn samples(value: u32, supported: u32) -> Result<(), Status> {
    if !value.is_power_of_two() || value > 64 {
        return Err(INVALID);
    }
    if value & supported == 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}

pub(in crate::foundation) fn render_scratch(colors: u32) -> Result<HostRequirements, Status> {
    if colors == 0 {
        return Ok(HostRequirements {
            size: 0,
            alignment: 1,
        });
    }
    let layout =
        Layout::array::<vk::VkRenderingAttachmentInfo>(colors as usize).map_err(|_| INVALID)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}

impl Device {
    pub(super) unsafe fn begin_render(
        &self,
        command: vk::VkCommandBuffer,
        domain: u32,
        desc: &RenderDesc,
    ) -> Result<(), Status> {
        desc.header.validate::<RenderDesc>(RENDER_DESC)?;
        if desc.view_mask != 0 || desc.flags != 0 {
            return Err(UNSUPPORTED);
        }
        let limits = &self.snapshot.graphics_limits;
        samples(desc.samples, 127)?;
        if desc.x < 0 || desc.y < 0 || desc.width == 0 || desc.height == 0 || desc.layers == 0 {
            return Err(INVALID);
        }
        if u64::from(desc.width) + desc.x as u64 > u64::from(limits.max_width)
            || u64::from(desc.height) + desc.y as u64 > u64::from(limits.max_height)
            || desc.layers > limits.max_layers
            || desc.color_count > limits.max_colors
        {
            return Err(UNSUPPORTED);
        }
        let req = render_scratch(desc.color_count)?;
        if desc.scratch.size < req.size {
            return Err(CAPACITY);
        }
        if req.size != 0
            && (desc.scratch.data.is_null()
                || desc.scratch.data as usize % req.alignment as usize != 0
                || desc.colors.is_null())
        {
            return Err(INVALID);
        }
        let output = desc.scratch.data.cast::<vk::VkRenderingAttachmentInfo>();
        for i in 0..desc.color_count as usize {
            let a = unsafe { &*desc.colors.add(i) };
            let native = unsafe { self.render_attachment(domain, desc, a, 1)? };
            unsafe {
                output.add(i).write(native);
            }
        }
        let depth = if desc.depth.is_null() {
            None
        } else {
            Some(unsafe { self.render_attachment(domain, desc, &*desc.depth, 2)? })
        };
        let stencil = if desc.stencil.is_null() {
            None
        } else {
            Some(unsafe { self.render_attachment(domain, desc, &*desc.stencil, 4)? })
        };
        if let (Some(d), Some(s)) = (&depth, &stencil) {
            if d.imageView != s.imageView
                || (!d.imageView.is_null() && d.imageLayout != s.imageLayout)
            {
                return Err(INVALID);
            }
        }
        let has_color = (0..desc.color_count as usize)
            .any(|i| unsafe { !(*output.add(i)).imageView.is_null() });
        if !has_color
            && depth.as_ref().is_none_or(|a| a.imageView.is_null())
            && stencil.as_ref().is_none_or(|a| a.imageView.is_null())
        {
            samples(desc.samples, limits.no_attachment_samples)?;
        }
        let info = vk::VkRenderingInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDERING_INFO,
            renderArea: vk::VkRect2D {
                offset: vk::VkOffset2D {
                    x: desc.x,
                    y: desc.y,
                },
                extent: vk::VkExtent2D {
                    width: desc.width,
                    height: desc.height,
                },
            },
            layerCount: desc.layers,
            colorAttachmentCount: desc.color_count,
            pColorAttachments: output,
            pDepthAttachment: depth.as_ref().map_or(ptr::null(), ptr::from_ref),
            pStencilAttachment: stencil.as_ref().map_or(ptr::null(), ptr::from_ref),
            ..Default::default()
        };
        unsafe {
            (self.f.vkCmdBeginRendering.unwrap())(command, &info);
        }
        Ok(())
    }
    unsafe fn render_attachment(
        &self,
        domain: u32,
        desc: &RenderDesc,
        a: &Attachment,
        aspect: u32,
    ) -> Result<vk::VkRenderingAttachmentInfo, Status> {
        if aspect != 1 && a.resolve_mode != 0 {
            return Err(UNSUPPORTED);
        }
        if a.resolve_mode == 0 && (!a.resolve_view.is_null() || a.resolve_state != 0) {
            return Err(INVALID);
        }
        if a.load_op > 2 || a.store_op > 1 {
            return Err(INVALID);
        }
        let mut info = vk::VkRenderingAttachmentInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO,
            loadOp: a.load_op,
            storeOp: a.store_op,
            ..Default::default()
        };
        if a.view.is_null() {
            if a.state != 0 || a.resolve_mode != 0 {
                return Err(INVALID);
            }
            return Ok(info);
        }
        let view = unsafe { &*a.view };
        info.imageView = view.attachment(self, domain, desc, a, aspect)?;
        if a.resolve_mode != 0 {
            if desc.samples == 1 || a.resolve_view.is_null() {
                return Err(INVALID);
            }
            let resolve = unsafe { &*a.resolve_view };
            info.resolveImageView = resolve.color_resolve(self, domain, desc, a, view)?;
            info.resolveMode = a.resolve_mode;
            info.resolveImageLayout = if a.resolve_state == 1 {
                vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL
            } else {
                vk::VkImageLayout_VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL
            };
        }
        info.imageLayout = match a.state {
            1 => vk::VkImageLayout_VK_IMAGE_LAYOUT_GENERAL,
            5 => vk::VkImageLayout_VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL,
            6 => vk::VkImageLayout_VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            7 => vk::VkImageLayout_VK_IMAGE_LAYOUT_DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            _ => return Err(INVALID),
        };
        if aspect == 2 && a.load_op == 1 {
            let clear = unsafe { a.clear.depth_stencil };
            if !clear.depth.is_finite() || !(0.0..=1.0).contains(&clear.depth) {
                return Err(INVALID);
            }
        }
        // C union is exactly the native four-word clear representation.
        info.clearValue = unsafe { std::mem::transmute::<ClearValue, vk::VkClearValue>(a.clear) };
        Ok(info)
    }
    pub(super) fn viewport(
        &self,
        command: vk::VkCommandBuffer,
        v: &ViewportState,
    ) -> Result<(), Status> {
        v.header.validate::<ViewportState>(VIEWPORT_STATE)?;
        if ![v.x, v.y, v.width, v.height, v.min_depth, v.max_depth]
            .iter()
            .all(|f| f.is_finite())
            || v.width <= 0.0
            || v.height == 0.0
            || !(0.0..=1.0).contains(&v.min_depth)
            || !(0.0..=1.0).contains(&v.max_depth)
            || v.scissor_x < 0
            || v.scissor_y < 0
            || u64::from(v.scissor_width) + v.scissor_x as u64 > i32::MAX as u64
            || u64::from(v.scissor_height) + v.scissor_y as u64 > i32::MAX as u64
        {
            return Err(INVALID);
        }
        let p = &self.snapshot.graphics_limits;
        if v.width > p.max_viewport[0] as f32
            || v.height.abs() > p.max_viewport[1] as f32
            || v.x < p.viewport_bounds[0]
            || v.x + v.width > p.viewport_bounds[1]
            || v.y.min(v.y + v.height) < p.viewport_bounds[0]
            || v.y.max(v.y + v.height) > p.viewport_bounds[1]
        {
            return Err(UNSUPPORTED);
        }
        let viewport = vk::VkViewport {
            x: v.x,
            y: v.y,
            width: v.width,
            height: v.height,
            minDepth: v.min_depth,
            maxDepth: v.max_depth,
        };
        let scissor = vk::VkRect2D {
            offset: vk::VkOffset2D {
                x: v.scissor_x,
                y: v.scissor_y,
            },
            extent: vk::VkExtent2D {
                width: v.scissor_width,
                height: v.scissor_height,
            },
        };
        unsafe {
            (self.f.vkCmdSetViewport.unwrap())(command, 0, 1, &viewport);
            (self.f.vkCmdSetScissor.unwrap())(command, 0, 1, &scissor);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_counts_are_exact() {
        assert_eq!(samples(4, 5), Ok(()));
        assert_eq!(samples(2, 5), Err(UNSUPPORTED));
        for n in [0, 3, 128, u32::MAX] {
            assert_eq!(samples(n, u32::MAX), Err(INVALID));
        }
    }
    #[test]
    fn rendering_scratch_has_no_fixed_color_capacity() {
        assert_eq!(
            render_scratch(0),
            Ok(HostRequirements {
                size: 0,
                alignment: 1
            })
        );
        for n in [1, 8, 16, 1024] {
            let r = render_scratch(n).unwrap();
            assert_eq!(
                r.size,
                u64::from(n) * size_of::<vk::VkRenderingAttachmentInfo>() as u64
            );
            assert_eq!(
                r.alignment,
                align_of::<vk::VkRenderingAttachmentInfo>() as u64
            );
        }
    }
}
