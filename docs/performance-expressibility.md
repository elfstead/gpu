# Performance-preserving expressibility

Selected 2026-09-19 after M1. This is a standing design gate, not a claim that
the existing API passes it or a replacement for M1's accepted measurements.

## Fundamental question

For the same workload, hardware, correctness requirements and resource/latency
budget, could a native implementation of this API attain the performance available
through Vulkan? An unavoidable disadvantage imposed by the public contract is a
reason to revise that contract, even if the current flagship hides its cost.
No finite benchmark suite proves a universal yes. Track structural restrictions
and concrete counterexamples; unavailable hardware leaves evidence unresolved.
API changes are welcome whenever evidence exposes a better API alternative.

Distinguish three points:

1. A strong native strategy, unconstrained by OGPU's current execution policy.
2. The best identified strategy expressible through OGPU, with a concrete legal
   native mapping. This is not necessarily the current backend implementation.
3. The current backend executing that strategy.

A difference between 1 and 2 challenges the contract; between 2 and 3 challenges
the implementation. Do not assume hidden caching, pointer analysis or a future
driver recovers information the application cannot communicate. A proposed mapping
must account for host work, storage, resource identity, synchronization, failure
and retirement, not merely generate equivalent GPU results.

M1 deliberately matched barriers, allocation policy, one-shot pools and one frame
in flight. It diagnoses that implementation strategy, not the attainable native
frontier. Keep its results; qualify the retain decision as provisional.

## Initial contract audit

Source of obligations: `include/ogpu.h`; implementation evidence is separate.
These restrictions are real; their workload-specific performance consequences
are not automatically proven. Alternative APIs below are candidates, not decisions.

| ID | Current contract | Potentially excluded strategy / required check | Initial disposition |
|---|---|---|---|
| P1 | One queue; all device/child host calls externally serialized | Independent command preparation; explicit cross-engine transfer/compute scheduling. One queue does not mean one frame in flight. Inventory queue families; test single-queue slots first, cross-queue control separately. | Unresolved; separate host and GPU concurrency questions |
| P2 | Batches consumed by submission; roots copied per recording | Explicit reusable command sequence with stable resources and mutable pointed-to data. Pool reuse may be a backend fix; avoiding repeated public recording needs a concrete mapping, not assumed command deduplication. | Small-dispatch experiment selected |
| P3 | CPU buffer access copies; all uses of entire allocation must complete | Direct production/consumption in mapped storage; independent streaming ranges. Separate buffers are legal, but include allocation count, bytes and CPU copies in their cost. | Streaming experiment selected; direct mapping/range case remains separate |
| P4 | Global access-class dependency, no resource/range identity | Express a producer/consumer dependency without ordering unrelated same-stage work. Audit execution ordering separately from visibility; a resource barrier alone is not automatically independent scheduling. | Unresolved; dependency-DAG control required |
| P5 | Any recorded/unretired heap use excludes all mutation | Update an unused descriptor slot during unrelated reads. Multiple immutable heaps have storage/rebinding/lifetime costs, not assumed equivalence. | Unresolved; descriptor-streaming probe required |
| P6 | HOST/DEVICE placement; dedicated buffers; narrow image policy | Caller-selected placement, direct device-local host access, suballocation and format/layout choices. Compare concrete cases under equal memory budgets. | Unresolved; platform-specific evidence required |
| P7 | Generated artifact/root and supported shader subset | Preserve device-code optimization opportunities, including subgroup/workgroup structure and graphics interfaces. Distinguish tool rejection from runtime restriction. | M2 compiler/output evidence required |

No implicit ownership inference, graph scheduler, legacy backend or stable ABI
promise is selected. A restriction can be absent from the current experimental
subset without being acceptable as a permanent fundamental limit.

## First experiment: repeated small compute

Use the existing generated integer transform, exact modulo-32-bit CPU reference,
guarded buffers and identical shaders. Sweep 1 and 64 dependent dispatches per
submission with one and three independent in-flight slots. Each slot has its own
buffer; no CPU access until its uses complete. This is deliberately host-sensitive,
not an application throughput or ML-quality claim.

Compare OGPU one-shot batches, native fresh pools, native reset/re-recorded pools,
and native pre-recorded replay. Keep successful execution off queue/device-idle;
use timeline completion. Replay is explicitly a stronger strategy, not a matched
implementation control. Native reset isolates pool allocation from repeated
encoding. The native controls share low-level setup, not OGPU execution.

Before timing: validate every retired frame against the full integer reference and
guards for 1,000 frames on Radeon; a smaller software-driver run checks correctness
only. Timing: 100 warmup frames, then 1,000 measured frames in three fresh processes
per configuration, rotating strategy order. Drain warmup before the timed window.
Retain all samples, wall throughput, observed completion latency, host record/
submit/wait costs, setup cost and allocation evidence. No timestamps in the primary
host-sensitive run; instrumentation is a separate control. Validate final buffers
after timing. No GPU allocation growth with frame count.

Compare at equal slot count first. Slot counts have different storage and latency
budgets; do not mix them into one speedup. Report noise/ranges and tails, not just
the fastest process. Recheck mismatches before attributing them to semantics.
A repeatable small inherent cost counts; no percentage threshold excuses it.

## Second experiment: streaming learned-image

Use the frozen model and generated shaders. Begin with 720p and odd extents; 4K is
a later size check, not a prerequisite for testing the execution contract. Compare
serial, two-slot and three-slot execution, 1,000 A/B frames, with per-slot staging/
intermediates/targets and completion-associated reuse. Check full correctness
outside timing against the accepted reference. Report throughput, observed latency,
CPU staging/readback, memory and allocation counts. Extra buffering must include
its latency/storage cost. No extra GPU or Mac is needed for single-queue work.

Allow native pool reuse, replay and narrower legally sufficient dependencies.
Record which opportunities OGPU expresses and which it cannot. Separate direct
mapped/range and cross-queue variants so causes remain distinguishable. Query
physical queue capabilities: multiple handles do not prove hardware overlap, and
no win here does not settle other hardware. Keep unexecuted overlap/mapping/
descriptor/compiler concerns explicitly open.

## Acceptance and progression

Each concern ends as demonstrated expressible for a stated case, an implementation
deficiency, an API-imposed disadvantage with a proposed/tested alternative, or
unresolved with missing evidence. Include correctness, failure/drain and lifetime
obligations. Never close a concern because GPU-heavy work hides it. No tested
native strategy is assumed globally optimal.

Commit brief, implementation, then revision-identified raw evidence and decision.
Preserve failures and resource limits. The first two experiments are pulled forward
from M3; they inform M2 rather than freezing its host contract. Remaining M3
lifecycle/diagnostic work is not completed by timing. Public contract changes need
their own tests and ABI review.

## First accepted result

The [small-compute control](performance-frontier-small.md) is complete at `c437fb9`.
It exposes a substantial storage-lifecycle opportunity concealed by M1's matched
policy. Native reset/re-record recovers most of the gain; replay further reduces
host encoding but does not win every throughput case. Current retirement wording
also needs review: releasing submission references need not mean destroying all
reusable native storage. This is not approval of the existing API or completion of
the streaming, cross-queue, mapping, descriptor or compiler checks.

The [streaming result](performance-frontier-stream.md) completes the second probe
at `ed50e00`, accepted 2026-09-20: 72,000 measured frames, full final-output checks,
post-window input/weight/guard integrity and matched allocation cleanup. Two slots
improve throughput at a clear memory/latency cost; three add no useful throughput
here. The [storage-retirement review](command-storage-review.md) is the next local
design/implementation gate. Neither result settles the other audit concerns or
establishes an unavoidable quantitative API overhead.

At ABI 13, the [storage experiment](command-storage-results.md) reduces the four
small-compute OGPU/native-reset wall ratios to 0.994–1.050, with new safety and
correctness checks. P2 now separates a demonstrated storage-policy improvement
from unresolved caller-owned storage and executable replay alternatives. The
device cache's admission/count bounds are not equal total-memory-budget evidence;
do not pass the fundamental gate from these warmed timings.

ABI 14's [explicit-owner result](recording-storage-results.md) addresses caller
reuse/release and the cache admission cliff (0.991–1.037 of native reset wall time
in six new cases). Keep explicit ownership as a candidate, not a final byte-budget
or arena/handle design. P2 now selects reusable executable recordings and examines
the prototype's remaining CPU buffering separately. The native replay advantage
in host work is still relevant even where GPU-heavy throughput is similar.

ABI 15's [command-list result](command-list-results.md) demonstrates encode-once
execution for fixed roots/addresses/launches with mutable pointed-to data. At 512
dispatches, OGPU host record/submit falls about 88%, to 13–14 µs versus native
replay's 13 µs. Native replay remains the baseline: compiled wall ratios span
0.967–1.044 and replay loses to re-recording in one case. Keep both strategies.
This resolves the fixed-command restriction in P2, not timed replay, changing
commands, final handle ergonomics, equal native memory budgets or any other P item.
The [next bounded probe](mapped-streaming-review.md) isolates P3's CPU copying
and whole-buffer exclusion before selecting a mapping/range surface. Backend
step-vector tuning is separate from that contract decision.
