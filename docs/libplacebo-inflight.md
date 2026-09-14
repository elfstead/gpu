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

## Status

Adapter and pinned callback contracts audited. Implementation and paired-driver
acceptance are next. Public runtime remains ABI 10.
