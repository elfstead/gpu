# M9 ray-tracing implementation brief

2026-11-07. Implementation work starts after the [accepted M8 checkpoint](m8-consolidation.md).
This document selects the batch; descriptions below are targets until accompanied
by an implementation and acceptance receipt.

## Selected boundary

Acceleration structures are opaque native interpretation objects over explicitly
provided backing. They do not allocate geometry, instance arrays, scratch or
shader tables. Requirements queries describe storage/build/update sizes and
alignments. Creating an object does not build it; destroying it never waits.
Build/update commands accept caller geometry/range arrays and caller host scratch
for translation. Native batch boundaries remain expressible. Applications own
build scheduling, geometry validation, update compatibility and scratch aliasing.

Use the foundation's required device-address command profile:
`vkCreateAccelerationStructure2KHR` consumes a backing address range. Do not add a
legacy buffer-handle construction path. Geometry inputs and shader-visible AS
addresses are ordinary device addresses. Address validity, native input/storage
usage and asynchronous lifetime are explicit caller obligations. This permits
future foreign-address work without requiring an OGPU geometry registry. Backing
initially uses existing placed memory; M10 owns typed external provenance/import.

The operation set is requirements, create/destroy/address, batched build/update,
batched indirect builds when available, clone/compact, property query writes,
serialization/deserialization and compatibility query. Query outputs use existing
query pools and resolve operations. Build, copy and shader accesses get explicit
native synchronization scopes. No CPU traversal, implicit builds or retained scene
graph. Triangle, AABB and instance inputs are independent tagged geometry data;
instance packing is a documented shader/native ABI, not an allocator.

Ray queries are a shader capability usable in compute and supported raster stages,
independent of ray pipelines. A Slang `RaytracingAccelerationStructure(uint64_t)`
can consume the AS address through existing argument bytes or pointer data. Check
this against the pinned compiler. Do not introduce a second descriptor or root
mechanism just to pass an AS.

Ray pipelines use existing executable preparation, shader artifacts and argument
layouts, extended with native shader groups, recursion limits and explicit shader
record data. Group-handle queries fill caller memory; applications lay out tables
using queried handle/record/base alignments. Direct trace, dimensions-only indirect
trace and full table+dimensions indirect trace retain their independent native
capabilities. No table builder, shader-local allocation or automatic stack policy.
Expose native stack size queries/dynamic selection where the driver supports it.
Generated ray tokens reuse the generated-command mechanism when supported.

## Capability and verification boundaries

AS, inline ray query, ray pipelines, indirect build, each indirect trace form,
maintenance copy/query behavior and primitive-culling shader permission remain
independently queried/enabled with explicit prerequisites. M9 is not a baseline
device requirement. Private capability storage now spans 256 bits; the public ABI
continues using counted opaque IDs.

Normal RADV on the local RX 5700 XT does not advertise the ray extensions. Raw
feature-chain booleans alone are insufficient: require the relevant extension as
well. `RADV_EXPERIMENTAL=emulate_rt` provides a possible **explicit diagnostic** lane.
It is not hardware ray acceleration or performance acceptance, and OGPU must never
set this mode or silently substitute software traversal. The native hardware lane
remains open even if that diagnostic consumer passes.

The deterministic consumer will build a BLAS/TLAS, check hit/miss, distance and
instance/geometry/primitive identity against a CPU reference, update geometry,
compact and reuse the result. Where available it runs both inline queries and ray
pipelines, direct and indirect dispatch, with synchronization validation and guards.
Malformed addresses/alignment/counts, unsupported independent profiles, shader-table
layout and resource lifetime constraints need focused checks. Native dispatch
translation must preserve caller ordering and contain no hot allocation.

Native Metal/DX12 mappings, physical ray acceleration, vendor motion/micromap/cluster
families and cross-device serialized-data compatibility remain separately assessed.
A serialized blob is native/version-specific, not an OGPU portable asset format.
Host-side AS build operations deprecated by the pinned native registry are not a
second core build runtime; CPU scene preparation remains application work.

References: [native AS operations](https://docs.vulkan.org/spec/latest/chapters/accelstructures.html),
[address-based creation](https://docs.vulkan.org/refpages/latest/refpages/source/vkCreateAccelerationStructure2KHR.html),
[ray maintenance](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_ray_tracing_maintenance1.html),
[Slang AS address constructor](https://docs.shader-slang.org/en/latest/external/core-module-reference/types/raytracingaccelerationstructure-0am/init.html).
The repository's pinned registry and valid-usage data govern the implementation.

## Acceleration/query tranche — implemented 2026-11-07

The C and direct Rust surfaces now implement requirements, address-backed objects,
batched direct/indirect builds, updates, clone/compact, property query writes and
serialization/compatibility. Independent native capabilities are extension-gated;
query-only devices do not enable ray-pipeline or shader-Int64 features. Native
bindings for the upcoming pipeline commands are generated, but their public
execution operations are still pending.

[The consumer](../examples/foundation_ray/main.rs) checks five scene states:
original, z-updated, preserved clone, compacted clone, deserialized clone. Four rays
per state verify hit/miss, distance, instance ID 37 and primitive ID 0, plus guarded
output. A queried compacted size can equal the original size; this is not evidence
of a memory-saving ratio. Serialized native version compatibility passes locally.
Five malformed recordings exercise scratch capacity/alignment, missing/overlarge
ranges and missing update source, followed by arena reuse. The current fixture is
one triangle, one instance, one geometry; multiple geometry identities, AABBs,
indirect builds and true native hardware traversal remain unaccepted.

The first diagnostic exposed a pinned compiler issue. Slang 2026.14.1's built-in
uint64 AS constructor emits `SPV_KHR_ray_tracing` even for a query-only artifact.
The fixture instead uses a source-level `spirv_asm` conversion from two u32 words,
which the [ray-query extension itself defines](https://github.khronos.org/SPIRV-Registry/extensions/KHR/SPV_KHR_ray_query.html).
This avoids both the unnecessary pipeline-extension declaration and Int64. It is
fixture/toolchain glue, not runtime rewriting or an expanded OGPU feature dependency.
The [pinned constructor source](https://github.com/shader-slang/slang/blob/v2026.14.1/source/slang/hlsl.meta.slang)
records the compiler's current choice; reassess when updating Slang.

[Receipt](results/m9-acceleration-query-2026-11-07.json): 126 host tests,
32 normal native tests, full C regression and the explicit emulation diagnostic
pass; ABI/surface/Clippy checks pass. No hardware ray-performance claim is made.
