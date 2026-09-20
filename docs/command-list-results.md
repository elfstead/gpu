# Command lists: accepted local result — 2026-09-20

At clean implementation `ae69055231ed498ee6685345ea686daf9597f60c` (ABI 15),
the [immutable command-list experiment](command-lists.md) passes ownership,
failure, independent-SDK and mixed compute/graphics checks. **Keep reusable
executables as an optional candidate.** The public API can now express encode-once
execution instead of imposing repeated recording calls on fixed-command workloads.
This closes that concrete P2 restriction, not the full performance-expressibility
audit or the choice of final handles/ergonomics.

## Measurement

Same shader, exact integer oracle, buffers, dependency chain, slots and clock
boundaries; the predeclared schema-3 comparison contains 72,000 timing samples.
Wall milliseconds per **1,000 measured frames**, median of three fresh processes.
Parentheses are process min–max, not confidence intervals or frame-latency tails.

| Slots / dispatches | Native reset | Native replay | OGPU owned recording | OGPU compiled list |
|---|---:|---:|---:|---:|
| 1 / 1 | 54.673 (53.023–55.092) | 51.281 (50.934–51.449) | 53.265 (52.926–53.942) | 51.994 (51.100–54.430) |
| 1 / 64 | 171.475 (171.317–174.417) | 161.925 (161.696–162.589) | 173.812 (173.077–178.580) | 159.258 (159.139–161.275) |
| 1 / 512 | 983.524 (980.636–1008.034) | 912.877 (912.779–922.959) | 1030.468 (1010.933–1031.349) | 953.263 (901.383–958.503) |
| 5 / 1 | 14.365 (14.315–15.233) | 12.927 (12.539–14.179) | 14.002 (13.907–17.275) | 12.578 (12.172–12.738) |
| 5 / 64 | 119.845 (118.708–121.112) | 128.783 (128.485–129.096) | 119.972 (119.787–120.013) | 124.510 (123.931–127.986) |
| 5 / 512 | 873.334 (872.543–874.851) | 875.611 (872.676–879.261) | 877.197 (872.076–881.441) | 872.471 (871.022–884.736) |

Compiled/native-replay wall ratios are **0.967–1.044**. The 1/512 compiled median
is 4.4% slower; its process range also spans below native replay. This is not
statistical equivalence or evidence of an unavoidable API tax. The 5/64 replay
strategies lose throughput to re-recording: compiled is 3.8% slower than OGPU
owned; native replay is 7.5% slower than native reset. Keep both strategies and
these unfavorable results; do not silently select the fastest individual process.

Host record+submit microseconds, median of per-process medians. For replay,
recording in the measured window is a checked no-op; actual encoding is in setup.

| Slots / dispatches | Native reset | Native replay | OGPU owned | OGPU compiled |
|---|---:|---:|---:|---:|
| 1 / 1 | 12.770 | 10.360 | 11.965 | 10.430 |
| 1 / 64 | 21.050 | 12.380 | 24.530 | 12.420 |
| 1 / 512 | 88.579 | 13.420 | 121.884 | 14.110 |
| 5 / 1 | 12.150 | 10.940 | 11.340 | 9.640 |
| 5 / 64 | 19.070 | 11.100 | 23.044 | 11.160 |
| 5 / 512 | 87.849 | 12.590 | 111.764 | 12.895 |

At 512 dispatches, replay removes about **88%** of current OGPU's repeated host
record/submit cost. Native and OGPU replay are close in this metric, unlike the
re-recording paths. Median process p95 host intervals at 512 dispatches are
19.630/19.769 µs (native/OGPU, one slot) and 16.440/17.240 µs (five slots).
All raw samples, tails, waits and observed latencies remain in the evidence;
these figures do not establish uniform tail parity.

Median process setup spans 13.8–15.5 ms across strategies/configurations. It
includes device/program creation and is not an isolated compile-amortization
measurement. Native replay retains its existing serial-use flags; OGPU uses
`SIMULTANEOUS_USE`, permitting a stronger behavior tested separately with a gate.
This flag difference is not isolated by the matrix: whether a serial-only usage
hint exposes a faster native mapping remains open. A repeatable cost imposed by
mandatory simultaneous-use semantics would warrant revising the contract, not
excusing it as a small percentage or assuming the driver optimizes it away.
Neither resets commands between measured executions. OGPU retains its CPU step/
root vectors for object ownership; removing that duplication is possible backend
work, not required by this public contract. Total native/CPU command memory is
not measured or proven equal.

## Acceptance and limits

All four exported reports and the installed SDK identify clean `ae69055`. GPU
reports share the same runtime binary hash; the SDK separately builds an explicit
native-target release. Environment: RX 5700 XT / RADV Mesa 26.2.1, Vulkan 1.4.354;
Ryzen 9 5900X, Linux 6.18.47. Software driver: llvmpipe LLVM 21.1.8. No clock or
affinity pinning, exclusive machine control or extra physical GPU. Rotated order
is not perfectly position-balanced over three rounds/four strategies.

- 25 runtime GPU tests pass on each driver with synchronization validation.
  Counters prove encode-once/no-reset-between-uses; exact integer outputs and
  guards cover mutable pointed-to data and copied roots. A closed real semaphore
  gate holds three executions of one list pending while its public handles are
  destroyed; out-of-order observation preserves all remaining ownership.
- Heap edits fail between executions and succeed after final list release, even
  while old receipts live. Explicit storage remains reserved for the list lifetime.
  Known submit OOM is retryable; unknown errors drain/poison. Begin/end failures,
  transient poll errors, lost-device compilation and final-use reset loss have
  cleanup checks. Loss is injected only with real work safely drained; this is
  not real hardware-loss evidence. Timed compilation rejects without consumption.
- 24,000 Radeon and 1,536 llvmpipe correctness frames check every integer/guard.
  72,000 uninstrumented timing frames follow 7,200 drained warmups. Another 24
  traced timing-mode processes execute 26,400 frames, not used for speed claims.
  All 216 tracked application allocations across these reports are freed.
  One/five slots request 388/1,940 bytes; Radeon allocates 400/2,000 and llvmpipe
  388/1,940. Equality is within each driver/slot count, not across drivers.
- Compiled learned-image commands pass **192 alternating-input frames per driver**
  across one/two/three slots at 65×47→131×95. Full final pixels, readback guards,
  final input/weight integrity and intermediate guards pass; all 51 allocations
  per driver are freed. This validates repeated copy/compute/render/readback,
  not full intermediate numerical or video-scale performance reacceptance.
- 38 ordinary tests, Clippy with warnings denied, reproducible bindings, 749 C/Rust
  layout checks, loader mocks and both frontier host-test suites pass.
- The relocated installed C example passes default, explicit-storage and replay
  paths on both drivers: 4,099 integers, three executions, guards, a partial input
  update and early parent destruction. Replay also zeroes the original root,
  checks null compile-output rejection and reserved-state trim rejection, and
  destroys its public list before the final wait. Manifest/hash/revision/ABI,
  dynamic-link relocation and absent-loader rejection pass without checkout tools.

No numerical gate was relaxed. No Metal compile/execution, GGML/libplacebo rerun,
timed replay, mutable command roots or universal Vulkan parity is claimed. Metal
exports explicit unsupported calls, not an emulated replay path.

## Decision and next work

Retain immutable lists alongside controllable one-shot storage. Fixed roots plus
mutable pointed-to data now have a concrete native mapping and lifetime evidence.
Do not make replay mandatory: the throughput results themselves argue against it.
Changing roots/dimensions and per-execution timestamp ownership remain distinct
P2 questions; neither is solved by these untimed fixed-command samples. Further
direct-encoding/vector optimization is backend work, not a reason to keep P2's
fixed replay experiment open indefinitely.

Move the next bounded contract probe to **P3: direct host access and independent
buffer ranges**, with [native controls first](mapped-streaming-review.md). It asks
whether copy-only CPU access and whole-allocation completion exclude a useful
native streaming strategy under a stated memory/latency budget. M2 compiler work
and the other audit concerns remain open; this result does not stabilize the API.

Evidence: [Radeon report](results/command-list-radv-2026-09-20/report.json),
[72,000 raw samples](results/command-list-radv-2026-09-20/samples.csv),
[llvmpipe report](results/command-list-lvp-2026-09-20/report.json),
[mixed Radeon](results/command-list-mixed-radv-2026-09-20/report.json),
[mixed llvmpipe](results/command-list-mixed-lvp-2026-09-20/report.json), and
[validation receipt](results/command-list-validation-2026-09-20.txt).
Exporters rechecked fixed matrices, log/source/fixture hashes, statistics and
allocation traces. Full local runs remain under `target/performance-frontier/`
as `small-44jy6ra_`, `small-c7lnxhlf`, `stream-2mzib6b7` and `stream-2b_sr8wt`.
