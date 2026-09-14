# Resident libplacebo performance diagnosis

**Diagnosis and runtime correction complete (`1f41d7e`).** Image memory-type
preference is the dominant cause of the observed near-4K resident gap. Changing
only that choice in a diagnostic loader control raises grouped OGPU from 104 to
563 fps versus a fresh native control at 592 fps. See results and next action below.

Selected 2026-09-15 after the [controlled comparison](libplacebo-performance.md).
The near-4K resident workload ran at 553 fps native versus 99 fps grouped OGPU;
host recording CPU was similar. Determine a reproducible implementation cause,
not an assumed API deficiency. Runtime ABI 10, workload, staging and upstream
revision remain unchanged. No shader mathematics, new formats or API expansion.

First source finding: pinned upstream `src/glsl/spirv_shaderc.c` requests
`shaderc_optimization_level_performance`; the adapter's `compiler.cpp` does not.
Test an explicitly labeled performance-optimized compiler variant against the
unchanged default. Use the same perf harness, native control, 8 warmup / 64
measured frames, three rotated fresh-process runs. Gate timing on all three
extents/modes passing same-driver references with synchronization validation on
RADV and llvmpipe. Preserve the original baseline and all observations.

If this does not explain the gap, add diagnostic-only device timestamps and
generated-shader/dispatch capture to separate compute/raster execution from host
collection. Use existing timing contracts, label instrumentation perturbation,
and compare against untimed controls. Inspect image/barrier paths only as the
evidence warrants; do not change several factors at once.

Stop when the dominant gap has an evidenced cause and a retain/change decision,
or report precisely what attribution remains unresolved. A validated compiler
policy correction is within this implementation investigation; a public API
alternative or larger optimization program needs a separate decision.

## Compiler-policy gate

The isolated `OGPU_DIAGNOSTIC_OPTIMIZE` build fails the first llvmpipe backend
specialization test: validation reports an undefined forward-referenced SPIR-V
ID, and compute pipeline creation fails. No optimized timing is accepted.
Reproducer: build with `build-diagnostics.sh`, then run
`backend-tests-optimized specialization-update` under validation. Full local
failure: `target/libplacebo-integration/diagnosis.xUcPLUQX/backend-checks.log`.
The ordinary compiler passes; this does not establish optimization's performance
impact or justify accepting invalid shaders. Upstream sources remain unchanged.

## Baseline profiling instrumentation

`run-profile.sh` builds a separate `perf-diagnostic` executable. It captures the
upstream GLSL, constants, device limits and dispatch grids before measurement.
A pinned backend-table hook (as in the existing reference capture) substitutes
separate native libplacebo compute/raster timers. Link wrappers opt OGPU batches
into the existing public timing API and retrieve durations after terminal
observation, before receipt destruction. Fixed bookkeeping holds at most 32
receipts; no runtime or normal adapter change is needed.

Run all extent/mode correctness gates on both drivers, then three rotated
near-4K resident runs each for native/per-operation/grouped execution with
diagnostic timing off and on. Require all 64 measured timer samples per relevant
pass/frame. Native already uses an internal timer in ordinary dispatch; "off"
means no extra diagnostic timing, not removal of that upstream behavior.
The wrapper clocks and query retrieval can perturb timing. Whole-batch and native
pass durations include their respective dependencies and need not be additive
or exactly equivalent boundaries. Host poll/wait/destroy/query totals include
measured-phase cleanup; they are not a disjoint CPU/GPU decomposition.

Initial Radeon timestamps localize the baseline gap to device intervals, not
receipt destruction: grouped frames take about 9.5 ms on-device; native passes
about 1.53 ms compute and 0.13 ms raster. OGPU per-operation compute/raster
intervals are each about 4.75 ms. Captured compute GLSL differs only by unused
extension-enabling directives before adapter lowering; constants/grids match.

Next isolation: `OGPU_DIAGNOSTIC_OMIT=compute|raster|both` skips only the selected
public command-recording call during the measured phase in the diagnostic binary.
Warmup still executes/validates the full workload; existing heap binds, global
barriers, submissions and collection remain. Use resident images only; this is
**not workload throughput**, and printed adapter pass counts represent attempted
calls, not omitted GPU commands. Validate each omission mode before timing it.
This distinguishes command work from remaining dependencies without modifying
runtime code, shaders or the original benchmark.

Allocation tracing found every OGPU image selected host-visible device-local
type 3, in the 256 MiB visible heap, rather than non-host-visible type 0/1 in
the larger device heap. The runtime's image allocator prefers DEVICE_LOCAL but
breaks ties in favor of the last eligible memory type. Descriptor heaps/staging
are host-visible system memory. Test a separate loader-shim control that chooses
an eligible non-host-visible DEVICE_LOCAL type **only for dedicated images**,
using each image's queried memoryTypeBits. Leave heaps, buffers and all other
parameters unchanged. Validate the full workload against native before timing;
no runtime selection change is adopted merely from this trace. Visible-heap
pressure/residency remains a hypothesis until the placement control is measured.

## Results — 2026-09-15

Radeon RX 5700 XT, RADV Mesa 26.2.1, same pinned libplacebo/toolchain as the original
comparison. All raw measurements, environment, rejected optimizer error and
allocation trace are [committed here](results/libplacebo-diagnosis-radv-2026-09-15.txt).
The original comparison remains historical evidence; do not compare its 553 fps
native result directly to a later day's variant. The fresh paired control below
captures the current machine conditions, still not isolated or clock-locked.

### Localization

Diagnostic timestamps reproduce about 9.50 ms per grouped baseline frame, versus
native compute 1.53 ms and raster 0.132 ms. OGPU poll/wait/destroy instrumentation
shows collection is overwhelmingly waiting, not receipt-destruction work.
With measured compute omitted, the frame interval is about 4.75 ms; with raster
omitted, about 4.75 ms; with both omitted, about 0.0051 ms. Each reported case has
three runs with 64 samples each. This rules out a large fixed dependency-only
cost; it does not prove synchronization with real work is free.

Compute generation agrees on 32x32 local size, 121x68x1 dispatch, specialization
constants and processing GLSL. The source differences before lowering are unused
extension-enable directives; heap lowering and vertex pulling remain different.
Neither subgroup generation nor different workgroup sizing explains this result.

### Image allocation control

The trace records all seven images in memory type 3, flags 0x7
(DEVICE_LOCAL | HOST_VISIBLE | HOST_COHERENT), heap 2 with 268,435,456 bytes.
Eligible type 0 is DEVICE_LOCAL without HOST_VISIBLE, in heap 0 with
8,321,499,136 bytes. The image requirements mask is 0x7ad, permitting both.
`image_memory_type` prefers DEVICE_LOCAL but ties choose the last eligible type,
unintentionally selecting the small visible heap. Images are **not** allocated
in ordinary system RAM; visibility and heap choice are the relevant distinction.

The forwarding loader changes only dedicated image allocations to eligible
type 0. Buffer/heap allocation, descriptor lowering, shader compiler policy,
GENERAL layouts, barriers, command grouping and completion rules remain unchanged.
All three extents and both data-flow modes match native exactly under synchronization
validation, in both the timestamped and ordinary benchmark harnesses. The special
placement control is Radeon-only: it deliberately aborts on a device without
an eligible non-host-visible local memory type; that is not a proposed runtime rule.

Fresh ordinary-harness near-4K resident results, 8 warmup / 64 measured frames,
three rotated processes per variant, no added diagnostic timestamps:

| Variant | Median fps | Min–max fps | Recording CPU ms/frame |
|---|---:|---:|---:|
| Native Vulkan | 591.60 | 590.87–594.15 | 0.1085 |
| Grouped OGPU, original image choice | 104.04 | 104.02–104.06 | 0.1142 |
| Grouped OGPU, non-host-visible local images | 562.93 | 562.64–564.56 | 0.1072 |

This is a 5.41x throughput change from the image choice alone; the remaining
throughput gap to native is about 4.85%, not established isolated API overhead.
Separate timed placement controls move the grouped device interval from
9.497 ms to 1.672 ms (medians of three run means). Native pass timings sum to
about 1.661 ms, with the earlier boundary/overlap caveats.

This establishes **memory-type selection as the dominant implementation cause**
for this workload on this device. Visible-heap pressure/eviction is a plausible
mechanism, not a measured fact: no residency, migration or memory-budget counters
were collected. No general claim about all host-visible VRAM, UMA or other drivers
follows. The failed optimized-compiler control remains a separate toolchain issue,
not a prerequisite for recovering most of this performance.

## Decision and next implementation step

Stop the diagnosis here. Recommend changing the runtime image allocator to prefer
eligible non-host-visible DEVICE_LOCAL memory, while still accepting host-visible
local memory where that is the suitable available choice (including unified-memory
devices). Preserve requirements masks and existing exclusion rules; do not hardcode
Radeon type indices, add a new public image-placement flag, or change HOST buffer
policy. The public image model already leaves backing-memory selection to the runtime.

At the diagnostic checkpoint, that correction still needed selector unit tests covering separate/visible-only/unified
memory and eligibility masks, both-driver image/consumer regressions, and the
ordinary native performance comparison repeated without the diagnostic loader.
It was not applied by the diagnostic turn. The subsequent implementation and
acceptance are recorded below; ABI 10 and ordinary adapter behavior remain unchanged.

Diagnostic baseline correctness passes on RADV and llvmpipe at every extent/mode.
All 64 timestamp samples are observed in each measured pass/frame. The ordinary
consumer regression remains separate from the deliberately invalid optimized
variant and measured-only command-omission controls. Ordinary nine-frame and both
36-frame consumer controls pass on both drivers, as do all 20 runtime GPU tests
per driver, 27 ordinary Rust tests, 745 ABI layout checks, clippy, formatting and
shell syntax checks. Full logs remain under
`profile.12ht5mnq`, `profile.M8mr41Rh`, `omission.O1TYQdCu`,
`image-placement.UI06HUyX` and `placement.aNVpKXBL` in
`target/libplacebo-integration`.

## Runtime correction — 2026-09-15

The selected implementation now ranks eligible image memory types by locality
first, then absence of HOST_VISIBLE. It preserves every eligibility/exclusion
check, accepts visible local/UMA memory and retains the existing non-local
fallback. No type index, heap size or vendor is hardcoded. Buffer and descriptor
heap placement, shader compilation, synchronization and ABI 10 are unchanged.
Three selector tests cover ordering, masks, unified/visible-only memory and
excluded types. Both-driver integration/GPU validation and the full original
54-run hardware comparison are the acceptance gates, using the real Vulkan
loader with no diagnostic variants. All gates now pass at `1f41d7e`:

- All three engines match native exactly at all three extents and both modes on
  RADV and llvmpipe, with synchronization validation and adapter failure tests.
- Original nine-frame and both 36-frame consumer controls pass on both drivers;
  all 20 runtime GPU tests pass per driver.
- 30 ordinary Rust tests, 745 ABI layout checks, clippy and formatting pass.
  UMA compatibility is covered by synthetic selector tests and llvmpipe execution,
  not a new physical integrated-GPU test.
- All 54 ordinary hardware timing processes finish; no loader shim, command
  omissions, extra timestamps or compiler variant participates.

Fresh RX 5700 XT / RADV Mesa 26.2.1 results, median completed fps of three runs:

| Input / mode | Native Vulkan | OGPU per operation | OGPU per frame |
|---|---:|---:|---:|
| 64x33 resident | 13,782 | 2,700 | 5,588 |
| 64x33 transfers | 9,465 | 1,804 | 5,395 |
| 960x540 resident | 2,385 | 1,831 | 1,944 |
| 960x540 transfers | 301.67 | 274.94 | 294.47 |
| 1920x1080 resident | 593.17 | 554.75 | 563.54 |
| 1920x1080 transfers | 45.63 | 63.97 | 65.22 |

Output extents remain 129x67, 1921x1081 and 3841x2161. Near-4K resident ranges
are 591.79–593.37 fps native and 560.70–564.42 grouped OGPU: the remaining
throughput gap is about 5.0%. Near-1080p resident remains about 18.5% below native;
the small workload retains a substantial host-overhead gap and wider variation.
This correction does not establish universal parity.

The near-4K transfer result favors OGPU under these declared adapter/staging
policies, but is not isolated GPU/API throughput. Collection includes host copies,
native readback/allocation policies differ, and wait counts depend on work completed
while the CPU copies prior outputs. No new explanation for that mode is claimed.
Use the [full raw observations and ranges](results/libplacebo-image-fix-radv-2026-09-15.txt),
not comparisons between different days' unmatched runs. GPU clocks/load are still
uncontrolled; shader generation and native/OGPU paths are not identical binaries.

Retain the corrected runtime preference and consumer-side frame batching. This
completes the selected fix; no public API change, generalized allocator, memory
migration policy or further performance target is selected. The remaining gaps
are documented implementation work, not grounds for another automatic API experiment.
Local full results: `target/libplacebo-integration/perf.2QfNDRcd` (RADV timing and
validation), `perf.uOANsWmu` (llvmpipe validation), and `image-fix-*` regression logs.
