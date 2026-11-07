# M9 boundary and handoff consolidation

2026-11-07. The scheduled Vulkan/Linux ray boundary and diagnostic consumer are
implemented and handed off at clean `ea0f268b0bdfe3127518dd839efcfd1c45c8454b`.
[Receipt](results/m9-sdk-acceptance-2026-11-07.json). This closes implementation and
local diagnostic consolidation; **native ray acceleration is not accepted** on
this RX 5700 XT. M10 proceeds independently of that unavailable hardware lane.

## Fundamental boundary review

| Native choice | OGPU expression | Cost/obligation |
|---|---|---|
| Size query and placed AS interpretation | Requirements plus explicit backing span | Cold native query; no hidden geometry/storage allocation |
| Independent builds in one native batch | Counted build array, caller host/GPU scratch | One native command; no inferred ordering or command splitting |
| Update, clone, compact | Explicit source/destination/mode and property query | No readback policy, rebuild or retained scene state |
| Native serialization | Explicit bytes and version compatibility | Native format; caller maintains BLAS references and lifetime |
| Inline query without pipeline | Shader capability and AS address in ordinary arguments | No descriptor registry, pipeline feature or shader-Int64 requirement |
| Ray pipeline groups and records | Existing executable/arguments plus group metadata and caller tables | Compilation stays cold; no shader rewriting or hidden record builder |
| Stack control | Per-group native queries and explicit dynamic state | Application computes sufficient stack; static native policy also expressible |
| Direct/indirect/generated trace | Separate native forms and independently queried profiles | No CPU expansion, hidden scheduling or readback |
| Reused or shared shader records | Caller address/size/stride, including zero stride | No imposed table uniqueness, allocation or traversal of reachable entries |

Recording validates supplied local structure and writes caller translation scratch.
GPU address validity, geometry contents, resource/record lifetime, alias safety,
update compatibility, stack sufficiency and indirect bounds remain trusted caller
obligations. Rust tokens are nonowning and unsafe; they do not add reference counts,
retention walks or deferred deletion. C and Rust share the same backend operations.
The structural review identifies no forced extra native operation/allocation in
these implemented paths. It is not a universal performance proof or a substitute
for native ray hardware measurements.

## Executed evidence

The public Rust consumer covers triangles and procedural AABBs, each through inline
query and ray pipelines. Six states × four rays × four profiles verify 96 results,
including two geometry IDs, instance/primitive identity, distance, any-hit execution,
shader-record data, guards and fresh output sentinels. Two independent BLAS builds
share one native batch with distinct scratch. Update, clone, compaction and native
serialization/version compatibility are reused for traversal. Ray pipelines also
execute direct, dimensions-indirect, table-indirect and generated integrated/explicit
preprocessing paths. Preprocessing buffers are explicitly allocated by the consumer.

Clean SDK source was relocated, rebuilt offline from fresh targets without checkout
inputs or a shader compiler, and all four profiles rerun with synchronization
validation. C compute/graphics consumers independently verify relocated header,
shared-library identity and failure handling. Public C ray signatures/layouts are
covered by the surface/ABI audit; ray GPU execution in this handoff uses direct Rust.
Normal local RADV exits 77 for missing ray support; no fallback is installed.

## Open evidence and follow-on assignments

- Native ray acceleration/performance and native Metal/DX12 mappings remain open.
- Native indirect AS builds and generated ray **pipeline selection** are not
  advertised locally. Host contracts are implemented; execution is unaccepted.
- The [M10 direct-native reproduction](m10-memory-interop.md#combined-preprocessingray-storage--native-reproduction-complete) reproduces the combined PREPROCESS/AS/table failure without OGPU resource/command lowering. Driver/emulation diagnosis and acceptance remain open. Separate caller preprocessing allocations pass; OGPU does not silently split or forbid the combination.
- M11's artifact audit covers ray libraries, capture/replay and pipeline binaries.
  Vendor motion/micromap/cluster/primitive families remain explicit native capability
  candidates, not a claim that ordinary triangles/AABBs express every hardware unit.
- Cross-device serialized data, untrusted shader safety, recovery, multi-device peer
  execution and additional hardware remain separate acceptance tracks.

The next implementation batch is M10 memory and interoperability. No missing native
hardware is treated as a reason to stop implementing that independent boundary.
