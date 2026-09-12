# Optional batch timing experiment

## Scope and API contract

Add a narrow optional way to measure the existing matrix workload on the device.
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
last boundary barrier. After fence completion, retrieve 64-bit results without
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
