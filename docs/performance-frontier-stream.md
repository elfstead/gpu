# Streaming learned-image frontier — 2026-09-20

The second [performance-expressibility probe](performance-expressibility.md) is
complete. It uses the frozen learned-image application at 720p and odd video
extents, one/two/three independently owned slots, and four execution strategies:
OGPU one-shot, native fresh pools, native reset/re-record and native replay.
The runtime, public ABI, shaders and numerical gates are unchanged.

## Result and tradeoff

Two in-flight slots are useful and already expressible through the public API.
At the selected extents, OGPU throughput improves about 24.9% and 27.2% over one
slot, at roughly twice the application GPU allocation and higher observed frame
latency. A third slot gives no useful throughput gain in these runs, while adding
another slot's storage and increasing latency further. This is a concrete budget
tradeoff, not a recommendation to buffer every workload more deeply.

Native storage reuse remains better than fresh allocation, but its wall-time
advantage is much smaller here than in the [small-compute probe](performance-frontier-small.md).
At equal slots/work, OGPU's median process wall time is 1.46–2.73% higher than
native reset/re-record. Native replay further reduces host recording work, but
does not consistently improve streaming throughput over reset/re-record.
GPU-heavy work must not be used to dismiss the larger host-sensitive opportunity.

The table reports **milliseconds per frame from total window wall time / 1,000**,
then the median of three fresh processes. These are throughput-equivalent frame
intervals, **not** median per-frame completion latencies. Parentheses show the
minimum–maximum of the three process values, not confidence intervals.

| Input -> output / slots | Native fresh | Native reset/re-record | Native replay | OGPU |
|---|---:|---:|---:|---:|
| 1280x720 -> 2560x1440 / 1 | 4.517 (4.472–4.533) | 4.408 (4.370–4.432) | 4.375 (4.334–4.385) | 4.503 (4.474–4.543) |
| 1280x720 -> 2560x1440 / 2 | 3.607 (3.603–3.608) | 3.519 (3.507–3.522) | 3.521 (3.509–3.524) | 3.605 (3.602–3.610) |
| 1280x720 -> 2560x1440 / 3 | 3.607 (3.603–3.608) | 3.510 (3.509–3.525) | 3.510 (3.509–3.523) | 3.606 (3.606–3.607) |
| 1919x1079 -> 2561x1441 / 1 | 6.036 (6.023–6.036) | 5.954 (5.916–5.967) | 5.939 (5.917–5.952) | 6.041 (6.038–6.095) |
| 1919x1079 -> 2561x1441 / 2 | 4.761 (4.750–4.767) | 4.664 (4.649–4.670) | 4.660 (4.653–4.679) | 4.748 (4.745–4.759) |
| 1919x1079 -> 2561x1441 / 3 | 4.754 (4.744–4.761) | 4.659 (4.652–4.679) | 4.660 (4.649–4.674) | 4.758 (4.744–4.761) |

OGPU observed frame latency (median of the process medians) and peak application
Vulkan allocation bytes, identical across all four strategies at a given extent/
slot count:

| Input / slots | Observed OGPU latency ms | Peak allocated bytes |
|---|---:|---:|
| 1280x720 / 1 | 4.480 | 130069760 |
| 1280x720 / 2 | 7.197 | 260139504 |
| 1280x720 / 3 | 10.804 | 390209248 |
| 1919x1079 / 1 | 6.007 | 181504592 |
| 1919x1079 / 2 | 9.484 | 363009168 |
| 1919x1079 / 3 | 14.256 | 544513744 |

Latency starts at CPU staging for that frame and ends after terminal observation
and CPU readback. It includes time spent preparing/interacting with other slots
before observing completion, not a calibrated GPU interval or external-arrival
latency guarantee. All samples, distributions and tails are retained. No universal
speed, statistical-equivalence or uniform tail-parity claim follows.

Median host recording+submission is roughly 85–89 microseconds OGPU, 33–35 native
reset/re-record and 19–20 replay. Host staging/readback remain in total wall time.
These are whole-path intervals, not an isolated per-call API tax. Both native and
OGPU use the same one-queue slot schedule; no measured gain is attributed to a
separate transfer engine. The native controls do not yet test direct host mapping,
narrowed dependencies or cross-queue scheduling and are not a global optimum.

## Acceptance and provenance

Tested implementation: `ed50e00907fb7cfa9265b0489e6e80e9937d26a5`. The Radeon run
started with a clean tree. The subsequent llvmpipe run records `dirty=true` because
the development/status/retirement-review documents were being updated; both runs'
source/header hashes match that commit (including the pinned Vulkan-Headers
submodule), and their binaries/runtime library are byte-identical. Do not relabel
the software run as a clean-tree test.

Environment: RX 5700 XT / RADV NAVI10, Mesa 26.2.1, Vulkan 1.4.354; Ryzen 9 5900X,
Linux 6.18.47 x86-64. Software correctness uses llvmpipe LLVM 21.1.8, 256 bits.
No CPU affinity, clock pinning or exclusive-machine control. Existing M1 reference
files were reused with checked model, weight, input and final-image hashes;
no training or shader regeneration occurred in this experiment.

- 24 Radeon validation processes, 1,000 alternating A/B frames each, check every
  final image against the unchanged CPU RGB <=1 / exact-alpha gate and readback
  guards. 12 llvmpipe configurations check 64 small odd frames each: 768 frames.
- 72 ordinary timing processes, each with 100 drained warmups and 1,000 measured
  frames: **72,000 retained samples**. Strategy order rotates by round/configuration.
  Validation and allocation tracing are disabled only for these timing processes.
- 24 independent timing-mode allocation runs, each with 1,100 frames; their times
  do not enter the performance comparison. Traced sizes/types match every strategy.
- After each window, every slot's final image is checked outside timing. A separate
  diagnostic submission then checks unchanged input/weights and all intermediate
  prefix/suffix guards using existing readback storage. Full intermediate scalar
  numerics were accepted by M1 and are **not reaccepted** by this streaming probe.
- All **1,020 tracked allocations** across the Radeon validation/allocation and
  llvmpipe controls are freed. Allocation count is `1 + 8*slots`, independent of
  frame count. This excludes driver-private command storage, CPU memory and physical
  residency; retained native command storage is not claimed to cost zero bytes.
- C builds use warnings as errors, native symbol inspection finds no OGPU linkage,
  the shared injected slot/drain tests and six small-runner tests pass, and streaming
  RGB/alpha/guard checks plus four parser/export tests pass. Both streaming C variants
  pass Clang static analysis; wrapper-only unused-linker-option warnings are recorded.

No full runtime GPU/GGML/libplacebo/SDK or native Metal reacceptance is claimed.
This changes benchmark/example code only. No failed GPU correctness gate or tolerance
relaxation occurred; initial field-name build errors were fixed before GPU preflight.

## Design decision and what remains

The first two probes now supply useful-scale and host-sensitive evidence for the
same question. Retain the ability to submit independent slots without waiting after
every submission; that is already expressible. Do **not** approve the fundamental
API from these results or retain mandatory native-pool destruction as an unquestioned
design rule. Use the [command-storage review](command-storage-review.md) to test a
better separation of storage, recordings and completion, including real heap edits,
failure handling, timing receipts and an explicit storage-retention bound.

The measured fresh/reset gap is primarily an implementation/storage-policy finding;
it is not proof that every native implementation of the current API would be slower.
The one-shot surface still cannot explicitly request executable-recording reuse.
Its quantitative unavoidable cost is not established by the Vulkan reset/replay gap.
Compare the concrete alternatives without assuming hidden command deduplication.

Cross-queue overlap, parallel recording, direct mapped/range access, finer dependency
expression, descriptor streaming and compiler restrictions remain open in the audit.
M3's broader injected lifetime/diagnostic work and the fundamental API gate are not
complete merely because these 1,000-frame streams pass. No Mac or second physical
GPU is required for the next local storage-contract experiment.

## Reproduce and inspect

See [commands and timing boundaries](../examples/performance_frontier/README.md#streaming-learned-image).
The exporter rechecks full matrices, retained log hashes, sample/statistic equality,
source/fixture hashes and full allocation traces; it does not rerun the GPU oracle.

- [Radeon report, statistics and allocation traces](results/performance-frontier-stream-radv-2026-09-20/report.json)
- [All 72,000 raw timing samples](results/performance-frontier-stream-radv-2026-09-20/samples.csv)
- [llvmpipe correctness evidence](results/performance-frontier-stream-lvp-2026-09-20/report.json)

Retained local runs: `target/performance-frontier/stream-26cdlfsd` (Radeon),
`stream-7lwp5pwp` (llvmpipe); logs `/tmp/ogpu-frontier-stream-final-radv.log` and
`/tmp/ogpu-frontier-stream-final-lvp.log`. Preflight artifacts remain; nothing was
automatically deleted. The run began on September 19 and was accepted September 20.
