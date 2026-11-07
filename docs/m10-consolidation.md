# M10 boundary and SDK consolidation

2026-11-07. M10 implementation and bounded Vulkan/Linux acceptance are complete
at clean `688ea240770b93256062d53af1c72b26386e57d6`, foundation revision 13.
[Receipt](results/m10-sdk-acceptance-2026-11-07.json). M11 implementation proceeds;
this does not close the explicitly listed native acceptance gaps below.

## Selected fundamental mechanisms

| Native requirement | Core expression | Policy and cost boundary |
| --- | --- | --- |
| Buffer/image placement and aliasing | Existing allocations/spans, explicit mixed flag, native origin/granularity queries | No suballocator, implicit split or alias activation |
| Existing device/resource use | Typed borrowed/adopted Vulkan handles and exact metadata | Cold CPU wrapper; no recreated storage, copied contents or resource registry |
| Linux external-handle sharing | Native FD transport composed with imports and external ownership barriers | No duplicate core FD broker or implicit ownership transition |
| Stable address-space reservation | Virtual memory kind and ordinary address/range operations | No physical pages until caller binds them |
| Sparse image storage | Existing image object plus queried blocks/tails/metadata | Native topology, no page-size assumption or hidden tile conversion |
| Bind/rebind/unmap | Counted sparse queue batches with explicit timeline/binary dependencies | One native call, caller scratch, no migration/copy/scheduling policy |
| Error recovery | Whole-call rejection or explicit UNKNOWN effect | No invented rollback guarantee, premature destruction or automatic retry |

The interface preserves native choices without offering redundant allocator or
streaming abstractions. Physical page lifetime, alias rights, total reservation
budget and ordering against GPU access remain caller proofs. The fully-bound
hint is an explicit promise for virtual storage, not inferred tracking. C and
native Rust share the backend; Rust does not introduce retention or hidden waits.
No implemented operation structurally forces an extra GPU copy, submission,
allocation or CPU payload roundtrip. This is a design audit, not universal timing
parity or evidence for unavailable native backends.

## Clean handoff evidence

The installed source bundle was moved into a fresh directory, built offline using
only its dependency graph and pinned Cargo cache, with no shader compiler and no
checkout/SDK shared-library dependency. All seven executable depfiles/linkages
pass. Headless compute/images, native producer/import consumption, Linux external
FD roundtrip, sparse buffer/image profiles and all four ray profiles execute.
Ray execution explicitly uses `RADV_EXPERIMENTAL=emulate_rt`; generated ray paths
are explicitly selected. The external FD run explicitly selects the documented
`OGPU_EXTERNAL_SYNC_CONTROL=1` fixture control. These are recorded environment
choices, not runtime fallbacks or raw external-path SyncVal acceptance.

Independent relocated C compute and graphics consumers pass runtime identity,
missing-library/error handling, guarded arithmetic, six image sizes and indirect
replays. The full C foundation regression also passes. Host coverage is 134 tests,
plus two C/direct-Rust correspondence tests; ordinary GPU unit coverage is 32 tests.
The header has 231 operations, 196 records and 419 constants; all C/Rust signatures,
1,917 surface values and 2,574 ABI layout values agree. Strict Clippy, shader/binding
reproduction and 22 tooling tests pass.

## Remaining acceptance obligations

- Metal/Windows mappings, other devices, cross-process transport, other external
  handle/format/OS profiles and multi-device peers remain separate lanes.
- The original FD path passes core validation/correctness; matching SyncVal lacks
  external timeline dependency resolution. The explicit control is clean, while
  the original path is not claimed SyncVal-clean.
- Sparse shader residency instructions, metadata-bearing images, 3D/MSAA,
  simultaneous aliasing, sparse descriptor heaps and the hinted permutation case
  remain unaccepted. The four bounded buffer/image profiles are accepted locally.
- The combined preprocessing/AS/table scene fails in a direct Vulkan reproducer
  as well as OGPU. Native compatible-type masks differ and are disjoint. Exact
  driver/emulation diagnosis remains open; OGPU neither forbids the composition
  nor secretly splits allocations.
- Ray emulation is diagnostic only. Native ray acceleration/performance is still
  unaccepted. No missing hardware is reported as passing evidence.

The [implementation brief](m10-memory-interop.md) owns precise contracts and
reproduction instructions. M11 now owns advanced compute/artifacts and plane-aware
media profiles, with independent acceptance for each capability.
