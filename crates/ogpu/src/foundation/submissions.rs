//! Selected C2 submission engine. One scratch span, original batch boundaries,
//! direct binary arrays and explicit acceptance through C and direct Rust.
//! Callers own live arrays/objects, queue exclusion, duplicate/pending legality,
//! and disjoint scratch/outcomes. No allocation, retained receipt or implicit wait.
use super::*;

impl SubmitOutcome {
    pub(in crate::foundation) fn rejected(status: Status) -> Self {
        Self {
            status,
            acceptance: SUBMIT_REJECTED,
        }
    }
    pub(super) fn result(self) -> Result<(), Status> {
        if self.status == OK {
            Ok(())
        } else {
            Err(self.status)
        }
    }
}

fn entry_layout(desc: &SubmitDesc) -> Result<(Layout, usize, usize), Status> {
    if desc.flags != 0 {
        return Err(UNSUPPORTED);
    }
    submit_layout(
        desc.list_count,
        desc.wait_count
            .checked_add(desc.binary_wait_count)
            .ok_or(INVALID)?,
        desc.signal_count
            .checked_add(desc.binary_signal_count)
            .ok_or(INVALID)?,
    )
}
fn layout(descs: &[SubmitDesc]) -> Result<Layout, Status> {
    let count = u32::try_from(descs.len()).map_err(|_| INVALID)?;
    let mut all = array_layout::<vk::VkSubmitInfo2>(count)?;
    for desc in descs {
        all = all.extend(entry_layout(desc)?.0).map_err(|_| INVALID)?.0;
    }
    Ok(all.pad_to_align())
}
pub(in crate::foundation) fn requirements(
    d: &Device,
    descs: &[SubmitDesc],
) -> Result<HostRequirements, Status> {
    d.ready()?;
    let layout = layout(descs)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}

pub(super) unsafe fn translate(
    queue: &Queue,
    d: &Device,
    desc: &SubmitDesc,
    storage: HostSpan,
) -> Result<vk::VkSubmitInfo2, Status> {
    let device = ptr::from_ref(d);
    let (layout, wait_offset, signal_offset) = entry_layout(desc)?;
    let wait_count = desc.wait_count + desc.binary_wait_count;
    let signal_count = desc.signal_count + desc.binary_signal_count;
    let lists = unsafe { slice(desc.lists, desc.list_count)? };
    let waits = unsafe { slice(desc.waits, desc.wait_count)? };
    let signals = unsafe { slice(desc.signals, desc.signal_count)? };
    let binary_waits = unsafe { slice(desc.binary_waits, desc.binary_wait_count)? };
    let binary_signals = unsafe { slice(desc.binary_signals, desc.binary_signal_count)? };
    let storage = scratch(
        storage.data,
        storage.size,
        HostRequirements {
            size: layout.size() as u64,
            alignment: layout.align() as u64,
        },
    )?;
    let commands = storage.cast::<vk::VkCommandBufferSubmitInfo>();
    // Null is permitted for empty arrays; those pointers are never accessed.
    let native_waits = storage
        .wrapping_add(wait_offset)
        .cast::<vk::VkSemaphoreSubmitInfo>();
    let native_signals = storage
        .wrapping_add(signal_offset)
        .cast::<vk::VkSemaphoreSubmitInfo>();
    for (i, pointer) in lists.iter().enumerate() {
        let list = unsafe { pointer.as_ref() }.ok_or(INVALID)?;
        if list.device != device
            || list.domain != queue.domain
            || list.state.get() != 2
            || list.level != 0
        {
            return Err(INVALID);
        }
        let info = vk::VkCommandBufferSubmitInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO,
            commandBuffer: list.command,
            deviceMask: 0,
            ..Default::default()
        };
        unsafe {
            commands.add(i).write(info);
        }
    }
    for (points, binaries, target) in [
        (waits, binary_waits, native_waits),
        (signals, binary_signals, native_signals),
    ] {
        for (i, point) in points.iter().enumerate() {
            let timeline = unsafe { point.point.timeline.as_ref() }.ok_or(INVALID)?;
            if timeline.device != device {
                return Err(INVALID);
            }
            let info = vk::VkSemaphoreSubmitInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
                semaphore: timeline.handle,
                value: point.point.value,
                stageMask: stages(d, queue.domain, point.stages, true)?,
                ..Default::default()
            };
            unsafe {
                target.add(i).write(info);
            }
        }
        for (i, point) in binaries.iter().enumerate() {
            let binary = unsafe { point.binary.as_ref() }.ok_or(INVALID)?;
            if binary.device != device {
                return Err(INVALID);
            }
            let info = vk::VkSemaphoreSubmitInfo {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
                semaphore: binary.handle,
                value: 0,
                stageMask: stages(d, queue.domain, point.stages, true)?,
                ..Default::default()
            };
            unsafe {
                target.add(points.len() + i).write(info);
            }
        }
    }
    Ok(vk::VkSubmitInfo2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO_2,
        commandBufferInfoCount: desc.list_count,
        pCommandBufferInfos: commands,
        waitSemaphoreInfoCount: wait_count,
        pWaitSemaphoreInfos: native_waits,
        signalSemaphoreInfoCount: signal_count,
        pSignalSemaphoreInfos: native_signals,
        ..Default::default()
    })
}

// The Vulkan OOM guarantee is specific to this native call. Do not infer these
// outcomes from a backend-neutral Status after a multi-call wait/execute/signal.
fn classify(native: vk::VkResult) -> SubmitOutcome {
    match native {
        vk::VkResult_VK_SUCCESS => SubmitOutcome {
            status: OK,
            acceptance: SUBMIT_ACCEPTED,
        },
        vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY | vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY => {
            SubmitOutcome::rejected(OUT_OF_MEMORY)
        }
        _ => SubmitOutcome {
            status: status(native).unwrap_err(),
            acceptance: SUBMIT_UNKNOWN,
        },
    }
}
pub(super) unsafe fn submit_native(
    queue: &Queue,
    d: &Device,
    count: u32,
    infos: *const vk::VkSubmitInfo2,
) -> SubmitOutcome {
    let result = classify(unsafe {
        (d.f.vkQueueSubmit2.unwrap())(queue.handle, count, infos, ptr::null_mut())
    });
    if result.acceptance == SUBMIT_UNKNOWN {
        d.lost.store(true, Ordering::Relaxed);
    }
    result
}
// Translation validated these lists. Accepted or uncertain one-shots may never
// be retried. Uncertain submission also poisons the device, including replay lists.
pub(super) unsafe fn consume(lists: *const *mut List, count: u32) {
    for i in 0..count as usize {
        let list = unsafe { &**lists.add(i) };
        if list.mode == 1 {
            list.state.set(3);
        }
    }
}
impl Queue {
    pub(in crate::foundation) unsafe fn submit_descriptions(
        &self,
        descs: &[SubmitDesc],
        outcomes: &mut [SubmitOutcome],
        storage: HostSpan,
    ) -> Result<(), Status> {
        let preflight_and_submit = || -> Result<SubmitOutcome, Status> {
            if outcomes.len() != descs.len() {
                return Err(INVALID);
            }
            let device = self.device.load(Ordering::Relaxed).cast_const();
            let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
            d.ready()?;
            let count = u32::try_from(descs.len()).map_err(|_| INVALID)?;
            if count == 0 {
                return Ok(SubmitOutcome {
                    status: OK,
                    acceptance: SUBMIT_ACCEPTED,
                });
            }
            let base = scratch(storage.data, storage.size, requirements(d, descs)?)?;
            let infos = base.cast::<vk::VkSubmitInfo2>();
            let mut used = array_layout::<vk::VkSubmitInfo2>(count)?;
            for (i, desc) in descs.iter().enumerate() {
                let nested = entry_layout(desc)?.0;
                let (next, offset) = used.extend(nested).map_err(|_| INVALID)?;
                unsafe {
                    infos.add(i).write(translate(
                        self,
                        d,
                        desc,
                        HostSpan {
                            data: base.add(offset).cast(),
                            size: nested.size() as u64,
                        },
                    )?);
                }
                used = next;
            }
            // No native effects or one-shot consumption until every entry passes.
            let outcome = unsafe { submit_native(self, d, count, infos) };
            if outcome.acceptance != SUBMIT_REJECTED {
                for desc in descs {
                    unsafe { consume(desc.lists, desc.list_count) };
                }
            }
            Ok(outcome)
        };
        // All local failures are whole-call rejections. One vkQueueSubmit2 gives
        // the same acceptance for every batch; never invent partial progress.
        let outcome = preflight_and_submit().unwrap_or_else(SubmitOutcome::rejected);
        outcomes.fill(outcome);
        outcome.result()
    }
}

#[cfg(test)]
#[path = "submission_tests.rs"]
mod tests;
