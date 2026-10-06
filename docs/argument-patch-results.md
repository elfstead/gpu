# Partial argument updates: native expressibility control

Accepted 2026-10-06 at clean **`f4405e3`**, ABI 19. Both local drivers pass the
bounded follow-up to the [bind-once comparison](argument-reuse-results.md).
This is correctness and command evidence, not a partial-update timing result.

## Evidence

Native full supply, native partial updates and current public full arguments use
the same accepted compute/vertex/fragment artifacts. Roots are 64/256 bytes.
Four scopes save A/A/B/A results; only the last scalar changes for the near draw
in B. The pointer and geometry stay unchanged. Every host root is immediately
poisoned after recording; public wrong-size calls reject before valid retries.

The native partial path initializes the complete graphics root before each
scope's first draw, because preceding compute work has overwritten the start
of the same bank. It then supplies no redundant A update, or exactly four bytes
at offset 60/252 for B. No implicit stage restoration or separate argument bank
is assumed. The loader records every actual offset/size update in order.

All **48 saved scope images, 96 indexed draw commands and 32 rejected-call
retries** pass across the two drivers and three paths. Independent interior
color/depth checks pass; all image, edge, depth and guard bytes match across
paths. Final generated geometry/index/indirect/guards also match the independent
oracle. Traces verify eight draws and four scopes per process, no counted draw
or queue-idle, and matched allocation sizes/types/peaks with zero final live
allocations. These diagnostic paths each retain ten GPU allocations, including
saved-output storage and the earlier fixture's unused alternate vertex buffer;
they are not minimum-memory or timing strategies.

Graphics-root traffic across four scopes, excluding four common 32-byte compute
updates:

| Root size | Native full supply | Native partial | Current public, after deduplication |
|---|---:|---:|---:|
| 64 bytes | 8 updates / 512 bytes | 5 updates / 260 bytes | 5 updates / 320 bytes |
| 256 bytes | 8 updates / 2,048 bytes | 5 updates / 1,028 bytes | 5 updates / 1,280 bytes |

Equal native update counts do not imply equal payload work. The public backend
could detect changed subranges and emit fewer native bytes, but its existing
call shape would still require the caller to supply the complete root and the
implementation to account for that newly supplied content. Explicit changed
ranges express information the current operation-local contract does not.

The original 8/64/256-byte pointer-plus-payload snapshot control also passes a
fresh Radeon regression at this revision. CPU tests cover single-word oracle
rejection and exact range/count corruption. No runtime/API behavior changed.

## Decision and next implementation

Select [recording-local byte-range argument updates](recording-arguments-prototype.md)
for the next prototype. Do not introduce a whole-block-only binding and later
assume it preserves the native strategy. Keep typed offline layouts and independent
executables; do not require a retained argument object merely to communicate
that unchanged values stay unchanged. Immutable argument objects remain a
possible higher-level convenience, not a prerequisite for this native mapping.

The prototype brief declares initialization, scope/stage behavior, caller-byte
snapshot timing, rejection and list ownership before implementation. It does
not freeze names or ABI, promise a particular speedup, or assign the whole
measured host gap to the existing contract. Vulkan's underlying range update is
documented in [VkPushDataInfoEXT](https://docs.vulkan.org/refpages/latest/refpages/source/VkPushDataInfoEXT.html).

[Committed receipts](results/argument-patch-2026-10-06/) preserve both complete
reports, the original-snapshot regression report and runner logs. Normalized
wrappers retain original report paths and SHA-256; binary outputs and raw native
traces remain at their hashed local paths. RX 5700 XT/RADV and llvmpipe are the
only tested implementations; no native Metal or second physical GPU is claimed.

Reproduce with `argument_patch.py --snapshots CLEAN_SNAPSHOT_REPORT` under the
matching ICD plus Vulkan/synchronization validation. Supplied native artifacts
are reused; no shader generation is necessary for this follow-up.
