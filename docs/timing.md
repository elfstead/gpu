# Optional batch timing experiment

## Scope and API contract

The implementation adds a narrow optional way to measure the matrix workload on the device.
One timed batch yields one approximate elapsed duration. No arbitrary markers,
per-stage profiling, calibrated host/device clocks, cross-queue comparisons,
query-pool handles, or mandatory timestamp support.

- `ogpu_device_timing_info(device, out_info, error)` reports the selected queue's
  timestamp period in nanoseconds and valid counter bits. Unsupported timing does
  not prevent device creation or ordinary execution. The new 16-byte info struct
  contains `double timestamp_period_ns`, `uint32_t timestamp_valid_bits`, and a
  zero `uint32_t reserved`; existing public structures remain unchanged.
- `ogpu_batch_enable_timing(batch, error)` opts a recording batch in, including
  an empty batch. Repeated calls are idempotent. Unsupported/failed calls leave
  recording unchanged; after a submission attempt the batch remains terminal.
- `ogpu_completion_elapsed_ns(completion, out_nanoseconds, error)` returns a double
  only after an explicit successful completion wait. It does not poll or wait.
  Untimed or not-yet-waited completions return INVALID_ARGUMENT. A failed wait
  prevents timing retrieval. A result-query failure is reported separately and
  does not overwrite the recorded wait outcome; retrieval can be retried unless
  the device is lost. Successful retrieval is cached. Output is zero on failure
  when a valid output object is supplied; NULL outputs are invalid.

All existing error-pointer, external serialization, and resource-lifetime rules
apply. A timed completion owns its query resources through draining destruction.
Discarding a recording allocates no Vulkan resources. Untimed submissions make no
query allocation/reset/write/read calls. The C ABI remains experimental.

## Backend measurement and limits

Use two Vulkan timestamp queries per timed submission: command-buffer reset,
TOP_OF_PIPE before the existing batch boundary barrier, BOTTOM_OF_PIPE after the
last boundary barrier. After timeline completion, retrieve 64-bit results without
WAIT_BIT. This conventional bracket is approximate: stage latching can happen
later, prior queued work may overlap, and barriers/contention/scheduling affect
the interval. It measures the instrumented batch, not isolated shader arithmetic.
Existing memory dependencies remain required; timestamps do not replace them.

Compute `(end - start) mod 2^valid_bits`, then multiply by the period in double
precision. Zero duration is valid at finite timer resolution. The caller must
ensure the interval is shorter than one full counter wrap (`2^bits * period`);
additional wraps cannot be detected from two timestamps. A single crossing of the
counter boundary is handled. Large durations can lose low-bit precision when
converted to double. This API deliberately exposes period and bit width so those
limits are not hidden. Invalid/zero period or unsupported counter width declines
timing support, not ordinary execution.

## Verification and workload use

Check capability/width/period handling, 36-/64-bit wrap arithmetic, zero duration,
NULL arguments, pre-wait and untimed reads, cached reads, independent completions,
query creation/read errors, and cleanup on preparation/submission/wait failures.
Run real timed compute and mixed-command regressions with Vulkan synchronization
validation; do not use timing thresholds as correctness assertions.

Extend the matrix runner with timed and untimed warmed samples. Keep host
execution latency, device batch duration, query retrieval, and host copies distinct.
Fall back explicitly to host-only results when timing is unsupported. Validate
results in both modes and record validation-disabled timing separately. Differences
between host and device intervals are not an exact decomposition of CPU overhead.

The Vulkan choices follow the primary [timestamp example](https://docs.vulkan.org/samples/latest/samples/api/timestamp_queries/README.html),
[timestamp command semantics](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdWriteTimestamp.html),
[queue counter properties](https://docs.vulkan.org/refpages/latest/refpages/source/VkQueueFamilyProperties.html),
and [query retrieval rules](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetQueryPoolResults.html).

## Implementation and evidence — 2026-09-12

Implementation checkpoint: `d82684c`. The [public header](../include/ogpu.h) adds
three functions and one new info structure; no previous layout or signature changes.
The Rust [batch implementation](../crates/ogpu/src/batch.rs) owns the two-query pool
inside each timed completion. Five core Vulkan commands were added to the pinned
binding allowlist; no extensions, optional device features, or new dependencies
are enabled. The generated bindings reproduce exactly, and C/Rust checks cover
550 layout values, including the new structure, query pool, and timestamp period.

Ordinary tests pass (20), and all seven Vulkan-backed tests pass locally with
synchronization validation. The [timing tests](../crates/ogpu/src/timing_tests.rs)
exercise malformed clock reports, narrow/full-width wraps, zero elapsed time,
optional-support fallback, untouched untimed/discarded recordings, pre-wait reads,
two outstanding query pools, cached retrieval, pending destruction without a read,
creation OOM, retryable query failures, and simulated loss only after real work
completes. Existing preparation/submission/wait injections also run with timing
enabled. Mixed compute/draw/copy batches retain their timing resources after public
owners are dropped. Actual loss and hardware with a narrow counter remain untested;
simulations and arithmetic tests do not replace that evidence.

The matrix C runner passes all 50 cases per kernel in **both** timed and untimed
modes on both local devices, plus every warmup and measured output. Unsupported
devices explicitly retain host-only measurement. The current local devices both
support timing: RX 5700 XT has 64 valid bits at 10 ns/tick, and llvmpipe has 64 bits
at 1 ns/tick. Counter period is a conversion scale, not an accuracy guarantee.
Clippy, ABI/mock checks, binding reproduction, and all previous C experiments pass.
Existing CI commands include the new tests and updated matrix runner; no remote
CI execution is claimed.

## Initial measurements

A validation-disabled run of the committed implementation used Rust 1.97.1
release, Clang 21.1.8 `-O2 -DNDEBUG`, the unchanged SPIR-V matrix shaders, Vulkan
loader 1.4.357, RADV/llvmpipe Vulkan 1.4.354, and a Ryzen 9 5900X host. Validation
environment variables were unset and `VK_LOADER_LAYERS_DISABLE` named
`VK_LAYER_KHRONOS_validation`. CPU/GPU clocks were not fixed; the machine was not
isolated. These are preliminary profiling observations, not controlled benchmarks.

Each kernel/mode gets two warmups and nine samples, alternating kernel and timing
mode order. All intervals below are median milliseconds for M=N=K=256:

| Device | Kernel | Untimed host | Timed host | Device batch | Query read |
|---|---|---|---|---|---|
| RX 5700 XT | Baseline | 0.5211 | 0.5504 | 0.3655 | 0.0032 |
| RX 5700 XT | Tiled 8×8 | 0.3155 | 0.3408 | 0.1618 | 0.0031 |
| llvmpipe | Baseline | 2.3140 | 2.4716 | 2.3797 | 0.0131 |
| llvmpipe | Tiled 8×8 | 2.8913 | 3.0514 | 2.9673 | 0.0136 |

RADV device-batch min/max were 0.3634–0.3693 ms baseline and 0.1600–0.1634 ms
tiled. llvmpipe's were 1.9768–2.8443 and 2.4247–3.4026 ms; its variability is
material. For 128³, baseline/tiled device medians were 0.1402/0.0456 ms on RADV
and 0.3694/0.4610 ms on llvmpipe. For (257,193,129), they were 0.1945/0.0942 ms
and 0.9727/1.3227 ms respectively. The runner prints full min/median/max for all
host/device intervals and separately reports allocation, upload, reset, and readback.

A second validation-disabled process run retained the same ordering for all three
shapes. Its 256³ device medians were 0.3597/0.1617 ms on RADV and 2.5302/3.1091 ms
on llvmpipe (baseline/tiled). This repeat does not eliminate scheduling or clock noise.

Timed host intervals include query-pool creation, reset/write recording, submission,
wait, and destruction, but exclude result retrieval. Device intervals cover the
instrumented command batch and its barriers. Query-read cost is charged separately;
it does not disappear. Untimed and timed samples are different executions, and
their clock domains are not calibrated: subtracting medians is not an exact CPU
overhead measurement. A device-time sample need not be below an unrelated untimed
host sample. Software Vulkan timing is not physical GPU timing.

The RADV tiled improvement is visible in the device bracket as well as host
latency. Tiling still has higher median latency on llvmpipe for these cases.
Timing instrumentation has measurable cost, reinforcing the opt-in design and the
need to keep untimed controls. Whole-batch measurements do not identify individual
barrier, dispatch, image-copy, or allocation bottlenecks.

Larger shape sweeps and multiple dispatches per submission could test scaling and
amortization before resource-reuse optimization or finer markers. These follow-ups
are now parked under the [integration milestone plan](plan.md), not automatically
scheduled API work. Accelerated variants, direct image access and per-region
profiling remain separate decisions; this experiment does not stabilize the API.

Run `cargo xtask matmul` for the comparison, and `cargo xtask gpu-tests` for backend
state/failure checks. See [matrix reproduction](matmul.md#reproduction) for separate
validation-enabled and disabled commands, and [development](development.md) for
loader/toolchain setup. Timing tests intentionally impose no speed threshold.
