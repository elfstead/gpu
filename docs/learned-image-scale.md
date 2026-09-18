# Learned-image scale: correctness slices

Selected 2026-09-18, implementing the first slice of [roadmap M1](roadmap.md).
This brief precedes implementation/results. It is not the complete performance
milestone, a new model, or a public API extension.

## Contract and cases

Keep the frozen 89-parameter model and existing Python oracle authoritative.
Preserve the scalar bound `2e-5 + 2e-5 * abs(reference)`, RGB error at most one
code, alpha 255, guard words, unchanged inputs/weights, and A/B/A reuse checks.
Do not reinterpret video-sized inputs as a photographic-quality claim.

First add two scale groups: 1280x720 -> 2560x1440 (upscale) and
1920x1080 -> 960x540 (downscale), each with seeds 2001/2002/2001. These exact
factor-two resizes isolate scale/dispatch correctness; arbitrary ratios, 4K and
odd video extents remain the next M1 slice. Run both normal/diagnostic modes and
both generated-interface variants (64/32 local threads and reversed root fields).

Map the existing flat invocation sequence onto an X/Y grid using device limits
and generated local dimensions. Pass the row stride explicitly in generated
roots. Reject unrepresentable sizes and padded invocation grids before allocating
or submitting. The shaders still use uint32 indexing; do not silently wrap counts,
guard sizes, signed image coordinates, or host allocation arithmetic.

Use at most 1024 workgroups per X row, clipped to queried device limits, so the
large cases exercise Y addressing even on devices permitting very wide X grids.
This application policy is not a new runtime limit or a tuning result.

## Reference and checks

Add a scalar C binary64 reference compiled without contraction or fast math.
It must reproduce input generation and every intermediate/final output of all 38
existing Python fixtures before it can supply large oracles. Use only a few rows
of memory: convolution reads clamped neighboring source rows; resize reads the
two required denoised rows. Clamp at the image boundary, not a chunk boundary.
Stream files and comparisons, including hashes and cross-variant equality, so
Python does not materialize large arrays of boxed numbers. Compare every value,
not a sample. No GPU output feeds the CPU reference.

Test size/dispatch boundaries without a GPU, including padding, overflow and
insufficient limits; test rejection of corruption beyond comparison chunk
boundaries. Preserve all small fixtures on Radeon and llvmpipe. Large acceptance
uses the existing Radeon with Vulkan and synchronization validation. Software
execution of the large cases is optional, not a new prerequisite.

## Completion and deferrals

Done when the above reference, boundary, small-regression and large GPU gates
pass with a recorded revision/toolchain/driver receipt. Record any failed gate
before changing the implementation or proposing a semantic/tolerance revision.

This slice does not add warmed performance modes, a native Vulkan control,
allocation policy changes, concurrent frames, or Metal graphics. Cold timings
remain diagnostics, not benchmark results. It does not close M1: 4K/odd extents,
general resize ratios, repeated timing and the matched native control remain.

## Initial failed run

At `770ffe2`, Radeon small fixtures and C/Python byte-equality checks passed.
The first large group failed the processed-color scalar gate, despite guard and
normal/diagnostic final-image checks passing. At output pixel (303, 1325), red
was `0.5249907970428467` versus reference `0.5249600958985924`: error
`3.07011442543e-5`, exceeding the existing bound `3.04992019180e-5`.
The checker stopped; neither this group nor the scale suite was accepted.
Raw failed invocation: `/tmp/ogpu-scale-radv-20260918.log` (local artifact).

The resize source computes `(coordinate + 0.5) * input_extent / output_extent`
in FP32. Investigate its coordinate precision at large extents without changing
the mathematical half-pixel resize, model, scalar tolerance or pixel gate.

At `db5d8ad`, forming the floating-point scale before the coordinate multiply
still produced the exact same failing value; reassociation alone is not accepted
as a correction. Raw retry: `/tmp/ogpu-scale-radv-retry-20260918.log`.
The next implementation uses checked integer quotient/remainder for source-pixel
selection and FP32 only for the bounded interpolation fraction. This preserves
the mathematical half-pixel rule without multiplying fractional rounding error
by a large coordinate. Its extra uint32 product limit must be checked explicitly.

## Accepted first slice

Complete at `86dd16e`; [receipt](results/learned-image-scale-2026-09-18.txt).
The checked integer-coordinate implementation passes all 38 small cases on
Radeon and llvmpipe, and both video-scale A/B/A groups on Radeon. Both modes and
both interface variants pass, including byte-identical outputs between variants.
Final acceptance uses real multi-row grids (hidden Y reaches 507), not merely
a code path that could use Y. Guards, full intermediate comparisons and all
original numerical gates remain intact. Maximum scale scalar error is
`2.00820588e-7`; maximum RGB code difference is one.

The C oracle reproduces all 38 Python cases byte-for-byte and uses at most
161,280 bytes of allocated row payload for these cases. The GPU still retains
full-image activations; this is not a tiled GPU inference implementation.
Integer resize products are checked per axis before allocations. Unsupported
products reject; no FP64 shader capability or hidden fallback was added.

Retain this implementation. Next: 4K/odd extents and arbitrary resize ratios
with unchanged gates, then warmed measurements and the matched native control.
The large case exposed an application precision defect, not a reason to widen
the public API. M1 as a whole remains active.

## Second slice: 4K, odd extents and non-integer ratios

Selected 2026-09-18, before implementation/results. Extend `--scale` with the
following four groups, retaining both first-slice groups:

| Input | Output | Coverage |
|---|---|---|
| 3840x2160 | 4097x2305 | 4K inference, non-integer upscale, odd output and tails |
| 3840x2160 | 1919x1079 | 4K inference, non-integer downscale, odd output |
| 1919x1079 | 2561x1441 | Odd input, non-integer upscale, odd output |
| 1919x1079 | 1277x719 | Odd input, non-integer downscale, odd output |

Every group uses seeds 2001/2002/2001, both normal/diagnostic modes and both
generated-interface variants. The full scale suite becomes 18 cases / 72 frame
executions; the original 38 small cases remain controls. All prior gates apply:
full binary64 intermediate references, unchanged scalar/pixel tolerances, guards,
unchanged inputs/weights, A/B/A and cross-variant equality. Continue cross-checking
the streaming C oracle against all small Python fixtures. Large CPU/reference
work may take minutes and several GB of disk; neither sampling nor success-only
case selection is permitted. Insufficient device limits remain unresolved cases.

Add host-only boundary tests for every selected extent, 64/32-thread launches,
poison guards, partial Y rows and resize-product limits. Run the complete suite on
the existing Radeon with synchronization validation; retain small llvmpipe checks.
Do not change runtime/shaders unless a failing gate identifies a concrete defect.

Done when all six scale groups pass, with a committed result covering failures
as well as successful checks. This closes M1's declared extent/correctness matrix,
not M1 itself: warmed timing modes, allocation accounting and a matched native
Vulkan control remain separate work. No new performance or portability claim.

## Accepted second slice

Complete at `6d36e93`; [4K/odd-ratio receipt](results/learned-image-4k-2026-09-18.txt).
All six scale groups pass on Radeon with both interface variants and both modes,
including full intermediate checks, guards, unchanged inputs/weights, A/B/A reuse
and byte-identical cross-variant outputs. Small controls also pass on Radeon and
llvmpipe. The full run covers 224 Radeon frames and 152 llvmpipe frames.

Maximum scale scalar error is `2.07155088e-7`, maximum error/bound is
`0.00768996319`, and RGB code difference remains at most one. No tolerance,
model, shader, application execution or runtime change was needed in this slice;
the first slice's integer-coordinate correction handles the non-integer ratios.
No new failed gates. Host boundary tests cover each selected extent and both
generated workgroup sizes, including guarded counts and incomplete final rows.
The C reference's allocated row payload peaks at 322,560 bytes, not counting
stdio/stack/allocator overhead; GPU activations remain full-image allocations.

The declared extent/correctness portion of M1 is complete. Next add warmed
resident/end-to-end timing modes and allocation accounting, then the matched
native Vulkan control with the same shaders and execution policy. Cold validation
timings here are not a benchmark or a parity claim; M1 remains active until its
measurement/control deliverables are complete.
