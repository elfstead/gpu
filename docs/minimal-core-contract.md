# C1: minimal composable foundation draft

Started 2026-10-31 on `minimal-core`, based on Vulkan `master` at `2374635`.
**C1 contract selected and consolidated, 2026-11-03; C2 implementation active.**
The [complete disposition and C2 cutover](minimal-core-migration.md) close the
foundation design checkpoint. The public Vulkan argument/descriptor cutovers, heap inheritance, generated argument layouts, per-entry dependencies and unified submission are implemented
in foundation schema 7, including the public borrowed-frame presentation cutover. The full C2 audit/local C3 remain. This is the next revision of the [whole-API target](whole-api-design.md), governed by the
[composition rule](design.md#minimal-composable-core) and [C1–C3 sequence](roadmap.md#minimal-core-revision).

## Decisions made in this draft

- Keep one logical argument interface per prepared executable. User code groups
  arguments, allocates persistent records and chooses when to update them.
- Preserve recording-time values and execution-time referenced data. They differ
  in lifetime and observation semantics, not merely in convenience. Investigate
  expressing both through one argument-update operation before retaining separate
  root and inline entry points.
- Prefer one descriptor-heap abstraction with explicit writes, copies and binding.
  Slot allocation, caching and transient-table construction stay outside core.
  Native memory access, when available, is a capability of the same storage object,
  not a second descriptor system with independently maintained contents.
- Keep one recorded-list model. Replaying an existing recording and recording
  different commands are different operations on its storage, not separate APIs
  for a frame runtime and an inference runtime.
- Keep independent surfaces. Build single-window and multi-window frame loops in
  consumers. A device owns neither a window nor a frame scheduler.
- Use existing memory, execution and synchronization primitives for higher-level
  graphics/ML algorithms. Specialized hardware capabilities remain scheduled;
  their existence does not justify a parallel allocation or scheduling system.

These decisions select the conceptual shape. Argument decisions A/B now have a
[selected contract and exact record sketches](minimal-core-arguments.md).
Descriptor decision D also has a [selected storage contract and records](minimal-core-descriptors.md).
Presentation decision P has a [selected lifecycle and record sketch](minimal-core-presentation.md).
[Dependency and submission decisions](minimal-core-dependencies.md) are also selected.
The [combined index, exact operation inventory and record migration](minimal-core-migration.md)
integrate those decisions and track the ongoing C2 implementation.

## Ordinary operation sequence

1. Discover capabilities; create a device and explicit queues.
2. Allocate backing and prepare resources, descriptor storage, executable
   interfaces and command storage. These operations may allocate/compile.
3. Record bindings, argument updates, work and dependencies. Ordinary work does
   not allocate argument buffers, inspect reachable resources or rebuild heaps.
4. Submit recorded lists with explicit waits and signals. Caller-owned reusable
   scratch accommodates native description translation when needed.
5. Observe completion, then reuse application storage or replay the list. Reset,
   destruction and resource reuse never insert a hidden wait.

Native Rust implements these operations directly; C adapts the same operations.
Neither interface adds ownership policy. Generated typed helpers can translate
user argument structures at build time and are replaceable by third-party tools.

## Arguments: one interface, distinct observation semantics

The present `set_root` implementation validates a slot and delegates to the same
eight-byte push-data update as `set_inline`; it is not a distinct Vulkan hardware
operation. See [command encoding](../crates/ogpu/src/foundation/commands.rs) and
[interface preparation](../crates/ogpu/src/foundation/executables.rs).
That makes separate public operations candidates for consolidation. It does not
prove that a raw byte bank is the correct universal contract.

The existing [reuse result](argument-reuse-results.md) demonstrates a real
bind-once workload; [partial-update evidence](argument-patch-results.md) covers
changing a small portion of a larger argument record. Requiring a full argument
resupply on every draw would discard known behavior. These are historical bounded
results, not new measurements or proof of a particular cross-backend ABI.

Selected operation shape (record details in the A/B contract):

```text
prepare_argument_layout(fields, execution_class) -> layout
prepare(artifact, layout, state) -> executable
bind_executable(encoder, executable)
set_arguments(encoder, layout, byte_offset, byte_count, values)
dispatch/draw/trace(encoder, work_parameters)
```

An argument interface describes values, their interpretation by the artifact and
their native update granularity. It is prepared outside recording. A reference
value captures an address/binding; the referenced bytes are consumed at execution.
An inline value is captured during recording. Address lifetime, alignment, resource
access and arbitrary shader dereference support are distinct requirements.
Do not infer nested shader pointers from a backend accepting a root buffer address.

An interface consisting of one address supports the NGPU-style root. A compact
inline record can hold several scalar values and addresses. Logical material,
scene and tensor groups are user structures; they do not require independently
named root objects in the core. A user may place a pointer table in GPU memory,
but the core must not force the extra indirection when direct argument delivery
can avoid it. D3D12's native root cost model confirms that inline values and
buffer references are not interchangeable representations. [Root costs][root-costs]

Cold metadata must distinguish actual constant-buffer/resource bindings from
arbitrary integer address bits. The shader ABI, binding role and native budget
must agree. The runtime must not reinterpret pointer bits as a different resource
kind, silently spill inline data into a buffer, or merge incomplete address writes
using a retained host copy.

**A selected, 2026-11-03:** one byte-range update with declared atomic native
address fields and partially writable inline fields. One root, a scalar patch and
a combined address/scalar update have explicit lowerings. Separate exported
`set_root` and `set_inline` paths consolidate; generated typed helpers use that
same operation. No argument shadow or mandatory per-draw resupply.

**B selected, 2026-11-03:** immutable layout metadata reports native storage
domain/base, binding-compatibility key and bind invalidation. Preserve native reuse
without promising a universal bank or universal stage isolation. Bind does not
restore cached values. See the [A/B rules, examples and migration](minimal-core-arguments.md)
and [C sketch](drafts/ogpu_arguments.h)/[Rust records](drafts/arguments.rs).
These are design decisions; runtime and native validation are still pending.

## Descriptors: storage and mutation, not allocation policy

Selected operation shape (record details in the D contract):

```text
heap_requirements(kind, capacity, requested_capabilities) -> requirements
heap_create(requirements, explicit_storage_choice) -> heap
write_descriptors(heap, destinations, descriptions, scratch)
copy_descriptors(destination_heap, source_heap, ranges)
bind_heap(encoder, heap)
heap_info(heap) -> actual borrowed spans when available + restrictions
```

Creation is cold. Writes/copies affect only selected locations and are independent
of binding. Binding never materializes recipes or traverses all descriptors.
Descriptor contents are observed according to their declared update/execution
boundary; applications order mutation against consuming work. Resource lifetimes
are not retained by descriptor entries. Disjoint updates obey reported native
granularity and reserved-storage restrictions.

The optional memory view is derived from heap info: if available, ordinary
mapping, buffer copies and shader accesses operate on the heap's actual storage.
There must be no shadow recipe buffer requiring a later commit/rebuild. If absent,
the heap remains opaque and those memory operations are unsupported. This is not
a portable GPU descriptor-editing promise: D3D12 exposes CPU editing, while Vulkan
descriptor heaps expose memory access. [D3D12 heaps][descriptor-heaps],
[Vulkan heaps][vulkan-heaps]

Applications normally keep stable descriptors and generate indices. GPU selection
of indices is not equivalent to GPU mutation of a descriptor. Preserve the latter
capability only through a native mapping, without CPU readback. Likewise, explicit
allocation/placement cannot be replaced by an automatic staging policy.

**D selected, 2026-11-03:** native-unit locations and queried per-description shapes
preserve compact Vulkan layouts and opaque D3D12 slots. One heap borrows actual
host/GPU backing or explicitly allocates native opaque storage; writes/copies and
binding use that same object. CPU copy-source capability exposes D3D12's restriction
without a hidden mirror. GPU movement uses ordinary accessible memory. See the
[contract and migration](minimal-core-descriptors.md) and its C/Rust records.

## Commands, memory and dependencies

Keep the current arena/encoder/list lifetime structure. Reuse flags describe
whether a list can be submitted once, serially or concurrently; they do not create
different scheduler abstractions. A user frame loop can choose never to replay.
A fixed inference loop can update data and resubmit. Pool reset is not command
replay. Secondary and generated execution remain capability-dependent because
recording composition and GPU selection cannot generally be derived from ordinary
CPU recording without extra work.

Keep explicit backing, ranges, placed images, views and queried requirements.
Application allocators compose them without mandatory registration per suballocation.
Host spans carry backing identity for native operations that need it; device pointers
remain shader values. M10 must resolve foreign/raw-address command endpoints rather
than assuming that every address is a known OGPU allocation.

Global stage/access dependencies are the default. Stage-scoped queue waits preserve
the fragment-only producer/consumer case without holding back a draw's independent
vertex work. Resource-specific transitions still need resource identity when native
state changes demand it. Do not equate the scope of memory visibility with the scope
of execution ordering. Existing ranged/split/ownership mechanisms are reviewed
individually in the [selected disposition](minimal-core-dependencies.md): retain
their justified native scopes, with per-entry stages in one batched dependency.
Submit uses one array operation with explicit acceptance outcomes and one caller
scratch span. Single-batch and binary variants become ordinary input shapes.

Memory placement, cache management, aliases, image transitions and copy footprints
remain explicit requirements. Query a supported footprint and use one native copy
where possible; row conversion, format conversion and staging allocation are user
algorithms. No compatibility lowering may silently expand one image transfer into
hundreds of row commands. Ordinary modern image access remains the default; legacy
layout alternatives are not restored without a concrete need.

Local validation failures occur before native side effects where promised.
Recording errors poison the recording. Submission distinguishes rejection before
acceptance, accepted work, and unknown/partial acceptance requiring conservative
recovery. Do not promise native batch atomicity, retryability or resource retirement
merely because one backend uses a single native submit call. Timeout is not
completion; device loss must not masquerade as a completed timeline value.

## Presentation and platform boundary

Keep surfaces independent of devices and input/window creation outside OGPU.
A surface identifies its native platform honestly. Prepared presentation storage
belongs to the caller's explicit lifecycle; acquisition must not allocate a new
ownership graph per frame.

**P selected, 2026-11-03:** a borrowed frame distinguishes native acquisition
signals from current-buffer/queue ordering. Explicit acceptance, frame and sync
effects separate rendering completion, presentation retirement and image reuse.
Native batch limits preserve Vulkan batching without hidden DXGI loops. Native
replacement retains old acquired images; in-place resize explicitly invalidates
borrowed images, including after a failed native attempt. No invented semaphore,
host wait or bridge submission. See the [lifecycle, mapping and migration](minimal-core-presentation.md).

The user-level loop chooses frame count, queue, pacing, resize policy, minimized
window behavior and retirement strategy. Multi-window uses the same primitives
with independent surface state. This boundary remains optional for compute.

## Whole-surface disposition and migration

“Keep” means capability retained in the target, not every existing signature.
“Review” leaves today's code intact until an explicit decision and migration.

| Family / current entry points | Disposition | Migration or user composition |
|---|---|---|
| Discovery, queries, device/queue creation | Keep core | Explicit enabled capabilities and native limits; no implicit current device. |
| Memory requirements/allocate/map/address/flush | Keep core | Arenas, suballocation, upload rings and deferred frees are user code. |
| Images/views and explicit backing | Keep core | Loaders, mip chains and render-target pools compose resources and transfers. |
| `write_*_descriptors`, `bind_heap` | D selected; migration pending | Native-unit heap locations and explicit storage/copy rights; migrate raw storage and generated indices together. No retained recipe path. |
| `set_inline`, `set_root`, argument metadata | A/B selected; migration pending | One byte-range update and prepared layout; generated root setters become ordinary user/tooling helpers. |
| Executable preparation and native cache | Keep core | Artifact/compiler cache policy stays outside; no command-time variant compilation. |
| Arena/encoder/list, reset/trim/reuse | Keep one model | Frame slots, repeated inference and retirement queues use the same storage. |
| Direct/indirect/count draw and dispatch | Keep hardware operations | Native GPU count consumption cannot be replaced by host readback or maximum-count empty work without changing cost. |
| Secondary/generated layouts, sets and preprocessing | Keep capability-dependent | Caller builds command topology; no render graph or automatic preprocessing allocation. |
| Queues, timeline points, scoped waits/signals | Keep core | User scheduler owns dependency graph and queue selection. |
| Global barriers and image state transitions | Keep necessary semantics | Default global visibility; native image/interop/alias transitions carry identities. |
| Buffer-range barriers, ownership modes, split events | Selected capability-dependent core | Preserve bounded native visibility/ownership and split producer scopes; per-entry stages avoid forced scope unions. No new benchmark gate. |
| Single/batched/binary submission | Consolidate | One batch-array operation, explicit acceptance outcomes and one translation span; no binary extension or duplicate runtime paths. |
| Transfer/fill/clear/resolve/blit | Keep hardware operations where non-derivable | Explicit footprint preparation and conversion kernels are user code; no hidden CPU/GPU conversion. |
| Raster state, attachments, vertex/index fetch | Keep hardware coverage | Scene graphs, materials and PSO selection are user code; remove duplicate setters only with equivalent native encoding. |
| Queries and diagnostics | Keep native observation | Profilers/capture tools compose explicit queries; instrumentation is opt-in. |
| Presentation extension and binary/fence operations | P selected; migration pending | Borrowed frames, explicit availability/effects and capability-dependent replacement/resize; retain necessary native synchronization. |
| Native Rust and C ABI | Same operations | Rust uses backend operations directly; C adapts errors/records. No safe wrapper hidden in either path. |
| Numerical, subgroup and matrix capabilities | Shader/artifact requirements first | Tensor shapes, kernels, fusion and numerical policy belong in consumer/compiler libraries. |
| M8 task/mesh and remaining advanced raster | Capability-dependent core | Reuse argument/execution/resource primitives; GPU culling remains an application algorithm. |
| M9 rays | Capability-dependent hardware | AS requirements/build/query/trace and shader tables; allocation/build scheduling and scene abstractions remain outside. |
| M10 interop, VM/sparse/residency | Capability-dependent hardware | Explicit shared memory/sync and mapping control where needed; eviction/streaming policy belongs above. |
| M11 richer launch and media | Capability-dependent hardware | Reuse shader/command capabilities; codec sessions expose native work, not a playback/model runtime. |

This table covers the whole target by mechanism family. Exact public-operation
deletions and replacement signatures are in the [consolidated inventory](minimal-core-migration.md); this is not permission to
remove unlisted native facilities or an assertion that future families are implemented.

## Public user-composition recipes

**Argument arena:** allocate mapped memory, suballocate aligned root records in
user code, populate and flush them when needed, and update the referenced argument
value. Track the maximum outstanding completion for each reused range. Small
recorded constants use the same interface's captured-value semantics, without an
application root allocation. No upload allocator is built into dispatch.

**Descriptor allocator/cache:** reserve locations inside one heap, memoize resource
descriptions in the library if useful, write only changed entries and recycle
locations after their last use. Stable descriptors plus a GPU-written index buffer
support ordinary bindless selection. Descriptor relocation uses native storage
capabilities only; the library does not receive a privileged backend API.

**Repeated inference:** prepare kernels once, record the fixed sequence and its
dependencies, then submit it repeatedly with ordered updates to input/argument
storage. Inline arguments remain recorded values. Changing command topology
requires rerecording or explicit generated work; the runtime does not infer it.

**GPU-culled rendering:** a compute kernel writes indirect work/counts and selected
data; an explicit dependency makes them visible to native indirect/generated
execution. Materials and root data are user structures. CPU readback of the count
is not an equivalent composition.

**Frame loop:** allocate several user frame slots; each owns recording storage,
application ranges and the presentation synchronization required by the chosen
surface. Wait/poll only the slot being reused, acquire, record, submit and present.
Retire old surface resources using their actual guarantees, not frame-number
guessing. Multiple windows instantiate this state independently on one device.

## DX12 mapping constraints, without a DX12 implementation on this branch

| Primitive | Native mapping to preserve | Constraint / outstanding check |
|---|---|---|
| Captured argument values | Root constants | Exact budget and update range; no buffer allocation/spill hidden by OGPU. |
| Address/resource arguments | Appropriate root buffer binding or shader-visible address representation | Shader ABI must declare the role; nested physical pointers are not established by a host root binding. |
| Interface reuse | Compatible prepared root signatures | A/B report native domains and bind invalidation; no unconditional dual writes or host replay. |
| Descriptor storage | Native resource/sampler heaps with explicit CPU writes/copies | D selects opaque native storage and explicit CPU-copy-source rights. Bind does no rebuild; no GPU byte-editing or shader-visible CPU-copy-source emulation. |
| Commands/replay | Native allocator/list lifecycle | Native reuse/allocator retirement must match declared mode; no rerecording sold as replay. |
| Dependencies | Enhanced barriers and queue fence ordering | Grouped/free-access lowering must be corrected; stage-scoped cross-queue guarantees cannot be fabricated from a whole-queue wait. |
| Transfers | Native footprints and copy operations | Query pitch/placement requirements; no automatic per-row fallback. |
| Presentation | Typed HWND surface and DXGI lifecycle | P selects queue-ordered current-buffer selection, explicit resize invalidation and honest native retirement capabilities; Windows validation remains open. |
| Failure | Per-operation native outcome | Queue wait/execute/signal can have partial acceptance; device-removed completion sentinels are failures. |

These constraints come from the [pinned review](dx12-review-handoff-2026-10-30.md)
and native documentation; they are not newly validated Windows behavior. The
reference stays on `dx12-backend`. Metal's [mapping proposal](m6-native-mapping.md)
remains an additional design check, with native execution deferred.

## C1 closure and implementation verification

The [consolidation](minimal-core-migration.md) supplies the combined C index,
signature-level operation inventory, C/Rust correspondence, record deletions,
shader/generated/secondary state rules and ordered C2 cutover. Every installed
operation is retained or mapped explicitly. C1 is complete for this foundation
revision; later hardware coverage and native backend acceptance stay separate.

The remaining work is runtime/compiler/consumer migration under C2, followed by
local C3 acceptance. Windows cross-queue, native failure and teardown checks remain
in the handoff, as do Metal's native mapping questions. No runtime support follows
from the draft's record and inventory checks.

C2 acceptance will check host ABI, generated layouts, lowering and affected Linux
consumers. Only uncertain native costs need targeted measurements. No fresh GPU,
new model download, DX12 emulation or per-feature benchmark campaign is required.

[root-costs]: https://learn.microsoft.com/en-us/windows/win32/direct3d12/root-signature-limits
[root-state]: https://learn.microsoft.com/en-us/windows/win32/direct3d12/using-a-root-signature
[descriptor-heaps]: https://learn.microsoft.com/en-us/windows/win32/direct3d12/descriptor-heaps-overview
[vulkan-heaps]: https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_descriptor_heap.html
