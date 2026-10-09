//! C command boundary. Encoder and executable-list handles borrow arena slots.
use super::*;

unsafe fn description<'a, T>(pointer: *const T, kind: u32) -> Result<&'a T, Status> {
    if pointer.is_null() {
        return Err(INVALID);
    }
    unsafe { pointer.cast::<Record>().read() }.validate::<T>(kind)?;
    Ok(unsafe { &*pointer })
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
        let d = unsafe { device.as_ref() }.ok_or(INVALID)?;
        let arena = Arena::create(d, unsafe { description(desc, ARENA_DESC)? })?;
        unsafe {
            out.write(Box::into_raw(arena));
        }
        Ok(())
    })
}
/// # Safety
/// Exclusive arena/pool host access; existing slot handles remain borrowed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_reserve(arena: *mut Arena, count: u32) -> Status {
    boundary(|| unsafe { arena.as_mut() }.ok_or(INVALID)?.reserve(count))
}
/// # Safety
/// No pending use; excludes every list/encoder/pool access. Invalidates all handles.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_reset(arena: *mut Arena) -> Status {
    boundary(|| unsafe { arena.as_mut() }.ok_or(INVALID)?.reset(false))
}
/// # Safety
/// Same exclusion/lifetime requirements as reset.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_trim(arena: *mut Arena, count: u32) -> Status {
    boundary(|| unsafe { arena.as_mut() }.ok_or(INVALID)?.trim(count))
}
/// # Safety
/// No pending or host uses; live parent device. NULL permitted. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_arena_destroy(arena: *mut Arena) {
    if !arena.is_null() {
        unsafe {
            drop(Box::from_raw(arena));
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
        let arena = unsafe { arena.as_mut() }.ok_or(INVALID)?;
        let encoder = arena.begin(unsafe { description(desc, RECORDING_DESC)? })?;
        unsafe {
            out.write(encoder);
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
        unsafe { encoder.as_ref() }.ok_or(INVALID)?.end()?;
        unsafe {
            out.write(encoder);
        }
        Ok(())
    })
}
/// # Safety
/// Live recording slot, exclusive pool access. NULL permitted.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_commands_cancel(encoder: *mut List) {
    if let Some(encoder) = unsafe { encoder.as_ref() } {
        encoder.cancel();
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
        let requirements = native::submit_scratch(lists, waits, signals)?;
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
        let requirements = native::barrier_scratch(memory_count, image_count)?;
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
        let queue = unsafe { queue.as_ref() }.ok_or(INVALID)?;
        unsafe { queue.submit(description(desc, SUBMIT_DESC)?) }
    })
}
unsafe fn encode(encoder: *mut List, f: impl FnOnce(&List) -> Result<(), Status>) {
    if let Some(encoder) = unsafe { encoder.as_ref() } {
        encoder.poison(boundary(|| f(encoder)));
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
        let pool = QueryPool::create(unsafe { device.as_ref() }.ok_or(INVALID)?, unsafe {
            description(desc, QUERY_POOL_DESC)?
        })?;
        unsafe {
            out.write(Box::into_raw(pool));
        }
        Ok(())
    })
}
/// # Safety
/// No recorded future/pending/host use and live device. NULL allowed. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_query_pool_destroy(pool: *mut QueryPool) {
    if !pool.is_null() {
        unsafe {
            drop(Box::from_raw(pool));
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
/// Live occlusion pool, unavailable query at execution and exclusive recording.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_query_begin(
    encoder: *mut List,
    pool: *mut QueryPool,
    index: u32,
) {
    unsafe {
        encode(encoder, |e| {
            e.query(pool.as_ref().ok_or(INVALID)?, index, true)
        });
    }
}
/// # Safety
/// Matching active query in this encoder and rendering scope.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_query_end(encoder: *mut List, pool: *mut QueryPool, index: u32) {
    unsafe {
        encode(encoder, |e| {
            e.query(pool.as_ref().ok_or(INVALID)?, index, false)
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
            out.write(native::render_scratch(colors)?);
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
        encode(encoder, |e| e.copy(dst, src));
    }
}
/// # Safety
/// Exclusive recording pool and live destination backing.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_fill_memory(encoder: *mut List, dst: Span, pattern: u32) {
    unsafe {
        encode(encoder, |e| e.fill(dst, pattern));
    }
}
/// # Safety
/// Exclusive pool access; live ranges and scratch disjoint from every input/object.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_barrier(encoder: *mut List, dep: *const Dependency) {
    unsafe {
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
