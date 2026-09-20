# Explicit recording storage — ABI-14 experiment

This follows the [bounded-cache result](command-storage-results.md). The question
is whether caller-owned reuse and release provide a better fundamental shape than
hidden cache admission thresholds, without entangling recordings or completion
receipts. This is a candidate implementation and comparison, not a stable surface.

## Selected shape and alternatives

`OgpuRecordingStorage` is a device-owned-by-reference, caller-controlled owner of
empty native recording capacity. It serves **one** recording/submission at a time.
It is neither executable commands nor a Vulkan pool exposed through the ABI.

```c
ogpu_recording_storage_create(device, &storage, &error);
ogpu_batch_create_in(storage, &batch, &error);
/* Record commands, then submit once. */
ogpu_batch_submit(batch, &done, &error);
ogpu_completion_wait(done, &error);
/* Old batch/receipt may survive; another one-shot recording can now use storage. */
ogpu_recording_storage_trim(storage, &error); /* optional: return capacity now */
ogpu_recording_storage_destroy(storage);
```

Check every result in real callers. The [installed C example](../examples/compiler/consumer.c)
has a checked optional path, enabled by `OGPU_EXAMPLE_RECORDING_STORAGE=1`.

Why a separate owner first:

- It separates storage lifetime from the single-use recording and durable receipt.
  A consumed batch never becomes recordable again; no generation-sensitive handle
  or meaning-changing reset operation is needed for this experiment.
- A resettable storage-owning batch would avoid an extra owner handle but couple
  reusable capacity and transient commands. That remains a possible ergonomic
  alternative; these tests do not prove a separate object is uniquely best.
- Device caching remains available through `ogpu_batch_create`, for comparison and
  convenience. Explicit owners never acquire or return the device's cached pools.
- Executable replay would retain commands/resources, not just empty capacity.
  It requires its own immutable/mutable argument and lifetime contract; not added here.

## State and lifetime rules

| Owner state | Create batch in owner | Trim | Destroy public owner |
|---|---|---|---|
| Idle, initially no allocation | Reserve successfully | Release idle capacity, or no-op | Free idle capacity |
| Reserved by unsubmitted recording | Reject | Reject | Recording keeps owner alive until discard/retirement |
| Submitted, not terminally observed | Reject, even if GPU finished | Reject | Submission keeps owner alive until retirement |
| Terminally observed or failed attempt cleaned up | Available again | Release idle capacity | Free idle capacity; old batch/receipt do not pin it |

Creation outputs are null on failure. Invalid pointers do not consume a recording.
Discarding a recording releases its reservation. Submit attempts transfer the
reservation to preparation/submission resources; failures release it after safe
cleanup, even if the consumed batch survives. Pending/transient-error polls keep
the reservation. Observing another receipt does not implicitly release this one.

Successful retirement resets native references **before** releasing heap, buffer
or executable references, then returns empty capacity and releases the reservation.
Failed preparation/submission, drained wait errors and loss destroy active capacity;
a preparation failure before taking idle capacity may leave that empty capacity
in the owner. Non-loss reset errors destroy the pool and retain successful execution
status; reset loss is sticky. Trim is destruction-only, never waits or establishes
completion, and is allowed when idle even after loss. It does not touch other owners.

Destroying the public owner never waits. A live recording/submission holds an
internal lease; its eventual discard/retirement releases the last owner if needed.
There is no cycle back from the device. All calls retain the existing device-wide
external-serialization rule. This does not add parallel host recording.

Native storage is allocated lazily and has no 256-step/64-KiB admission cutoff.
It retains high-water capacity until trim/destruction or a failure fallback.
This offers lifetime and owner-count control, **not** an exact native-byte budget,
preallocation guarantee, allocation-free submission guarantee or reusable CPU
`Step`/root-vector storage. A driver may still allocate/grow internal memory.

## Backend and version boundary

ABI 14 adds four C functions and an opaque handle, without changing public struct
layouts. Existing version-mismatch rejection applies; use matching SDK/header/library.
Vulkan implements this experiment. Metal exports explicit UNSUPPORTED stubs with
cleared outputs; no native storage reuse, new fallback or Mac validation is claimed.
The existing default Metal batch path is unchanged. Successful owner creation is
the current optional-feature check; do not assume availability from ABI alone.

## Acceptance and comparison protocol

Safety tests must cover idle/discard/reuse, trim while reserved, pending/transient
error, unobserved completion, owner release before submit and before retirement,
surviving batch/timing receipts, preparation/submit/reset failure and device loss.
Real heap replacement must work through reused storage; native references must be
invalidated before edits/frees. Explicit owners must never populate the device cache.
Heterogeneous 1/256/257/4096-step recordings must reuse one pool across 50 iterations;
trim must destroy it while old batches/receipts live and the next use allocate again.
Pool counters prove object reuse/release, not native-byte consumption.

The separate frontier `--storage` matrix uses the same integer shader, exact oracle,
guards, scheduling, native fresh/reset/replay paths and clock boundaries. It adds
an `owned` public-API strategy beside the current `ogpu` device-cache strategy.
One/five slots × 64/129/512 dependent dispatches × five strategies: 30 validation
configurations, 90 timing processes (three rounds), and 30 independent timing-mode
allocation controls. Each process validates 1,000 frames or times 1,000 after 100
drained warmups. Software controls use 64 validated frames per configuration, no timing.
129 dispatches produce 258 recorded steps, deliberately crossing cache admission.
Order rotates across configurations/rounds; five strategies and three rounds do
not yield perfectly balanced positions. Preserve all intervals/tails/raw samples.

Only if ownership/release tests pass and the larger-workload gap closes without a
material new small-workload cost should explicit storage remain a preferred
candidate. Equal total native-byte budgets, caller preallocation, CPU recording
cost and executable replay remain unresolved. Do not infer universal parity.

Reproduce with `python3 examples/performance_frontier/run.py --storage` on the
selected validated Radeon environment, `--storage --software` on llvmpipe, and
the existing `export.py` for either fixed matrix schema. `--check` requires no GPU.
The SDK check adds `python3 tools/test-install.py --prefix /new/sdk --recording-storage`.
