# Fragment shading rate

Foundation revision 11 implements an optional native shading-rate profile through
C and direct Rust. It adds pipeline rate, primitive output and attachment inputs,
independent enabling, physical properties and rate/sample tuples. It preserves
explicit caller control of image contents, compilation flags, dynamic state and
synchronization. This is an implemented Vulkan path with host/compiler coverage;
positive native shading-rate rendering remains unaccepted on the local GPU.

## Contract

`PIPELINE_SHADING_RATE`, `PRIMITIVE_SHADING_RATE` and `ATTACHMENT_SHADING_RATE` are
independent feature IDs. Request only the native inputs the application uses.
`MESH_PRIMITIVE_SHADING_RATE` additionally requires explicit `MESH_SHADER` and
`PRIMITIVE_SHADING_RATE`; none is enabled implicitly. Raster operations retain
their existing `RASTER` and graphics-queue requirements.

`QUERY_SHADING_RATE_LIMITS` returns one physical property record;
`QUERY_SHADING_RATES` returns counted `(fragment_size, samples)` tuples using the
ordinary query/capacity protocol. `samples` is a sample-count bitmask. Adapter and
device queries report the same physical properties, independently of the enabled
feature list. An absent native extension produces zero limits and an empty list.
Check feature availability separately: a format flag is not a device enable.

`graphics_state.shading_rate` optionally supplies a static `shading_rate_state`.
NULL means 1×1 and KEEP/KEEP. `DYNAMIC_SHADING_RATE` instead selects the recording
operation `set_shading_rate` and ignores the static pointer. The caller initializes
dynamic state before use and observes native invalidation across pipeline/list
changes; OGPU does not save or restore it.

The first combiner combines pipeline and primitive rates; the second combines that
result with the attachment rate. KEEP retains the first input, REPLACE selects the
second. MIN/MAX/MUL need the `nontrivial_combiners` property. Each fragment dimension
is 1, 2 or 4. Without pipeline enabling, the size must be 1×1; without primitive or
attachment enabling, the corresponding combiner must be KEEP. At least one input
must be enabled to supply static or dynamic rate state.

The physical rate table and interaction properties describe final-rate behavior,
including reduction to a supported rate. They are not extra rejection criteria:
a valid requested 4×4 size is not rejected merely because the device's final-rate
maximum is smaller. This follows the distinction between hard validity and soft
limits in the [KHR proposal](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_fragment_shading_rate.html).
Native reduction is not an OGPU emulation strategy.

`render_desc.shading_rate` optionally borrows a `shading_rate_attachment` during
render begin. Its view/image/backing remain live through GPU execution. The image
is 2D, single-sampled, has `IMAGE_SHADING_RATE` usage and a format supporting
`FORMAT_SHADING_RATE`; view type is 2D or 2D array. Query the exact format/profile
combination. The current native encoding uses unsigned shading-rate texels, not a
runtime-selected density format. The view uses identity components and GENERAL
or SHADING_RATE layout. `texel_size` is power-of-two and satisfies the queried
minimum, maximum and aspect ratio. The selected base mip, scaled by texel size,
must cover the complete offset render area. One view layer broadcasts; otherwise
the view covers the ordinary layer count or the highest multiview bit. Multiple
layers need `layered_attachments`. The same view cannot also be an ordinary render
attachment or resolve target. NULL render pointer is the sole absence encoding.

Graphics executables used with a rate attachment explicitly request
`COMPILE_SHADING_RATE_ATTACHMENT`. It requires attachment enabling, is invalid for
compute, and is never added automatically. Preparing the image, populating its
texels, assigning backing and transitioning/synchronizing it are caller operations.
`STAGE_SHADING_RATE` and `ACCESS_SHADING_RATE_READ` expose the attachment read scope;
ordinary copy/shader operations supply producer dependencies. Nothing in render
begin generates an image, inserts a barrier or rewrites descriptors.

The pinned Vulkan registry's `validusage.json` supplies the concrete image, view,
pipeline, rendering and setter restrictions. The backend lowers to
`VK_KHR_fragment_shading_rate`; density maps and vendor-specific alternate shading
rate models are not implicitly substituted. DX12/Metal mappings remain separate
implementation and acceptance work, including combiner/layer differences.

## Compiler and representation

The bounded Slang vertex/fragment compiler recognizes `SV_ShadingRate`, checks its
SPIR-V capability/extension and stage-specific built-in, and rejects it in legacy
generation. Vertex primitive output requires `PRIMITIVE_SHADING_RATE`. Fragment
input requires **any** of the three enabled native inputs. Its generated
`*_compatible` helper checks that disjunction without demanding all three or
choosing pipeline enabling for the application. Core shader requirement lists are
conjunctive; trusted artifact producers/callers retain responsibility for this
alternative requirement when bypassing the helper. The runtime performs no shader
reflection. Mesh primitive output is enabled in the backend but is not covered by
this bounded vertex/fragment generator fixture.

Revision 11 appends pointers to `graphics_state` (192 bytes) and `render_desc`
(112 bytes) on the supported 64-bit ABI. It adds four small records and one
recording operation. Header, library, generated interfaces and native Rust must
come from the same revision. Counted feature lists are unchanged; the private mask
is now 128 bits and is not public storage.

Discovery and executable preparation remain cold. The dynamic setter validates
scalar values and issues one native command. Render begin adds one stack structure
to the native chain. There is no per-draw allocation, shader rewrite, compilation,
readback, state cache or hidden GPU operation. This preserves the native scheduling
choices and command structure; it is not a measured performance-parity claim.

## Verification still needed

The [local receipt](results/m8-shading-rate-2026-11-06.md) separates real capability
queries/rejections, host rules, compiler artifacts and intercepted argument lowering
from positive native execution. The current Radeon exposes none of the inputs.
No second GPU is assumed or required to continue implementing M8.

A future native acceptance fixture must:

1. Independently enable each supported input. Render a shader that writes its
   observed `SV_ShadingRate` to an integer/readback-friendly target; compare against
   native allowed final rates, not a guaranteed coarse rate on every implementation.
2. Cover static/dynamic state and invalidation, primitive output, attachment
   KEEP/REPLACE and supported MIN/MAX/MUL combinations, including sample counts and
   the reported shader-depth/sample-mask interaction properties.
3. Populate the rate image through explicit copy and shader producer paths; check
   nonzero area offsets, base mip, single-layer broadcast, layered/multiview views,
   exact extent boundaries and guard pixels. Exercise primary/secondary replay and
   explicit generated-executable compatibility where available.
4. Verify attachment executable flags, enable dependencies, malformed records and
   missing synchronization with validation; compare a direct-native control for
   any ambiguous reduction/combiner result. Record supported subsets and skips.
5. Add mesh primitive output on actual supporting hardware. Keep unsupported
   capabilities and cross-backend execution explicitly unaccepted.

M8 next implements tile-local/input-attachment and pass-continuation profiles,
then remaining tessellation/geometry stages. This receipt does not close M8 or
substitute host tests for those native acceptance obligations.
