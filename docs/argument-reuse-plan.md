# Per-operation arguments: explicit reuse audit

Selected 2026-10-04 after the [scope/scale measurements](graphics-scope-scale-results.md).
This is an active performance-preservation question, not a decision to introduce
a bind-state API, argument object or new ABI. The [installed-scene handoff](graphics-scene-handoff-results.md)
is accepted; run this bounded probe before accepting the per-operation argument
shape as fundamental and before expanding the next graphics slice.

## Why the existing classification is insufficient

Draw and dispatch recording snapshot caller argument bytes. The caller may reuse
or modify the same host address immediately after a successful call. Identical
pointer identity therefore cannot prove identical argument content; the backend
must obtain the current values that the shader can observe. Deduplicating copied
roots during encoding removes redundant native pushes, not this call-time
obligation. The accepted range scene uses only an eight-byte vertex root.

Vulkan can leave a previously supplied argument block bound while recording new
draw operations. That hoists argument supply independently of whole-command-list
replay. Repeating the public arguments each call may be a structural disadvantage
even if this scene hides it. Conversely, a measured gap may predominantly come
from avoidable Vec allocation, validation, reference counting or two-pass encoding.
Do not assign the measured gap to either category without separating them.

## Controls and expectations

Use the existing host-sensitive 257×193 single-record scene, grouped in one scope,
one/two slots and capacities 1/64/512. Keep real separate native draw commands and
DrawIndex=0 per call. Use 8/64/256-byte roots where the selected device's declared
limit permits; explicitly report unsupported sizes, never silently reduce them.
Shaders must consume the declared payload, so unused fields cannot disappear.
Native/public artifacts, work, output, slot budgets and dependency policy match
within each root-size case; do not compare different shaders as equivalent work.

Compare three paths with changed command sequences on re-recording:

1. Strong native: bind immutable root bytes once per recording; issue distinct
   draw commands with changing record offsets/order.
2. Native resupply control: present the same root on every draw, isolating repeated
   argument supply from OGPU's collection, validation and retention machinery.
3. Public: current per-operation roots, with its identical native draw sequence.

Report both caller input bytes processed and actual native pushes separately.
An additional native host compare/dedup control can distinguish payload checking
from repeated native command emission. Do not claim exact bytes read from timing
alone; instrument diagnostic controls outside timed runs. Decompose public host
collection and native encoding before prescribing an optimization.

Add a mutation control: reuse one host address but change payload between draws
and overwrite it after recording. Verify that each draw observes its own snapshot,
including equal→changed→equal values and invalid-call retry. Also retain the
immutable-root case across rendering boundaries. A repeated-list control is
useful, but is not a substitute for changing command topology with stable roots.

Do not rewrite DrawIndex, fuse ranges, infer unchanged caller memory, add hidden
CPU/GPU synchronization, or change the shader representation to hide the cost.
Moving parameters behind a GPU pointer is a separate mapped strategy: include its
extra shader loads, backing/lifetime and mutation rules rather than assuming it
is equivalent to native pushed arguments.

## Decision rule

First document whether an efficient native root-reuse strategy is actually
expressible with the same semantics and budgets. A structural counterexample can
justify a better API alternative without a large local timing win. Candidate
alternatives include an explicitly immutable argument snapshot or recording-local
argument binding; compare those against the current operation-local values before
selecting a surface. Preserve typed offline-generated layouts, explicit pointer
lifetimes, independent executable choice, retry behavior and replay ownership.

If a candidate is selected, declare snapshot/binding lifetime, replacement,
cross-device/size rejection, empty operations, boundary behavior, failed recording
and destruction/retirement before implementation. Keep convenience per-call
arguments only if they are clearly layered and do not constrain the foundation.
No current timing result authorizes general parity or API freezing.

## First implementation increment

`examples/graphics_scene/argument_snapshot.py` implements a diagnostic precursor,
not the strategy/timing matrix above. It uses 8/64/256-byte roots with observable
pointer and weighted payload fields, four saved rendering scopes, same-address
mutation within a scope, immediate host overwrite, and wrong-size rejection with
valid retry. Native and public use exact same-size shader artifacts, an independent
interior color/depth oracle and complete output-byte comparison. The eight-byte
control queries limits before larger variants; unsupported sizes remain explicit.
The extra saved-output allocation is a correctness instrument, not a timing budget.
[Clean acceptance](argument-snapshot-results.md) at `328282b` passes all three
sizes on both local drivers. [Candidate alternatives](argument-reuse-alternatives.md)
separate immutable snapshots from recording-local values; neither is selected.

The subsequent [accepted reuse matrix](argument-reuse-results.md) at `955bdd7`
covers changing sequences, bind-once/resupply, input/native-push counts and host
phase boundaries. Partial native updates and candidate contract selection remain;
the optional host compare/dedup control has not been measured.
Passing snapshot semantics is not evidence that obligating every caller to
resupply the bytes is the best fundamental design.

## Matched reuse controls and timing protocol

`examples/graphics_scene/argument_reuse.py --native IDENTITY_REPORT --snapshots
SNAPSHOT_REPORT` implements the next matrix. The clean references establish exact
geometry/coverage, observable root artifacts and device limits; reference colors
are independently remapped for each constant payload, without changing depth,
edges or guards. New native/public executions must match every reference byte.
Larger unsupported root sizes are explicit, based on the accepted same-device
snapshot limits, and pipeline creation still validates the current device.

Three paths use 8/64/256-byte roots, 1/64/512 separate draws and one/two slots.
Each slot's root is constructed once. Re-recording alternates forward/reverse
record offsets, including GPU-zeroed holes, while preserving local DrawIndex=0.
There is one scope per recording, no fusion, root mutation, list replay or added
GPU parameter indirection. Native bind-once and resupply differ only in argument
push placement; public uses its existing consecutive-state elision. Storage,
readback and dependency policies remain the accepted range-control policies
(including the existing public/native barrier-count difference).

Diagnostic runs count root supply calls/declared bytes at call boundaries and
trace actual native push bytes, draw order directions, scopes and draws. These
are not physical CPU load or cache-traffic measurements. `Batch::draw` currently
copies the supplied bytes into each retained step; the source audit identifies
that allocation/copy, not a new hardware counter. Every frame checks full output,
geometry/count/control/guards, allocation size/type multisets, peak/lifetime
budgets, native storage resets and absence of successful queue-idle. Default is
64 frames/configuration; 16 is preflight and 992 provides complete sustained
eight-generation cycles with two slots. The 54-case full matrix includes all
three root sizes. No graphics argument API change is part of these controls.

`argument_timing.py --correctness REPORT` requires clean matching sources,
runtime and complete correctness. It executes 162 fresh processes: three rounds
of the 54-case matrix, rotating path and root-size order. Each process drains 100
warmups, measures 992 samples, then checks every final slot byte after both
windows. Validation, command tracing and argument-supply counters are disabled.
`--preflight` is one round/16 samples and never timing acceptance. Full GPU
readback copies remain in the measured command sequence; no isolated GPU time
or universal parity claim follows.

The existing wall/host/wait/retirement clocks remain. Four additional monotonic
clock reads per frame, identically placed on native/public paths, bracket:

- Public: batch creation plus command collection; then batch submit, which
  includes deferred native encoding/finalization and queue submission.
- Native: pool reset/begin plus native recording/finalization; then submission
  preparation and the queue call.

These are deliberately different phase meanings, not interchangeable labels for
pure driver encoding. HOST control writes and remaining bookkeeping stay outside
the subintervals but inside whole record+submit. Clock overhead remains in wall
measurements; do not present this as an uninstrumented cost or assign all public
collection time to copying roots. The optional compare/dedup native control and
candidate contract implementation remain separate decisions after this evidence.

## Selected follow-up: partial native updates

The [accepted matrix](argument-reuse-results.md) supports explicit argument
update/reuse as the direction, not a whole-block-only final shape. Use the
existing observable 64/256-byte shaders with one changed trailing scalar. Compare
native full resupply, native four-byte update and current public full arguments.
Save each A/A/B/A scope, poison host input after recording, validate all output
and geometry bytes, and trace actual update byte ranges on both local drivers.
No timing claim or public API change is needed for this bounded control. Then
settle initialization/coverage, partial replacement, compute/graphics interaction,
scope persistence, invalid retry, zero-byte/empty operations and list ownership
for the selected candidate before implementation.

`examples/graphics_scene/argument_patch.py --snapshots SNAPSHOT_REPORT` implements
this control using the accepted native shader bytes. The two shapes are 64/256
bytes; only the final scalar changes in B, with no pointer or geometry change.
Each scope follows a compute update, so native partial mode explicitly initializes
the whole graphics value before its first draw; between draws it either keeps A
or updates exactly the last four bytes. It does not assume separate compute and
graphics argument banks or implicit restoration. Native full/public full controls
use the same saved-output allocations, shaders and generated geometry. Actual
offset/size events, draw count, memory lifetime and every saved output are checked.
This diagnostic includes the earlier snapshot fixture's unused alternate-pointee
buffer; it is equal across paths, not a minimum-allocation or timing claim.
Clean acceptance is recorded separately from development tests.
