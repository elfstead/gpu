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

Explicit per-domain arenas reserve primary or secondary list slots, not fictitious native-byte
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

This is still an unstabilized subset.
Synchronization now exposes separate copy, clear/fill, blit, resolve, vertex shader,
index input, vertex fetch and early/late depth scopes. Explicit TRANSFER, VERTEX_INPUT,
DEPTH, PRE_RASTER and ALL_GRAPHICS aggregates remain available. Narrow shader sampled,
storage and uniform accesses coexist with broad shader and independent heap accesses.
Scopes lower exactly, without adding neighboring stages. In this source-only revision,
COPY no longer means all transfers and VERTEX no longer includes fixed-function input;
the consumers are updated accordingly. Shader artifact/root stage identities remain
COMPUTE/VERTEX/FRAGMENT; synchronization-only masks do not enable new shader stages.
Native [stage](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineStageFlagBits2.html)
and [access](https://docs.vulkan.org/refpages/latest/refpages/source/VkAccessFlagBits2.html)
vocabularies determine the mapping. Queue/feature checks reject unsupported scopes,
and access checks distinguish fetch from shading and transfer reads from clear writes.
Public-C coverage includes CLEAR-to-indirect, copy/fill/copy chains, standalone
clear/copy/resolve image chains, sampled/storage heap consumption, early/late depth,
vertex-stage timestamps and a CLEAR-signal/COPY-wait edge. Native blits also use
their separate scope; no performance equivalence claim follows from these checks.
Multiple lists and timeline edges are
batched in one native submit record, or an ordered array of distinct wait/work/signal
records can be submitted in one native call. None is flattened or silently split.
The batch API adds caller scratch for native headers, in addition to each record's
existing list/semaphore scratch; all regions must be mutually disjoint. Scratch
can be reused immediately after the call. A zero-count batch call is a no-op on a
live device. All records are translated/validated before touching the queue;
local failure submits nothing and consumes no one-shots. Native OOM also leaves
the entire call retryable, following [Vulkan submission failure rules](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueueSubmit2.html).
Only successful native submission consumes one-shot lists. Caller-owned timeline
ordering and simultaneous-list requirements apply across the whole batch array.

The C fixture checks producer/empty/consumer records, an inter-record timeline
edge and a host gate, GPU copy/readback, immediate scratch overwrite while pending,
local rejection in the last record, one-shot consumption and separate serial replay.
Rust injection checks one native call with the full batch count, no native call on
local rejection, OOM retry, successful one-shot consumption and terminal loss.
No performance
equivalence claim follows merely from the absence of OGPU hot-path allocation.

### Caller-owned split dependencies

Independent device-only split objects map to native events. They are initially
unsignaled, explicitly created/destroyed and never retained by command lists.
They borrow their device; the caller keeps events and referenced resources alive
through every recorded future and pending use. Endpoints are not tied to an arena:
release/acquire may be recorded in either CPU order, in different lists or
standalone secondaries. Commands require a graphics/compute domain and are outside
rendering. HOST scopes are invalid. Execution is on one native queue, with release
earlier in submission order; timeline edges, not these events, synchronize queues.

Release takes the complete dependency. Each acquire repeats exactly the same
dependency values and array ordering (not necessarily the same host pointers).
The caller guarantees matching endpoints; no hidden registry, resource retention,
dependency comparison walk or device-wide lock is added. Multiple acquires map to
one native wait with caller-owned scratch for events, native dependency headers and
global barriers, plus each dependency's separate range scratch. All descriptions
validate before issuing that call; a bad last record emits no partial wait. All
scratch and description arrays may be reused as soon as the call returns.

Reset takes an explicit source-stage mask. It is never inserted automatically and
does not implicitly order itself with future releases. The caller supplies the
necessary execution edges, including earlier acquires before reuse and reset before
release. The C fixture demonstrates an explicit ALL-to-ALL edge after prefix resets.
Replay mode does not repair shared-event cycles: simultaneous-capable recordings
still require correctly ordered event use. See native
[release](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetEvent2.html),
[wait](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdWaitEvents2.html)
and [reset](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdResetEvent2.html)
contracts. This replaces the source-only sketch's arena-local split-token proposal;
the installed ABI-20 recording-local experiment is unchanged pending migration.

Public C execution on both local drivers records the acquire list first, releases
two events around independent work in another arena, then uses one batched acquire
for a ranged buffer dependency and image transition/global dependency. Host-gated
submissions check copy/fill/image readback, guard bytes and immediate scratch reuse.
One-shot mode and three changed-input executions of both serial and simultaneous
recording modes pass. These executions are deliberately serial: no overlapping
shared-event execution, queue overlap or performance benefit is claimed. Rust native
injection checks one complete wait, distinct global-barrier storage, atomic rejection,
primary/standalone-secondary recording, failed creation output ownership and sticky
synthetic device loss. Secondary split GPU execution remains unverified.

### Secondary command lists

Arena level is fixed at creation; recording level must match. Standalone secondary
lists support compute/transfers and complete rendering scopes. Rendering-continuation
secondaries declare formats/sample count, then record inside a caller-owned primary
rendering scope. Attachment compatibility remains a caller proof, without retaining
attachment arrays or walking child resources. Primary scopes explicitly select
inline draws or secondary contents; mixed/nested secondary execution is not enabled.

An optional inheritance record declares occlusion and pipeline-statistics masks.
The independent `INHERITED_QUERIES` feature is required to execute under active
queries. The caller must not start conflicting query types inside a child. Radeon
executes inherited occlusion/statistics and checks their draw counts; llvmpipe does
not expose query inheritance. Secondaries may still own their own query scopes.

Execution translates the complete list array into caller-owned scratch and validates
it before one native call. It neither retains a child graph nor allocates OGPU memory.
Ordinary pipeline, arguments, viewport and vertex/index bindings become
undefined after execution; callers re-establish them. Render/query scopes survive.
Children do not inherit those bindings either. Resource and sampler heaps can be
inherited independently by naming exact heap-binding records at begin. A child
cannot rebind an inherited heap, but can bind the other heap normally. Callers prove
each inherited range/reservation matches the primary at every execution; no state
scan, retained binding description or inserted bind is added. A primary heap binding
survives only if every executed child inherits that heap. See native
[heap inheritance](https://docs.vulkan.org/refpages/latest/refpages/source/VkCommandBufferInheritanceDescriptorHeapInfoEXT.html)
and [state preservation](https://docs.vulkan.org/spec/latest/chapters/cmdbuffers.html).
Backing and native reserved-range obligations extend through both child and parent
future/pending uses. The C fixture checks neither/either/both heaps inherited,
parent dispatch after execution without rebinding inherited heaps, changed-descriptor
replay, pending disjoint sampler writes and rejection of inherited-heap rebinding.
Parent dispatch overwrites only half the child output using different sampled rows;
readback requires both executions, so a successful parent cannot hide a broken child.

Secondary lifetime and replay obligations are explicit: reset/destruction invalidates
referring primaries, a simultaneous parent needs simultaneous children, and callers
track indirect one-shot consumption and pending uses. Submission does not traverse
children. Direct secondary queue submission is rejected. See native
[execution rules](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdExecuteCommands.html)
and [rendering inheritance](https://docs.vulkan.org/refpages/latest/refpages/source/VkCommandBufferInheritanceRenderingInfo.html).

The C consumer executes secondary compute with changed-input replay, standalone
graphics, multisampled inherited rendering, stencil/vertex-fetch continuation and
inherited query counting. Rust tests additionally check atomic local rejection,
scratch capacity, one native execute call, level/mode rules and query masks. Arena,
inheritance and rendering-inheritance records are respectively 40, 64 and 72 bytes;
this remains source-only revision 1, not an installed ABI change.

## Implemented images and views

`QUERY_FORMATS` exposes immutable per-format optimal/linear tiling and formatted-buffer
capabilities on adapters and devices, including zero-support formats. It uses the
existing atomic capacity protocol and does not imply optional feature enabling.
An allocation-free `image_limits_query` reports extent/mip/layer/sample/resource-size
limits for an explicit optimal-image format/dimension/usage/flags profile. Output
is unchanged on failure; disabled multisampled storage remains excluded. Physical
capabilities are not a memory budget or a guarantee that combined maximum values
can be instantiated. Concrete descriptions still use `image_requirements` for exact
support, memory types/alignment and dedicated-placement rules. Native
[format properties](https://docs.vulkan.org/refpages/latest/refpages/source/VkFormatProperties.html)
and [image limits](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetPhysicalDeviceImageFormatProperties.html)
remain separate. The public C consumer compares adapter/device snapshots, checks
short-capacity atomicity and failure-output preservation, and discovers blit prerequisites
after discovery teardown instead of assuming format support.

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
usage subsets and component mappings. The current table covers eighteen common
uncompressed color/depth/stencil formats. 1D/2D/3D and array/cube views and ordinary
multisample images are implemented subject to exact native support. Optional
cube arrays, 2D views of 3D slices, multisampled storage, extended usage, compressed
and multi-planar formats, linear tiling, sparse/external/disjoint images and
separate depth/stencil layout enabling remain outside this implemented profile.
They are not ruled out by the design and are never silently substituted.

Native image blits expose independent source/destination endpoint pairs, mip/layer
selection, nearest/linear filtering, axis reversal and format conversion. No shader
fallback, temporary image, implicit barrier or automatic mip chain is introduced.
A graphics queue is necessary; raster feature enabling is not. Single samples,
format-class/filter support, bounds, aspect/layer matching and inactive-dimension
rules are checked before recording one native call. Degenerate source axes remain
legal; destination axes must be nonzero. The application proves non-overlap including
the filter footprint, not just the declared source region. See native
[blit semantics](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBlitImage.html).
Depth/stencil requires matching formats and nearest filtering; integer signedness
must match. Current 3D cross-dimensional blits require base layer zero and count one;
array/3D slice mapping, cubic filters and broader formats remain separate profiles.

The public C consumer checks layered RGBA-to-BGRA upscaling with reversal,
UNORM-to-float conversion, and explicit same-image linear downsampling with a pattern
that distinguishes linear from nearest. It runs three changed-input replays, checks
guards and rejects invalid endpoints/mips/layers/aspects/filter records. These paths
pass Radeon 26.2.4 and the llvmpipe 26.2.1 diagnostic. Native filter/rounding precision
is not a portable bitwise numerical guarantee. Depth/stencil, sRGB, 3D and degenerate
source GPU workloads remain unverified here; host tests cover endpoint/format rules.

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
Blend constants and stencil reference/compare-mask/write-mask may now each be
selected independently as dynamic. The immutable graphics-limits record reports
the supported bitmask; all combinations with required viewport/scissor are supported
in this Vulkan profile. Unsupported bits fail during preparation. Each setter maps
to one native command and copies no resource data, allocates nothing in OGPU and
does not generate or look up a pipeline variant. Stencil setters select front/back
faces independently and preserve full-width values. Blend constants are passed
unchanged, without clamping to the normalized range.

The caller owns initialization and native dynamic-state lifetime. Set-before-bind
is legal; binding a pipeline with a field static invalidates its earlier dynamic
value, while a dynamic bind preserves it. A setter after a static bind needs a
subsequent compatible bind before drawing. Executing secondary lists invalidates
ordinary state, with no automatic restoration. There is no new per-draw state walk
or shadow-state cache. These rules follow native
[dynamic state lifetime](https://docs.vulkan.org/guide/latest/dynamic_state.html),
[blend constants](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetBlendConstants.html)
and [stencil reference](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetStencilReference.html).
Initialization of newly exposed fields is a trusted caller obligation, not a promise
that OGPU's existing inexpensive draw checks diagnose every misuse.

The C consumer tests each optional bit independently and all together in a rendering
secondary, ignores deliberately wrong static values, restores after a static bind,
changes reference/masks between draws and checks quarter/half-strength blended
stripes. Dynamic reference eliminates a reference-only pipeline variant. Exact
pixels, changed-input replay and guards pass on Radeon and llvmpipe (the existing
stencil-copy diagnostic exclusion still applies there). Native preparation injection
checks all sixteen combinations and rejects unknown/missing-required bits before a
native creation call. Nonfinite blend execution and genuinely different front/back
face values are not verified by this fixture.

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
vertex with optional fragment (vertex-only for static discard), optional D16/D32 or combined
D24S8/D32S8 depth/stencil, explicit supported sample counts and static raster/sample
controls, vertex pulling or native vertex fetch, one viewport/scissor and optional
independent per-color blend state.
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

Dynamic stencil operations/test enabling, depth/stencil resolves, custom sample positions,
dynamic vertex input/stride and additional
stages, list restart, richer dynamic state and tile-local dependencies
remain work. Unsupported requested state
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
The source-only graphics limits/state records are now 96/176 bytes; ABI 20 is unchanged.

### Static raster and sample controls

The optional 96-byte `RASTER_OPTIONS` record exposes strip primitive restart,
depth clamp, raster discard, fill/line/point polygon modes, depth bias and its clamp,
line width, depth bounds, a 64-bit sample mask, minimum sample shading, alpha coverage
and alpha-to-one. Null selects the previous defaults. Non-null records are explicit:
zero width is invalid and zero mask suppresses coverage; no hidden default rewriting.
Seven independent feature bits map to native enabling; RASTER does not enable them.
Line-width range/granularity are immutable reported limits. State is prepared once,
with no command-time state cache, shader transformation or extra draw calls.

Discard takes one vertex shader, not an unused fragment artifact, and zero
color/depth formats. Static discard has no native fragment-output interface;
attached execution is available with the independent unused-attachments feature
described below. No implicit extra render scope is introduced.
Depth clamp also disables Z clipping in this profile; independent clipping remains
work. Sample shading expresses a minimum fraction, not an exact invocation count.
Alpha coverage is native, not a portable bit-exact coverage pattern. See native
[rasterization state](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineRasterizationStateCreateInfo.html),
[multisample state](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineMultisampleStateCreateInfo.html)
and [depth/bounds state](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineDepthStencilStateCreateInfo.html).

Ordinary tests cover malformed fields, independent feature rejection and strip/list
restart separation. Native preparation tests enable each feature separately and
reject the others. The C consumer checks explicit/default sample masks, zero masks,
ignored high bits for single-sample targets, zero-alpha coverage, alpha-to-one,
four-sample partial-mask resolves and full sample shading with changed-input replay
and guarded readback. Attachmentless primary/secondary draws separately verify that
discard preserves vertex storage writes while suppressing fragment writes.
Pixel execution of non-solid fill/wide lines and strip restart,
exact sample-rate invocation behavior and 64-sample execution remain
verification work; pipeline creation alone is not evidence for those outputs.
The above C cases and all 14 foundation GPU tests pass with validation on Radeon
26.2.4 and llvmpipe 26.2.1 (the latter retains the documented stencil-copy exclusion).
The ordinary suite now has 80 passing tests. No timing or Metal claim follows.

### Vertex-only depth rendering

The fragment shader is optional independently of raster discard. A vertex-only
pipeline still rasterizes and performs fixed depth/stencil operations; OGPU creates
no placeholder fragment artifact and records no additional commands. Present stages
share one argument interface, and the executable reports only its actual stage mask.
Color outputs without a fragment shader are undefined, not implicitly zero: callers
disable color writes/alpha coverage or use depth-only scopes. This follows the native
[graphics pipeline contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkGraphicsPipelineCreateInfo.html).

The new C depth-only consumer uses one vertex artifact, zero color attachments and
a placed D32 target. Guarded readback checks near/far depth rejection; out-of-range
Z clipping versus clamping; depth bounds using **stored** depth on opposite halves;
positive and negative clamped constant bias; and nonzero unclamped bias. Changed
roots replay the same list, including swapping which half passes depth bounds.
Clamped outputs use exactly representable values; the unclamped case checks the
direction and a small range without imposing a cross-device bias unit.
All seven modes pass under validation on Radeon 26.2.4 and llvmpipe 26.2.1, together
with the 14 foundation GPU tests. No fragment module is loaded by this consumer.
Slope bias, arbitrary depth formats and vertex-only multisample/stencil execution
remain verification work; existing two-stage consumers still cover multisample/stencil.

`SHADER_OUTPUT_LAYER` independently enables vertex-stage `Layer` output (Slang
`SV_RenderTargetArrayIndex`) using the existing image/view/render layer records.
The native [Vulkan 1.2 feature](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceVulkan12Features.html)
does not imply multiview or additional shader stages. Layer indices are relative
to the attachment view, bounded by the render scope and consistent per primitive;
these remain caller shader contracts. OGPU adds no per-layer draw loop, duplicated
view, index readback or hidden transition.

An eighth depth-only case renders through a two-layer view starting at layer one
of a three-layer D32 image. The shader selects opposite layers for two scissored
draws; replay changes depths and swaps the selected layers. Readback checks every
pixel, render clears in unselected halves, the unchanged layer outside the view
and host guards. This and actual Layer-artifact preparation (including feature-off
rejection before native pipeline creation) pass on Radeon 26.2.4 and llvmpipe
26.2.1 with validation. Color, multisample and cube-array layered execution remain
verification work; this does not claim multiview support.

### Explicit depth/stencil attachment resolves

The existing attachment records now accept depth/stencil resolve targets and modes.
Graphics limits report per-aspect mode masks plus independent-mode and independent-
NONE support; this source-only limits record grows to 112 bytes. No new feature bit,
owned resolve object or host command is needed. `SAMPLE_ZERO`, `AVERAGE`, `MIN` and
`MAX` map to native modes; support is queried, not emulated. Paired depth/stencil
resolves obey the native independence limits and share their destination view/state.
Absent aspects do not force paired mode selection. Targets have one sample,
matching format and explicit attachment usage/state. Source discard still permits
resolved output; all placement, transitions and lifetimes remain caller-owned.
See the native [resolve properties](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceDepthStencilResolveProperties.html)
and [render-scope constraints](https://docs.vulkan.org/refpages/latest/refpages/source/VkRenderingInfo.html).

The C depth consumer writes different values into samples 0/1 and 2/3 using explicit
sample-mask pipelines, then discards source storage. D32 readback checks all four
depth modes, with changed-depth replay swapping the minimum/maximum source. Combined
D32S8 targets separately resolve stencil sample-zero/min/max: a subsequent read-only
render pass tests equality to each possible value, with occlusion queries checking
64 passing samples and zero for the other value. This observes resolved stencil
without combined-plane host copies. The latter remains a distinct existing llvmpipe
driver problem, not a resolve fallback. Guards and root preservation are checked.

All seven new execution cases pass under validation on Radeon 26.2.4. llvmpipe
26.2.1 advertises only sample-zero and passes that depth/stencil execution; unsupported
modes are reported as not exercised. A 25-pair record-only matrix checks mode support,
independence rules and native acceptance; malformed modes, absent/multisampled
destinations, read-only targets and mismatched paired target views/states reject.
Synthetic unit coverage also checks all independence policies, not just local ones.
All 14 foundation GPU tests, 81 ordinary tests, Clippy, generated shader/binding
reproduction and the 851-value native/installed ABI checks pass. Installed ABI 20
remains unchanged; these are source-only records.

The consumer explicitly orders attachment resolve accesses at `COLOR` with
`COLOR_READ`/`COLOR_WRITE`, including depth/stencil targets; these are not standalone
`RESOLVE` transfer operations or `DEPTH` accesses. Synchronization validation caught
the initial depth-only target barriers, which were corrected in caller code without
adding runtime barriers. Layered resolves, alternative depth formats, mismatched
depth/stencil mode execution and resolved-depth shader sampling remain verification
work. No timing, Metal or portable throughput claim follows.

### Unused attachments and sparse output interfaces

`UNUSED_ATTACHMENTS` explicitly enables native
[`VK_EXT_dynamic_rendering_unused_attachments`](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceDynamicRenderingUnusedAttachmentsFeaturesEXT.html).
Pipeline and render-scope color counts may differ; absent/null attachments and
undefined output formats no longer force artificial pipeline variants. Defined,
non-null pairs still match formats; sample counts, synchronization and lifetimes
remain caller contracts. Color format zero now describes an unused pipeline slot,
without a format-support query. That description itself needs no optional feature;
using it with non-null attachments needs the feature. If a shader writes such a
slot while its view is non-null, the caller must set its write mask to zero. OGPU
does not reflect shaders or silently rewrite masks. Native
[draw compatibility](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdDraw.html)
still applies; this is not blanket permission to ignore formats.

The C cases cover vertex-only discard inside a two-target scope, a masked undefined
output with a real view, a defined output with a null view, and differing output/
attachment counts in both directions. Clears, explicit resolves, unchanged targets,
changed-input replay and guards are read back. Unused shader outputs can emit native
validation warnings; validation errors still fail the runner. Static discard inside
an attached scope is currently single-sample only: native static discard omits its
multisample state. Multisample attached discard and depth/stencil compatibility
execution coverage remain work, not silent scope rewriting or emulation.
All five new execution cases and the expanded feature-enabling preparation matrix
pass on Radeon 26.2.4 and llvmpipe 26.2.1 under validation. Generated bindings
reproduce; C/Rust ABI checks cover 851 layout values, with installed ABI 20 unchanged.

### Independent targets and indirect base instances

`DUAL_SOURCE_BLEND` independently enables the four secondary-source blend factors
in any source/destination color/alpha factor field. The second value is source
index one at the same output location, not another target. The compiler artifact
provides the interface; OGPU creates no temporary attachment and performs no shader
rewriting or extra draw. Graphics limits report `max_dual_source_attachments` in the
previously reserved word (the later resolve fields grow the record to 112 bytes). That is a bound on all
statically used fragment output locations when dual-source blending is active, not
a blanket bound on the pipeline's declared attachment array. Output interfaces and
their static-use limit remain caller artifact contracts, without a reflection scan.
See [native blend factors](https://docs.vulkan.org/refpages/latest/refpages/source/VkBlendFactor.html)
and Slang's [location/index attributes](https://docs.shader-slang.org/en/latest/external/slang/docs/user-guide/a2-01-spirv-target-specific.html).

The dedicated C consumer prepares all sixteen factor/field combinations using a
Slang fragment artifact with both outputs at location zero, source indices zero
and one. Two draws blend into RGBA32_FLOAT; binary-fraction inputs permit exact
readback including values above one, separate color/alpha factors and dependence
on existing destination contents. The same list replays changed primary and
secondary values with guards. Feature-off/invalid-factor cases reject before native
preparation. These cases and all 14 foundation GPU tests pass on Radeon 26.2.4 and
llvmpipe 26.2.1 (with the existing stencil-copy exclusion); the new shader reproduces
with pinned Slang and validates. Multiple dual-source output locations, normalized
target rounding and dual-source multisample execution remain verification work.

`LOGIC_OP` independently enables all sixteen native color logic operations. The
graphics state carries explicit enable/op fields; one operation applies across
targets, before each target's channel mask. It operates on converted attachment
components, not shader-float bit patterns. Native unsupported color formats pass
source values through; no shader emulation or substitute formats are introduced.
See the [native logic operation semantics](https://docs.vulkan.org/refpages/latest/refpages/source/VkLogicOp.html).

The C consumer checks every operation on two RGBA8 targets, distinct per-draw source
values, different channel masks, four-sample resolve, two changed-input replays and
guards. Feature-off preparation fails before reaching Vulkan; feature-on preparation
captures the exact native operation and preserves explicitly enabled blend state.

Known driver discrepancy: with both `logicOpEnable=1` and attachment `blendEnable=1`,
RADV 26.2.4 on this RX 5700 XT blends instead of honoring logic precedence. For CLEAR,
the updated two-source fixture reads `c0ff60a0` instead of zero. The same case and
all operations pass on llvmpipe 26.2.1. Both drivers pass all operations with the
consumer explicitly setting blendEnable=0. The normal fixture uses that explicit
state; `OGPU_FOUNDATION_LOGIC_BLEND_BOTH=1 cargo xtask foundation` retains the failing
Radeon diagnostic (and passing llvmpipe coverage). OGPU does not rewrite blend state,
blacklist the device or claim this native combination has been fixed. Integer,
sRGB and unsupported-format pass-through execution remain verification work.

`INDEPENDENT_BLEND` permits different blend factors/operations/enabling and channel
write masks for each color target. Without it, differing native attachment state
still fails preparation; no state is normalized or substituted. The existing color
array and commands are sufficient. A two-target consumer adds color twice in one
target while replacing only red/blue channels in the other, verifying preserved
green/alpha clear values, every pixel, changed-input replay and guard bytes.

`INDIRECT_FIRST_INSTANCE` permits nonzero base instances in all indirect draw
forms, independently of direct draw support. Indirect arguments remain GPU-resident:
no CPU read, rewrite or draw-time scan is added. The caller guarantees valid records
and enabled-feature requirements at execution time. The vertex-fetch consumer now
uses GPU fills to write nonzero first-instance fields, with an explicit CLEAR-to-INDIRECT
dependency. Instance zero contains depth-rejecting data; instance one produces the
expected output. Indexed/nonindexed and counted/uncounted variants pass changed-data
replay and final argument readback. These controls map to native
[independent blend and indirect first instance](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceFeatures.html).

Both consumers pass on Radeon and llvmpipe; no independent-blend throughput claim
or GPU-generated arbitrary draw stream is inferred. Rust tests enable the bits
independently, check declared requirements, and reject distinct color write masks
when independent blend is disabled.

## Implemented graphics-stage storage writes

`VERTEX_STORES_ATOMICS` and `FRAGMENT_STORES_ATOMICS` independently expose native
storage writes and atomics in those shader stages. Neither implicitly enables RASTER,
new shader stages, an image format or 64-bit/floating-point atomic operations.
Artifact requirements declare the needed bits and preparation rejects unavailable or
disabled requirements before shader creation. There is no shader scan, translation,
resource list or extra host command for an operation expressed in device code.
These map to the native
[per-stage storage features](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceFeatures.html).

The public consumer draws without color/depth attachments, writing through a GPU
address root. Separate vertex-only, fragment-only and combined artifacts plus a
standalone secondary test cover vertex atomic OR, fragment ordinary stores and an
atomic sum, explicit shader-to-host visibility, three changed-seed replays and guards.
Both local drivers pass. Vertex checks deliberately use idempotent atomics instead
of assuming exactly one invocation per logical vertex. The one-sample, single-triangle
fragment test has one contributing invocation per pixel; it makes no broader promise
about helpers, overdraw or early/late tests. Rust tests enable neither/either/both
features on independent devices and reject disabled declared artifact requirements.
Storage-image writes in graphics, graphics 64-bit atomics, multisample side effects
and cross-draw shader-read dependencies remain unverified by this fixture.

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
Broader matrix/subgroup profiles, wider atomic profiles, floating-point controls, uniform/push narrow storage,
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

### Native floating-point atomics

FP32 buffer and workgroup atomics now have independent operation bits: atomic
load/store/exchange, add, and min/max. Requesting one does not enable the others;
graphics storage-write enabling remains separate. The Vulkan mapping uses
[shader atomic float](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceShaderAtomicFloatFeaturesEXT.html)
and [atomic float min/max](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceShaderAtomicFloat2FeaturesEXT.html)
features and their extension dependencies. No host command, CAS loop, shader
rewriting, extra barrier or reduction-order policy is introduced. Native artifacts
declare exact operation/domain requirements; numerical and progress guarantees
are not inferred from general floating-point control properties.

Six pinned Slang artifacts exercise global addition across two workgroups, shared
addition, buffer/shared min/max and buffer/shared exchange including previous
values. SPIR-V inspection confirms native floating add/min/max/exchange instructions.
Finite binary-fraction inputs make addition exact regardless of invocation order;
shared exchange checks exactly one first exchanger without assuming which lane wins.
Guarded readback, root preservation and changed-input replay pass on llvmpipe 26.2.1
for all six cases. Radeon 26.2.4 passes five: buffer FP32 addition is not advertised
and is explicitly reported as not exercised. The runtime does not emulate it.

The preparation matrix requests each operation independently, tries the other
floating-atomic feature bits, and rejects missing artifact requirements. Unit tests
check all 64 enabling subsets and ensure wider/image/sparse features remain off.
All 15 foundation GPU tests pass on both drivers; the ordinary suite has 82 passing
tests. C/Rust native layout checks now include both atomic feature structures.
Atomic load/store execution, image/sparse and 16/64-bit operations, exceptional
values, denormals, contention throughput and device-wide progress remain work.
These results do not establish a completed ML numerical profile or new Metal support.

The same six independent operation/domain bits are now implemented for FP64.
They do not implicitly enable `FLOAT64`; shader artifacts declare that type
requirement separately. Native extension selection is shared with the FP32 path,
without enabling unrelated operations. The ordinary mapping test covers all 4096
FP32/FP64 combinations and keeps FP16/image/sparse features disabled.

Six further pinned artifacts use actual double-precision atomic operands, checked
in disassembly and SPIR-V validation. The C consumer shares its source with FP32
but uses 64-bit storage and a `2^-30` bias component that FP32 would lose. On Radeon
26.2.4, buffer/shared FP64 min/max and exchange pass exact results, previous-value
checks and changed-input replay. Neither buffer nor shared FP64 addition is
advertised by this GPU. llvmpipe 26.2.1 advertises none of these FP64 atomic profiles;
the consumer reports each as not exercised. Thus FP64 addition is **implemented
and artifact-validated, but not GPU-executed locally**, not an inferred success.
Preparation checks still exercise disabled and partially enabled type/operation
profiles and preserve early requirement rejection. Both full foundation regressions
pass within the documented driver limits, with 15 GPU tests each and 82 ordinary
tests; generated shaders and bindings reproduce, Clippy and 881 ABI values pass.
Image atomics, FP64-add execution, atomic load/store execution and exceptional
floating-point behavior remain separate verification/implementation work.

FP16 now also exposes six independent buffer/workgroup operation bits, with
`FLOAT16` arithmetic declared separately. Every FP16 operation selects the native
atomic-float-2 feature chain, not just min/max; enabling leaves unrelated widths,
image/sparse operations and vector-atomic extensions disabled. The mapping unit test
now checks all 262,144 combinations across the 18 operation/domain/width bits.

Six pinned half artifacts validate with native half-typed atomic add, min/max and
exchange instructions. The shared C consumer now takes an explicit 16/32/64-bit
width. FP16 addition uses bounded quarter-unit terms so every possible partial sum
is exactly representable regardless of invocation order; it does not reuse FP32's
larger, rounding-sensitive accumulation. Host conversion rejects non-exact fixture
constants and separately checks finite values, subnormals, infinities and NaN decoding
even when the GPU profile is absent. This is fixture code, not runtime tensor casting.

Neither Radeon 26.2.4 nor llvmpipe 26.2.1 advertises these six FP16 profiles. Therefore
FP16 atomics are **implemented and shader-validated, but not GPU-executed locally**;
preparation checks cover disabled/partial type requirements, not positive FP16
pipeline acceptance. No CAS or FP32 atomic fallback is substituted. Existing FP32/
FP64 execution and the full regressions still pass with the documented llvmpipe
stencil-copy exclusion. Half exceptional arithmetic, arbitrary reduction order,
positive feature-enabled pipeline creation and hardware execution remain unverified.

### Native cooperative-matrix tuples and execution

`QUERY_MATRIX_TUPLES` returns exact native M/N/K, A/B/C/result component types,
saturating accumulation, scope and stage tuples. The current profile covers KHR
subgroup-scope compute and its core component types; it does not combine independent
maxima into invented combinations. Results are snapshotted at discovery, preserve
native enumeration order and use the existing allocation-free count/capacity protocol.
Device queries return the same physical tuples regardless of feature enabling.
Native enumeration initializes every output structure, retries bounded `INCOMPLETE`,
checks returned counts and propagates allocation/native failures.

`COOPERATIVE_MATRIX`, `VULKAN_MEMORY_MODEL` and scalar arithmetic bits are independent
opt-ins. `VULKAN_MEMORY_MODEL_DEVICE_SCOPE` additionally requires the base memory-model
bit; incomplete device requests fail locally. This separation matters even to older
artifacts: Vulkan's [Device-scope rule](https://docs.vulkan.org/spec/latest/appendices/spirvenv.html#VUID-RuntimeSpirv-vulkanMemoryModel-06265)
forbids Device memory scope in **all shaders** when the memory model is enabled without
Device scope. The combined consumer explicitly requests both because its existing
atomic fixtures use Device scope. Neither availability/visibility chains nor robust
matrix-buffer access are implicitly enabled. Shader memory order, convergence,
tuple selection, layout/stride, address bounds and feature metadata remain caller/
compiler obligations; there is no shader reflection or inserted barrier.

No host matrix command, tensor allocation, workspace, launch policy or shader fallback
is needed. A pinned Slang artifact uses native cooperative loads, multiply-add and
stores through ordinary caller-owned memory, a device root and compute dispatch.
The fixture selects an advertised 8x8x8 FP16 A/B, FP32 C/result tuple, specializes its
workgroup size to one explicitly requested full subgroup, and launches two workgroups.
Asymmetric row-major A, column-major B and nonzero C detect layout/accumulation errors.
All 128 output elements match an exact CPU oracle over two changed-input replays;
input/root preservation and surrounding guards are checked after explicit visibility
and timeline completion. The 8x8x8 shape is a fixture choice, not an API restriction.

llvmpipe 26.2.1 exposes four tuples and passes this execution test. Radeon 26.2.4
exposes none; optional function loading is avoided, unsupported feature enabling
is rejected, and execution is explicitly reported as not exercised. This establishes
software-driver execution, **not hardware matrix acceleration or throughput**.
Other shapes/types, integer saturation, exceptional numerical values, divergent
control flow and newer workgroup/flexible-dimension profiles are not verified here.

All 15 foundation GPU tests pass on both local drivers. Full C regressions pass on
Radeon and on llvmpipe with the existing documented combined-stencil-copy exclusion.
The ordinary suite has 85 passing tests; Clippy, strict C/C++ compilation, 902 native
ABI values and shader/binding reproduction pass. Installed ABI 20 and Metal are
unchanged; compiler-generated requirements and consumer migration remain outstanding.

### Integer dot products for quantized kernels

`INTEGER_DOT` explicitly enables native signed/unsigned/mixed integer dot and
accumulating-saturating instructions. It does not bundle arithmetic/storage widths:
packed 4x8 consumes ordinary 32-bit words without INT8 or STORAGE8, while artifacts
using other scalar types declare those independently. This follows the native
[feature contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceShaderIntegerDotProductFeatures.html).

`QUERY_INTEGER_DOT_INFO` returns two immutable physical acceleration masks: ordinary
dot and accumulating-saturating dot. Each keeps unsigned/signed/mixed and unpacked
8/16/32/64-bit versus packed 4x8 profiles distinct. These are
[native acceleration hints](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceShaderIntegerDotProductProperties.html),
not support masks, algorithm choices or measured throughput promises. Zero does not
forbid supported instructions. Adapter/device queries agree independently of enabling;
no shader rewrite, packing pass, quantization allocator or workspace is introduced.

The pinned Slang fixture emits `OpSDot` and `OpUDot` with packed-4x8 operands, then
ordinary 32-bit accumulation. The C consumer compares 64 pairs of signed/unsigned
results with a scalar byte-extraction oracle, including negative bytes and wrapping
accumulators, over two changed-input replays. It checks input preservation and guards
using the existing numerical fixture's explicit memory, roots, barriers and timeline.
Both Radeon 26.2.4 and llvmpipe 26.2.1 pass, although both acceleration masks are zero.
This is native instruction execution via the driver, **not dedicated dot hardware
or accelerated throughput evidence**. The compiler emits no manual multiply/add
replacement in these artifacts; driver instruction lowering remains native policy.

Unit mapping tests isolate all 30 acceleration properties. Preparation tests accept
the artifact on a device enabling only INTEGER_DOT and reject its declared requirement
when disabled. The ordinary suite has 86 passing tests; all 16 foundation GPU tests
pass on both drivers. Full C passes with the same documented llvmpipe stencil-copy
exclusion; Clippy, strict headers, 935 ABI values and shader/binding reproduction pass.
Mixed-signedness, accumulating saturation and wider/unpacked execution remain
unverified, as do any quantized-model accuracy/performance claims. Installed ABI 20,
Metal and consumer/compiler migration are unchanged.

### Floating-point policy discovery

`QUERY_FLOAT_CONTROLS` returns one immutable physical record for binary16/32/64
preservation, denormal and rounding-mode support, plus separate denormal/rounding
independence rules. Adapter and device results match regardless of optional type
enabling. The mapping follows native
[float-control properties](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceFloatControlsProperties.html)
and [width independence](https://docs.vulkan.org/refpages/latest/refpages/source/VkShaderFloatControlsIndependence.html).
Support is not selection: the compiler's artifact must request legal modes, and
the caller must enable arithmetic types separately. No default preservation,
precision widening, fast-math choice, emulation or throughput promise is inferred.
Instruction-specific exceptions and precision rules still apply; this is not a
blanket IEEE-754, transcendental accuracy, NaN-payload or reproducibility guarantee.

Host tests isolate all fifteen native mode/width fields and preserve independence
values. Both local drivers pass adapter/device equality, count-only and failure-output
checks; GPU setup tests compare results with optional features disabled and enabled.
Radeon 26.2.4 reports all five modes for all three widths, with 32-bit-only policy
independence (16/64 are coupled). llvmpipe 26.2.1 reports signed-zero/Inf/NaN preservation
and nearest-even rounding, with all-width independence. Its missing explicit denormal
mode bits do not establish an unspecified artifact's actual denormal behavior.
This tranche is discovery only: numerical execution-mode litmus tests, artifact
mode requirement/reflection validation and float-controls-2 remain work. There is
no command-time floating-point policy setter.

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
profiles and calibrated clocks remain follow-up work; timestamps use the expanded
stage vocabulary when the requested scope lowers to one supported native stage bit.

## Verification so far

- 79 ordinary Rust tests pass, including foundation contract, status and
  cache-boundary/usage tests, plus the expanded C/Rust layout expectations.
- `gpu_foundation_setup` and `gpu_foundation_failures` pass on Radeon RX 5700 XT
  and llvmpipe with validation enabled. They cover exact multi-domain/multi-queue
  creation where available, optional features off/on, four concurrent host workers,
  timeline timeout/signal/poll, failed creation and sticky synthetic device loss.
- `cargo xtask foundation` passes on Radeon (currently Mesa 26.2.4); llvmpipe
  26.2.1 passes with the documented stencil-plane-copy diagnostic exclusion. It
  includes C/Rust layout expectations and device survival after discovery teardown.
  On October 10, newly installed llvmpipe 26.2.4 instead stops earlier: native
  pipeline-cache export returns OK with 20 bytes after reporting a 32-byte required
  size, too short for the required version-one header. The C fixture reports this
  failure; no cache workaround or successful full run is claimed for that driver.
- The existing C compute example still passes on Radeon; ABI-20 checks pass
  (847 layout values). Clippy and C11/C++17 combined-header syntax checks pass.
- The public C example now also checks compatible host-visible placement, aligned
  addresses, subrange offsets, shared mapped views, cache calls, and remapping.
  `gpu_foundation_memory` adds invalid descriptions, requirements-capacity atomicity,
  raw opaque backing, concurrent sharing where available, and allocation-failure
  cleanup. Noncoherent native call parameters are also checked with injected calls
  on real backing; this is not evidence from a new noncoherent physical GPU.
  All fourteen foundation GPU tests pass on Radeon 26.2.4 and llvmpipe 26.2.1. Pinned Vulkan
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
