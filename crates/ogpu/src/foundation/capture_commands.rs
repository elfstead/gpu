//! Capture commands lower borrowed spans into caller scratch, then emit one command.
use super::*;
impl List {
    unsafe fn capture_infos(
        &self,
        first: u32,
        count: u32,
        spans: *const Span,
        workspace: HostSpan,
        counters: bool,
    ) -> Result<*const vk::VkBindTransformFeedbackBuffer2InfoEXT, Status> {
        let d = self.recording()?;
        stages(d, self.domain, CAPTURE_STAGE, false)?;
        capture::range(first, count, d.snapshot.capture_limits.max_buffers)?;
        let spans = unsafe { slice(spans, count)? };
        let output = scratch(workspace.data, workspace.size, capture::scratch(count)?)?
            .cast::<vk::VkBindTransformFeedbackBuffer2InfoEXT>();
        for (i, s) in spans.iter().enumerate() {
            let mut info = vk::VkBindTransformFeedbackBuffer2InfoEXT {
                sType:
                    vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_TRANSFORM_FEEDBACK_BUFFER_2_INFO_EXT,
                ..Default::default()
            };
            if !s.memory.is_null() {
                let m = unsafe { &*s.memory };
                let (_, _, address) = m.command_range(
                    d,
                    self.domain,
                    s.offset,
                    s.size,
                    if counters {
                        CAPTURE_COUNTER_USAGE
                    } else {
                        CAPTURE_USAGE
                    },
                )?;
                if address % 4 != 0 || counters && s.size < 4 {
                    return Err(INVALID);
                }
                if !counters && s.size > d.snapshot.capture_limits.max_buffer_size {
                    return Err(UNSUPPORTED);
                }
                info.addressRange = vk::VkDeviceAddressRangeKHR {
                    address,
                    size: s.size,
                };
                info.addressFlags = m.capture_address_flags();
            } else if s.offset != 0 || s.size != 0 {
                return Err(INVALID);
            }
            unsafe {
                output.add(i).write(info);
            }
        }
        Ok(output)
    }
    pub(in crate::foundation) unsafe fn bind_capture(
        &self,
        first: u32,
        count: u32,
        spans: *const Span,
        workspace: HostSpan,
    ) -> Result<(), Status> {
        if self.capture_active.get() {
            return Err(INVALID);
        }
        let p = unsafe { self.capture_infos(first, count, spans, workspace, false)? };
        let d = unsafe { &*self.device };
        if count != 0 {
            unsafe {
                (d.capture_commands.bind.ok_or(UNSUPPORTED)?)(self.command, first, count, p);
            }
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn capture(
        &self,
        begin: bool,
        first: u32,
        count: u32,
        counters: *const Span,
        workspace: HostSpan,
    ) -> Result<(), Status> {
        let d = self.drawing(false)?;
        if self.capture_active.get() == begin || self.view_mask.get() != 0 {
            return Err(INVALID);
        }
        let p = unsafe { self.capture_infos(first, count, counters, workspace, true)? };
        let command = if begin {
            d.capture_commands.begin
        } else {
            d.capture_commands.end
        }
        .ok_or(UNSUPPORTED)?;
        unsafe {
            command(self.command, first, count, p);
        }
        self.capture_active.set(begin);
        Ok(())
    }
    pub(in crate::foundation) unsafe fn draw_byte_count(
        &self,
        counter: Span,
        offset: u32,
        stride: u32,
        instances: u32,
        first_instance: u32,
    ) -> Result<(), Status> {
        let d = self.drawing(false)?;
        if d.snapshot.features.enabled & TRANSFORM_FEEDBACK == 0
            || d.snapshot.capture_limits.draw_byte_count == 0
        {
            return Err(UNSUPPORTED);
        }
        if offset % 4 != 0
            || stride % 4 != 0
            || stride == 0
            || stride > d.snapshot.capture_limits.max_stride
            || counter.size < 4
        {
            return Err(INVALID);
        }
        features::instance_range(
            self.view_mask.get(),
            d.snapshot.graphics_limits.max_multiview_instance_index,
            first_instance,
            instances,
        )?;
        let m = unsafe { counter.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, address) = m.command_range(d, self.domain, counter.offset, counter.size, 4)?;
        if address % 4 != 0 {
            return Err(INVALID);
        }
        let info = vk::VkBindTransformFeedbackBuffer2InfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_TRANSFORM_FEEDBACK_BUFFER_2_INFO_EXT,
            addressRange: vk::VkDeviceAddressRangeKHR {
                address,
                size: counter.size,
            },
            addressFlags: m.capture_address_flags(),
            ..Default::default()
        };
        unsafe {
            (d.capture_commands.draw.ok_or(UNSUPPORTED)?)(
                self.command,
                instances,
                first_instance,
                &info,
                offset,
                stride,
            );
        }
        Ok(())
    }
}
