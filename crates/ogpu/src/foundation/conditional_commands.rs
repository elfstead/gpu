use super::*;
impl List {
    pub(in crate::foundation) unsafe fn conditional_begin(
        &self,
        predicate: Span,
        flags: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, CONDITIONAL_STAGE, false)?;
        if flags & !CONDITIONAL_INVERTED != 0 {
            return Err(UNSUPPORTED);
        }
        if self.conditional.get().is_some() || predicate.size < 4 {
            return Err(INVALID);
        }
        let memory = unsafe { predicate.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, address) = memory.command_range(
            d,
            self.domain,
            predicate.offset,
            predicate.size,
            CONDITIONAL_USAGE,
        )?;
        if address % 4 != 0 {
            return Err(INVALID);
        }
        let info = vk::VkConditionalRenderingBeginInfo2EXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_CONDITIONAL_RENDERING_BEGIN_INFO_2_EXT,
            addressRange: vk::VkDeviceAddressRangeKHR { address, size: 4 },
            addressFlags: memory.capture_address_flags(),
            flags,
            ..Default::default()
        };
        unsafe {
            (d.execution_controls.begin.ok_or(UNSUPPORTED)?)(self.command, &info);
        }
        self.conditional.set(Some(self.rendering.get()));
        Ok(())
    }
    pub(in crate::foundation) fn conditional_end(&self) -> Result<(), Status> {
        let d = self.recording()?;
        if self.conditional.get() != Some(self.rendering.get()) {
            return Err(INVALID);
        }
        unsafe {
            (d.execution_controls.end.ok_or(UNSUPPORTED)?)(self.command);
        }
        self.conditional.set(None);
        Ok(())
    }
}
