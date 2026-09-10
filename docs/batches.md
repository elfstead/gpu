# One-shot asynchronous batches

This is the next execution experiment, not a stable API. It separates recording,
submission, and completion while keeping one externally serialized queue per
device and the existing host-visible allocations. Graphics and additional queues
remain follow-up experiments; the dependency vocabulary below currently covers
only compute accesses.

## Recording and submission

`ogpu_batch_create` creates an empty recording. `ogpu_batch_dispatch` copies inline
argument bytes and retains the kernel. The kernel must belong to the batch's
device. The existing one-dimensional dispatch and shader contracts apply.

`ogpu_batch_barrier` records a global memory/execution dependency between earlier
and later compute commands on the same queue. Source and destination masks are
nonzero combinations of `OGPU_ACCESS_COMPUTE_READ` and
`OGPU_ACCESS_COMPUTE_WRITE`. For a producer followed by a consumer, use WRITE →
READ. These are our access values, not Vulkan flags. Dependencies are not inferred
from pointers or inserted between dispatches automatically. A barrier at the start
of a batch can cover earlier submissions on this device's queue.

Invalid recording arguments leave the recording unchanged. Destroying an
unsubmitted batch discards it without GPU execution. An empty batch is legal.

`ogpu_batch_submit` attempts submission exactly once and returns an
`OgpuCompletion` on success. The batch then remains a valid handle only for
destruction; recording or submitting again returns INVALID_ARGUMENT. A submission
attempt also makes the batch terminal if command preparation or submission fails.
Invalid required pointers are rejected before an attempt and do not consume it.
Creation outputs, including the completion output, are NULL on failure.

Submission success means accepted, not completed. It does not intentionally wait
for GPU completion, though allocation, command preparation, and driver submission
can take time. The completion owns the submitted command resources and retained
kernels, independently of the original batch, kernel, device, and probe handles.
GPU execution failures can surface during waiting rather than submission.

## Dependencies and host access

Each batch establishes host-write → compute-read/write visibility at its start
and compute-write → host-read visibility at its end. These boundary dependencies
do not replace explicit dependencies between GPU operations. Submission order
alone is not a memory dependency between dispatches or batches.

The caller keeps every allocation reachable through recorded GPU addresses alive
from recording until completion (or until the unsubmitted batch is discarded).
Recorded addresses are non-owning; retaining kernels cannot retain pointees.
Do not read, write, or destroy a buffer while submitted work can access it. This
restriction covers the entire allocation, even disjoint byte ranges, because the
initial transfer implementation performs whole-allocation cache maintenance.
All API calls involving a device and its children remain externally serialized;
GPU execution may continue between these calls. Unrelated allocations may still
be used by the CPU while a submission runs.

## Completion and errors

`ogpu_completion_wait` blocks without a timeout until that submission completes or
the device is lost. It waits on a per-submission fence, not queue idle. Repeated
waits are allowed and preserve the recorded wait outcome. A non-loss wait error
does not establish completion: resources stay owned while waiting is retried,
and the first error is returned only after draining or device loss. Timeout
responses are retried, not treated as completed work. Persistent failures can
block indefinitely, as in the existing synchronous helper.

`ogpu_completion_destroy` waits if necessary before freeing anything; it does not
cancel submitted work. It has no error return, so explicitly wait first to obtain
diagnostics. Destroy completions before referenced buffers during cleanup. After
device loss, destruction and draining existing completions are allowed; recording,
new submission, and buffer transfers are not.

On command preparation failure nothing was submitted. Vulkan submission memory
errors leave submitted resources unaffected; device loss permits cleanup. For an
unexpected submission error, conservatively drain the queue before releasing
command resources. Failure of this attempt does not establish completion of other
outstanding submissions; their completion handles still govern their lifetimes.
The backend follows the [Vulkan submission failure guarantees](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueueSubmit.html)
and [fence wait semantics](https://docs.vulkan.org/refpages/latest/refpages/source/vkWaitForFences.html).

`ogpu_dispatch_wait` remains an ordered convenience operation built on a batch,
an initial compute-read/write → compute-read/write dependency, and a completion
wait. This preserves its prior synchronous visibility contract. It does not
authorize host access to buffers still used by later submissions.

## Evidence required

- Two different kernels, an intermediate allocation, an explicit WRITE → READ
  dependency, one submission, and one final CPU wait/readback.
- Copied argument bytes and kernel/device retention after user handles are freed.
- Rejected wrong-device kernels, invalid masks, invalid dispatches, and batch reuse.
- Empty/discarded batches, repeated waits, and destruction without an explicit wait.
- Wait-error draining and device-loss behavior, including partial construction.
- Existing synchronous examples and ABI/binding reproducibility remain passing.

Not included: completion polling/timeouts, reusable recordings, multiple queues,
GPU transfer commands, device-local staging, graphics commands, or tensor semantics.
