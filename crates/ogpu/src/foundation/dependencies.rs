//! C2 dependency engine. Per-entry native scopes and one caller-owned scratch
//! span; no stage union, endpoint registry, host allocation or state inference.
//! Unsafe calls require readable arrays/live objects and suitably aligned,
//! writable scratch disjoint from every input. No endpoint matching, pending-use
//! tracking or asynchronous lifetime proof is inferred from the records.
use super::*;

// Cold capability derivation uses exactly the validators used at recording and
// submission. Scope bits describe representability, not measured concurrency.
pub(in crate::foundation) fn limits(
    domain: u32,
    flags: u32,
    enabled: FeatureBits,
) -> DependencyLimits {
    let mut supported = 0;
    for bit in 0..27 {
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
fn sample_layout(
    globals: u32,
    memory: u32,
    images: u32,
    samples: bool,
) -> Result<(Layout, usize, usize, usize), Status> {
    let (base, mo, io) = layout(globals, memory, images)?;
    if !samples || images == 0 {
        return Ok((base, mo, io, 0));
    }
    let (result, offset) = base
        .extend(array_layout::<vk::VkSampleLocationsInfoEXT>(images)?)
        .map_err(|_| INVALID)?;
    Ok((result.pad_to_align(), mo, io, offset))
}
fn has_sample_locations(d: &Device) -> bool {
    d.snapshot.features.enabled & SAMPLE_LOCATIONS != 0
}
fn requirements(layout: Layout) -> HostRequirements {
    HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    }
}
pub(in crate::foundation) fn barrier_requirements(
    d: &Device,
    globals: u32,
    memory: u32,
    images: u32,
) -> Result<HostRequirements, Status> {
    d.ready()?;
    Ok(requirements(
        sample_layout(globals, memory, images, has_sample_locations(d))?.0,
    ))
}
fn entry_layout(dep: &Dependency, samples: bool) -> Result<Layout, Status> {
    if dep.flags & !(DEPENDENCY_BY_REGION | DEPENDENCY_VIEW_LOCAL) != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(sample_layout(dep.global_count, dep.memory_count, dep.image_count, samples)?.0)
}
fn split_headers(count: u32) -> Result<(Layout, usize), Status> {
    array_layout::<vk::VkEvent>(count)?
        .extend(array_layout::<vk::VkDependencyInfo>(count)?)
        .map_err(|_| INVALID)
}
// Counts/flags only. Do not touch split handles or nested barrier arrays here.
unsafe fn split_layout(waits: &[SplitDependency], samples: bool) -> Result<Layout, Status> {
    let (mut result, _) = split_headers(u32::try_from(waits.len()).map_err(|_| INVALID)?)?;
    for wait in waits {
        let dep = unsafe { wait.dependency.as_ref() }.ok_or(INVALID)?;
        if dep.flags != 0 {
            return Err(UNSUPPORTED);
        }
        result = result
            .extend(entry_layout(dep, samples)?)
            .map_err(|_| INVALID)?
            .0;
    }
    Ok(result.pad_to_align())
}
pub(in crate::foundation) unsafe fn split_requirements(
    d: &Device,
    waits: &[SplitDependency],
) -> Result<HostRequirements, Status> {
    d.ready()?;
    Ok(requirements(unsafe {
        split_layout(waits, has_sample_locations(d))?
    }))
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

fn local_context(
    enabled: FeatureBits,
    inside: bool,
    secondary_contents: bool,
    views: u32,
    flags: u32,
) -> Result<(), Status> {
    if flags & !(DEPENDENCY_BY_REGION | DEPENDENCY_VIEW_LOCAL) != 0 {
        return Err(UNSUPPORTED);
    }
    if inside {
        if enabled & LOCAL_READ == 0 {
            return Err(UNSUPPORTED);
        }
        if secondary_contents || (views.count_ones() > 1 && flags & DEPENDENCY_VIEW_LOCAL == 0) {
            return Err(INVALID);
        }
    } else if flags & DEPENDENCY_VIEW_LOCAL != 0 {
        return Err(INVALID);
    }
    Ok(())
}
fn local_stages(s: BarrierScope, flags: u32) -> Result<(), Status> {
    const FRAMEBUFFER: u64 = 16 | 64 | 128 | EARLY_SCOPE | LATE_SCOPE;
    if (s.stages_before | s.stages_after) & !FRAMEBUFFER != 0
        || (s.stages_before != 0 && flags & DEPENDENCY_BY_REGION == 0)
    {
        return Err(INVALID);
    }
    Ok(())
}
fn local_scope(
    d: &Device,
    domain: u32,
    s: BarrierScope,
    split: bool,
    inside: bool,
    flags: u32,
) -> Result<vk::VkMemoryBarrier2, Status> {
    if inside {
        local_stages(s, flags)?;
    }
    scope(d, domain, s, split)
}

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
        let d = self.recording()?;
        if split && dep.flags != 0 {
            return Err(UNSUPPORTED);
        }
        let inside = self.rendering.get();
        local_context(
            d.snapshot.features.enabled,
            inside,
            self.secondary_contents.get(),
            self.view_mask.get(),
            dep.flags,
        )?;
        let samples = has_sample_locations(d);
        let allocation = entry_layout(dep, samples)?;
        let base = scratch(storage.data, storage.size, requirements(allocation))?;
        let (_, memory_offset, image_offset, sample_offset) =
            sample_layout(dep.global_count, dep.memory_count, dep.image_count, samples)?;
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
                globals.add(i).write(local_scope(
                    d,
                    self.domain,
                    *entry,
                    split,
                    inside,
                    dep.flags,
                )?);
            }
        }
        for (i, entry) in unsafe { slice(dep.memory, dep.memory_count)? }
            .iter()
            .enumerate()
        {
            let s = local_scope(d, self.domain, entry.scope, split, inside, dep.flags)?;
            if inside && entry.source_domain != entry.destination_domain {
                return Err(INVALID);
            }
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
            let s = local_scope(d, self.domain, entry.scope, split, inside, dep.flags)?;
            if inside
                && (entry.old_state != entry.new_state
                    || entry.discard != 0
                    || entry.source_domain != entry.destination_domain)
            {
                return Err(INVALID);
            }
            let image = unsafe { entry.image.as_ref() }.ok_or(INVALID)?;
            let mut native = image.barrier(d, self.domain, entry)?;
            if !entry.next.is_null() {
                let sample = unsafe { image.sample_transition(entry.next)? };
                let out = base
                    .wrapping_add(sample_offset)
                    .cast::<vk::VkSampleLocationsInfoEXT>()
                    .wrapping_add(i);
                unsafe {
                    out.write(sample);
                }
                native.pNext = out.cast();
            }
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
            dependencyFlags: dep.flags,
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
    pub(in crate::foundation) unsafe fn barrier(
        &self,
        dep: &Dependency,
        storage: HostSpan,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        let info = unsafe { self.translate_entries(dep, storage, false)? };
        if dep.global_count != 0 || dep.memory_count != 0 || dep.image_count != 0 {
            unsafe {
                (d.f.vkCmdPipelineBarrier2.unwrap())(self.command, &info);
            }
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn split_release(
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
    pub(in crate::foundation) unsafe fn split_acquire(
        &self,
        waits: &[SplitDependency],
        storage: HostSpan,
    ) -> Result<(), Status> {
        let d = self.split_recording()?;
        let allocation = unsafe { split_layout(waits, has_sample_locations(d))? };
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
            let entry_layout = entry_layout(dep, has_sample_locations(d))?;
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

#[cfg(test)]
mod local_tests {
    use super::*;
    #[test]
    fn framebuffer_dependency_contract() {
        assert_eq!(local_context(0, true, false, 0, 1), Err(UNSUPPORTED));
        assert_eq!(local_context(LOCAL_READ, true, true, 0, 1), Err(INVALID));
        assert_eq!(local_context(LOCAL_READ, true, false, 5, 1), Err(INVALID));
        assert_eq!(local_context(LOCAL_READ, true, false, 8, 1), Ok(()));
        assert_eq!(local_context(LOCAL_READ, true, false, 5, 3), Ok(()));
        assert_eq!(local_context(LOCAL_READ, false, false, 0, 2), Err(INVALID));
        assert_eq!(local_context(0, false, false, 0, 1), Ok(()));
        let mut s = BarrierScope {
            stages_before: 64,
            stages_after: 16,
            access_before: 0,
            access_after: 0,
        };
        assert_eq!(local_stages(s, 1), Ok(()));
        assert_eq!(local_stages(s, 0), Err(INVALID));
        for stage in [1, 2, 4, 8, GRAPHICS_SCOPE, SHADING_RATE_STAGE] {
            s.stages_before = stage;
            assert_eq!(local_stages(s, 1), Err(INVALID));
        }
        s.stages_before = 0;
        assert_eq!(local_stages(s, 0), Ok(()));
    }
}
