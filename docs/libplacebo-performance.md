# libplacebo performance comparison

Selected 2026-09-14. Next bounded decision: does the measured cost come from
consumer submission policy, or expose a better runtime API alternative? Compare
pinned libplacebo's native Vulkan backend, OGPU per-operation submissions, and
OGPU one-batch-per-frame submission using the existing public API (ABI 10).
This compares implementations of one workload, not OGPU versus every Vulkan
program, and not isolated runtime overhead when generated shaders differ.

## Method fixed before timing

Use the same upstream EWA Lanczos compute and nearest raster processing, RGBA8,
two source/intermediate/output texture sets and at most two uncollected frames.
Native Vulkan uses a single queue policy (async compute/transfer disabled).
No external shader cache is loaded. Compile/allocate/first-use costs belong to
setup/warmup, not steady-state throughput. Inputs are pre-generated outside timing.

Three input/output extents: 64x33 -> 129x67 (fixed-overhead diagnostic),
960x540 -> 1921x1081, and 1920x1080 -> 3841x2161. The latter are deliberately
near-1080p/4K outputs retaining the original odd-scale/tail behavior.

Two modes: resident images (upload before timing; no timed readbacks) and transfers
(one upload and two callback readbacks per frame, preserving the acceptance loop).
Resident collection still observes completion before texture/bank reuse. Separate
fresh processes per backend/mode/extent/run; eight warmup frames, 64 measured frames,
three runs with rotated backend order. Report individual results and median/range;
do not select the fastest run. Software execution is correctness evidence, not a
performance comparison with hardware. GPU clocks/load are not controlled: record
device/driver/toolchain/environment and call results local observations.

Before timing, compare all three backends' intermediate/final outputs in memory
on both drivers, at every extent and both data-flow modes. Preserve RGB <=2/255,
alpha exactly 255, exact intermediate/final nearest copying and repeated-input
checks. No per-frame logging, disk writes or output checking inside timed loops.
Validate batched failure cleanup and the existing integration before accepting it.
Run timing with validation disabled only after validated correctness passes.

Metrics: device/setup wall time; warmup wall time; completed-frame throughput;
mean/p50/p95 host-observed frame latency (begin recording through explicit collection,
not display latency or a GPU timestamp); host recording/submission wall and thread
CPU time; collection wall time; total process CPU usage; OGPU submission/wait/poll
counts; common texture payload and peak process RSS. Do not equate RSS/payload to
resident GPU memory. Native queue-submit count and native allocation padding are
not available through the chosen instrumentation; label them unavailable rather
than estimating them. Shader generation/copies/driver calls are included in host
recording time, not claimed as pure OGPU call overhead.

Both OGPU variants keep the same operations, heaps, barriers and staging policy;
only submission/receipt grouping changes. Correctness and bounded ownership are
gates; no speed target or API change is preselected. Stop with an overhead breakdown,
attribution limits and retain/change recommendation. No allocator, scheduler,
kernel tuning, new formats or new runtime API is authorized by this milestone.

## Status

Benchmark and consumer-side aggregation implemented. All three engines match
exactly on llvmpipe at all three extents and both data-flow modes. Existing
per-operation and new grouped-batch pending/failure/queued-specialization checks
pass. No runtime change. Radeon correctness and controlled timing are next.
