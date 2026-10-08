//! Explicit native command storage. No pending registry, resource retention,
//! hot-path allocator, implicit signal, or host wait.
use super::*;
use std::{alloc::Layout, cell::Cell, ffi::c_void};

pub struct Arena {
    device: *const Device,
    pool: vk::VkCommandPool,
    domain: u32,
    // Box addresses remain stable when the explicitly reserved slot array grows.
    #[allow(clippy::vec_box)]
    slots: Vec<Box<List>>,
    used: usize,
    usable: bool,
}
pub struct List {
    device: *const Device,
    command: vk::VkCommandBuffer,
    domain: u32,
    mode: u32,
    state: Cell<u32>, // 0 invalid, 1 recording, 2 executable, 3 consumed one-shot
    error: Cell<Status>,
}

impl Arena {
    pub(in crate::foundation) fn create(d: &Device, desc: &ArenaDesc) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<ArenaDesc>(ARENA_DESC)?;
        let queue = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == desc.domain && q.count > 0)
            .ok_or(INVALID)?;
        if queue.flags & (GRAPHICS | COMPUTE | TRANSFER) == 0 {
            return Err(UNSUPPORTED);
        }
        let mut result = Box::new(Self {
            device: d,
            pool: ptr::null_mut(),
            domain: desc.domain,
            slots: Vec::new(),
            used: 0,
            usable: true,
        });
        let info = vk::VkCommandPoolCreateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
            queueFamilyIndex: desc.domain,
            ..Default::default()
        };
        let mut pool = ptr::null_mut();
        // SAFETY: validated domain of the live device, synchronous output adoption.
        unsafe {
            d.result((d.f.vkCreateCommandPool.unwrap())(
                d.handle,
                &info,
                ptr::null(),
                &mut pool,
            ))?;
        }
        result.pool = pool;
        result.reserve(desc.list_capacity)?;
        Ok(result)
    }
    pub(in crate::foundation) fn reserve(&mut self, count: u32) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        let count = count as usize;
        if count <= self.slots.len() {
            return Ok(());
        }
        let extra = count - self.slots.len();
        self.slots
            .try_reserve_exact(extra)
            .map_err(|_| OUT_OF_MEMORY)?;
        let mut slots = Vec::new();
        slots.try_reserve_exact(extra).map_err(|_| OUT_OF_MEMORY)?;
        let mut commands = Vec::new();
        commands
            .try_reserve_exact(extra)
            .map_err(|_| OUT_OF_MEMORY)?;
        commands.resize(extra, ptr::null_mut());
        for _ in 0..extra {
            slots.push(Box::new(List {
                device: self.device,
                command: ptr::null_mut(),
                domain: self.domain,
                mode: 0,
                state: Cell::new(0),
                error: Cell::new(OK),
            }));
        }
        let info = vk::VkCommandBufferAllocateInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
            commandPool: self.pool,
            level: vk::VkCommandBufferLevel_VK_COMMAND_BUFFER_LEVEL_PRIMARY,
            commandBufferCount: extra as u32,
            ..Default::default()
        };
        // SAFETY: native allocation is all-or-nothing. Failure output isn't adopted.
        unsafe {
            d.result((d.f.vkAllocateCommandBuffers.unwrap())(
                d.handle,
                &info,
                commands.as_mut_ptr(),
            ))?;
        }
        for (slot, command) in slots.iter_mut().zip(commands) {
            slot.command = command;
        }
        self.slots.extend(slots);
        Ok(())
    }
    pub(in crate::foundation) fn reset(&mut self, release: bool) -> Result<(), Status> {
        // Invalidate even when reset fails: no potentially stale executable list.
        self.usable = false;
        for slot in &self.slots {
            slot.state.set(0);
        }
        let d = unsafe { &*self.device };
        d.ready()?;
        let flags = if release {
            vk::VkCommandPoolResetFlagBits_VK_COMMAND_POOL_RESET_RELEASE_RESOURCES_BIT
        } else {
            0
        };
        // SAFETY: the caller proves no pending use and excludes all pool access.
        unsafe {
            d.result((d.f.vkResetCommandPool.unwrap())(
                d.handle, self.pool, flags,
            ))?;
        }
        self.used = 0;
        self.usable = true;
        Ok(())
    }
    pub(in crate::foundation) fn trim(&mut self, retained: u32) -> Result<(), Status> {
        if retained as usize > self.slots.len() {
            return Err(INVALID);
        }
        self.reset(true)?;
        let d = unsafe { &*self.device };
        while self.slots.len() > retained as usize {
            let slot = self.slots.pop().unwrap();
            unsafe {
                (d.f.vkFreeCommandBuffers.unwrap())(d.handle, self.pool, 1, &slot.command);
            }
        }
        self.slots.shrink_to_fit();
        unsafe {
            (d.f.vkTrimCommandPool.unwrap())(d.handle, self.pool, 0);
        }
        Ok(())
    }
    pub(in crate::foundation) fn begin(
        &mut self,
        desc: &RecordingDesc,
    ) -> Result<*mut List, Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        desc.header.validate::<RecordingDesc>(RECORDING_DESC)?;
        if desc.level != 0 || !desc.inheritance.is_null() {
            return Err(UNSUPPORTED);
        }
        let flags = match desc.replay_mode {
            1 => vk::VkCommandBufferUsageFlagBits_VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
            2 => 0,
            3 => vk::VkCommandBufferUsageFlagBits_VK_COMMAND_BUFFER_USAGE_SIMULTANEOUS_USE_BIT,
            _ => return Err(INVALID),
        };
        if !self.usable {
            return Err(INVALID);
        }
        let slot = self.slots.get_mut(self.used).ok_or(CAPACITY)?;
        self.used += 1;
        slot.state.set(0);
        slot.error.set(OK);
        slot.mode = desc.replay_mode;
        let info = vk::VkCommandBufferBeginInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
            flags,
            ..Default::default()
        };
        unsafe {
            d.result((d.f.vkBeginCommandBuffer.unwrap())(slot.command, &info))?;
        }
        slot.state.set(1);
        Ok(ptr::from_mut(&mut **slot))
    }
}
impl Drop for Arena {
    fn drop(&mut self) {
        if !self.pool.is_null() {
            let d = unsafe { &*self.device };
            // SAFETY: caller excludes pending/host use. Deliberately no idle wait.
            unsafe {
                (d.f.vkDestroyCommandPool.unwrap())(d.handle, self.pool, ptr::null());
            }
        }
    }
}

fn stages(d: &Device, domain: u32, mask: u64, semaphore: bool) -> Result<u64, Status> {
    if mask & !511 != 0 {
        return Err(UNSUPPORTED);
    }
    if semaphore && mask & 256 != 0 {
        return Err(INVALID);
    }
    let flags = d
        .snapshot
        .queues
        .iter()
        .find(|q| q.domain == domain)
        .ok_or(INVALID)?
        .flags;
    if mask & (8 | 16 | 64 | 128) != 0
        && (flags & GRAPHICS == 0 || d.snapshot.features.enabled & RASTER == 0)
        || mask & 4 != 0 && flags & COMPUTE == 0
        || mask & 32 != 0 && flags & (COMPUTE | GRAPHICS) == 0
        || mask & 2 != 0 && flags & (COMPUTE | GRAPHICS | TRANSFER) == 0
    {
        return Err(UNSUPPORTED);
    }
    let bits = [
        vk::VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        vk::VK_PIPELINE_STAGE_2_ALL_TRANSFER_BIT,
        vk::VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,
        vk::VK_PIPELINE_STAGE_2_VERTEX_SHADER_BIT | vk::VK_PIPELINE_STAGE_2_VERTEX_INPUT_BIT,
        vk::VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT,
        vk::VK_PIPELINE_STAGE_2_DRAW_INDIRECT_BIT,
        vk::VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
        vk::VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT
            | vk::VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT,
        vk::VK_PIPELINE_STAGE_2_HOST_BIT,
    ];
    Ok(bits.iter().enumerate().fold(0, |v, (i, bit)| {
        v | if mask & (1 << i) != 0 { *bit } else { 0 }
    }))
}
fn access(mask: u64, stages: u64) -> Result<u64, Status> {
    if mask & !65535 != 0 {
        return Err(UNSUPPORTED);
    }
    // ALL_COMMANDS covers GPU, not host accesses. Generic read/write require a scope.
    let allowed = [
        511,
        511,
        3,
        3,
        1 | 4 | 8 | 16,
        1 | 4 | 8 | 16,
        256,
        256,
        1 | 32,
        1 | 8,
        1 | 8,
        1 | 4 | 8 | 16,
        1 | 64,
        1 | 64,
        1 | 128,
        1 | 128,
    ];
    let bits = [
        vk::VK_ACCESS_2_MEMORY_READ_BIT,
        vk::VK_ACCESS_2_MEMORY_WRITE_BIT,
        vk::VK_ACCESS_2_TRANSFER_READ_BIT,
        vk::VK_ACCESS_2_TRANSFER_WRITE_BIT,
        vk::VK_ACCESS_2_SHADER_READ_BIT,
        vk::VK_ACCESS_2_SHADER_WRITE_BIT,
        vk::VK_ACCESS_2_HOST_READ_BIT,
        vk::VK_ACCESS_2_HOST_WRITE_BIT,
        vk::VK_ACCESS_2_INDIRECT_COMMAND_READ_BIT,
        vk::VK_ACCESS_2_INDEX_READ_BIT,
        vk::VK_ACCESS_2_VERTEX_ATTRIBUTE_READ_BIT,
        vk::VK_ACCESS_2_UNIFORM_READ_BIT,
        vk::VK_ACCESS_2_COLOR_ATTACHMENT_READ_BIT,
        vk::VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT,
        vk::VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_READ_BIT,
        vk::VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
    ];
    let mut value = 0;
    for (i, bit) in bits.iter().enumerate() {
        if mask & (1 << i) != 0 {
            if stages & allowed[i] == 0 {
                return Err(INVALID);
            }
            value |= bit;
        }
    }
    Ok(value)
}

fn scoped_access(d: &Device, domain: u32, mask: u64, mut scope: u64) -> Result<u64, Status> {
    if scope & 1 != 0 {
        // ALL_COMMANDS expands only to operations supported by this family; it
        // doesn't make shader accesses valid on a transfer-only queue.
        let flags = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == domain)
            .ok_or(INVALID)?
            .flags;
        scope &= !1;
        if flags & (TRANSFER | COMPUTE | GRAPHICS) != 0 {
            scope |= 2;
        }
        if flags & COMPUTE != 0 {
            scope |= 4 | 32;
        }
        if flags & GRAPHICS != 0 {
            scope |= 8 | 16 | 32 | 64 | 128;
        }
    }
    access(mask, scope)
}

fn array_layout<T>(count: u32) -> Result<Layout, Status> {
    if count == 0 {
        return Ok(Layout::from_size_align(0, 1).unwrap());
    }
    Layout::array::<T>(count as usize).map_err(|_| INVALID)
}
fn submit_layout(lists: u32, waits: u32, signals: u32) -> Result<(Layout, usize, usize), Status> {
    let commands = array_layout::<vk::VkCommandBufferSubmitInfo>(lists)?;
    let (layout, wait_offset) = commands
        .extend(array_layout::<vk::VkSemaphoreSubmitInfo>(waits)?)
        .map_err(|_| INVALID)?;
    let (layout, signal_offset) = layout
        .extend(array_layout::<vk::VkSemaphoreSubmitInfo>(signals)?)
        .map_err(|_| INVALID)?;
    Ok((layout.pad_to_align(), wait_offset, signal_offset))
}
pub(in crate::foundation) fn submit_scratch(
    lists: u32,
    waits: u32,
    signals: u32,
) -> Result<HostRequirements, Status> {
    let (layout, _, _) = submit_layout(lists, waits, signals)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
fn barrier_layout(memory: u32, images: u32) -> Result<(Layout, usize), Status> {
    let (layout, offset) = array_layout::<vk::VkBufferMemoryBarrier2>(memory)?
        .extend(array_layout::<vk::VkImageMemoryBarrier2>(images)?)
        .map_err(|_| INVALID)?;
    Ok((layout.pad_to_align(), offset))
}
pub(in crate::foundation) fn barrier_scratch(
    memory: u32,
    images: u32,
) -> Result<HostRequirements, Status> {
    let (layout, _) = barrier_layout(memory, images)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
fn scratch(
    pointer: *mut c_void,
    size: u64,
    requirements: HostRequirements,
) -> Result<*mut u8, Status> {
    if size < requirements.size {
        return Err(CAPACITY);
    }
    if requirements.size != 0
        && (pointer.is_null() || pointer as usize % requirements.alignment as usize != 0)
    {
        return Err(INVALID);
    }
    Ok(pointer.cast())
}
unsafe fn slice<'a, T>(pointer: *const T, count: u32) -> Result<&'a [T], Status> {
    array_layout::<T>(count)?;
    if count == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(INVALID);
    }
    // SAFETY: trusted C array extent and alignment; size arithmetic checked above.
    Ok(unsafe { std::slice::from_raw_parts(pointer, count as usize) })
}

impl List {
    fn recording(&self) -> Result<&Device, Status> {
        if self.state.get() != 1 {
            return Err(INVALID);
        }
        if self.error.get() != OK {
            return Err(self.error.get());
        }
        let d = unsafe { &*self.device };
        d.ready()?;
        Ok(d)
    }
    pub(in crate::foundation) fn poison(&self, status: Status) {
        if status != OK && self.error.get() == OK {
            self.error.set(status);
        }
    }
    pub(in crate::foundation) fn end(&self) -> Result<(), Status> {
        let result = self
            .recording()
            .and_then(|d| unsafe { d.result((d.f.vkEndCommandBuffer.unwrap())(self.command)) });
        self.state.set(if result.is_ok() { 2 } else { 0 });
        result
    }
    pub(in crate::foundation) fn cancel(&self) {
        self.state.set(0);
    }
    pub(in crate::foundation) unsafe fn copy(&self, dst: Span, src: Span) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 2, false)?;
        if src.size != dst.size {
            return Err(INVALID);
        }
        let source = unsafe { src.memory.as_ref() }.ok_or(INVALID)?;
        let target = unsafe { dst.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, src_address) =
            source.command_range(self.device, self.domain, src.offset, src.size, 1)?;
        let (_, _, dst_address) =
            target.command_range(self.device, self.domain, dst.offset, dst.size, 2)?;
        if src.size == 0 {
            return Ok(());
        }
        if src_address.abs_diff(dst_address) < src.size {
            return Err(INVALID);
        }
        let region = vk::VkDeviceMemoryCopyKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_MEMORY_COPY_KHR,
            srcRange: vk::VkDeviceAddressRangeKHR {
                address: src_address,
                size: src.size,
            },
            dstRange: vk::VkDeviceAddressRangeKHR {
                address: dst_address,
                size: dst.size,
            },
            srcFlags: source.address_flags(),
            dstFlags: target.address_flags(),
            ..Default::default()
        };
        let info = vk::VkCopyDeviceMemoryInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_INFO_KHR,
            regionCount: 1,
            pRegions: &region,
            ..Default::default()
        };
        unsafe {
            (d.f.vkCmdCopyMemoryKHR.unwrap())(self.command, &info);
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn fill(&self, dst: Span, pattern: u32) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 2, false)?;
        let target = unsafe { dst.memory.as_ref() }.ok_or(INVALID)?;
        let (buffer, offset, _) =
            target.command_range(self.device, self.domain, dst.offset, dst.size, 2)?;
        if offset % 4 != 0 || dst.size % 4 != 0 {
            return Err(INVALID);
        }
        if dst.size != 0 {
            unsafe {
                (d.f.vkCmdFillBuffer.unwrap())(self.command, buffer, offset, dst.size, pattern);
            }
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn barrier(&self, dep: &Dependency) -> Result<(), Status> {
        let d = self.recording()?;
        dep.header.validate::<Dependency>(DEPENDENCY)?;
        if dep.flags != 0 {
            return Err(UNSUPPORTED);
        }
        let before = stages(d, self.domain, dep.before, false)?;
        let after = stages(d, self.domain, dep.after, false)?;
        let global = vk::VkMemoryBarrier2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
            srcStageMask: before,
            dstStageMask: after,
            srcAccessMask: scoped_access(d, self.domain, dep.global_before, dep.before)?,
            dstAccessMask: scoped_access(d, self.domain, dep.global_after, dep.after)?,
            ..Default::default()
        };
        let ranges = unsafe { slice(dep.memory, dep.memory_count)? };
        let storage = scratch(
            dep.scratch,
            dep.scratch_size,
            barrier_scratch(dep.memory_count, dep.image_count)?,
        )?
        .cast::<vk::VkBufferMemoryBarrier2>();
        for (i, range) in ranges.iter().enumerate() {
            let memory = unsafe { range.range.memory.as_ref() }.ok_or(INVALID)?;
            if range.range.size == 0 {
                return Err(INVALID);
            }
            let (buffer, offset, _) = memory.command_range(
                self.device,
                self.domain,
                range.range.offset,
                range.range.size,
                0,
            )?;
            let (src, dst) = (range.source_domain, range.destination_domain);
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
                    || (self.domain != src && self.domain != dst))
            {
                return Err(INVALID);
            }
            let native = vk::VkBufferMemoryBarrier2 {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER_2,
                srcStageMask: before,
                dstStageMask: after,
                srcAccessMask: scoped_access(d, self.domain, range.before, dep.before)?,
                dstAccessMask: scoped_access(d, self.domain, range.after, dep.after)?,
                srcQueueFamilyIndex: src,
                dstQueueFamilyIndex: dst,
                buffer,
                offset,
                size: range.range.size,
                ..Default::default()
            };
            unsafe {
                storage.add(i).write(native);
            }
        }
        let (_, image_offset) = barrier_layout(dep.memory_count, dep.image_count)?;
        let image_storage = storage
            .cast::<u8>()
            .wrapping_add(image_offset)
            .cast::<vk::VkImageMemoryBarrier2>();
        for (i, b) in unsafe { slice(dep.images, dep.image_count)? }
            .iter()
            .enumerate()
        {
            let image = unsafe { b.image.as_ref() }.ok_or(INVALID)?;
            let mut native = image.barrier(self.device, self.domain, b)?;
            native.srcStageMask = before;
            native.dstStageMask = after;
            native.srcAccessMask = scoped_access(d, self.domain, b.before, dep.before)?;
            native.dstAccessMask = scoped_access(d, self.domain, b.after, dep.after)?;
            unsafe {
                image_storage.add(i).write(native);
            }
        }
        let info = vk::VkDependencyInfo {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
            memoryBarrierCount: u32::from(
                (dep.memory_count == 0 && dep.image_count == 0)
                    || dep.global_before != 0
                    || dep.global_after != 0,
            ),
            pMemoryBarriers: &global,
            bufferMemoryBarrierCount: dep.memory_count,
            pBufferMemoryBarriers: storage,
            imageMemoryBarrierCount: dep.image_count,
            pImageMemoryBarriers: image_storage,
            ..Default::default()
        };
        unsafe {
            (d.f.vkCmdPipelineBarrier2.unwrap())(self.command, &info);
        }
        Ok(())
    }
    pub(in crate::foundation) fn clear_image(
        &self,
        image: &Image,
        state: u32,
        range: Subresources,
        value: ClearValue,
    ) -> Result<(), Status> {
        self.recording()?;
        image.record_clear(self.device, self.domain, self.command, state, range, value)
    }
    pub(in crate::foundation) unsafe fn copy_image_memory(
        &self,
        image: &Image,
        span: Span,
        copy: &ImageCopy,
        to_image: bool,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        stages(d, self.domain, 2, false)?;
        unsafe { image.record_copy(self.device, self.domain, self.command, span, copy, to_image) }
    }
}

impl Queue {
    pub(in crate::foundation) unsafe fn submit(&self, desc: &SubmitDesc) -> Result<(), Status> {
        let device = self.device.load(Ordering::Relaxed).cast_const();
        let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
        d.ready()?;
        desc.header.validate::<SubmitDesc>(SUBMIT_DESC)?;
        let lists = unsafe { slice(desc.lists, desc.list_count)? };
        let waits = unsafe { slice(desc.waits, desc.wait_count)? };
        let signals = unsafe { slice(desc.signals, desc.signal_count)? };
        let (_, wait_offset, signal_offset) =
            submit_layout(desc.list_count, desc.wait_count, desc.signal_count)?;
        let storage = scratch(
            desc.scratch,
            desc.scratch_size,
            submit_scratch(desc.list_count, desc.wait_count, desc.signal_count)?,
        )?;
        let commands = storage.cast::<vk::VkCommandBufferSubmitInfo>();
        // wrapping_add permits NULL for zero-size scratch; those pointers aren't accessed.
        let native_waits = storage
            .wrapping_add(wait_offset)
            .cast::<vk::VkSemaphoreSubmitInfo>();
        let native_signals = storage
            .wrapping_add(signal_offset)
            .cast::<vk::VkSemaphoreSubmitInfo>();
        for (i, pointer) in lists.iter().enumerate() {
            let list = unsafe { pointer.as_ref() }.ok_or(INVALID)?;
            if list.device != device || list.domain != self.domain || list.state.get() != 2 {
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
        for (points, target) in [(waits, native_waits), (signals, native_signals)] {
            for (i, point) in points.iter().enumerate() {
                let timeline = unsafe { point.point.timeline.as_ref() }.ok_or(INVALID)?;
                if timeline.device != device {
                    return Err(INVALID);
                }
                let info = vk::VkSemaphoreSubmitInfo {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
                    semaphore: timeline.handle,
                    value: point.point.value,
                    stageMask: stages(d, self.domain, point.stages, true)?,
                    ..Default::default()
                };
                unsafe {
                    target.add(i).write(info);
                }
            }
        }
        let info = vk::VkSubmitInfo2 {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_SUBMIT_INFO_2,
            commandBufferInfoCount: desc.list_count,
            pCommandBufferInfos: commands,
            waitSemaphoreInfoCount: desc.wait_count,
            pWaitSemaphoreInfos: native_waits,
            signalSemaphoreInfoCount: desc.signal_count,
            pSignalSemaphoreInfos: native_signals,
            ..Default::default()
        };
        // SAFETY: validated records; caller owns queue exclusion, pending/lifetime
        // rules, timeline order and scratch. No allocation or wait in this path.
        let result = unsafe {
            d.result((d.f.vkQueueSubmit2.unwrap())(
                self.handle,
                1,
                &info,
                ptr::null_mut(),
            ))
        };
        if let Err(error) = result {
            if error != OUT_OF_MEMORY {
                d.lost.store(true, Ordering::Relaxed);
            }
            return Err(error);
        }
        for pointer in lists {
            let list = unsafe { &**pointer };
            if list.mode == 1 {
                list.state.set(3);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    thread_local! {
        static FAILURE: Cell<u32> = const { Cell::new(0) };
        static SUBMITS: Cell<u32> = const { Cell::new(0) };
        static BEGIN: Cell<vk::PFN_vkBeginCommandBuffer> = const { Cell::new(None) };
        static END: Cell<vk::PFN_vkEndCommandBuffer> = const { Cell::new(None) };
        static RESET: Cell<vk::PFN_vkResetCommandPool> = const { Cell::new(None) };
        static ALLOCATE: Cell<vk::PFN_vkAllocateCommandBuffers> = const { Cell::new(None) };
        static SUBMIT: Cell<vk::PFN_vkQueueSubmit2> = const { Cell::new(None) };
    }
    unsafe extern "C" fn begin(
        command: vk::VkCommandBuffer,
        info: *const vk::VkCommandBufferBeginInfo,
    ) -> vk::VkResult {
        if FAILURE.get() == 1 {
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { BEGIN.get().unwrap()(command, info) }
    }
    unsafe extern "C" fn end(command: vk::VkCommandBuffer) -> vk::VkResult {
        if FAILURE.get() == 2 {
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { END.get().unwrap()(command) }
    }
    unsafe extern "C" fn reset(
        device: vk::VkDevice,
        pool: vk::VkCommandPool,
        flags: vk::VkCommandPoolResetFlags,
    ) -> vk::VkResult {
        if FAILURE.get() == 6 {
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { RESET.get().unwrap()(device, pool, flags) }
    }
    unsafe extern "C" fn allocate(
        device: vk::VkDevice,
        info: *const vk::VkCommandBufferAllocateInfo,
        out: *mut vk::VkCommandBuffer,
    ) -> vk::VkResult {
        if FAILURE.get() == 7 {
            // Native failure output must never be adopted, submitted or freed.
            unsafe {
                out.write(ptr::dangling_mut());
            }
            return vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        unsafe { ALLOCATE.get().unwrap()(device, info, out) }
    }
    unsafe extern "C" fn submit(
        queue: vk::VkQueue,
        count: u32,
        infos: *const vk::VkSubmitInfo2,
        fence: vk::VkFence,
    ) -> vk::VkResult {
        SUBMITS.set(SUBMITS.get() + 1);
        match FAILURE.get() {
            3 => vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            4 => vk::VkResult_VK_ERROR_DEVICE_LOST,
            5 => vk::VkResult_VK_ERROR_UNKNOWN,
            _ => unsafe { SUBMIT.get().unwrap()(queue, count, infos, fence) },
        }
    }

    #[test]
    #[ignore = "requires modern Vulkan; command failure adoption, retry, reset poisoning and terminal submit loss"]
    fn gpu_foundation_commands() {
        let instance = Arc::new(Instance::new().unwrap());
        let mut tested = 0;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let domain = snapshot
                .queues
                .iter()
                .find(|q| q.count > 0 && q.flags & (GRAPHICS | COMPUTE | TRANSFER) != 0)
                .unwrap()
                .domain;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let requests = [QueueRequest {
                domain,
                count: 1,
                priority: 0.5,
            }];
            let ad = ArenaDesc {
                header: Record::new::<ArenaDesc>(ARENA_DESC),
                domain,
                list_capacity: 1,
            };
            let rd = RecordingDesc {
                header: Record::new::<RecordingDesc>(RECORDING_DESC),
                replay_mode: 1,
                level: 0,
                inheritance: ptr::null(),
            };
            for terminal in [4, 5] {
                let mut d = Device::create(&adapter, &requests, 0).unwrap();
                // Install mocks before borrowing the device into any child.
                BEGIN.set(d.f.vkBeginCommandBuffer);
                d.f.vkBeginCommandBuffer = Some(begin);
                END.set(d.f.vkEndCommandBuffer);
                d.f.vkEndCommandBuffer = Some(end);
                RESET.set(d.f.vkResetCommandPool);
                d.f.vkResetCommandPool = Some(reset);
                ALLOCATE.set(d.f.vkAllocateCommandBuffers);
                d.f.vkAllocateCommandBuffers = Some(allocate);
                SUBMIT.set(d.f.vkQueueSubmit2);
                d.f.vkQueueSubmit2 = Some(submit);
                FAILURE.set(7);
                assert!(matches!(Arena::create(&d, &ad), Err(OUT_OF_MEMORY)));
                FAILURE.set(0);
                let mut arena = Arena::create(&d, &ad).unwrap();
                FAILURE.set(7);
                assert_eq!(arena.reserve(2), Err(OUT_OF_MEMORY));
                assert_eq!(arena.slots.len(), 1);
                FAILURE.set(1);
                assert_eq!(arena.begin(&rd), Err(OUT_OF_MEMORY));
                assert_eq!(arena.used, 1);
                FAILURE.set(0);
                arena.reset(false).unwrap();
                let encoder = arena.begin(&rd).unwrap();
                FAILURE.set(2);
                assert_eq!(unsafe { &*encoder }.end(), Err(OUT_OF_MEMORY));
                assert_eq!(unsafe { &*encoder }.state.get(), 0);
                FAILURE.set(0);
                arena.reset(false).unwrap();
                let list = arena.begin(&rd).unwrap();
                unsafe { &*list }.end().unwrap();
                let timeline = Timeline::create(&d, 0).unwrap();
                let point = SyncPoint {
                    point: Point {
                        timeline: ptr::from_ref(&*timeline).cast_mut(),
                        value: 1,
                    },
                    stages: 1,
                };
                let requirements = submit_scratch(1, 0, 1).unwrap();
                let mut scratch = vec![0u64; requirements.size.div_ceil(8) as usize];
                assert!(requirements.alignment <= align_of::<u64>() as u64);
                let mut desc = SubmitDesc {
                    header: Record::new::<SubmitDesc>(SUBMIT_DESC),
                    list_count: 1,
                    wait_count: 0,
                    signal_count: 1,
                    lists: &list,
                    waits: ptr::null(),
                    signals: &point,
                    scratch: scratch.as_mut_ptr().cast(),
                    scratch_size: requirements.size,
                };
                let queue = d.queue(domain, 0).unwrap();
                SUBMITS.set(0);
                FAILURE.set(3);
                assert_eq!(unsafe { queue.submit(&desc) }, Err(OUT_OF_MEMORY));
                assert_eq!(unsafe { &*list }.state.get(), 2);
                assert_eq!(d.ready(), Ok(()));
                desc.scratch_size = 0;
                assert_eq!(unsafe { queue.submit(&desc) }, Err(CAPACITY));
                assert_eq!(SUBMITS.get(), 1);
                desc.scratch_size = requirements.size;
                FAILURE.set(0);
                unsafe { queue.submit(&desc) }.unwrap();
                timeline.wait(1, 10_000_000_000).unwrap();
                assert_eq!(unsafe { queue.submit(&desc) }, Err(INVALID));
                assert_eq!(SUBMITS.get(), 2);
                FAILURE.set(6);
                assert_eq!(arena.reset(false), Err(OUT_OF_MEMORY));
                assert_eq!(arena.begin(&rd), Err(INVALID));
                // Internal state check only: public handles were invalidated by reset.
                assert_eq!(arena.slots[0].state.get(), 0);
                FAILURE.set(0);
                arena.reset(false).unwrap();
                let list = arena.begin(&rd).unwrap();
                unsafe { &*list }.end().unwrap();
                desc.lists = &list;
                desc.signal_count = 0;
                FAILURE.set(terminal);
                assert_eq!(
                    unsafe { queue.submit(&desc) },
                    Err(if terminal == 4 {
                        DEVICE_LOST
                    } else {
                        BACKEND_ERROR
                    })
                );
                assert_eq!(unsafe { queue.submit(&desc) }, Err(DEVICE_LOST));
                assert_eq!(SUBMITS.get(), 3);
                assert_eq!(arena.begin(&rd), Err(DEVICE_LOST));
                // The injected loss did not submit native work; real device is idle.
                drop(arena);
                drop(timeline);
                drop(d);
            }
            // Separate arenas permit independent simultaneous host recording.
            let d = Device::create(&adapter, &requests, 0).unwrap();
            std::thread::scope(|scope| {
                for _ in 0..4 {
                    let d = &d;
                    scope.spawn(move || {
                        let ad = ArenaDesc {
                            header: Record::new::<ArenaDesc>(ARENA_DESC),
                            domain,
                            list_capacity: 1,
                        };
                        let rd = RecordingDesc {
                            header: Record::new::<RecordingDesc>(RECORDING_DESC),
                            replay_mode: 2,
                            level: 0,
                            inheritance: ptr::null(),
                        };
                        let mut arena = Arena::create(d, &ad).unwrap();
                        for _ in 0..16 {
                            let list = arena.begin(&rd).unwrap();
                            unsafe { &*list }.end().unwrap();
                            arena.reset(false).unwrap();
                        }
                    });
                }
            });
            tested += 1;
        }
        assert_ne!(tested, 0, "No suitable device, not a passing skip");
    }
    #[test]
    fn scratch_layout_and_scope_contracts() {
        assert_eq!(
            submit_scratch(0, 0, 0),
            Ok(HostRequirements {
                size: 0,
                alignment: 1
            })
        );
        for (l, w, s) in [(1, 0, 0), (0, 1, 1), (3, 5, 7)] {
            let (layout, wo, so) = submit_layout(l, w, s).unwrap();
            assert!(wo >= l as usize * size_of::<vk::VkCommandBufferSubmitInfo>());
            assert!(so >= wo + w as usize * size_of::<vk::VkSemaphoreSubmitInfo>());
            assert!(layout.size() >= so + s as usize * size_of::<vk::VkSemaphoreSubmitInfo>());
            assert_eq!(wo % align_of::<vk::VkSemaphoreSubmitInfo>(), 0);
            assert_eq!(so % align_of::<vk::VkSemaphoreSubmitInfo>(), 0);
        }
        assert_eq!(access(64, 1), Err(INVALID));
        assert_eq!(access(64, 256), Ok(vk::VK_ACCESS_2_HOST_READ_BIT));
        assert_eq!(access(8, 0), Err(INVALID));
        assert_eq!(access(16, 2), Err(INVALID));
        assert_eq!(access(1 << 16, 1), Err(UNSUPPORTED));
        let req = submit_scratch(1, 0, 0).unwrap();
        assert_eq!(scratch(ptr::null_mut(), 0, req), Err(CAPACITY));
        assert_eq!(scratch(ptr::null_mut(), req.size, req), Err(INVALID));
    }
}
