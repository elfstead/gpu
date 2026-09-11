# Offscreen compute → graphics experiment

This narrow optional profile tests shared allocation, argument, submission, and
completion rules. It does not define a complete graphics API or add presentation.

Run the [C example](../examples/graphics.c) with `cargo xtask graphics`.

## Device and executable

`ogpu_device_create_graphics` requires a single queue family supporting both
graphics and compute, in addition to the existing Vulkan 1.2/BDA requirements.
It returns UNSUPPORTED if none exists. Ordinary `ogpu_device_create` continues to
accept compute-only devices; discovery remains independent. No new optional
Vulkan shader features are enabled.

`OgpuRaster` prepares valid descriptor-free Vulkan 1.2 vertex and fragment SPIR-V
entry points named `main`, with a caller-defined copied root block shared by both
stages. Vertex attributes are fetched through GPU addresses: there is no vertex
binding layout. Vertex/fragment storage writes and atomics are not enabled. Shader
validity, stage interfaces, and reachable address bounds remain trusted contracts.

Raster state is deliberately fixed: triangle list, fill, no culling, full-target
viewport/scissor, one sample, one RGBA8 UNORM color output, no blending/depth/stencil.
This is an experiment constraint, not a commitment to hard-code graphics state.

## Images and drawing

`ogpu_target_create_rgba8` owns a two-dimensional, single-layer, single-mip image
with specialized optimal storage, a view, and attachment resources. It has no
device address or CPU mapping. Width/height and format support are checked.

`ogpu_batch_draw_indirect` clears the target to opaque black, then executes one
non-indexed indirect draw. Its record is four uint32 values in order:
`vertex_count`, `instance_count`, `first_vertex`, `first_instance`. The last must
be zero because optional indirect-first-instance support is not enabled. The
record offset must be a multiple of four and all 16 bytes must fit the buffer.
GPU-produced contents cannot be validated by the CPU; the caller is responsible
for their validity and for shader-address bounds for every resulting invocation.

The draw takes an owning buffer handle plus byte offset for the indirect record.
This allows bounds checking and retention while the Vulkan 1.2 backend uses its
buffer-based draw command. It is not a new allocation type; the same allocation
can be written through a GPU address by compute. Address-range commands remain a
future experiment rather than requiring a newer Vulkan extension now.

`ogpu_batch_copy_target` copies the whole target into an ordinary buffer at a
four-byte-aligned offset, tightly packed as width × height × 4 RGBA bytes, row by
row starting at image coordinate (0,0). It must follow a draw to that target in
the SAME batch. This local rule avoids an implicit cross-submission image-state
tracker in the first experiment. Repeated draws/copies, including target reuse
in subsequent batches, are permitted; every draw discards previous image contents.

Known target operations manage their own image transitions and attachment/copy
dependencies. The Vulkan backend uses an initial UNDEFINED layout (discard), a
color attachment subpass, and a final TRANSFER_SRC layout, with external
dependencies ordering prior target use and making color writes visible to copies.
This is a documented clear/draw/readback operation, not a general image-state model.

## Shared dependencies and ownership

The access vocabulary extends with vertex-read, fragment-read, indirect-read,
color-write, transfer-read, and transfer-write. Graphics accesses are rejected on
a compute-only execution queue. Batch barriers translate each access to its
execution stage; they remain global and do not inspect pointer arguments.

The example explicitly records COMPUTE_WRITE → VERTEX_READ | INDIRECT_READ before
the draw. Draw order alone does not establish this data dependency. Batch boundary
dependencies now cover host writes and GPU writes across all supported command
types. A target copy followed by compute requires TRANSFER_WRITE → COMPUTE_READ.

Batches and completions retain directly supplied targets, raster executables,
indirect buffers, and copy destinations. Destroying those public handles does not
free the retained resources. Allocations referenced ONLY through GPU addresses
still need caller-managed lifetime. Do not perform CPU buffer access until all
submitted uses of that whole allocation complete. The existing external
serialization, one-shot state, failure cleanup, and draining-destruction rules apply.

## Evidence and limits

The runnable example generates three vertex positions and a draw record on
the GPU, draws into a 64×64 target, copies to a readback buffer, and waits once.
It checks opaque-black background and solid-red interior pixels away from
rasterization boundaries. No CPU readback of vertices or draw arguments is needed.

Tests cover graphics queue selection without breaking compute-only selection,
invalid extents/shaders, buffer bounds/alignment, wrong-device objects, missing
prior draw, retained-resource lifetimes, image reuse, and partial creation failures.
`cargo xtask gpu-tests` runs the Vulkan-backed graphics tests with the existing
compute tests and fails on reported validation errors. Creation-failure injection
exercises render-pass/image/memory/view/framebuffer cleanup, second-module failure,
layout failure, and a returned pipeline handle alongside an error. Queue selection,
extent limits, memory selection, access masks, and NULL C arguments also have tests
that do not require a GPU.

Not included: windows, surfaces, swapchains, sampling, general image uploads,
depth/stencil, blending, indexing, mesh shaders, or multiple queues. Classic Vulkan
render passes/pipelines are backend details, not public objects.

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
RX 5700 XT (RADV) and llvmpipe with synchronization validation enabled. Existing
compute examples still pass. CI is configured to run the examples, Vulkan-backed
tests, and SPIR-V validation; local execution is not a claim that hosted CI has run.

Backend references: [indirect draws](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdDrawIndirect.html),
[attachment dependencies](https://docs.vulkan.org/refpages/latest/refpages/source/VkSubpassDependency.html),
and [image readback](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdCopyImageToBuffer.html).
