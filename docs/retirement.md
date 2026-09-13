# Scratch retirement and allocation retention

D4 experiment, 2026-09-13. The question is whether the shared graphics/compute
model benefits from application-managed retirement, explicit runtime retention,
or a combined range-use declaration. These mechanisms provide different guarantees.
Implementation: `26133a5`; preceding loader and timeline corrections: `3c23fef`
and `e82cfff`.

## Decision

Adopt two independent, optional operations:

- `ogpu_completion_poll(completion, &complete, error)` observes completion without
  waiting for GPU progress. Allocators can reclaim a range after SUCCESS with `1`
  on every outstanding use of that range. The runtime cannot discover those uses
  from arbitrary shader pointers.
- `ogpu_batch_retain_buffer(batch, buffer, error)` keeps an entire backing allocation
  alive through batch discard, failed-submission cleanup, or completion-handle
  destruction. Duplicate declarations are harmless. It adds no access declaration,
  dependency, range lock, or recursive pointer tracing.

No mandatory range-use list or runtime allocator is selected. The example's
allocator knows its scratch slots; asking the runtime to track the same intervals
would duplicate that bookkeeping without improving this workload. Optional whole
allocation retention lets a submitting owner release its handle before execution
finishes. Neither successful execution nor compatibility with the old API was a
veto: this exposes a better API alternative by making progress observable and
ownership assistance independent of synchronization.

At this checkpoint, the existing public C structures and signatures stayed ABI 1.
The additional symbols need matching headers/library from that source revision;
ABI 1 alone does not promise that older libraries export them. Subsequent
[image/heap changes](descriptor-heaps.md) bring the current runtime to ABI 3 without
changing these retirement operations. A future central retirement
queue could be an internal change; this experiment does not establish a need for it.

## Comparison and workload

| Choice | Prevents allocation destruction | Decides when a range can be reused | Evidence |
|---|---|---|---|
| Application ownership + completion polling | Application holds buffer | Application tracks each outstanding use | C and gated tests pass |
| Optional runtime buffer retention + polling | Batch/completion holds buffer | Same application bookkeeping | Same numerical results; early owner release is safe |
| Mandatory range-use/access declarations | Could retain allocations and track declared ranges | Requires complete declarations and overlap tracking | Not implemented; no extra guarantee needed by this bounded workload |

`cargo xtask retirement` runs [the C example](../examples/retirement.c) in both
implemented modes. Twelve independent jobs use three scratch slots; each runs the
existing produce/consume shaders on 65 elements. Nine reuses are approved by
successful completion polling while other completion handles remain live. Every
job has its own output region and checked guard words. In retained mode the
scratch buffer handle is destroyed after recording/submitting its final uses and
before their waits. Both modes verify all 780 outputs and 24 output guard words.

Live completion handles alone do not prove unfinished GPU work. The separate
`gpu_retirement` test injects a timeline wait into submission 2 and holds it behind
a host-controlled gate. Submission 1 completes, its scratch range is reclaimed and
submitted for reuse, and polling submission 2 still reports pending. Opening the
gate lets the remaining jobs finish; results and a neighboring scratch guard are
checked. Weak ownership references verify retention, duplicate declarations,
discard cleanup and final release. The gate opens before draining during test
cleanup, including unwinding. Its host signal command is test-only and is never
loaded by the production execution backend.

This is a lifetime/correctness test. It does not establish parallel execution,
throughput gains, CPU/GPU overlap, or allocator performance. No new shader or
consumer dependency is needed.

## Contracts that matter

Polling performs a zero-timeout Vulkan semaphore wait. Pending returns SUCCESS
with `0`. A transient error returns an error with `0` and retains pending ownership;
it does not block to drain, cache a terminal failure, or authorize reuse. A later
poll or wait can succeed. Device loss is terminal and permits cleanup, but never
reports successful completion. Repeated observations preserve a recorded wait
failure. SUCCESS with `1` supplies the same visibility as waiting and permits
timestamp retrieval. Resources remain retained until completion destruction.

Whole-allocation retention does not prevent premature suballocation reuse. All
pointer-reachable allocations must be retained explicitly or owned by the caller.
Dependencies remain explicit even when an allocation is retained. For this
host-visible memory implementation, CPU read/write still requires completion of
ALL uses of the entire allocation: cache maintenance can touch the whole mapping.
The experiment therefore reuses shared ranges only from the GPU and defers CPU
copies until all submissions finish.

The timeline review also enforces the Vulkan
[pending-value limit](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceTimelineSemaphoreProperties.html),
burns failed-attempt values, and rejects 64-bit exhaustion without wrapping.
Polling semantics follow [vkWaitSemaphores](https://docs.vulkan.org/refpages/latest/refpages/source/vkWaitSemaphores.html).

## Verification

Local verification on 2026-09-13 passed:

- Formatting, Clippy with warnings denied, generated-bindings check, and loader mocks.
- 21 ordinary tests, all 9 execution tests with Vulkan/synchronization validation,
  and 633 C/Rust ABI checks.
- Both C retirement modes, the image-loop regression, and timed/untimed matmul cases.
- GGML lifecycle checks and all six direct/scheduled MNIST acceptance cases
  (batch sizes 1, 17, and 64; 10,000 images each).

## Stopping condition

Met locally on Mesa 26.2.1 llvmpipe with Vulkan and synchronization validation:
scratch ranges are reused while other work remains pending, ownership is safe in
both modes, and failures do not authorize early reuse. This completes the D4
comparison. The subsequent [direct image access/sampling experiment](heap-images.md)
passes locally with descriptor heaps. A subsequent [hardware run](hardware-validation.md)
passes both retirement modes and the gated Vulkan tests on RX 5700 XT / RADV.
Provisioned remote execution CI remains pending.
