# Offscreen compute → graphics experiment

This narrow optional profile tests shared allocation, argument, submission, and
completion rules. It does not define a complete graphics API or add presentation.

ABI 18 implements the scoped/indexed/depth slice selected by the
[proposal](indexed-depth-proposal.md). The [migration checkpoint](indexed-depth-migration.md)
separates its focused checks from the remaining M4 scene/performance acceptance.
ABI 19 replaces single-record argument pairs with [explicit indirect ranges and
GPU counts](indirect-draw-ranges.md), including local draw identity.

Run the [C example](../examples/graphics.c) with `cargo xtask graphics`.

## Device and executable

`ogpu_device_create_graphics` requires a single queue family supporting both
graphics and compute, dynamic rendering, multiDrawIndirect, drawIndirectCount and
shaderDrawParameters, in addition
to the [modern execution baseline](modern-baseline.md).
It returns UNSUPPORTED if none exists. At ABI 8, ordinary `ogpu_device_create`
enables compute and images/heaps, never rasterization, even on a shared queue.
Color/depth attachments, raster creation and graphics access masks require explicit
graphics creation. Discovery remains independent. No new optional
graphics shader arithmetic features are enabled beyond that baseline.

Unified image layouts are optional: the extension guarantees layout efficiency,
but the current GENERAL-only image operations are legal without it. Both enabled
and unavailable cases use the same commands; no layout tracker or legacy path is added.
See [physical RADV and llvmpipe validation](hardware-validation.md).

`OgpuRaster` prepares valid vertex and fragment SPIR-V entry points named `main`,
with no descriptor-set bindings and a caller-defined copied root block shared by
both stages. Each stage uses an `OgpuShaderDesc` with its own specialization
constants; identical constant IDs in different stages are independent. See the
[executable contract](execution.md#c-api-and-ownership).
Native heap access uses [independent image/sampler heaps](descriptor-heaps.md);
existing descriptor-free Vulkan 1.2-targeted modules remain valid inputs.
The runtime still requires the modern device baseline. Vertex attributes
are fetched through GPU addresses: there is no vertex
binding layout. Vertex/fragment storage writes and atomics are not enabled. Shader
validity, stage interfaces, and reachable address bounds remain trusted contracts.

Raster creation selects triangle list or triangle strip (ABI 7); other topology
values are rejected. Primitive restart is disabled. Remaining state is fixed: fill, no culling, full-target
viewport/scissor, one sample, one RGBA8 UNORM or RGBA16F color output, no blending/stencil.
`OgpuRasterDesc` selects optional D32 depth, test/write flags and one of the eight
ordinary comparisons. Formats must match the rendering scope even with depth
testing disabled. Disabled testing suppresses fragment depth writes. No depth
format requires both flags zero and compare ALWAYS; there is no shader depth
output, depth bias/bounds test or primitive restart.
This is an experiment constraint, not a commitment to hard-code graphics state.

## Images and drawing

`ogpu_image_create` takes a copied description: 1D/2D, an explicit supported format,
extents and sampled/storage/color/depth/copy usages. The image owns specialized
optimal storage and a view when used as a color attachment. It has no device address
or CPU mapping. Format/dimension/usage support is checked; color use is limited to
2D RGBA8/RGBA16F. ABI 11 raster creation specifies the target format;
draws reject a mismatched image before modifying the recording. Image-heap slots supply sampled/storage descriptors for the actual format
and dimension. The older RGBA8-only target constructor and type were removed.

ABI 11 also admits RGBA16 UNORM for sampled/copy usage only, with real linear
filter support checks. It preserves pinned libplacebo's selected clipping-gamut
policy; see the [HDR consumer](libplacebo-hdr.md). Both sixteen-bit RGBA formats
use eight-byte packed texels and eight-byte copy offsets. Neither requires FP16
shader arithmetic.

Backing-memory selection is runtime policy: prefer eligible device-local memory,
then non-host-visible memory within that preference. Host-visible local memory
remains usable on unified-memory devices or when the image's requirements exclude
device-only types. Images remain unmapped. This avoids unintentionally preferring
a small visible VRAM heap; it does not add memory-budget tracking or allocation
retry/migration. See the [measured allocator correction](libplacebo-diagnosis.md#runtime-correction--2026-09-15).

`ogpu_image_check_support` checks the exact description on the created device
without allocating image/memory/descriptor resources. It shares creation's preflight,
including linear-filter support for sampled images; success does not guarantee
available allocation memory. See [query results and capability boundaries](execution-capabilities.md).

`ogpu_batch_begin_rendering` starts a scope with one required color attachment and
optional same-sized D32 depth attachment. Each independently selects CLEAR (explicit
finite value), LOAD or DISCARD, and STORE or DISCARD. Clear depth is in [0,1].
Images must already be initialized with `discard_image` before first use: CLEAR
defines values, not layout. LOAD needs defined prior contents; discarded values
cannot be read before being written. Begin/end add no implicit data barrier;
the caller orders prior/later attachment accesses, including load/store operations.
Empty scopes still perform attachment operations. End does not submit or wait.

Multiple compatible draws, heap bindings and buffer-retention calls may occur
inside a scope; dispatch/copy/discard/barrier/split endpoints may not. Timing
selection remains batch metadata. Invalid operations leave recording unchanged;
an open scope rejects submit/compile without consumption so the caller can end
and retry. Ordinary attachment ordering is native rendering behavior, not a
global barrier per draw. Attachment feedback/local reads remain unsupported.

`ogpu_batch_draw_indirect` executes one non-indexed indirect draw within the scope.
Its record is four uint32 values in order:
`vertex_count`, `instance_count`, `first_vertex`, `first_instance`. The last must
be zero because optional indirect-first-instance support is not enabled. The
record offset must be a multiple of four and all 16 bytes must fit the buffer.
GPU-produced contents cannot be validated by the CPU; the caller is responsible
for their validity and for shader-address bounds for every resulting invocation.

The draw takes an owning buffer handle plus byte offset for the indirect record.
This allows bounds checking and retention; the backend resolves the address at
recording and uses `vkCmdDrawIndirect2KHR`. It is not a new allocation type: compute
writes the same memory. The public handle is an ownership choice, not a workaround
for an older Vulkan command interface.

`ogpu_batch_draw_indexed_indirect` additionally supplies a retained `OgpuIndexRange`
within a buffer created with `OgpuBufferDesc.extra_usage=OGPU_BUFFER_INDEX`.
Other buffers use zero extra usage. This eligibility does not prevent compute
writes or sharing vertex/index/indirect ranges within a buffer. UINT16 and UINT32
are supported, with aligned nonempty ranges; the 20-byte indirect record contains
index_count, instance_count, first_index, signed vertex_offset, first_instance.
`first_index` is relative to the range. The caller guarantees generated counts
stay in range, first_instance is zero, and effective vertex addresses are valid.
The backend binds an address range and issues a real native indexed draw. There
is no CPU inspection, shader-emulated indexing, hidden copy or new allocation type.

D32 images use DEPTH plus optional COPY_SRC/COPY_DST only, 2D and one mip/layer/sample.
Do not equate D32 with R32 color storage. Exact support queries and creation share
validation; depth operations use the depth aspect. Sampled/storage depth is deferred.

`ogpu_batch_copy_image_to_buffer` copies the whole image into an ordinary buffer at a
texel-size-aligned offset, tightly packed as width × height × texel-size bytes
(four for RGBA8/R32F/D32F, eight for RGBA16F/RGBA16_UNORM) in the image's
format, row by
row starting at image coordinate (0,0). The caller must initialize the target through
explicit discard followed by writes/CLEAR, in this or an earlier successfully
submitted batch, with all copied texels written before the copy. There is no same-batch
initialization scan. Rejected/abandoned initialization does not authorize later reads.
Subsequent uses preserve contents unless CLEAR or discard explicitly invalidates them.

`ogpu_batch_copy_buffer_to_image` uploads the same packed representation from a
HOST or DEVICE buffer, retaining both operands. Initialize GENERAL before first use;
the upload orders prior GPU accesses, and later shader/attachment consumers require
an explicit TRANSFER_WRITE dependency. Repeated uploads preserve the layout.

Discard and copy operations manage their own image transitions and copy
dependencies. The Vulkan backend uses an initial UNDEFINED layout (discard), a
transition to GENERAL, separately scoped dynamic rendering, and a global synchronization2
dependency making earlier image writes visible to address-based image readback. Ordinary
use stays in GENERAL; no render-pass/framebuffer objects or mutable layout tracker
remain. The discard transition orders prior uses, including across submissions.
Explicit discard also prepares compute-written images without drawing or clearing;
write texels before reading them. Preservation spans ordered submissions on the
one queue; initialization and explicit data dependencies are caller obligations.
No layout-state tracker or hidden initialization submission is needed.

## Shared dependencies and ownership

The access vocabulary extends with vertex-read, fragment-read, indirect-read,
color-read/write, index-read, depth-read/write, transfer-read, and transfer-write.
INDEX_READ maps to fixed-function index input, not vertex shader reads; depth
accesses cover early and late tests. Graphics accesses are rejected on
a compute-only execution queue. Batch barriers translate each access to its
execution stage; they remain global and do not inspect pointer arguments.

The example explicitly records COMPUTE_WRITE → VERTEX_READ | INDIRECT_READ before
the draw. Draw order alone does not establish this data dependency. Batch boundary
dependencies now cover host writes and GPU writes across all supported command
types. A target copy followed by compute requires TRANSFER_WRITE → COMPUTE_READ.

Batches and submission resources retain directly supplied targets, raster executables,
image/sampler heaps, indirect buffers, and copy destinations. At ABI 9, wait/terminal
poll retires native commands before releasing these references. Image heaps retain
their populated entries; editing requires releasing every recording/unretired use,
but completed receipts may remain alive. Destroying public resource handles does not
free resources while retained uses remain. Allocations referenced ONLY through GPU addresses
still need caller-managed lifetime. Do not perform CPU buffer access until all
submitted uses of that whole allocation complete. The existing external
serialization, one-shot state, failure cleanup, and draining-destruction rules apply.

## Evidence and limits

The runnable example generates three vertex positions and a draw record on
the GPU, draws into a 64×64 target, copies to a readback buffer, and waits once.
It checks opaque-black background and solid-red interior pixels away from
rasterization boundaries. No CPU readback of vertices or draw arguments is needed.

The [image-loop experiment](image-loop.md), run with `cargo xtask image-loop`,
extends this to graphics → compute → graphics using image-to-buffer copies and
fragment-shader address reads. It uses this same profile without API additions;
it does not introduce sampled images or storage-image compute access. The separate
[heap-image experiment](heap-images.md) adds both and avoids intermediate copies.
Its [current follow-up](descriptor-heaps.md) verifies preserved images across three
submissions and independent heaps with nearest/linear clamp/repeat sampling.

Tests cover graphics queue selection without breaking compute-only selection,
invalid extents/shaders, buffer bounds/alignment, wrong-device objects,
retained-resource lifetimes, LOAD/CLEAR, image reuse, abandoned/rejected discard,
and partial creation failures.
`cargo xtask gpu-tests` runs the Vulkan-backed graphics tests with the existing
compute tests and fails on reported validation errors. Creation-failure injection
exercises image/memory/view cleanup, second-module failure,
and a returned pipeline handle alongside an error. Queue selection,
extent limits, memory selection, access masks, and NULL C arguments also have tests
that do not require a GPU.

Not included: windows, surfaces, swapchains, general image uploads or filtering,
depth/stencil, blending, indexing, mesh shaders, or multiple queues. Vulkan pipeline
objects remain backend details; classic render passes are no longer used.

## Shader reproduction and verification

The three checked-in shader binaries were generated with glslang 16.4.0 and
validated with SPIRV-Tools 1.4.357.0. Normal builds do not invoke a shader compiler.
The compute root contains two GPU addresses at offsets 0 and 8; the vertex shader
uses only the first. Positions are three 16-byte-aligned vec4 values. The producer
runs once and writes the entire 16-byte indirect record, with first_instance = 0.

```sh
glslangValidator -V --target-env vulkan1.2 examples/shaders/triangle.comp -o examples/shaders/triangle.comp.spv
glslangValidator -V --target-env vulkan1.2 examples/shaders/triangle.vert -o examples/shaders/triangle.vert.spv
glslangValidator -V --target-env vulkan1.2 examples/shaders/triangle.frag -o examples/shaders/triangle.frag.spv
spirv-val --target-env vulkan1.2 examples/shaders/triangle.comp.spv
spirv-val --target-env vulkan1.2 examples/shaders/triangle.vert.spv
spirv-val --target-env vulkan1.2 examples/shaders/triangle.frag.spv
cargo xtask graphics
cargo xtask gpu-tests
```

The image checks are correctness evidence, not a performance measurement or proof
of cross-vendor rasterization equivalence. Actual hardware device loss and arbitrary
shader faults are not exercised. See [development](development.md) for validation
layer settings and loader selection.

The C example and Rust graphics/reuse/failure tests have passed locally on the
RX 5700 XT (RADV) and llvmpipe with synchronization validation enabled before the
modern migration. The modern backend is verified on llvmpipe only so far. Existing
compute examples still pass. CI is configured to run the examples, Vulkan-backed
tests, and SPIR-V validation; local execution is not a claim that hosted CI has run.

Backend references: [address commands](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_device_address_commands.html),
[dynamic rendering](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_dynamic_rendering.html),
and [unified layouts](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_unified_image_layouts.html).
