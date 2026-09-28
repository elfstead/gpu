# M3 allocation/submission decision

Accepted 2026-09-28 against clean measurement revision `bb76447`, following the
[predeclared protocol](resource-reuse-performance-plan.md). Offline negative export
tests were added separately at `aeefe54`. Runtime, public ABI 17, shader artifacts
and numerical gates are unchanged. The subsequent [installed handoff](resource-reuse-handoff.md)
at `89d71e5` completes bounded M3.

## Decision

Use **two arena-backed slots with serial compiled lists** for the fixed-command
learned-image handoff. Keep dedicated allocation and owned reset/re-record as
supported comparison/application choices. Keep allocation/range management in the
consumer; do not introduce a runtime allocator, scheduler or new public placement
primitive from this result.

The public API expresses the tested native allocation and replay strategies, with
matching ranges and native allocation sizes/types. This experiment exposes no
better public allocator/submission alternative for this bounded workload. It does
not establish that buffer handles are better than Aaltonen's public heap/range
design, settle image placement or approve the entire fundamental API. Breaking
changes remain welcome when another consumer exposes a better alternative.

## Measurements

Radeon RX 5700 XT, RADV Mesa 26.2.1. Four fresh processes per configuration,
100 warmups and 500 recorded frames each: **192 timing processes / 96,000 samples**.
Eight allocation/backend/submission combinations, one/two/three slots and small/
odd-video extents. Process order rotates; validation/tracing are disabled for
timing and enabled in separate correctness/memory controls. No device timestamps,
clock locking or claim of statistically independent per-frame trials.

Numbers below are medians across processes (and, for host/latency, medians of each
process's per-frame medians). All individual samples and per-process ranges are
retained in the [Radeon export](results/resource-reuse-performance-radv-2026-09-28/report.json)
and its [raw samples](results/resource-reuse-performance-radv-2026-09-28/samples.csv).

### Matched public/native paths

Across the 12 matched serial-replay cases, public/native wall ratios are
**0.995–1.009**. That is bounded replay evidence, not general parity or uniform tail
equivalence. Comparing the *best observed* public/native choices at each extent and
slot count gives ratios 0.995–1.008; selecting minima does not prove a true optimum.

Re-recording is not equally close in every case. The six small-case public/native
wall ratios are **1.006–1.088**; video ratios are 0.985–1.002. Process ranges overlap,
so do not treat every percentage as a resolved deterministic penalty. Nevertheless,
the small-case excess remains a measured optimization target, not something to
hide behind the GPU-heavy result.

The [current runtime](../crates/ogpu/src/batch.rs) collects `Step` values and copies roots before native encoding
at submission; the control records native commands directly. That is a concrete
implementation difference and a candidate for investigation, **not a measured
attribution of the entire gap**. This experiment demonstrates no missing native
strategy in the fixed-command public contract. It does not close the separate
mutable-root/dimension or direct-recording questions in the performance audit.
The earlier [host-sensitive replay result](command-list-results.md) still matters;
near-equal video throughput alone would not justify retaining this API.

For the selected two-slot arena policy:

| Case / path | Wall ms/frame | Host record+submit µs | Median frame latency ms |
|---|---:|---:|---:|
| Small, public re-record | 0.039005 | 22.587 | 0.0755 |
| Small, public replay | 0.032512 | 11.880 | 0.0627 |
| Small, native replay | 0.032267 | 11.402 | 0.0616 |
| Odd video, public re-record | 4.665646 | 35.575 | 9.3170 |
| Odd video, public replay | 4.664191 | 19.370 | 9.3139 |
| Odd video, native replay | 4.655180 | 18.485 | 9.2972 |

Replay cuts the selected public host record/submit median about 47% in the small
case and 46% in video. Small-case wall time falls about 17%; video throughput is
essentially unchanged. Its selection is for fixed-command reuse and lower host
work, not a claimed GPU execution speedup. Changing roots/launches still calls for
re-recording or another explicitly investigated strategy.

### Slots, allocation and memory

Two selected public arena/replay slots take 4.664 ms/video frame versus 5.930 ms
with one slot: about 21% less wall time/frame, at higher median latency
(9.314 versus 5.898 ms) and almost twice the explicit device-memory budget. Three
slots take 4.663 ms/frame, with 13.973 ms median latency and another slot's memory.
The small case gains about 2% going from two to three replay slots, while latency
increases about 46%. Two slots are the handoff's throughput/latency compromise,
not a universally optimal queue depth; low-latency consumers can choose one.

Allocation objects decrease, bytes do not materially decrease:

| Odd-video slots | Dedicated allocations / peak bytes | Arena allocations / peak bytes |
|---|---:|---:|
| 1 | 9 / 181,508,496 | 5 / 181,508,464 |
| 2 | 17 / 363,016,976 | 6 / 363,016,912 |
| 3 | 25 / 544,525,456 | 7 / 544,525,360 |

Both backends have identical signatures for a given allocation policy. Requested
buffer budgets match between dedicated/arena policies; tiny native byte differences
are rounding. Every allocation is freed. For two video slots, logical range guards
account for 1,792 bytes and additional HOST capacity padding for 7,856 bytes in
either policy. Padding is checked after drain, not silently treated as payload.

Arena versus dedicated replay shows no convincing material throughput advantage:
at two slots, public arena is 0.46% lower in the small case and 0.24% higher in video.
The choice favors fewer explicit allocation objects and an already tested bounded
consumer layout—not claimed VRAM savings or proven allocator speed. Inclusive
setup medians are roughly 14–16 ms small and 68–86 ms video across all controls;
they include files, initialization, pipelines and recordings, with reused caches.
They are not isolated allocation cost or cold-start measurements.

Known application storage is also recorded: at video scale, 90,373,952 fixture
bytes, 28,000 sample-array bytes for the 500-frame window, and the fixed stream
struct (4,200 bytes public / 9,040 native). These are requested application storage,
not measured total RSS. Runtime and driver command-memory sizes remain **unknown**;
the reports explicitly use null, never zero. Equal tracked backing allocations do
not prove equal total command-memory budgets. The mapped paths still copy fixture
data into/out of HOST views equally; no CPU zero-copy claim follows.

## Correctness, evidence and limitations

- Performance-suite correctness checks 64 full-output frames per configuration:
  48 configurations on Radeon (small and odd video), 24 on llvmpipe (small).
  Serial references add 12 frames per extent. Every checked frame matches the
  public serial output byte-for-byte and the independent CPU RGB ≤1/exact-alpha
  gate. Native and public range layouts and allocation sizes/types agree.
- A separate 1,000-frame small matrix passes on both drivers: 24 configurations
  each, **48,000 candidate frames**, plus serial references. This includes both
  allocation strategies, all slot counts and independent native reset/replay.
- Timed windows check every slot's final output, then input/weight integrity,
  intermediate guards and HOST padding outside the measured interval; they do
  **not** claim to check every timed frame. Forty-eight separate traced lifecycle
  controls use the same 100-frame warmup and a 64-frame measured window.
- All 96 failure configurations and legacy native/public preflights pass again
  after the shared buffer-adapter changes. CPU native/public range tests and new
  native cache-argument tests pass, including bounds, atom rounding and allocation
  tails. ASan/UBSan and Clang analysis pass; LeakSanitizer remains disabled because
  of environment tracing. Forty ordinary Rust tests, strict Clippy and format pass.
- Export revalidates all logs, 96,000 samples, matrix/order, statistics, ranges,
  allocation signatures and source/fixture hashes before publication. Ten offline
  negative tests reject incomplete/dirty reports, duplicates, changed samples,
  statistics, allocation/layout data, source hashes, process order or instrumented
  timing. See the [receipt](results/resource-reuse-performance-validation-2026-09-28.txt).

The [llvmpipe export](results/resource-reuse-performance-lvp-2026-09-28/report.json)
is correctness only. Both main performance reports are from clean `bb76447`;
subsequent regression runs recorded dirty worktrees because the already-exported
evidence directories were untracked. Their tested source hashes still match the
implementation. Offline negative export tests came later and changed no measured
code. No second physical GPU, Metal, real noncoherent hardware, full new M1 scale
suite, real device loss, general native optimum or broad API-performance approval
is implied.

## Next gate

Package the selected two-slot arena/replay consumer as an independent installed-SDK
handoff: no checkout headers or private runtime APIs, explicit range/list/receipt
lifetimes, reproducible diagnostics and documented unsupported backends. That is
the final M3 milestone gate, now accepted in the [handoff receipt](resource-reuse-handoff.md),
before the M7 experimental-release checkpoint.
Other performance-audit questions remain open rather than being silently accepted
by this workload's decision.
