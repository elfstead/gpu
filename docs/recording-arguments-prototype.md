# Selected prototype: recording-local argument byte ranges

Selected 2026-10-06 from the [reuse measurements](argument-reuse-results.md) and
[partial-update control](argument-patch-results.md). This is the next
implementation brief, **not an implemented or stable public API**. ABI 19 remains
current. Prefer this foundation to a compulsory immutable argument object:
native update/reuse needs neither a separately allocated public object nor
object lifetime management, and partial updates must not force full-root
materialization. An immutable convenience can be considered later if useful.

## Logical contract

One recording has one logical argument byte bank shared by compute and graphics.
Its bound is the created device's `max_push_data_bytes`, not a particular
executable's layout. An explicit update specifies a byte offset, caller bytes
and length; it snapshots **only that range** when the call succeeds. Subsequent
operations observe the latest preceding updates. Caller host storage can be
overwritten immediately. Embedded GPU addresses retain the existing caller-owned
pointee lifetime/race obligations; no pointer tracing or implicit retention.

Updates must be four-byte aligned in offset and length, with checked bounds and
overflow. Null data is allowed only for zero length. A zero-length update is a
validated no-op, not clearing or rebinding. Updates are legal inside or outside
rendering scopes while recording. Invalid arguments/state leave the command
sequence unchanged. There is no global/device binding, implicit submission,
resource allocation requirement, shader rewrite or argument indirection.

Unchanged bytes survive partial updates, barriers, scope boundaries and
executable changes. Compute and graphics do **not** automatically restore one
another's previous values. A caller that overwrites a shared range must explicitly
restore it before another shader needs the old content. Compatible executables
can use different prefixes without resupplying unchanged data; layout/type
correctness remains a trusted shader/caller contract, not runtime reflection.

### Initialization is a caller obligation, not compulsory zero-fill

A new recording inherits no defined argument values from any prior recording,
submission or list. The caller must initialize every byte a shader actually
reads before that operation. Unused fields/padding need not be initialized, and
partial writes may leave holes that no shader reads. Do not impose a full-prefix
initialization requirement merely because the executable declares a maximum
push-data extent: that would exclude sparse native initialization strategies.

Reading uninitialized argument bytes is caller misuse, like reading uninitialized
pointee data; it is not promised to produce a recording-time error. Optional
debug diagnostics must not become a mandatory reflection pass, per-operation
payload scan or stronger normative initialization rule. Tests must not execute
deliberately undefined shader reads. Zero-root shaders need no updates, and
zero-capacity draws do not initialize, clear or consume argument state.

The executable's declared push extent still bounds its legal shader accesses
and is validated against device limits at creation. It does not constrain the
bank to that exact current size: writing a larger valid bank range then using a
smaller compatible executable must remain expressible without extra updates.

## Recording, failure and replay ownership

Recorded updates own their copied bytes, not the caller's host pointer. Batches,
compiled lists and accepted submissions retain those updates under their existing
lifetime/retirement rules. Compiling a list fixes update ranges/values and the
operation sequence; later caller writes or recording-storage reuse cannot modify
them. Serial/simultaneous replay must reproduce those same values. No argument
state is inherited from the submitting caller or another list.

Failed individual calls leave prior valid updates intact. Existing operation-local
convenience calls should become a validated update-plus-operation path, preserving
their exact-size checks and failure atomicity: validate the complete operation
before appending either part. A rejected draw must not leave its convenience
root update behind. Successful convenience calls affect the same bank; they do
not secretly save/restore a separate per-operation state. Empty updates preserve
the bank. Preserve all existing explicit buffer/index/count/executable retention.

## Implementation sequence and gates

1. Separate root updates from draw/dispatch steps and from pipeline/index binding
   caches. Retain owned update ranges, not a root Vec per argument-free operation.
   Map updates directly to native range pushes; no per-draw read/copy of caller
   payload. Keep the current convenience path layered, not a second encoder.
2. Add the experimental C surface and generated typed-layout examples. Declare
   the ABI change only with implementation. Give unsupported backends explicit
   results; preserve existing Metal compute behavior without claiming native
   validation or blocking Linux on a Mac round trip.
3. Test bounds/alignment/zero-length, overlap and untouched bytes, host overwrite,
   same-address mutation, compatible executable prefixes, compute/graphics
   interleaving, scope persistence and independent recordings. Test invalid
   update/draw retries and convenience-call atomicity without undefined GPU reads.
4. Test owned storage, compile/replay/retirement and destruction ordering. Re-run
   existing compute/graphics consumers, generated SDK workflows and strict
   counted→fixed regression without a software-driver waiver.
5. Add the candidate to the existing matched bind-once and scalar-update controls.
   Require equal shader/output/slot budgets and actual native update ranges;
   repeat host-sensitive timings. Attribute changes to measured collection and
   encoding phases, not a promised elimination of the entire current gap.

Names and exact function grouping remain implementation choices. No automatic
scheduler, new allocator framework, permanent parallel legacy path or API freeze
is part of this prototype. Mip/views resume after this bounded retain/change
decision, not after an open-ended sequence of ever-larger performance studies.
