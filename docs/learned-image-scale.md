# Learned-image scale: first correctness slice

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
