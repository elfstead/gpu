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
runner checks its undefined symbols. Shaders and native controls are frozen;
runtime changes must be identified by revision and ABI in each result review.

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
and replay retain command pools until process cleanup; fresh destroys pools per
submission. OGPU's tested implementation is identified by the report's revision:
ABI 12 destroyed each pool; ABI 13 caches bounded reset storage. This is a new
runtime policy, not a change to the frozen native controls. All shader-reachable allocations remain owned until every slot
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
one of these controls. Streaming learned-image uses the distinct control below.

## Streaming learned-image

```sh
python3 examples/performance_frontier/stream.py --check
python3 examples/performance_frontier/stream.py --preflight # 12 small odd frames
python3 examples/performance_frontier/stream.py --software  # 64 small odd frames
python3 examples/performance_frontier/stream.py             # Radeon full matrix
python3 examples/performance_frontier/export_stream.py \
  target/performance-frontier/stream-IDENTIFIER/report.json /path/to/new-evidence-directory
```

Same loader/layer environment. Reuses the checked M1 `reference`/`reference-scale`
fixtures from `SLANGC=/path/to/slangc cargo xtask learned-image --scale --check`.
The runner verifies model, weights, A/B inputs and CPU final-image hashes before
execution. It does not regenerate multi-GB fixtures on each timing run.

Full matrix: 1280x720 -> 2560x1440 and 1919x1079 -> 2561x1441, each at one/two/three
slots and all four strategies. Every slot owns five DEVICE buffers, HOST upload/
readback and an RGBA8 target; immutable draw arguments are shared. Input data
alternates A/B, including for reusable native recordings with stable addresses.
Each frame includes input HOST write, GPU upload/compute/raster/readback, then CPU
read after completion. This is end-to-end streaming, not resident M1 timing.

Validation checks every full final image for 1,000 frames per configuration against
the unchanged CPU gate (RGB delta <=1, alpha exactly 255), plus readback guards.
After the window, a separate diagnostic submission checks every slot's unchanged
input/weights and all intermediate prefix/suffix guards. It uses existing readback
storage and is outside timing; it does not reaccept full intermediate numerics.
Timing checks every slot's final image after the measured window and runs the same
buffer-integrity diagnostic. Do not treat this as new shader/compiler acceptance.

There are 24 validated Radeon processes, 72 timing processes (100 warmups + 1,000
frames, three rounds, rotated strategy order) and 24 independently traced timing-
mode processes. llvmpipe checks only the 12 small odd configurations. Full results
retain 72,000 samples. Allocation count is 1 + 8*slots, independent of frame count;
all allocations must match native/OGPU sizes/types and be freed. Driver-private
pool storage is excluded. More slots increase both storage and observed latency.

No cross-queue control, direct host mapping, narrowed-dependency strategy or heap
mutation is implemented here. Three-slot success establishes expressibility only
for independently allocated slots on the current queue. It does not approve the
rest of the fundamental API. Runtime and ABI remain unchanged.

Accepted results: [small compute](../../docs/performance-frontier-small.md) and
[streaming image](../../docs/performance-frontier-stream.md). The
[next contract review](../../docs/command-storage-review.md) separates reusable
storage from executable replay and defines the remaining retirement tests.
