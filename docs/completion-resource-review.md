# Completion receipts and submission resources

D4 review, 2026-09-14, against `d2462e3` (ABI 8). **Recommendation, not implemented.**
The runtime and header still retain submission resources until completion destruction.
This follows the [capability cleanup](execution-capabilities.md) and revisits the
completion lifetime, not the [allocation/range-retirement decision](retirement.md).

## Recommendation

Keep one public `OgpuCompletion`, but separate its execution result/diagnostics from
the resources required by the submitted work. On a wait/poll that establishes safe
cleanup, destroy the native command pool and release that submission's retained
objects before returning. Keep the completion handle usable for repeated status
observations and optional timing retrieval. Pending destruction still drains.

This is caller-driven retirement at an observation point, not background collection,
queue-wide sweeping, cancellation or automatic suballocation reuse. No second public
handle or new release function is recommended for the first experiment. The better
API alternative is semantic: keeping a completed result should not by itself keep
already-used GPU allocations alive or prevent heap mutation.

It would change explicit lifetime guarantees, so the implementation experiment must
bump to **ABI 9**, even without changing signatures. This document does not do so.

## What the code establishes today

[`Completion`](../crates/ogpu/src/batch.rs) combines three responsibilities:

- A timeline value, pending state and sticky wait outcome.
- A native command pool, recorded `Step` objects (including copied roots and owning
  references to kernels, raster programs, images, heaps and explicit copy operands),
  plus explicitly retained whole buffers.
- Optional timing: a two-query pool and a cached duration after successful retrieval.

Wait/poll update status but release none of these resources. Destruction waits if
needed, destroys the native command pool and query pool, then Rust drops the owning
references. Thus the current cleanup order is safe; the issue is its coupling to
the lifetime of a result handle. Root-only GPU addresses are never discovered or
retained by this machinery.

The consumers show different consequences:

| Caller | Current behavior | What a separate receipt would improve |
|---|---|---|
| [libplacebo](../integrations/libplacebo/backend.c), `finish` and `pass_run` | Every operation waits and destroys its completion; only then can the pass clear/rewrite heaps | Could retain status/timing while reusing pass heaps; current synchronous adapter already cleans up promptly, so no demonstrated throughput gain |
| [GGML](../integrations/ggml/backend.cpp), `copy_bytes` and `graph_compute` | Local RAII completion waits and dies at scope exit; tensors/staging have independent owners | Cleaner semantics if results are kept later; no current long-lived receipt problem, and no change to the synchronous graph contract |
| [Retirement test](../crates/ogpu/src/retirement_tests.rs) | Uses weak references to prove assisted allocation ownership persists after waits until completions die | Exposes the behavior that must deliberately change; it currently even reads through a weak upgrade backed by those completed handles |
| [Heap test](../crates/ogpu/src/heap_tests.rs) | A gated completion finishes, but both heaps remain non-exclusive until its destruction | Direct regression target: retain the receipt and edit heaps after safe cleanup, without weakening checks on other recordings/submissions |

There is no measured leak or existing-consumer speedup here. The motivation is a
more coherent ownership contract for diagnostics and reuse, not a fabricated
performance requirement or a claim that either consumer is blocked.

## Alternatives

| Model | Benefit | Cost / limitation | Assessment |
|---|---|---|---|
| Retain everything until handle destruction (today) | One owner; observation does not run resource destruction | Keeping a result pins resources and blocks heap edits | Safe baseline, but conflates independent lifetimes |
| Release submission resources on terminal observation | One handle; no extra cleanup protocol; retained results stop pinning work resources | Wait/poll can perform significant host cleanup; unobserved completions still retain resources | Recommended first experiment |
| Explicit `completion_release_resources` after observation | Caller chooses exactly when destruction cost occurs; status queries stay observational | Additional state/call and a forgotten-release failure mode; completion no longer alone describes whether heaps remain retained | Strong alternative if cleanup placement matters to an actual caller; not rejected on compatibility grounds |
| Device-owned pending list plus lightweight receipts | Can reclaim finished work even when its receipt is never observed; can support non-draining receipt destruction | New collection trigger, ownership/shutdown/error rules and per-submission tracking; timing must survive independently | Defer; not justified by these two synchronous consumers |

A separate public submission-owner handle and receipt can also give explicit
cleanup placement, but adds two ownership lifecycles to each submission. An internal
split tests the semantic benefit without committing callers to that object model.

The explicit-release alternative is real, not merely a compatibility option. Our
recommendation instead makes the normal observation/reuse path complete in one
operation. Poll already updates cached state, but destruction is materially more
work: document that it does not wait for GPU progress, **not** that it is constant
time or cheap. Driver destruction and dropping retained objects can dominate its
host cost. Neither model has a measured latency advantage yet.

## Required cleanup and timing rules

The Vulkan heap reservation is a substantive constraint: application access to
driver-reserved heap storage remains prohibited while bound command buffers exist,
even after their execution finishes. Those command buffers must be freed or reset.
This is narrower than a universal ban on editing any descriptor after completion.
See the [Vulkan descriptor-heap specification](https://docs.vulkan.org/spec/latest/chapters/descriptorheaps.html).

OGPU's current heap implementation uses exclusive ownership and whole-allocation
cache maintenance, including the reservation. Therefore the proposed sequence is:

1. Establish completion/draining or terminal device loss.
2. Destroy that submission's native command pool, freeing its command buffers.
3. Drop its recorded steps, heap references and explicit buffer retention.
4. Keep the result handle; other live users can still prevent heap mutation.

Do not drop references first or replace the exclusivity check with a check of one
completion. A heap can be retained by another recording or submission, including
earlier bindings superseded within a batch. Image entries still own their images
until cleared/replaced/destroyed. These are independent lifetime constraints.

Timing must not hold this sequence hostage. Keep its optional query pool separate
from submission resources: after successful execution, it can remain owned by the
receipt for lazy retrieval and retry. Cache a successful read, then the pool can be
destroyed; receipt destruction discards unread timing. Do not eagerly read timing
inside wait/poll or turn query-read failure into failed execution. Device-loss and
untimed/unconfirmed-read rules remain as documented in [timing](timing.md).

Consequently, a timed receipt is not necessarily a CPU-only value: it retains its
device and, until read/discarded, a small query pool. No new public timing handle
is needed. Do not promise that every GPU object disappears at execution completion.

## State and error behavior for the experiment

Cleanup safety and execution outcome must remain separate. An error alone does
not establish safety; the existing [draining implementation](../crates/ogpu/src/compute.rs)
distinguishes transient poll failure from a wait error returned after draining.

| Observation / event | Submission resources | Receipt result |
|---|---|---|
| Poll pending, or transient poll error | Retain; no draining | Pending, or retryable query error; no reuse permission |
| Successful wait, or successful poll with complete=1 | Release in the order above | Cache success; subsequent wait/poll keep that result |
| Non-loss wait error, returned only after draining | Release; GPU access is known to have ended | Preserve the recorded error; do not authorize successful output/timing |
| Device loss | Cleanup is permitted; release | Report loss, never successful execution; device stays poisoned |
| Partial preparation or rejected/uncertain submit | Preserve existing partial-ownership and queue-drain rules | No public completion on submit failure; never wait for an unsignaled rejected value |
| Destruction while pending | Drain before releasing | No cancellation; caller must observe explicitly for diagnostics |

Retirement must be exactly once and introduce no new fallible allocation. Repeated
terminal status observations and destruction must not repeat cleanup or timeline
polling. A later device loss must
not rewrite a receipt's already recorded successful wait into a different outcome;
timing still follows the device-loss restrictions. Observing a later submission
does not automatically retire other completion objects: this is deliberately not
a device-wide collector. Observe/destroy each object whose resources should retire.

No public `reclaimed` flag is needed for the recommended model: terminal observation
establishes cleanup as part of its contract. Internally, optional owned submission
resources should express that state rather than scattering nullable native handles
and independently cleared vectors across wait/poll/destruction paths.

## Ownership break to document explicitly

`ogpu_batch_retain_buffer` would guarantee backing through submitted GPU use and
safe retirement, **not** through the entire lifetime of a completed receipt. A caller
needing an allocation after wait/poll must retain its own owning buffer handle.
Keeping an address and an old receipt would no longer keep that address backed.
The assisted retirement test must acquire an independent owner for its post-wait
readback, and separately verify that an allocation with no remaining owner actually
dies while its receipt survives. Do not just erase its existing ownership assertions.

Allocation retention still does not decide scratch-range reuse. Every outstanding
use must have finished; later GPU work still needs explicit dependencies. HOST
buffer reads/writes still require completion of all uses of the whole allocation.
The split changes none of those rules and adds no pointer tracing or range allocator.

## Bounded implementation gate

The next proposed step is an ABI-9 implementation experiment, not asynchronous
consumer scheduling. Its stopping condition is all of the following:

- Keep completed receipts alive while native pools and otherwise-unowned buffers,
  kernels and heap references retire; verify destruction order and exactly-once cleanup.
- Gate pending work; prove pending/transient-error polls retain resources. With a
  heap shared by two submissions or an unsubmitted recording, releasing one use
  must not allow edits until every other retained use is released.
- Preserve sticky wait errors, loss handling, partial construction, uncertain-submit
  draining and pending destruction. Test timed and untimed cases.
- Keep timing readable/retryable after submission resources retire, without query
  reads in wait/poll; query failure must not re-pin work resources or rewrite results.
- Exercise heap reuse with an old receipt alive through the public C API and the
  bounded libplacebo checks; keep both consumers' processing/scheduling unchanged.
- Run both driver suites, C examples, GGML HOST/DEVICE acceptance and paired
  libplacebo pixel comparisons. Update the header/lifetime docs and ABI version
  together; no legacy retention mode or new Vulkan compatibility path.

Reject or revise the candidate if it cannot preserve these safety/diagnostic rules.
Compare explicit release if a caller needs separate cleanup placement; do not add
a collector, pools, range tracking or concurrency merely to make this experiment pass.

## Review verification

Read-only source/test review at ABI 8; the ordinary suite (27 tests) and current
llvmpipe GPU suite (19 tests, Vulkan/synchronization validation) were rerun and pass.
They verify the **existing** lifetime behavior, not the proposed split. The preceding
[capability checkpoint](execution-capabilities.md#verification) records the physical
RADV and paired consumer regressions; they were not rerun for this documentation-only
review. No runtime/header/shader code, upstream source or dependency changes here.
