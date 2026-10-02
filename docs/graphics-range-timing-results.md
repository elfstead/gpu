# Grouped indirect ranges: first matched timings

Accepted 2026-10-01 at clean **`cee13c9`**, following a fresh 72,000-frame
validation/traced correctness run of the same source. This is the first timing
increment of the [predeclared brief](indexed-depth-performance-plan.md#first-timing-increment-grouped-gpu-copy-complete),
not full M4 or general Vulkan parity.

Follow-up: the [draw-state correction](graphics-draw-state-results.md) is accepted
at `3df8349`; this page preserves the original pre-correction measurements.

## Scope and measurements

RX 5700 XT / RADV, 257×193, capacities 1/64/512, separate/fixed/count draw strategies,
one/two slots, reset/re-record/serial replay, native/public. Three rotated fresh
process rounds produce **216 processes and 216,000 samples**, each after a drained
100-frame warmup. No validation, tracing or implicit layers in timed processes.
Native/public pairs preserve shaders, outputs, allocations and scheduling policy.

Every frame copies GPU color/depth/geometry results to HOST storage. CPU readback
and full-byte verification occur after each warmup/measured window for every final
slot, not inside timing. Wall/frame is therefore **GPU-copy-complete**, not full
host-output consumption. Host record/submit includes the control update and public
batch cleanup; retirement includes observed completion, not isolated GPU duration.
Setup and post-window verification are excluded. All final checks/drains pass.

The table reports medians across three process summaries, not pooled samples.
Full process medians, nearest-rank p95, maxima and wall ranges are retained.

| 512-record case | Slots | Public/native wall ratio | Native/public host record+submit (µs) |
|---|---:|---:|---:|
| Separate calls, reset | 1 | 1.803 | 39.38 / 122.87 |
| Separate calls, replay | 1 | 1.066 | 13.80 / 13.86 |
| Separate calls, reset | 2 | 1.090 | 39.77 / 112.61 |
| Separate calls, replay | 2 | 1.087 | 12.52 / 12.95 |
| Fixed range, reset | 1 | 1.016 | 22.82 / 25.66 |
| GPU count, reset | 1 | 1.029 | 24.30 / 26.34 |
| Fixed range, replay | 2 | 1.006 | 12.10 / 11.02 |
| GPU count, replay | 2 | 1.008 | 10.99 / 12.30 |

Across all fixed/count cases, wall ratios range from 0.986 to 1.054. This bounded
result preserves the selected native range strategies; it is not proof of the
fundamental API's performance on all workloads or devices. No uniform tail parity,
confidence interval or hardware-independent cost is inferred from three processes.

At 512 separate calls/one slot, wall/frame is 0.2049 ms native versus 0.3694 ms
public. Replay greatly reduces host work but retains a wall gap. The
[native traces](graphics-range-reuse-results.md) identify repeated pipeline/root/
index bindings in public single-call encoding as a candidate avoidable cost;
different barrier counts and other backend work remain possible contributors.
These measurements alone do not assign the whole gap to one cause.

## Decision and next experiment

Retain explicit fixed/count ranges and their identity/ownership contract. Do not
approve or dismiss the separate-call gap because multi-record calls are faster:
separate operations have distinct draw identity and remain legitimate programs.

Next test a bounded backend correction: elide redundant state binds/pushes across
consecutive draws with identical raster, index range and copied root. Preserve
every native draw operation, range/count descriptor and draw-identity boundary;
do not fuse calls, infer pointer contents or alter dependencies. Invalidate at
non-draw boundaries and cover empty ranges and changed bindings explicitly. Trace
the changed native commands, repeat correctness and matched timing, and retain
the correction only with evidence. The 2026-10-02 candidate uses a recording-local
borrowed state key (raster identity, exact index binding and copied root bytes).
Zero-capacity ranges leave actual native state unchanged; every non-draw step
invalidates the key. CPU mutation tests, the generated 540-frame Radeon consumer
and traced native/public preflight pass. Fresh clean-source sustained correctness
and timing are still required before accepting this candidate. Any remaining gap still needs classification
as implementation cost, contract-imposed cost or unresolved behavior.

Useful-scale and labelled scope-break controls, complete scene installation and
the remaining mip/view/blend slices stay open. The separate
[llvmpipe counted→fixed regression](draw-count-followup.md) is not waived by Radeon
timing; no new Metal or physical-GPU coverage is claimed.

## Receipts

[Timing](results/range-timing-2026-10-01/timing.json),
[same-source correctness](results/range-timing-2026-10-01/correctness.json) and
[derived 36-pair summary](results/range-timing-2026-10-01/summary.json).
The first two are normalized wrappers preserving every original report field,
plus original byte hashes/paths. Raw per-process sample/stdout/stderr hashes and
source/runtime/shader/oracle/build/environment identities remain in the reports;
raw samples are retained at those local paths, not copied wholesale into Git.
