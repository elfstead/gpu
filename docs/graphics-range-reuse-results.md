# Indirect ranges: matched slot/storage/replay correctness

Accepted 2026-10-01 at clean **`6ae1d5a`**. This extends the
[draw-contract acceptance](indirect-draw-ranges.md) to the matched scene's
caller-owned storage and replay policy. It is not timing or full M4 acceptance.

## Matrix and ownership

Capacities 1/64/512 × separate single-record/fixed-range/GPU-counted commands ×
one/two slots × reset/re-record/serial replay × native/public = 72 cases per driver.
All cases use 257×193 color/depth targets and the accepted native identity shaders.
Each slot independently cycles eight inputs: full, zero, one, partial,
above-capacity, full, zero, one. Compute generates records and counts on the GPU;
counted records remain nonempty, so masking cannot conceal missing count reads.
Inputs are updated only after their slot retires; both slots submit before either
is observed. Shared executables outlive every slot. No extra producer submission,
list per input, hidden CPU count readback or successful queue-idle call is added.

| Driver | Frames/case | Checked frames |
|---|---:|---:|
| RX 5700 XT / RADV | 1,000 | 72,000 |
| llvmpipe | 64 | 4,608 |

All **76,608 frames** pass complete image/geometry/count/guard comparison with
validation/sync validation. The runner first rechecks native reference hashes and
their independent analytic image and exact producer oracles. Each retired frame
then compares every byte to the appropriate reference and verifies the entire
control allocation. CPU mutation tests reject altered control/geometry guard
bytes, while parser tests reject false command/slot/ownership traces.

## Native strategy and budgets

Every matched pair has identical allocation size/type multisets and peak bytes,
nine buffer/image allocations per slot, and zero final tracked live allocations.
Reset retains one hot pool per slot; no per-frame pool allocation. Replay records
one immutable sequence per slot and has zero hot encodes, scopes, draw recordings,
bindings, root pushes, barriers or pool resets/creation/destruction. Submission
still occurs once per frame. Public list destruction resets its owned storage
before release; this teardown work is explicitly accounted for, outside the hot
window. Private native command-memory bytes remain unknown.

The stronger native control is not made to repeat public implementation work.
At 512 separate draw calls, one public encoding performs 513 pipeline binds/root
pushes (compute plus 512 raster) and 512 index binds; native performs two and one.
A fixed/count range binds graphics state once on both paths. This is a measured
command-count difference, **not a time measurement or unavoidable API penalty**.
The separate-call control also observes different draw identity from a range,
so the runtime cannot simply fuse the operations without a valid proof.

Public/native command barriers remain eight/six per recorded frame in this
scene's copy/dependency policy. Neither these counts nor equal allocation objects
establish equal hidden command-memory budgets or physical GPU overlap.

The [RADV receipt](results/range-reuse-2026-10-01/radv.json) and
[software receipt](results/range-reuse-2026-10-01/lvp.json) retain every parsed
report field, source/runtime/shader/reference hashes, command and memory traces,
and per-case raw log hashes. They are explicitly normalized JSON wrappers with
the original report's byte hash/path, not byte-identical copies. Full raw frame
logs remain at the recorded local paths; runner logs are committed alongside.

Earlier development preflights also reran the original UINT16/UINT32 grouped and
LOAD/clear slot controls on both drivers (512 frames each plus serial reference
regressions). Those bounded runs are regression evidence, not new sustained
acceptance of every historical workload.

## Next

Use these correctness controls to gate untraced, validation-disabled matched
measurements with explicit warmup/sample windows and per-process distributions.
Useful-scale and labelled per-draw-scope controls, complete scene installation,
and M4 mip/view/blend slices still remain. The
[llvmpipe counted→fixed regression](draw-count-followup.md) remains unresolved:
passing independent count/fixed scenes does not waive mixed-operation correctness.
No new physical GPU, Metal validation or general API/performance approval follows.
