# Indexed scene reuse and native-performance gate

Selected 2026-09-30 after the [matched scene](graphics-scene-public-results.md)
and [contract matrix](indexed-depth-contract-results.md). This is the next
experiment. Its [slot/replay correctness increment](graphics-scene-reuse-results.md)
is now accepted at `637c7db`. The [native multi-record/count correctness probe](graphics-indexed-frontier-results.md)
passes at `62f1ad7`. The [identity-sensitive control](graphics-draw-identity-results.md)
at `e6baf7b` supports the [selected range/count contract](indirect-draw-ranges.md).
The public ABI-19 implementation and [focused migration checks](indirect-draw-range-results.md)
are complete, followed by the [generated range handoff](draw-count-followup.md).
The [small matched range/replay correctness](graphics-range-reuse-results.md)
passes at `6ae1d5a`; **useful-scale/scope controls and timing remain open**. The local
software counted→fixed failure is retained separately; Radeon work can continue.
It refines step 4 of the [slice proposal](indexed-depth-proposal.md).

## Draw-contract decision and remaining acceptance

This checkpoint records a retain/change decision for explicit record ranges,
stride, GPU counts and shader-visible draw identity—not merely a timing report.
First close the semantic mapping: exercise draw identity in single/multi/count
execution and specify retained backing, bounds, dependency and replay behavior.
Do not fuse N calls by assuming their shaders cannot observe the change.

Select the smallest contract preserving the native strategies. A demonstrated
structural restriction may justify the alternative before timing; then implement
its capability/rejection/lifetime gates and compare matched strategies. Existing
one-record behavior is a semantic control, not a compatibility veto. Timing below
remains required to assess implementation costs, not to grant permission for a
better API. Finish this checkpoint before expanding into the remaining M4 slices.

Decision recorded 2026-09-30: select explicit fixed/counted ranges with identity
local to each range. The native identity probe supplies a concrete semantic
distinction; ABI 18 itself does not enable this shader capability. Follow the
selected contract's implementation, lifetime, capability and boundary matrix next.
The measurements below remain acceptance work, not a reason to delay the choice.

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

### First timing increment: grouped, GPU-copy-complete

After the 76,608-frame range reuse gate, measure the 72 grouped 257×193
configurations first, in three fresh process rounds with rotated draw-strategy
order and alternating native/public order. Each process drains a separate
100-frame warmup, then times 1,000 frames using the same slot encoders and shaders.
The runner requires clean sustained correctness with matching source/runtime/
shader hashes before accepting a non-preflight run.

`record_submit_ms` includes host control update, reset/record or list submission,
queue submission and consumed public batch cleanup. `wait_ms` includes observation
and public receipt retirement/destruction; native retains its slot pool.
`retirement_ms` spans the start of control/record/submit to observed retirement,
not GPU execution alone. Wall time spans the complete measured submit/drain
window. Every frame retains the same GPU color/depth/geometry readback copies,
but CPU readback/oracle inspection happens only for each final slot after the
window (and after warmup). Label this **GPU-copy-complete**, not complete host
output consumption or the every-frame correctness runner's wall time.

Validation, diagnostic interposition and implicit layers are disabled only for
these timing processes. Keep raw samples; report per-process medians/nearest-rank
p95 and wall/frame, then summarize processes without pretending samples are
independent repetitions. Scope-break and useful-scale controls remain separate
increments. Do not compare identity-sensitive single and range programs as if
their outputs were equivalent; native/public pairs within each strategy are the
matched comparisons.

## Do not approve an artificially weak native frontier

The shared native setup intentionally enables only the OGPU baseline. Its lack
of enabled multi-draw is **not evidence of hardware absence**. Stronger controls
must query and enable their required features and check indirect-count limits,
with the same device selection and provenance. Keep these experimental features
out of unrelated controls.

Native multi-record execution and GPU count-buffer execution are distinct cases.
After matched single-record grouping, test native multi-record commands, then
GPU-generated active counts (including zero and changing counts) without CPU
readback. ABI 19 now exposes both operations explicitly. Do not assume a
backend can fuse separate calls: provide a legal mapping preserving shader-visible
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
