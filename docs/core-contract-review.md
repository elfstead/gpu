# Core-contract review before stabilization

Selected 2026-09-30. This is a bounded decision gate after the graphics/ML consumer
milestones, not a new implementation milestone blocking M4/M5 or experimental
packaging. It applies the [performance-preservation rule](performance-expressibility.md).

## Required dispositions

For each concern below, write a short acceptance brief when activated. Compare
(1) a strong native strategy, (2) the best concretely mapped public strategy and
(3) the implementation. Include ownership, dependencies, failure/retirement,
host work and equal storage/latency budgets. Do not infer hidden pointer analysis,
command fusion, scheduling or allocation policies that the contract cannot express.

| Concern | Bounded question / proposed control | Current evidence boundary |
|---|---|---|
| GPU-driven work and replay | Mutable indirect ranges/counts, draw identity and changing work without CPU readback; distinguish fixed replay from changing command topology | M4 draw decision supplies its bounded result; wider device-generated commands are not thereby accepted |
| Argument/state reuse | Hoist unchanged argument supply independently of changed command topology; compare immutable snapshots/recording-local binding with per-call value snapshots | [Active M4 follow-up](argument-reuse-plan.md), pulled forward after scope/scale timing; not deferred to stabilization |
| Descriptor independence | Update an unused slot while another slot is in flight; compare with duplicated immutable heaps under equal budgets | Whole-heap mutation restriction remains; consumer/control brief not yet selected |
| Allocation, placement and aliasing | Compare a selected suballocation/streaming or transient-image workload with a native placement/aliasing strategy, including backing and visibility lifetime | HOST ranges accepted for stated cases; image aliasing, broader placement and noncoherent hardware remain open |
| Host concurrency | Independent command preparation from two host threads with explicit shared-object rules | Current host serialization is a subset restriction, not a demonstrated permanent requirement |
| GPU concurrency | One-queue versus explicit transfer/compute queues for a selected dependency graph, including cross-queue drain/failure rules | Queue handles do not prove overlap; available hardware may leave performance unresolved |
| Executable freedom | Native mapping of M4 graphics and M5 subgroup/numerical requirements, including unsupported cases | Pinned Slang is selected; no new frontend unless a named consumer exposes a better alternative |
| Portability boundary | Map common ownership/dependency semantics and explicit optional capabilities without hidden expensive fallback | M6 needs native Mac execution; Linux checks cannot accept mixed Metal parity |

These are proposed controls, not already executed or scheduled acceptance matrices.
Reuse accepted evidence when it answers the question; do not rerun every historical
experiment or demand hardware the user has not supplied. Missing hardware is
recorded as missing evidence, never as a passing skip.

## Exit record

Every concern ends in one of: preserved for stated cases; an implementation cost;
an adopted better API alternative with its acceptance obligations; or explicitly
unsupported/unvalidated with the missing evidence and scope limitation named.
A structural counterexample can justify revision without a local timing win.
No finite benchmark suite proves universal native-performance equivalence.

The review can finish with open capabilities. It cannot turn those capabilities
into approved fundamental limitations or support a universal performance claim.
The subsequent stabilization discussion must identify exactly which contract and
capability scope could be promised, with broader hardware/backend and independent
adoption evidence assessed separately. Closure of M1–M5 establishes useful bounded
consumers, not blanket approval of the foundation.
