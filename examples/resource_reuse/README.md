# Consumer-side resource reuse

Consumer-side implementation of the [M3 brief](../../docs/resource-reuse-plan.md).
No runtime changes or allocator performance claim. The mapped-stream control below
now connects these helpers to the existing workload; see the M3 status for accepted
run coverage rather than treating a successful build as GPU evidence.

```sh
mkdir -p target/resource-reuse
cc -std=c11 -O2 -DNDEBUG -Wall -Wextra -Werror \
  examples/resource_reuse/test_reuse.c -o target/resource-reuse/test-reuse
target/resource-reuse/test-reuse
```

`ranges.h` reserves guarded, aligned payloads inside a caller-provided capacity.
It uses the least common multiple of alignment and cache granularity, with checked
arithmetic; alignment need not be a power of two. Whole spans own complete cache
atoms. No allocation, individual free, compaction, growth or implicit cache work.
Query the **actual** HOST view before choosing ranges. Check backing base alignment
separately and initialize/check all padding and gaps in the GPU consumer.

`slots.h` tracks one mutable range's execution/CPU-consumption cycle. Each acquire
issues a monotonically increasing, slot-local ticket; generation wrap rejects.
Successful submission moves recorded → pending; confirmed completion moves pending
→ ready; CPU consumption precedes release. Known unsubmitted work may abort.
Unknown submit outcomes and terminal failures quarantine the slot; explicit proven
drain allows teardown but never reuse. Incomplete polls and transient errors leave
it pending. Calls assert facts supplied by the caller; they cannot establish that a
real GPU completed, stop direct pointer writes, or discover transitive ownership.

Tickets must stay associated with their slot; they are not globally unique handles.
Externally serialize all helper access. Keep actual memory, list, recording and
completion owners elsewhere. A compiled list may retain backing after an execution
is drained, so the helper's drained predicate alone never authorizes freeing it.

CPU tests cover 23,808 mixed-alignment ranges, boundaries/overflow and unchanged
outputs on rejection, 1,000 reuse generations, stale tickets, pending/ready reuse
rejection, known-rejected submission, unknown-outcome/terminal quarantine, explicit
drain and generation exhaustion. This is bookkeeping validation, **not** injected
runtime/GPU failure evidence. The separate [mapped-stream checkpoint](../../docs/resource-reuse-results.md)
accepts sustained success-path integration and allocation accounting at `6ef0328`;
neither follows from CPU helper tests alone.

## Mapped-stream integration

```sh
python3 examples/resource_reuse/run.py --check
# Select VK_DRIVER_FILES and enable Vulkan/sync validation first:
python3 examples/resource_reuse/run.py --preflight
python3 examples/resource_reuse/run.py         # 1,000 small frames/configuration
python3 examples/resource_reuse/run.py --scale # also 720p and odd-video cases
python3 examples/resource_reuse/run.py --native # add matched Vulkan reset/replay
```

The opt-in `FRONTIER_REUSE` build of the existing stream compares dedicated buffers
with three parent buffers: DEVICE data, HOST upload and HOST readback. Every mode
uses the same mapped-host copying/cache policy and unchanged shader artifacts.
Each slot has caller-owned recording storage; `ogpu` resets/re-records and `compiled`
replays a serial immutable list. Raw-pointer backing remains live until all lists
and executions retire. The old public/native controls remain separately built.

A one-slot reference first passes the independent CPU pixel oracle and saves two
guarded images. All later frames must pass BOTH that CPU gate and byte-exact serial
comparison. Input/weights, intermediate guards and HOST inter-range/trailing padding
are checked after drain. The trace verifies allocation/free/peak counts, not private
driver command-memory size. Runtime shaders, arithmetic and tolerances are unchanged.
Inputs alternate on each reuse of each slot; merely alternating global frame input
would leave two-slot streams permanently assigned A/B and miss stale uploads.

To keep requested bytes equal between strategies, HOST capacity budgets round each
logical range to 4 KiB in both modes. This is a declared test budget, not a claim
that a cache atom is 4 KiB: actual views determine placement and flush/invalidation.
Insufficient capacity rejects before recording; no hidden fallback, growing ring or
per-frame backing allocation. DEVICE slices need only scalar alignment here and
have no inter-range gaps. Real noncoherent hardware is not inferred from coherent
driver success; CPU range tests exercise larger/odd granularity independently.

The runner records revision/dirty state, hashes, individual process logs, logical
range ownership, requested bytes and traced peak bytes under a fresh
`target/resource-reuse/stream-*` directory. These runs keep validation/tracing on,
so their elapsed times are diagnostic, not performance evidence. The separate
failure integration below supplies bounded injection evidence; matched native
timing is now covered by the separate comparison below. Independent installed
handoff remains the M3 gate. There is no implied GPU race detection from the helper.

At `c4009c0`, optimized checks, AddressSanitizer/UBSan and Clang analysis pass.
LeakSanitizer cannot run under the test environment's tracing; the sanitizer run
uses `ASAN_OPTIONS=detect_leaks=0`. This is not leak-detection evidence.

## Failure/drain integration

The [bounded checkpoint](../../docs/resource-reuse-failures.md) is accepted at
`e771e66`: all 48 configurations pass on each available Vulkan driver. It does not
complete M3; the measurement/decision follows separately below, then independent handoff.

```sh
python3 examples/resource_reuse/failures.py --check # build and CPU checks
python3 examples/resource_reuse/failures.py         # selected driver, Vulkan/sync validation
```

`failures.c` uses the same mapped stream, shaders, ranges, slot states and teardown
through the public C API. It is a separate diagnostic executable, not a new runtime
fault-injection API or timing path. Each process warms up for 12 checked frames,
then arms one fault explicitly while the other slots have unretired submissions.
The matrix covers dedicated/arena × owned reset/serial replay × two/three slots.

| Fault | Expected consumer action |
|---|---|
| Submit OOM before native acceptance | Discard consumed one-shot recording, abort the known-unsubmitted generation, acquire a new one; a compiled list remains retryable |
| Poll timeout or temporary error | Keep receipt and range pending; reject reuse, then wait normally |
| Wait error before real completion | Let the runtime drain before returning the sticky error; quarantine, never accept the output or recycle |
| Synthetic device loss | Test shim first drains the real queue; consumer then retires all receipts as errors and quarantines all slots |
| Submit accepted by driver, then reported as indeterminate | Runtime must drain the queue despite returning no completion; quarantine, never recycle |

Recoverable cases check the outstanding frames and another 64 frames against both
the serial images and CPU oracle, then check input/weight integrity, guards and
HOST padding. Terminal cases check safe teardown, not valid output. Other receipts
are explicitly observed even when a queue drain has physically completed their work.

`failure_shim.c` chains the existing allocation tracer and real Vulkan loader. It
never forwards the rejected OOM submission, never waits its unsignaled value, and
aborts if backing memory is freed before all real work has drained. Synthetic loss
is emitted only after a successful real queue-idle call; this is **not actual
hardware-loss evidence**. Artificial timeout/error observations establish the
consumer's ownership response, not that the physical GPU was still executing.
The shim supports exactly this serialized, single-device/queue/timeline workload.
Do not use it as a general loader, allocator, or production recovery mechanism.

`REUSE_EVENT` records frame/slot/generation, operation, result, state and retained
receipt/list ownership. The JSON report joins those events to device/enabled
requirements, named resources/executables, ranges, source/binary/runtime hashes,
fixtures, allocation and native-drain traces. Process failures and timeouts leave
an incomplete report and captured logs for diagnosis. A missing gate or failed
process is never accepted merely because the injection was intentional.

## Matched native performance control

The [decision and evidence](../../docs/resource-reuse-performance.md) select two
arena-backed slots with serial compiled lists for the fixed-command handoff.
Allocation remains consumer-side; no general native-parity or byte-saving claim.

```sh
python3 examples/resource_reuse/performance.py --check
python3 examples/resource_reuse/performance.py --software # selected llvmpipe, correctness only
python3 examples/resource_reuse/performance.py            # selected Radeon, full matrix
python3 examples/resource_reuse/export_performance.py /run/report.json /new/evidence-directory
python3 examples/resource_reuse/test_export_performance.py /original/hardware-run/report.json
```

The direct Vulkan range build links no OGPU, sharing only consumer policy/oracles
and retaining independent command encoding. It uses actual native cache atoms and
range-scoped flush/invalidate, not legacy whole-buffer host helpers on shared ranges.
Allocation sizes/types and layouts must agree with the public path. The runner
separates every-frame correctness, uninstrumented timing and traced lifecycle
accounting; see the [protocol](../../docs/resource-reuse-performance-plan.md).
All samples are retained; export checks the complete matrix, process order, hashes,
statistics and native/public policy matching before publishing evidence.
