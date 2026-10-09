# Foundation revision: implementation status

Started 2026-10-08, following the [whole-API design](whole-api-design.md).
The source-only [next header](../include/ogpu_next.h) contains implemented Linux
Vulkan declarations; the [combined sketch](drafts/ogpu_next.h) adds the remaining
unimplemented surface. Neither is installed by the SDK yet. ABI 20 still owns the
working examples/consumers. This is a migration boundary, not two permanent runtimes.

## Implemented setup

- Immutable discovery snapshot with borrowed adapter handles. Created devices
  retain the native connection, not discovery or a global current context.
- Concrete versioned records for identity, queue domains/counts/granularity,
  memory types/heaps, and available versus requested/enabled optional features.
  Heap size is not presented as free memory/budget. Unknown records/features fail.
- Explicit device queue requests, exact counts and finite priorities. No queue
  clamping, family substitution, aliasing or automatically selected queue zero.
  Device queries report created counts; adapter queries report physical counts.
- Separate timeline objects and non-owning value points; poll, timed wait and host
  signal. No heap-allocated completion receipt, implicit retention or wait-on-drop.
- Object-local host concurrency. Independent native creations/queries/waits share
  immutable device data; terminal loss is atomic, not a device-wide mutex.

The baseline remains modern Vulkan with descriptor heaps, untyped pointers and
address commands. Raster feature enabling, FP16, sampler anisotropy and unified image layouts are
explicit optional bits, not silently enabled merely because hardware supports them.
These bits describe native enabling, not a promise that every corresponding
graphics/numerical profile is implemented. Compute and raster commands are described below.
Sparse/protected queue flags and memory properties describe physical facilities;
their execution features are not implicitly enabled.

Version 1 accepts exact record sizes and rejects record chains/flags it does not
understand. Query buffers belong to the caller: count-only queries allocate nothing;
insufficient capacity reports the required count without a partial array write.
Descriptions are validated before reading the larger record. Every handle creation
clears its output on failure. Unspecified native failure outputs are not adopted
as handles; explicitly guaranteed partial pipeline outputs are cleaned up.

Queue domain IDs map directly to Vulkan families; requests lower to separate native
queue-create records. This follows Vulkan's exact family/count/priority rules,
without inventing a portable scheduler. See the
[native queue-create contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkDeviceQueueCreateInfo.html).
Stage-scoped submission preserves actual wait/signal scopes rather
than treating every signaled value as whole-list completion; see
[native semaphore submit scopes](https://docs.vulkan.org/refpages/latest/refpages/source/VkSemaphoreSubmitInfo.html).

## Implemented backing and ranges

The next header also implements explicit linear/opaque allocations, compatible
memory-type/size/alignment requirements, non-owning 64-bit spans, GPU addresses,
persistent mappings, and range flush/invalidate. Callers choose the exact memory
type and explicit exclusive/concurrent queue-family sharing; there is no hidden
HOST/DEVICE preference, staging allocator, resource retention or wait.

Linear backing is one native addressable buffer/allocation which the application
partitions freely. It does not create a native buffer per span. Ordinary alignment
uses Vulkan's native address guarantee with no added padding. Larger requested GPU
alignment uses bounded leading padding, included in the requirements query. Opaque
backing allocates raw memory without a buffer; optimal images can now be placed
in it. Mixed buffer/image placement is not implemented yet. Device byte sizes remain 64-bit; host mapping
alone checks whether an allocation fits the host pointer range.

The native requirements query creates neither temporary buffers nor GPU memory.
Vulkan maintenance4 is explicitly enabled for this path. See the
[requirements query](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetDeviceBufferMemoryRequirements.html)
and [address-alignment guarantee](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetBufferDeviceAddress.html).
Noncoherent views expose both atom size and the first-byte offset within an atom;
cache operations round outward without overflowing or exceeding the allocation.
The application owns synchronization of every affected byte, including neighbors.
Mapping/unmapping one allocation is externally serialized; distinct views share
one mapping, and unmap invalidates them all.

## Implemented command storage and submission

Explicit per-domain arenas reserve primary list slots, not fictitious native-byte
budgets. Begin consumes a reserved slot without growing OGPU storage. Reserve can
grow without moving existing list handles; reset retains capacity, and trim resets,
frees excess slots and asks the driver to release pool storage. Neither destruction
nor cancellation waits. Failed begin/end consume their slot until reset; any reset
attempt invalidates handles, and failed reset prevents recording until a successful
reset. Independent arenas can record on different host threads.

One-shot, serial replay and simultaneous replay map to native command-buffer modes.
The caller owns pending-use tracking, synchronization and resource lifetimes; an
arena retains no memory, timeline or resource list. One-shot work becomes consumed
only on successful submission. Serial completion and simultaneous-use hazards are
caller preconditions, not an internal completion registry. See the native
[submission](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueueSubmit2.html)
and [pool reset](https://docs.vulkan.org/refpages/latest/refpages/source/vkResetCommandPool.html)
contracts.

Submission borrows explicitly sized/aligned caller scratch for native array
translation. Batched ranged memory dependencies use the same approach and lower
to one native barrier call. Query once and reuse stack/heap storage; no OGPU heap
allocation occurs in begin/end, command encoding or submission. Driver-internal
allocations remain possible. This choice does **not** require per-submit allocation
or make a native command-memory byte-budget promise. Duplicate-list/point checks
and signal ordering are trusted preconditions: no quadratic scans or hidden sets.

Copy uses device-address commands and preserves the allocation's known usage flags;
fill writes a selected backing range. Invalid range/device/usage/scope or encoding
errors poison end, which returns no executable list. Global/ranged barriers separate
stage and access masks. Exclusive queue-family ownership transfers require explicit
release/acquire records plus timeline edges; concurrent sharing remains an explicit
allocation choice. There is no implicit current owner or queue serialization.

Wait/signal stages are preserved, with no automatic completion signal. A narrow
signal is not a blanket completion receipt. Submit OOM is retryable with one-shot
work intact; loss is sticky. Unexpected native submission errors conservatively
poison the device instead of pretending a retry or resource reuse is safe.

This is still an unstabilized subset. Secondary recording/inheritance and split
dependencies are not implemented, and the current
stage vocabulary includes broad transfer/vertex/depth groups. Finer graphics scopes
must be added with the executable surface. Multiple lists and timeline edges are
batched in one native submit record; an array of distinct submit records in one
native call remains to implement. None is ruled out by the design. No performance
equivalence claim follows merely from the absence of OGPU hot-path allocation.

## Implemented images and views

Images interpret caller-selected opaque backing spans: format, dimensionality,
mips, layers, samples, uses, mutable-format list, alias permission and concurrent
queue families are explicit. Requirements query the exact native tuple without
creating a GPU object or allocating GPU storage. Compatible types, size/alignment
and dedicated-required/preferred metadata follow the existing capacity protocol.
Two disjoint optimal images may share an allocation, including at nonzero offsets;
no runtime allocator chooses placement or tracks aliases. Linear-buffer backing
remains a dedicated buffer; mixed buffer/image placement needs a later raw-backing
buffer interpretation, not an implicit alias behind that object.

There are two creation paths: create into an existing span, or explicitly create
an unbound image, allocate memory dedicated to that image, then bind. The latter
handles the native requirement that dedicated allocation names an existing image.
It does not allocate implicitly or combine image and memory ownership. Binding is
one-shot. No view or GPU command accepts an unbound image. See the native
[allocation-free requirements query](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetDeviceImageMemoryRequirements.html)
and [dedicated-image association](https://docs.vulkan.org/refpages/latest/refpages/source/VkMemoryDedicatedAllocateInfo.html).

Independent views select compatible formats, dimensions, mip/layer/aspect ranges,
usage subsets and component mappings. The initial table covers sixteen common
uncompressed color/depth/stencil formats. 1D/2D/3D and array/cube views and ordinary
multisample images are implemented subject to exact native support. Optional
cube arrays, 2D views of 3D slices, multisampled storage, extended usage, compressed
and multi-planar formats, linear tiling, sparse/external/disjoint images and
separate depth/stencil layout enabling remain outside this implemented profile.
They are not ruled out by the design and are never silently substituted.

Image transitions join buffer/global dependencies in one scratch-backed native
barrier. Native-compatible GENERAL, transfer, shader-read and attachment states
are explicit, with no current-layout tracker or automatic initialization. Discard
only discards contents; it does not remove hazard synchronization. Color and depth/
stencil clears and address-based memory/image copies preserve caller-specified
states. Copies expose mip/layer/subregions and byte row/slice pitches; checked
size arithmetic, queue granularity, texel alignment and native pitch limits prevent
invalid encoding. There is no staging allocation, hidden transfer or resource
retention. See the native [copy-region constraints](https://docs.vulkan.org/refpages/latest/refpages/source/VkCopyDeviceMemoryImageInfoKHR.html).

The source-only barrier-scratch query now takes both memory and image counts.
The combined draft imports these declarations; installed ABI 20 is unchanged.
Image-to-image copies now take a single source/destination region record with
explicit layouts. Matching-dimensional copies preserve sample count, mip/layer
offsets and raw compatible-format bits; disjoint copies within one image are
allowed. Shared bounds/granularity checks also serve the memory/image path. Caller
proof of memory non-overlap includes aliases: there is no allocation-wide alias
registry or implicit staging. See the [native copy contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkCopyImageInfo2.html).

Standalone color resolves consume multisample images into single-sample images
with explicit source/destination subregions. The current `NATIVE_COLOR` strategy
preserves Vulkan's implementation-dependent default resolve mode/precision and
integer-sample selection; it is deliberately **not** advertised as an exact average
or sample-zero policy. Format support is captured at image preparation, not queried
per command. Explicit-mode/depth/stencil and attachment resolves still need their
feature profiles. See the [native resolve semantics](https://docs.vulkan.org/refpages/latest/refpages/source/VkResolveImageInfo2.html).

The C path verifies two-layer mip-to-mip copies with moved rectangles, a disjoint
same-image GENERAL copy, a four-sample image copy and a partial standalone resolve,
with full output/guard checks. All resolved source samples have the same known
color: this proves transfer/region lowering, not per-sample numerical behavior.
Depth/stencil image-to-image, compatible-format reinterpretation, other dimensions,
image ownership/alias execution and nonuniform-sample numerical coverage remain
to verify. Compressed/multiplanar and cross-dimensional copies remain unimplemented.
Creation support is not a claim of full shader-consumer verification.

## Implemented descriptor encoding and binding

The earlier owned `heap_create(capacity)` draft is replaced with host-byte encoding
and borrowed GPU-range binding. Capacity, placement, mixed descriptor layout,
slot allocation, upload strategy and resource lifetimes belong to the caller.
The device exposes descriptor sizes/alignments, heap limits and reserved ranges.
Resource writers encode sampled/storage images, storage/uniform buffer spans and
input attachments; sampler writers encode ordinary filtering/address/compare/LOD
state with explicitly enabled optional anisotropy. Input-attachment execution is
not integrated yet. Null, texel-buffer, acceleration-structure, tensor, YCbCr and
combined descriptors and extended sampler modes remain unimplemented profiles.

Each writer validates the entire batch, builds native records in caller scratch,
then makes one native encoding call. There is no OGPU allocation, slot table,
retained resource, implicit flush, staging copy or heap lock. Local rejection
leaves outputs unchanged; native failure may partially write destinations. The
caller chooses recovery rather than paying for rollback staging or heap-wide
poisoning. Independent host ranges can be encoded concurrently. See native
[resource encoding](https://docs.vulkan.org/refpages/latest/refpages/source/vkWriteResourceDescriptorsEXT.html)
and [sampler encoding](https://docs.vulkan.org/refpages/latest/refpages/source/vkWriteSamplerDescriptorsEXT.html).

Opaque device-specific bytes can be written directly into mapped heap storage or
copied there explicitly. Binding names a linear-memory span with DESCRIPTOR_HEAP
usage and an implementation-reserved subrange. Reserved bytes remain inaccessible
from recording until **all referencing command buffers are reset or destroyed**,
not just GPU completion. Partially overlapping reservations are forbidden; exact
same-kind reservations may be shared. The caller owns that proof and cache-atom
isolation. Nonreserved slots remain independently mutable subject to hazards;
no whole-heap freeze or cloning. See the native
[resource-heap reservation contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBindResourceHeapEXT.html).
Dedicated resource/sampler heap-read access masks express explicit copy-to-shader
dependencies without tracking descriptor contents or current resource states.

Shader consumption now joins this path in the public C foundation fixture. Its
Slang kernel samples a texture through a sampler descriptor, writes a storage image
and writes a storage-buffer descriptor in the same dispatch. The caller chooses a
mixed resource layout, explicitly uploads its encoded bytes into LOCAL heap memory
and inserts COPY-to-resource-heap-read synchronization. Samplers use mapped heap
storage. Only descriptor payload bytes are copied/flushed; reservations are never
touched. LOCAL can be unified/host-visible memory: this does not assume discrete
VRAM on every driver.

An important artifact contract is explicit: this Slang lowering uses typed heap
arrays with `OpConstantSizeOfEXT`. The shader's stride is each descriptor size
**rounded up to its alignment**, not its raw encoded size and not an application
slot size. The fixture converts selected byte offsets into those typed indices;
no runtime slot registry/translation or one fixed mixed-slot layout is required.
See [native shader descriptor sizes](https://docs.vulkan.org/spec/latest/chapters/interfaces.html#interfaces-resources-layout).
This is a manually stated fixture ABI; generated metadata still needs to capture
offset/stride/format/index conventions for independent consumers.

Two executions of the same recorded list observe changed source pixels and a
changed buffer descriptor destination. Readback checks both storage-image texels
and buffer output, including untouched surrounding bytes. While each shader-using
list is gated pending, the host updates an unused sampler slot in a separate cache
atom, then releases the gate. This tests pending-use range independence, not
simultaneous GPU execution and host writes or a performance comparison. Uniform/
input-attachment consumption, broader formats, generated integration and Metal
lowering remain to verify. No descriptor serialization or cross-device byte
compatibility is promised. No runtime API addition was needed for this consumer.

## Implemented compute preparation and arguments

The source-only surface now creates native compute executables from SPIR-V with
an explicit UTF-8 entry name, byte-typed specialization records, argument interface
and shader requirements. The producer states the actual post-specialization local
size, shared-memory use and optional features; OGPU checks limits/enabling but does
not reflect or validate shader semantics. Valid device code, descriptor interfaces,
reachable address bounds, alignment, races and uniformity remain caller contracts.
Prepared metadata is copied; shader bytes and specialization storage are borrowed
only during creation. Executables borrow their device and do not retain resources.

Preparation creates a native compute pipeline with descriptor-heap access and no
pipeline layout; it needs no shader-object feature or runtime translation. Shader
modules are released immediately after preparation. Pipeline failure outputs are
cleaned up according to Vulkan's partial-creation contract. This differs from
unspecified failed shader-module outputs, which are never adopted. Additional
artifact formats, set/binding mappings, additional shader stages
and richer numerical/launch requirements remain unimplemented, not silently
substituted. See [native compute preparation](https://docs.vulkan.org/refpages/latest/refpages/source/VkComputePipelineCreateInfo.html).

Argument metadata names a byte footprint and any number of nonoverlapping root
slots with explicit offsets, stages and pointee alignments. The implemented compute
and graphics profiles expose **one shared byte namespace**, not fictitious independent native
stage banks. Inline updates copy specified bytes during recording. Root updates
copy only an eight-byte address into the declared slot: no pointee copy, address
registry, implicit upload allocation, refcount or retained backing. Pointees are
read at execution, including replay. Root updates do not touch other slots/inline
bytes. Explicit inline writes may target the same bytes; callers own initialization
and compatible interpretation across executable binds. Stage-isolated graphics
domains still need an explicit mapping/profile, not hidden shadow copies. See the
[native push-data model](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdPushDataEXT.html).

Direct launch preserves group dimensions; indirect launch reads three u32 values
from an application-selected INDIRECT span, without readback or argument staging.
The caller synchronizes GPU-written dimensions and proves their execution-time
bounds. Dynamic shared memory and launch-extension records currently return
UNSUPPORTED, never command-time JIT specialization. Binding and launch require a
compute-capable queue and keep no completion/resource registry. See
[native indirect dispatch](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdDispatchIndirect.html).

The checked-in Slang fixtures use named entries, device roots, inline controls,
specialization and direct descriptor heaps. `cargo xtask foundation-shaders --check`
reproduces all four artifacts using Slang 2026.14.1 and validates SPIR-V. This is an explicit test
artifact ABI, not yet integration with the generated consumer metadata workflow.

## Implemented graphics preparation and recording

Vertex/fragment SPIR-V preparation now shares the executable and argument model.
Static state states topology, culling, front face, color formats/write masks/blend,
depth format/test/write/compare and blend constants. Viewport/scissor is explicitly
dynamic; no state is inferred from an attachment and no draw compiles a pipeline.
Vertex and fragment interfaces describe the same byte namespace, with root slots
visible to either or both stages. Compute and graphics pipeline binds are independent,
but push bytes are shared even across those bind points: callers own compatible
interpretation and initialization, not hidden per-stage state restoration.

Rendering scopes borrow views in declared layouts and explicit load/store state,
area/layers and caller-owned translation scratch. Clear-only and attachmentless
scopes need no executable. Local attachment checks finish before the native begin;
there is no attachment retention or fixed internal color-array capacity. Draw-time
pipeline/attachment compatibility and shader accesses are trusted caller contracts,
not per-draw scans. Transfers, dispatch and generic barriers currently occur outside
scopes; inter-scope preservation and hazards require explicit LOAD and dependencies.
See the native [rendering contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkRenderingInfo.html).

Direct/indexed draws preserve count, instance, first and signed base-vertex values.
16/32-bit indices bind an explicit INDEX span using native address commands.
Indirect and GPU-counted variants consume caller-defined strided INDIRECT spans
directly, with separate native-layout wire records for indexed/nonindexed arguments.
GPU count remains on the GPU; no scan, repack, allocation, retained memory owner,
implicit barrier or host wait is added. Host-visible range/stride checks do not prove
GPU-written contents: callers guarantee index bounds, valid execution-time fields
and synchronization. The [native counted-draw contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkDrawIndirectCount2InfoKHR.html)
also constrains the GPU count itself, not only the application's maximum.

The implemented profile is deliberately stated, not a fundamental restriction:
vertex/fragment, optional D16/D32 or combined D24S8/D32S8 depth/stencil, explicit supported sample counts, fill/depth
clipping, vertex pulling or native vertex fetch, one viewport/scissor and matching per-color blend state.
Color attachments now resolve explicitly at scope end: average for normalized/float,
sample zero for integer formats, independent of whether the multisample source is
stored or discarded. The single-sample resolve view, matching format, usage/layout
and lifetime belong to the caller; no transient target or implicit transition is
created. Integer-color, depth, stencil and attachmentless sample masks are reported
separately. See the [native attachment resolve contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkRenderingAttachmentInfo.html).

Static front/back stencil operations, compare, masks and references map directly to
native pipeline state. Depth and stencil have independent load/store/clear records;
when both are supplied they name the same view and layout. Combined layout barriers
cover both aspects. Draw-time read-only permissions remain caller obligations, not
per-draw state reconstruction. No separate depth/stencil layouts are enabled.

Dynamic stencil state, depth/stencil resolves, sample shading/masks/custom positions, independent
blend, dynamic vertex input/stride and additional
stages, restart, wide lines, richer dynamic state, tile-local dependencies and
nonzero indirect first-instance enabling remain work. Unsupported requested state
is rejected, never emulated with a hidden state cache or fallback. The existing
sample masks still require exact format/image-tuple support, not guessed allocation.

The public C offscreen consumer checks opaque and additive pipelines, depth rejection,
LOAD across scopes, negative base vertex, direct/indexed and all four indirect
variants in disjoint pixel regions, and replay with changed roots and GPU-resident
count. Every output pixel/depth and surrounding guards is checked. A shader detail
caught by this test: Slang's `SV_VertexID` subtracts base vertex; the fixture explicitly
uses `SV_VulkanVertexID` to test native offset semantics. This distinction belongs in
future generated graphics metadata, not a runtime draw rewrite. See
[Slang's SPIR-V semantics](https://docs.shader-slang.org/en/latest/external/slang/docs/user-guide/a2-01-spirv-target-specific.html).

The same consumer now runs at one and four samples. Four-sample depth rejection,
blending, LOAD, all draw forms and replay resolve into caller-owned single-sample
storage. An actual geometry edge produces mixed covered/uncovered samples, checking
intermediate resolved color without assuming exact rounding. A fractional viewport
alone was not a reliable coverage fixture; no API workaround was introduced.
Multisample depth is tested through visibility, not direct depth readback. Integer
sample-zero resolve and other sample counts remain to verify on real shader output.

A third run uses D32S8: depth-passing draws replace stencil with different references,
depth failure keeps the old value, and EQUAL gates subsequent blue draws. Both replay
outputs and separately copied depth/stencil planes pass on Radeon. Both faces use the
same state in this execution fixture; unit tests cover unmodified field translation,
not every operation or independently different front/back state on hardware.

**Known llvmpipe combined-plane copy failure (2026-10-09):** the default full consumer
fails on D32S8 readback. Depth bytes are interleaved float/stencil words (pixel 1 reads
`0x00000001` instead of float 0.25); stencil bytes start `00 00 80 3e` (float 0.25).
This indicates whole-format copies rather than aspect extraction, including writes
beyond the requested plane span. The allocation is larger than these fixture writes,
but this path must not be treated as safe on that driver. The inspected
[lavapipe address-copy implementation](https://raw.githubusercontent.com/chaotic-cx/mesa-mirror/main/src/gallium/frontends/lavapipe/lvp_execute.c)
delegates to the host image-copy path; the exact driver defect still needs an upstream
reproducer/fix. No fallback, byte repacking or driver-name branch was added to OGPU.
`OGPU_FOUNDATION_STENCIL_RASTER_ONLY=1 cargo xtask foundation` explicitly omits combined
plane copies and their checks; stencil-dependent colors, replay and untouched guards
then pass on llvmpipe. This is diagnostic isolation, not a full llvmpipe pass or a
runtime workaround. Default testing keeps the failure visible. D24S8 execution and
combined depth/stencil multisampling remain unverified.

Fixed-function vertex input now has explicit binding IDs, byte strides, vertex/instance
rates and format/offset/location records. Preparation checks limits, duplicate IDs,
missing bindings and native vertex-format support; it does not rewrite shader inputs.
Sparse IDs, zero strides and attributes extending beyond a stride remain legal native
strategies. `RG32_FLOAT` and `RGB32_FLOAT` extend the shared format vocabulary; image
support for any exact tuple is still queried, not implied by vertex-fetch support.

Vertex binding translates consecutive caller VERTEX spans into address bindings using
caller-owned host scratch, then issues one native command. No hidden vertex buffer,
layout conversion, upload or command-time allocation is introduced. Pipeline strides
remain static. At draw time the caller proves binding completeness and all fetched
addresses, alignment, first/base indices and lifetimes; no per-draw resource scan is
added. See [native address binding](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBindVertexBuffers3KHR.html)
and [attribute layout](https://docs.vulkan.org/refpages/latest/refpages/source/VkVertexInputAttributeDescription.html).

The additional C run uses two sparse binding IDs, padded records and nonzero attribute
offsets, a batched bind before rendering, per-instance depth scaling, nonzero direct
first-instance, negative indexed base vertex, every indirect draw variant, and replay
after changing instance data. Pixel/depth outputs and guards are checked. This validates
RG32 float fetch and divisor-one instancing; other formats, zero-stride execution,
dynamic input state and nontrivial divisors still need their own coverage/profiles.
The source-only graphics limits/state records grow to 72/160 bytes; ABI 20 is unchanged.

## Implemented scalar numerical profiles

Alongside existing FP16, devices now advertise and explicitly enable `INT8`, `INT16`,
`INT64`, `FLOAT64` and independent `STORAGE8`. Each maps to its native shader arithmetic
or storage feature; 16-bit storage-buffer access remains part of the modern baseline.
An application can request 8-bit storage without 8-bit arithmetic, or vice versa.
Executable requirements must be a subset of enabled features. Neither preparation
nor dispatch converts precision, inserts packing kernels or emulates unavailable
types. See [Vulkan scalar/storage enabling](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceVulkan12Features.html)
and [core shader types](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceFeatures.html).

The C numerical kernel consumes caller-addressed 8/16/64-bit integer, FP16 and FP64
arrays through one root. Its SPIR-V contains 8-bit add, 16-bit multiply/add, 64-bit
bit operations and FP16/FP64 multiply/add. Both local drivers pass exact wraparound,
high-word, half-bit-pattern and double checks over two changed-input replays, with
root and surrounding guard bytes unchanged. Double inputs exceed FP32's exact integer
range, so accidental narrowing would be visible. Rust tests create devices with each
available scalar feature independently and reject the combined artifact until all its
declared requirements are enabled. A driver lacking that combined profile is reported
as not exercising the C numerical subtest, not as passing those arithmetic checks.

These are scalar capabilities, not a completed ML profile or throughput guarantees.
Matrix tuples, broader subgroup profiles, wider atomic profiles, floating-point controls, uniform/push narrow storage,
quantized consumer kernels and compiler-generated requirement metadata remain work.
This fixture does not prove signed arithmetic, exceptional values, denormal behavior,
all rounding modes or compiler/backend optimization quality. No timing gate is added.

64-bit integer atomics are now independently enabled for address-backed buffers and
workgroup memory (`BUFFER_ATOMIC64`, `SHARED_ATOMIC64`), alongside the artifact's
`INT64` requirement. The runtime does not add barriers or infer memory ordering.
A second compute kernel accumulates high-word values from two workgroups into one
buffer counter while independently reducing each workgroup through a shared 64-bit
counter and explicit shader barriers. Exact global/partial results, changed-seed
replay and untouched data/guards pass on both local drivers. SPIR-V inspection
confirms 64-bit atomic adds and workgroup control barriers, not a host reduction.
Device/artifact tests cover each atomic domain independently and reject a combined
artifact with either domain or INT64 missing. This does not establish all atomic
operations, signed/floating-point atomics, device-wide synchronization algorithms,
forward-progress properties or contention throughput.

## Implemented subgroup queries and stage controls

The subgroup query reports native default/min/max widths, compute workgroup subgroup
limits, exposed shader-stage support, operation groups and stages supporting requested
widths. This is not a promise that all devices have 32-lane or 64-lane waves, or that
invocation IDs map to lanes in a particular order. The stage mask currently describes
the implemented compute/vertex/fragment vocabulary; other stages remain future work.

Each shader can provide a subgroup record declaring operation groups, an exact width,
permission to vary width, and/or a full-subgroup requirement. Preparation checks native
support, explicit feature enabling and post-specialization workgroup dimensions. Exact
and varying widths are mutually exclusive. Full subgroups in the current stage profile
are compute-only; local X must be divisible by the selected/default width or, when
varying, the maximum width. Exact widths also constrain total workgroup invocations.
Graphics stages use the same per-stage translation, not a device-wide wave policy.
See [native stage constraints](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineShaderStageCreateInfo.html)
and [required width](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineShaderStageRequiredSubgroupSizeCreateInfo.html).

`SUBGROUP_EXTENDED_TYPES` independently enables narrow/wide subgroup operands; it does
not substitute for scalar arithmetic/storage enabling. No shader rewriting, padded
dispatch, extra shared-memory reduction or command-time specialization is introduced.
The artifact producer still proves declared operations/types, convergence and memory
semantics. This profile does not expose every later subgroup extension.

The C consumer verifies every invocation's integer reduction and exclusive prefix
against its returned active-lane ballot, including changed-seed replay and guards.
It checks native default width, default/full, each supported requested width fitting
the 64-thread fixture, and varying/full. Radeon passes requested 32 and 64; llvmpipe
passes 8. Ballots avoid assuming subgroup-to-workgroup lane assignment. Unit tests
cover unsupported operations/stages, invalid widths, disabled features, dimension
constraints and quad restrictions. Graphics-stage subgroup execution, extended scalar
operand types, divergent participation and other operation groups remain unverified;
this is not a throughput comparison. Source-only shader records grow to 56 bytes;
installed ABI 20 is unchanged.

## Implemented explicit executable caches

Preparation can borrow an independent cache, shared across compute and graphics.
The cache is neither an executable owner nor a device-global lookup table: callers
choose its lifetime, partitioning and persistence. Executables may outlive it; no
cache is touched during binding, recording, submission or execution. Native cache
contents are advisory, not a promise of a hit, bounded compilation time or a fully
portable executable. No OGPU disk I/O, eviction or implicit empty-cache retry exists.

Two synchronization strategies are explicit. Native synchronization permits concurrent
preparation against one cache. Caller synchronization requires enabled `CACHE_CONTROL`
and maps to Vulkan's externally synchronized cache bit, avoiding native cache locking.
Independent caches may be prepared concurrently. Export excludes mutation; merge
requires exclusive destination access and no source mutation. Merge may allocate a
temporary native-handle array on this cold path. Cache destruction excludes all use,
but needs no GPU wait. See [native cache creation](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineCacheCreateInfo.html)
and [merging](https://docs.vulkan.org/refpages/latest/refpages/source/vkMergePipelineCaches.html).

Import checks the little-endian native header's size/version, vendor/device IDs and
cache UUID before entering the driver. The remaining payload must still be intact
previously exported bytes; this is not a validator for arbitrary untrusted binaries.
Export writes directly into caller storage. Insufficient capacity returns `CAPACITY`
and the bytes actually written, potentially a valid partial cache; unlike array
queries it is not atomic and does not report the required capacity. A null-data
query obtains the maximum size again. This preserves [native export semantics](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetPipelineCacheData.html)
without allocating or copying an intermediate blob.

C consumers exercise native-synchronized compute caches and caller-synchronized
graphics caches where enabled: export/import/merge, short-output guards, incompatible
identity rejection, pipeline reconstruction and destroying caches before executing
the reconstructed pipelines. Rust tests additionally cover concurrent shared/independent
preparation, disabled-feature rejection, cross-device rejection, invalid native failure
outputs, retry after allocation/export failure and sticky synthetic device loss.
Preparation also accepts an explicit `COMPILE_FAIL_IF_REQUIRED` flag, requiring
enabled `CACHE_CONTROL`. It maps directly to the [native creation flag](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineCreateFlagBits2.html)
for compute and graphics, with or without an explicit cache. A native compilation
miss returns `COMPILE_REQUIRED` and no executable; it does not poison the device,
retry, schedule background compilation or defer compilation to recording. The
caller decides whether/when to retry with compilation allowed. Preparation can
still allocate, validate and synchronize; this is not a bounded-latency promise.
Unknown flags/reserved fields and disabled capability requests are rejected.

C consumers attempt the no-compile path before/after cache export/import and
explicitly retry only as application policy. They accept either native outcome;
an earlier preparation or imported cache does not guarantee reuse. Deterministic
Rust injection tests check native flag delivery, `COMPILE_REQUIRED` propagation,
null outputs, shader-module cleanup and successful subsequent ordinary preparation.
No cache-hit, persistent-driver-cache independence or compile-time performance claim
is made. Pipeline binaries and non-Vulkan artifacts remain separate work. The
source-only executable record is now 80 bytes; installed ABI 20 is unchanged.

## Implemented explicit queries

Independent timestamp, occlusion and pipeline-statistics pools own only their native query storage
and borrow the device. Reset, timestamp, begin/end and result-copy commands operate
on explicit slots; no automatic timer, completion object, host polling, CPU result
cache or resource-retention registry is created. Occlusion queries may live inside
one rendering scope or surround complete scopes. Opt-in `PRECISE_OCCLUSION` and a
per-begin `PRECISE` flag expose exact passing-sample counts; ordinary occlusion
keeps the native visibility-only guarantee. Timestamp period and per-queue valid bits are explicit metadata, not a
calibrated host clock or a guarantee of comparable clocks across arbitrary queues.

Results copy into caller COPY_DST spans with chosen 32/64-bit width, byte stride,
availability and optional explicit **GPU** wait. Timestamp partial results are
rejected. Reset/use/replay synchronization remains the caller's responsibility;
the same query slots cannot be reused concurrently without satisfying native rules.
Small encoder-local records check matching begin/end and scopes per native query type,
without retaining the pool or tracking every query's global execution state.
Occlusion and statistics can overlap, but a second active query of the same type
is invalid. In-scope queries must end before the scope; outside queries may enclose
complete scopes. See [native begin-query rules](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBeginQuery.html).
Output memory visibility, host wait and invalidation are explicit just as for other
GPU writes. See [native query result copies](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdCopyQueryPoolResults.html).

The C consumer now also checks padded 64-bit timestamp/availability records and
32-bit visible/empty occlusion records, serial replay/reset, enclosing and in-scope
queries, and guard bytes. It does not infer a performance comparison from timestamps.
Statistics require independently enabled `PIPELINE_STATISTICS` and a nonempty
queried counter mask. Counters use ascending mask-bit order, followed by one optional
availability word, all at the selected width; record size includes every selected
counter. Queue capabilities must cover each requested operation. Compute statistics
do not require RASTER, and querying geometry/tessellation counters does not enable
their shader stages. Counters expose native counting rules, not portable performance
predictions. This path adds no command-time allocation or automatic host readback.

Radeon and llvmpipe pass compute direct/indirect invocation counts, empty statistics,
ordered 32/64-bit graphics counters, simultaneous occlusion/statistics, precise
64/256-sample results, padding guards and serial replay. Negative checks cover
disabled enabling, zero masks, duplicate active types, wrong scope/index, resetting
or resolving active slots, short multi-counter destinations and invalid precise use.
The known llvmpipe combined-plane-copy caveat is unchanged. Performance-counter
profiles, finer timestamp stage vocabulary and calibrated clocks remain follow-up work.

## Verification so far

- 71 ordinary Rust tests pass, including foundation contract, status and
  cache-boundary/usage tests, plus the expanded C/Rust layout expectations.
- `gpu_foundation_setup` and `gpu_foundation_failures` pass on Radeon RX 5700 XT
  and llvmpipe with validation enabled. They cover exact multi-domain/multi-queue
  creation where available, optional features off/on, four concurrent host workers,
  timeline timeout/signal/poll, failed creation and sticky synthetic device loss.
- `cargo xtask foundation` builds/runs the public C example on both drivers. It
  includes C/Rust layout expectations and device survival after discovery teardown.
- The existing C compute example still passes on Radeon; ABI-20 checks pass
  (847 layout values). Clippy and C11/C++17 combined-header syntax checks pass.
- The public C example now also checks compatible host-visible placement, aligned
  addresses, subrange offsets, shared mapped views, cache calls, and remapping.
  `gpu_foundation_memory` adds invalid descriptions, requirements-capacity atomicity,
  raw opaque backing, concurrent sharing where available, and allocation-failure
  cleanup. Noncoherent native call parameters are also checked with injected calls
  on real backing; this is not evidence from a new noncoherent physical GPU.
  All twelve foundation GPU tests pass on each available driver. Pinned Vulkan
  bindings reproduce exactly after adding the requirements/property records.
- No-GPU C example/header checks are added to CI configuration; no hosted CI run
  is claimed.
- The C compute path passes on both drivers: named-entry preparation, specialization,
  released host artifact/metadata, two roots with independent rebinds, partial inline
  updates, GPU-written indirect dimensions, and three serial replays observing
  changed root data/counts. Results distinguish both roots and inline values;
  untouched output tails are checked. Invalid metadata, root alignment and dispatch
  before binding fail. The test initializes the full declared inline block before
  partial updates (including unused padding, as current validation expects).
- `gpu_foundation_executables` injects module and pipeline failures on real devices,
  covering ignored unspecified failed module outputs, module cleanup, valid pipeline
  output cleanup even when creation fails, and successful retry. These are synthetic
  failures, not evidence of actual OOM/device loss or measured overhead.
- The C descriptor path passes on Radeon and llvmpipe: batched buffer/image/sampler
  encoding, guard bytes, whole-batch local rejection, scratch capacity, explicit
  host-visible placement, both heap binds, host-gated pending unused-slot writes,
  reset-before-reservation-release and invalid-binding poisoning. The separate
  heap-execution fixture adds actual sampled-image/sampler/storage-image/buffer
  consumption, explicit LOCAL descriptor uploads, unchanged-list replay with a
  changed descriptor destination, pending disjoint sampler writes, and guarded
  readback on both drivers. `gpu_foundation_descriptors` adds four independent native host
  encoders and injected partial-write/OOM/sticky-loss checks; no real loss event
  or allocation-free driver implementation is claimed.
- The C command path passes copy/upload, explicit local working backing, fill,
  global/ranged dependencies, readback and three changed-input replays on both
  drivers. It also covers stable handles across reserve, capacity failure, sticky
  encoding failure, cancellation, one-shot rejection, two host-gated simultaneous
  pending executions, reset and trim. Radeon additionally passes exclusive
  ownership transfer from family 0 to family 1 with a timeline edge and readback;
  llvmpipe reports that it has no second transfer-capable family.
- `gpu_foundation_commands` injects allocation/begin/end/reset/submit failures on
  real devices, checks failed-output adoption, OOM retry without one-shot
  consumption, terminal-loss/unknown-error poisoning and no native retry after
  loss. Four host workers independently record/reset arenas. These tests do not
  claim a real GPU-loss event or measured concurrent GPU execution.
- The C image path passes on both drivers: two nonzero-offset images in one
  allocation, independently dedicated color/depth images, mip/layer sRGB views,
  explicit batched transitions, color/D32 clear/readback, pitched two-layer partial
  upload and readback, and untouched padding/guard bytes. Invalid placement,
  unbound views, rebinds, mip ranges, pitches and undefined destination states fail.
- `gpu_foundation_images` checks that requirements create/allocate no GPU objects,
  capacity failure preserves output arrays, and failed image/view/allocation
  outputs are not adopted. It covers bind failure/retry and dedicated-association
  mismatch. Native creation/view checks cover 1D arrays, 3D, cubes, multisample
  arrays and depth/stencil tuples. D24S8's tested tuple is reported UNSUPPORTED on
  Radeon and supported on llvmpipe; no fallback format is substituted.
- The C graphics path passes on Radeon and llvmpipe with synchronization validation:
  color/depth readback, all draw variants and changed-root/count replay. Invalid
  scope, extent, missing viewport/pipeline/index binding, index bounds, indirect
  stride and count ranges are diagnosed before native draws. The native graphics
  preparation test injects first/second-module failure and null/partial pipeline
  failure, checks exact cleanup and cleared outputs, then succeeds on the same device.
- Query result readback and negative range/type/stage/scope checks pass on both
  drivers. `gpu_foundation_queries` checks invalid descriptions, failed native-output
  adoption, successful destruction and sticky injected device loss. These are
  controlled failure checks, not a claim of actual device loss or hosted CI execution.

No timing campaign, GPU queue-overlap claim, real device-loss event, Metal support,
or new SDK support follows. Hardware execution requires sandbox-external GPU access;
the initial sandboxed Radeon discovery failed, then passed with that access.

## Next implementation work

Broader graphics and compute/argument profiles, richer query facilities and consumer
cutover as the coordinated tranche proceeds. Graphics is usable offscreen, not a
complete graphics contract or a completed M4 consumer. Presentation remains separate.
Pipeline binaries, set/binding-to-address mappings and generated interface integration
remain explicit work; the current compute slice does not close the whole foundation.
The existing setup policy in ABI 20 is temporary migration weight and should be
removed at consumer cutover, not maintained as a fallback backend. Native connection
loading and the modern-baseline predicate are shared; no old `Rc` device, internal
timeline, command cache or resource-retention policy is reused by the new handles.
