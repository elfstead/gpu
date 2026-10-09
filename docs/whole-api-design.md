# Whole-API design: an explicit foundation, not a workload runtime

Selected design direction, 2026-10-08. **Target design, not a support declaration.**
The installed interface remains ABI 20. The companion
[C surface sketch](drafts/ogpu_next.h) is intentionally not installed; only its
imported source-header subset is implemented and linkable.
This document supersedes the experiment-by-experiment *design sequence*, not the
historical results or their evidence limits. The [working plan](plan.md) owns status.
Implementation now covers the [setup, backing, commands, images, descriptors, compute and offscreen raster boundaries](foundation-implementation.md);
only declarations in `include/ogpu_next.h` are implemented on Linux. The combined
sketch imports them and adds the remaining proposal; the installed ABI is unchanged.

## Decision in brief

Design the complete foundation now and implement it as one coordinated revision.
Do not require a timing campaign before exposing a clearly useful native strategy.
Use measurements to investigate uncertain mappings and validate the implementation,
not to discover that forced copies, global locks or forced waits restrict callers.

Keep the original idea: memory, GPU pointers, executable code, explicit commands
and dependencies; images and rasterization are first-class special hardware.
Keep Rust for implementation, a C ABI for hosts, and generated Slang interfaces.
Remove mandatory ownership/scheduling conveniences from the fundamental layer.
Make graphics and ML different uses of the same foundation, not separate runtimes.

“Minimal” means few independent concepts and no unnecessary policy, **not the
fewest functions**. “Anything” means no application-domain assumptions and an
extensible path to native hardware facilities. It cannot mean every device
supports every operation, or that every native API has equivalent facilities.
Unsupported strategies must be visible, not silently emulated at a performance cost.

## 1. What the native APIs teach us

This is an architectural comparison, not a measured speed ranking. In particular,
runtime versus driver API does not determine kernel performance by itself.

| API | Boundary and usual reason to choose it | Lesson for OGPU |
|---|---|---|
| Vulkan | Explicit resources, memory, command recording, queues and synchronization; broad graphics/compute control across vendors. Large state/feature surface and substantial application responsibility. | Preserve strategy control without reproducing legacy binding machinery or every backend object. |
| Metal, especially Metal 4 | Apple-native graphics/compute with tightly integrated tools and shader compilation. Metal 4 exposes explicit command allocation, dependencies and residency, alongside higher-level facilities. | A pleasant object API can still be low-level; memory residency, tile rendering and compilation boundaries cannot be wished away. |
| CUDA Driver API (`libcuda`, `cu*`) | Explicit contexts, modules and execution control; useful to language runtimes, loaders and systems integrating independently generated code. NVIDIA compute, not a general raster API. | Code loading and execution are distinct from a source language or operator library. |
| CUDA Runtime API (`libcudart`, `cuda*`) | Convenient CUDA language integration and implicit management of contexts/modules. Also exposes advanced streams, graphs and memory facilities. | Convenience need not imply slower GPU work. The distinction is which policies are implicit, shared or unavoidable. |
| Direct3D 12 | Explicit command lists/queues, descriptor heaps, memory/residency and resource lifetime; attractive for Windows graphics, tooling and ecosystem integration. | Expose ownership and scheduling. Prepared executable state is legitimate; per-draw lifetime bookkeeping need not be. |

CUDA's two APIs largely overlap and interoperate; choosing `cuLaunchKernel` instead
of `cudaLaunchKernel` is not intrinsically a faster algorithm. Their context/module
management differences matter to embedding and composition. OGPU should offer
runtime-like ergonomics in optional helpers without making a process-global implicit
device/context part of the contract. [NVIDIA's comparison](https://docs.nvidia.com/cuda/cuda-runtime-api/driver-vs-runtime-api.html).

Vulkan explicitly permits independent command pools to be used in parallel;
external synchronization is object-scoped, not a reason to serialize a device.
[Khronos threading guide](https://docs.vulkan.org/guide/latest/threading.html).
D3D12 gives the application responsibility for GPU hazards and resource lifetime
while moving substantial state preparation into pipeline creation.
[Microsoft's API comparison](https://learn.microsoft.com/en-us/windows/win32/direct3d12/important-changes-from-directx-11-to-directx-12).

Metal 4 separates command allocation from queues, exposes explicit dependencies
and residency sets, and offers both ordinary shader work and ML facilities. Those
features belong at different abstraction levels: a tensor-capable instruction or
resource is not the same thing as a mandatory model executor.
[Apple's Metal 4 overview](https://developer.apple.com/videos/play/wwdc2025/205/).

These are not five points on one simplicity/speed line. Ecosystem, hardware access,
compilation, presentation and programming model often decide the choice before
host-call overhead does. An explicit API can also be implemented badly.

### Common rules for a thin foundation

1. **Separate description, preparation, recording and execution.** Compilation,
   linking, memory allocation and resource creation have explicit boundaries.
   Ordinary draw/dispatch must not discover a missing pipeline variant and compile it.
2. **Expose independent lifetimes.** Backing memory, placed resources, descriptors,
   command storage, executable commands and completion values are not one object.
3. **Let callers choose storage and scheduling.** No implicit staging, per-call
   root allocation, device-wide lock, hidden dependency graph or mandatory host wait.
4. **Keep reusable state reusable.** Binding and updating state is independent of
   issuing work. Support direct work, indirect work and command replay, not just one.
5. **Expose actual hazards, not a resource-ownership fiction.** Dependencies describe
   execution and memory visibility. Pointer reachability is not automatically known.
6. **Distinguish capability from emulation.** Exact formats, numeric operations,
   stage behavior and queue semantics are queryable. Unsupported is a normal result.
7. **Allow composition.** Native interop, independent libraries and multiple queues
   must not require taking over the application's allocator, window or device.
8. **Put safety where it is effective.** Static typing, generated layouts and cold
   validation are valuable. Mandatory transitive resource scans and retention on
   every command are not a substitute for a valid asynchronous lifetime contract.

A native driver still manages hardware, may allocate command storage and may
schedule submissions internally. “Thin” does not promise zero driver activity or
zero host instructions. The enforceable promise is no extra mandatory OGPU policy;
command-storage exhaustion and any OGPU-side growth policy must be explicit.

## 2. Latest Aaltonen comparison

Freshly cloned `sebbbi/NoGraphicsAPI` on 2026-10-08, HEAD
[`04004140f5b3b8ec7c566fd43bfed77d586155f8`](https://github.com/sebbbi/NoGraphicsAPI/commit/04004140f5b3b8ec7c566fd43bfed77d586155f8),
dated 2026-10-06. This is not the earlier prototype previously discussed.
The [original article](https://www.sebastianaaltonen.com/blog/no-graphics-api)
argues for pointers, simpler binding and explicit dependencies instead of historical
graphics API scaffolding. We retain that direction, not every restriction of a
particular implementation.

| Area | Latest ngpu | Proposed OGPU decision |
|---|---|---|
| Linear memory | Raw application-partitioned GPU heaps; no public buffer objects | Adopt backing/range separation; retain backing identity for operations that need native handles |
| Images | Placed textures, view ranges, explicit alias activation | Adopt; add exact memory compatibility, subresource state and optional sparse backing |
| Descriptors | Separate indexed texture/sampler heaps; disjoint CPU slot updates | Adopt slot/range ownership; no whole-heap freeze |
| Host/GPU concurrency | Independent pools/queues; timeline points; immediate destruction | Adopt explicit object-local synchronization and lifetimes |
| Arguments | One caller-owned GPU root address per draw/dispatch; no inline root payload | Offer GPU roots **and** inline state; neither becomes a compulsory representation |
| Executables | Prepared PSOs; several raster states remain baked | Prepare native-required combinations explicitly; keep independently dynamic state independent |
| Commands | Reusable pools, one-shot command buffers | Keep one-shot and reusable executable lists, including explicit simultaneous-use capability |
| GPU-driven work | Indirect work and GPU-written root contents; CPU chooses root binding and draw count | Keep GPU count/draw identity; add capability-gated generated commands rather than claiming indirect equals fully GPU-driven |
| Image barriers | Global stage/access barriers, ordinary `GENERAL` layout | Simple global path plus explicit range/transition/ownership facilities where native strategies need them |
| Capabilities | Deliberately narrow modern baseline, including Vulkan mesh support | Modern baseline, but raster/mesh/matrix/video are independent profiles, not compute prerequisites |

See its pinned [public header](https://github.com/sebbbi/NoGraphicsAPI/blob/04004140f5b3b8ec7c566fd43bfed77d586155f8/include/NoGraphicsAPI/NoGraphicsAPI.hpp),
[design comparison](https://github.com/sebbbi/NoGraphicsAPI/blob/04004140f5b3b8ec7c566fd43bfed77d586155f8/docs/no-graphics-api-comparison.md)
and [shader contract](https://github.com/sebbbi/NoGraphicsAPI/blob/04004140f5b3b8ec7c566fd43bfed77d586155f8/docs/slang.md).

The new root mapping is important: Vulkan pushes an address mapped as a uniform
buffer, while Metal uses a device-address-backed structured root. Neither backend
copies the root structure. That is an excellent large/GPU-produced argument path;
it does **not** prove that making all scalar arguments memory-resident preserves
every native inline-constant strategy. Nor does one-shot recording subsume native
replay. These are architectural reasons to keep alternatives, without another
benchmark gate.

ngpu is now ahead of our implemented interface on several explicit-resource and
graphics facilities. It is not a complete compute capability/numerical contract,
and we should not infer its author's intentions about ML from that. Its Metal
address-to-buffer lookup and finite live-heap limit also illustrate why an elegant
address-only interface can hide backend work. Our non-owning host `Span` carries
backing identity; shader pointers remain ordinary GPU addresses.
[Metal mapping](https://github.com/sebbbi/NoGraphicsAPI/blob/04004140f5b3b8ec7c566fd43bfed77d586155f8/docs/metal-support.md).

## 3. The proposed foundation

Three layers, with only the first fundamental:

- **Core and capability extensions:** explicit C driver-level contract, Rust
  implementation, trusted commands and shader artifacts, no implicit ownership.
- **Checked interfaces/tooling:** generated host/shader layouts, Rust typed views,
  validation and diagnostics. Compile-time checks are preferred; expensive dynamic
  tracking is opt-in and its costs must be visible.
- **Convenience libraries:** allocators, upload rings, deferred deletion, retained
  submissions, tensor operators, render graphs and model runtimes. These use the
  same public core and can be replaced by an independent consumer.

### Objects and ownership

| Concept | Owns | Does not implicitly own |
|---|---|---|
| Device / queue | Backend connection / submission endpoint | Application scheduler or process-global current context |
| Memory | Explicit backing allocation | Every pointer or descriptor that refers into it |
| Span / GPU address | Nothing: backing+offset+size / shader-visible address | Lifetime, bounds checks in shaders or an allocation policy |
| Image / view | Native typed storage interpretation / selected subresources | Backing allocation unless an explicit dedicated-creation helper is used |
| Descriptor heap binding | Nothing: borrowed backing range and reserved subrange | Backing and referenced resources |
| Executable | Prepared native code and required state | Memory reachable through its arguments |
| Command arena / list | Recording storage / encoded work using that storage | Referenced application resources |
| Timeline point | Nothing: semaphore handle and value | A heap-allocated completion receipt or deferred deletion queue |
| Residency set, when supported | Explicit residency membership | Allocation lifetime or an implicit per-dispatch resource walk |

All asynchronously used storage survives its last use. Destruction never secretly
waits; waiting is a separate operation. Reset invalidates the arena's lists and is
legal only once all their pending uses finish. Reusable lists keep references valid
until the application stops replaying them, not merely until their first completion.
Memory aliasing is explicit; discarded contents must be initialized before reading.

The ordinary path remains small: allocate memory, prepare an executable, record
bindings/work/dependencies into an arena, submit to a queue with a timeline signal,
then reuse storage after completion. Image views, virtual memory, presentation and
specialized execution are only involved when the application uses those facilities.
Capability profiles must not turn ordinary dispatch into a long feature negotiation
on every call; requirements are resolved at device/executable preparation.

### Whole surface inventory

The companion header sketches the core signatures and data flow. Opaque description
types mark schemas still to be finalized, **not already available extension APIs**.
The following is the intended coverage, including capability modules beyond the
first implementation tranche.

| Family | Operations / contract |
|---|---|
| Discovery | Enumerate adapters, identify backend/native device, query versioned capability records and exact support, request queues/features, create/destroy device |
| Memory | Query compatible types/budgets/requirements; allocate/free; address; persistent map/unmap; flush/invalidate; explicit placement and alias reuse |
| Virtual memory profile | Reserve/free address space; create backing; map/unmap ranges; access permissions and sparse bindings ordered explicitly; no mandatory relocating allocator |
| Images/views | Dimensions, mip/layer/sample counts, format/aspects/usage; placed creation; compatible reinterpretation and sampling/render/storage views; exact support queries |
| Descriptors | Caller-owned encoded bytes and GPU backing ranges; independent resource/sampler binding; explicit reservation and range ownership; ordinary host/GPU copies |
| Executables | Load native artifact, validate interface/requirements; specialize/link/prepare outside recording; compute and graphics programs; explicit cache import/export |
| Arguments | Recording-local inline range updates and device-root binding; generated layouts/stage visibility; no arguments copied again by current-state draws/dispatches |
| Commands | Caller-owned arenas; reserve/reset/trim; begin/end; one-shot or replay; explicit simultaneous-use support; secondary/bundle execution where supported |
| Submission | Queue selection, multiple lists, timeline waits/signals and stage scopes; empty signal submissions; polling and finite/infinite host waits; explicit failure state |
| Synchronization | Separate stage/access masks; global/range barriers; split producer/consumer dependencies; image transitions; queue ownership handoff; alias activation |
| Transfers | Range copy/fill, image↔memory with row/slice pitch, image↔image and subresources; resolve/clear; no automatic format conversion or staging allocation |
| Graphics | Multiple color/depth/stencil attachments, views, load/store/resolve, samples/layers/render area; viewport/scissor, depth/stencil, blend/raster state; direct/indexed/indirect/count draws and draw identity |
| Advanced rendering profiles | Task/mesh, tessellation/geometry, native vertex fetch, multiview, shading rate, tile-local/input attachments and pass continuation; no universal emulation |
| Compute/ML | Direct/indirect dispatch; workgroup/shared-memory/subgroup requirements; exact numeric/atomic/matrix capabilities and artifact contracts; ordinary memory for tensors |
| GPU-driven execution profile | Command layout and executable tables, explicit preparation/storage, GPU-selected commands/arguments/counts; capability-specific token subset |
| Queries/diagnostics | Timestamp, occlusion and pipeline-statistics pools; asynchronous result resolution; labels/capture hooks; explicit loss/creation/submit errors |
| Residency profile | Explicit sets/ranges, queue or submit association, budget information and optional make-resident/evict operations; no per-command pointer traversal |
| Presentation/platform | Separate surface/swapchain objects, acquire/present dependencies, format/color space/resize; headless device does not imply a window or queue-zero presentation |
| Native/external interop | Borrow/import/export device/resource/synchronization handles with ownership rules; native command sections with explicit entry/exit state; external memory and peer access capabilities |
| Specialized modules | Ray tracing/acceleration structures, video, cooperative/cluster launch, device enqueue and native tensor instructions/resources where available |

This is a coverage draft, not a promise to implement every native extension in one
patch. The single implementation tranche below completes the common foundation
and remaining graphics/ML consumer surface. Specialized hardware modules get
explicit capability boundaries now; their native-specific records/operations need
separate specifications before they can be advertised as supported. A generic
`get_proc_address` is extensibility infrastructure, **not implementation of them**.

## 4. Decisions that prevent accidental policy

### Memory and descriptors

Use 64-bit sizes/offsets. Allocation requirements report size, alignment, compatible
memory types and dedicated requirements; do not assume every image fits one heap
type or every device is coherent/UMA. Large native copy limits can be handled by
explicitly bounded encoding, not by narrowing the public address space to 4 GiB.

`Span { memory, offset, size }` is host operation metadata, not a shader buffer
binding. It lets Metal obtain backing handles without a global address search.
Address-consuming native paths still receive the same GPU address. Imported or
reserved virtual addresses carry an explicitly registered backing/range object when
an operation requires one; an arbitrary shader pointer is not automatically a
valid CPU copy endpoint. Query alignment and cache-maintenance granularity.
An address-native command profile can also accept raw addresses on backends that
support them; the portable Span path must not outlaw that strategy. Requiring a
new backing registration for every native address-only operation would itself be
a restriction to avoid, not an acceptable hidden tax.

Descriptor updates affect only the specified range. In-flight use of other slots
does not lock the heap. The application prevents CPU writes racing GPU reads of
those same slots and retains the referenced resources. If native descriptor
granularity prevents a promised independent update, report that constraint; do
not add invisible heap cloning. GPU-writable descriptor bytes are a distinct
capability, not implied by GPU-writable indices.

The implemented foundation refines the initial owned-heap sketch: encode opaque
descriptors into caller host bytes, then bind a selected GPU span. No heap object
chooses allocation, slot stride or upload policy. Mixed resource descriptor layouts
are possible; shader metadata and GPU placement must obey queried sizes/alignments.
Native reserved bytes are explicit and protected until referencing command buffers
are reset/destroyed. Local batch validation is atomic; native write failure may be
partial, with recovery owned by the caller. See the
[implementation contract and remaining proof](foundation-implementation.md#implemented-descriptor-encoding-and-binding).

The first shader consumer now exercises sampled images, samplers, storage images
and buffer descriptors together, including explicit LOCAL descriptor uploads and
changed-descriptor replay. It uses a caller-chosen mixed layout without new runtime
policy. Typed heap indices must follow the artifact's aligned descriptor strides,
not be mistaken for a universal application slot number. Generated interface
metadata must preserve that distinction.

### Arguments: do not replace one compulsory representation with another

Retain inline constants/range updates for small control data, and add device-backed
argument blocks for large, persistent or GPU-written data. Executable metadata
declares inline ranges and root slots, stage visibility, alignment, address space
and native mapping. A simple program can use just one shared root. Programs that
need independently updated stage roots are not forbidden from doing so.

The address is state; pointed-to bytes are read during GPU execution. Inline bytes
are captured during recording. Neither path implies a copy of pointees. Rebinding
one root does not invalidate unrelated inline bytes. Replay preserves encoded
inline state/addresses but can observe new root contents after the required memory
dependencies. Changing executable invalidates only incompatible argument domains,
as described by interface compatibility metadata.

Inline and root paths are **requested strategies**, not promises that all backends
have the same fast ABI. Exact support and preparation must reject an unmappable
strategy instead of silently spilling inline bytes into an upload allocation.
Keep compiler artifacts capable of selecting a different representation explicitly.
The shared Slang struct remains useful, but layout, address space and uniformity
must all be described; equal field offsets alone do not prove equal code generation.

The implemented Vulkan compute profile now uses declared byte offsets for any
number of root slots plus partial inline updates. Its capabilities explicitly say
that these share one native byte namespace; no per-stage bank isolation is implied.
Artifact producers choose separate offsets when they need independent fields.
Bindings preserve bytes, and callers reinitialize any incompatible interpretations.
Vertex/fragment now uses the same shared namespace with explicit root visibility;
other native mappings remain to implement. See the
[compute/argument contract](foundation-implementation.md#implemented-compute-preparation-and-arguments).

### Synchronization, layouts and queues

A global stage/access barrier is the simple common case. Preserve ranged scopes,
split endpoints, subresources and ownership transfers so an application can express
native strategies that are not one whole-device dependency. Queue family/type and
ownership rules are visible; an implementation must not fabricate independent
execution by silently funneling every queue onto one queue.

Use unified image access where the backend supports the requested strategy. Keep
explicit image-state transitions for presentation, interop, aliasing and native
optimized layouts. Do not declare “all layouts are always equivalent” merely
because a GENERAL-layout path is correct. If an efficient implicit layout choice
cannot be justified, the fast path is explicit or capability-gated. This preserves
the modern baseline without bringing back a legacy compatibility backend.

Host operations synchronize at the object actually being mutated: one encoder or
arena per recording worker; independent queues/pools may proceed concurrently.
Creation and immutable queries are not protected by a compulsory device-wide lock.
Timeline signal values have explicit monotonic ordering, including across queues;
stage-scoped waits/signals are accepted only when the backend can honor their
semantics. Host signal and GPU signals must not race to violate that ordering.
A stage-scoped signal is not automatically completion of the entire list. Arena
reset or wholesale resource retirement requires a point covering every relevant
operation, normally a terminal all-commands signal. Queue ordering alone is not a
substitute for the memory visibility dependencies required by the accesses.

### Executables and graphics

Expose prepared executables, not a universal claim that hardware needs no PSOs.
Vulkan shader objects and descriptor heaps remove particular binding/preparation
objects; they do not establish that all backends execute every state combination
without preparation. [Shader object proposal](https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_shader_object.html),
[descriptor heap proposal](https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_descriptor_heap.html).

The executable description partitions state into declared dynamic state and
prepared static state. A requested dynamic-state combination has an exact support
query; unsupported requests fail at preparation, never trigger compilation at draw.
Attachment/blend variants that Metal needs prepared remain explicit preparations.
Allow offline artifacts and persistent native caches with compatibility identity.

Add full view/mip/layer handling, attachment arrays, stencil, blending, raster state,
MSAA/resolve, direct and indexed work, indirect counts, and compute-written geometry
through one coherent set of structures. Render area must be explicit, including
attachmentless rendering where supported. Do not require fragments to be read-only
or require every pass to store transient attachments to external memory.

Pointer vertex pulling is the default, not a ban on optional fixed-function vertex
fetch or other native stages. A model intended for anything must be able to express
those paths if they expose a useful hardware strategy. Tile-local dependencies,
attachment feedback and pass continuation are explicit profiles rather than a
claim that ending/restarting a pass is always free.

### ML is primarily a device-code and capability contract

Do **not** put matmul, attention, a tensor allocator or an operator graph into the
fundamental host API. Add enough execution/memory/capability control to implement
them externally, including quantized data and application-chosen numerical policy.

Describe storage types separately from arithmetic and accumulation types; exact
subgroup sizes/operations, shared-memory limits, atomic width/scope/order, rounding,
contraction/denormal behavior and matrix instruction shapes/layouts are part of
artifact requirements. Return supported tuples, not one `supports_ml` Boolean.
Specify NaN, overflow/saturation and quantization behavior in each executable's
numerical contract rather than promising all devices use identical arithmetic.

Support configurable local size and dynamic shared memory where native execution
can supply them; otherwise explicitly prepared variants. Persistent kernels,
cooperative launch, device enqueue, asynchronous memory movement and clusters are
capability profiles, not silently replaced by CPU loops. A hardware tensor map or
matrix resource belongs in an extension if the instruction needs one: avoiding a
model runtime is not a reason to forbid specialized hardware objects.

CUDA's explicit virtual-memory operations and runtime graph facilities illustrate
that memory management and reusable execution are independent axes, not one
“high-level versus low-level” choice.
[Virtual memory](https://docs.nvidia.com/cuda/cuda-driver-api/cuda_driver_api/group__CUDA__VA.html),
[runtime graphs](https://docs.nvidia.com/cuda/cuda-runtime-api/cuda_runtime_api/group__CUDART__GRAPH.html).

### Safety and failure

The fundamental C interface is a trusted, asynchronous systems interface. It cannot
make arbitrary shader pointers memory-safe. Rust prevents classes of implementation
errors; Slang/generated layouts prevent classes of interface mismatch. Neither
proves GPU lifetime, race freedom or numeric correctness.

Check object descriptions, arithmetic overflow, support and artifact compatibility
on cold paths. Report allocation/creation/recording/submit failures; no panic across
FFI and no “success” after silently omitting work. Recording may accumulate an
error and report it at `end`; an invalid list cannot be submitted. Trusted hot-path
preconditions avoid mandatory transitive scans, locks or resource reference counts.
Debug validation can diagnose contract violations; release invalid use remains
outside the contract, rather than a promise that unsafe foreign pointers can be checked.

A Rust safe wrapper can own a restricted, provable subset using lifetimes and
completion tokens. Extracting unrestricted addresses or submitting arbitrary code
requires an unsafe boundary. Automatic retention/deferred destruction is available
only in an explicitly chosen helper; ordinary handle destruction never waits.
Device loss is a distinct terminal condition, not successful completion of writes;
in-flight resources follow a documented teardown path and must not be reused as
if work finished successfully. No implicit synchronization is introduced to hide it.

## 5. One coordinated implementation tranche

### Native mapping review before freezing records

These are intended mappings, not newly tested backend support:

| Core choice | Vulkan mapping direction | Metal 4 mapping direction / unresolved boundary |
|---|---|---|
| Memory + non-owning spans | Device memory and addressable backing; placed images | Buffers/placement heaps; native buffer handle available directly from a span |
| Explicit command storage | Pools, command buffers, reset and replay flags | Command allocators and command buffers; replay/secondary semantics need exact profile review, not assumed parity |
| Queues and timeline points | Queue submissions, timeline semaphores and queue-family ownership | Command queues/events with native ordering rules; supported stage scopes must be established explicitly |
| Independent descriptor ranges | Native descriptor heaps with application-owned slots | Texture-view pools/argument resources and sampler entries; mutation granularity must match native guarantees |
| Device-root arguments | Address-backed root mapping without root-structure copies | Argument-table address bindings with compiler-declared layout/address space |
| Inline argument state | Native push/inline path where the artifact mapping supports it | No claim that a Vulkan-like inline path exists; request must be supported exactly or choose a different artifact explicitly |
| Prepared graphics state | Shader/pipeline preparation and supported dynamic state | Explicit pipeline specialization plus native independent encoder state |
| Dependencies and residency | Memory/image barriers, split dependencies and submission dependencies | Explicit barriers/events and residency sets; no inferred transitive pointer reachability |

The same principle applies to future CUDA/D3D12 implementations: a common concept
must preserve native strategy, while a capability may remain backend-specific.
Do not force Metal to emulate Vulkan's command replay or inline ABI just to make
a feature table look uniform. Equally, do not remove those strategies from Vulkan
because a second backend cannot expose them unchanged.

### Implementation sequence

Implement the core below together, in dependency-ordered commits on the same
development line. These are engineering boundaries, **not separate research gates
or repeated requests for approval**. Prefer a deliberate ABI break over years of
compatibility branches. Keep tests/consumers migrated at usable checkpoints.

1. **Contract and compiler metadata.** Finalize the draft record layouts/enums,
   enabled capability tables, object-local threading, failure and ownership rules.
   Add generated inline/root interfaces and artifact requirements. Document a
   native mapping for every core operation before committing its ABI promise.
2. **Resource and command foundation.** Memory/Span and placed images/views;
   independent descriptor ranges; explicit command arenas/lists; queue handles,
   timeline points and submit arrays; global/ranged/split/ownership dependencies.
   Remove mandatory retention and device-global serialization from the core.
3. **Complete common graphics and compute surface.** Both argument strategies;
   direct/indirect/count operations; attachment arrays, depth/stencil, mip/layer
   views, blending, raster state, resolve and pitched/subresource transfers;
   queries and prepared state variants. Keep existing replay and draw-identity
   capabilities. Add subgroup/numerical requirements and indirect compute.
4. **Integration boundary and consumers.** Capability-versioned extension lookup,
   native resource/synchronization interop contract, separated presentation API;
   implement native hooks needed by available Linux tests. Move old conveniences
   above core. Migrate examples/generators/GGML/libplacebo and the installed SDK;
   finish the M4 scene and M5 transformer/quantized-linear consumers using this surface.
5. **Consolidated verification and cleanup.** Run the bounded correctness/contract
   suite, out-of-tree installation and selected native-strategy comparisons. Remove
   the replaced internal paths and update support/ABI documentation in that tranche.

This is a substantial implementation, not a claim that one turn can safely rewrite
every backend. Existing code remains the executable reference until migrated.
Specialized ray/video/VM/generated-command modules are designed into the coverage
map but not fabricated as working Vulkan/Metal features in this tranche. Finalize
their concrete native-specific tables when implementing them; no blanket feature
claim follows from a placeholder. CUDA/D3D12 are comparison targets, not newly
selected backends to implement.

Metal mappings should be reviewed and compilable where tooling allows; absent a
Mac, native execution remains explicitly unvalidated. No new GPU or Mac round trip
is needed to draft or implement the Vulkan path. M6 still owns native parity.

## 6. Assumptions and what remains to prove

The architectural judgments above are enough to choose a broad draft. They are
not a universal performance theorem. The test for a foundational restriction is
still: could a native implementation preserve the native strategy under equivalent
correctness, resource and latency requirements? A counterexample changes the API.

| Assumption / risk | Verification, after or alongside implementation | Consequence if false |
|---|---|---|
| Span metadata gives cheap native operations without restricting shader addresses | Trace address/offset lowering and imported/placed resources; inspect allocations and lookups | Change the range/backing representation, not force shader buffer bindings |
| Inline and GPU roots preserve distinct native argument strategies | Inspect generated shader/native bindings; targeted scalar-heavy and GPU-written-root tests with equal budgets | Add/adjust argument ABI profiles; reject hidden spills/copies |
| Disjoint descriptor slots really are independent at advertised granularity | Concurrent host updates plus pending GPU users of other slots; native validation | Expose native granularity/profile; never silently freeze or duplicate every heap |
| Unified access is efficient for the claimed image profile | Native mapping review first; targeted compression/layout/tile workload where uncertain | Require explicit state or narrow the profile; correctness alone does not establish performance |
| Explicit ownership removes framework work without losing a usable safety boundary | Invalid-description/overflow tests, safe-wrapper compile tests, lifecycle tests and device-loss teardown review | Tighten contracts/wrapper boundaries; do not restore mandatory implicit waits |
| Queue/split scopes preserve native scheduling freedom | Dependency litmus tests, cross-queue handoffs and object-local parallel recording; inspect lowering | Narrow unsupported scopes or change semantics; local lack of overlap is not proof of equivalence |
| Prepared executable partitions avoid hidden JIT and preserve useful dynamic state | Enumerate native preparation keys, instrument compilation during recording, incompatible-state tests | Revise prepare/dynamic interface before promising support |
| ML requirements describe actual generated arithmetic | Artifact inspection and numerical references for subgroup/reduction/attention/quantization; accelerated variants only where available | Correct numeric profile or compiler metadata; do not call scalar fallback acceleration |
| Optional profiles leave room for native-only facilities | Paper mapping of representative ray/video/VM/tile/generated-command operations | Expand an extension contract; do not pollute ordinary draw/dispatch with a scheduler |

Run basic ABI/build/validation/correctness checks continuously, especially around
unsafe code; these are not postponed behind a grand rewrite. Batch performance
verification around the genuinely uncertain boundaries above. Do not repeat a
full timing matrix for an obvious policy removal or every new enum. Existing
recording-argument results remain evidence, but their pending timing refresh is
not a gate on designing queues, placement or the remaining graphics/ML interface.

The draft deliberately does **not** yet settle binary field numbering, extension
record schemas, every backend capability limit, all numerical tuples, native
external-handle ownership cases or portable device-loss recovery. The companion
header makes these gaps visible. Finalizing them is implementation work with
focused review, not justification for leaving the whole API direction unspecified.
