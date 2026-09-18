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

Next add the generated compute/raster programs, image backing and full normal/
diagnostic workload, then validate all outputs before adding paired timing.
Root/grid checks, image policy matching, query pools and the command-policy
inventory remain part of those later slices. The OGPU baseline is unchanged.
