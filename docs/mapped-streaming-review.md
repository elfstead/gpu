# Next probe: direct host access and independent buffer ranges

Selected after [fixed-command replay](command-list-results.md), 2026-09-20.
This is a bounded P3 acceptance brief, **not an implemented API or accepted result**.

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
