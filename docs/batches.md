# One-shot asynchronous batches

This is the implemented one-shot submission contract, not a stable API. It separates
recording, submission, and completion while keeping one externally serialized queue
per device. The [offscreen graphics profile](graphics.md) uses this same model for
raster draws, images, and image readback. Additional queues remain future work.
See [the design overview](design.md) and [experiment ledger](experiments.md).

[Optional timing](timing.md) can bracket a whole batch with device timestamps.
Enable it while recording, explicitly wait successfully, then read the duration
from the completion. Untimed batches allocate no query resources. Timing retrieval
does not wait or change a previous wait outcome, and timestamps do not replace
the memory dependencies described below.

Run the [C example](../examples/batch.c) with `cargo xtask batch`. It uploads 4099
integers, dispatches [a producer](../examples/shaders/produce.comp) into an
intermediate allocation, then [a consumer](../examples/shaders/consume.comp) into
a separate output allocation. The consumer reads adjacent intermediate elements,
including across workgroup boundaries. One explicit WRITE → READ barrier connects
the kernels; one submission and one CPU wait precede the final readback. There is
no intermediate host readback or upload.

The application-defined 24-byte root block contains two GPU addresses at bytes 0
and 8, the element count at byte 16, and four initialized padding bytes. Both
kernels use 64 invocations per workgroup and guard excess invocations. All output
elements are compared against a separate CPU calculation. This is correctness
evidence, not a dispatch-overhead or overlap benchmark.

## Recording and submission

`ogpu_batch_create` creates an empty recording. `ogpu_batch_dispatch` copies inline
argument bytes and retains the kernel. The kernel must belong to the batch's
device. The existing one-dimensional dispatch and shader contracts apply.

`ogpu_batch_barrier` records a global memory/execution dependency between earlier
and later commands on the same queue. The initial compute source/destination masks are
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

Each batch establishes host-write → GPU-read/write visibility at its start
and GPU-write → host-read visibility at its end, including graphics and transfers.
These boundary dependencies
do not replace explicit dependencies between GPU operations. Submission order
alone is not a memory dependency between dispatches or batches.

The caller keeps every allocation reachable through recorded GPU addresses alive
from recording until completion (or until the unsubmitted batch is discarded).
Recorded addresses are non-owning; retaining kernels cannot retain pointees.
Do not perform CPU reads/writes while submitted work can access the buffer. Keep
ownership until completion; commands taking explicit buffer handles can retain
them, but addresses alone cannot. This host-access
restriction covers the entire allocation, even disjoint byte ranges, because the
initial transfer implementation performs whole-allocation cache maintenance.
All API calls involving a device and its children remain externally serialized;
GPU execution may continue between these calls. Unrelated allocations may still
be used by the CPU while a submission runs.

## Completion and errors

`ogpu_completion_wait` blocks without a timeout until that submission completes or
the device is lost. It waits on a per-submission timeline value, not queue idle. Repeated
waits are allowed and preserve the recorded wait outcome. A non-loss wait error
does not establish completion: resources stay owned while waiting is retried,
and the first error is returned only after draining or device loss. Timeout
responses are retried, not treated as completed work. Persistent failures can
block indefinitely, as in the existing synchronous helper.

The device enforces `maxTimelineSemaphoreValueDifference` before submission and
rejects exhausted 64-bit values without wrapping. Both cases return OUT_OF_RANGE
and consume the batch attempt. For a pending-value limit, complete outstanding
work and record a new batch; exhaustion needs a new device. Failed submission
attempts burn their reserved values, so a later success can leave gaps.

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
and [timeline semaphore wait semantics](https://docs.vulkan.org/refpages/latest/refpages/source/vkWaitSemaphores.html).

`ogpu_dispatch_wait` remains an ordered convenience operation built on a batch,
an initial compute-read/write → compute-read/write dependency, and a completion
wait. This preserves its prior synchronous visibility contract. It does not
authorize host access to buffers still used by later submissions.

## Validation coverage

- Two different kernels, an intermediate allocation, an explicit WRITE → READ
  dependency, one submission, and one final CPU wait/readback.
- Copied argument bytes and kernel/device retention after user handles are freed.
- Rejected wrong-device kernels, invalid masks, invalid dispatches, and batch reuse.
- Empty/discarded batches, repeated waits, and destruction without an explicit wait.
- Wait-error draining and device-loss behavior, including partial construction.
- Regression checks for synchronous examples and ABI/binding reproducibility.

Not included in this first compute slice: completion polling/timeouts, reusable
recordings, multiple queues, device-local staging, or tensor semantics. The graphics
extension adds a narrow image-to-buffer GPU copy, not a general transfer interface.

## Reproduction

The shader binaries are checked in; normal builds do not invoke a shader compiler.
They were generated with glslang 16.4.0 and validated with SPIRV-Tools 1.4.357.0:

```sh
glslangValidator -V --target-env vulkan1.2 examples/shaders/produce.comp -o examples/shaders/produce.spv
glslangValidator -V --target-env vulkan1.2 examples/shaders/consume.comp -o examples/shaders/consume.spv
spirv-val --target-env vulkan1.2 examples/shaders/produce.spv
spirv-val --target-env vulkan1.2 examples/shaders/consume.spv
cargo xtask batch
cargo xtask gpu-tests
```

`gpu-tests` also exercises discarded/empty batches, multiple outstanding
submissions, cross-submission dependencies, copied arguments, wrong-device
kernels, terminal batch state, parent/kernel retention, and draining destruction.
Failure injection covers timeline/pool/command preparation, rejected/unknown
submissions, transient waits, and simulated device loss. Simulated wait-time loss
is reported only after actual work has drained; it is not a real device-loss test.
Both runners reject reported Vulkan validation errors. See [development](development.md)
for enabling validation and choosing a Vulkan loader.
