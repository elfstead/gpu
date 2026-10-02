# Useful-scale indirect range correctness

Accepted 2026-10-02 at clean **`c7fb560`** on RX 5700 XT / RADV.
This closes the [useful-scale correctness increment](indexed-depth-performance-plan.md#useful-scale-correctness-increment),
not useful-scale timing, scope-break controls or full M4.

The native identity reference passes all 18 cases at 257×193 and 1280×720:
capacities 1/64/512 × separate/fixed/count strategies, eight count/phase inputs
each. The runner rechecks the original scene's analytic color/depth oracle and
full output hashes, then checks identity recoloring and exact generated records,
counts and guards. Separate calls still differ from ranges in the expected
identity-sensitive cases. Ordinary native scene regressions pass at both extents.

The matched public/native slot matrix passes **144 cases and 73,152 frames**:

| Extent | Cases | Frames/case | Total |
|---|---:|---:|---:|
| 257×193 | 72 | 1,000 | 72,000 |
| 1280×720 | 72 | 16 | 1,152 |

Each extent covers all capacities and strategies, one/two slots, owned reset and
serial replay, native/public. At 720p, every slot visits all eight count/phase
inputs (twice with one slot). Every retired frame checks complete color/depth,
geometry/record/count, control and guard bytes. Vulkan and synchronization
validation report no errors. No CPU count readback or additional producer
submission is introduced.

Trace checks preserve native draw commands, per-recording state-bind counts and
pool/reset/replay behavior. Nine tracked allocations per slot are retained; matched
allocation size/type multisets and peaks agree, and all are freed. At 720p and
capacity 512, requested bytes are 14,777,948 per slot and tracked peak bytes are
15,425,160; two slots double both. Private native command-memory bytes remain
unknown. Two unretired submissions establish schedule, not physical overlap.

Retain the range/count/identity contract and bounded state correction for the next
controls. This short useful-scale cycle is not a sustained 720p timing result and
does not erase the [remaining separate-reset host cost](graphics-draw-state-results.md).
Next: labelled per-draw-scope controls and useful-scale measurements, then the
complete installed scene handoff. The known llvmpipe counted→fixed failure remains
separate; no additional physical GPU or Metal coverage is claimed.

## Receipts

[Native reference](results/range-scale-2026-10-02/native.json) and
[matched correctness](results/range-scale-2026-10-02/correctness.json) preserve all
original report fields in normalized wrappers, with original byte hashes/paths.
Runner logs are adjacent; raw output/per-case logs remain at the recorded local
paths with their hashes. Existing native shader artifacts and analytic references
are unchanged; the harness's extent-aware budget/matrix checks have rejection tests.
