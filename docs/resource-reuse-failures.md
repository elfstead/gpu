# M3 failure/drain checkpoint

Accepted 2026-09-27 at clean `e771e66`, following the
[mapped-stream success checkpoint](resource-reuse-results.md). This closes the
selected injected-failure and consumer-report portion of M3, **not M3 as a whole**.
No runtime, public ABI 17, shader or numerical-policy change.

## Result

The actual mapped learned-image workload now has a diagnostic executable that
tests failure responses through the public C API. A separate, test-only Vulkan
loader injects one explicitly armed error after warmup. It retains allocation and
native-drain traces; it does not add fault controls to the runtime or affect the
ordinary streaming/timing executable.

All 48 configurations pass on each of Radeon RX 5700 XT/RADV and llvmpipe:
six modes × dedicated/arena allocation × owned reset/serial replay × two/three
slots. Each warms up for 12 fully checked frames, then arranges unretired work in
the other slots before exercising the selected slot.

| Injected condition | Verified behavior |
|---|---|
| Submit OOM without calling the real driver | No completion; consumed one-shot batch discarded; known-unsubmitted generation aborted; a new generation submits successfully. The existing compiled list remains retryable. Other receipts are not inferred retired. |
| Poll timeout | SUCCESS with incomplete output; receipt and range remain pending; attempted range reacquisition rejects. |
| Transient poll error | Error with incomplete output; same retention/reuse rejection, followed by successful ordinary retirement. |
| Wait error before real completion | Runtime retries to establish drain before returning the original error; repeated wait/poll preserve that error. Quarantine permits teardown but never reuse or acceptance of the failed execution's output. |
| Synthetic device loss | Shim first successfully drains the real queue, then reports loss. Every outstanding receipt retires as an error; every slot remains quarantined. |
| Real submit accepted, followed by an indeterminate error | Runtime drains the queue despite returning no completion. The consumer quarantines the slot and explicitly retires the other receipts; no retry or output-validity claim. |

The three recoverable modes validate their outstanding outputs and another 64
frames against both byte-exact within-driver serial images and the unchanged CPU
RGB ≤1/exact-alpha oracle. They then check input/weight integrity, intermediate
guards, HOST padding and idle slot state. Across both drivers that includes 3,072
post-recovery frames, in addition to warmup, outstanding-frame checks and serial
references. Terminal modes validate cleanup, not numerical output after the error.

All 96 scenarios have zero live tracked allocations/bytes after teardown, with
unchanged dedicated counts 17/25 or arena counts 6/7 for two/three slots. No Vulkan
validation errors. The shim rejects a wait on the known-unsubmitted timeline value
and aborts on backing-memory frees before all real work drains. Its CPU tests
deliberately exercise those abort guards, plus refusal to synthesize loss after a
failed real drain. ASan/UBSan and Clang analysis pass; LeakSanitizer remains disabled
because of environment tracing.

## Reports and limitations

The [Radeon report](results/resource-reuse-failures-radv-2026-09-27.json) and
[llvmpipe report](results/resource-reuse-failures-lvp-2026-09-27.json) retain the
clean implementation revision, source/executable/runtime/shim hashes, device and
enabled requirements, named resources/executables, range ownership, and raw
allocation/drain records. `REUSE_EVENT` adds frame, slot, generation, operation,
result/error text, reuse state and receipt/list retention. Error text is JSON
escaped. Unexpected process errors or timeouts leave incomplete reports and logs;
they are not accepted as expected injection behavior.

The complete raw local process logs are hash-identified in
`target/resource-reuse/failures-r5a9171i` and `failures-oyy4nd6m`. Both reports were
reparsed against their raw logs and source hashes after completion. Seven parser
tests reject bad context/ranges, missing drain/quarantine, premature synthetic
loss, missing injection, leaks and retry without a new generation. The initial
dirty-tree Radeon run is only a development control; acceptance uses the final
clean revision on both drivers.

These are **synthetic faults with real work and real draining**, not hardware
device-loss or driver-failure reliability evidence. Artificial pending/error polls
prove the ownership response, not that the GPU was physically still executing.
The shim assumes one externally serialized device, queue and timeline; it is not a
general Vulkan layer. Its allocation accounting excludes driver-private command
memory and CPU allocations. No noncoherent-hardware, Metal, concurrency, setup-OOM,
cancellation, bounded-retry or performance claim follows. Persistent real wait
failure can still block indefinitely under the existing API contract.

Diagnostics are currently scoped to this consumer's submission/observation paths,
not a runtime-wide label API or a general capture/debugger. A quarantine transition
does not itself prove drain: the public operation's documented outcome and the
test's native trace supply that evidence. Parent allocations and immutable lists
retain their separate lifetime obligations after per-execution retirement.

## Reproduce and regressions

```sh
python3 examples/resource_reuse/failures.py --check
python3 examples/resource_reuse/failures.py
python3 examples/resource_reuse/run.py --preflight
```

Select one ICD, set `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` and
`VK_LAYER_VALIDATE_SYNC=1`, and leave `OGPU_TRACE_LOADER` unset; the runner installs
its own shim over the selected loader. See the
[example instructions](../examples/resource_reuse/README.md#failuredrain-integration).

The ordinary mapped stream's 12 configurations plus serial reference pass on each
driver after the failure suite. Existing CPU helper/parser gates, forty ordinary
Rust tests, strict Clippy and formatting pass. The unchanged stream C/shaders and
runtime do not need another 48,000-frame scale run or full M1/runtime GPU/ABI suite
for this diagnostic-only addition; those historical results are not claimed as
new runs. Exact commands and hashes are in the
[validation receipt](results/resource-reuse-failures-validation-2026-09-27.txt).

## Next

The subsequent [matched allocation/submission decision](resource-reuse-performance.md)
is now accepted at `bb76447`, without changing the runtime/API. It retains the
earlier host-sensitive evidence and exposes residual re-recording costs rather
than approving the fundamental API from GPU-heavy parity. Independent relocated-SDK
handoff remains the final M3 gate; broader performance-audit items and the post-M3
roadmap remain separate.
