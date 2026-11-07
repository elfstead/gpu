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
record!(/// Update a range in the bound executable's prepared argument layout.
    fn set_arguments(list, layout: ArgumentLayoutHandle, offset: u32, bytes: &[u8]) |e|
        e.set_arguments(layout.get(), offset, count(bytes.len())?, bytes.as_ptr().cast()));
record!(fn dispatch(list, launch: &Launch) |e| e.dispatch(launch));
record!(/// GPU dimensions must satisfy enabled limits when the command executes.
    fn dispatch_indirect(list, args: Span, dynamic_shared_bytes: u32) |e| e.dispatch_indirect(args, dynamic_shared_bytes));
record!(/// Reserved descriptor bytes obey heap lifetime/mutation rules; no retention walk.
    fn bind_heap(list, heap: DescriptorHeapHandle) |e| e.bind_heap(heap.get()));
record!(fn render_begin(list, desc: &RenderDesc) |e| e.render_begin(desc));
record!(/// End an active rendering scope.
    fn render_end(list) |e| e.render_end());
record!(fn set_graphics_state(list, state: &ViewportState) |e| e.viewport(state));
record!(fn draw(list, draw: &DrawDesc) |e| e.draw(draw, false));
record!(fn draw_indexed(list, draw: &DrawDesc) |e| e.draw(draw, true));
record!(fn draw_indirect(list, desc: &Indirect) |e| e.draw_indirect(desc, false));
record!(fn draw_indexed_indirect(list, desc: &Indirect) |e| e.draw_indirect(desc, true));
record!(fn draw_mesh(list, groups: Extent) |e| e.draw_mesh(groups));
record!(fn draw_mesh_indirect(list, desc: &Indirect) |e| e.draw_mesh_indirect(desc));
record!(fn generated_preprocess(list, batch: &GeneratedBatch, state: ListHandle) |e| e.generated_preprocess(batch, state.get()));
record!(fn generated_execute(list, batch: &GeneratedBatch, preprocessed: u32) |e| e.generated_execute(batch, preprocessed));
record!(fn bind_indices(list, span: Span, kind: u32) |e| e.bind_indices(span, kind));
record!(fn bind_vertices(list, first: u32, spans: &[VertexBufferBinding], scratch: HostSpan) |e| e.bind_vertices(first, count(spans.len())?, spans.as_ptr(), scratch));
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
record!(fn query_begin(list, pool: QueryPoolHandle, index: u32, stream:u32, flags: u32) |e| e.query(pool.get(), index, true, flags,stream));
record!(fn query_end(list, pool: QueryPoolHandle, index: u32, stream:u32) |e| e.query(pool.get(), index, false, 0,stream));
record!(fn timestamp(list, pool: QueryPoolHandle, index: u32, scope: u64) |e| e.timestamp(pool.get(), index, scope));
record!(#[allow(clippy::too_many_arguments)]
    fn queries_resolve(list, pool: QueryPoolHandle, first: u32, count: u32, dst: Span, stride: u64, flags: u32) |e| e.queries_resolve(pool.get(), first, count, dst, stride, flags));
record!(fn split_reset(list, split: SplitHandle, before: u64) |e| e.split_reset(split.get(), before));
record!(/// Caller guarantees matching acquire and correct event-cycle/queue ordering.
    fn split_release(list, split: SplitHandle, dependency: &Dependency, scratch: HostSpan) |e| e.split_release(split.get(), dependency, scratch));
record!(fn split_acquire(list, waits: &[SplitDependency], scratch: HostSpan) |e| e.split_acquire(waits, scratch));

macro_rules! scratch {
    ($name:ident, $native:ident) => {
        pub fn $name(count: u32) -> Result<HostRequirements> {
            native::$native(count)
        }
    };
}
/// # Safety
/// Live device; readable dependency records. Nested barrier arrays and split
/// handles are not read by this counts/flags-only query.
pub unsafe fn split_scratch_requirements(
    device: DeviceHandle,
    waits: &[SplitDependency],
) -> Result<HostRequirements> {
    count(waits.len())?;
    unsafe { native::split_scratch(device.get(), waits) }
}
scratch!(execute_scratch_requirements, execute_scratch);
scratch!(inheritance_scratch_requirements, inheritance_scratch);
scratch!(render_scratch_requirements, render_scratch);
scratch!(vertex_scratch_requirements, vertex_scratch);

record!(fn set_shading_rate(list, state: &ShadingRateState) |e| e.shading_rate(state));

record!(fn set_render_locations(list, state: &RenderLocations) |e| e.render_locations(state));
record!(fn set_render_inputs(list, state: &RenderInputs) |e| e.render_inputs(state));

record!(fn set_conservative_mode(list, value:u32) |e| e.raster_mechanism_value(DYNAMIC_CONSERVATIVE_MODE_STATE,value));

record!(fn set_extra_overestimation(list, value:f32) |e| e.extra_overestimation(value));

record!(fn set_provoking_vertex(list, value:u32) |e| e.raster_mechanism_value(DYNAMIC_PROVOKING_VERTEX_STATE,value));

record!(fn set_depth_clip(list, value:u32) |e| e.raster_mechanism_value(DYNAMIC_DEPTH_CLIP_STATE,value));

record!(fn set_depth_convention(list, value:u32) |e| e.raster_mechanism_value(DYNAMIC_DEPTH_CONVENTION_STATE,value));

record!(fn set_line_mode(list, value:u32) |e| e.raster_mechanism_value(DYNAMIC_LINE_MODE_STATE,value));

record!(fn set_line_stipple_enable(list, value:u32) |e| e.raster_mechanism_value(DYNAMIC_LINE_STIPPLE_ENABLE_STATE,value));

record!(fn set_line_stipple(list, factor:u32, pattern:u32) |e| e.line_stipple(factor,pattern));

record!(fn set_sample_locations(list, locations: &SampleLocations) |e| e.sample_locations(locations));
record!(fn set_sample_locations_enable(list, enable: u32) |e| e.sample_locations_enable(enable));

/// Counts-only scratch query; no device or input arrays required.
pub fn vertex_input_scratch_requirements(
    bindings: u32,
    attributes: u32,
) -> Result<HostRequirements> {
    native::vertex_input_scratch(bindings, attributes)
}
record!(fn set_vertex_input(list, input: &VertexInput, scratch: HostSpan) |e| e.vertex_input(input, scratch));

record!(fn set_topology(list, topology: u32) |e| e.dynamic_raster_value(DYNAMIC_TOPOLOGY_STATE, topology));

record!(fn set_primitive_restart(list, enable: u32) |e| e.dynamic_raster_value(DYNAMIC_PRIMITIVE_RESTART_STATE, enable));

record!(fn set_rasterizer_discard(list, enable: u32) |e| e.dynamic_raster_value(DYNAMIC_RASTERIZER_DISCARD_STATE, enable));

record!(fn set_line_width(list, width: f32) |e| e.line_width(width));

record!(fn set_depth_bias(list, constant: f32, clamp: f32, slope: f32) |e| e.depth_bias(constant, clamp, slope));

record!(fn set_depth_bias_enable(list, enable: u32) |e| e.dynamic_raster_value(DYNAMIC_DEPTH_BIAS_ENABLE_STATE, enable));

record!(fn set_depth_bounds(list, min: f32, max: f32) |e| e.depth_bounds(min, max));

record!(fn set_depth_bounds_enable(list, enable: u32) |e| e.dynamic_raster_value(DYNAMIC_DEPTH_BOUNDS_ENABLE_STATE, enable));

record!(fn set_depth_clamp(list, value: u32) |e| e.extended_raster_value(DYNAMIC_DEPTH_CLAMP_STATE, value));

record!(fn set_polygon_mode(list, value: u32) |e| e.extended_raster_value(DYNAMIC_POLYGON_MODE_STATE, value));

record!(fn set_rasterization_samples(list, value: u32) |e| e.extended_raster_value(DYNAMIC_RASTERIZATION_SAMPLES_STATE, value));

record!(fn set_alpha_to_coverage(list, value: u32) |e| e.extended_raster_value(DYNAMIC_ALPHA_TO_COVERAGE_STATE, value));

record!(fn set_alpha_to_one(list, value: u32) |e| e.extended_raster_value(DYNAMIC_ALPHA_TO_ONE_STATE, value));

record!(fn set_logic_op_enable(list, value: u32) |e| e.extended_raster_value(DYNAMIC_LOGIC_OP_ENABLE_STATE, value));

record!(fn set_logic_op(list, value: u32) |e| e.extended_raster_value(DYNAMIC_LOGIC_OP_STATE, value));

record!(fn set_sample_mask(list, samples: u32, mask: u64) |e| e.sample_mask(samples, mask));

record!(fn set_color_blend_enable(list, first: u32, values: &[u32]) |e| e.color_values(DYNAMIC_COLOR_BLEND_ENABLE_STATE, first, count(values.len())?, values.as_ptr()));

record!(fn set_color_write_mask(list, first: u32, values: &[u32]) |e| e.color_values(DYNAMIC_COLOR_WRITE_MASK_STATE, first, count(values.len())?, values.as_ptr()));

record!(fn set_color_blend_equation(list, first: u32, values: &[BlendEquation]) |e| e.color_equations(first, count(values.len())?, values.as_ptr()));

record!(fn set_viewports(list, first:u32, values:&[Viewport], flags:u32) |e| e.viewports(first,count(values.len())?,values.as_ptr(),flags));

record!(fn set_scissors(list, first:u32, values:&[Scissor], flags:u32) |e| e.scissors(first,count(values.len())?,values.as_ptr(),flags));

record!(fn set_depth_clamp_range(list,mode:u32,min:f32,max:f32) |e| e.depth_clamp_range(mode,min,max));

record!(fn set_depth_bias_control(list,constant:f32,clamp:f32,slope:f32,representation:u32,exact:u32) |e| e.depth_bias_control(constant,clamp,slope,representation,exact));

record!(fn set_color_write_enable(list,enables:&[u32]) |e| e.color_write_enable(count(enables.len())?,enables.as_ptr()));

record!(fn set_patch_control_points(list,value:u32) |e| e.extended_raster_value(DYNAMIC_PATCH_CONTROL_POINTS_STATE,value));

record!(fn set_tessellation_origin(list,value:u32) |e| e.extended_raster_value(DYNAMIC_TESSELLATION_ORIGIN_STATE,value));

scratch!(capture_scratch_requirements, capture_scratch);
record!(fn bind_capture_buffers(list,first:u32,buffers:&[Span],scratch:HostSpan) |e| e.bind_capture(first,count(buffers.len())?,buffers.as_ptr(),scratch));
record!(fn capture_begin(list,first:u32,counters:&[Span],scratch:HostSpan) |e| e.capture(true,first,count(counters.len())?,counters.as_ptr(),scratch));
record!(fn capture_end(list,first:u32,counters:&[Span],scratch:HostSpan) |e| e.capture(false,first,count(counters.len())?,counters.as_ptr(),scratch));
record!(fn draw_byte_count(list,counter:Span,offset:u32,stride:u32,instances:u32,first_instance:u32) |e| e.draw_byte_count(counter,offset,stride,instances,first_instance));
record!(fn set_rasterization_stream(list,stream:u32) |e| e.extended_raster_value(DYNAMIC_RASTERIZATION_STREAM_STATE,stream));
