//! Explicit D3D12 command storage and submission. No pending registry, resource
//! retention, hot-path allocator, implicit signal or host wait. Caller-provided
//! scratch holds every translated native record.
use super::*;

/// Arena: an explicit slot pool of command allocators and lists. Capacity counts
/// primary list slots exactly as in the shared contract; native storage grows
/// only through reserve/trim.
pub struct Arena {
    device: *const Device,
    domain: u32,
    level: u32,
    list_type: d3d::D3D12_COMMAND_LIST_TYPE,
    slots: Vec<Box<List>>,
    used: usize,
    usable: bool,
}

pub struct List {
    pub(super) device: *const Device,
    pub(super) domain: u32,
    level: u32,
    mode: u32,
    state: Cell<u32>, // 0 invalid, 1 recording, 2 executable, 3 consumed one-shot
    error: Cell<Status>,
    allocator: d3d::ID3D12CommandAllocator,
    pub(super) command: d3d::ID3D12GraphicsCommandList,
    /// Enhanced-barrier command level, queried once per slot at creation.
    barriers: Option<d3d::ID3D12GraphicsCommandList7>,
    /// Bound compute executable; arguments require one.
    executable: Cell<*const Executable>,
    /// Kinds of descriptor heaps currently bound (bit 1 resource, bit 2 sampler).
    heaps: Cell<u32>,
    /// Whether a render scope is active on this recording.
    pub(super) rendering: Cell<bool>,
    /// Argument namespace shadow. Root arguments are invalidated by a root
    /// signature change, so binding re-emits these recorded bytes to preserve
    /// the contract's byte-preservation rule. Preallocated per slot; recording
    /// never allocates.
    arguments: RefCell<Vec<u8>>,
}

use std::cell::{Cell, RefCell};

fn list_type(domain: u32) -> Result<d3d::D3D12_COMMAND_LIST_TYPE, Status> {
    match domain {
        DOMAIN_GRAPHICS => Ok(d3d::D3D12_COMMAND_LIST_TYPE_DIRECT),
        DOMAIN_COMPUTE => Ok(d3d::D3D12_COMMAND_LIST_TYPE_COMPUTE),
        DOMAIN_COPY => Ok(d3d::D3D12_COMMAND_LIST_TYPE_COPY),
        _ => Err(UNSUPPORTED),
    }
}

impl Arena {
    pub(in crate::foundation) fn create(d: &Device, desc: &ArenaDesc) -> Result<Box<Self>, Status> {
        d.ready()?;
        desc.header.validate::<ArenaDesc>(ARENA_DESC)?;
        if desc.reserved != 0 {
            return Err(UNSUPPORTED);
        }
        // Secondary recording maps to D3D12 bundles with different replay and
        // inheritance rules; that profile is unresolved (see the mapping matrix).
        if desc.level != 0 {
            return Err(UNSUPPORTED);
        }
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
            domain: desc.domain,
            level: desc.level,
            list_type: list_type(desc.domain)?,
            slots: Vec::new(),
            used: 0,
            usable: true,
        });
        result.reserve(desc.list_capacity)?;
        Ok(result)
    }

    pub(in crate::foundation) fn reserve(&mut self, count: u32) -> Result<(), Status> {
        // SAFETY: device pointer is live for every arena operation.
        let d = unsafe { &*self.device };
        d.ready()?;
        let count = count as usize;
        if count <= self.slots.len() {
            return Ok(());
        }
        self.slots
            .try_reserve(count - self.slots.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        while self.slots.len() < count {
            // SAFETY: well-formed allocator/list creation; outputs are adopted.
            let (allocator, command, barriers) = unsafe { self.create_slot(d)? };
            self.slots.push(Box::new(List {
                device: self.device,
                domain: self.domain,
                level: self.level,
                mode: 0,
                state: Cell::new(0),
                error: Cell::new(OK),
                allocator,
                command,
                barriers,
                executable: Cell::new(ptr::null()),
                heaps: Cell::new(0),
                rendering: Cell::new(false),
                arguments: RefCell::new(vec![0; MAX_ARGUMENT_BYTES as usize]),
            }));
        }
        Ok(())
    }

    unsafe fn create_slot(
        &self,
        d: &Device,
    ) -> Result<
        (
            d3d::ID3D12CommandAllocator,
            d3d::ID3D12GraphicsCommandList,
            Option<d3d::ID3D12GraphicsCommandList7>,
        ),
        Status,
    > {
        // SAFETY: creation adopts the returned owned references.
        unsafe {
            let allocator = d
                .device
                .CreateCommandAllocator(self.list_type)
                .map_err(map_error)?;
            let command: d3d::ID3D12GraphicsCommandList = d
                .device
                .CreateCommandList(
                    0,
                    self.list_type,
                    &allocator,
                    None::<&d3d::ID3D12PipelineState>,
                )
                .map_err(map_error)?;
            // Leave the freshly created list in the closed state until begin.
            command.Close().map_err(map_error)?;
            let barriers = command.cast::<d3d::ID3D12GraphicsCommandList7>().ok();
            Ok((allocator, command, barriers))
        }
    }

    pub(in crate::foundation) fn reset(&mut self, release: bool) -> Result<(), Status> {
        // Invalidate even when reset fails: no potentially stale executable list.
        self.usable = false;
        for slot in &self.slots {
            slot.state.set(0);
        }
        // SAFETY: device pointer is live for every arena operation.
        let d = unsafe { &*self.device };
        d.ready()?;
        for slot in &self.slots {
            // Defensive close of abandoned recordings; ignored if already closed.
            let _ = unsafe { slot.command.Close() };
            if release {
                // SAFETY: caller proves no pending use of this allocator.
                unsafe {
                    slot.allocator.Reset().map_err(map_error)?;
                }
            }
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
        self.slots.truncate(retained as usize);
        self.slots.shrink_to_fit();
        Ok(())
    }

    pub(in crate::foundation) fn begin(
        &mut self,
        desc: &RecordingDesc,
    ) -> Result<*mut List, Status> {
        // SAFETY: device pointer is live for every arena operation.
        let d = unsafe { &*self.device };
        d.ready()?;
        desc.header.validate::<RecordingDesc>(RECORDING_DESC)?;
        if desc.level != self.level {
            return Err(INVALID);
        }
        // Primary recording has no inheritance record; bundles are not mapped.
        if !desc.inheritance.is_null() {
            return Err(UNSUPPORTED);
        }
        if !(1..=3).contains(&desc.replay_mode) {
            return Err(INVALID);
        }
        if !self.usable {
            return Err(INVALID);
        }
        let slot = self.slots.get_mut(self.used).ok_or(CAPACITY)?;
        self.used += 1;
        slot.state.set(0);
        slot.error.set(OK);
        slot.mode = desc.replay_mode;
        // SAFETY: caller proves no pending use of this arena generation; the
        // allocator/list pair is exclusively owned by this slot. The defensive
        // close discards any abandoned recording so reset can proceed.
        let result = unsafe {
            let _ = slot.command.Close();
            slot.allocator.Reset().and_then(|_| {
                slot.command
                    .Reset(&slot.allocator, None::<&d3d::ID3D12PipelineState>)
            })
        };
        if let Err(error) = result {
            let status = map_error(error);
            return Err(status);
        }
        slot.state.set(1);
        Ok(ptr::from_mut(&mut **slot))
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        // Slots drop their COM references; the caller excluded pending/host use.
    }
}

impl List {
    pub(super) fn recording(&self) -> Result<&Device, Status> {
        if self.state.get() != 1 {
            return Err(INVALID);
        }
        if self.error.get() != OK {
            return Err(self.error.get());
        }
        // SAFETY: device pointer is live for every list operation.
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
        let recording = self.recording();
        // Close even when poisoned: a list left in the recording state would
        // block the allocator at the next begin. Contents are discarded by 0.
        let close = unsafe { self.command.Close() };
        let result = recording.and_then(|d| d.result(close));
        self.state.set(if result.is_ok() { 2 } else { 0 });
        result
    }

    pub(in crate::foundation) fn cancel(&self) {
        // Closing releases the allocator's recording state; contents are discarded.
        let _ = unsafe { self.command.Close() };
        self.state.set(0);
    }

    pub(super) fn command_list(&self) -> &d3d::ID3D12GraphicsCommandList {
        &self.command
    }

    pub(super) fn mode(&self) -> u32 {
        self.mode
    }

    pub(super) fn set_consumed(&self) {
        self.state.set(3);
    }

    pub(super) fn device_matches(&self, device: *const Device) -> bool {
        self.device == device
    }

    pub(super) fn domain(&self) -> u32 {
        self.domain
    }

    pub(super) fn executable(&self) -> bool {
        self.state.get() == 2
    }

    pub(super) fn primary(&self) -> bool {
        self.level == 0
    }

    /// Buffer-to-buffer copy of equal spans. No staging, allocation or retention.
    /// # Safety
    /// Spans stay live and correctly synchronized through all pending uses.
    pub(in crate::foundation) unsafe fn copy(&self, dst: Span, src: Span) -> Result<(), Status> {
        self.recording()?;
        if src.size != dst.size {
            return Err(INVALID);
        }
        // SAFETY: span memory handles are trusted live objects by contract.
        let source = unsafe { src.memory.as_ref() }.ok_or(INVALID)?;
        // SAFETY: span memory handles are trusted live objects by contract.
        let target = unsafe { dst.memory.as_ref() }.ok_or(INVALID)?;
        let (src_resource, src_offset, src_address) =
            source.command_range(self.device, self.domain, src.offset, src.size, USAGE_COPY_SRC)?;
        let (dst_resource, dst_offset, dst_address) =
            target.command_range(self.device, self.domain, dst.offset, dst.size, USAGE_COPY_DST)?;
        if src.size == 0 {
            return Ok(());
        }
        if src_address.abs_diff(dst_address) < src.size {
            return Err(INVALID);
        }
        // SAFETY: validated disjoint ranges of live resources; recording only.
        unsafe {
            self.command.CopyBufferRegion(
                dst_resource,
                dst_offset,
                src_resource,
                src_offset,
                src.size,
            )
        };
        Ok(())
    }

    /// Buffer fill has no direct D3D12 command; the descriptor-based native
    /// alternative belongs to the descriptor profile (D2). Never staged.
    pub(in crate::foundation) unsafe fn fill(
        &self,
        _dst: Span,
        _pattern: u32,
    ) -> Result<(), Status> {
        let _d = self.recording()?;
        Err(UNSUPPORTED)
    }

    /// One batched dependency: global scope plus explicit backing ranges, mapped
    /// to enhanced-barrier groups through caller-provided scratch.
    /// # Safety
    /// Dependency arrays, ranges and scratch are valid, disjoint and live.
    pub(in crate::foundation) unsafe fn barrier(&self, dep: &Dependency) -> Result<(), Status> {
        let d = self.recording()?;
        let requirements = barrier_scratch(dep.memory_count, dep.image_count)?;
        let storage = scratch(dep.scratch, dep.scratch_size, requirements)?;
        let groups = unsafe { self.translate_dependency(d, dep, storage)? };
        // SAFETY: scratch holds every native barrier record for this call.
        unsafe {
            let list7 = self.barriers.as_ref().ok_or(UNSUPPORTED)?;
            list7.Barrier(std::slice::from_raw_parts(
                groups.0,
                groups.1.try_into().map_err(|_| INVALID)?,
            ));
        }
        Ok(())
    }

    /// Translate one dependency into barrier groups in caller scratch. Returns
    /// the group array pointer and count. Global and per-range records live in
    /// scratch and stay valid through the native call.
    unsafe fn translate_dependency(
        &self,
        d: &Device,
        dep: &Dependency,
        storage: *mut u8,
    ) -> Result<(*const d3d::D3D12_BARRIER_GROUP, usize), Status> {
        dep.header.validate::<Dependency>(DEPENDENCY)?;
        if dep.flags != 0 {
            return Err(UNSUPPORTED);
        }
        let scope_before = stages(dep.before)?;
        let scope_after = stages(dep.after)?;
        let global_before = access(dep.global_before)?;
        let global_after = access(dep.global_after)?;
        if (dep.before == 0 && dep.global_before != 0) || (dep.after == 0 && dep.global_after != 0) {
            return Err(INVALID);
        }
        // D3D12 pairs each sync bit with the accesses it justifies: the native
        // sync scope is derived from the declared accesses and narrowed to the
        // declared stage scope. Stages that cannot produce the declared access
        // are exact to drop; the fallback keeps the full required set.
        let before = sync_for_access(global_before, scope_before);
        let after = sync_for_access(global_after, scope_after);
        let (groups_offset, global_offset, buffers_offset, textures_offset) =
            barrier_offsets(dep.memory_count, dep.image_count)?;
        let groups = unsafe { storage.add(groups_offset) }.cast::<d3d::D3D12_BARRIER_GROUP>();
        let global = unsafe { storage.add(global_offset) }.cast::<d3d::D3D12_GLOBAL_BARRIER>();
        let buffers = unsafe { storage.add(buffers_offset) }.cast::<d3d::D3D12_BUFFER_BARRIER>();
        let textures = unsafe { storage.add(textures_offset) }.cast::<d3d::D3D12_TEXTURE_BARRIER>();
        let mut count = 0;
        if global_before != d3d::D3D12_BARRIER_ACCESS_NO_ACCESS
            || global_after != d3d::D3D12_BARRIER_ACCESS_NO_ACCESS
        {
            // SAFETY: scratch slot for the global barrier record.
            unsafe {
                global.write(d3d::D3D12_GLOBAL_BARRIER {
                    SyncBefore: before,
                    SyncAfter: after,
                    AccessBefore: global_before,
                    AccessAfter: global_after,
                });
                groups
                    .add(count)
                    .write(d3d::D3D12_BARRIER_GROUP {
                        Type: d3d::D3D12_BARRIER_TYPE_GLOBAL,
                        NumBarriers: 1,
                        Anonymous: d3d::D3D12_BARRIER_GROUP_0 {
                            pGlobalBarriers: global,
                        },
                    });
            }
            count += 1;
        }
        let memory_count = dep.memory_count as usize;
        if memory_count != 0 {
            // SAFETY: caller-provided memory barrier array of declared count.
            let memories = unsafe { slice(dep.memory, dep.memory_count)? };
            for (i, memory) in memories.iter().enumerate() {
                // SAFETY: span memory handles are trusted live objects by contract.
                let backing = unsafe { memory.range.memory.as_ref() }.ok_or(INVALID)?;
                if memory.range.size == 0 {
                    return Err(INVALID);
                }
                let (resource, offset, _) = backing.command_range(
                    d,
                    self.domain,
                    memory.range.offset,
                    memory.range.size,
                    0,
                )?;
                domains(d, memory.source_domain, memory.destination_domain)?;
                let access_before = access(memory.before)?;
                let access_after = access(memory.after)?;
                let before = buffer_sync(sync_for_access(access_before, scope_before));
                let after = buffer_sync(sync_for_access(access_after, scope_after));
                // SAFETY: scratch slot for this buffer barrier record.
                unsafe {
                    buffers.add(i).write(d3d::D3D12_BUFFER_BARRIER {
                        SyncBefore: before,
                        SyncAfter: after,
                        AccessBefore: access_before,
                        AccessAfter: access_after,
                        // Borrowed resource reference: scratch is never dropped,
                        // so the borrowed interface wrapper is not released.
                        pResource: core::mem::ManuallyDrop::new(Some(
                            d3d::ID3D12Resource::from_raw(resource.as_raw()),
                        )),
                        Offset: offset,
                        Size: memory.range.size,
                    });
                }
            }
            // SAFETY: scratch slot for the buffer barrier group record.
            unsafe {
                groups.add(count).write(d3d::D3D12_BARRIER_GROUP {
                    Type: d3d::D3D12_BARRIER_TYPE_BUFFER,
                    NumBarriers: dep.memory_count,
                    Anonymous: d3d::D3D12_BARRIER_GROUP_0 {
                        pBufferBarriers: buffers,
                    },
                });
            }
            count += 1;
        }
        let image_count = dep.image_count as usize;
        if image_count != 0 {
            // SAFETY: caller-provided image barrier array of declared count.
            let images = unsafe { slice(dep.images, dep.image_count)? };
            for (i, barrier) in images.iter().enumerate() {
                // SAFETY: image handles are trusted live objects by contract.
                let image = unsafe { barrier.image.as_ref() }.ok_or(INVALID)?;
                if barrier.new_state == STATE_UNDEFINED {
                    return Err(INVALID);
                }
                domains(d, barrier.source_domain, barrier.destination_domain)?;
                let before_access = access(barrier.before)?;
                let after_access = access(barrier.after)?;
                let before = sync_for_access(before_access, scope_before);
                let after = sync_for_access(after_access, scope_after);
                let layout_before = layout(barrier.old_state, barrier.before)?;
                let layout_after = layout(barrier.new_state, barrier.after)?;
                let subresources = subresources(&barrier.range, image)?;
                // ABI values of D3D12_TEXTURE_BARRIER_FLAG_NONE/DISCARD.
                let flags = d3d::D3D12_TEXTURE_BARRIER_FLAGS(if barrier.discard != 0 { 1 } else { 0 });
                // SAFETY: scratch slot for this texture barrier record.
                unsafe {
                    textures.add(i).write(d3d::D3D12_TEXTURE_BARRIER {
                        SyncBefore: before,
                        SyncAfter: after,
                        AccessBefore: before_access,
                        AccessAfter: after_access,
                        LayoutBefore: layout_before,
                        LayoutAfter: layout_after,
                        // Borrowed resource reference: scratch is never dropped,
                        // so the borrowed interface wrapper is not released.
                        pResource: core::mem::ManuallyDrop::new(Some(
                            d3d::ID3D12Resource::from_raw(image.resource()?.as_raw()),
                        )),
                        Subresources: subresources,
                        Flags: flags,
                    });
                }
            }
            // SAFETY: scratch slot for the texture barrier group record.
            unsafe {
                groups.add(count).write(d3d::D3D12_BARRIER_GROUP {
                    Type: d3d::D3D12_BARRIER_TYPE_TEXTURE,
                    NumBarriers: dep.image_count,
                    Anonymous: d3d::D3D12_BARRIER_GROUP_0 {
                        pTextureBarriers: textures,
                    },
                });
            }
            count += 1;
        }
        Ok((groups, count))
    }
}

/// Native sync scope justified by one side's declared accesses. D3D12 validates
/// sync/access pairings: a sync bit must be motivated by an access on the same
/// side. The required set is intersected with the caller's stage scope; when the
/// intersection is empty the full required set stands (conservative, never less).
fn sync_for_access(
    access: d3d::D3D12_BARRIER_ACCESS,
    scope: d3d::D3D12_BARRIER_SYNC,
) -> d3d::D3D12_BARRIER_SYNC {
    use d3d::{
        D3D12_BARRIER_ACCESS as A, D3D12_BARRIER_SYNC as S, D3D12_BARRIER_ACCESS_CONSTANT_BUFFER,
        D3D12_BARRIER_ACCESS_COPY_DEST, D3D12_BARRIER_ACCESS_COPY_SOURCE,
        D3D12_BARRIER_ACCESS_DEPTH_STENCIL_READ, D3D12_BARRIER_ACCESS_DEPTH_STENCIL_WRITE,
        D3D12_BARRIER_ACCESS_INDEX_BUFFER, D3D12_BARRIER_ACCESS_INDIRECT_ARGUMENT,
        D3D12_BARRIER_ACCESS_RENDER_TARGET, D3D12_BARRIER_ACCESS_RESOLVE_DEST,
        D3D12_BARRIER_ACCESS_RESOLVE_SOURCE, D3D12_BARRIER_ACCESS_SHADER_RESOURCE,
        D3D12_BARRIER_ACCESS_UNORDERED_ACCESS, D3D12_BARRIER_ACCESS_VERTEX_BUFFER,
    };
    let mut required = S(0);
    let has = |bit: A| bit.0 != 0 && (access.0 & bit.0) == bit.0;
    if has(D3D12_BARRIER_ACCESS_COPY_SOURCE) || has(D3D12_BARRIER_ACCESS_COPY_DEST) {
        required |= d3d::D3D12_BARRIER_SYNC_COPY;
    }
    if has(D3D12_BARRIER_ACCESS_RESOLVE_SOURCE) || has(D3D12_BARRIER_ACCESS_RESOLVE_DEST) {
        required |= d3d::D3D12_BARRIER_SYNC_RESOLVE;
    }
    if has(D3D12_BARRIER_ACCESS_RENDER_TARGET) {
        required |= d3d::D3D12_BARRIER_SYNC_RENDER_TARGET;
    }
    if has(D3D12_BARRIER_ACCESS_DEPTH_STENCIL_READ) || has(D3D12_BARRIER_ACCESS_DEPTH_STENCIL_WRITE) {
        required |= d3d::D3D12_BARRIER_SYNC_DEPTH_STENCIL;
    }
    if has(D3D12_BARRIER_ACCESS_UNORDERED_ACCESS)
        || has(D3D12_BARRIER_ACCESS_SHADER_RESOURCE)
        || has(D3D12_BARRIER_ACCESS_CONSTANT_BUFFER)
    {
        // The explicit shading set, so a narrower caller scope can select the
        // exact shading stages that justify the access.
        required |= d3d::D3D12_BARRIER_SYNC_DRAW
            | d3d::D3D12_BARRIER_SYNC_COMPUTE_SHADING
            | d3d::D3D12_BARRIER_SYNC_PIXEL_SHADING
            | d3d::D3D12_BARRIER_SYNC_NON_PIXEL_SHADING
            | d3d::D3D12_BARRIER_SYNC_ALL_SHADING;
    }
    if has(D3D12_BARRIER_ACCESS_INDEX_BUFFER) {
        required |= d3d::D3D12_BARRIER_SYNC_INDEX_INPUT;
    }
    if has(D3D12_BARRIER_ACCESS_VERTEX_BUFFER) {
        required |= d3d::D3D12_BARRIER_SYNC_VERTEX_SHADING;
    }
    if has(D3D12_BARRIER_ACCESS_INDIRECT_ARGUMENT) {
        required |= d3d::D3D12_BARRIER_SYNC_EXECUTE_INDIRECT;
    }
    if required.0 == 0 {
        d3d::D3D12_BARRIER_SYNC_NONE
    } else {
        let narrowed = S(required.0 & scope.0);
        if narrowed.0 == 0 {
            required
        } else {
            narrowed
        }
    }
}

/// Buffer barriers accept only buffer-relevant scopes: resolve, render-target
/// and depth/stencil stages never access buffers, so dropping those bits is an
/// exact mapping, not a weakened one.
fn buffer_sync(sync: d3d::D3D12_BARRIER_SYNC) -> d3d::D3D12_BARRIER_SYNC {
    let texture_only = d3d::D3D12_BARRIER_SYNC_RESOLVE.0
        | d3d::D3D12_BARRIER_SYNC_RENDER_TARGET.0
        | d3d::D3D12_BARRIER_SYNC_DEPTH_STENCIL.0;
    d3d::D3D12_BARRIER_SYNC(sync.0 & !texture_only)
}

fn barrier_offsets(
    memory: u32,
    images: u32,
) -> Result<(usize, usize, usize, usize), Status> {
    let groups = std::alloc::Layout::new::<d3d::D3D12_BARRIER_GROUP>()
        .repeat(3)
        .map_err(|_| INVALID)?
        .0;
    let (layout, global_offset) = groups
        .extend(std::alloc::Layout::new::<d3d::D3D12_GLOBAL_BARRIER>())
        .map_err(|_| INVALID)?;
    let (layout, buffers_offset) = layout
        .extend(array_layout::<d3d::D3D12_BUFFER_BARRIER>(memory)?)
        .map_err(|_| INVALID)?;
    let (layout, textures_offset) = layout
        .extend(array_layout::<d3d::D3D12_TEXTURE_BARRIER>(images)?)
        .map_err(|_| INVALID)?;
    let total = layout.pad_to_align();
    if total.size() == 0 {
        return Err(INTERNAL_ERROR);
    }
    Ok((0, global_offset, buffers_offset, textures_offset))
}

pub(in crate::foundation) fn barrier_scratch(
    memory: u32,
    images: u32,
) -> Result<HostRequirements, Status> {
    let (groups_offset, global_offset, buffers_offset, textures_offset) =
        barrier_offsets(memory, images)?;
    let _ = (groups_offset, global_offset, buffers_offset, textures_offset);
    let groups = std::alloc::Layout::new::<d3d::D3D12_BARRIER_GROUP>()
        .repeat(3)
        .map_err(|_| INVALID)?
        .0;
    let (layout, _) = groups
        .extend(std::alloc::Layout::new::<d3d::D3D12_GLOBAL_BARRIER>())
        .map_err(|_| INVALID)?;
    let (layout, _) = layout
        .extend(array_layout::<d3d::D3D12_BUFFER_BARRIER>(memory)?)
        .map_err(|_| INVALID)?;
    let (layout, _) = layout
        .extend(array_layout::<d3d::D3D12_TEXTURE_BARRIER>(images)?)
        .map_err(|_| INVALID)?;
    Ok(HostRequirements {
        size: layout.pad_to_align().size() as u64,
        alignment: layout.align() as u64,
    })
}

/// Submission scratch holds only the translated native command-list pointers.
pub(in crate::foundation) fn submit_scratch(
    lists: u32,
    _waits: u32,
    _signals: u32,
) -> Result<HostRequirements, Status> {
    let layout = array_layout::<Option<d3d::ID3D12CommandList>>(lists)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}

/// D3D12 submits need no batch headers beyond each descriptor's own scratch.
pub(in crate::foundation) fn submit_batch_scratch(_count: u32) -> Result<HostRequirements, Status> {
    Ok(HostRequirements {
        size: 0,
        alignment: 1,
    })
}

/// Stage scope translation. HOST has no D3D12 barrier equivalent and is rejected
/// rather than dropped; unenabled shader profiles (mesh/task/preprocess) are
/// rejected until their feature mappings exist.
fn stages(mask: u64) -> Result<d3d::D3D12_BARRIER_SYNC, Status> {
    const KNOWN: u64 = (1 << 23) - 1;
    if mask & !KNOWN != 0 {
        return Err(UNSUPPORTED);
    }
    if mask & (STAGE_HOST | MESH_STAGE | TASK_STAGE | PREPROCESS_STAGE) != 0 {
        return Err(UNSUPPORTED);
    }
    let mut result = d3d::D3D12_BARRIER_SYNC_NONE;
    let mut rest = mask;
    for (bit, native) in [
        (STAGE_ALL, d3d::D3D12_BARRIER_SYNC_ALL),
        (STAGE_TRANSFER, d3d::D3D12_BARRIER_SYNC_COPY | d3d::D3D12_BARRIER_SYNC_RESOLVE),
        (STAGE_COMPUTE, d3d::D3D12_BARRIER_SYNC_COMPUTE_SHADING),
        (STAGE_VERTEX, d3d::D3D12_BARRIER_SYNC_VERTEX_SHADING),
        (STAGE_FRAGMENT, d3d::D3D12_BARRIER_SYNC_PIXEL_SHADING),
        (STAGE_INDIRECT, d3d::D3D12_BARRIER_SYNC_EXECUTE_INDIRECT),
        (STAGE_COLOR, d3d::D3D12_BARRIER_SYNC_RENDER_TARGET),
        (STAGE_DEPTH, d3d::D3D12_BARRIER_SYNC_DEPTH_STENCIL),
        (STAGE_COPY, d3d::D3D12_BARRIER_SYNC_COPY),
        (STAGE_INDEX_INPUT, d3d::D3D12_BARRIER_SYNC_INDEX_INPUT),
        (STAGE_VERTEX_FETCH, d3d::D3D12_BARRIER_SYNC_VERTEX_SHADING),
        (STAGE_EARLY_DEPTH, d3d::D3D12_BARRIER_SYNC_DEPTH_STENCIL),
        (STAGE_LATE_DEPTH, d3d::D3D12_BARRIER_SYNC_DEPTH_STENCIL),
        (STAGE_CLEAR, d3d::D3D12_BARRIER_SYNC_COPY),
        (STAGE_RESOLVE, d3d::D3D12_BARRIER_SYNC_RESOLVE),
        (STAGE_BLIT, d3d::D3D12_BARRIER_SYNC_COPY),
        (STAGE_VERTEX_INPUT, d3d::D3D12_BARRIER_SYNC_INDEX_INPUT),
        (STAGE_ALL_GRAPHICS, d3d::D3D12_BARRIER_SYNC_DRAW),
        (STAGE_PRE_RASTER, d3d::D3D12_BARRIER_SYNC_DRAW),
    ] {
        if mask & bit != 0 {
            result |= native;
            rest &= !bit;
        }
    }
    if rest != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(result)
}

/// Access translation. Broad generic reads/writes map to the conservative union
/// of their native accesses; descriptor-heap accesses belong to the D2 profile.
fn access(mask: u64) -> Result<d3d::D3D12_BARRIER_ACCESS, Status> {
    const KNOWN: u64 = (1 << 21) - 1;
    if mask & !KNOWN != 0 {
        return Err(UNSUPPORTED);
    }
    if mask & (ACCESS_HOST_READ | ACCESS_HOST_WRITE | ACCESS_RESOURCE_HEAP_READ | ACCESS_SAMPLER_HEAP_READ) != 0
    {
        return Err(UNSUPPORTED);
    }
    // NO_ACCESS is an exclusive sentinel, never combined with real access bits.
    let mut result = d3d::D3D12_BARRIER_ACCESS(0);
    for (bit, native) in [
        (ACCESS_COPY_READ, d3d::D3D12_BARRIER_ACCESS_COPY_SOURCE),
        (ACCESS_COPY_WRITE, d3d::D3D12_BARRIER_ACCESS_COPY_DEST),
        (ACCESS_SHADER_READ, d3d::D3D12_BARRIER_ACCESS_SHADER_RESOURCE),
        (ACCESS_SHADER_WRITE, d3d::D3D12_BARRIER_ACCESS_UNORDERED_ACCESS),
        (ACCESS_INDIRECT_READ, d3d::D3D12_BARRIER_ACCESS_INDIRECT_ARGUMENT),
        (ACCESS_INDEX_READ, d3d::D3D12_BARRIER_ACCESS_INDEX_BUFFER),
        (ACCESS_VERTEX_READ, d3d::D3D12_BARRIER_ACCESS_VERTEX_BUFFER),
        (ACCESS_UNIFORM_READ, d3d::D3D12_BARRIER_ACCESS_CONSTANT_BUFFER),
        (ACCESS_COLOR_READ, d3d::D3D12_BARRIER_ACCESS_RENDER_TARGET),
        (ACCESS_COLOR_WRITE, d3d::D3D12_BARRIER_ACCESS_RENDER_TARGET),
        (ACCESS_DEPTH_READ, d3d::D3D12_BARRIER_ACCESS_DEPTH_STENCIL_READ),
        (ACCESS_DEPTH_WRITE, d3d::D3D12_BARRIER_ACCESS_DEPTH_STENCIL_WRITE),
        (ACCESS_SAMPLED_READ, d3d::D3D12_BARRIER_ACCESS_SHADER_RESOURCE),
        (ACCESS_STORAGE_READ, d3d::D3D12_BARRIER_ACCESS_UNORDERED_ACCESS),
        (ACCESS_STORAGE_WRITE, d3d::D3D12_BARRIER_ACCESS_UNORDERED_ACCESS),
    ] {
        if mask & bit != 0 {
            result |= native;
        }
    }
    if mask & ACCESS_READ != 0 {
        result |= d3d::D3D12_BARRIER_ACCESS_COPY_SOURCE
            | d3d::D3D12_BARRIER_ACCESS_SHADER_RESOURCE
            | d3d::D3D12_BARRIER_ACCESS_INDIRECT_ARGUMENT
            | d3d::D3D12_BARRIER_ACCESS_INDEX_BUFFER
            | d3d::D3D12_BARRIER_ACCESS_VERTEX_BUFFER
            | d3d::D3D12_BARRIER_ACCESS_CONSTANT_BUFFER
            | d3d::D3D12_BARRIER_ACCESS_DEPTH_STENCIL_READ;
    }
    if mask & ACCESS_WRITE != 0 {
        result |= d3d::D3D12_BARRIER_ACCESS_COPY_DEST
            | d3d::D3D12_BARRIER_ACCESS_UNORDERED_ACCESS
            | d3d::D3D12_BARRIER_ACCESS_RENDER_TARGET
            | d3d::D3D12_BARRIER_ACCESS_DEPTH_STENCIL_WRITE;
    }
    if result.0 == 0 {
        Ok(d3d::D3D12_BARRIER_ACCESS_NO_ACCESS)
    } else {
        Ok(result)
    }
}

/// Explicit image states map 1:1 to native layouts. STATE_GENERAL is the one
/// multi-use state: it resolves deterministically from its paired access mask
/// (storage writes -> UNORDERED_ACCESS, shader reads -> SHADER_RESOURCE, copy
/// accesses -> the matching copy layout, otherwise COMMON).
fn layout(state: u32, access_mask: u64) -> Result<d3d::D3D12_BARRIER_LAYOUT, Status> {
    Ok(match state {
        STATE_UNDEFINED => d3d::D3D12_BARRIER_LAYOUT_UNDEFINED,
        STATE_GENERAL => {
            if access_mask & (ACCESS_SHADER_WRITE | ACCESS_STORAGE_WRITE | ACCESS_WRITE) != 0 {
                d3d::D3D12_BARRIER_LAYOUT_UNORDERED_ACCESS
            } else if access_mask
                & (ACCESS_SHADER_READ | ACCESS_STORAGE_READ | ACCESS_SAMPLED_READ | ACCESS_READ)
                != 0
            {
                d3d::D3D12_BARRIER_LAYOUT_SHADER_RESOURCE
            } else if access_mask & ACCESS_COPY_READ != 0 {
                d3d::D3D12_BARRIER_LAYOUT_COPY_SOURCE
            } else if access_mask & ACCESS_COPY_WRITE != 0 {
                d3d::D3D12_BARRIER_LAYOUT_COPY_DEST
            } else {
                d3d::D3D12_BARRIER_LAYOUT_COMMON
            }
        }
        STATE_COPY_SRC => d3d::D3D12_BARRIER_LAYOUT_COPY_SOURCE,
        STATE_COPY_DST => d3d::D3D12_BARRIER_LAYOUT_COPY_DEST,
        STATE_SHADER_READ => d3d::D3D12_BARRIER_LAYOUT_SHADER_RESOURCE,
        STATE_COLOR_ATTACHMENT => d3d::D3D12_BARRIER_LAYOUT_RENDER_TARGET,
        STATE_DEPTH_STENCIL_ATTACHMENT => d3d::D3D12_BARRIER_LAYOUT_DEPTH_STENCIL_WRITE,
        STATE_DEPTH_STENCIL_READ => d3d::D3D12_BARRIER_LAYOUT_DEPTH_STENCIL_READ,
        STATE_PRESENT => d3d::D3D12_BARRIER_LAYOUT_PRESENT,
        _ => return Err(INVALID),
    })
}

/// Subresource range translation: mips/layers map directly, aspects map to
/// depth/stencil planes (depth plane 0, stencil plane 1).
fn subresources(
    range: &Subresources,
    image: &Image,
) -> Result<d3d::D3D12_BARRIER_SUBRESOURCE_RANGE, Status> {
    if range.mip_count == 0 || range.layer_count == 0 {
        return Err(INVALID);
    }
    if range.first_mip.checked_add(range.mip_count) > Some(image.mip_count())
        || range.first_layer.checked_add(range.layer_count) > Some(image.layer_count())
    {
        return Err(INVALID);
    }
    let (first_plane, planes) = match range.aspects & (ASPECT_COLOR | 2 | 4) {
        a if a == (2 | 4) => (0, 2),
        a if a == 2 => (0, 1),
        a if a == 4 => (1, 1),
        _ => (0, 1),
    };
    Ok(d3d::D3D12_BARRIER_SUBRESOURCE_RANGE {
        IndexOrFirstMipLevel: range.first_mip,
        NumMipLevels: range.mip_count,
        FirstArraySlice: range.first_layer,
        NumArraySlices: range.layer_count,
        FirstPlane: first_plane,
        NumPlanes: planes,
    })
}

/// Ownership endpoints: D3D12 has no resource-ownership state, so release and
/// acquire map to ordinary scoped barriers plus the caller's queue fence edge.
fn domains(d: &Device, source: u32, destination: u32) -> Result<(), Status> {
    if source == DOMAIN_IGNORED && destination == DOMAIN_IGNORED {
        return Ok(());
    }
    if source == DOMAIN_IGNORED || destination == DOMAIN_IGNORED {
        return Err(INVALID);
    }
    if d.snapshot.queues.iter().any(|q| q.domain == source)
        && d.snapshot.queues.iter().any(|q| q.domain == destination)
    {
        Ok(())
    } else {
        Err(UNSUPPORTED)
    }
}

/// Upper bound of the argument namespace (63 root-constant dwords).
pub(super) const MAX_ARGUMENT_BYTES: u32 = 252;

impl List {
    /// The bound executable for one argument scope. COMPUTE is the only scope
    /// of this profile.
    fn bound_executable(&self, scope: u64) -> Result<&Executable, Status> {
        let d = self.recording()?;
        if scope != STAGE_COMPUTE {
            return Err(UNSUPPORTED);
        }
        let _ = d;
        // SAFETY: the executable pointer is set only by bind_executable and the
        // contract keeps executables alive through all recorded/pending uses.
        unsafe { self.executable.get().as_ref() }.ok_or(INVALID)
    }

    /// Bind prepared compute state. Root arguments are re-emitted from the
    /// argument shadow so bytes survive root-signature changes.
    pub(in crate::foundation) fn bind_executable(
        &self,
        executable: &Executable,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        if executable.stages != STAGE_COMPUTE {
            return Err(UNSUPPORTED);
        }
        if !ptr::eq(executable.device, d) {
            return Err(INVALID);
        }
        let domain = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?;
        if domain.flags & COMPUTE == 0 {
            return Err(UNSUPPORTED);
        }
        // SAFETY: prepared objects stay live through all recorded uses.
        unsafe {
            self.command.SetComputeRootSignature(&executable.signature);
            self.command.SetPipelineState(&executable.pipeline);
            self.emit_arguments(executable)?;
        }
        self.executable.set(executable);
        Ok(())
    }

    /// Re-emit recorded argument bytes as root constants and root descriptors.
    /// Bounded native re-recording; no allocation, compilation or retention.
    /// # Safety
    /// The bound executable and command list are live and exclusively recorded.
    unsafe fn emit_arguments(&self, executable: &Executable) -> Result<(), Status> {
        let bytes = executable.byte_size as usize;
        // Recording is externally serialized per the contract; the borrow only
        // guards accidental aliasing in this module.
        let shadow = self.arguments.borrow();
        // SAFETY: recording slot and executable are live; bytes are plain data.
        unsafe {
            self.command.SetComputeRoot32BitConstants(
                0,
                executable.byte_size / 4,
                shadow[..bytes].as_ptr().cast(),
                0,
            );
        }
        for (slot, root) in executable.roots.iter().enumerate() {
            let mut address = [0u8; 8];
            address.copy_from_slice(&shadow[root.offset as usize..root.offset as usize + 8]);
            // SAFETY: recording slot is live; the address is caller-owned data.
            unsafe {
                self.command
                    .SetComputeRootUnorderedAccessView(1 + slot as u32, u64::from_le_bytes(address));
            }
        }
        Ok(())
    }

    /// Update shared argument bytes. Ranges overlapping a root slot's bytes are
    /// rejected: a native root descriptor binding cannot follow byte writes.
    /// # Safety
    /// Data points to `size` readable bytes for the duration of the call.
    pub(in crate::foundation) unsafe fn inline(
        &self,
        scope: u64,
        offset: u32,
        size: u32,
        data: *const std::ffi::c_void,
    ) -> Result<(), Status> {
        let executable = self.bound_executable(scope)?;
        if offset % 4 != 0
            || size % 4 != 0
            || offset > executable.byte_size
            || size > executable.byte_size - offset
            || (size != 0 && data.is_null())
        {
            return Err(INVALID);
        }
        if executable
            .roots
            .iter()
            .any(|r| offset < r.offset + 8 && r.offset < offset + size)
        {
            return Err(UNSUPPORTED);
        }
        if size == 0 {
            return Ok(());
        }
        {
            let mut shadow = self.arguments.borrow_mut();
            // SAFETY: data points to size readable bytes by the caller contract.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.cast::<u8>(),
                    shadow[offset as usize..].as_mut_ptr(),
                    size as usize,
                );
            }
        }
        // SAFETY: recording slot is live; data is readable for this call.
        unsafe {
            self.command
                .SetComputeRoot32BitConstants(0, size / 4, data.cast(), offset / 4);
        }
        Ok(())
    }

    /// Bind one root slot as a native root descriptor; the namespace bytes at
    /// the slot's offset are updated to match, as in the shared contract.
    pub(in crate::foundation) fn root(
        &self,
        scope: u64,
        slot: u32,
        address: u64,
    ) -> Result<(), Status> {
        let executable = self.bound_executable(scope)?;
        let root = executable.roots.get(slot as usize).ok_or(INVALID)?;
        if root.stages != scope {
            return Err(UNSUPPORTED);
        }
        if address % u64::from(root.alignment) != 0 {
            return Err(INVALID);
        }
        self.arguments.borrow_mut()[root.offset as usize..root.offset as usize + 8]
            .copy_from_slice(&address.to_le_bytes());
        // SAFETY: recording slot is live; the address is caller-owned data.
        unsafe {
            self.command
                .SetComputeRootUnorderedAccessView(1 + slot, address);
        }
        Ok(())
    }

    pub(in crate::foundation) fn dispatch(&self, launch: &Launch) -> Result<(), Status> {
        self.bound_executable(STAGE_COMPUTE)?;
        if launch.dynamic_shared_bytes != 0 || !launch.extensions.is_null() {
            return Err(UNSUPPORTED);
        }
        // SAFETY: device pointer is live for every list operation.
        let d = unsafe { &*self.device };
        let groups = [launch.groups.x, launch.groups.y, launch.groups.z];
        if groups
            .iter()
            .zip(d.snapshot.execution_limits.max_groups)
            .any(|(n, max)| *n > max)
        {
            return Err(UNSUPPORTED);
        }
        // SAFETY: recording slot is live.
        unsafe {
            self.command.Dispatch(groups[0], groups[1], groups[2]);
        }
        Ok(())
    }

    /// Indirect dispatch through the device's dispatch command signature; the
    /// arguments span supplies three u32 dimensions.
    /// # Safety
    /// The arguments span stays live and valid for all recorded/pending uses.
    pub(in crate::foundation) unsafe fn dispatch_indirect(
        &self,
        args: Span,
        dynamic_shared_bytes: u32,
    ) -> Result<(), Status> {
        self.bound_executable(STAGE_COMPUTE)?;
        if dynamic_shared_bytes != 0 {
            return Err(UNSUPPORTED);
        }
        // SAFETY: device pointer is live for every list operation.
        let d = unsafe { &*self.device };
        // SAFETY: span memory handle is a trusted live object by contract.
        let memory = unsafe { args.memory.as_ref() }.ok_or(INVALID)?;
        let (buffer, offset, _) =
            memory.command_range(d, self.domain, args.offset, args.size, USAGE_INDIRECT)?;
        if offset % 4 != 0 || args.size < 12 {
            return Err(INVALID);
        }
        // SAFETY: validated arguments range of a live buffer; recording only.
        unsafe {
            self.command.ExecuteIndirect(
                &d.dispatch_signature,
                1,
                buffer,
                offset,
                None,
                0,
            );
        }
        Ok(())
    }
}

impl List {
    /// Bind a descriptor-heap range for direct heap indexing. Recipes in the
    /// range are materialized into the memory's native heaps; D3D12 binds the
    /// heap set, so both currently bound kinds are re-issued together.
    /// # Safety
    /// Storage is live descriptor-heap memory; descriptor contents own nothing.
    pub(in crate::foundation) unsafe fn bind_heap(
        &self,
        binding: &HeapBinding,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        binding.header.validate::<HeapBinding>(HEAP_BINDING)?;
        if binding.flags != 0 {
            return Err(UNSUPPORTED);
        }
        if binding.reserved_size != 0 {
            // Heap reservations (out-of-bounds fallbacks) are not mapped here.
            return Err(UNSUPPORTED);
        }
        let domain = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?;
        if domain.flags & (GRAPHICS | COMPUTE) == 0 {
            return Err(UNSUPPORTED);
        }
        let (stride, heap_bit) = match binding.kind {
            HEAP_RESOURCE => (descriptors::RESOURCE_STRIDE, 1),
            HEAP_SAMPLER => (descriptors::SAMPLER_STRIDE, 2),
            _ => return Err(INVALID),
        };
        // SAFETY: storage memory handle is a trusted live object by contract.
        let memory = unsafe { binding.storage.memory.as_ref() }.ok_or(INVALID)?;
        if memory.usage & USAGE_DESCRIPTOR_HEAP == 0 {
            return Err(INVALID);
        }
        if binding.storage.offset % stride != 0
            || binding.storage.size % stride != 0
            || binding.storage.offset > memory.size
            || binding.storage.size > memory.size - binding.storage.offset
        {
            return Err(INVALID);
        }
        let heaps = memory.descriptor_heaps().ok_or(INVALID)?;
        let heap = match binding.kind {
            HEAP_RESOURCE => &heaps.resource,
            _ => &heaps.sampler,
        };
        let capacity = heaps.capacity(binding.kind)?;
        let base_index = binding.storage.offset / stride;
        let count = binding.storage.size / stride;
        if base_index + count > capacity {
            return Err(INVALID);
        }
        if count != 0 {
            // Recipes materialize from host-visible heap memory through the
            // memory's persistent mapping; no allocation or retention.
            // SAFETY: external serialization covers mapping state.
            let mapping = memory.map(binding.storage.offset, binding.storage.size)?;
            for i in 0..count {
                // SAFETY: the mapping covers the recipe region.
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        mapping.data.cast::<u8>().add((i * stride) as usize),
                        stride as usize,
                    )
                };
                // SAFETY: live heap and validated recipe bytes.
                unsafe {
                    descriptors::materialize(
                        d,
                        heap,
                        binding.kind,
                        (base_index + i) as u32,
                        bytes,
                    )?
                };
            }
        }
        // D3D12 binds the heap set; re-issue both kinds to keep earlier binds.
        let kinds = self.heaps.get() | heap_bit;
        self.heaps.set(kinds);
        // The native array must contain only present heaps: a NULL entry is
        // rejected. One resource and one sampler heap may be bound together.
        let mut bound: Vec<Option<d3d::ID3D12DescriptorHeap>> = Vec::with_capacity(2);
        if kinds & 1 != 0 {
            bound.push(Some(heaps.resource.clone()));
        }
        if kinds & 2 != 0 {
            bound.push(Some(heaps.sampler.clone()));
        }
        // SAFETY: recording slot is live; heap references stay live through use.
        unsafe {
            self.command.SetDescriptorHeaps(&bound);
        }
        // Changing descriptor heaps invalidates bound tables and root
        // arguments: re-emit arguments and bind the range-start table.
        if let Some(executable) = unsafe { self.executable.get().as_ref() } {
            // SAFETY: recording slot and executable are live.
            unsafe { self.emit_arguments(executable)? };
            let (gpu, _stride) = heaps.gpu_start(d, binding.kind)?;
            // SAFETY: recording slot is live; the heap stays live through use.
            unsafe {
                self.command.SetComputeRootDescriptorTable(
                    1 + executable.roots.len() as u32,
                    d3d::D3D12_GPU_DESCRIPTOR_HANDLE {
                        ptr: gpu.ptr
                            + u64::from(super::descriptors::native_stride(d, binding.kind))
                                * u64::from(base_index),
                    },
                );
            }
        }
        Ok(())
    }
}

impl Queue {
    /// # Safety
    /// Exclusive queue host access; caller retains all resources and obeys the
    /// replay, timeline-order and uncertain-submission rules of ogpu_next.h.
    pub(in crate::foundation) unsafe fn submit(&self, desc: &SubmitDesc) -> Result<(), Status> {
        // SAFETY: queue parent binding is live for every submission.
        let d = unsafe { self.device.as_ref() }.ok_or(INVALID)?;
        d.ready()?;
        // SAFETY: validated records; caller owns all lifetime obligations.
        unsafe {
            self.execute_submit(d, desc)?;
            consume_submit(desc);
        }
        Ok(())
    }

    /// One ordered sequence of native queue operations per descriptor, in batch
    /// order; D3D12 has no single batched submit call.
    /// # Safety
    /// Same obligations as submit across the whole batch.
    pub(in crate::foundation) unsafe fn submit_batch(
        &self,
        count: u32,
        descs: *const SubmitDesc,
        _storage: HostSpan,
    ) -> Result<(), Status> {
        // SAFETY: queue parent binding is live for every submission.
        let d = unsafe { self.device.as_ref() }.ok_or(INVALID)?;
        d.ready()?;
        if count == 0 {
            return Ok(());
        }
        // SAFETY: caller-provided descriptor array of declared count.
        let descs = unsafe { slice(descs, count)? };
        // Every batch validates before any native submission.
        for desc in descs {
            // SAFETY: preflight only; no native calls in validation.
            unsafe {
                self.validate_submit(d, desc)?;
            }
        }
        for desc in descs {
            // SAFETY: validated above; caller owns all lifetime obligations.
            unsafe {
                self.execute_submit(d, desc)?;
                consume_submit(desc);
            }
        }
        Ok(())
    }

    unsafe fn validate_submit(&self, _d: &Device, desc: &SubmitDesc) -> Result<(), Status> {
        SubmitDesc::validate_header(desc.header)?;
        if !desc.header.next.is_null() {
            // Binary semaphores map to the presentation profile (D3); validate
            // the record shape first so malformed extensions stay distinguishable.
            // SAFETY: reads only the extension record header.
            unsafe { desc.header.next.read() }.validate::<BinarySubmitDesc>(BINARY_SUBMIT_DESC)?;
            return Err(UNSUPPORTED);
        }
        // SAFETY: caller-provided arrays of declared counts.
        let lists = unsafe { slice(desc.lists, desc.list_count)? };
        // SAFETY: caller-provided arrays of declared counts.
        let waits = unsafe { slice(desc.waits, desc.wait_count)? };
        // SAFETY: caller-provided arrays of declared counts.
        let signals = unsafe { slice(desc.signals, desc.signal_count)? };
        scratch(
            desc.scratch,
            desc.scratch_size,
            submit_scratch(desc.list_count, desc.wait_count, desc.signal_count)?,
        )?;
        for pointer in lists {
            // SAFETY: list handles are trusted live objects by contract.
            let list = unsafe { pointer.as_ref() }.ok_or(INVALID)?;
            if !list.device_matches(self.device)
                || list.domain() != self.domain
                || !list.executable()
                || !list.primary()
            {
                return Err(INVALID);
            }
        }
        for point in waits.iter().chain(signals.iter()) {
            // SAFETY: timeline handles are trusted live objects by contract.
            let timeline = unsafe { point.point.timeline.as_ref() }.ok_or(INVALID)?;
            if timeline.device() != self.device {
                return Err(INVALID);
            }
            // HOST waits/signals are invalid in submissions.
            stages(point.stages)?;
        }
        Ok(())
    }

    unsafe fn execute_submit(&self, d: &Device, desc: &SubmitDesc) -> Result<(), Status> {
        // SAFETY: full validation of this descriptor; see validate_submit.
        unsafe { self.validate_submit(d, desc)? };
        // SAFETY: caller-provided arrays of declared counts.
        let lists = unsafe { slice(desc.lists, desc.list_count)? };
        // SAFETY: caller-provided arrays of declared counts.
        let waits = unsafe { slice(desc.waits, desc.wait_count)? };
        // SAFETY: caller-provided arrays of declared counts.
        let signals = unsafe { slice(desc.signals, desc.signal_count)? };
        let storage = scratch(
            desc.scratch,
            desc.scratch_size,
            submit_scratch(desc.list_count, desc.wait_count, desc.signal_count)?,
        )?;
        let native = storage.cast::<Option<d3d::ID3D12CommandList>>();
        for (i, pointer) in lists.iter().enumerate() {
            // SAFETY: validated list above; borrowed wrapper never drops.
            let list = unsafe { pointer.as_ref() }.unwrap();
            // SAFETY: scratch slot holds a borrowed interface pointer pattern,
            // read back as a slice below and never released.
            unsafe {
                native
                    .add(i)
                    .write(Some(d3d::ID3D12CommandList::from_raw(
                        list.command_list().as_raw(),
                    )));
            }
        }
        for point in waits {
            // SAFETY: validated timeline above.
            let timeline = unsafe { point.point.timeline.as_ref() }.unwrap();
            d.submission_result(unsafe { self.handle.Wait(timeline.fence(), point.point.value) })?;
        }
        // SAFETY: scratch holds one borrowed native list pointer per entry.
        let native_lists = unsafe {
            std::slice::from_raw_parts(native, lists.len())
        };
        unsafe {
            self.handle.ExecuteCommandLists(native_lists);
        }
        for point in signals {
            // SAFETY: validated timeline above.
            let timeline = unsafe { point.point.timeline.as_ref() }.unwrap();
            d.submission_result(unsafe { self.handle.Signal(timeline.fence(), point.point.value) })?;
        }
        Ok(())
    }
}

/// One-shot lists are consumed by successful submission; replay modes are the
/// caller's resubmission permission.
unsafe fn consume_submit(desc: &SubmitDesc) {
    for i in 0..desc.list_count as usize {
        // SAFETY: validated at submission; lists stay live through this call.
        let list = unsafe { &**desc.lists.add(i) };
        if list.mode() == 1 {
            list.set_consumed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_and_access_translation_rejects_visible_gaps() {
        // HOST scopes have no D3D12 barrier equivalent and stay visible failures.
        assert!(stages(STAGE_HOST).is_err());
        assert!(access(ACCESS_HOST_READ).is_err());
        // Unenabled shader profiles are rejected until their mapping exists.
        assert!(stages(MESH_STAGE).is_err());
        assert!(stages(PREPROCESS_STAGE).is_err());
        assert!(stages(u64::MAX).is_err());
        // Ordinary scopes map to native sync/access values.
        assert!(stages(STAGE_COMPUTE).is_ok());
        assert!(stages(STAGE_ALL_GRAPHICS).is_ok());
        assert_eq!(stages(0).unwrap(), d3d::D3D12_BARRIER_SYNC_NONE);
        assert_eq!(access(0).unwrap(), d3d::D3D12_BARRIER_ACCESS_NO_ACCESS);
        assert!(access(ACCESS_STORAGE_WRITE).is_ok());
    }

    #[test]
    fn sync_scopes_narrow_to_accesses_they_justify() {
        // Transfer scopes carry resolve too; copy accesses justify only copy.
        let transfer = stages(STAGE_TRANSFER).unwrap();
        assert_eq!(
            sync_for_access(access(ACCESS_COPY_WRITE).unwrap(), transfer),
            d3d::D3D12_BARRIER_SYNC_COPY
        );
        // No declared access needs no sync at all.
        assert_eq!(
            sync_for_access(access(0).unwrap(), transfer),
            d3d::D3D12_BARRIER_SYNC_NONE
        );
        // A narrower caller scope wins when it justifies the access.
        assert_eq!(
            sync_for_access(
                access(ACCESS_SHADER_WRITE).unwrap(),
                stages(STAGE_COMPUTE).unwrap()
            ),
            d3d::D3D12_BARRIER_SYNC_COMPUTE_SHADING
        );
        // Otherwise the full required set stands: never weaker than the access.
        let full_shading = d3d::D3D12_BARRIER_SYNC_DRAW
            | d3d::D3D12_BARRIER_SYNC_COMPUTE_SHADING
            | d3d::D3D12_BARRIER_SYNC_PIXEL_SHADING
            | d3d::D3D12_BARRIER_SYNC_NON_PIXEL_SHADING
            | d3d::D3D12_BARRIER_SYNC_ALL_SHADING;
        assert_eq!(
            sync_for_access(
                access(ACCESS_SHADER_READ).unwrap(),
                stages(STAGE_COPY).unwrap()
            ),
            full_shading
        );
    }

    #[test]
    fn general_state_resolves_deterministically_from_access() {
        assert_eq!(
            layout(STATE_GENERAL, ACCESS_STORAGE_WRITE).unwrap(),
            d3d::D3D12_BARRIER_LAYOUT_UNORDERED_ACCESS
        );
        assert_eq!(
            layout(STATE_GENERAL, ACCESS_SAMPLED_READ).unwrap(),
            d3d::D3D12_BARRIER_LAYOUT_SHADER_RESOURCE
        );
        assert_eq!(
            layout(STATE_GENERAL, 0).unwrap(),
            d3d::D3D12_BARRIER_LAYOUT_COMMON
        );
        assert_eq!(
            layout(STATE_COPY_SRC, 0).unwrap(),
            d3d::D3D12_BARRIER_LAYOUT_COPY_SOURCE
        );
        assert!(layout(99, 0).is_err());
    }
}
