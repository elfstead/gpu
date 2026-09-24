# P4: dependency-scope acceptance brief

Selected 2026-09-24 after [HOST-view acceptance](host-view-results.md).
This is the next bounded contract review, not an API expansion or an accepted result.
Use the existing Radeon and llvmpipe; another GPU is not a prerequisite.

## Decision

Can the current global access-class dependency express the strongest identified
legal native producer/consumer schedule under the same correctness, storage and
latency budget? If not, does an explicit dependency endpoint/token, resource scope,
or another small alternative expose the missing information without automatic
pointer analysis or a runtime scheduler?

Separate execution ordering from memory visibility. A resource/range barrier does
not by itself establish that unrelated same-stage commands can execute independently.
Likewise, a faster backend encoding of the same dependency is not an API restriction.

## Review before code

1. Map the C barrier contract and Vulkan lowering, including prior submissions,
   access/stage masks and command-list replay. Identify what is actually promised,
   what is merely current encoding and what callers cannot express.
2. Check current primary Vulkan specification text for pipeline barriers, split
   event dependencies and submission ordering. Write a legal native mapping before
   claiming that a narrower dependency permits overlap. Include event reset/reuse,
   failure and resource-lifetime requirements in any candidate.
3. Start with producer A feeding consumer C and independent work B using disjoint
   storage in the same stage. Compare every legal topological command order under
   the existing contract, not just an intentionally inconvenient A/B/barrier/C
   sequence. If reordering removes the restriction, say so; do not manufacture a
   need for new handles. Add a second dependency only if needed to distinguish a
   real remaining structural restriction, with that reason written down first.

## Minimum discriminating controls

- Strong native global-dependency and native split/scoped alternatives, plus the
  best legal existing-API ordering. Match shaders, dispatch work, memory placement,
  allocation bytes, in-flight count and output requirements. Account for any extra
  event/command objects and host work; a second queue belongs to P1, not this probe.
- Exact integer outputs and guards for dependent and independent regions, checked
  under synchronization validation on both drivers. Make the consumer actually
  depend on producer data; check all regions, completion, failure cleanup and reuse.
  Missing-barrier code is not a performance control and must not execute a data race.
- If the semantic review finds a discriminating case, declare sizes, process count,
  warmups, sample count and stopping condition before collecting acceptance timing.
  Retain full outputs outside primary timing and all timing samples. Separate host
  submission cost, wall throughput and observed latency. Run allocation and any
  timestamp instrumentation separately so they cannot silently change the comparison.
- Treat overlap as a hypothesis to test, not a property proved by API names or
  similar timing. Hardware may serialize legally independent work. No local speedup
  does not prove the missing scheduling freedom irrelevant on native implementations.

## Stop and classify

Finish with one of: demonstrated expressible for the stated graph/budgets; backend
encoding deficiency; contract restriction with a concrete native mapping and a
proposed public alternative; or unresolved with the exact missing evidence.
Neither a single winning sample nor lack of an observed win decides the contract.
Do not implement a new public dependency abstraction until the review identifies
what information it adds and tests its lifetime/failure implications.

This does not reopen accepted host mapping, add queues, create a graph scheduler,
stabilize the API or complete P1/P5/P6/P7. M2 remains the following programming/
compiler milestone; unmeasurable hardware questions stay explicit, not blockers
requiring hardware acquisition or a Mac round trip.

## Semantic review and selected control — 2026-09-24

At `683edea`, `ogpu_batch_barrier` promises a global earlier-to-later queue
dependency, including earlier submissions. Vulkan lowers each public access class
to a stage/access pair and ORs combined masks, then emits one `VkMemoryBarrier2`.
Compute read/write both select COMPUTE_SHADER; transfer selects ALL_TRANSFER;
vertex/fragment/indirect/color select their corresponding stages. There is no
execution-only mask or independent stage selector. Replay preserves the barrier
positions; changing submission boundaries does not narrow the documented scopes.
Automatic HOST→ALL_COMMANDS and ALL_COMMANDS→HOST visibility surround recordings;
they do not replace an internal GPU producer/consumer dependency. Metal currently
uses broader dispatch/blit barriers in three scopes; that is separate backend
evidence, not validation of a new Metal dependency primitive.

The Vulkan specification distinguishes stage-scoped execution from access-scoped
visibility; submission order alone adds neither dependency.
[Synchronization and submission order](https://docs.vulkan.org/spec/latest/chapters/synchronization.html#synchronization-submission-order)
Outside render passes, a pipeline barrier covers earlier/later queue commands,
filtered by its stages. A buffer barrier narrows memory access scopes to its range,
not execution to only commands touching that range.
[Pipeline barriers](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdPipelineBarrier2.html),
[buffer barriers](https://docs.vulkan.org/refpages/latest/refpages/source/VkBufferMemoryBarrier2.html)

For three same-stage read/write dispatches, A and C transform the same X region,
while B transforms disjoint Y. The only application edge is A→C. Enumerating every
topological order and every single-barrier cut that covers that edge gives:

| Recording (`|` = compute barrier) | Required extra execution edge |
|---|---|
| A `|` B C | A→B |
| A B `|` C | B→C |
| B A `|` C | B→C |
| A `|` C B | A→B |

More global barriers only add edges. Different buffers, retained owners, replay or
extra submissions do not remove these same-stage edges. This is a structural
counterexample, not yet a measured hardware cost. In an ideal resource-compatible
schedule with durations A=1, B=2, C=1, A→C alone permits completion at 2; every
listed barrier graph needs at least 3. Those are illustrative time units, not a
GPU forecast or a claim that this shader saturates only part of a GPU.

Native `A; set(event); B; wait(event); C` captures only A in the first compute
scope and C in the second. B is outside both endpoints. Use identical full
dependency descriptions at set/wait (flags zero), no asymmetric-event extension,
and no queue-family transfer. The native mapping is real even if this GPU does
not exploit the freedom.
[Set scope](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetEvent2.html),
[wait scope](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdWaitEvents2.html)
One host-resettable event is owned by the control; reset only after the preceding
submission has completed, including all waits, before reusing the recording.
Drain on failures before releasing the event/commands/buffers. No concurrent
executions share the event. This does not solve simultaneous public-list replay.
[Host reset](https://docs.vulkan.org/refpages/latest/refpages/source/vkResetEvent.html),
[event destruction](https://docs.vulkan.org/refpages/latest/refpages/source/vkDestroyEvent.html)

### Frozen first matrix

Use the existing exact uint32 `3*x+7` transform, X=64 KiB and Y=64 KiB/4 MiB,
each with 64-byte guards on both ends. A/C each execute once on X; B once on Y.
One queue, one in-flight execution, two HOST allocations, fixed compiled commands,
mutable initialized bytes, final output observed on the CPU. No shader fusion,
splitting B, extra queues or extra buffering. Repeated executions use explicit
compute visibility from prior submissions, identical for all policies.

All four listed orders run through native global barriers, native X-buffer-range
barriers and existing public global barriers. Add native split-event A/B/C:
13 policies × two sizes = 26 cases. Every native policy uses the same serial-use
command flags; public lists retain their existing simultaneous-use flag. One
event object/reset per execution is additional native-split cost, recorded rather
than hidden; total driver-private memory is not equated.

Correctness: 256 full-output/guard checks per case on Radeon, 32 on llvmpipe.
Primary Radeon timing: 100 drained warmups, then 1,000 executions in each of three
fresh processes per case, rotating policy order (78,000 measured executions).
Host event reset is included in the submit interval; full readback/oracle work is
outside timing. Retain all samples, wall time, submit/wait intervals and latency.
Separate traced correctness/allocation runs verify equal application allocations
and freeing; no timing layers/tracing or device timestamps in the primary matrix.
Stop after this matrix, report every order and process range, and do not tune the
workload until a desired speedup appears. Lack of local speedup leaves quantitative
cost unresolved; it does not erase the demonstrated scheduling restriction.

### Reset-protocol correction before acceptance

The first full run at `3eaa639` is **rejected**, not accepted performance evidence.
Its separate 1,100-execution allocation control reported `VUID-vkResetEvent-event-03822`
despite successful timeline waits and correct final outputs. Logs remain under
`target/performance-frontier/dependencies-myc82_4w/memory-64-split-a-bc`.
Current VVL source wakes semaphore waiters while retiring a submission, before
removing that submission from its pending-event search. This is consistent with
a tracking race, but is not a verified diagnosis of this installed layer build.
[Queue retirement/search implementation](https://github.com/KhronosGroup/Vulkan-ValidationLayers/blob/main/layers/state_tracker/queue_state.cpp)

The acceptance mapping now uses **recorded GPU reset**, not host reset: before A,
reset with ALL_COMMANDS, then an ALL_COMMANDS→ALL_COMMANDS execution-only barrier
to order reset before set. This also orders earlier event waits before reuse.
Both operations are outside A/B/C, and the control already observes completion
between executions. The extra GPU commands and their cost remain in the split
policy; no queue-idle call, layer suppression or retry-until-clean acceptance.
The host-reset statements above describe the rejected first protocol.
[Reset scopes](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdResetEvent2.html),
[set/reset ordering requirement](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdWaitEvents2.html)
Repeat the declared entire matrix at a new clean revision; do not pool old samples.
The experiment remains serial and does not establish safe event sharing between
simultaneous executions of a command list.
