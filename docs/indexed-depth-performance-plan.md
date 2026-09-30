# Indexed scene reuse and native-performance gate

Selected 2026-09-30 after the [matched scene](graphics-scene-public-results.md)
and [contract matrix](indexed-depth-contract-results.md). This is the next
experiment. Its [slot/replay correctness increment](graphics-scene-reuse-results.md)
is now accepted at `637c7db`; **timing and the stronger native frontier remain open**.
It refines step 4 of the [slice proposal](indexed-depth-proposal.md).

## First increment: stable slots and replay

Use one device/queue, shared prepared compute/raster executables, and one or two
independent slots. Each slot owns stable color/depth attachments, DEVICE geometry,
indices and indirect records, HOST control/readback storage, and exactly one
caller-owned recording-storage owner or serial compiled list. Do not create one
device per slot or a separate precompiled list for each A/B state.

The existing scene passes phase/empty values directly in its copied compute root.
For replay, move those values into a small per-slot input allocation addressed by
an immutable root. Compile the same revised shader for native and public controls.
Update input only after that slot's previous execution retires; compute still
generates geometry and records. Do not emulate replay with CPU-generated geometry,
recompilation, an extra submission per phase change or many prebuilt variants.

Separate recording, submission and observation. Alternate A/B on **each reuse of
each slot**, not global frame parity (which leaves two slots permanently assigned
A/B). Submit both slots before observing either, then retire the oldest required
slot before reuse. Keep immutable list backing until list destruction as well as
completion. No queue-idle call on a successful hot path.

Validate reset/re-record and serial replay at one/two slots against the serial
reference and analytic oracle. Check every retired output and all guards; include
empty counts and independent attachment preservation. Use 1,000 small frames per
case on Radeon, 64 on llvmpipe, and useful-scale A/B/A on Radeon. Report input/output
visibility, allocation counts, peak/requested bytes, maximum outstanding receipts
and observed slot generations. Two unretired receipts establish the consumer's
schedule, not physical GPU overlap.

## Host-sensitive measurements

Use 1/64/512 indexed indirect records, GPU-generated in one contiguous allocation.
Match geometry, shader artifacts, depth state, outputs, copy policy and slot budgets.
Keep the selected 257×193 target and the 1280×720 scene; the latter cannot replace
the small host-sensitive cases.

| Control | What it isolates |
|---|---|
| Native grouped, reset/re-record | One scope and stable pipeline/index binding, N single-record draw commands |
| Public grouped, owned storage | Same grouping and slot policy through the C boundary |
| Native/public grouped, serial replay | Same immutable work and mutable input; one sequence per slot |
| Labelled native/public per-draw scopes | Avoidable scope breaks, LOAD preservation and required dependencies—not an API requirement |
| Native multi-record indirect, where supported | Stronger native strategy, distinct from the matched N-call baseline |

Per-draw scopes clear only the first scope and preserve subsequent ones. Keep
fixed preparation outside hot measurements and count it separately. Report binds,
root pushes, scopes, barriers, encodes, submissions, resets and allocations in
validation/traced controls. The current backend repeats pipeline/index/root
encoding per public draw; that is not itself a contract requirement.

Correctness/tracing/validation runs precede timing. Use three fresh Radeon
processes per timing configuration in rotated strategy order, 100 warmups and
1,000 measured frames. Report per-process host record/submit distributions, wall
throughput and retirement latency; do not pool correlated samples into one huge
independent population. Require equal slot counts and declared storage/input/
readback budgets. Unmeasurable private command-memory bytes stay unknown; object
counts are not a substitute. No llvmpipe timing claim.

## Do not approve an artificially weak native frontier

The shared native setup intentionally enables only the OGPU baseline. Its lack
of enabled multi-draw is **not evidence of hardware absence**. Stronger controls
must query and enable their required features and check indirect-count limits,
with the same device selection and provenance. Keep these experimental features
out of unrelated controls.

Native multi-record execution and GPU count-buffer execution are distinct cases.
After matched single-record grouping, test native multi-record commands, then
GPU-generated active counts (including zero and changing counts) without CPU
readback. The current public surface exposes neither operation. Do not assume a
future backend can fuse calls: provide a legal mapping preserving shader-visible
draw identity, mutable record/count semantics, ownership, ordering and budgets.
Otherwise leave the strategy unexpressed and consider the better API alternative.

A native advantage from fewer calls/binds is implementation evidence until its
cause is isolated. An efficient native strategy unexpressible under the same
semantics/budgets challenges the contract even if this GPU or scene hides its
advantage. Conversely, preserving one strategy does not approve untested dynamic
depth state, depth-only rendering, transient memory or multi-queue scheduling.

Record retained choices, implementation costs, exposed API alternatives and
unresolved cases. Generated/installed-scene handoff follows, then M4 mip/views.
No second physical GPU or Metal prerequisite.
