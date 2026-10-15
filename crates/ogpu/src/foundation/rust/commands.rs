//! Direct native recording, sharing the C path's validated backend operations.
use super::*;

macro_rules! record {
    ($(#[$doc:meta])* fn $name:ident($list:ident $(, $arg:ident: $ty:ty)*) |$e:ident| $body:expr) => {
        $(#[$doc])*
        /// # Safety
        /// Live recording slot and parent device; exclusive arena/pool host access.
        /// Referenced resources, addresses and executable inputs must remain valid
        /// for all recorded/pending uses, with explicit dependencies and native
        /// state/replay rules satisfied. Nested pointers/scratch must be valid and
        /// disjoint as specified by the matching operation in `ogpu_next.h`.
        pub unsafe fn $name($list: ListHandle, $($arg: $ty),*) {
            unsafe { encode($list, |$e| $body); }
        }
    };
}
record!(/// Bind prepared execution state without compilation.
    fn bind_executable(list, executable: ExecutableHandle) |e| e.bind_executable(executable.get()));
record!(/// Copy inline bytes into command state; bytes are borrowed only for this call.
    fn set_inline(list, stages: u64, offset: u32, bytes: &[u8]) |e| e.inline(stages, offset, count(bytes.len())?, bytes.as_ptr().cast()));
record!(/// Set a non-owning device address root; all reachable shader data is caller-owned.
    fn set_root(list, stages: u64, slot: u32, address: u64) |e| e.root(stages, slot, address));
record!(fn dispatch(list, launch: &Launch) |e| e.dispatch(launch));
record!(/// GPU dimensions must satisfy enabled limits when the command executes.
    fn dispatch_indirect(list, args: Span, dynamic_shared_bytes: u32) |e| e.dispatch_indirect(args, dynamic_shared_bytes));
record!(/// Reserved descriptor bytes obey heap lifetime/mutation rules; no retention walk.
    fn bind_heap(list, binding: &HeapBinding) |e| e.bind_heap(binding));
record!(fn render_begin(list, desc: &RenderDesc) |e| e.render_begin(desc));
record!(/// End an active rendering scope.
    fn render_end(list) |e| e.render_end());
record!(fn set_graphics_state(list, state: &ViewportState) |e| e.viewport(state));
record!(fn draw(list, draw: &DrawDesc) |e| e.draw(draw, false));
record!(fn draw_indexed(list, draw: &DrawDesc) |e| e.draw(draw, true));
record!(fn draw_indirect(list, desc: &Indirect) |e| e.draw_indirect(desc, false));
record!(fn draw_indexed_indirect(list, desc: &Indirect) |e| e.draw_indirect(desc, true));
record!(fn bind_indices(list, span: Span, kind: u32) |e| e.bind_indices(span, kind));
record!(fn bind_vertices(list, first: u32, spans: &[Span], scratch: HostSpan) |e| e.bind_vertices(first, count(spans.len())?, spans.as_ptr(), scratch));
record!(fn set_blend_constants(list, values: &[f32; 4]) |e| e.blend_constants(values));
record!(fn set_stencil_reference(list, faces: u32, value: u32) |e| e.stencil_value(4, faces, value));
record!(fn set_stencil_compare_mask(list, faces: u32, value: u32) |e| e.stencil_value(8, faces, value));
record!(fn set_stencil_write_mask(list, faces: u32, value: u32) |e| e.stencil_value(16, faces, value));
record!(fn set_depth_test(list, value: u32) |e| e.dynamic_raster_value(32, value));
record!(fn set_depth_write(list, value: u32) |e| e.dynamic_raster_value(64, value));
record!(fn set_depth_compare(list, value: u32) |e| e.dynamic_raster_value(128, value));
record!(fn set_stencil_test(list, value: u32) |e| e.dynamic_raster_value(256, value));
record!(fn set_cull(list, value: u32) |e| e.dynamic_raster_value(1024, value));
record!(fn set_front_face(list, value: u32) |e| e.dynamic_raster_value(2048, value));
record!(fn set_stencil_ops(list, faces: u32, fail: u32, pass: u32, depth_fail: u32, compare: u32) |e| e.stencil_ops(faces, fail, pass, depth_fail, compare));
record!(fn clear_image(list, image: ImageHandle, state: u32, range: Subresources, value: ClearValue) |e| e.clear_image(image.get(), state, range, value));
record!(fn copy_to_image(list, image: ImageHandle, src: Span, copy: &ImageCopy) |e| e.copy_image_memory(image.get(), src, copy, true));
record!(fn copy_from_image(list, dst: Span, image: ImageHandle, copy: &ImageCopy) |e| e.copy_image_memory(image.get(), dst, copy, false));
record!(fn copy_image(list, dst: ImageHandle, src: ImageHandle, transfer: &ImageTransfer) |e| e.transfer_image(dst.get(), src.get(), transfer, 0));
record!(fn resolve_image(list, dst: ImageHandle, src: ImageHandle, transfer: &ImageTransfer, mode: u32) |e| {
    if mode != 1 { return Err(UNSUPPORTED); }
    e.transfer_image(dst.get(), src.get(), transfer, mode)
});
record!(fn blit_image(list, dst: ImageHandle, src: ImageHandle, blit: &ImageBlit) |e| e.blit_image(dst.get(), src.get(), blit));
record!(/// Referenced secondary lists/arenas outlive the primary and all executions.
    fn execute_lists(list, lists: &[ListHandle], scratch: HostSpan) |e| e.execute(count(lists.len())?, lists.as_ptr().cast(), scratch));
record!(/// Caller orders query reset and use; no pending query may be reset unsafely.
    fn queries_reset(list, pool: QueryPoolHandle, first: u32, count: u32) |e| e.queries_reset(pool.get(), first, count));
record!(fn query_begin(list, pool: QueryPoolHandle, index: u32, flags: u32) |e| e.query(pool.get(), index, true, flags));
record!(fn query_end(list, pool: QueryPoolHandle, index: u32) |e| e.query(pool.get(), index, false, 0));
record!(fn timestamp(list, pool: QueryPoolHandle, index: u32, scope: u64) |e| e.timestamp(pool.get(), index, scope));
record!(#[allow(clippy::too_many_arguments)]
    fn queries_resolve(list, pool: QueryPoolHandle, first: u32, count: u32, dst: Span, stride: u64, flags: u32) |e| e.queries_resolve(pool.get(), first, count, dst, stride, flags));
record!(fn split_reset(list, split: SplitHandle, before: u64) |e| e.split_reset(split.get(), before));
record!(/// Caller guarantees matching acquire and correct event-cycle/queue ordering.
    fn split_release(list, split: SplitHandle, dependency: &Dependency) |e| e.split_release(split.get(), dependency));
record!(fn split_acquire(list, waits: &[SplitDependency], scratch: HostSpan) |e| e.split_acquire(count(waits.len())?, waits.as_ptr(), scratch));

macro_rules! scratch {
    ($name:ident, $native:ident) => {
        pub fn $name(count: u32) -> Result<HostRequirements> {
            native::$native(count)
        }
    };
}
scratch!(split_scratch_requirements, split_scratch);
scratch!(submit_batch_scratch_requirements, submit_batch_scratch);
scratch!(execute_scratch_requirements, execute_scratch);
scratch!(inheritance_scratch_requirements, inheritance_scratch);
scratch!(render_scratch_requirements, render_scratch);
scratch!(vertex_scratch_requirements, vertex_scratch);

/// Atomic preflight then one native submission call; each record retains its own
/// lists, dependencies and translation scratch, without allocating a flattening list.
/// # Safety
/// Live exclusively accessed queue; descriptors/nested arrays and disjoint scratch
/// valid. Caller retains all resources until proven completion and obeys replay,
/// synchronization and uncertain-submission rules, exactly as for queue_submit.
pub unsafe fn queue_submit_batch(
    queue: QueueHandle,
    descriptions: &[SubmitDesc],
    scratch: HostSpan,
) -> Result<()> {
    unsafe {
        queue_submit_batch_raw(
            queue,
            count(descriptions.len())?,
            descriptions.as_ptr(),
            scratch,
        )
    }
}
pub(in crate::foundation) unsafe fn queue_submit_batch_raw(
    queue: QueueHandle,
    count: u32,
    descriptions: *const SubmitDesc,
    scratch: HostSpan,
) -> Result<()> {
    unsafe { queue.get().submit_batch(count, descriptions, scratch) }
}
