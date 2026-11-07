# M10 memory and interoperability

Implementation starts 2026-11-07 on Vulkan/Linux. M9 is consolidated; native ray,
Metal and Windows acceptance remain separate. This brief owns implementation order,
not a claim that every row below is already implemented.

## Selected boundary

The existing allocation/span/image mechanisms remain authoritative. A linear
allocation may explicitly permit placed images when native requirements allow it;
this prevents dedicating the native allocation to its buffer. No separate mixed
allocator, tracked suballocation or implicit alias activation is introduced.
Expose the native buffer/image alias granularity and actual allocation-relative
origin, because GPU address over-alignment can introduce leading padding. Image
placement offsets are still relative to the logical OGPU allocation.

Native interoperability is an explicitly backend-typed unsafe boundary. Borrowed
native exports confer no ownership. Imported native objects state ownership and
immutable creation metadata; no registration of all reachable GPU addresses is
required. Sharing a device/queue requires the same native device and external host
synchronization. Cross-device/OS-handle sharing additionally requires matching
native identity, allocation compatibility, handle ownership and explicit GPU
synchronization. Importing a handle does not transition layouts or infer hazards.

## Ordered implementation and acceptance

1. Mixed image/linear backing, placement limits and allocation-origin query;
   exercise disjoint use and discarded alias reuse with synchronization validation.
2. Typed native device/queue/resource/synchronization handoff and explicit ownership;
   use a native producer with OGPU consumption and no CPU payload transfer.
3. Linux external memory/semaphore handles, exact native capability queries and
   identity; exercise export/import ownership, errors and zero-copy GPU handoff.
4. Sparse/virtual resources and counted binding submissions with explicit completion;
   expose only native primitives, never emulate remapping by copying or migration.
   Keep reserve/address/mapping constraints visible; other backends may expose
   stronger independent facilities. Exercise map/rebind/unmap and bounded reuse.
5. Reproduce M9's combined PREPROCESS/AS/table usage with a direct native control,
   then consolidate the contract, SDK and remaining native acceptance obligations.

C and native Rust advance together. The structural performance criterion is no
required extra GPU operation, allocation, submission, CPU readback or policy when
the native primitive can express the requested work directly. Cold wrapper
validation and caller translation scratch do not establish universal parity.

Unproven constraints include external image metadata/modifiers, cross-process and
cross-vendor handles, native sparse topology and residency behavior, and the M9
emulation anomaly. Multi-device peer execution remains its separate roadmap track.
M11 proceeds after this implementation/consolidation; unavailable hardware is
recorded per profile rather than treated as a successful test.

## Mixed placement tranche — implemented

`MEMORY_ALLOW_IMAGES` selects non-dedicated linear backing using the existing
allocation operation. Mandatory native dedication rejects that request. Opaque
and explicitly dedicated-image paths remain usable. `QUERY_PLACEMENT_LIMITS`
exposes alias granularity; `memory_info_query` exposes the allocation-relative
logical origin. Image binding now translates that origin rather than assuming zero.
No new allocation kind or hot-path resource registry is introduced.

The independent native Rust consumer now executes six image phases: three with
explicit dedicated image memory, then three with mixed backing. Each mixed phase
fills/copies a disjoint linear range, overwrites the image's buffer alias, issues
an explicit global dependency and discard transition, clears the image, and checks
all 64 RGBA pixels plus guards. Over-aligned allocation is requested; a host test
separately forces a nonzero prefix so alignment correctness does not depend on
the driver's chosen address. Local RADV passes synchronization validation.

[Native aliasing rules](https://docs.vulkan.org/spec/latest/chapters/resources.html#resources-memory-aliasing)
remain caller obligations. Arbitrary image-byte preservation through a buffer alias
is not promised. The next tranche is typed native interoperability.

## Native handoff tranche — implemented

Typed Vulkan exports/imports now cover devices, queues (borrowed export), memory,
images, timelines and binary semaphores. Discovery may borrow an external instance;
a device may be borrowed or adopted. Resource adoption is component-specific:
allocation and buffer ownership are independent. Ownership transfers only after
successful validation/construction; imported images retain caller-specified state,
with no transition or implicit synchronization. Ordinary destruction releases the
wrapper and only the explicitly adopted native objects, without waiting.

Linear imports distinguish allocation and buffer offsets. A buffer-only import
needs no `VkDeviceMemory` registration, mapping or allocation query. Mapping uses
the allocation origin; native address commands use the buffer origin. These
imports currently promise fully bound, nonsparse objects and exact metadata.
The memory behind an imported image remains the native caller's responsibility.

The public `native` Rust consumer creates buffer/image/semaphore objects through
Vulkan, imports them through a separately borrowed discovery/device, records a
native producer and OGPU consumer, and hands off with timeline plus binary signals.
It checks 32 words, 64 pixels and guards. Buffer and allocation origins deliberately
differ; a buffer-only wrapper is destroyed/reimported before consumption. Imported
resource ownership is adopted, native image memory stays separately owned, and the
original device remains usable after destroying its borrowed OGPU wrapper. Local
RADV synchronization validation passes without a CPU producer-to-consumer transfer.
Final readback is solely an assertion of results. This is same-device sharing;
it does not yet establish external OS handles or peer-device execution.

### Native device creation contract

Imports assert a native Vulkan 1.4-capable device and the same mandatory enabling
as the normal constructor: `storageBuffer16BitAccess`, `bufferDeviceAddress`,
`timelineSemaphore`, `synchronization2`, `maintenance4`, `maintenance5`,
`descriptorHeap`, `deviceAddressCommands`, and `shaderUntypedPointers`. The three
extension names are `VK_EXT_descriptor_heap`, `VK_KHR_device_address_commands`,
and `VK_KHR_shader_untyped_pointers`. Queues use ordinary, unprotected creation.
Optional OGPU IDs assert the corresponding native enabling in the
[constructor](../crates/ogpu/src/foundation/native.rs); extra native features and
extensions may coexist without an OGPU abstraction. Import cannot introspect
already-enabled native features, so inaccurate declarations violate its unsafe
contract. A borrowed instance must belong to the selected Vulkan loader and outlive
all resulting children. Its API ceiling and surface-extension flags are explicit.

### External-handle composition to verify next

The native boundary may already express platform-specific external memory and
semaphore transport without additional core variants: enable native extensions
when creating the caller-owned device, query exact handle capabilities/identity,
export/import OS handles natively, then wrap the resulting objects once. The next
consumer must demonstrate the zero-copy synchronization/ownership path before this
is accepted as the M10 external-handle disposition. This is a proposed composition,
not evidence that FD, Win32, modifiers or cross-process profiles already pass.

## External Linux handles — implemented composition

The initial transport profile is Linux `OPAQUE_FD` device memory plus persistent
timeline semaphores, on one physical GPU/driver. Two independently created native
logical devices share one allocation and one semaphore payload. The native producer
fills the allocation, releases ownership and signals; OGPU acquires, copies the
original contents, overwrites the same bytes, releases and signals; the native
producer reacquires and copies the result. All 128 result words and guards pass.
Only the final assertion reads bytes on the CPU. Each native device is explicitly
retired before teardown. Both devices are adopted through the existing native
boundary; no second OGPU device-creation/extension-selection path is introduced.

The composition exposed a missing core primitive: `DOMAIN_EXTERNAL` is now an
endpoint in the existing memory/image ownership barriers. The recording domain
must still be one endpoint; external-to-external is rejected before native recording.
No separate external-barrier operation, implicit layout transition or ownership
tracker is required. Resource and semaphore transport remain native calls in the
consumer, followed by one cold OGPU wrapper import. A generic allocator/handle
broker can build on this without extra GPU work or mandatory CPU payload copies.
This is the selected disposition for platform-specific transport; it does not
claim that native escape hatches replace portable shader/execution primitives.

Exact buffer usage and timeline handle capabilities are queried before use, and
native device/driver UUIDs are recorded. Opaque imports preserve the exported
allocation size, memory type, device-address flags and dedicated-buffer association.
OS FD ownership uses `OwnedFd`: an unsuccessful native import retains the FD for
normal cleanup, and only success relinquishes it to Vulkan. The allocation's native
reference survives closing a duplicate FD. Core wrapper metadata rejection likewise
never transfers native destruction authority.
[Memory FD contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkImportMemoryFdInfoKHR.html),
[semaphore FD contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkImportSemaphoreFdInfoKHR.html).

### Validation boundary

The original external path passes core validation and produces the correct data.
Synchronization validation 1.4.357 reports one READ_AFTER_WRITE on the final native
read, referring to the original native write. Inspection of the matching
[validation source](https://github.com/KhronosGroup/Vulkan-ValidationLayers/blob/e4786f7ce8f1319215eff0d938f4be4651cbb85d/layers/sync/sync_submit.cpp#L710)
shows that an unresolved external timeline wait is assumed satisfied without
resolving/importing the other device's signal dependency. This supports attributing
the diagnostic to missing external dependency tracking; it is not a claim that
validation proves the whole external-memory graph.

`OGPU_EXTERNAL_SYNC_CONTROL=1` explicitly adds a local native write-to-read
barrier to the fixture's return path. That control passes synchronization validation
with zero diagnostics. It is never selected automatically, is printed in results,
and adds no OGPU runtime policy. The original path remains available and separately
passes core validation. The SDK's optional `--external` check records this environment
and fails on any validation messages; it does not filter the known message.
[Receipt](results/m10-external-fd-2026-11-07.json).

This accepts the bounded opaque-FD composition, not cross-process transport,
DMA-BUF/modifier images, foreign-driver domains, Win32 handles or multi-GPU peer
execution. Those handle/format/OS profiles need their own capability and native
acceptance lanes. Plane/modifier requirements intersect M11's media image work.
Sparse/virtual resource mapping is implemented below.


## Sparse/virtual tranche — implemented

Three operations complete the native storage mechanism: an image block/tail query,
a counted binding-scratch query and a queue binding call. `MEMORY_VIRTUAL` reserves
buffer address space without physical allocation. The existing opaque allocation
can request address-capable backing; existing spans select physical pages. Sparse
image flags select opaque binding or tiled residency. Ordinary requirement queries
still return compatible memory types, alignment and reserved size. Separate sparse
image records expose granularity, mip tails, array strides and metadata.

Binding submissions preserve caller batch boundaries and use caller storage in one
`vkQueueBindSparse` call. All translation/validation precedes that call. Timeline
and binary arrays are direct inputs; no bridge submission, resource registry,
copy-based remapping or retention graph exists. Local rejection permits retry;
native failures conservatively report UNKNOWN for the entire call and poison the
device. The queue-submit OOM rollback guarantee is not inferred for this command.
Caller policy owns reservation budget, physical placement, alias rights, ordering,
eviction, migration and retirement. Native external backing requires matching
resource handle declarations. Sparse binding does not perform queue ownership or
resource-content visibility/layout transitions.

The public consumer verifies stable addresses through page permutation, two copies
of three 64-KiB pages, a real hole with reads of the remaining resident pages,
whole-call rejection/retry, and explicit unmapping. Image tests cover both opaque
binding and tiled residency across two array layers and nine mips. Queried 128×128
blocks and separate per-layer mip tails produce 130 checked pixels per profile,
with all surrounding readback bytes preserved. Four profiles pass on local RADV
with zero validation/synchronization diagnostics. The SDK checker exposes these
optional native profiles through `--sparse`.

`MEMORY_FULLY_BOUND` is an explicit promise about every address-command range
accessed through a virtual allocation. It enables the native optimization hint;
OGPU never infers it by tracking page state. An early fixture using that hint
reported VUID 13097 after permutation despite correct output. The matching VVL
source checks current bound ranges during recording; precise attribution remains
open. The accepted fixtures use the ordinary unhinted sparse path. This does not
establish acceptance of the hinted permutation case or a performance difference.
The hint remains available rather than being removed or enforced by a runtime
residency walk. Native residency-instruction shaders, metadata-bearing images,
3D/MSAA sparse profiles, simultaneous aliasing, sparse descriptor heaps and the
hinted permutation case retain separate acceptance obligations. No unbound-read
zero-value assertion is inferred from testing reads of resident pages.

[Native sparse contract](https://docs.vulkan.org/spec/latest/chapters/sparsemem.html),
[local receipt](results/m10-sparse-2026-11-07.json).
The remaining M10 work is the direct-native M9 combined-usage reproducer and clean
SDK/contract consolidation; M11 follows without waiting for another GPU.
