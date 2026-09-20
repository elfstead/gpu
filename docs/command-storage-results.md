# Bounded command-storage experiment — 2026-09-20

At `01697a06640cd46ef0ff02a2f92297d348a014ec` (ABI 13), the
[device-owned empty-storage alternative](command-storage-review.md) passes its
local safety tests and the repeated small-compute comparison. Most of the prior
wall-time gap disappears. This supports separating storage from recording/resource
retirement; it does not establish native performance parity for all workloads or
select a final public storage/replay interface.

## Same workload, stronger native controls

The shader, native fresh/reset/replay strategies, numerical gates, queue, slot
counts and timing protocol are unchanged from the
[original result](performance-frontier-small.md). Only OGPU's runtime policy and
versioned lifetime contract changed. All new performance/software reports record
a clean `01697a0`; their runtime library hashes match.

The table is wall **milliseconds per 1,000 measured frames**, median of three
fresh processes. Parentheses are process minimum–maximum, not confidence intervals.
These are throughput windows, not individual-frame medians.

| Slots / dispatches | Native fresh | Native reset/re-record | Native replay | OGPU ABI 13 |
|---|---:|---:|---:|---:|
| 1 / 1 | 219.078 (218.202–220.984) | 52.990 (52.219–55.797) | 50.773 (50.333–52.949) | 55.614 (54.395–57.306) |
| 1 / 64 | 274.033 (270.333–288.033) | 168.999 (168.120–173.292) | 163.115 (160.636–163.501) | 175.921 (174.708–176.756) |
| 3 / 1 | 114.989 (107.351–188.925) | 12.681 (12.578–15.922) | 12.793 (11.967–13.153) | 12.964 (12.900–16.312) |
| 3 / 64 | 222.986 (222.510–225.011) | 120.602 (118.960–121.206) | 128.856 (121.624–129.052) | 119.869 (119.307–120.392) |

OGPU/reset ratios are **1.050, 1.041, 1.022 and 0.994**, versus the original
roughly 1.62–8.15 range. Historical runs are not interleaved before/after pairs;
the current native fresh control independently shows the large lifecycle cost.
The lower last ratio is not evidence that OGPU is fundamentally faster. Process
variation, tails and all raw samples remain visible; no statistical-equivalence
or uniform tail-parity claim follows.

For 64 dispatches, median host record+submit is about 23.5–24.4 microseconds OGPU,
20.2–20.3 native reset and 12.0–12.7 replay. There is remaining host work worth
examining, but this is not an isolated or unavoidable API tax. OGPU now resets
during terminal retirement (inside the measured wait), while native reset resets
on next recording. Compare full windows, not just submit times. Native replay
does not consistently improve three-slot throughput in these samples.

Median process setup ranges from 13.4–14.9 ms across configurations/strategies;
per-process setup is retained in the report. OGPU's initially empty cache fills
during the 100 drained warmups; these are warmed results, not a cold-submit
latency comparison. One/three slots allocate 400/1,200 application Vulkan bytes,
equal across strategies. Native reset/replay and OGPU retain command storage,
but the allocation tracer does **not** measure that driver-private storage.
Consequently equal total memory budgets are not established by this experiment.

## Acceptance

Same RX 5700 XT / RADV, Mesa 26.2.1, Vulkan 1.4.354, Ryzen 9 5900X and Linux
6.18.47 as the original probe. No affinity, clock pinning or exclusive-machine
control. Software checks use llvmpipe LLVM 21.1.8. Timing is separate from
synchronization validation and allocation tracing.

- 16,000 Radeon and 1,024 llvmpipe validation frames, all exact integer outputs
  and guards checked. No numerical tolerance changes.
- 48,000 ordinary measured samples, after 4,800 warmups; all final slot outputs
  checked outside timing. A separate 16-process timing-mode allocation matrix
  includes 17,600 frames. All 96 tracked allocations across validation/allocation/
  software controls are freed and their sizes/types match.
- 38 ordinary tests, 749 ABI layout checks, Clippy with warnings denied,
  reproducible Vulkan bindings and C mock-loader checks pass.
- All 22 runtime GPU tests pass on each driver with synchronization validation.
  Storage bounds, warm-cache failures, lazy queries, real heap replacement,
  pending/error gates and final-device cleanup are described in the
  [implementation review](command-storage-review.md#abi-13-implementation-experiment--2026-09-20).
- The mixed compute/raster streaming workload passes 768 small llvmpipe frames
  and a 144-frame Radeon preflight. This does not reaccept video-scale streaming
  performance or M1's full intermediate scalar numerical matrix.
- A clean revision-pinned ABI-13 SDK installs and relocates successfully. Its
  independent C consumer builds outside the repo, resolves the relocated library,
  rejects a missing loader and passes 4,099 integers/three passes/guards/partial
  upload/parent destruction on Radeon. The C heap-image example also passes all
  six extents and heap mutation cases. No GGML/libplacebo or native Metal rerun.

Initial development found a Rust borrow-check error and a test-only weak-reference
mistake: `Rc::get_mut` rejects weak owners too. Both were corrected before accepted
runs; the failed logs remain local. No GPU numerical or synchronization-validation
failure was accepted or suppressed.

## Decision and next question

Keep the separation of empty storage, executable references and submission
retirement. Retain the bounded cache as an **implementation experiment**, not the
fundamental answer to caller storage ownership. It has arbitrary admission limits:
recordings beyond 256 steps or 64 KiB roots still take the fresh-storage path,
and a caller cannot trim or reserve capacity. Increasing those constants would
not resolve the design question. Count/admission bounds do not equal a native-byte
budget, and the three-slot measurements do not prove behavior at every scale.

Next compare an explicit caller-owned recording-storage/resettable-owner shape
against this cache for heterogeneous and long-lived workloads, including release
control. Separately specify and test reusable executable recordings: which values
are copied, which resources stay owned, how pointed-to data changes, and when reuse
is legal. Do not conflate either with the storage-lifecycle speedup already found.
That is the next P2 design check, before unrelated widening or treating M2's
programming interface as settled. P1/P3–P7 remain open in the
[expressibility audit](performance-expressibility.md).

## Evidence and reproduction

Use the unchanged [frontier runner commands](../examples/performance_frontier/README.md).
The runtime revision, header, library hash and native artifact hashes identify the
new policy. Exporters recheck retained logs/matrices/statistics/allocations.

- [Radeon report](results/command-storage-radv-2026-09-20/report.json) and
  [48,000 raw samples](results/command-storage-radv-2026-09-20/samples.csv).
- [Small-compute software report](results/command-storage-lvp-2026-09-20/report.json).
- [Mixed-workload software report](results/command-storage-stream-lvp-2026-09-20/report.json).
- [Runtime, ABI and SDK validation receipt](results/command-storage-validation-2026-09-20.txt).

Local runs: `target/performance-frontier/small-065vd3oz`, `small-peq4g9j3`,
`stream-0n41m6n4` and Radeon preflight `stream-jxgeckol`. No artifacts were deleted.
