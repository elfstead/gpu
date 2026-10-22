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
    /// Bound graphics executable; drawing requires one.
    graphics: Cell<*const Executable>,
    /// Kinds of descriptor heaps currently bound (bit 1 resource, bit 2 sampler).
    /// Currently bound descriptor-heap range per kind. D3D12 sets the complete
    /// bound-heap set at once and invalidates descriptor tables whenever it
    /// changes, so the list remembers both bindings and re-issues them together.
    resource_binding: RefCell<Option<TableBinding>>,
    sampler_binding: RefCell<Option<TableBinding>>,
    /// Whether a render scope is active on this recording.
    pub(super) rendering: Cell<bool>,
    /// One active begin/end query per query type (occlusion, statistics).
    pub(super) active_queries: [Cell<Option<super::queries::ActiveQuery>>; 2],
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
                graphics: Cell::new(ptr::null()),
                resource_binding: RefCell::new(None),
                sampler_binding: RefCell::new(None),
                active_queries: super::queries::active_cells(),
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
            // Defensive close of abandoned recordings only: closing an
            // already-closed list is invalid and re-examines its references.
            if slot.state.get() == 1 {
                let _ = unsafe { slot.command.Close() };
            }
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
        // Only an abandoned recording (state 1) needs the defensive close:
        // closing an already-closed list is invalid and the debug layer
        // re-examines its recorded references against current resources.
        let abandoned = slot.state.get() == 1;
        slot.state.set(0);
        slot.error.set(OK);
        slot.mode = desc.replay_mode;
        // SAFETY: caller proves no pending use of this arena generation; the
        // allocator/list pair is exclusively owned by this slot. The defensive
        // close discards any abandoned recording so reset can proceed.
        let result = unsafe {
            if abandoned {
                let _ = slot.command.Close();
            }
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
    /// Recording state with no render scope active (queries reset/resolve).
    pub(super) fn outside_render(&self) -> Result<&Device, Status> {
        let d = self.recording()?;
        if self.rendering.get() {
            return Err(INVALID);
        }
        Ok(d)
    }

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
        // An already-closed list must not be closed again: the native close of
        // a closed list is invalid and re-examines its recorded references.
        eprintln!("[trace] end() state {} list {:p}", self.state.get(), windows::core::Interface::as_raw(&self.command));
        let close = if self.state.get() == 1 {
            unsafe { self.command.Close() }
        } else {
            Ok(())
        };
        let result = recording.and_then(|d| d.result(close));
        self.state.set(if result.is_ok() { 2 } else { 0 });
        result
    }

    pub(in crate::foundation) fn cancel(&self) {
        // Closing releases the allocator's recording state; contents are
        // discarded. Only an open recording is closed.
        if self.state.get() == 1 {
            let _ = unsafe { self.command.Close() };
        }
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
        eprintln!("[trace] barrier mem {} img {} global {} {} scratch {}",
            dep.memory_count, dep.image_count, dep.global_before, dep.global_after, dep.scratch_size);
        let requirements = barrier_scratch(dep.memory_count, dep.image_count)?;
        eprintln!("[trace] barrier requires {}", requirements.size);
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
        if (dep.before == 0 && dep.global_before != 0) || (dep.after == 0 && dep.global_after != 0) {
        return Err(INVALID);
        }
        // The native global barrier expresses one implied layout per side.
        // A mask mixing layout-implying accesses splits into the cross
        // product of its layout groups: every before group is ordered before
        // every after group, exactly the contract's ordering, with the
        // layout-free accesses joining every record (conservative, never
        // less). The sync bits order the stages on every record.
        let before_groups = global_groups(dep.global_before);
        let after_groups = global_groups(dep.global_after);
        let free_before = dep.global_before & ACCESS_GENERIC_FREE;
        let free_after = dep.global_after & ACCESS_GENERIC_FREE;
        // D3D12 pairs each sync bit with the accesses it justifies: the native
        // sync scope is derived from the declared accesses and narrowed to the
        // declared stage scope.
        let before = sync_for_access(access(dep.global_before)?, scope_before);
        let after = sync_for_access(access(dep.global_after)?, scope_after);
        let (groups_offset, global_offset, buffers_offset, textures_offset) =
            barrier_offsets(dep.memory_count, dep.image_count)?;
        let groups = unsafe { storage.add(groups_offset) }.cast::<d3d::D3D12_BARRIER_GROUP>();
        let global = unsafe { storage.add(global_offset) }.cast::<d3d::D3D12_GLOBAL_BARRIER>();
        let buffers = unsafe { storage.add(buffers_offset) }.cast::<d3d::D3D12_BUFFER_BARRIER>();
        let textures = unsafe { storage.add(textures_offset) }.cast::<d3d::D3D12_TEXTURE_BARRIER>();
        let mut count = 0;
        {
            let mut emitted = 0u32;
            for b in &before_groups {
                for a in &after_groups {
                    if *b == 0 || *a == 0 {
                        continue;
                    }
                    let global_before = access(*b | free_before)?;
                    let global_after = access(*a | free_after)?;
                    // The native global barrier accepts a DEPTH_STENCIL_WRITE
                    // before-side only against a DEPTH_STENCIL_READ after-side;
                    // other depth-write pairs express their ordering through
                    // the sync bits alone (the caller's resource barriers
                    // carry the cache ordering per image).
                    let depth_write_before = *b & ACCESS_DEPTH_WRITE != 0;
                    let depth_read_after = *a & ACCESS_DEPTH_READ != 0;
                    let (global_before, global_after) =
                        if depth_write_before && !depth_read_after {
                            (
                                d3d::D3D12_BARRIER_ACCESS_NO_ACCESS,
                                d3d::D3D12_BARRIER_ACCESS_NO_ACCESS,
                            )
                        } else {
                            (global_before, global_after)
                        };
                    // Each record's sync bits pair with its own accesses.
                    let sync_before = sync_for_access(global_before, scope_before);
                    let sync_after = sync_for_access(global_after, scope_after);
                    // SAFETY: scratch holds MAX_GLOBAL_BARRIERS records.
                    unsafe {
                        global.add(emitted as usize).write(d3d::D3D12_GLOBAL_BARRIER {
                            SyncBefore: sync_before,
                            SyncAfter: sync_after,
                            AccessBefore: global_before,
                            AccessAfter: global_after,
                        });
                    }
                    emitted += 1;
                }
            }
            if before_groups.iter().all(|g| *g == 0) && after_groups.iter().all(|g| *g == 0) {
                let global_before = access(free_before)?;
                let global_after = access(free_after)?;
                if global_before.0 != 0 || global_after.0 != 0 {
                    // SAFETY: scratch holds MAX_GLOBAL_BARRIERS records.
                    unsafe {
                        global.add(emitted as usize).write(d3d::D3D12_GLOBAL_BARRIER {
                            SyncBefore: sync_for_access(global_before, scope_before),
                            SyncAfter: sync_for_access(global_after, scope_after),
                            AccessBefore: global_before,
                            AccessAfter: global_after,
                        });
                    }
                    emitted += 1;
                }
            }
            if emitted != 0 {
                // SAFETY: one global group covering the emitted records.
                unsafe {
                    groups.add(count).write(d3d::D3D12_BARRIER_GROUP {
                        Type: d3d::D3D12_BARRIER_TYPE_GLOBAL,
                        NumBarriers: emitted,
                        Anonymous: d3d::D3D12_BARRIER_GROUP_0 {
                            pGlobalBarriers: global,
                        },
                    });
                }
                count += 1;
            }
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
                eprintln!("[trace] image barrier {} -> {} access {} -> {}", barrier.old_state, barrier.new_state, barrier.before, barrier.after);
                if barrier.new_state == STATE_UNDEFINED {
        return Err(INVALID);
                }
                domains(d, barrier.source_domain, barrier.destination_domain)?;
                let before_access = access(barrier.before)?;
                let after_access = access(barrier.after)?;
                // The native pairing rule: an UNDEFINED layout pairs with
                // NO_ACCESS only. A discarded side has no prior access to
                // express, whatever the caller's access mask names.
                let before_access = if barrier.old_state == STATE_UNDEFINED {
                    d3d::D3D12_BARRIER_ACCESS_NO_ACCESS
                } else {
                    before_access
                };
                let after_access = if barrier.new_state == STATE_UNDEFINED {
                    d3d::D3D12_BARRIER_ACCESS_NO_ACCESS
                } else {
                    after_access
                };
                let before = sync_for_access(before_access, scope_before);
                let after = sync_for_access(after_access, scope_after);
                eprintln!("[trace] barrier states {} -> {} access {} -> {}", barrier.old_state, barrier.new_state, barrier.before, barrier.after);
                let layout_before = layout(barrier.old_state, barrier.before)?;
                let layout_after = layout(barrier.new_state, barrier.after)?;
                eprintln!("[trace] subresources enter mips {}+{} layers {}+{} aspects {}", barrier.range.first_mip, barrier.range.mip_count, barrier.range.first_layer, barrier.range.layer_count, barrier.range.aspects);
                let subresources = subresources(&barrier.range, image)?;
                // ABI values of D3D12_TEXTURE_BARRIER_FLAG_NONE/DISCARD.
                eprintln!("[trace] texture record flags {} sub {:?}x{:?}", barrier.discard, subresources.IndexOrFirstMipLevel, subresources.NumMipLevels);
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

/// Worst-case native global records for one dependency: a mixed-access global
/// barrier splits into the cross product of its implied-layout groups (five
/// groups per side by the access classification below).
const MAX_GLOBAL_BARRIERS: u32 = 25;

fn barrier_offsets(
    memory: u32,
    images: u32,
) -> Result<(usize, usize, usize, usize), Status> {
    let groups = std::alloc::Layout::new::<d3d::D3D12_BARRIER_GROUP>()
        .repeat(3)
        .map_err(|_| INVALID)?
        .0;
    let (layout, global_offset) = groups
        .extend(array_layout::<d3d::D3D12_GLOBAL_BARRIER>(MAX_GLOBAL_BARRIERS)?)
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
    // The same layout as translate_dependency builds in scratch: one source
    // of truth for the record groups (including the global-barrier split).
    let (groups_offset, global_offset, buffers_offset, textures_offset) =
        barrier_offsets(memory, images)?;
    let _ = (groups_offset, global_offset);
    let groups = std::alloc::Layout::new::<d3d::D3D12_BARRIER_GROUP>()
        .repeat(3)
        .map_err(|_| INVALID)?
        .0;
    let (layout, _) = groups
        .extend(array_layout::<d3d::D3D12_GLOBAL_BARRIER>(MAX_GLOBAL_BARRIERS)?)
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
pub(super) fn stages(mask: u64) -> Result<d3d::D3D12_BARRIER_SYNC, Status> {
    const KNOWN: u64 = (1 << 23) - 1;
    if mask & !KNOWN != 0 {
        return Err(UNSUPPORTED);
    }
    if mask & (MESH_STAGE | TASK_STAGE | PREPROCESS_STAGE) != 0 {
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
    let rest = rest & !STAGE_HOST;
    if rest != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(result)
}

/// Accesses that imply no native layout: they join every split global barrier.
const ACCESS_GENERIC_FREE: u64 = ACCESS_COPY_READ
    | ACCESS_COPY_WRITE
    | ACCESS_INDIRECT_READ
    | ACCESS_INDEX_READ
    | ACCESS_VERTEX_READ
    | ACCESS_UNIFORM_READ
    | ACCESS_HOST_READ
    | ACCESS_HOST_WRITE;

/// Partition one access mask into its implied-layout groups. Each group maps
/// to exactly one native layout; the generic read/write bits expand into the
/// groups they cover. The order is render target, depth write, depth read,
/// shader resource, unordered access.
fn global_groups(mask: u64) -> [u64; 5] {
    let read = mask & ACCESS_READ != 0;
    let write = mask & ACCESS_WRITE != 0;
    let expand = |bit: u64, on: bool| -> u64 { if on { bit } else { 0 } };
    [
        (mask & (ACCESS_COLOR_READ | ACCESS_COLOR_WRITE)) | expand(ACCESS_COLOR_WRITE, write),
        (mask & ACCESS_DEPTH_WRITE) | expand(ACCESS_DEPTH_WRITE, write),
        (mask & ACCESS_DEPTH_READ) | expand(ACCESS_DEPTH_READ, read),
        (mask & (ACCESS_SHADER_READ | ACCESS_SAMPLED_READ)) | expand(ACCESS_SHADER_READ, read),
        (mask & (ACCESS_SHADER_WRITE | ACCESS_STORAGE_READ | ACCESS_STORAGE_WRITE))
            | expand(ACCESS_SHADER_WRITE, write),
    ]
}

/// Access translation. Broad generic reads/writes map to the conservative union
/// of their native accesses; descriptor-heap accesses belong to the D2 profile.
fn access(mask: u64) -> Result<d3d::D3D12_BARRIER_ACCESS, Status> {
    const KNOWN: u64 = (1 << 21) - 1;
    if mask & !KNOWN != 0 {
        return Err(UNSUPPORTED);
    }
    if mask & (ACCESS_RESOURCE_HEAP_READ | ACCESS_SAMPLER_HEAP_READ) != 0
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
        let _ = d;
        // SAFETY: the executable pointer is set only by bind_executable and the
        // contract keeps executables alive through all recorded/pending uses.
        let cell = match scope {
            STAGE_COMPUTE => &self.executable,
            s if s != 0
                && s & !(STAGE_VERTEX | STAGE_FRAGMENT | MESH_STAGE | TASK_STAGE) == 0 =>
            {
                &self.graphics
            }
            _ => return Err(UNSUPPORTED),
        };
        unsafe { cell.get().as_ref() }.ok_or(INVALID)
    }

    /// Bind prepared compute state. Root arguments are re-emitted from the
    /// argument shadow so bytes survive root-signature changes.
    pub(in crate::foundation) fn bind_executable(
        &self,
        executable: &Executable,
    ) -> Result<(), Status> {
        let d = self.recording()?;
        if !ptr::eq(executable.device, d) {
        return Err(INVALID);
        }
        let domain = d
            .snapshot
            .queues
            .iter()
            .find(|q| q.domain == self.domain)
            .ok_or(INVALID)?;
        let compute = executable.stages == STAGE_COMPUTE;
        if compute {
            if domain.flags & COMPUTE == 0 {
                return Err(UNSUPPORTED);
            }
        } else if domain.flags & GRAPHICS == 0 {
            return Err(UNSUPPORTED);
        }
        // SAFETY: prepared objects stay live through all recorded uses.
        unsafe {
            self.command.SetComputeRootSignature(&executable.signature);
            self.command.SetGraphicsRootSignature(&executable.signature);
            self.command.SetPipelineState(&executable.pipeline);
            self.emit_arguments(executable)?;
        }
        self.apply_descriptor_state(d)?;
        if compute {
            self.executable.set(executable);
        } else {
            // Input-assembler state follows the executable's topology.
            // SAFETY: recording slot is live.
            unsafe {
                self.command.IASetPrimitiveTopology(topology(executable.topology)?);
            }
            self.graphics.set(executable);
        }
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
        if executable.byte_size == 0 {
            return Ok(());
        }
        // SAFETY: recording slot and executable are live; bytes are plain data.
        // Both pipeline classes see one shared namespace (the SHARED_BYTES
        // profile), so every update reaches compute and graphics root state.
        unsafe {
            self.command.SetComputeRoot32BitConstants(
                0,
                executable.byte_size / 4,
                shadow[..bytes].as_ptr().cast(),
                0,
            );
            self.command.SetGraphicsRoot32BitConstants(
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
                self.command.SetComputeRootUnorderedAccessView(
                    executable.root_parameter(slot as u32),
                    u64::from_le_bytes(address),
                );
                self.command.SetGraphicsRootUnorderedAccessView(
                    executable.root_parameter(slot as u32),
                    u64::from_le_bytes(address),
                );
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
        if size == 0 {
            return Ok(());
        }
        // SHARED_BYTES: inline may intentionally change root-slot bytes ("set_root
        // changes only its 8 bytes; inline may intentionally change those same
        // bytes"). Any write touching a slot updates its root descriptor from the
        // resulting namespace bytes, exactly as set_root would.
        let mut rebound: Vec<(u32, u64)> = Vec::new();
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
            for (slot, root) in executable.roots.iter().enumerate() {
                if offset < root.offset + 8 && root.offset < offset + size {
                    let start = root.offset as usize;
                    let bytes: [u8; 8] = shadow[start..start + 8].try_into().unwrap();
                    rebound.push((slot as u32, u64::from_le_bytes(bytes)));
                }
            }
        }
        for (slot, address) in rebound {
            // SAFETY: recording slot is live; the address is caller-owned data.
            unsafe {
                self.command
                    .SetComputeRootUnorderedAccessView(executable.root_parameter(slot), address);
                self.command
                    .SetGraphicsRootUnorderedAccessView(executable.root_parameter(slot), address);
            }
        }
        // SAFETY: recording slot is live; data is readable for this call.
        unsafe {
            self.command
                .SetComputeRoot32BitConstants(0, size / 4, data.cast(), offset / 4);
            self.command
                .SetGraphicsRoot32BitConstants(0, size / 4, data.cast(), offset / 4);
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
                .SetComputeRootUnorderedAccessView(executable.root_parameter(slot), address);
            self.command
                .SetGraphicsRootUnorderedAccessView(executable.root_parameter(slot), address);
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

    /// Indirect draws through the device's draw command signatures. The wire
    /// records are the native draw argument layouts; the optional count span
    /// supplies one u32 argument count (native min(count, maximum) semantics).
    /// # Safety
    /// The argument and count spans stay live and valid for all recorded and
    /// pending uses; execution-time index/binding obligations are the caller's.
    pub(in crate::foundation) unsafe fn draw_indirect(
        &self,
        desc: &Indirect,
        indexed: bool,
    ) -> Result<(), Status> {
        self.bound_executable(STAGE_VERTEX)?;
        if !self.rendering.get() {
            return Err(INVALID);
        }
        // SAFETY: device pointer is live for every list operation.
        let d = unsafe { &*self.device };
        if desc.maximum_count > d.snapshot.graphics_limits.max_indirect_count {
            return Err(UNSUPPORTED);
        }
        // Native wire records are the D3D12 draw argument layouts.
        let record = if indexed { 20u32 } else { 16u32 };
        let stride = desc.stride;
        if desc.maximum_count > 1 {
            // The native command stride is fixed to the record size; packed
            // records are the portable shape and wider strides stay visible.
            if stride != record {
                return Err(UNSUPPORTED);
            }
        } else if stride != 0 && (stride % 4 != 0 || stride < record) {
            return Err(INVALID);
        }
        if desc.maximum_count == 0 {
            return Err(INVALID);
        }
        // SAFETY: span memory handle is a trusted live object by contract.
        let arguments = unsafe { desc.arguments.memory.as_ref() }.ok_or(INVALID)?;
        let span = desc.arguments;
        let (buffer, offset, _) =
            arguments.command_range(d, self.domain, span.offset, span.size, USAGE_INDIRECT)?;
        if offset % 4 != 0 {
            return Err(INVALID);
        }
        let covered = u64::from(desc.maximum_count - 1) * u64::from(stride) + u64::from(record);
        if span.size < covered {
            return Err(INVALID);
        }
        let count_span = desc.count;
        let (count_buffer, count_offset) = if count_span.memory.is_null() {
            if count_span.offset != 0 || count_span.size != 0 {
                return Err(INVALID);
            }
            (None, 0)
        } else {
            // SAFETY: span memory handle is a trusted live object by contract.
            let memory = unsafe { count_span.memory.as_ref() }.ok_or(INVALID)?;
            let (count_buffer, count_offset, _) = memory.command_range(
                d,
                self.domain,
                count_span.offset,
                count_span.size,
                USAGE_INDIRECT,
            )?;
            if count_offset % 4 != 0 || count_span.size < 4 {
                return Err(INVALID);
            }
            (Some(count_buffer), count_offset as u64)
        };
        let signature = if indexed {
            &d.draw_indexed_signature
        } else {
            &d.draw_signature
        };
        // SAFETY: validated argument/count ranges of live buffers; recording only.
        unsafe {
            self.command.ExecuteIndirect(
                signature,
                desc.maximum_count,
                buffer,
                offset as u64,
                count_buffer,
                count_offset,
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
        eprintln!("[trace] bind_heap enter kind {} off {} size {}", binding.kind, binding.storage.offset, binding.storage.size);
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
        let (stride, _heap_bit) = match binding.kind {
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
        // Remember this kind's binding and re-issue the complete heap set:
        // SetDescriptorHeaps replaces the bound set and invalidates tables.
        let (gpu, _stride) = heaps.gpu_start(d, binding.kind)?;
        let handle = d3d::D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: gpu.ptr
                + u64::from(super::descriptors::native_stride(d, binding.kind))
                    * u64::from(base_index),
        };
        let table = TableBinding {
            heap: match binding.kind {
                HEAP_SAMPLER => heaps.sampler.clone(),
                _ => heaps.resource.clone(),
            },
            handle,
        };
        match binding.kind {
            HEAP_SAMPLER => *self.sampler_binding.borrow_mut() = Some(table),
            _ => *self.resource_binding.borrow_mut() = Some(table),
        };
        self.apply_descriptor_state(d)?;
        Ok(())
    }

    /// Re-issue the complete descriptor-heap set and every bound table. Any
    /// change to the set invalidates tables and root arguments natively.
    fn apply_descriptor_state(&self, _d: &Device) -> Result<(), Status> {
        let resource = self.resource_binding.borrow();
        let sampler = self.sampler_binding.borrow();
        let mut bound: Vec<Option<d3d::ID3D12DescriptorHeap>> = Vec::with_capacity(2);
        if let Some(binding) = resource.as_ref() {
            bound.push(Some(binding.heap.clone()));
        }
        if let Some(binding) = sampler.as_ref() {
            bound.push(Some(binding.heap.clone()));
        }
        if !bound.is_empty() {
            // SAFETY: recording slot is live; heap references stay live through use.
            unsafe {
                self.command.SetDescriptorHeaps(&bound);
            }
        }
        // Root arguments reach both root states; table parameters come from
        // each bound executable's own layout (compute and graphics can differ).
        for bound_executable in [self.executable.get(), self.graphics.get()] {
            let Some(executable) = (unsafe { bound_executable.as_ref() }) else {
                continue;
            };
            // SAFETY: recording slot and executable are live.
            unsafe { self.emit_arguments(executable)? };
            // SAFETY: recording slot is live; heap bindings stay live through use.
            unsafe {
                if let Some(binding) = resource.as_ref() {
                    // UAV and SRV tables share the heap range start: one slot
                    // space for buffers and images alike.
                    for offset in [0u32, 1] {
                        self.command.SetComputeRootDescriptorTable(
                            executable.table_parameter() + offset,
                            binding.handle,
                        );
                        self.command.SetGraphicsRootDescriptorTable(
                            executable.table_parameter() + offset,
                            binding.handle,
                        );
                    }
                }
                if let Some(binding) = sampler.as_ref() {
                    self.command.SetComputeRootDescriptorTable(
                        executable.sampler_parameter(),
                        binding.handle,
                    );
                    self.command.SetGraphicsRootDescriptorTable(
                        executable.sampler_parameter(),
                        binding.handle,
                    );
                }
            }
        }
        Ok(())
    }
}

/// One bound descriptor-heap range: the native heap and the table start handle.
#[derive(Clone)]
pub(super) struct TableBinding {
    heap: d3d::ID3D12DescriptorHeap,
    handle: d3d::D3D12_GPU_DESCRIPTOR_HANDLE,
}

/// Shared primitive topology mapping for the input assembler.
fn topology(value: u32) -> Result<windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY, Status> {
    use windows::Win32::Graphics::Direct3D as d3dtopo;
    Ok(match value {
        POINTS => d3dtopo::D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        LINES => d3dtopo::D3D_PRIMITIVE_TOPOLOGY_LINELIST,
        LINE_STRIP => d3dtopo::D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
        TRIANGLES => d3dtopo::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        TRIANGLE_STRIP => d3dtopo::D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        _ => return Err(INVALID),
    })
}

/// Scratch holds the native vertex-buffer view array (one per binding).
pub(in crate::foundation) fn vertex_scratch(count: u32) -> Result<HostRequirements, Status> {
    if count == 0 {
        return Ok(HostRequirements {
            size: 0,
            alignment: 1,
        });
    }
    Ok(HostRequirements {
        size: u64::from(count) * 24,
        alignment: 8,
    })
}

impl List {
    /// Viewport and scissor state (dynamically supported on this backend).
    pub(in crate::foundation) fn viewport(&self, state: &ViewportState) -> Result<(), Status> {
        self.bound_executable(STAGE_VERTEX | STAGE_FRAGMENT)?;
        state.header.validate::<ViewportState>(VIEWPORT_STATE)?;
        if state.width == 0.0 || state.height == 0.0 {
        return Err(INVALID);
        }
        let viewport = d3d::D3D12_VIEWPORT {
            TopLeftX: state.x,
            TopLeftY: state.y,
            Width: state.width,
            Height: state.height,
            MinDepth: state.min_depth,
            MaxDepth: state.max_depth,
        };
        let scissor = windows::Win32::Foundation::RECT {
            left: state.scissor_x,
            top: state.scissor_y,
            right: state.scissor_x + state.scissor_width as i32,
            bottom: state.scissor_y + state.scissor_height as i32,
        };
        // SAFETY: recording slot is live; state is validated.
        unsafe {
            self.command.RSSetViewports(&[viewport]);
            self.command.RSSetScissorRects(&[scissor]);
        }
        Ok(())
    }

    /// Blend constants are dynamic on this backend.
    pub(in crate::foundation) fn blend_constants(&self, values: &[f32; 4]) -> Result<(), Status> {
        self.bound_executable(STAGE_VERTEX | STAGE_FRAGMENT)?;
        if values.iter().any(|v| !v.is_finite()) {
        return Err(INVALID);
        }
        // SAFETY: recording slot is live.
        unsafe { self.command.OMSetBlendFactor(Some(values)) };
        Ok(())
    }

    /// Stencil reference is dynamic; stencil masks are PSO state on this backend.
    pub(in crate::foundation) fn stencil_value(
        &self,
        field: u32,
        _faces: u32,
        value: u32,
    ) -> Result<(), Status> {
        self.bound_executable(STAGE_VERTEX | STAGE_FRAGMENT)?;
        if field != 4 {
            return Err(UNSUPPORTED);
        }
        // SAFETY: recording slot is live.
        unsafe { self.command.OMSetStencilRef(value) };
        Ok(())
    }

    /// Depth/stencil/cull/front-face dynamic states are PSO state on this
    /// backend and reject rather than being silently baked.
    pub(in crate::foundation) fn dynamic_raster_value(
        &self,
        _field: u32,
        _value: u32,
    ) -> Result<(), Status> {
        self.bound_executable(STAGE_VERTEX | STAGE_FRAGMENT)?;
        Err(UNSUPPORTED)
    }

    /// Stencil operations are PSO state on this backend.
    pub(in crate::foundation) fn stencil_ops(
        &self,
        _faces: u32,
        _fail: u32,
        _pass: u32,
        _depth_fail: u32,
        _compare: u32,
    ) -> Result<(), Status> {
        self.bound_executable(STAGE_VERTEX | STAGE_FRAGMENT)?;
        Err(UNSUPPORTED)
    }

    pub(in crate::foundation) fn draw(&self, draw: &DrawDesc, indexed: bool) -> Result<(), Status> {
        self.bound_executable(STAGE_VERTEX)?;
        if draw.instances == 0 {
        return Err(INVALID);
        }
        // SAFETY: recording slot is live.
        unsafe {
            if indexed {
                self.command.DrawIndexedInstanced(
                    draw.count,
                    draw.instances,
                    draw.first,
                    draw.vertex_offset,
                    draw.first_instance,
                );
            } else {
                self.command.DrawInstanced(
                    draw.count,
                    draw.instances,
                    draw.first,
                    draw.first_instance,
                );
            }
        }
        Ok(())
    }

    pub(in crate::foundation) fn bind_indices(
        &self,
        span: Span,
        kind: u32,
    ) -> Result<(), Status> {
        // The contract: index binding persists across render scopes and
        // pipeline binds and requires no executable.
        let d = self.recording()?;
        let _ = d;
        let format = match kind {
            INDEX_U16 => windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16_UINT,
            INDEX_U32 => windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R32_UINT,
            _ => return Err(INVALID),
        };
        // SAFETY: span memory handle is a trusted live object by contract.
        let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
        let (buffer, offset, _) =
            // SAFETY: the device pointer is live for every list operation.
            memory.command_range(unsafe { &*self.device }, self.domain, span.offset, span.size, USAGE_INDEX)?;
        let view = d3d::D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: {
                // SAFETY: buffer stays live for the query.
                let base = unsafe { buffer.GetGPUVirtualAddress() };
                base + offset
            },
            SizeInBytes: span.size as u32,
            Format: format,
        };
        // SAFETY: recording slot is live; the view names a live buffer.
        unsafe { self.command.IASetIndexBuffer(Some(&view)) };
        Ok(())
    }

    /// Vertex bindings translate to native vertex-buffer views placed in the
    /// caller scratch, one view per binding.
    /// # Safety
    /// Spans and scratch stay live through all recorded/pending uses.
    pub(in crate::foundation) fn bind_vertices(
        &self,
        first: u32,
        count: u32,
        spans: *const Span,
        scratch: HostSpan,
    ) -> Result<(), Status> {
        // The contract's vertex binding leaves pipeline static strides
        // unchanged: the bound pipeline supplies the stride for the native
        // vertex-buffer view, so a bound executable is required here until a
        // deferred-stride binding exists (recorded in the D2 receipt).
        let executable = self.bound_executable(STAGE_VERTEX)?;
        // SAFETY: trusted span array of the declared count.
        let spans = unsafe { super::slice(spans, count)? };
        let requirement = vertex_scratch(count)?;
        if count == 0 {
            return Ok(());
        }
        if scratch.size < requirement.size
            || scratch.data.is_null()
            || scratch.data as usize % requirement.alignment as usize != 0
        {
        return Err(INVALID);
        }
        if u64::from(first) + u64::from(count) > 32 {
            return Err(UNSUPPORTED);
        }
        let views = scratch.data.cast::<d3d::D3D12_VERTEX_BUFFER_VIEW>();
        for (index, span) in spans.iter().enumerate() {
            // SAFETY: span memory handle is a trusted live object by contract.
            let memory = unsafe { span.memory.as_ref() }.ok_or(INVALID)?;
            let (buffer, offset, _) = memory.command_range(
                // SAFETY: the device pointer is live for every list operation.
                unsafe { &*self.device },
                self.domain,
                span.offset,
                span.size,
                USAGE_VERTEX,
            )?;
            // SAFETY: scratch is writable for the declared requirement.
            unsafe {
                views.add(index).write(d3d::D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: {
                        // SAFETY: buffer stays live for the query.
                        let base = unsafe { buffer.GetGPUVirtualAddress() };
                        base + offset
                    },
                    SizeInBytes: span.size as u32,
                    StrideInBytes: *executable
                        .strides
                        .get(first as usize + index)
                        .ok_or(INVALID)?,
                });
            }
        }
        // SAFETY: recording slot is live; the views name live buffers.
        unsafe {
            self.command.IASetVertexBuffers(
                first,
                Some(std::slice::from_raw_parts(views, count as usize)),
            );
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
            // Binary semaphores lower to native queue wait/signal pairs.
            // SAFETY: complete extension record behind the chained header.
            unsafe { desc.header.next.read() }.validate::<BinarySubmitDesc>(BINARY_SUBMIT_DESC)?;
            // SAFETY: complete extension record behind the chained header.
            let extension = unsafe { &*desc.header.next.cast::<BinarySubmitDesc>() };
            // SAFETY: trusted point arrays of the declared counts.
            let waits = unsafe { slice(extension.waits, extension.wait_count)? };
            let signals = unsafe { slice(extension.signals, extension.signal_count)? };
            for point in waits.iter().chain(signals.iter()) {
                // SAFETY: binary handles are trusted live objects by contract.
                if unsafe { point.binary.as_ref() }.is_none() {
                    return Err(INVALID);
                }
            }
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
        // Binary waits order against earlier submissions' signals.
        if !desc.header.next.is_null() {
            // SAFETY: complete extension record behind the chained header.
            let extension = unsafe { &*desc.header.next.cast::<BinarySubmitDesc>() };
            // SAFETY: validated point arrays of the declared counts.
            let binary_waits = unsafe { slice(extension.waits, extension.wait_count)? };
            for point in binary_waits {
                // SAFETY: validated binary above.
                let binary = unsafe { point.binary.as_ref() }.unwrap();
                unsafe { binary.wait_on(&self.handle) }?;
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
        // Binary signals retire against later waits on their queue.
        if !desc.header.next.is_null() {
            // SAFETY: complete extension record behind the chained header.
            let extension = unsafe { &*desc.header.next.cast::<BinarySubmitDesc>() };
            // SAFETY: validated point arrays of the declared counts.
            let binary_signals = unsafe { slice(extension.signals, extension.signal_count)? };
            for point in binary_signals {
                // SAFETY: validated binary above.
                let binary = unsafe { point.binary.as_ref() }.unwrap();
                unsafe { binary.schedule(&self.handle) }?;
            }
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
        assert_eq!(access(ACCESS_HOST_READ).map(|a| a.0), Ok(0));
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
