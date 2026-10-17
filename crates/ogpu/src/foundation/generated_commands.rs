//! One native generated-command operation per recording call; no metadata cache.
use super::*;

impl List {
    unsafe fn generated_info(
        &self,
        batch: &GeneratedBatch,
        pipeline: &mut vk::VkGeneratedCommandsPipelineInfoEXT,
    ) -> Result<vk::VkGeneratedCommandsInfoEXT, Status> {
        let d = self.recording()?;
        let (layout, executable, set) = unsafe { generated::work(d, &batch.work)? };
        stages(d, self.domain, layout.stages, false)?;
        let needed = u64::from(batch.work.max_sequences - 1)
            .checked_mul(u64::from(layout.stride))
            .and_then(|n| n.checked_add(layout.extent))
            .ok_or(INVALID)?;
        if batch.arguments.size == 0 || batch.arguments.size < needed {
            return Err(INVALID);
        }
        let address = |span: Span, usage, optional| -> Result<u64, Status> {
            if optional && span.memory.is_null() {
                return if span.offset == 0 && span.size == 0 {
                    Ok(0)
                } else {
                    Err(INVALID)
                };
            }
            let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
            let (_, _, address) =
                memory.command_range(d, self.domain, span.offset, span.size, usage)?;
            if address % 4 != 0 {
                return Err(INVALID);
            }
            Ok(address)
        };
        let indirect = address(batch.arguments, 4, false)?;
        let preprocess = address(batch.preprocess, PREPROCESS_USAGE, true)?;
        let count = address(batch.count, 4, true)?;
        if count != 0 && batch.count.size < 4 {
            return Err(INVALID);
        }
        *pipeline = vk::VkGeneratedCommandsPipelineInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_GENERATED_COMMANDS_PIPELINE_INFO_EXT,
            pipeline: executable.pipeline,
            ..Default::default()
        };
        Ok(vk::VkGeneratedCommandsInfoEXT {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_GENERATED_COMMANDS_INFO_EXT,
            pNext: if set.is_null() {
                ptr::from_ref(pipeline).cast()
            } else {
                ptr::null()
            },
            shaderStages: generated::native_stages(layout.stages),
            indirectExecutionSet: set,
            indirectCommandsLayout: layout.handle,
            indirectAddress: indirect,
            indirectAddressSize: batch.arguments.size,
            preprocessAddress: preprocess,
            preprocessSize: batch.preprocess.size,
            maxSequenceCount: batch.work.max_sequences,
            sequenceCountAddress: count,
            maxDrawCount: batch.work.max_draws,
        })
    }
    pub(in crate::foundation) unsafe fn generated_preprocess(
        &self,
        batch: &GeneratedBatch,
        state: &List,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        if self.level != 0 || self.device != state.device {
            return Err(INVALID);
        }
        let (layout, initial, _) = unsafe { generated::work(d, &batch.work)? };
        if layout.flags & 1 == 0 || !ptr::eq(state.executable(layout.stages)?, initial) {
            return Err(INVALID);
        }
        let mut pipeline = Default::default();
        let info = unsafe { self.generated_info(batch, &mut pipeline)? };
        unsafe {
            (generated::functions(d)?
                .vkCmdPreprocessGeneratedCommandsEXT
                .unwrap())(self.command, &info, state.command)
        };
        Ok(())
    }
    pub(in crate::foundation) unsafe fn generated_execute(
        &self,
        batch: &GeneratedBatch,
        preprocessed: u32,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        let (layout, initial, _) = unsafe { generated::work(d, &batch.work)? };
        if preprocessed > 1 || preprocessed != layout.flags & 1 || self.level != 0 {
            return Err(INVALID);
        }
        if !ptr::eq(self.executable(layout.stages)?, initial) {
            return Err(INVALID);
        }
        if layout.stages == 4 {
            self.outside_render()?;
        } else if layout.stages & MESH_STAGE != 0 {
            self.mesh_drawing()?;
        } else {
            self.drawing(matches!(layout.action, 6 | 9))?;
        }
        let mut pipeline = Default::default();
        let info = unsafe { self.generated_info(batch, &mut pipeline)? };
        unsafe {
            (generated::functions(d)?
                .vkCmdExecuteGeneratedCommandsEXT
                .unwrap())(self.command, preprocessed, &info)
        };
        // Native execution-set tokens invalidate bindings. Do not restore state by
        // recording extra commands; subsequent ordinary work must explicitly bind.
        if layout.selection {
            if layout.stages == 4 {
                self.executable.set(ptr::null());
            } else {
                self.graphics.set(ptr::null());
            }
        }
        Ok(())
    }
}
