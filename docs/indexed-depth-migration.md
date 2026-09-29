# ABI 18: scoped indexed/depth implementation checkpoint

Implemented 2026-09-29 from the [candidate proposal](indexed-depth-proposal.md).
This is a runtime/caller migration and focused correctness checkpoint, **not**
acceptance of the full matched scene, graphics performance or M4. The accepted
[native reference](graphics-scene-native-results.md) remains the comparison target.

## Implemented shape and upgrade

- `ogpu_buffer_create` now takes `OgpuBufferDesc`; old size/placement callers
  supply those fields with `extra_usage=0`. INDEX is opt-in, retains ordinary
  address/copy/indirect use, and adds native index usage only to that allocation.
- `ogpu_raster_create` takes `OgpuRasterDesc`: root size, topology, color/depth
  formats, test/write/compare state. Existing color-only callers select NONE,
  test/write zero, compare ALWAYS. No per-frame pipeline preparation is introduced.
- Begin/end rendering own independent color/depth load/store/clear choices.
  Draws no longer carry an attachment or load operation. Both indexed and
  non-indexed draws share this path; the old attachment-bearing encoder is removed.
- Initialize image layout explicitly with discard before first use. CLEAR now
  only defines values. The scope adds no hidden data barrier; explicit access
  dependencies order prior/later uses. Multiple compatible draws stay in one scope.
- UINT16/UINT32 indexed draws retain a checked buffer subrange and a 20-byte
  indirect record. Native indexing applies signed vertex offsets; GPU-produced
  values and reachable pointer bounds remain caller obligations.
- D32 supports DEPTH and optional copies, with depth aspects throughout view,
  discard and copy encoding. New INDEX_READ and DEPTH_READ/WRITE access classes
  do not overload vertex shader reads.

All repository C callers, GGML/libplacebo adapters and diagnostic signatures are
migrated. Existing one-draw examples explicitly reproduce their former
discard/clear policy; they do not silently claim the grouped-scope optimization.
The historical Rust graphics fixtures use a test-only scope helper; production
has no compatibility entry point. The dedicated public tests call begin/end directly.
Metal exports match the new signatures; zero-extra-usage allocation is preserved,
INDEX and graphics return UNSUPPORTED. No current Mac validation is claimed.

## Focused gates

The new public-boundary indexed test uses both index widths, poisoned index prefixes,
nonzero range/first-index offsets and signed -1/+2 vertex offsets. Two overlapping
triangles in one scope leave depth 0.25. An A/empty/A sequence replays a fixed list
with mutable GPU-readable indirect records and checks color/depth/guards. It also
destroys public image/index/vertex/indirect/raster handles before another replay,
then checks backing release after list retirement.

Recording rejection checks cover nested/no/open scopes, invalid range eligibility,
format/alignment/reserved fields, indirect bounds/root sizes through existing and
new tests, and forbidden copy/discard/barrier/split endpoints. Open-scope submit
and compile fail without consumption; end/draw/retry succeeds. CPU tests cover
overflow, allocation descriptions, clear nonfinite/range values and masks. Raster
creation rejects malformed depth/format/test/write/compare/topology descriptions.

A second public test covers exact D32 support-description rejection, depth upload,
empty clear scopes, color CLEAR/depth LOAD and color LOAD/depth CLEAR independently,
discarded results followed by explicit clear, and abandoned open-scope ownership.
It compares all uploaded/preserved/cleared depth texels, not just an interior probe.

Development verification uses Radeon RX 5700 XT and llvmpipe with validation and
sync validation enabled, plus ordinary CPU/layout/binding/mock/compiler and migrated
consumer checks. These runs are on the development worktree; the following clean
revision run and evidence receipt identify the final checkpoint. Do not substitute
this for the native scene's analytical full-image oracle or sustained measurements.

## Still required for this slice

1. Public counterpart of all ten accepted native scene modes, at odd and useful
   extents, using its GPU-written geometry/indices/records and independent oracle.
   The focused test uses host-written records; it does not prove that full chain.
2. Complete the proposal's broader cases: every compare operation, disabled
   test/write behavior, read-only depth with actual draws, mismatch/cross-device
   matrices and indexed preparation/loss fault injection. Existing generic
   failure/drain tests are regression evidence, not exhaustive new-path coverage.
3. Two-slot scene reuse and matched small grouped-draw/replay measurements. No
   fundamental/native performance approval follows from passing these tests.
4. Widen generated stage-pair checking for the scene's vertex root/flat varying,
   then add its standalone installed consumer and record the bounded decision.

After that: mip/view sampling, blend/viewport/scissor and the rest of the M4 scene.
Depth-only rendering, multi-draw/count buffers, dynamic depth state, transient-memory
policy, attachment feedback and broader rendering remain explicit open questions.
