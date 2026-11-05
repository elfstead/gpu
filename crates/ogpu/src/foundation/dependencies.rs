//! C2 dependency engine. Per-entry native scopes and one caller-owned scratch
//! span; no stage union, endpoint registry, host allocation or state inference.
//! Records remain private until the coordinated public/consumer cutover.
//! Unsafe calls require readable arrays/live objects and suitably aligned,
//! writable scratch disjoint from every input. No endpoint matching, pending-use
//! tracking or asynchronous lifetime proof is inferred from the records.
use super::*;

pub(super) const DEPENDENCY_SPLIT: u32 = 1;
pub(super) const DEPENDENCY_OWNERSHIP: u32 = 2;
pub(super) const DEPENDENCY_BINARY: u32 = 4;
pub(super) const MEMORY_SCOPE_BYTES: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DependencyLimits {
    pub domain: u32,
    pub flags: u32,
    pub memory_scope: u32,
    pub reserved: u32,
    pub wait_stages: u64,
    pub signal_stages: u64,
    pub split_stages: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct BarrierScope {
    pub stages_before: u64,
    pub stages_after: u64,
    pub access_before: u64,
    pub access_after: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct MemoryBarrier {
    pub scope: BarrierScope,
    pub range: Span,
    pub source_domain: u32,
    pub destination_domain: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct ImageBarrier {
    pub scope: BarrierScope,
    pub image: *mut Image,
    pub range: Subresources,
    pub old_state: u32,
    pub new_state: u32,
    pub source_domain: u32,
    pub destination_domain: u32,
    pub discard: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct Dependency {
    pub global_count: u32,
    pub memory_count: u32,
    pub image_count: u32,
    pub flags: u32,
    pub global: *const BarrierScope,
    pub memory: *const MemoryBarrier,
    pub images: *const ImageBarrier,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct SplitDependency {
    pub split: *const Split,
    pub dependency: *const Dependency,
}

// Cold capability derivation uses exactly the validators used at recording and
// submission. Scope bits describe representability, not measured concurrency.
pub(super) fn limits(domain: u32, flags: u32, enabled: u64) -> DependencyLimits {
    let mut supported = 0;
    for bit in 0..23 {
        let scope = 1 << bit;
        if stage_features(enabled, scope).is_ok()
            && stage_mask(flags, enabled & RASTER != 0, scope, true).is_ok()
        {
            supported |= scope;
        }
    }
    let split = flags & (GRAPHICS | COMPUTE) != 0;
    DependencyLimits {
        domain,
        flags: DEPENDENCY_OWNERSHIP | DEPENDENCY_BINARY | if split { DEPENDENCY_SPLIT } else { 0 },
        memory_scope: MEMORY_SCOPE_BYTES,
        reserved: 0,
        wait_stages: supported,
        signal_stages: supported,
        split_stages: if split { supported } else { 0 },
    }
}

pub(super) fn layout(
    globals: u32,
    memory: u32,
    images: u32,
) -> Result<(Layout, usize, usize), Status> {
    let (layout, memory_offset) = array_layout::<vk::VkMemoryBarrier2>(globals)?
        .extend(array_layout::<vk::VkBufferMemoryBarrier2>(memory)?)
        .map_err(|_| INVALID)?;
    let (layout, image_offset) = layout
        .extend(array_layout::<vk::VkImageMemoryBarrier2>(images)?)
        .map_err(|_| INVALID)?;
    Ok((layout.pad_to_align(), memory_offset, image_offset))
}
fn requirements(layout: Layout) -> HostRequirements {
    HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    }
}
pub(super) fn barrier_requirements(
    d: &Device,
    globals: u32,
    memory: u32,
    images: u32,
) -> Result<HostRequirements, Status> {
    d.ready()?;
    Ok(requirements(layout(globals, memory, images)?.0))
}
fn entry_layout(dep: &Dependency) -> Result<Layout, Status> {
    if dep.flags != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(layout(dep.global_count, dep.memory_count, dep.image_count)?.0)
}
fn split_headers(count: u32) -> Result<(Layout, usize), Status> {
    array_layout::<vk::VkEvent>(count)?
        .extend(array_layout::<vk::VkDependencyInfo>(count)?)
        .map_err(|_| INVALID)
}
// Counts/flags only. Do not touch split handles or nested barrier arrays here.
unsafe fn split_layout(waits: &[SplitDependency]) -> Result<Layout, Status> {
    let (mut result, _) = split_headers(u32::try_from(waits.len()).map_err(|_| INVALID)?)?;
    for wait in waits {
        let dep = unsafe { wait.dependency.as_ref() }.ok_or(INVALID)?;
        result = result.extend(entry_layout(dep)?).map_err(|_| INVALID)?.0;
    }
    Ok(result.pad_to_align())
}
pub(super) unsafe fn split_requirements(
    d: &Device,
    waits: &[SplitDependency],
) -> Result<HostRequirements, Status> {
    d.ready()?;
    Ok(requirements(unsafe { split_layout(waits)? }))
}

fn scope(
    d: &Device,
    domain: u32,
    s: BarrierScope,
    split: bool,
) -> Result<vk::VkMemoryBarrier2, Status> {
    if split && (s.stages_before | s.stages_after) & 256 != 0 {
        return Err(INVALID);
    }
    Ok(vk::VkMemoryBarrier2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
        srcStageMask: stages(d, domain, s.stages_before, false)?,
        dstStageMask: stages(d, domain, s.stages_after, false)?,
        srcAccessMask: scoped_access(d, domain, s.access_before, s.stages_before)?,
        dstAccessMask: scoped_access(d, domain, s.access_after, s.stages_after)?,
        ..Default::default()
    })
}

// Shared with the currently installed translator until its public cutover.
// Preserve native buffer identity/range and exclusive ownership endpoints.
pub(super) unsafe fn buffer_barrier(
    d: &Device,
    domain: u32,
    range: Span,
    src: u32,
    dst: u32,
) -> Result<vk::VkBufferMemoryBarrier2, Status> {
    let memory = unsafe { range.memory.as_ref() }.ok_or(INVALID)?;
    if range.size == 0 {
        return Err(INVALID);
    }
    let (buffer, offset, _) = memory.command_range(d, domain, range.offset, range.size, 0)?;
    if (src != u32::MAX || dst != u32::MAX)
        && (src == u32::MAX
            || dst == u32::MAX
            || memory.concurrent()
            || !d
                .snapshot
                .queues
                .iter()
                .any(|q| q.domain == src && q.count != 0)
            || !d
                .snapshot
                .queues
                .iter()
                .any(|q| q.domain == dst && q.count != 0)
            || (domain != src && domain != dst))
    {
        return Err(INVALID);
    }
    Ok(vk::VkBufferMemoryBarrier2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER_2,
        srcQueueFamilyIndex: src,
        dstQueueFamilyIndex: dst,
        buffer,
        offset,
        size: range.size,
        ..Default::default()
    })
}

impl List {
    // Returned native pointers borrow storage only. The complete translation
    // succeeds before the caller can issue any native command.
    unsafe fn translate_entries(
        &self,
        dep: &Dependency,
        storage: HostSpan,
        split: bool,
    ) -> Result<vk::VkDependencyInfo, Status> {
        let d = self.outside_render()?;
        let allocation = entry_layout(dep)?;
        let base = scratch(storage.data, storage.size, requirements(allocation))?;
        let (_, memory_offset, image_offset) =
            layout(dep.global_count, dep.memory_count, dep.image_count)?;
        let globals = base.cast::<vk::VkMemoryBarrier2>();
        let memory = base
            .wrapping_add(memory_offset)
            .cast::<vk::VkBufferMemoryBarrier2>();
        let images = base
            .wrapping_add(image_offset)
            .cast::<vk::VkImageMemoryBarrier2>();
        for (i, entry) in unsafe { slice(dep.global, dep.global_count)? }
            .iter()
            .enumerate()
        {
            unsafe {
                globals.add(i).write(scope(d, self.domain, *entry, split)?);
            }
        }
        for (i, entry) in unsafe { slice(dep.memory, dep.memory_count)? }
            .iter()
            .enumerate()
        {
            let s = scope(d, self.domain, entry.scope, split)?;
            let mut native = unsafe {
                buffer_barrier(
                    d,
                    self.domain,
                    entry.range,
                    entry.source_domain,
                    entry.destination_domain,
                )?
            };
            native.srcStageMask = s.srcStageMask;
            native.dstStageMask = s.dstStageMask;
            native.srcAccessMask = s.srcAccessMask;
            native.dstAccessMask = s.dstAccessMask;
            unsafe {
                memory.add(i).write(native);
            }
        }
        for (i, entry) in unsafe { slice(dep.images, dep.image_count)? }
            .iter()
            .enumerate()
        {
            if entry.reserved != 0 {
                return Err(INVALID);
            }
            let s = scope(d, self.domain, entry.scope, split)?;
            let image = unsafe { entry.image.as_ref() }.ok_or(INVALID)?;
            // Only image identity/state fields are consumed by the existing image
            // validator. Remove this stack record when the public record migrates.
            let mut native = image.barrier(
                d,
                self.domain,
                &super::ImageBarrier {
                    image: entry.image,
                    range: entry.range,
                    before: 0,
                    after: 0,
                    old_state: entry.old_state,
                    new_state: entry.new_state,
                    source_domain: entry.source_domain,
                    destination_domain: entry.destination_domain,
                    discard: entry.discard,
                },
            )?;
            native.srcStageMask = s.srcStageMask;
            native.dstStageMask = s.dstStageMask;
            native.srcAccessMask = s.srcAccessMask;
            native.dstAccessMask = s.dstAccessMask;
            unsafe {
                images.add(i).write(native);
            }
        }
        Ok(vk::VkDependencyInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
            memoryBarrierCount: dep.global_count,
            pMemoryBarriers: if dep.global_count == 0 {
                ptr::null()
            } else {
                globals
            },
            bufferMemoryBarrierCount: dep.memory_count,
            pBufferMemoryBarriers: if dep.memory_count == 0 {
                ptr::null()
            } else {
                memory
            },
            imageMemoryBarrierCount: dep.image_count,
            pImageMemoryBarriers: if dep.image_count == 0 {
                ptr::null()
            } else {
                images
            },
            ..Default::default()
        })
    }
    pub(super) unsafe fn barrier_entries(
        &self,
        dep: &Dependency,
        storage: HostSpan,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        let info = unsafe { self.translate_entries(dep, storage, false)? };
        if dep.global_count != 0 || dep.memory_count != 0 || dep.image_count != 0 {
            unsafe {
                (d.f.vkCmdPipelineBarrier2.unwrap())(self.command, &info);
            }
        }
        Ok(())
    }
    pub(super) unsafe fn split_release_entries(
        &self,
        split: &Split,
        dep: &Dependency,
        storage: HostSpan,
    ) -> Result<(), Status> {
        let d = self.split_recording()?;
        split.check(d)?;
        let info = unsafe { self.translate_entries(dep, storage, true)? };
        unsafe {
            (d.f.vkCmdSetEvent2.unwrap())(self.command, split.handle, &info);
        }
        Ok(())
    }
    pub(super) unsafe fn split_acquire_entries(
        &self,
        waits: &[SplitDependency],
        storage: HostSpan,
    ) -> Result<(), Status> {
        let d = self.split_recording()?;
        let allocation = unsafe { split_layout(waits)? };
        let base = scratch(storage.data, storage.size, requirements(allocation))?;
        if waits.is_empty() {
            return Ok(());
        }
        let count = u32::try_from(waits.len()).map_err(|_| INVALID)?;
        let (mut cursor, info_offset) = split_headers(count)?;
        let events = base.cast::<vk::VkEvent>();
        let infos = base
            .wrapping_add(info_offset)
            .cast::<vk::VkDependencyInfo>();
        for (i, wait) in waits.iter().enumerate() {
            let split = unsafe { wait.split.as_ref() }.ok_or(INVALID)?;
            let dep = unsafe { wait.dependency.as_ref() }.ok_or(INVALID)?;
            split.check(d)?;
            let entry_layout = entry_layout(dep)?;
            let (next, offset) = cursor.extend(entry_layout).map_err(|_| INVALID)?;
            cursor = next;
            let region = HostSpan {
                data: base.wrapping_add(offset).cast(),
                size: entry_layout.size() as u64,
            };
            let info = unsafe { self.translate_entries(dep, region, true)? };
            unsafe {
                events.add(i).write(split.handle);
                infos.add(i).write(info);
            }
        }
        unsafe {
            (d.f.vkCmdWaitEvents2.unwrap())(self.command, count, events, infos);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "dependency_tests.rs"]
mod tests;
