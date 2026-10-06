# Argument reuse: candidates, not a selected ABI

The [snapshot control](argument-snapshot-results.md) establishes the existing
copy-at-call behavior. The [audit](argument-reuse-plan.md) still has to distinguish
avoidable implementation work from the cost of mandatory per-operation supply.
This comparison narrows what to prototype after the native controls; it is not
authorization to freeze a binding API or introduce two overlapping mechanisms.

## The precise missing expression

A caller can keep one root value unchanged while changing indirect offsets,
draw order, executable choice or command count during re-recording. Native
Vulkan can supply that root once and leave it in place for subsequent compatible
draws. Our current draw calls provide a pointer and size each time, but no promise
that the pointed-to host bytes are unchanged. Equal host addresses do not provide
that promise; the accepted mutation control intentionally depends on this.

Whole-list replay fixes the command sequence as well as root bytes, so it does
not express this independent reuse. Moving values behind a GPU pointer changes
shader loads, storage ownership and mutation rules, and must be evaluated as a
separate strategy. Neither substitutes for comparing the same native root
representation and changed command sequence.

This is a structural distinction, not yet a measured lower bound in nanoseconds.
Count semantic bytes supplied and concrete implementation copies separately;
do not infer physical memory traffic or a mandatory allocation from the contract.

## Candidates

| Shape | Where bytes are captured | Reuse signal | Cost/contract to examine |
|---|---|---|---|
| Current operation-local values | Every draw/dispatch call | None beyond comparing supplied content | Convenient and locally explicit, but binds payload supply to operation count |
| Immutable argument snapshot | Once when creating a snapshot; operations reference it | Stable object identity | Avoid mandatory per-draw content inspection; examine creation/storage costs, retention and whether an object allocation is forced where native needs none |
| Recording-local argument value | Explicit update while recording; subsequent operations consume that value | No intervening update | Closest to native bind-once opportunity without a new public retained object; introduces ordering/state rules that must remain small and precise |

The leading **prototype candidate** is recording-local argument supply, because
the observed missing expression is independent payload update, not a need for a
long-lived resource object. That is not a selection: the immutable-snapshot
alternative may offer clearer composition or cross-recording reuse. Compare both
against changing-command workloads and host allocation/retention budgets before
adding a public primitive. An operation-local convenience can be layered on an
explicit update plus operation if semantics and failure atomicity match; do not
make that convenience constrain the lower-level path.

## Questions a candidate must answer before implementation

- Snapshot timing and ownership: when may caller host bytes be overwritten?
  Must any object/backing allocation exist? Who retains captured values through
  failed recording, compilation, submission, replay and terminal retirement?
- Executable independence: can one value serve compatible executables without
  recopying? How are size and device mismatches rejected? Shader pointer lifetimes
  remain caller-owned; an argument object must not imply pointer tracing.
- State scope: if recording-local, does the value survive rendering boundaries,
  barriers and executable changes? Is compute/graphics state shared or separate?
  Do not invent implicit root restoration merely because the backend uses two
  execution stages. Declare the mapping and any native extra work.
- Failure/empty operations: an invalid update or draw leaves the last valid state
  and recorded work unchanged. Define zero-byte roots, zero-capacity draws,
  missing values and rejected retries before relying on implicit state.
- Re-recording and replay: no accidental inheritance between recordings. Lists
  retain the values captured at compilation, not a mutable caller binding or a
  hidden dependency on an argument handle remaining alive.
- Composition: retain generated named layouts and compatible native artifacts;
  avoid root offsets inferred from executable identity, shader rewriting,
  descriptor coupling or a compulsory scheduler.

The [matched reuse measurements](argument-reuse-results.md) retain native
bind-once as the stronger control and distinguish declared input bytes from
native pushes. Explicit update/reuse is the direction, but the exact surface
remains unselected. In particular, native byte-range updates must be preserved;
compare a four-byte change inside a 64/256-byte root before choosing a
whole-block replacement or immutable-object representation.

A small wall-time gap on this GPU would not erase a
structural disadvantage; a large gap would not by itself establish which
candidate is best. Keep M4's mip/view expansion behind this bounded decision.
