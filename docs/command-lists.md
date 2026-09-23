# Reusable command lists — ABI-15 experiment

The [explicit-storage result](recording-storage-results.md) removed a recording
capacity cliff, but did not expose native executable reuse. This experiment asks
whether an immutable executable is a better API alternative for repeated work.
It is not a stable interface or a claim that every workload should use replay.

The [accepted local result](command-list-results.md) retains the candidate after
native-replay comparison, concurrent-use/lifetime checks and mixed/installed-C
execution. Timing and mutable-command variants remain separate questions.

## Contract

```c
ogpu_batch_create_in(storage, &batch, &error); /* ordinary creation also works */
/* Record commands, copied roots, dependencies and explicit buffer retention. */
ogpu_batch_compile(batch, &list, &error);       /* encode, do not submit */
ogpu_batch_destroy(batch);
ogpu_command_list_submit(list, &done, &error);  /* repeat as needed */
ogpu_completion_wait(done, &error);
ogpu_completion_destroy(done);
ogpu_command_list_destroy(list);
```

Check every result in callers; the installed C consumer has a checked path under
`OGPU_EXAMPLE_REPLAY=1`. ABI 15 adds one opaque type and three functions, without
changing public struct layouts. Match header/library revisions. Vulkan implements
this optional experiment; Metal exports UNSUPPORTED stubs, with no native acceptance
claim and no new fallback. The default one-shot path remains available.

| State / operation | Ownership and effect |
|---|---|
| Compile | Consumes the batch, including preparation failure; invalid required pointers do not consume. No GPU work submitted. |
| Idle executable | Retains fixed commands, copied roots, addresses, launch sizes, recorded objects and explicitly retained buffers. |
| Execute | Returns an independent receipt; several executions of the same list may be pending. |
| Observe one execution | Releases that execution's list reference, not the public list's persistent ownership. |
| Destroy public list | Never waits; unretired executions keep it alive. |
| Release last owner | Reset/destroy native commands before releasing recorded objects; return eligible empty capacity. |

Explicit recording storage stays reserved for the entire executable lifetime,
even between executions. Consumed batches and retired receipts do not pin it.
Heap mutation is rejected while any list or unretired execution retains that heap.
This includes earlier bindings superseded within the recording.

Pointed-to bytes are not copied or made immutable. They may change under explicit
host-visibility and GPU-dependency rules. CPU copy helpers require whole-buffer
completion; ABI 16's [borrowed HOST views](host-view-candidate.md) permit independent
range/atom access after conflicting uses complete. Raw addresses confer no
ownership: keep every reachable allocation valid for every possible execution, or
declare whole-buffer retention before compilation. Replay does not add range
tracking, automatic barriers, graph scheduling, patching or parallel host calls.

The Vulkan mapping uses `SIMULTANEOUS_USE`: the same native command buffer may be
submitted while pending, but the caller still supplies race-free dependencies.
The native flag expressly permits this usage. [Vulkan command-buffer usage flags](https://docs.vulkan.org/refpages/latest/refpages/source/VkCommandBufferUsageFlagBits.html)
Descriptor heap reserved ranges must remain valid until binding command buffers
are reset/freed, hence ownership between executions rather than only while pending.
[Vulkan descriptor heaps](https://docs.vulkan.org/spec/latest/chapters/descriptorheaps.html)

Known unaccepted OOM leaves a list retryable. An indeterminate submit error drains
the queue and poisons the list; future submissions reject it. Device loss is sticky.
Transient polling errors retain ownership. Final-use cleanup reset loss reaches
that receipt, as with one-shot retirement; non-loss reset failure destroys capacity.

Timed batches reject compilation with UNSUPPORTED **without consumption** and can
still be submitted normally. Replay receipts are untimed; elapsed-time queries
reject them. Reusing a timestamp query would overwrite an older execution's result.
Per-execution query ownership is explicitly unresolved, not silently emulated.

## Alternatives and limits

Keeping batches permanently one-shot cannot express executable reuse. Mutable
resettable recordings would combine storage, encoding and executable lifetime;
this separate immutable object keeps those contracts distinct. It is a candidate,
not evidence that the extra handle is uniquely best. Native implementations need
not keep Rust step/root vectors: the current backend does, so their memory and
initial encoding cost are implementation costs, not inherent API requirements.

This fixed-command shape does not answer changing addresses/launch dimensions,
per-execution GPU timing, multi-queue execution or host-parallel recording. Those
need discriminating experiments before accepting the fundamental surface. Do not
claim universal Vulkan parity from an amortized fixed-command case.

## Predeclared acceptance protocol

Runtime counters must show one encoding across repeated execution, no reset between
uses, and reset/destruction before resource release. Exact integer/guard tests vary
pointed-to data, preserve old receipts, destroy public parents early and inject
preparation/submit/reset/poll failures. A real semaphore gate must keep three uses
of one list pending, allow nonblocking list destruction, and retain its resources
until every receipt retires (including out-of-order observation). Heap tests must
reject edits between uses, then allow edits after final release.

The separate `run.py --replay` schema-3 matrix is one/five slots × one/64/512
dependent dispatches × native reset/native replay/OGPU owned/OGPU compiled.
It has 24 correctness configurations, 72 timing processes (three fresh rounds),
and 24 separate timing-mode allocation controls. Each validates 1,000 frames or
times 1,000 following 100 drained warmups. Rotation is not perfectly position-
balanced with three rounds/four strategies. Preserve all samples and tails.
Software controls validate 64 frames per configuration, without timing claims.

Both replay paths encode during setup. Native replay uses its existing serial-use
command buffer; OGPU permits simultaneous uses, a stronger capability not exercised
by this per-slot schedule. Requested buffers, shader, exact oracle, dependencies,
slot ownership and clock boundaries remain matched. Trace explicit device-memory
allocations separately; this does not measure native command storage or CPU vectors.
Setup includes device/program creation, so it does not isolate compile amortization.

Also run the frozen mixed learned-image workload through compiled lists: one/two/
three slots, alternating inputs, 64 frames each, full final pixels/readback guards,
input/weight integrity and intermediate guards. This is a small correctness check,
not intermediate numerical reacceptance or a graphics performance claim.

Commands (use one selected ICD and synchronization validation):

```sh
cargo xtask gpu-tests
python3 examples/performance_frontier/run.py --replay
python3 examples/performance_frontier/run.py --replay --software
python3 examples/performance_frontier/stream.py --software --compiled
python3 tools/test-install.py --prefix /new/sdk --recording-storage --replay
```

The `--software` switch selects the small correctness protocol, even when used on
Radeon. Export with `export.py` / `export_stream.py`; exporters check complete fixed
matrices, retained logs, source hashes, traces and samples. Accept the candidate
only after the lifetime gates pass and comparing against native replay, retaining
regressions and remaining limitations rather than declaring the entire audit done.
