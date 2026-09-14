# Two-frame libplacebo execution

Selected 2026-09-14, D4 follow-up. Test whether consumer-owned bounded scheduling
and resource reuse remain practical through the current public OGPU API. Compare
the existing wait-after-operation control with two caller-managed frame slots.
Keep pinned libplacebo `3330a515d62139259c26239014f286e233bd3a5c`, EWA Lanczos
compute, nearest raster, formats, extents and numerical limits unchanged.

## Implementation boundary

The adapter will expose begin/end/collect helpers for exactly two frame slots.
These are consumer glue, not additions to `include/ogpu.h`. Keep one submission
per operation to isolate scheduling from command batching. Each prepared pass has
two mutable image/sampler-heap and vertex-data banks; compiled executables remain
shared. Frame receipts retain submitted operations. Collection observes terminal
completion for every receipt before reusing its resources, then delivers queued
host readbacks using libplacebo's transfer callbacks. Uploads copy caller bytes
before returning; callback destinations remain live until delivery. No worker
thread, hidden unbounded queue, central runtime allocator or additional GPU queue.

Use two independent source/intermediate/output texture sets and one upstream
dispatch cache/LUT per extent. Keep transfers packed and whole-image. Read-only
LUT sharing is allowed; mutable staging and pass resources must not be overwritten
while outstanding. A slot may be reused only after explicit collection. Unsupported
scope, capacity overflow or premature reuse must fail closed without hidden waits.
Finish/destruction drain outstanding work; failure cleanup must also preserve
GPU operand and callback-buffer lifetimes. Calls and callbacks remain serialized;
the bounded acceptance callbacks only mark completion and do not reenter the API.

## Gates declared before asynchronous results

- Run 12 frames per extent (36 total), repeating the original A/B/A inputs, in
  synchronous control and two-slot mode. Compare all intermediate/final images
  against the corresponding existing same-driver reference frame: RGB tolerance
  <=2/255, alpha exactly 255. Intermediate and final must still match exactly;
  repeated A/B output, upstream executable reuse and teardown checks remain.
- Submit frames 0 and 1 before collecting either. Reach exactly two outstanding
  frame receipts, never three; repeat slot reuse across the full sequence. This
  establishes host-side overlap, not simultaneous execution on the single GPU queue.
- No completion wait inside ordinary async upload/pass/download recording. Poll
  at collection, wait only when needed for the selected slot. Report submissions,
  waits, polls, frame depth, callbacks and resource allocation counters. Require
  <=one explicit wait per async frame, fewer than the synchronous control's
  per-operation waits; no wall-time speedup or overlap-on-silicon requirement.
- Bound per-slot operations and mutable banks. After both slots are warm, no new
  texture, staging or bank allocation, pass compilation or queue growth is allowed
  until the next extent. Report high-water counts/bytes and zero live objects after
  teardown. Two slots intentionally trade memory for fewer host waits.
- Exercise polling, premature slot reuse, capacity/unsupported operation rejection,
  destruction with queued operations, and partial-frame failure cleanup. Keep the
  existing specialization, rejection, live-child and synchronous consumer tests.
- Validate on llvmpipe and physical RADV with synchronization validation. Existing
  runtime gates remain regression evidence; no API stability or remote-CI claim.

Stop with a retain/revise decision: whether the explicit consumer-managed model
is sufficient, or the integration exposes a better API alternative. Do not turn
successful execution into automatic approval for a scheduler/allocator framework,
general libplacebo support, new formats or further streaming optimization.

## Implementation and acceptance — 2026-09-14

Complete. Contract committed at `1e257c9` before asynchronous results; implementation
`bcbe72b`, lifetime hardening `c6fd4f3`. Public runtime remains ABI 10, with no runtime
code or public header change. New begin/end/collect helpers are consumer glue only.
The [runner](../integrations/libplacebo/run-consumer.sh) now executes the original
nine-frame acceptance plus the 36-frame synchronous and two-slot modes.

All intermediate/final comparisons are exact on llvmpipe and physical RX 5700 XT /
RADV: **1,145,952 bytes, zero differences**, per 36-frame mode per driver. The
original nine-frame control also retains its exact 286,488-byte comparison. Both
streaming modes execute 36 compute passes, 36 raster passes, 39 uploads (including
three LUTs), 72 downloads and 72 callbacks. No tolerance changed.

| Final acceptance | Submissions | Explicit waits | Polls | Peak uncollected frames |
|---|---:|---:|---:|---:|
| llvmpipe synchronous | 183 | 183 | 0 | 0 |
| llvmpipe two-slot | 183 | 0 | 219 | 2 |
| RADV synchronous | 183 | 183 | 0 | 0 |
| RADV two-slot | 183 | 1 | 221 | 2 |

Wait counts depend on when the GPU finishes; an earlier RADV run made six waits.
The gate is <=one explicit wait per frame and **zero waits while recording normal
frame operations**, not a fixed number or a speedup. Polls are zero-timeout queries;
driver calls still have overhead. No timing comparison or physical GPU overlap is
claimed. Two frames are demonstrably submitted before either is collected; completed
but uncollected receipts still reserve their banks until explicitly observed.

Both modes intentionally use the same two texture sets and two banks per pass,
isolating wait policy rather than minimizing the synchronous control's memory.
Allocations stop after both slots warm up. Peak resources: seven textures (six
frame images plus one LUT), four mutable pass banks (eight-slot image/sampler heaps,
with 64 vertex bytes per raster bank), 156,208 texture payload bytes and 156,208
staging payload bytes. There are 21 staging allocations across the three extents;
the two-slot peak is eleven operation receipts, bounded by two fixed arrays of
eight. Last completed pass receipts can also survive separately, at most one per
bank. These are adapter ownership/payload counters, not total resident GPU memory;
driver allocation padding, caches and transient pipeline compilation are unmeasured.
All live texture/pass/bank/staging/frame ownership counters return to zero.

Both drivers pass eight frame-transfer checks: premature slot reuse, operation
capacity, partial transfer, repeated staging, destruction while queued, forced
pending poll, transient poll error and rejected submission. Three pass checks cover
synchronous specialization A/B/A, queued specialization replacement with two slots,
and premature bank reuse. The failed active-bank test delivers its callback only
after draining and leaves poisoned output invalid. Child destruction also drains
partially open frames. A pending poll preserves callbacks and ownership; forced
blocking collection makes exactly one wait. Test-only link wrappers never fabricate
GPU success or retirement and are absent from normal consumer executables.
The existing three rejection tests and live-child abort test also pass.

Ordinary Rust tests (27), Clippy, formatting, shell syntax and 745 C/Rust ABI layout
checks pass; all 20 runtime GPU tests pass on both drivers. Validation and
synchronization validation reported no errors. This is local acceptance, not a
remote-CI or arbitrary device-loss recovery claim.

Local artifacts under ignored `target/libplacebo-integration/`:

| Driver | Reference | Sync | Two-slot |
|---|---|---|---|
| llvmpipe | `reference.L8UYcRCG` | `stream-sync.pnh6ri3T` | `stream-two.YP2yw07l` |
| RADV | `reference.TNJs572g` | `stream-sync.XTekEZOT` | `stream-two.Y5PhhTbj` |

Full logs: `inflight-final-lvp.log`, `inflight-final-radv.log`; backend checks:
`backend-checks.Qq5JnGnm.log`, `backend-checks.fV8zcogn.log`. Runtime logs are
`target/inflight-{lvp,radv}-tests.log`. Reproduction uses the documented runner
and pinned sources, not retained local artifacts.

## Decision and remaining friction

**Retain consumer-managed bounded scheduling.** The workload removes per-operation
host waits without changing the runtime's memory, submission or ownership model.
The consumer knows frame identity, output-buffer lifetime, reuse policy and the
acceptable number of outstanding frames; those are not facts the runtime should
guess. Prepared executables can remain shared while mutable banks are duplicated.
The same public completion and heap-exclusion rules suffice for ordinary and
queued specialization changes.

The comparison does expose costs. Waiting for a frame's last timeline receipt
does not retire earlier receipts: collection explicitly observes every operation.
This adds bookkeeping and polls. A future single-batch-per-frame adapter could
reduce those costs with the existing API, but that was deliberately excluded
from this comparison. A runtime-owned collector might also hide the bookkeeping,
but would introduce collection triggers, shutdown/error policy and ownership
machinery without a demonstrated benefit for this bounded workload. Do not add it
on this evidence alone.

Exclusive heap mutation means distinct outstanding uses need distinct banks. That
is explicit memory expenditure, not a discovered need for concurrent heap writes.
The fixed two-slot/eight-operation policy, conservative drain-on-child-destruction,
full-image transfers and non-reentrant callbacks remain adapter restrictions. A
general backend would need broader scheduling/lifetime decisions; this checkpoint
does not establish it. No better runtime API alternative emerged strongly enough
to select a change; that conclusion remains revisable with different consumers.

The selected experiment is finished. Submission aggregation, a general scheduler,
additional formats, optional numeric profiles and portability are separate future
scope decisions, not automatic follow-ups.
