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
