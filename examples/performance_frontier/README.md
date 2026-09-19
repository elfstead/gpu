# Performance expressibility controls

These experiments test whether the public contract preserves useful native
execution strategies. They are not a general Vulkan speed ranking. The
[audit and acceptance brief](../../docs/performance-expressibility.md) owns scope.

## Repeated small compute

```sh
python3 examples/performance_frontier/run.py --check
# Select a single ICD; enable VK_LAYER_KHRONOS_validation and VK_LAYER_VALIDATE_SYNC=1.
python3 examples/performance_frontier/run.py --software  # llvmpipe correctness only
python3 examples/performance_frontier/run.py             # Radeon acceptance/timing
python3 examples/performance_frontier/export.py \
  target/performance-frontier/small-IDENTIFIER/report.json /path/to/new-evidence-directory
```

Needs the normal Rust/C/Python/Vulkan environment; no new shader compiler or data
download. Uses the committed, previously validated compiler-generated integer
transform. Source/header/binary hashes identify the actual artifact. The native
executable reuses benchmark-only loader/setup helpers and never links OGPU; the
runner checks its undefined symbols. The runtime ABI/shader are unchanged.

Four strategies: OGPU one-shot, native fresh pool, native pool reset/re-record,
native pre-recorded replay. One/three independent HOST buffers each hold 65 uint32
values plus guards (388 requested bytes). Each submission does 1/64 dependent
in-place transforms. Buffers persist through the whole process; completed slot
reuse is explicit. Replay's fixed addresses and roots are part of this workload,
not a claim that arbitrary mutable commands can be reused.

Radeon: 16 validation processes check every frame for 1,000 frames; timing uses
three processes per configuration, each with 100 drained warmups and 1,000 frames.
Strategy order rotates by round and configuration, so each strategy occupies each
position equally across the matrix. A further 16 traced timing-mode processes
check allocation policy/cleanup, excluding their times from comparisons. Software
controls check 64 frames for each of the 16 configurations, never performance.

Wall time includes fill, slot waits, encoding, submissions and final drain, but
excludes final CPU checks/logging. Validation timings include every-frame checks
and are not benchmark samples. Observed frame latency starts at recording and ends
at the CPU's terminal observation: it is not a calibrated device execution interval
or an external input-arrival SLA. Wait samples include terminal object retirement.
Record+submit must be considered together: OGPU lowers commands at submit. CPU
clocks perturb very small operations; all strategies use the same clock sites.
No per-frame GPU timestamps, queue idle or log/file writes enter successful timing.

Replay setup encoding is included in setup cost, outside warmed throughput. Reset
and replay retain command pools until process cleanup; fresh and OGPU retire pools
per submission. All shader-reachable allocations remain owned until every slot
drains, including on failure. Injected tests cover premature reuse, failed reset,
failed submit without waiting on a rejected signal, transient wait drain, device
loss and pool retirement. Allocation traces measure application Vulkan memory,
not driver-private command-pool storage or physical residency. Retained commands
have a storage cost not captured by that tracer.

Results live under unique `target/performance-frontier/small-*` directories. The
exporter refuses incomplete matrices and existing destinations, verifies retained
logs/statistics/traces and writes JSON plus all 48,000 ordinary CSV samples.
Outputs are not auto-deleted. This small suite needs tens of MB, not M1's large
intermediate dumps. New strategies must be labeled rather than silently replacing
one of these controls. Streaming learned-image is the next distinct experiment.
