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
