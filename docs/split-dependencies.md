# Candidate: recording-local split dependencies and explicit replay mode

Selected 2026-09-24 by the [P4 native controls](dependency-scope-results.md).
Implemented at experimental ABI 17; public/native comparison acceptance is pending.
This preserves missing
scheduling freedom, not a promised local speedup. Ordinary barriers stay available.

## Shape

`ogpu_batch_dependency_begin(batch, source_access, destination_access, &point, error)`
captures the first dependency scope at the current recording position.
`ogpu_batch_dependency_end(batch, point, error)` establishes its second scope at
a later position. `point` is an opaque nonzero uint64 recording-local token, not a
public owning event, CPU completion or resource address. Both masks are specified
at begin so native encoding has the complete dependency description early.

Begin selects earlier queue operations in the source access classes' stages;
end selects later queue operations in the destination classes' stages. Availability/
visibility use the access classes. Intervening commands are not included merely
because their stage matches. No resource inference, automatic scheduler or resource-
specific execution guarantee. This preserves A→C without adding B when B is between
the two endpoints; resource-range visibility is a separate unselected alternative.

Each point ends exactly once in the same batch. Reject zero, foreign, stale or
already-ended tokens; invalid calls do not mutate recording and begin clears its
output on failure. Allow nested and crossed intervals; end order need not be LIFO.
Reject submission/compilation with unmatched points without consuming the batch,
allowing correction. Discarding an incomplete recording is always legal.
No point can cross batch or submission boundaries in this first candidate; ordinary
global barriers retain their existing cross-submission contract.

## Replay and ownership

Compilation's execution policy is explicit through a flags argument to
`ogpu_batch_compile`. Flags zero select **serial use**; an explicit
`OGPU_COMMAND_LIST_SIMULTANEOUS` flag permits several unretired executions.
Unknown flags reject without consuming the batch. Serial resubmission while a
previous receipt remains unretired rejects without waiting; observing completion
releases the reservation even if the old receipt survives. Separate compiled lists
can serve separate in-flight slots. Existing commands support either mode.

Vulkan initially supports split dependencies for one-shot batches and serial lists.
Split + simultaneous compilation returns UNSUPPORTED without consuming the batch,
not a silently serialized executable. This is a stated implementation support limit,
not a claim that all native implementations fundamentally need it. Timed replay
remains separately unsupported. Both mode choices need tests; expose no mandatory
simultaneous native flag when the application does not need it.

Native events are private recording/executable resources. Partial preparation owns
every successful allocation immediately. Reset/destroy native command references
before destroying events; recorded buffer/heap/kernel ownership remains unchanged.
One-shot events start unsignaled and need no reset prefix. Serial list replay uses
the validated reset/order prefix outside its split regions. That prefix's broad
cross-execution ordering is a backend limitation to track, not an extra portable
contract promise. No per-submit event allocation or CPU reset call is required.
Do not cache live event state in retired one-shot command storage.

Known unaccepted submission failures release serial reservations and permit retry;
indeterminate failures drain and poison the list as before. Loss remains sticky.
Timeout/transient poll failure retains reservation/resources. Completed list handles
and their objects remain reusable; public destruction never waits. Metal's optional
calls return UNSUPPORTED, with cleared outputs; no native Mac claim.

## Acceptance before retaining the candidate

ABI 17 changes the compile signature. Update every
consumer and installed SDK example together. Add token-state, nested/crossed scope,
invalid-mask/output, incomplete-recording, event-creation failure, serial pending/
retry/loss/retirement and command-before-event cleanup tests. Run the GPU suite on
both existing drivers, plus bindings/ABI/mocks and independent installed consumers.

Add a public split policy with the same event lifetime/reset strategy to the P4
matrix and make public global replay serial to match the native flag. Retain the
original control result as historical evidence; use a new schema/revision for the
comparison. Test real execution/output/guards, not only token metadata. No claim
that matched timings settle concurrent split replay, cross-recording endpoints,
resource-scoped caches, Metal or other performance-audit questions.
