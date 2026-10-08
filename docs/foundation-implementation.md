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
These bits describe native enabling, **not implemented new draw/dispatch commands**.
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
Image-to-image copies, resolves, image ownership/
alias execution coverage, and shader sampling/rendering of these new views remain
to integrate. Creation support is not a claim of full shader-consumer verification.

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
unspecified failed shader-module outputs, which are never adopted. Native cache
import/export, additional artifact formats, set/binding mappings, graphics stages
and richer numerical/launch requirements remain unimplemented, not silently
substituted. See [native compute preparation](https://docs.vulkan.org/refpages/latest/refpages/source/VkComputePipelineCreateInfo.html).

Argument metadata names a byte footprint and any number of nonoverlapping root
slots with explicit offsets, stages and pointee alignments. The implemented compute
profile exposes **one shared byte namespace**, not fictitious independent native
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
reproduces both using Slang 2026.14.1 and validates SPIR-V. This is an explicit test
artifact ABI, not yet integration with the generated consumer metadata workflow.

## Verification so far

- 61 ordinary Rust tests pass, including foundation contract, status and
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
  All seven foundation GPU tests pass on each available driver. Pinned Vulkan
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

No timing campaign, GPU queue-overlap claim, real device-loss event, Metal support,
or new SDK support follows. Hardware execution requires sandbox-external GPU access;
the initial sandboxed Radeon discovery failed, then passed with that access.

## Next implementation work

Graphics preparation/rendering and broader compute/argument profiles, then consumers
as the coordinated tranche proceeds.
Cache control, set/binding-to-address mappings and generated interface integration
remain explicit work; the current compute slice does not close the whole foundation.
The existing setup policy in ABI 20 is temporary migration weight and should be
removed at consumer cutover, not maintained as a fallback backend. Native connection
loading and the modern-baseline predicate are shared; no old `Rc` device, internal
timeline, command cache or resource-retention policy is reused by the new handles.
