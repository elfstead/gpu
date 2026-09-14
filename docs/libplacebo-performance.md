# libplacebo performance comparison

Selected 2026-09-14. Next bounded decision: does the measured cost come from
consumer submission policy, or expose a better runtime API alternative? Compare
pinned libplacebo's native Vulkan backend, OGPU per-operation submissions, and
OGPU one-batch-per-frame submission using the existing public API (ABI 10).
This compares implementations of one workload, not OGPU versus every Vulkan
program, and not isolated runtime overhead when generated shaders differ.

## Method fixed before timing

Use the same upstream EWA Lanczos compute and nearest raster processing, RGBA8,
two source/intermediate/output texture sets and at most two uncollected frames.
Native Vulkan uses a single queue policy (async compute/transfer disabled).
Native source uploads use two explicitly allocated, reusable host-visible
`pl_buf` staging buffers, one per frame slot, through public libplacebo APIs.
OGPU uses its adapter's existing two-slot host staging. Both copy input bytes
inside transfer-mode timing; native downloads retain the pointer/callback path.
This compares these declared staging policies, not native's default upload heuristic.
No external shader cache is loaded. Compile/allocate/first-use costs belong to
setup/warmup, not steady-state throughput. Inputs are pre-generated outside timing.

Three input/output extents: 64x33 -> 129x67 (fixed-overhead diagnostic),
960x540 -> 1921x1081, and 1920x1080 -> 3841x2161. The latter are deliberately
near-1080p/4K outputs retaining the original odd-scale/tail behavior.

Two modes: resident images (upload before timing; no timed readbacks) and transfers
(one upload and two callback readbacks per frame, preserving the acceptance loop).
Resident collection still observes completion before texture/bank reuse. Separate
fresh processes per backend/mode/extent/run; eight warmup frames, 64 measured frames,
three runs with rotated backend order. Report individual results and median/range;
do not select the fastest run. Software execution is correctness evidence, not a
performance comparison with hardware. GPU clocks/load are not controlled: record
device/driver/toolchain/environment and call results local observations.

Before timing, compare all three backends' intermediate/final outputs in memory
on both drivers, at every extent and both data-flow modes. Preserve RGB <=2/255,
alpha exactly 255, exact intermediate/final nearest copying and repeated-input
checks. No per-frame logging, disk writes or output checking inside timed loops.
Validate batched failure cleanup and the existing integration before accepting it.
Run timing with validation disabled only after validated correctness passes.

Metrics: device/setup wall time; warmup wall time; completed-frame throughput;
mean/p50/p95 host-observed frame latency (begin recording through explicit collection,
not display latency or a GPU timestamp); host recording/submission wall and thread
CPU time; collection wall time; total process CPU usage; OGPU submission/wait/poll
counts; common texture payload and peak process RSS. Do not equate RSS/payload to
resident GPU memory. Native queue-submit count and native allocation padding are
not available through the chosen instrumentation; label them unavailable rather
than estimating them. Shader generation/copies/driver calls are included in host
recording time, not claimed as pure OGPU call overhead.

Both OGPU variants keep the same operations, heaps, barriers and staging policy;
only submission/receipt grouping changes. Correctness and bounded ownership are
gates; no speed target or API change is preselected. Stop with an overhead breakdown,
attribution limits and retain/change recommendation. No allocator, scheduler,
kernel tuning, new formats or new runtime API is authorized by this milestone.

## Native upload validation finding

Pre-timing method amendment: the initial Radeon validation gate stopped on a
native upload READ_AFTER_WRITE hazard at 960x540. In pinned libplacebo
`src/vulkan/gpu_buf.c`, the unmapped-buffer write path barriers with COPY stage
then issues `vkCmdUpdateBuffer` (CLEAR stage), followed by an image-copy read.
The original pointer-upload heuristic selected this path on Radeon, but not
llvmpipe. The failed run is preserved locally in
`target/libplacebo-integration/perf.D5NKRBRe`; no timing was accepted.
Explicit HOST staging above avoids that path without patching upstream or
disabling validation. This amendment was committed as `90dd3d7` before timing;
both-driver validation was repeated successfully. See pinned upstream
[buffer writes](https://github.com/haasn/libplacebo/blob/3330a515d62139259c26239014f286e233bd3a5c/src/vulkan/gpu_buf.c)
and the Vulkan [clear commands](https://docs.vulkan.org/spec/latest/chapters/clears.html).

## Acceptance and measurements — 2026-09-14

Complete at source `90dd3d7`, following methodology `d7a6411` and implementation
`d7eef08`. ABI 10 and runtime code are unchanged. On both llvmpipe and physical
RX 5700 XT / RADV Mesa 26.2.1, all three engines match exactly at all three extents
and both modes: zero differing bytes, including intermediate/final equality.
Synchronization validation is clean with the explicit native upload policy.
Both drivers pass the per-operation and aggregated pending, poll-error,
submit-error, capacity, partial-frame, staging-reuse and destruction checks,
plus the existing specialization and queued-bank checks.
The original nine-frame consumer and both 36-frame scheduling controls still
match their references exactly on both drivers. All 20 runtime GPU tests pass
per driver; 27 ordinary Rust tests, 745 C/Rust ABI layout checks, clippy and
format/shell-syntax checks pass. The upstream checkout remains unmodified.

All 54 hardware timing processes completed. Below are completed frames/second,
median of three fresh-process runs, with min–max in parentheses. These are short,
local observations on one unlocked, non-isolated GPU, not sustained-throughput
guarantees. The small-workload ranges particularly caution against precision claims.

| Input / mode | Native Vulkan | OGPU per operation | OGPU per frame |
|---|---:|---:|---:|
| 64x33 resident | 12,391 (12,171–14,854) | 2,936 (2,881–3,038) | 4,948 (4,043–5,134) |
| 64x33 transfers | 9,088 (7,928–9,578) | 1,346 (1,331–1,366) | 4,936 (4,821–4,972) |
| 960x540 resident | 2,227 (2,215–2,249) | 931 (929–933) | 968 (966–968) |
| 960x540 transfers | 284 (284–284) | 228 (228–230) | 237 (237–237) |
| 1920x1080 resident | 553 (552–554) | 98.9 (98.7–98.9) | 99.1 (99.0–99.2) |
| 1920x1080 transfers | 45.10 (44.98–45.16) | 41.83 (41.82–41.86) | 41.91 (41.91–41.91) |

Outputs are 129x67, 1921x1081 and 3841x2161 respectively, not equal-sized inputs
and outputs. [All raw observations, exact ranges and environment](results/libplacebo-perf-radv-2026-09-14.txt)
are committed, including setup/warmup, latency percentiles, CPU, payload and RSS.
Local full logs: `target/libplacebo-integration/perf.8Wm1n6xK` (RADV) and
`perf.ZpymcVzI` (llvmpipe verification). No software timing is used in the table.

### Cost breakdown and attribution limits

Median host measurements below are milliseconds per frame. Recording/submission
includes shader dispatch preparation, copies where applicable, and driver calls.
Collection includes completion observation, resource retirement and any readback
copies. It is **not** GPU timestamp duration. Work overlaps between the two slots;
latencies must not be added together to infer throughput.

| Resident output / engine | Recording wall | Recording thread CPU | Collection wall | Host latency p50 |
|---|---:|---:|---:|---:|
| near-1080p native | 0.084 | 0.082 | 0.362 | 0.823 |
| near-1080p per operation | 0.187 | 0.175 | 0.887 | 2.054 |
| near-1080p per frame | 0.107 | 0.105 | 0.928 | 1.973 |
| near-4K native | 0.111 | 0.109 | 1.695 | 3.547 |
| near-4K per operation | 0.179 | 0.176 | 9.935 | 20.118 |
| near-4K per frame | 0.117 | 0.114 | 9.971 | 20.070 |

Grouping reduces measured OGPU submissions from 128 to 64 in resident mode and
320 to 64 with transfers. All larger OGPU configurations make 64 explicit waits;
grouping does not remove the need to wait for slot reuse. Near-1080p recording CPU
falls from 0.175 to 0.105 ms resident and 0.575 to 0.336 ms with transfers. The
small-workload median throughput improves 1.69x resident and 3.67x with transfers;
the near-1080p gain is about 4% and the near-4K gain below 1% in both modes.

The large resident gap is therefore **not explained by submission count alone**.
Native is about 2.30x the grouped throughput near-1080p and 5.58x near-4K. At
near-4K, grouped host recording CPU is already close to native, while collection
wall time remains much larger. This localizes the observed difference outside
ordinary host recording, but does not distinguish shader execution, image layout /
compression, barriers, driver behavior or retirement overhead. Those require
separate instrumentation; no cause or irreducible API overhead is established.

Transfers change the workload substantially: native falls from 553 to 45.10 fps
near-4K and grouped OGPU from 99.1 to 41.91. Total process CPU averages about
15.15/15.72 ms per frame there, versus 0.129/0.172 resident. This mode includes
large host copies and does not prove that GPU execution is nearly equivalent.
Native staging/readback allocation policy still differs from OGPU, as declared.

Common near-4K texture payload is 149,395,216 bytes (excluding LUT). Grouped OGPU
retains 149,396,240 staging bytes resident because warmup performs readbacks;
transfer mode retains 157,690,640. Thus resident means **no timed transfers**, not
minimum-memory residency. Grouping changes receipts, not staging allocation.
Native allocation totals remain unmeasured; process RSS must not substitute for them.

## Decision and stopping point

Retain consumer-side one-frame batching as the preferred measured policy for this
bounded workload, with per-operation submission preserved as a diagnostic control.
Do not add a runtime frame scheduler or new submission API: the existing batch
contract expresses the grouping and preserves failure/lifetime guarantees.
This is not a commitment to compatibility if further work exposes a better API
alternative. It is a measured policy decision, not API stabilization.

The comparison milestone stops here. The recommended next task is **diagnosing
the resident near-4K gap**, not widening the API or tuning blindly: first separate
device compute/raster work from host collection/retirement, then compare generated
shader and image/barrier paths against native. Keep this a separately scoped
implementation investigation, with a cause-and-evidence stopping condition.
No accelerated arithmetic, broader formats or general backend work is selected.
