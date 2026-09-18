# Learned-image measurement: OGPU baseline

Selected 2026-09-18 after [scale correctness](learned-image-scale.md). This is
the OGPU measurement half of roadmap M1, not the matched native Vulkan control
or a performance-parity claim. Define the protocol before collecting results.

## Modes and workload

Use the accepted model, generated shaders, checked X/Y grids and one graphics
queue. Select four existing scale groups: 1280x720 -> 2560x1440,
1920x1080 -> 960x540, 3840x2160 -> 1919x1079, and 1919x1079 -> 2561x1441.
Inputs alternate seeds 2001/2002; weights upload once. No arithmetic tuning.

- Resident: preload both guarded inputs into separate DEVICE buffers. Alternate
  their addresses each frame. Measure three dispatches, one draw, dependencies,
  submission and completion; no per-frame input transfer or image readback.
  Read the last image in a separate, untimed submission.
- End-to-end: preload both inputs into CPU memory. Each frame includes HOST
  staging write, guarded upload into one DEVICE input buffer, the same compute/
  raster stages, final image copy and CPU readback. Disk I/O is excluded.

Use 10 warmups and 30 measured frames per fresh process, three Radeon processes
per extent/mode. Rotate mode order between rounds. Re-record one-shot batches;
one frame is in flight. Report this as instrumented serialized frame latency,
not peak throughput. Separate setup, CPU upload, recording, submission, wait,
query retrieval, readback, retirement and complete-frame time. Record optional
whole-batch device time, including whichever transfers that mode records.
Do not subtract clocks to claim isolated API overhead. Preserve all samples;
report per-run mean/median/p95 and across-run ranges, not just best FPS.

## Acceptance

Before timing, run A/B/A in both modes with Vulkan synchronization validation,
both normally and with full diagnostic copies. Check every scalar intermediate
and final pixel against the unchanged oracles. Exact equality can establish the
same gate transitively for repeated A, resident vs end-to-end, or interface
variants; no sampled comparisons. Verify guards and unchanged input/weights.
Include a small case and original/reversed-root/32-thread interfaces there;
large validation and timing use the original generated interface. Retain the
ordinary small correctness run on Radeon and llvmpipe after harness changes.

Timing disables validation and allocation tracing. Every timed process must
finish successfully, produce all 30 finite/nonnegative samples, and reproduce
the validated final B image. Full diagnostic copies, output comparisons, logs,
file I/O and the resident-mode final snapshot are outside measured intervals.
Validation is a separate execution of the same mode, not proof that every timed
intermediate was checked. No large-run timings on llvmpipe are acceptance evidence.

## Memory accounting

Report explicit application CPU payload allocations, requested HOST/DEVICE
buffer bytes and logical RGBA8 image bytes separately. They are not actual native
allocation sizes. Do not retain the full diagnostic readback allocation in the
ordinary timing mode; validation may allocate it and must say so.

Use a separate single-device loader diagnostic to observe successful Vulkan
allocation/free calls, including sizes, memory types, counts, peak live bytes and
zero remaining tracked allocations after cleanup. Never alter placement or link
the diagnostic into ordinary timing. Check its bookkeeping without a GPU. Trace
the same 40-frame modes and verify the final output; exclude traced timings from
the benchmark. Vulkan allocation bytes exclude driver-private memory, CPU/Rust
objects and physical residency; report those limits instead of claiming total
process/VRAM usage. Retain full traces so per-frame allocation activity is visible.

## Completion and next step

Done when the gates pass and a revision-identified receipt contains raw samples,
summary statistics, transfer policy, allocation peaks/counts and a bounded reading
of the costs. Unsupported timing is explicit, not a zero-duration sample. Reject
malformed/partial samples and stale or corrupted output in host tests. Commit
failed gates before semantic changes; do not silently loosen tolerances.

Then implement the benchmark-only native Vulkan control with identical SPIR-V,
grids, memory placement, stage/transfer/queue policy and timing boundaries. M1
remains open until that comparison is completed; no runtime/API expansion or
kernel optimization is selected just to improve these baseline numbers.

## Acceptance interruption — 2026-09-19

At `5befd7f`, the Radeon measurement sequence completed all 24 timing processes,
eight allocation traces and twelve mode/interface validation processes. Its local
report is `target/learned-image/measurement-jtn0ggzk/report.json`. Do not treat it
as full accepted regression evidence: a subsequent ordinary llvmpipe run crashed
in the mutated 65x47 -> 131x95 A/B/A case. An unmodified retry crashed in the same
group with the original interface. Radeon ordinary small regression passed.

The retained core for PID 1528135 identifies a faulting llvmpipe fragment raster
worker while the main thread waits for completion. Register values include row
95 for a 95-row target. The display shader uses unchecked fragment coordinates
to read the raw color pointer and carries width but not height: invocations past
the image edge can form out-of-range reads. This is an application shader defect,
not evidence that moving batch destruction after completion is invalid or proof
of a driver bug. Guard words only detect writes; they do not make reads safe.
The faulting JIT instruction loads from `0x7d71d8bcf210`; the SIMD base pointer
is `0x7d71d8b9e040`, with float index 50292. The color payload contains only
`131 * 95 * 4 = 49780` floats, so the read is 2048 bytes beyond its end, well
beyond the 64-byte trailing guard. This confirms the address error directly.

Vulkan permits additional helper fragment invocations, and physical-storage
pointer accesses must stay inside a buffer's address range. See the
[fragment execution model](https://docs.vulkan.org/spec/latest/chapters/shaders.html)
and [physical storage buffer rules](https://docs.vulkan.org/spec/latest/chapters/descriptors.html).
Clamp coordinates to both image dimensions before forming the pointer index,
preserving all visible pixels. Regenerate both interfaces and rerun acceptance;
do not loosen tolerances or accept the earlier timings as the corrected baseline.

Local failed logs: `/tmp/ogpu-measure-small-lvp-20260918.log` and
`/tmp/ogpu-measure-small-lvp-retry-20260919.log`. Core extracted to
`/tmp/ogpu-app-mutated-1528135.core`; systemd also retained the original core.
