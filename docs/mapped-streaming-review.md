# Next probe: direct host access and independent buffer ranges

Selected after [fixed-command replay](command-list-results.md), 2026-09-20.
This is the bounded P3 acceptance brief. The [native controls are now accepted](host-access-results.md),
selecting a [borrowed host-view candidate](host-view-candidate.md); that public API
has not been implemented or accepted.
The benchmark controls are implemented under `examples/performance_frontier/host_access.*`;
the public runtime remains ABI 15, unchanged.

## Decision

Does the current buffer contract exclude a useful native streaming strategy for
the same work, correctness and declared storage/latency budget? Two separate
restrictions matter: CPU read/write copies, and requiring all GPU uses of the
entire buffer to finish before any CPU access. Separate buffers can express some
range independence, but their allocation count/rounding is not automatically free.
Do not attribute a CPU-copy limitation to whole-buffer synchronization, or vice versa.

Start with native controls before choosing mapping handles, range leases or new
allocator objects. A supported host pointer alone would not solve coherence,
pending-use ownership or noncoherent atom sharing.

## Bounded experiment

Use the existing generated integer transform and exact modulo-32-bit oracle,
guards and alternating inputs. The application directly produces inputs and
consumes outputs; its native path may do so in mapped storage. Do not give that
path precomputed results or omit the producer/consumer work. The copied path must
perform the same work using caller staging plus the current C buffer API.

1. **Copy isolation:** one/two slots, 64 KiB and 4 MiB payload per slot. Compare
   native persistent mappings with native copy-in/copy-out wrappers and current
   OGPU copy-in/copy-out, using separate buffers in all three. Use immutable
   commands, identical shader/barriers and the same slot reuse rules. Report CPU
   production/copy/consumption, submit/wait, wall throughput and completion latency.
   Time staging and mapped access honestly; do not count driver setup as a per-frame
   copy or subtract caller work to manufacture an API-only number.
2. **Range isolation:** native two-buffer versus one-buffer/two-disjoint-range
   ownership, without changing producer/consumer or command policy. Report requested
   bytes, actual Vulkan allocations/types/rounding, host staging and command storage
   separately. With a real submission gate, prove CPU reuse of a completed range
   while the other remains GPU-pending. Keep each range's guard and exact output
   check. A contract counterexample needs the stated allocation/latency constraint;
   merely using one handle instead of two is not a performance proof.
3. Only then select a public candidate or document why current alternatives suffice
   for this case. Distinguish a genuine contract restriction from backend memory
   selection/copy implementation. A small repeatable inherent penalty counts;
   do not set a percentage tolerance that excuses an imposed restriction.

Use one selected Radeon ICD; llvmpipe supplies correctness, never hardware speed
evidence. Require identical memory types between controls on each driver or reject
that comparison. Flush/invalidate noncoherent ranges with correct atom alignment;
do not assume coherent memory, BAR access or equal allocated sizes. Shared atom
ranges may impose additional exclusion even when logical byte ranges do not overlap.
If no suitable noncoherent type exists locally, leave actual noncoherent execution
unvalidated and test the alignment/range arithmetic independently.

Keep validation and timing separate. Predeclare 1,000 correctness frames and three
fresh timing processes of 1,000 frames after 100 drained warmups per configuration,
rotating policy order and retaining all samples/tails. Software uses 64 correctness
frames. Small host-scheduling tests must not depend on relative GPU speed; use a
gate. No timing instrumentation or allocation tracing in primary speed samples.
Keep final outputs/guards outside timing; production/consumption are actual work
inside it. Trace allocations in separate controls and free everything after drain.

## Stop condition

Finish with a revision-identified report separating copy costs, range scheduling
and storage budgets, and a decision on the smallest justified public alternative.
If controls expose a better API, give its ownership/coherence/failure contract and
tests before implementation. If they do not discriminate, report that limitation;
do not stabilize the existing restriction from an inconclusive workload.

No general allocator, unified-memory system, cross-queue scheduling, new GPU,
Metal coordination, replay timestamp design or compiler expansion is included.

## Implemented control protocol — 2026-09-22

`host_access.c` uses the existing generated transform, native/public executable
encoding and timeline/receipt machinery. Native binaries do not link OGPU.
Each slot alternates two inputs, generates them in place, executes the integer
transform and consumes every output word into a checked 64-bit sum. Producer and
consumer work are inside the measured window for every policy. Copied strategies
use one caller-owned staging block per slot for both production and readback;
mapped strategies use none. Shader-visible storage includes 64-byte guards at
both ends. Full element/guard checks are outside primary timing (every frame in
correctness runs, final slot contents in timing); checksums are checked every frame.

The fixed matrix has 14 cases: one/two slots × 64 KiB/4 MiB payload × native-copy,
native-mapped, OGPU-copy, plus shared-native only for two slots. Three fresh rounds
yield 42,000 measured frames after 4,200 warmups. Four separate gate cases cover
both payload sizes and native separate/shared allocations. Traced correctness and
traced timing-mode runs are separate from uninstrumented speed runs. All outputs,
checksums, ordering, matrix completeness, identities, metadata and traces are checked
again by the exporter. Three strategies at one slot and four at two slots mean
rotation is not perfectly balanced across the latter's three rounds.

Each native gate establishes completion of range 0, holds range 1 behind a real
timeline wait, confirms it is pending with a zero-timeout poll, then reads and
rewrites range 0 before opening the gate. Both ranges' subsequent exact outputs
must pass. Failure cleanup signals the gate before any draining wait, including
a rejected gated submission. This proves an available native schedule, not a
hardware-overlap speedup. OGPU is never called in violation of its whole-buffer rule.

Shared slots are rounded to noncoherent atom boundaries. Range flush/invalidate
covers only the containing atoms, with the allocation-end exception; overflow,
shared-atom rejection, coherent bypass and noncoherent call/error behavior have
host tests. Coherent mapped execution does not validate noncoherent hardware.
These rules follow [VkMappedMemoryRange](https://docs.vulkan.org/refpages/latest/refpages/source/VkMappedMemoryRange.html).

No extra device timestamps or copy shaders are added. Native and OGPU reuse
commands from setup; native replay remains serial-use while OGPU supports
simultaneous use. The copy-isolation conclusion must therefore come first from
native-copy versus native-mapped, whose command encoding is identical. Within
each driver/size/slot count, copied/mapped separate allocations must match size
and type. Shared allocation may change count/rounding, but must retain that type.
All budgets report application memory separately from unmeasured command storage.

```sh
python3 examples/performance_frontier/host_access.py --check
# Select one ICD and enable synchronization validation before either GPU command:
python3 examples/performance_frontier/host_access.py
python3 examples/performance_frontier/host_access.py --software
python3 examples/performance_frontier/host_access.py --export /run/report.json /new/evidence
```

The software switch selects the 64-frame correctness-only protocol and can also
be used as a Radeon preflight. Runner subprocesses have a 120-second timeout;
ordinary expected failures are drained in C before freeing resources.
