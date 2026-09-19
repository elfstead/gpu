# Learned-image native Vulkan control

Selected next in M1, 2026-09-19. Implement only after the corrected
[OGPU measurement baseline](learned-image-measurement.md) passes. This is a
benchmark-only control for this application, not a second runtime or a new public
backend. No additional GPU or Mac is needed.

## Question and fixed conditions

How much serialized frame latency does the current OGPU execution path add to
direct Vulkan for the same workload? The answer is a whole-path comparison, not
an isolated per-call API tax or peak-throughput claim.

Use the same four extents, frozen weights/inputs, exact generated SPIR-V, generated
root layouts, workgroup sizes and checked X/Y dispatch grids. Preserve the three
compute stages, fullscreen raster, guarded DEVICE buffers, RGBA8 target, and
resident/end-to-end transfer policies. Clamp display coordinates in both controls.
No kernel fusion, arithmetic changes, different image formats or output shortcuts.

Use the same physical device and graphics/compute queue family, current modern
Vulkan features, compute/graphics pipelines, address-based command inputs and
root-data path. Do not introduce descriptor-set or legacy-copy alternatives to
make the control easier. Use the repository's pinned Vulkan headers where needed.
The control calls Vulkan directly and must not link OGPU or call its private
runtime helpers. Sharing fixture, generated shader and host validation code is
appropriate; sharing submission implementation would defeat the comparison.

## Match execution before timing

Match buffer/image memory-type selection and record the actual selected indices,
flags, requested sizes and native allocation sizes for both controls. DEVICE
buffers and image backing must not silently move into host-visible memory.
Preserve host mapping/coherency behavior, image initialization/layout policy,
barrier stage/access scopes, indirect draw arguments and timestamp placement.
Compare a command-policy inventory against the current OGPU lowering, including
implicit beginning/end host dependencies and image transitions.

One frame remains in flight. Re-record each frame, including fresh per-submission
command/query pool creation and terminal retirement, matching the current OGPU
policy. Use a timeline completion signal and explicit host wait; do not replace
it with queue/device idle. Pipeline construction and input/weight setup remain
outside frame timing. A later reusable-pool or multi-frame control must be labeled
separately, not silently substituted for this matched baseline.

OGPU records operations before lowering them into Vulkan commands at submission.
Direct Vulkan recording does not have that same split. Keep the existing clock
boundaries visible, but interpret recording plus submission together as well as
total frame latency. Do not interpret either component alone as wrapper overhead.

## Acceptance and report

1. Add host tests for native size/grid/root/accounting helpers and failure cleanup.
   Build with warnings as errors. Reject unsupported features/device limits with
   explicit diagnostics; a rejection is not a passing sample.
2. With synchronization validation, run A/B/A normally and diagnostically. Compare
   all scalar intermediates and final pixels against the frozen CPU oracle, or
   byte-for-byte against fully checked OGPU outputs. Check guards, unchanged
   inputs/weights, A/B distinction and repeated A. Include the formerly crashing
   small odd-edge case on llvmpipe; run the four selected large groups on Radeon.
3. Run both controls afresh in the same session with validation/tracing disabled:
   10 warmups, 30 measured frames, three fresh processes per extent/mode/control.
   Rotate control and mode order. Record final B equality outside timing. Preserve
   raw samples, per-run distributions, setup costs, artifact hashes, device/driver
   identity and environment. Earlier OGPU numbers are context, not paired samples.
4. Trace allocation/free activity separately in both controls, verify final pixels
   and zero remaining tracked allocations, and compare native peaks/counts. Retain
   complete traces; successful Vulkan allocations are not physical VRAM residency
   or driver-private memory accounting.
5. Publish absolute latencies and ratios with across-run ranges. Explain unmatched
   implementation conditions rather than attributing every difference to the API.
   If an unexpected gap appears, record it before selecting a bounded diagnostic.

M1 ends with an explicit retain/revise decision. There is no required speedup.
Evidence may expose a better API alternative, a runtime implementation issue, or
neither. Resource reuse belongs to M3 unless the matched comparison first exposes
a correctness defect that needs repair. Do not start an unbounded tuning campaign.

## Implementation status — accepted foundation slice

`examples/learned_image/native.c` now provides the standalone device, dedicated
buffer and address-copy/submission path. It uses the pinned Vulkan headers and
dynamic loader, with no OGPU runtime linkage. Its Vulkan feature selection,
first graphics/compute queue, memory-type scoring (including tie-breaking),
buffer usages, dedicated address allocations, whole-allocation cache maintenance,
host dependencies and timeline completion follow the current OGPU policies.
The benchmark deliberately requires exactly one device from the selected ICD.
It reports device/driver identity and each buffer's requested/native size, memory
type and property flags. No fallback backend is introduced.

The host tests inject allocation/bind/map/address, command preparation, submission
and wait failures. Cleanup retains pending ownership through transient wait/drain
errors and never waits on an unaccepted timeline value. A small native A/B/A
upload -> DEVICE -> readback test passes exact byte checks with synchronization
validation on Radeon and llvmpipe, including independent allocation/free tracing.
This is setup/transfer evidence only, not model execution or a matched measurement.
Clean-commit acceptance is recorded at `de85a07`; see the
[foundation receipt](results/learned-image-native-foundation-2026-09-19.txt).

## Workload slice and command-policy inventory

The native control now implements `--validate resident|end-to-end` using the same
generated headers/SPIR-V as `app.c`. It produces all 21 normal/diagnostic A/B/A
output files, checks guards and unchanged inputs/weights, and snapshots normal
resident outputs in separate submissions. The runner checks every scalar/pixel
against the frozen CPU oracle once per extent, then exact byte equality across
native/OGPU, modes and small-case interface variants. It does not accept sampled
comparisons. A fresh OGPU execution is used, not old measurement output files.

All correctness processes use synchronization validation and the allocation
diagnostic. Native buffer/image descriptions must agree with the independent
allocation trace; native/OGPU size/type sequences and peaks must match for each
mode. These validation readbacks include all intermediates and are intentionally
larger than the eventual timing-mode final-only readback. Traced native allocation
bytes still do not cover driver-internal allocations or physical residency.

Inventory against the current lowering (`compute.rs`, `graphics.rs`, `batch.rs`):

| Part | Matched native workload policy |
|---|---|
| Executables | Same six SPIR-V modules and roots; four compute pipelines including diagnostic poison; descriptor-heap pipeline flag, null layout, empty specialization, `main` entry points |
| Raster state | Triangle list; no vertex bindings, culling or blending; one RGBA8 attachment, single sample, dynamic viewport/scissor; same fullscreen indirect draw arguments |
| Image allocation | COLOR_ATTACHMENT + TRANSFER_SRC, optimal tiling, dedicated backing; same eligibility/scoring/last-tie preference for device-only local memory; queried format limits |
| Buffers | Same sizes, usages, guards, dedicated address allocation and HOST/DEVICE type policy; host mapping/whole-allocation cache maintenance unchanged |
| Setup | Weights copy and scratch guard poisoning once; resident A/B uploads in separate completed submissions; readback guard initialization once |
| Batch boundaries | Fresh command pool; HOST_WRITE -> ALL_COMMANDS/MEMORY_READ+WRITE at start; ALL_COMMANDS/MEMORY_WRITE -> HOST_READ at end; timeline submit and terminal wait |
| Frame reuse | Same compute/fragment/transfer dependency, optional guarded input copy, transfer-write -> compute-read dependency |
| Compute | Diagnostic poison plus write/write dependency when selected; hidden -> denoise -> process with write/read dependencies, then compute-write -> fragment-read |
| Draw | Same ALL_COMMANDS read/write image discard UNDEFINED -> GENERAL, CLEAR/STORE rendering, push data and address-based indirect draw |
| Copies | Same ALL_COMMANDS/MEMORY_WRITE -> TRANSFER/READ+WRITE image-readback dependency, GENERAL image-to-address copy; full guarded diagnostic copies after compute/transfer -> transfer-read dependency |
| Lifetime | One submission in flight; wait before reusing staging or retiring command pool; drain pending work before shader-reachable resources on errors |

The original correctness slice below did not yet match OGPU's optional timestamp
queries. The timing implementation now creates two-query pools before command
pools, resets/writes TOP_OF_PIPE before the initial host barrier, writes BOTTOM_OF_PIPE
after the final host barrier, and retrieves 64-bit results without a query wait
after successful timeline completion. Terminal wait retires the command pool;
query retrieval retires its query pool. Native clock eligibility (36–64 valid bits,
finite positive period) and wrapping conversion match OGPU. Unsupported clocks
produce null device samples, never a fabricated zero-duration measurement.

Native `--measure` uses 10 warmups and 30 measured frames, with the same final-only
readback allocation, host clocks and final B output policy as OGPU. Direct native
recording and OGPU's deferred lowering have different recording/submission clock
boundaries, as declared above; compare their sum as well as full frame latency.
Only setup identity/clock logging was added to `app.c`; runtime/shaders are unchanged.

## Accepted workload correctness — 2026-09-19

At clean commit `219254e`, all 32 traced validation processes pass (192 frames):
the small odd-edge case on both drivers, plus four large Radeon groups. Every
scalar intermediate and final pixel passes the frozen reference gates; native
and fresh OGPU outputs are byte-identical across modes and the small-case original/
mutated interfaces. No shader, model, runtime, public API or tolerance change.

Native allocation descriptions agree with tracing, and native/OGPU allocation
size/type sequences match. All 304 allocations are freed. The 4K validation peak
is 780170032 bytes end-to-end and 813347760 resident; these include full diagnostic
readbacks and must not replace the smaller ordinary-timing baseline allocations.

See the [receipt](results/learned-image-native-workload-2026-09-19.txt),
[Radeon evidence](results/learned-image-native-workload-radv-2026-09-19.json) and
[llvmpipe evidence](results/learned-image-native-workload-llvmpipe-2026-09-19.json).
The exports retain output hashes, full allocation traces and artifact provenance;
local output hashes and trace consistency were rechecked before export.

At this checkpoint correctness and memory-policy parity were accepted; timing
remained pending. The following implementation collects the fresh paired result.

## Paired measurement implementation

`compare_native.py` builds both controls, regenerates/checks shader interfaces and
references, then reruns full traced validation for both controls at the selected
small/large extents. It matches device IDs/API and clock eligibility/period/bits
across every validation, timing and allocation process. The native log also keeps
driver identity and queue family; the single-device ICD gate avoids ambiguous
device selection. Identical allocation types/sizes establish the memory control.

Timing uses 48 fresh processes: four extents, two modes, two controls, three rounds.
Mode order reverses each round; engine order alternates with round+extent+mode
index, producing 12 pairs in each first-engine order. Validation and allocation
tracing are disabled only for timing. All 1440 ordinary samples and final-image
hashes are retained, followed by 16 independent traced 40-frame runs. Traced timings
are discarded; successful allocations must match and be freed in both controls.

`export_comparison.py` rechecks local logs, complete matrices, samples/statistics,
validated final-image hashes and full allocation traces before exporting a new
directory with samples.csv, report.json and checked validation.json. It refuses
incomplete matrices or an existing output directory. This implementation is not
accepted performance evidence until a completed run and decision are recorded.
