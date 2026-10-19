//! D3D12 render scopes: output-merger targets from validated view records and
//! explicit load/store/clear operations. No transition, initialization or
//! implicit viewport/scissor is added.
use super::*;
use windows::Win32::Foundation::RECT;

/// Scratch holds the native render-target handle array (one slot per color
/// target plus depth and stencil), sized per color count as the contract states.
pub(in crate::foundation) fn render_scratch(colors: u32) -> Result<HostRequirements, Status> {
    let slots = u64::from(colors) + 2;
    Ok(HostRequirements {
        size: slots * 8,
        alignment: 8,
    })
}

impl List {
    /// Begin a render scope: bind the attachment views as render/depth targets
    /// and apply explicit CLEAR load operations for the declared area.
    /// # Safety
    /// Attachment views and scratch stay live through all recorded/pending uses.
    pub(in crate::foundation) unsafe fn render_begin(&self, desc: &RenderDesc) -> Result<(), Status> {
        let d = self.recording()?;
        desc.header.validate::<RenderDesc>(RENDER_DESC)?;
        if desc.flags != 0 {
            // RENDER_SECONDARY maps to bundles/secondaries (D2 follow-up).
            return Err(UNSUPPORTED);
        }
        if desc.view_mask != 0 || desc.layers == 0 {
            return Err(UNSUPPORTED);
        }
        if desc.width == 0 || desc.height == 0 {
        return Err(INVALID);
        }
        if desc.x < 0 || desc.y < 0 {
        return Err(INVALID);
        }
        if desc.color_count > 8 {
            return Err(UNSUPPORTED);
        }
        let requirement = render_scratch(desc.color_count)?;
        if desc.scratch.size < requirement.size
            || (requirement.size != 0
                && (desc.scratch.data.is_null()
                    || desc.scratch.data as usize % requirement.alignment as usize != 0))
        {
        return Err(INVALID);
        }
        let domain = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?;
        if domain.flags & GRAPHICS == 0 {
            return Err(UNSUPPORTED);
        }
        // SAFETY: trusted color attachment array of the declared count.
        let colors = unsafe { super::slice(desc.colors, desc.color_count)? };
        // The handle array lives in caller scratch; borrowed for this call.
        let slots = desc.scratch.data.cast::<*mut std::ffi::c_void>();
        let mut count = 0usize;
        for (index, attachment) in colors.iter().enumerate() {
            validate_attachment(d, attachment, true)?;
            let view = unsafe { attachment.view.as_ref() }.ok_or(INVALID)?;
            // SAFETY: scratch is writable for the declared size.
            unsafe {
                slots
                    .add(index)
                    .write(view.render_target().ok_or(INVALID)?.ptr as *mut _);
            }
            count += 1;
        }
        let mut depth_handle = d3d::D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 };
        if !desc.depth.is_null() {
            // SAFETY: trusted optional depth record.
            let attachment = unsafe { &*desc.depth };
            validate_attachment(d, attachment, false)?;
            let view = unsafe { attachment.view.as_ref() }.ok_or(INVALID)?;
            depth_handle = view.depth_stencil().ok_or(INVALID)?;
        }
        if !desc.stencil.is_null() {
            // SAFETY: trusted optional stencil record.
            let attachment = unsafe { &*desc.stencil };
            validate_attachment(d, attachment, false)?;
            if !desc.depth.is_null() {
                // Paired records must name the same view and state.
                // SAFETY: trusted paired record.
                let depth = unsafe { &*desc.depth };
                if depth.view != attachment.view || depth.state != attachment.state {
        return Err(INVALID);
                }
            }
        }
        let has_depth = !desc.depth.is_null() || !desc.stencil.is_null();
        // SAFETY: live targets from validated views; borrowed for the scope.
        unsafe {
            self.command.OMSetRenderTargets(
                count as u32,
                Some(slots.cast()),
                false,
                if has_depth {
                    Some(ptr::from_ref(&depth_handle))
                } else {
                    None
                },
            );
        }
        let rect = RECT {
            left: desc.x,
            top: desc.y,
            right: desc.x + desc.width as i32,
            bottom: desc.y + desc.height as i32,
        };
        for attachment in colors {
            if attachment.load_op == CLEAR {
                let view = unsafe { attachment.view.as_ref() }.unwrap();
                // SAFETY: clear value and rect are validated above.
                unsafe {
                    self.command.ClearRenderTargetView(
                        view.render_target().unwrap(),
                        &attachment.clear.f32,
                        Some(&[rect]),
                    );
                }
            } else if attachment.load_op != LOAD && attachment.load_op != DONT_CARE {
        return Err(INVALID);
            }
        }
        if has_depth {
            // Depth-only or stencil-only records describe both sides by
            // falling back to each other. Resolution stays lazy: the absent
            // pointer must never be dereferenced.
            let depth = unsafe { desc.depth.as_ref() }
                .or_else(|| unsafe { desc.stencil.as_ref() })
                .ok_or(INVALID)?;
            let stencil = unsafe { desc.stencil.as_ref() }
                .or_else(|| unsafe { desc.depth.as_ref() })
                .ok_or(INVALID)?;
            // Clear flags must match the format's aspects: a depth-only view
            // rejects stencil clears and vice versa.
            // SAFETY: the record's view is validated live by validate_attachment.
            let depth_view = unsafe { depth.view.as_ref() }.ok_or(INVALID)?;
            let aspects = super::formats::meta(depth_view.desc.format).ok_or(UNSUPPORTED)?.aspects;
            let mut clear_flags = d3d::D3D12_CLEAR_FLAGS(0);
            if depth.load_op == CLEAR {
                if aspects & 2 != 0 {
                    clear_flags |= d3d::D3D12_CLEAR_FLAG_DEPTH;
                }
            } else if depth.load_op != LOAD && depth.load_op != DONT_CARE {
        return Err(INVALID);
            }
            if stencil.load_op == CLEAR {
                if aspects & 4 != 0 {
                    clear_flags |= d3d::D3D12_CLEAR_FLAG_STENCIL;
                }
            } else if stencil.load_op != LOAD && stencil.load_op != DONT_CARE {
        return Err(INVALID);
            }
            if clear_flags != d3d::D3D12_CLEAR_FLAGS(0) {
                let depth_clear = depth.clear;
                let value = unsafe { depth_clear.depth_stencil };
                if value.depth < 0.0 || value.depth > 1.0 {
        return Err(INVALID);
                }
                // SAFETY: validated depth target and clear value.
                unsafe {
                    self.command.ClearDepthStencilView(
                        depth_handle,
                        clear_flags,
                        value.depth,
                        value.stencil as u8,
                        Some(&[rect]),
                    );
                }
            }
        }
        self.rendering.set(true);
        Ok(())
    }

    /// End the render scope. D3D12 render targets are state, so no native call
    /// is required beyond clearing the recorded scope.
    pub(in crate::foundation) fn render_end(&self) -> Result<(), Status> {
        self.recording()?;
        if !self.rendering.replace(false) {
        return Err(INVALID);
        }
        Ok(())
    }
}

/// Attachment validation per the render contract (2D/array identity views,
/// one mip, matching state and usage).
fn validate_attachment(d: &Device, attachment: &Attachment, color: bool) -> Result<(), Status> {
    let expected_state = if color {
        STATE_GENERAL | STATE_COLOR_ATTACHMENT
    } else {
        STATE_GENERAL | STATE_DEPTH_STENCIL_ATTACHMENT | STATE_DEPTH_STENCIL_READ
    };
    if attachment.state & !expected_state != 0 || attachment.state == 0 {
        return Err(INVALID);
    }
    if color && attachment.state == STATE_DEPTH_STENCIL_READ {
        return Err(INVALID);
    }
    if attachment.resolve_view.is_null() != (attachment.resolve_mode == 0) {
        return Err(INVALID);
    }
    if !attachment.resolve_view.is_null() {
        // Resolve attachments follow with the multisample profile.
        return Err(UNSUPPORTED);
    }
    let view = unsafe { attachment.view.as_ref() }.ok_or(INVALID)?;
    if view.image.is_null() {
        return Err(INVALID);
    }
    // SAFETY: the view's image is live through every recorded use.
    let image = unsafe { &*view.image };
    if image.sample_count() != 1 {
        // Multisampled targets follow with the multisample profile.
        return Err(UNSUPPORTED);
    }
    if view.desc.range.mip_count != 1 {
        return Err(UNSUPPORTED);
    }
    if view.desc.range.aspects == 0 {
        return Err(INVALID);
    }
    if color && view.desc.usage & IMAGE_COLOR_ATTACHMENT as u32 == 0 {
        return Err(INVALID);
    }
    if !color && view.desc.usage & IMAGE_DEPTH_STENCIL_ATTACHMENT as u32 == 0 {
        return Err(INVALID);
    }
    if !color && attachment.load_op == CLEAR {
        // Read-only depth/stencil forbids CLEAR (enforced by usage too).
        if view.desc.usage & (IMAGE_DEPTH_STENCIL_ATTACHMENT as u32) == 0 {
        return Err(INVALID);
        }
    }
    let _ = d;
    Ok(())
}
