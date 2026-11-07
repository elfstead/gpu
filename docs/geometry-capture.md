# Ordered geometry capture

M8, 2026-11-07. Foundation revision 13 adds native transform feedback and geometry
streams, using the existing memory, shader, query, recording and dependency model.
It also adds independent dynamic rasterization-stream selection. No stream manager,
implicit allocation, CPU geometry expansion or readback is introduced.

## Why a distinct mechanism

Storage writes can implement application-selected vertex processing. They do not
by themselves preserve the native capture order of emitted primitives, bounded
capture/overflow counts, or native pause/resume byte counters. Building those from
shader writes can require additional ordering, compaction and dispatch work.
Likewise, deriving a draw from a byte counter in a shader adds a scheduling step
when the native draw command already performs that calculation. These are concrete
capability/execution differences; the core retains the native boundary. Applications
choose storage-based algorithms when that is a better fit for their workload.

The [Vulkan extension](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_transform_feedback.html)
is principally intended for API translation. That makes it optional, not a device
baseline or an automatic choice for new geometry processing. Vulkan's intended use
alone is not grounds for excluding translation workloads from OGPU.

## Interface and lowering

Feature IDs 121/122 independently select `TRANSFORM_FEEDBACK` and
`GEOMETRY_STREAMS`. The former requires explicitly selecting `RASTER`, the latter
`GEOMETRY`. ID 123 selects dynamic rasterization stream (dynamic bit 50).
`CaptureLimits` query 29 reports stream/buffer counts, byte/data/stride limits and
independent native query, line/triangle-stream, stream-rasterization and byte-count
draw properties. Physical support does not imply device enabling.

`CAPTURE` and `CAPTURE_COUNTER` memory usages name native resource purposes.
`bind_capture_buffers(first, spans, scratch)` sets output ranges. `capture_begin`
and `capture_end` read/write optional uint32 byte counters; an empty array starts
at zero or omits counter storage. Zero/null entries express individual reset/omit
behavior, and zero/null data spans unbind output slots. These are C count/pointer
arrays and native Rust slices. `capture_scratch_requirements(count)` permits one
caller allocation reused after each host call. Native address commands use that
scratch directly; no temporary vector is allocated during recording.

Capture occurs inside rendering with a compatible last pre-raster shader declaring
Xfb, and without multiview. The caller provides output decorations, matching buffers,
strides, counters, capacities and lifetimes. A small scope bit rejects nested
begin, unmatched end, rebinding outputs/pipeline while active, and unclosed capture.
It does not track output slots or infer memory dependencies.

`draw_byte_count(counter, offset, stride, instances, first_instance)` reads an
ordinary `INDIRECT` span. It does not require that OGPU produced the count or that
the memory has capture-counter usage. Offset/stride are multiples of four, stride
is nonzero and bounded, first vertex is zero, and the native GPU calculation derives
vertex count from the adjusted byte count. No CPU readback or count-conversion
kernel is inserted. The native draw property must be available.

`RasterizationStream` (record 146) chains to `GraphicsState`. Default stream zero
needs no new selection. Nonzero selection requires `GEOMETRY_STREAMS` and the
reported rasterization selection property; dynamic selection ignores the static
stream value. The command sets only the stream and does not enable shader stages.

Dependencies add `STAGE_CAPTURE`, `ACCESS_CAPTURE_WRITE`, and counter read/write
accesses. Counter reads used by a byte-count draw belong to the indirect stage;
resume counter reads belong to capture. The application supplies ordering between
capture passes and subsequent reads. Graphics-domain availability queries include
capture only when enabled. Ordinary and split dependencies share these mappings.

## Query migration

`QUERY_CAPTURE_STREAM` uses the existing query pool and asynchronous resolve
operations. Each result contains primitives written, primitives needed, then
optional availability. The two existing query begin/end operations now take an
explicit stream argument. Existing occlusion/statistics callers pass zero. This is
a revision-13 calling-contract change: rebuild matching header/runtime/Rust sources.
There is no second family of stream-specific public query entry points.

Simultaneous stream queries use distinct slots allocated during explicit arena
reservation, sized from the selected device's native stream limit. Recording does
not allocate. Existing lifetime, reset, result and scope checks apply to those slots.
General query inheritance and secondary composition retain their native caller
obligations; this fixture does not validate all such combinations.

Native mappings use the already-required device-address command extension's
[bind](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBindTransformFeedbackBuffers2EXT.html),
[begin](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBeginTransformFeedback2EXT.html)
and [end](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdEndTransformFeedback2EXT.html)
operations, plus indexed native query commands. Public spans currently borrow the
existing memory object; M10 owns the broader foreign/raw-memory endpoint audit.

## Artifact and native evidence

The bounded fixture has three GLSL artifacts: a captured vertex stage, an ordinary
vertex stage feeding a two-stream geometry stage, and that captured geometry stage.
`cargo xtask capture-shaders --check` reproduces them with glslang 16.6.0 and validates
SPIR-V for Vulkan 1.4. Set `GLSLANG_VALIDATOR` to the compiler executable when it is
not on PATH. The ordinary Slang fixture lane remains unchanged.

Pinned Slang 2026.14.1 rejects the GLSL `xfb_buffer`, `xfb_stride` and `xfb_offset`
qualifiers in the probe. No general Slang capture-artifact workflow is claimed.
The native fixture establishes a usable raw-SPIR-V path and identifies the tooling
work separately; the runtime never parses or decorates shader output to simulate it.

RX 5700 XT / RADV Mesa 26.2.5 passes six profiles, 12 changed-seed replay submissions
and 96 ordered captured vertices, with guards. Profiles cover one/two streams,
exact byte counters, insufficient output capacity, written/needed query counts,
and pause/resume across two render scopes. Pipeline statistics independently check
byte-count draw input counts with offset subtraction and two instances. Seven
malformed recordings reject. This GPU supports four streams/buffers and a 512-byte
maximum stride; nonzero rasterization-stream selection is unavailable.

The first GPU run produced correct data but failed validation: indirect byte-count
reads needed INDIRECT usage, and only the last pre-raster stage may declare Xfb.
Both were corrected. Native address usage hints now include capture usage for
counter ranges sharing the capture buffer, and offset/stride alignment is checked.
The failing logs remain in the receipt. The corrected full C run has zero validation
errors and four existing unused-output warnings. This is functional/lowering
evidence, not a performance comparison.

121 host tests, two C/native Rust correspondence tests, strict Clippy, 2,363 ABI
layout values and the full surface check pass (198 operations, 162 records, 314
selected constants, 1,567 representation values), as do 22 tooling tests. Native
library acceptance and clean installed SDK results are recorded with the
[receipt](results/m8-geometry-capture-2026-11-07.json).

Tessellation capture, triangle/line streams, nonzero stream rasterization,
secondary/generated composition and broader backend acceptance remain unverified.
Capture query counters are implemented; unrelated primitive-generation query types
are not implied. M8 next consolidates local-read/depth/MSAA and substantial consumer
composition, with a final explicit scope/acceptance disposition before M9.
