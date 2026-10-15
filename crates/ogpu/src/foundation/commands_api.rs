//! C command boundary. Encoder and executable-list handles borrow arena slots.
use super::*;

/// # Safety
/// Live device, disjoint writable output; caller owns split and asynchronous uses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_split_create(
    device: *mut Device,
    out: *mut *mut Split,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let split = unsafe { rust::split_create(d)? };
        unsafe {
            out.write(split.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// NULL or uniquely owned split, live parent device, no recorded future/pending use.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_split_destroy(split: *mut Split) {
    if let Some(split) = unsafe { rust::SplitHandle::from_raw(split) } {
        unsafe {
            rust::split_destroy(split);
        }
    }
}
/// # Safety
/// Live same-device objects, exclusive encoder access and caller-ordered reset.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_split_reset(
    encoder: *mut List,
    split: *const Split,
    before: u64,
) {
    unsafe {
        encode(encoder, |e| {
            e.split_reset(split.as_ref().ok_or(INVALID)?, before)
        });
    }
}
/// # Safety
/// Valid objects/dependency/scratch, exclusive encoder; caller guarantees matching
/// acquire, event-cycle ordering, asynchronous resource lifetime and queue rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_split_release(
    encoder: *mut List,
    split: *const Split,
    dep: *const Dependency,
) {
    unsafe {
        encode(encoder, |e| {
            e.split_release(
                split.as_ref().ok_or(INVALID)?,
                description(dep, DEPENDENCY)?,
            )
        });
    }
}
/// # Safety
/// Disjoint writable requirements output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_split_scratch_requirements(
    count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = rust::split_scratch_requirements(count)?;
        Ok(())
    })
}
/// # Safety
/// Valid matching events/dependencies and disjoint scratch; see ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_split_acquire(
    encoder: *mut List,
    count: u32,
    waits: *const SplitDependency,
    storage: HostSpan,
) {
    unsafe {
        encode(encoder, |e| e.split_acquire(count, waits, storage));
    }
}

unsafe fn description<'a, T>(pointer: *const T, kind: u32) -> Result<&'a T, Status> {
    if pointer.is_null() {
        return Err(INVALID);
    }
    unsafe { pointer.cast::<Record>().read() }.validate::<T>(kind)?;
    Ok(unsafe { &*pointer })
}
/// # Safety
/// Live exclusive encoder and readable four-float array; native dynamic-state lifetime rules apply.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_blend_constants(
    encoder: *mut List,
    values: *const [f32; 4],
) {
    unsafe {
        encode(encoder, |e| {
            e.blend_constants(values.as_ref().ok_or(INVALID)?)
        })
    };
}
/// # Safety
/// Live exclusive encoder; caller initializes required faces and obeys native state lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_stencil_reference(
    encoder: *mut List,
    faces: u32,
    value: u32,
) {
    unsafe { encode(encoder, |e| e.stencil_value(4, faces, value)) };
}
/// # Safety
/// Same as ogpu_next_set_stencil_reference.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_stencil_compare_mask(
    encoder: *mut List,
    faces: u32,
    value: u32,
) {
    unsafe { encode(encoder, |e| e.stencil_value(8, faces, value)) };
}
/// # Safety
/// Same as ogpu_next_set_stencil_reference.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_stencil_write_mask(
    encoder: *mut List,
    faces: u32,
    value: u32,
) {
    unsafe { encode(encoder, |e| e.stencil_value(16, faces, value)) };
}
/// # Safety
/// Live device and description/output as documented in include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_create(
    device: *mut Device,
    desc: *const ArenaDesc,
    out: *mut *mut Arena,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let arena = unsafe { rust::arena_create(d, description(desc, ARENA_DESC)?)? };
        unsafe {
            out.write(arena.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Exclusive arena/pool host access; existing slot handles remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_reserve(arena: *mut Arena, count: u32) -> Status {
    boundary(|| unsafe {
        rust::arena_reserve(rust::ArenaHandle::from_raw(arena).ok_or(INVALID)?, count)
    })
}
/// # Safety
/// No pending use; excludes every list/encoder/pool access. Invalidates all handles.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_reset(arena: *mut Arena) -> Status {
    boundary(|| unsafe { rust::arena_reset(rust::ArenaHandle::from_raw(arena).ok_or(INVALID)?) })
}
/// # Safety
/// Same exclusion/lifetime requirements as reset.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_trim(arena: *mut Arena, count: u32) -> Status {
    boundary(|| unsafe {
        rust::arena_trim(rust::ArenaHandle::from_raw(arena).ok_or(INVALID)?, count)
    })
}
/// # Safety
/// No pending or host uses; live parent device. NULL permitted. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_destroy(arena: *mut Arena) {
    if let Some(arena) = unsafe { rust::ArenaHandle::from_raw(arena) } {
        unsafe {
            rust::arena_destroy(arena);
        }
    }
}
/// # Safety
/// Exclusive pool access, readable description and writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_commands_begin(
    arena: *mut Arena,
    desc: *const RecordingDesc,
    out: *mut *mut List,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let arena = unsafe { rust::ArenaHandle::from_raw(arena) }.ok_or(INVALID)?;
        let encoder = unsafe { rust::commands_begin(arena, description(desc, RECORDING_DESC)?)? };
        unsafe {
            out.write(encoder.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Live recording slot with exclusive pool access; output does not alias input.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_commands_end(encoder: *mut List, out: *mut *mut List) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let encoder =
            unsafe { rust::commands_end(rust::ListHandle::from_raw(encoder).ok_or(INVALID)?)? };
        unsafe {
            out.write(encoder.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Live recording slot, exclusive pool access. NULL permitted.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_commands_cancel(encoder: *mut List) {
    if let Some(encoder) = unsafe { rust::ListHandle::from_raw(encoder) } {
        unsafe {
            rust::commands_cancel(encoder);
        }
    }
}
/// # Safety
/// Writable requirements output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_submit_scratch_requirements(
    lists: u32,
    waits: u32,
    signals: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let requirements = rust::submit_scratch_requirements(lists, waits, signals)?;
        unsafe {
            out.write(requirements);
        }
        Ok(())
    })
}
/// # Safety
/// Writable requirements output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_barrier_scratch_requirements(
    memory_count: u32,
    image_count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let requirements = rust::barrier_scratch_requirements(memory_count, image_count)?;
        unsafe {
            out.write(requirements);
        }
        Ok(())
    })
}
/// # Safety
/// See include/ogpu_next.h: queue exclusion, scratch, resource and pending lifetimes.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_queue_submit(
    queue: *mut Queue,
    desc: *const SubmitDesc,
) -> Status {
    boundary(|| {
        let queue = unsafe { rust::QueueHandle::from_raw(queue) }.ok_or(INVALID)?;
        if desc.is_null() {
            return Err(INVALID);
        }
        SubmitDesc::validate_header(unsafe { desc.cast::<Record>().read() })?;
        unsafe { rust::queue_submit_record(queue, &*desc) }
    })
}
/// # Safety
/// Writable requirements output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_submit_batch_scratch_requirements(
    count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let requirements = rust::submit_batch_scratch_requirements(count)?;
        unsafe {
            out.write(requirements);
        }
        Ok(())
    })
}
/// # Safety
/// Live queue, readable descriptor array and mutually disjoint scratch. Caller
/// excludes queue access and satisfies all pending/list/timeline lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_queue_submit_batch(
    queue: *mut Queue,
    count: u32,
    descs: *const SubmitDesc,
    scratch: HostSpan,
) -> Status {
    boundary(|| {
        let queue = unsafe { rust::QueueHandle::from_raw(queue) }.ok_or(INVALID)?;
        unsafe { rust::queue_submit_batch_raw(queue, count, descs, scratch) }
    })
}
/// # Safety
/// Writable requirements output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_execute_scratch_requirements(
    count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let value = rust::execute_scratch_requirements(count)?;
        unsafe {
            out.write(value);
        }
        Ok(())
    })
}
/// # Safety
/// Writable requirements output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_inheritance_scratch_requirements(
    count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let value = rust::inheritance_scratch_requirements(count)?;
        unsafe {
            out.write(value);
        }
        Ok(())
    })
}
/// # Safety
/// Live primary encoder and secondary handles. Caller owns disjoint translation
/// scratch, parent/child lifetimes, reset invalidation, replay and compatibility.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_execute_lists(
    encoder: *mut List,
    count: u32,
    lists: *const *mut List,
    scratch: HostSpan,
) {
    unsafe {
        encode(encoder, |e| e.execute(count, lists, scratch));
    }
}
pub(super) unsafe fn encode(encoder: *mut List, f: impl FnOnce(&List) -> Result<(), Status>) {
    unsafe {
        native_encode(encoder, |handle| rust::encode(handle, f));
    }
}
unsafe fn native_encode(encoder: *mut List, f: impl FnOnce(rust::ListHandle)) {
    if let Some(handle) = unsafe { rust::ListHandle::from_raw(encoder) } {
        // The native encoder poisons on errors/unwind; C additionally contains
        // panics. No duplicate poison bookkeeping or C call on the Rust path.
        let _ = boundary(|| {
            f(handle);
            Ok(())
        });
    }
}
/// # Safety
/// Live device and readable description; disjoint output. Device outlives the pool.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_query_pool_create(
    device: *mut Device,
    desc: *const QueryPoolDesc,
    out: *mut *mut QueryPool,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let pool = unsafe { rust::query_pool_create(device, description(desc, QUERY_POOL_DESC)?)? };
        unsafe {
            out.write(pool.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// No recorded future/pending/host use and live device. NULL allowed. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_query_pool_destroy(pool: *mut QueryPool) {
    if let Some(pool) = unsafe { rust::QueryPoolHandle::from_raw(pool) } {
        unsafe {
            rust::query_pool_destroy(pool);
        }
    }
}
/// # Safety
/// Live pool/encoder, exclusive pool recording; caller synchronizes query reuse.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_queries_reset(
    encoder: *mut List,
    pool: *mut QueryPool,
    first: u32,
    count: u32,
) {
    unsafe {
        encode(encoder, |e| {
            e.queries_reset(pool.as_ref().ok_or(INVALID)?, first, count)
        });
    }
}
/// # Safety
/// Live occlusion/statistics pool, unavailable query at execution and exclusive recording.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_query_begin(
    encoder: *mut List,
    pool: *mut QueryPool,
    index: u32,
    flags: u32,
) {
    unsafe {
        encode(encoder, |e| {
            e.query(pool.as_ref().ok_or(INVALID)?, index, true, flags)
        });
    }
}
/// # Safety
/// Matching active query in this encoder and rendering scope.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_query_end(encoder: *mut List, pool: *mut QueryPool, index: u32) {
    unsafe {
        encode(encoder, |e| {
            e.query(pool.as_ref().ok_or(INVALID)?, index, false, 0)
        });
    }
}
/// # Safety
/// Live timestamp pool, query reset before execution, valid scope and exclusive recording.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_timestamp(
    encoder: *mut List,
    pool: *mut QueryPool,
    index: u32,
    scope: u64,
) {
    unsafe {
        encode(encoder, |e| {
            e.timestamp(pool.as_ref().ok_or(INVALID)?, index, scope)
        });
    }
}
/// # Safety
/// Live pool/destination, valid initialized query lifecycle and synchronized output uses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_queries_resolve(
    encoder: *mut List,
    pool: *mut QueryPool,
    first: u32,
    count: u32,
    dst: Span,
    stride: u64,
    flags: u32,
) {
    unsafe {
        encode(encoder, |e| {
            e.queries_resolve(
                pool.as_ref().ok_or(INVALID)?,
                first,
                count,
                dst,
                stride,
                flags,
            )
        });
    }
}
/// # Safety
/// Writable disjoint output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_render_scratch_requirements(
    colors: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        unsafe {
            out.write(rust::render_scratch_requirements(colors)?);
        }
        Ok(())
    })
}
/// # Safety
/// See header: live attachments, declared layouts, compatible draw state and scratch.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_render_begin(encoder: *mut List, desc: *const RenderDesc) {
    unsafe {
        encode(encoder, |e| e.render_begin(description(desc, RENDER_DESC)?));
    }
}
/// # Safety
/// Exclusive recording pool, active rendering scope.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_render_end(encoder: *mut List) {
    unsafe {
        encode(encoder, |e| e.render_end());
    }
}
/// # Safety
/// Exclusive recording pool and readable versioned viewport/scissor state.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_graphics_state(encoder: *mut List, state: *const Record) {
    unsafe {
        encode(encoder, |e| {
            e.viewport(description(state.cast(), VIEWPORT_STATE)?)
        });
    }
}
/// # Safety
/// See header: compatible executable/attachments and all shader accesses valid.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_draw(encoder: *mut List, draw: *const DrawDesc) {
    unsafe {
        encode(encoder, |e| e.draw(draw.as_ref().ok_or(INVALID)?, false));
    }
}
/// # Safety
/// Live index span through recorded/pending use; exclusive pool access.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_bind_indices(encoder: *mut List, span: Span, kind: u32) {
    unsafe {
        encode(encoder, |e| e.bind_indices(span, kind));
    }
}
/// # Safety
/// Writable disjoint output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_vertex_scratch_requirements(
    count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        unsafe {
            out.write(rust::vertex_scratch_requirements(count)?);
        }
        Ok(())
    })
}
/// # Safety
/// Exclusive pool, live vertex spans, disjoint readable inputs and writable scratch.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_bind_vertices(
    encoder: *mut List,
    first: u32,
    count: u32,
    spans: *const Span,
    scratch: HostSpan,
) {
    unsafe {
        encode(encoder, |e| e.bind_vertices(first, count, spans, scratch));
    }
}
/// # Safety
/// Same requirements as draw plus valid bound indices and resulting shader accesses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_draw_indexed(encoder: *mut List, draw: *const DrawDesc) {
    unsafe {
        encode(encoder, |e| e.draw(draw.as_ref().ok_or(INVALID)?, true));
    }
}
/// # Safety
/// Readable description, valid GPU argument contents and synchronized spans at execution.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_draw_indirect(encoder: *mut List, desc: *const Indirect) {
    unsafe {
        encode(encoder, |e| {
            e.draw_indirect(desc.as_ref().ok_or(INVALID)?, false)
        });
    }
}
/// # Safety
/// Same as indirect draw plus valid bound indices and resulting shader accesses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_draw_indexed_indirect(
    encoder: *mut List,
    desc: *const Indirect,
) {
    unsafe {
        encode(encoder, |e| {
            e.draw_indirect(desc.as_ref().ok_or(INVALID)?, true)
        });
    }
}
/// # Safety
/// Recording pool exclusion and live executable through recorded/pending use.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_bind_executable(
    encoder: *mut List,
    executable: *mut Executable,
) {
    unsafe {
        encode(encoder, |e| {
            e.bind_executable(executable.as_ref().ok_or(INVALID)?)
        });
    }
}
/// # Safety
/// Recording pool exclusion; readable bytes borrowed only during this call.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_inline(
    encoder: *mut List,
    stages: u64,
    offset: u32,
    size: u32,
    data: *const std::ffi::c_void,
) {
    unsafe {
        encode(encoder, |e| e.inline(stages, offset, size, data));
    }
}
/// # Safety
/// Recording pool exclusion; address and reachable data valid at every execution.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_root(
    encoder: *mut List,
    stages: u64,
    slot: u32,
    address: u64,
) {
    unsafe {
        encode(encoder, |e| e.root(stages, slot, address));
    }
}
/// # Safety
/// Recording pool exclusion; shader inputs, bindings and hazards satisfy header.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_dispatch(encoder: *mut List, launch: *const Launch) {
    unsafe {
        encode(encoder, |e| e.dispatch(launch.as_ref().ok_or(INVALID)?));
    }
}
/// # Safety
/// Same as dispatch; GPU argument dimensions obey device limits at execution.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_dispatch_indirect(
    encoder: *mut List,
    args: Span,
    dynamic_shared_bytes: u32,
) {
    unsafe {
        encode(encoder, |e| e.dispatch_indirect(args, dynamic_shared_bytes));
    }
}
/// # Safety
/// Exclusive recording pool, live backing, and ranges as documented in header.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_copy_memory(encoder: *mut List, dst: Span, src: Span) {
    unsafe {
        native_encode(encoder, |handle| rust::copy_memory(handle, dst, src));
    }
}
/// # Safety
/// Exclusive recording pool and live destination backing.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_fill_memory(encoder: *mut List, dst: Span, pattern: u32) {
    unsafe {
        native_encode(encoder, |handle| rust::fill_memory(handle, dst, pattern));
    }
}
/// # Safety
/// Exclusive pool access; live ranges and scratch disjoint from every input/object.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_barrier(encoder: *mut List, dep: *const Dependency) {
    unsafe {
        // Decode the C record inside shared recording error handling.
        encode(encoder, |e| e.barrier(description(dep, DEPENDENCY)?));
    }
}
/// # Safety
/// Exclusive encoder/pool; descriptor storage and reserved bytes obey header lifetimes.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_bind_heap(encoder: *mut List, binding: *const HeapBinding) {
    unsafe {
        encode(encoder, |e| {
            e.bind_heap(description(binding, HEAP_BINDING)?)
        });
    }
}
/// # Safety
/// Exclusive encoder/pool, live bound image/range/value; caller proves image state.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_clear_image(
    encoder: *mut List,
    image: *mut Image,
    state: u32,
    range: *const Subresources,
    value: *const ClearValue,
) {
    unsafe {
        encode(encoder, |e| {
            e.clear_image(
                image.as_ref().ok_or(INVALID)?,
                state,
                *range.as_ref().ok_or(INVALID)?,
                *value.as_ref().ok_or(INVALID)?,
            )
        });
    }
}
/// # Safety
/// Exclusive encoder/pool, live source/destination, valid region, explicit hazards/state.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_copy_to_image(
    encoder: *mut List,
    image: *mut Image,
    src: Span,
    copy: *const ImageCopy,
) {
    unsafe {
        encode(encoder, |e| {
            e.copy_image_memory(
                image.as_ref().ok_or(INVALID)?,
                src,
                copy.as_ref().ok_or(INVALID)?,
                true,
            )
        });
    }
}
/// # Safety
/// Exclusive encoder/pool, live source/destination, valid region, explicit hazards/state.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_copy_from_image(
    encoder: *mut List,
    dst: Span,
    image: *mut Image,
    copy: *const ImageCopy,
) {
    unsafe {
        encode(encoder, |e| {
            e.copy_image_memory(
                image.as_ref().ok_or(INVALID)?,
                dst,
                copy.as_ref().ok_or(INVALID)?,
                false,
            )
        });
    }
}
/// # Safety
/// Live images and readable transfer; caller proves layouts, non-overlap and synchronization.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_copy_image(
    encoder: *mut List,
    dst: *mut Image,
    src: *mut Image,
    transfer: *const ImageTransfer,
) {
    unsafe {
        encode(encoder, |e| {
            e.transfer_image(
                dst.as_ref().ok_or(INVALID)?,
                src.as_ref().ok_or(INVALID)?,
                transfer.as_ref().ok_or(INVALID)?,
                0,
            )
        });
    }
}
/// # Safety
/// Same lifetime/non-overlap contract as copy; valid native multisample resolve profile.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_resolve_image(
    encoder: *mut List,
    dst: *mut Image,
    src: *mut Image,
    transfer: *const ImageTransfer,
    mode: u32,
) {
    unsafe {
        encode(encoder, |e| {
            if mode != 1 {
                return Err(UNSUPPORTED);
            }
            e.transfer_image(
                dst.as_ref().ok_or(INVALID)?,
                src.as_ref().ok_or(INVALID)?,
                transfer.as_ref().ok_or(INVALID)?,
                mode,
            )
        });
    }
}
/// # Safety
/// Live same-device images, valid description and caller-proven non-overlapping
/// filtered source/destination memory, layouts, synchronization and lifetimes.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_blit_image(
    encoder: *mut List,
    dst: *mut Image,
    src: *mut Image,
    blit: *const ImageBlit,
) {
    unsafe {
        encode(encoder, |e| {
            e.blit_image(
                dst.as_ref().ok_or(INVALID)?,
                src.as_ref().ok_or(INVALID)?,
                blit.as_ref().ok_or(INVALID)?,
            )
        });
    }
}
/// # Safety
/// Live exclusive encoder; caller initializes dynamic state and obeys native lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_depth_test(encoder: *mut List, value: u32) {
    unsafe { encode(encoder, |e| e.dynamic_raster_value(32, value)) };
}
/// # Safety
/// Live exclusive encoder; caller initializes dynamic state and obeys native lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_depth_write(encoder: *mut List, value: u32) {
    unsafe { encode(encoder, |e| e.dynamic_raster_value(64, value)) };
}
/// # Safety
/// Live exclusive encoder; caller initializes dynamic state and obeys native lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_depth_compare(encoder: *mut List, value: u32) {
    unsafe { encode(encoder, |e| e.dynamic_raster_value(128, value)) };
}
/// # Safety
/// Live exclusive encoder; caller initializes dynamic state and obeys native lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_stencil_test(encoder: *mut List, value: u32) {
    unsafe { encode(encoder, |e| e.dynamic_raster_value(256, value)) };
}
/// # Safety
/// Live exclusive encoder; caller initializes dynamic state and obeys native lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_cull(encoder: *mut List, value: u32) {
    unsafe { encode(encoder, |e| e.dynamic_raster_value(1024, value)) };
}
/// # Safety
/// Live exclusive encoder; caller initializes dynamic state and obeys native lifetime rules.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_front_face(encoder: *mut List, value: u32) {
    unsafe { encode(encoder, |e| e.dynamic_raster_value(2048, value)) };
}
/// # Safety
/// Live exclusive encoder; caller initializes every required stencil face.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_set_stencil_ops(
    encoder: *mut List,
    faces: u32,
    fail: u32,
    pass: u32,
    depth_fail: u32,
    compare: u32,
) {
    unsafe {
        encode(encoder, |e| {
            e.stencil_ops(faces, fail, pass, depth_fail, compare)
        })
    };
}
