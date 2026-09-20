# Explicit storage: accepted local result — 2026-09-20

At clean implementation `7fa076345e7108aebb93d1d67e713de41f72ef3b` (ABI 14), the
[explicit recording-storage experiment](recording-storage.md) passes lifetime,
failure, public-C/installed-SDK and extended frontier checks. Caller-owned storage
removes the cache's admission cliff while keeping recordings one-shot and letting
the caller explicitly release idle capacity.

Retain explicit ownership as the preferred **candidate** for controlled recording
storage. This does not prove a separate owner is uniquely better than a resettable
storage-owning batch, nor settle executable replay or the complete fundamental API.
Keep the existing cache as a convenience/comparison path during experimentation.

## Measured result

Same integer shader, modulo-32-bit oracle, guards, native command paths and clock
boundaries as the earlier frontier; the new fixed matrix adds `owned` and larger
workloads. Each dependent dispatch adds a barrier and dispatch step: 129 crosses
the device cache's 256-step limit. One/five independent slots each own one buffer;
the explicit path also owns one recording-storage object per slot.

Wall milliseconds per **1,000 measured frames**, median of three fresh processes.
Parentheses show process min–max, not confidence intervals. These are throughput
windows, not individual-frame completion-latency medians.

| Slots / dispatches | Native fresh | Native reset | Native replay | OGPU cache | OGPU explicit owner |
|---|---:|---:|---:|---:|---:|
| 1 / 64 | 273.306 (272.196–288.558) | 171.540 (170.329–172.949) | 160.205 (159.513–162.771) | 174.097 (173.825–175.902) | 174.912 (174.761–176.072) |
| 1 / 129 | 399.234 (390.928–400.229) | 296.396 (295.716–297.666) | 281.161 (278.342–286.150) | 401.267 (397.491–410.989) | 305.250 (302.281–305.880) |
| 1 / 512 | 1072.431 (1064.262–1118.121) | 999.980 (980.021–1006.380) | 906.815 (898.671–908.241) | 1165.530 (1119.068–1167.688) | 1037.402 (1017.621–1049.847) |
| 5 / 64 | 221.669 (221.231–223.843) | 120.787 (120.446–120.824) | 121.070 (120.726–121.868) | 120.580 (120.071–121.554) | 119.723 (119.623–121.267) |
| 5 / 129 | 340.017 (338.656–341.805) | 231.442 (230.219–232.673) | 229.665 (228.302–230.258) | 340.054 (339.985–342.549) | 230.795 (228.841–231.142) |
| 5 / 512 | 971.367 (970.852–978.483) | 873.716 (873.052–874.593) | 875.196 (872.602–882.270) | 970.556 (970.293–980.563) | 873.980 (866.577–882.207) |

Explicit/native-reset wall ratios range **0.991–1.037**. At 129 dispatches,
explicit-owner throughput is **31.5% higher with one slot and 47.3% higher with
five slots** than the cache path. At 512 it is about 12.4% and 11.1% higher.
At 64 dispatches, where cache admission succeeds, no large owner penalty is shown
by these samples; this is not a statistical-equivalence or uniform-tail-parity claim.
Five slots exceed the cache's idle pool count but do not imply per-frame thrashing:
the ring normally retires one slot immediately before recording its successor.

Host recording+submission still costs more than native reset. Median of process
medians, microseconds:

| Slots / dispatches | Native reset | Native replay | OGPU explicit owner |
|---|---:|---:|---:|
| 1 / 64 | 20.970 | 12.230 | 25.260 |
| 1 / 129 | 31.874 | 13.030 | 34.979 |
| 1 / 512 | 87.824 | 13.505 | 120.939 |
| 5 / 64 | 20.240 | 10.290 | 22.650 |
| 5 / 129 | 27.304 | 11.990 | 35.605 |
| 5 / 512 | 87.039 | 12.460 | 117.559 |

OGPU still constructs CPU step/root vectors and lowers them on submit. Explicit
storage does not reuse those vectors or remove repeated public recording calls.
This measured host difference is **not proof of unavoidable API overhead**: a
native implementation could lower commands during recording into reserved storage,
without the prototype's extra buffered representation. That mapping still needs
its own implementation/correctness evidence. Replay eliminates more repeated host
work and improves one-slot throughput here, but not every five-slot case.

Native reset happens at recording; OGPU resets during terminal retirement. Full
windows include both, so submit-only numbers are not the complete lifecycle cost.
Median process setup ranges 13.5–15.2 ms across configurations/strategies. Allocation
is lazy; native pools fill/grow during the 100 drained warmups. This is not a cold
submission or preallocation guarantee. All setup values, sample distributions,
tails and raw frames are retained.

## Ownership and acceptance

Both GPU reports and the installed SDK record the same clean `7fa0763`. Radeon
and llvmpipe reports contain identical runtime and benchmark artifact hashes.
Environment: RX 5700 XT / RADV, Mesa 26.2.1, Vulkan 1.4.354; Ryzen 9 5900X,
Linux 6.18.47. Software: llvmpipe LLVM 21.1.8. No clock/affinity pinning or exclusive
machine control. Strategy order rotates; it is not perfectly balanced over three
rounds with five strategies. No universal native-parity conclusion follows.

- All 23 runtime GPU tests pass on each driver with synchronization validation.
  The explicit-owner test alternates 1/256/257/4096 steps over 50 submissions,
  observes one native pool allocation, then trims it while old batches and unread
  timing receipts survive. Reuse after trim allocates fresh storage. Explicit
  owners never populate the device cache. Heap replacement/edit/release is tested
  through both routes; gated submissions reject early owner reuse/trim and preserve
  reservations on transient poll errors. Old receipts cannot release other owners.
- Warm explicit reuse covers begin/end/submit OOM, reset OOM and simulated reset
  loss after a real terminal wait. Failure cleanup releases both objects and the
  reservation. Destruction-before-submit/retirement and submission on an already
  lost device release owners without reviving consumed batches. No real hardware
  loss claim is made.
- 38 ordinary tests, Clippy with warnings denied, 749 ABI layout checks,
  reproducible bindings and C mock-loader checks pass. The frontier's seven runner
  tests and native failure/oracle tests pass. The streaming runner's four host
  tests/build pass; this is not new mixed-workload GPU acceptance.
- 30,000 validated Radeon frames and 1,920 llvmpipe frames check every output and
  guard. 90,000 ordinary measured frames follow 9,000 warmups, with final slot
  checks outside timing. Thirty separate timing-mode allocation runs add 33,000
  frames; their timings do not enter the comparison.
- All 270 tracked allocations across Radeon correctness/allocation and software
  controls are freed. One/five slots allocate 400/2,000 application Vulkan bytes,
  equal across strategies. This excludes driver-private command memory, CPU step/
  root storage and physical residency. Pool reuse/destruction counters establish
  object lifetime, not an exact byte budget or equality of total retained memory.
- A relocated clean ABI-14 SDK builds/runs the independent C transform application
  using only installed artifacts. Both default and explicit-owner paths validate
  4,099 integers over three passes, guards, partial upload and early parent release.
  The explicit path also checks reserved-state rejection, null submit-output rejection,
  abandon/reuse, trim with old handles alive and early public-owner destruction.
  Missing loader failure and SDK revision/ABI/hash/relocation checks pass.

No numerical gates were relaxed; all accepted runs passed. No native Metal compile/
execution, GGML/libplacebo rerun, or video-scale streaming performance acceptance
is claimed. Metal's new calls are explicit unsupported stubs, not native reuse.

## Decision and next bounded step

Keep explicit recording-storage ownership as a preferred controllable candidate:
the caller chooses owner count and when high-water capacity is returned, without
cache admission cliffs or device teardown. Keep the cache as an optional convenience
while comparing surfaces; do not present its thresholds as fundamental limits.
A multi-record arena or resettable owner may still expose a better shape; no exact
byte reservation/limit or parallel host encoding is established here.

Next test **reusable executable recordings**, not another cache-size tweak. Specify
copied roots versus mutable pointed-to data, recorded-object ownership between
executions, heap mutation, storage ownership, and submission/timing/error lifetimes
before adding calls. Use native replay as the stronger control. Also distinguish
the current CPU-buffering cost from a direct-recording backend alternative; passing
wall-time tests does not excuse avoidable host work or prove replay unnecessary.
P2 remains open for that comparison; P1/P3–P7 remain open in the
[fundamental audit](performance-expressibility.md).

## Evidence

- [Radeon report](results/recording-storage-radv-2026-09-20/report.json) and
  [all 90,000 samples](results/recording-storage-radv-2026-09-20/samples.csv).
- [llvmpipe correctness report](results/recording-storage-lvp-2026-09-20/report.json).
- [Runtime/SDK validation receipt](results/recording-storage-validation-2026-09-20.txt).
- [Protocol and reproduction](recording-storage.md#acceptance-and-comparison-protocol).

Retained local runs: `target/performance-frontier/small-311i_ejl` and
`small-pvmlbb5g`. SDK `/tmp/ogpu-sdk-owned-7fa0763`, external application
`/tmp/ogpu-external-7gx4qmrr`. No artifacts were deleted.
