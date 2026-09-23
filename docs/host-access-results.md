# Host access: accepted native controls — 2026-09-22

At clean control revision `ef723f0fcecc1ae314bfd395590b11de6090ed6b`, the
[P3 experiment](mapped-streaming-review.md) exposes a better API alternative:
**direct host access with explicit range-scoped cache maintenance**. The runtime
is unchanged at ABI 15. This accepts the controls and selects the next candidate;
it does not claim that OGPU already supports mapped or independently synchronized
host ranges.

Follow-up: [ABI-16 public-view acceptance](host-view-results.md) now demonstrates
these strategies through the public API. The ABI-15 results below remain historical
controls, not the current support boundary.

The important evidence is native-copy versus native-mapped: identical commands,
buffer allocation/type, shader, slot count, producer and consumer. The native
producer can write into mapped storage and the consumer can read from it, while
the existing public API requires independent caller storage and copies. This
application boundary cannot be repaired solely by replacing the backend's memcpy:
the API cannot return the storage into which the application should produce data.

## Copy cost versus throughput

Wall milliseconds per **1,000 frames**, median of three fresh timing processes.
Parentheses are process min–max, not confidence intervals. All 42,000 raw samples
and individual interval/tail statistics are retained.

| Slots / payload | Native copied | Native mapped | OGPU copied | Native shared/mapped |
|---|---:|---:|---:|---:|
| 1 / 64 KiB | 74.292 (73.590–77.324) | 73.110 (67.863–75.419) | 75.883 (74.581–78.003) | — |
| 1 / 4 MiB | 1447.565 (1447.295–1495.077) | 1267.502 (1264.699–1279.782) | 1434.973 (1428.852–1442.912) | — |
| 2 / 64 KiB | 30.927 (28.647–33.219) | 28.603 (28.552–30.804) | 29.257 (29.036–29.800) | 29.435 (28.333–30.924) |
| 2 / 4 MiB | 956.723 (954.735–974.521) | 961.510 (959.762–975.657) | 964.501 (954.468–979.222) | 955.577 (955.212–978.102) |

At one slot/4 MiB, mapped native takes **12.4% less wall time**, or 14.2% higher
throughput, than copied native. Its three process windows are all below the copied
ones. It is also 11.7% lower in wall time than current OGPU. The smaller cases are
noisier; no statistical equivalence or uniform speedup is claimed.

At two slots/4 MiB, mapping does **not** improve throughput in this sample: separate
mapped is 0.5% slower than copied native, with overlapping process ranges. Additional
in-flight work hides much of the CPU-copy cost. That does not make forced copying
free, or answer the single-slot latency/storage case.

CPU production + upload/cache operation + readback/cache operation + consumption,
microseconds per frame, median of process medians:

| Slots / payload | Native copied | Native mapped | OGPU copied | Native shared/mapped |
|---|---:|---:|---:|---:|
| 1 / 64 KiB | 6.570 | 4.520 | 6.510 | — |
| 1 / 4 MiB | 452.099 | 271.811 | 437.544 | — |
| 2 / 64 KiB | 6.770 | 4.350 | 6.270 | 4.590 |
| 2 / 4 MiB | 445.910 | 286.621 | 437.769 | 280.601 |

Mapped access lowers native CPU work about **40%/36%** at one/two slots and 4 MiB.
Median process p95 CPU intervals there are 512.313→328.376 µs and
518.064→367.576 µs. Consumer reads are slower directly from mapped storage than
from freshly copied staging (roughly 175–190 versus 124 µs at 4 MiB); the experiment
includes that cost rather than equating removal of memcpy with an equal speedup.
The same source functions perform actual production and summation in every path.
Native/OGPU builds still differ in generated host code/inlining and submission
machinery; their difference is not an isolated API tax. The causal comparison for
copy elimination is the two policies in the same native binary.

Median observed completion latency at one slot/4 MiB is 1.437 ms native copied
versus 1.259 ms mapped. At two slots, both are about 1.91 ms. Do not compare the
two-slot throughput benefit without its extra storage and latency. Setup takes
12.7–16.3 ms across individual processes and includes device/program creation;
it is not isolated mapping cost.

## Range scheduling and storage

Four real gated cases per driver cover both payload sizes with separate/shared
native allocations. Range 0 completes, range 1 is held behind a timeline wait,
and a zero-timeout poll confirms it remains pending before **and after** CPU
read/rewrite/publication of range 0. Opening the gate and executing both ranges
then passes exact outputs/guards. This is a native schedule excluded by the
current whole-buffer host-access rule; no illegal OGPU call is used to demonstrate it.

The shared variant changes two allocation objects to one. On these drivers it
does **not** save allocated bytes or establish a throughput advantage:

| Payload per slot | One separate slot: GPU bytes / caller staging | Two separate slots: GPU bytes / caller staging | Two shared mapped ranges: GPU bytes / caller staging |
|---|---:|---:|---:|
| 64 KiB | 65,664 / 65,664 copied, 0 mapped | 131,328 / 131,328 copied, 0 mapped | 131,328 / 0 |
| 4 MiB | 4,194,432 / 4,194,432 copied, 0 mapped | 8,388,864 / 8,388,864 copied, 0 mapped | 8,388,864 / 0 |

GPU byte counts include 128 guard bytes per slot and match allocated bytes here.
Host counts are requested staging bytes, not malloc bookkeeping or total RSS.
Command pools, CPU command vectors, executable/device objects and driver-private
storage are not included. Command count and scheduling are unchanged by sharing.
The gate establishes range expressibility; **only under a stated one-allocation
constraint** does it exclude the separate-buffer workaround. It is not a measured
general allocation-performance benefit. Direct mapping separately eliminates
staging storage without changing the GPU allocation count or byte budget.

All controls use the same selected type within a driver: Radeon type 5, flags 14
(host-visible/coherent/cached), and llvmpipe type 0, flags 15 (also device-local).
Both report a 64-byte noncoherent atom. Shared ranges are padded to atom boundaries;
the selected sizes need no additional padding. Neither device exposes an eligible
ordinary host-visible noncoherent type in this enumeration. Actual noncoherent GPU
execution remains unvalidated; arithmetic and flush/invalidate range/error behavior
have host tests. Coherent results do not justify omitting cache operations from
the prospective public contract.

## Acceptance and scope

- Both reports identify clean `ef723f0`, the same runtime and benchmark artifact
  hashes, explicit single-driver selection and synchronization validation.
- Radeon: 14,000 exact correctness frames; 42,000 uninstrumented timing frames
  after 4,200 drained warmups; 14 separate allocation runs add 15,400 frames.
  Software: 896 correctness frames, no speed claim. Each driver also executes
  four gates with three GPU executions each.
- Full outputs/guards, alternating per-slot input, checked output sums, final
  buffer checks and draining/freeing pass. All 72 traced application allocations
  across both drivers' correctness/allocation/gate controls are freed.
- Native/public C selftests, noncoherent range/failure/coherent-bypass tests, gate
  rejection cleanup, five evidence-parser tests and the existing eight frontier
  runner tests pass. The gate opens before error-path draining to avoid deadlock.
  Thirty-eight ordinary Rust tests and formatting pass; no runtime change needed
  a new GPU-suite or SDK acceptance claim.
- Export rechecks complete fixed matrices, all samples/statistics, source/log
  hashes, device identity, native memory metadata, gate size/markers and allocation
  signatures. No primary timing includes validation layers or allocation tracing.

Environment: RX 5700 XT / RADV Mesa 26.2.1, Vulkan 1.4.354; Ryzen 9 5900X,
Linux 6.18.47. Software: llvmpipe LLVM 21.1.8. No exclusive machine, affinity or
clock control. Rotated ordering is not perfectly balanced for four two-slot
strategies over three rounds. No new GPU or Mac dependency. This does not evaluate
DEVICE/BAR mapping, arbitrary applications, noncoherent hardware, host-parallel
recording, cross-queue overlap or a general allocator.

## Decision

Copy-only access should remain a convenience, **not the sole fundamental host
interface**. Select the [borrowed host-view candidate](host-view-candidate.md): a
stable CPU pointer plus explicit range publication/invalidation and a reported
independent-access granularity, with caller-owned synchronization/lifetime.
No tracking inference from shader addresses or hidden whole-device wait.

The next implementation must reproduce both native mapped controls through the
public API, with matching memory budgets, ownership/error tests and an independent
installed consumer. The candidate remains experimental until that evidence exists.
Keep P3 open until then; do not declare the whole performance audit passed.

Evidence: [Radeon report](results/host-access-radv-2026-09-22/report.json),
[42,000 samples](results/host-access-radv-2026-09-22/samples.csv),
[llvmpipe report](results/host-access-lvp-2026-09-22/report.json), and
[validation receipt](results/host-access-validation-2026-09-22.txt).
Full local runs remain at `target/performance-frontier/host-lczyxobx` and
`host-27gb7lrq`; dirty Radeon preflight `host-4xsd_zcx` is not acceptance evidence.
