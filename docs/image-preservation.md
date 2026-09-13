# Image preservation checkpoint

Implemented 2026-09-13 at `a835f19`, first checkpoint of the
[ownership review](image-ownership-review.md). The draw signature gained CLEAR / LOAD
in **ABI 2**, rejecting old clients at probe creation without a compatibility branch.
The [independent-heap follow-up](descriptor-heaps.md) is now implemented at ABI 3.
Rebuild callers with the current matching header/library; the evidence below records
the preservation checkpoint, not the later heap interface.

An image initialized by an accepted producer may be read/written in subsequent ordered
submissions without another discard. Copies no longer require same-batch initialization.
Initialization, valid texels and explicit dependencies remain caller obligations; no
mutable layout tracker or hidden initialization submission was added. Failed/abandoned
producer batches do not authorize later reads.

CLEAR retains opaque-black clear behavior. LOAD preserves existing texels, skips the
discard transition, and requires dependencies to COLOR_READ | COLOR_WRITE. Both store
contents at the end. Loading does not enable blending or attachment feedback.

Local evidence with llvmpipe and Vulkan/synchronization validation:

- The six-size heap-image example now spans three submissions with no intervening
  wait, followed by one explicit final wait and exact processed/final pixel checks.
- `gpu_image_preservation` checks explicit recovery from rejected first initialization,
  abandoned/rejected discards over existing data, LOAD preserving background probes,
  CLEAR resetting them, and copies from a separate submission. Covered pixels are red.
- All 11 execution tests, the existing image-loop regression and 691 ABI checks pass.

Independent heaps complete the second implementation checkpoint. Physical-GPU and remote
execution-CI coverage remain pending; this is not a performance or portability result.
