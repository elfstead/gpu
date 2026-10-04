# Grouped and per-draw-scope performance at useful scale

Accepted measurements 2026-10-04 at clean **`497e019`**. Correctness was collected
2026-10-02 at the same source/runtime. This completes the declared small/720p
grouped and labelled scope timing matrix, not full M4 or universal API parity.

## Gates and scope

All **103,680 correctness frames** pass full image/depth/geometry/count/control
and guard checks, with Vulkan and synchronization validation:

| Policy | Radeon small / 720p frames | llvmpipe small frames |
|---|---:|---:|
| Grouped separate/fixed/count | 72,000 / 1,152 | 4,608 |
| Per-draw scopes, separate calls only | 24,000 / 384 | 1,536 |

Capacities are 1/64/512, with one/two slots and owned reset/serial replay.
The scoped control clears color/depth once, then LOADs both across explicitly
ordered scopes. It preserves every separate call's local DrawIndex=0 and matches
grouped single-call output byte-for-byte. It is not an alternative implementation
of a multi-record range, whose identity differs.

Matched pairs retain nine allocations per slot, equal allocation size/type
multisets and peak bytes, and zero final live allocations. Trace checks establish
the real scopes, draw commands, dependencies, storage resets, submissions and
replay compilation. At capacity 512, scoped recording adds 511 attachment
dependencies: native/public barriers are 517/519 instead of 6/8. Native binds
graphics pipeline/root/index and sets viewport/scissor once; public repeats each
512 times because its current cache invalidates across non-draw steps. Native is
not made to repeat that implementation work. Hidden command-memory bytes remain
unknown. No successful hot path uses queue-idle.

## Timings

RX 5700 XT / RADV; unchanged shader artifacts and matched slot/readback policies.
There are **576 fresh processes and 576,000 measured samples**: 432 grouped,
144 scoped, three rounds/configuration, 100 drained warmups and 1,000 samples.
Validation/tracing/implicit layers are disabled for timing. Every final slot after
warmup and measurement passes full-byte checks and all work drains. Metrics retain
the [GPU-copy-complete definitions](indexed-depth-performance-plan.md#first-timing-increment-grouped-gpu-copy-complete).

These values are medians across three process summaries, not pooled independent
samples. Full per-process distributions, nearest-rank p95 and wall ranges remain
in the receipts. No confidence interval, isolated GPU duration, general tail
parity or cross-device claim follows.

| 512-record case | Extent | Slots | Native/public wall (ms/frame) | Public/native | Native/public host record+submit (µs) |
|---|---|---:|---:|---:|---:|
| Grouped separate, reset | 257×193 | 1 | 0.2053 / 0.2554 | 1.244 | 40.17 / 75.22 |
| Grouped separate, replay | 257×193 | 1 | 0.1772 / 0.1778 | 1.003 | 12.52 / 12.68 |
| Grouped separate, reset | 1280×720 | 1 | 1.4821 / 1.5383 | 1.038 | 48.45 / 89.87 |
| Grouped separate, reset | 1280×720 | 2 | 1.4156 / 1.4216 | 1.004 | 49.06 / 91.83 |
| Grouped fixed range, reset | 1280×720 | 1 | 1.4567 / 1.4605 | 1.003 | 31.83 / 34.25 |
| Grouped GPU count, reset | 1280×720 | 1 | 1.4387 / 1.4435 | 1.003 | 32.30 / 34.09 |
| Per-draw scopes, reset | 257×193 | 1 | 1.4251 / 1.6966 | 1.190 | 345.91 / 556.66 |
| Per-draw scopes, replay | 257×193 | 1 | 1.0987 / 1.1180 | 1.018 | 15.60 / 15.59 |
| Per-draw scopes, reset | 1280×720 | 1 | 2.8960 / 3.1928 | 1.102 | 378.60 / 622.48 |
| Per-draw scopes, replay | 1280×720 | 1 | 2.5472 / 2.5757 | 1.011 | 17.09 / 17.54 |

Grouped fixed/count ratios across both extents span 0.990–1.056. The small
separate-reset gap repeats the previous corrected result; larger GPU work and
two-slot throughput conceal much of its host cost. Neither is grounds to dismiss
it. Caller-chosen scope breaks are expensive on both implementations: at 512
records/one-slot reset, native scoped wall time is about 6.9× grouped at the small
extent and 2.0× at 720p. These are separate sequential policy matrices, not a
randomized estimate of the isolated cost of a single scope command.

## Decision and remaining question

Retain explicit rendering scopes, real indexed/depth execution, fixed/counted
ranges, local draw identity and caller-owned reset/replay. The public interface
expresses the measured grouped/native strategies; it does not force a scope per
draw. Keep the accepted consecutive-state correction. Cross-scope rebinding and
redundant viewport/scissor commands are identifiable backend work, not a reason
to add a public state API merely to match this implementation.

**Do not classify the entire remaining per-call host gap as backend-only.** The
contract snapshots caller-supplied roots on each operation. A native Vulkan
caller can bind an unchanged root once while changing subsequent draw commands.
Direct encoding can remove our deferred vectors/allocations, but cannot assume
unchanged bytes at the same caller pointer. Current timings use an eight-byte
vertex root and do not isolate this distinction or prove its magnitude.
The [argument/state reuse audit](argument-reuse-plan.md) makes that a concrete
contract question, separately from the measured redundant-state correction.

Proceed with the full generated/relocated indexed scene handoff. Before treating
the per-operation argument shape as an accepted fundamental interface, execute
the bounded root-reuse probe; it may expose a better API alternative even if the
large scene's wall ratios remain close. Mip/views, blending and M5 remain later
slices. The strict llvmpipe counted→fixed failure is still retained separately;
independent scoped/fixed/count success does not waive it. No Metal coverage added.

## Receipts

[Grouped timing](results/graphics-scope-scale-2026-10-04/timing-grouped.json),
[scoped timing](results/graphics-scope-scale-2026-10-04/timing-scoped.json), and
[96 matched-pair summaries](results/graphics-scope-scale-2026-10-04/summary.json).
Adjacent correctness reports cover grouped/scoped Radeon and llvmpipe runs.
All reports are normalized wrappers preserving original fields and byte hashes/
paths; runner logs are archived alongside. Raw per-frame/sample files remain at
their recorded local paths with hashes, rather than being copied wholesale to Git.
