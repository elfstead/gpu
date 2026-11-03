# C1 D: one descriptor storage model

Selected design, 2026-11-03. **Not implemented.** Resolves decision D of the
[minimal-core contract](minimal-core-contract.md). The [C sketch](drafts/ogpu_descriptors.h)
and [Rust record mirror](drafts/descriptors.rs) are uninstalled draft artifacts.
Vulkan/Linux implements first; the DX12 branch remains a source/mapping reference.

## Decision

A descriptor heap is one explicitly created storage object with locations,
independent writes/copies, and binding. Its contents have one authoritative native
representation. An available memory view exposes those same contents; it never
exposes recipes or a shadow that must later be materialized.

Locations use native storage units: bytes on the current Vulkan path, descriptor
slots on the D3D12 mapping. Query size/alignment/array stride for each descriptor
description. Do not pad every Vulkan buffer descriptor to the largest image size,
and do not present D3D12 handle increments as application-writable descriptor bytes.
Native D3D12 handle arithmetic stays inside the backend. [Handle increments][dx-increment]

One storage abstraction does not mean every device can place it in arbitrary
memory. Creation explicitly identifies backing that must either be borrowed or
natively allocated. That is a real placement distinction, not three allocators
or three descriptor systems. Slot allocation, caches, upload rings and transient
tables remain user code using the same locations and write operations.

## Records and location arithmetic

| Record | Meaning |
|---|---|
| `heap_request` | Resource/sampler kind, backing source, exact requested usage and capacity in native units |
| `heap_requirements` | Byte-backed storage size/alignment, bytes per location unit when accessible, reserved interval requirements and minimum memory usage |
| `heap_desc` | Request plus actual borrowed memory/host spans and reserved interval |
| `descriptor_shape` | Size, alignment and native typed-index array stride, all in location units |
| `heap_location` | Heap plus offset in its units; not a shader index, pointer or portable serialized ID |
| `descriptor_copy` | Source/destination locations and count of units to copy on the CPU timeline |

Resource/sampler shape queries take the actual descriptor description. This avoids
assuming that every future image format or conversion uses a single fixed-size
entry. A supported shape has nonzero size, alignment and array stride. Unsupported
format/resource/descriptor combinations reject; the query does not create a view,
write a descriptor or allocate backing. Descriptor shapes for a device/heap kind
use the same unit convention regardless of its backing source.

A location is valid if its descriptor extent fits capacity, satisfies alignment
and avoids the reserved interval. Capacity includes that interval. Reserved size
and alignment are queried in units; the caller chooses its offset. Integer
products/sums are checked before native operations. Shape size is the copied/written
descriptor extent; array stride is the native typed-index addressing step and may
include padding. Never infer either value from a host resource handle's size.

For a concrete *illustrative* layout, assume Vulkan reports 16-byte buffers,
32-byte images and matching alignments. Two buffers followed by one image occupy
64 units at offsets 0, 16 and 32. The corresponding D3D12 layout occupies three
slots at offsets 0, 1 and 2. These are not measured device values; they show why a
mandatory maximum-sized common slot loses compact native layouts.

The shader artifact declares its indexing ABI. For the current typed heap-array
path, a descriptor at location `L` is addressed by index `L / array_stride`, only
when exactly divisible and representable in the artifact's index width. Generate
that value when constructing CPU data or producing GPU indices; OGPU does not
insert a shader-side conversion or descriptor lookup table. A raw-location shader
ABI may consume offsets directly where supported. Bounds/alignment for writing
and representability by a particular shader access path are different checks.
The existing [mixed-heap shader](../examples/shaders/foundation-heaps.slang) already
uses type-specific indices; [compiler validation](../examples/compiler/heaps.py)
checks native descriptor-size strides. Update those interfaces together in C2.

## Backing and capabilities

Creation requests exact usages: shader visibility, CPU descriptor writes, CPU copy
source and CPU copy destination. An unsupported combination fails at requirements
query/creation. It does not trigger fallback allocation. The immutable info query
returns the descriptor and requirements, including the actual borrowed spans.
Capacity is nonzero; unknown kinds, storage values, usage bits or nonzero flags
reject. All spans are trusted valid caller references with checked extents. Queries
leave outputs unchanged on failure; creation publishes no heap on failure.

| Backing source | Lifetime and placement | Vulkan-first mapping / DX12 constraint |
|---|---|---|
| `HOST` | Caller-owned ordinary host bytes; no GPU binding | Vulkan CPU staging without a GPU allocation; unsupported as raw borrowed storage on D3D12 |
| `MEMORY` | Caller-owned GPU span; optional host span must be its actual live mapped interval | Vulkan explicit placement, host-visible or device-local; unsupported as arbitrary placement for current D3D12 descriptor heaps |
| `NATIVE` | Native heap allocated at explicit heap creation and released at destruction | D3D12 CPU staging or shader-visible heap; Vulkan convenience allocation is unnecessary and is not selected |

For `HOST`, memory is empty and host span covers requirements. Shader-visible usage
is invalid. For `MEMORY`, the span covers the queried size/alignment/usage; CPU
operations require a matching, sufficiently large live mapping supplied in `host`.
No implicit map/flush/invalidate occurs. For `NATIVE`, both spans are empty and
byte size/unit are zero: internal allocation/accounting is not an accessible byte
representation. Backing alignment is one and reservation size zero when inapplicable.
Descriptions with unused nonempty spans reject rather than silently ignore them.
The bound MEMORY range is exactly the requested capacity times `unit_bytes`, even
if the supplied backing span is larger. Bytes beyond that range are not heap
locations. The supplied host interval begins at the same backing-relative origin.

Requirements do not choose a memory type or allocate. Heap creation may allocate
its small wrapper/native object; subsequent binding/writes/copies do not allocate
OGPU storage. Borrowed backing/mappings outlive the heap and all recorded/future
uses. Destruction never waits. A heap does not own resources referenced by entries.
Repeated binding borrows the existing heap; it does not snapshot its contents.

For accessible storage, `unit_bytes` translates locations to actual byte offsets.
On current Vulkan it is one. `heap_info` exposes the borrowed spans already used
by the heap, so a separate `heap_memory_view` operation is unnecessary. Host reads
and ordinary memory operations retain their mapping/usage/coherency obligations.
Native opaque heaps expose no bytes; a handle increment is not permission to map,
dereference or copy native driver data.

## Writes and CPU copies

Writers accept descriptor descriptions and destination locations, with caller-owned
scratch. They validate the complete batch before native side effects. No persistent
type/ownership table is maintained per slot. Invalid input leaves destinations
unchanged; a native failure may have partially written them. Input arrays and
scratch live only for the call; copied descriptor contents remain in the heap.
Uninitialized/invalid descriptors must not be consumed by shaders.

CPU copying takes unit ranges in heaps of the same device and heap kind. The
source requires `CPU_COPY_SOURCE`, the destination `CPU_COPY_DESTINATION`.
All source and destination ranges must be valid and mutually nonoverlapping,
including aliases through different heap wrappers. Caller proves unknown aliases;
local validation checks identities/ranges it has. No implicit `memmove` ordering
or temporary copy is promised. A zero-count batch is a no-op; zero-sized ranges
have no memory effects after object/kind validation.

**D3D12 shader-visible heaps cannot be CPU-copy sources.** A caller that needs a
CPU source keeps an explicit non-shader-visible heap and copies chosen entries
into a shader-visible heap. That source is user policy; OGPU neither allocates nor
maintains it automatically. Direct writes into a shader-visible heap remain useful
when no copy-source cache is wanted. [D3D12 copy contract][dx-copy]

On Vulkan, host-accessible unit ranges can use ordinary native descriptor bytes;
copies do not need a retained CPU mirror. Reads from write-combined memory may be
an unattractive caller choice, not a reason to silently duplicate every heap.
Resource and sampler native batches may require different translation scratch;
the draft exposes one requirements query keyed by operation and count.

Disjoint locations can be updated concurrently while unrelated GPU reads are
pending, subject to native descriptor and cache-maintenance granularity. The
application orders conflicting descriptor reads/writes and keeps referenced
resources alive. A CPU copy's return establishes CPU completion, not GPU visibility
or completion; flushes and dependencies remain explicit.

## GPU movement and reserved storage

For `MEMORY` heaps, GPU copies or shader writes use the same public memory span
and ordinary memory commands. Actual backing usage/capabilities must permit each
operation. There is no descriptor-specific GPU scheduler or CPU readback fallback.
Device-local upload is a user composition: encode into host staging bytes, place
them in a caller-chosen transfer source, record a copy to heap memory, then declare
the copy-to-descriptor-read dependency. Mapped staging memory can avoid a separate
host copy. This is not claimed to work on opaque D3D12 heaps. [Vulkan storage][vk-heaps]

The bytes remain implementation-defined: move valid encodings from the same device;
do not fabricate or arbitrarily edit them. Stable descriptors plus GPU-generated
indices are another user algorithm; they are not the same as GPU descriptor mutation.

Reserved regions are inaccessible to descriptor writers, CPU copies and application
GPU work. On the current Vulkan baseline they remain protected until every
referencing recording has been reset/destroyed, even after its last submission
completed. Cache operations must not race native reserved-region accesses. Existing
reservation overlap/lifetime constraints survive the move from a borrowed binding
record to a heap wrapper; no global allocation/alias registry enforces them.

## Binding and argument compatibility

`bind_heap(encoder, heap)` binds that heap kind and preserves the other kind.
Cost is independent of capacity and initialized descriptor count. Vulkan emits
the native heap bind. D3D12 can remember the two currently bound native handles
and supply the required pair to its native binding command. This fixed-size
bookkeeping is not a copy of descriptors or retained resource ownership.

The first Vulkan/DX12 mappings use native direct heap indexing. Binding never
writes user argument bytes, iterates initialized locations, creates descriptors,
or repairs stale argument interfaces. An executable's prepared artifact must
declare compatible heap access. Backend-private heap binding slots, if needed by
a later native artifact ABI, must be reserved during preparation and disjoint
from the user coordinates selected in [A/B](minimal-core-arguments.md). Any native
invalidation must be declared; it cannot be hidden behind argument restoration.

No implicit heap rebinding occurs on draw/dispatch. Secondary/generated/native
interop execution must declare heap inheritance/invalidation, as for argument
state. This decision does not grant all such paths preservation merely because a
heap wrapper remains alive.

## Migration and native checks

| Current surface / behavior | Selected replacement |
|---|---|
| Raw host-output arrays in descriptor writers | `heap_location` destinations in explicit HOST/MEMORY/NATIVE storage; same resource/sampler descriptions |
| Caller `heap_binding` record at every bind | Cold immutable heap description; bind the created heap object |
| Opaque bytes plus raw GPU span as separate responsibilities | One authoritative storage object; info exposes actual borrowed spans where supported |
| Whole-heap copy/mirroring assumptions | Explicit CPU range-copy rights, ordinary GPU memory operations only when available |
| Fixed portable descriptor index | Native-unit locations plus queried shape and artifact-specific typed indices |
| DX12 recipe bytes materialized during binding | Native descriptor writes/copies at requested locations; constant-capacity bind work |
| Proposed separate memory-view operation | Derive the view from immutable heap info; no additional primitive |

Vulkan checks for C2: mixed buffer/image packing, native resource/sampler encoding,
host staging and device-local upload, post-record/pre-submit mutation, changed-entry
replay, disjoint updates with pending work, reservation/cache alignment and no
descriptor-write calls during binding. Keep native artifact indexing and written
locations consistent; verify output as well as traces.

Windows later checks the native descriptor source restriction, direct writes,
explicit staging copies and native heap bindings. No hidden mirror is permitted
to make shader-visible sources appear supported. Source inspection and record
checks here do not close DX12 review R2 or establish Windows acceptance.

The shader-visible index representation is an artifact ABI, not a guarantee that
the same serialized argument bytes work on every backend. User structures and
generated accessors can remain shared while values/layout requirements change
explicitly. This design retains the useful native strategies without making all
backends simulate byte-addressable heaps.

## Record check, 2026-11-03

`python3 tools/check-c1-drafts.py` now checks both argument and descriptor drafts,
including the borrowed span records imported from the current C header. `CC` and
`RUSTC` select explicit compiler executables when needed. GCC 16.2.0 and Rust
1.98.1 on Linux x86-64 pass C11/C++17 syntax and all 88 size/alignment/member-offset/
constant comparisons. This is record consistency, not backend or shader acceptance.

[dx-increment]: https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12device-getdescriptorhandleincrementsize
[dx-copy]: https://learn.microsoft.com/en-us/windows/win32/direct3d12/copying-descriptors
[vk-heaps]: https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_descriptor_heap.html
