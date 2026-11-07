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
