# Flagship: learned image processing

Selected 2026-09-17. This acceptance brief precedes training and GPU results.
The milestone is one complete mixed ML/graphics application, not a new host
operator API or another isolated kernel benchmark.

## Application and decision

Run a small learned grayscale denoiser, conventional bilinear resize and affine
color processing, then render to an offscreen RGBA8 target. Keep activations and
processed pixels on the GPU from input upload through final rendering. The
question is whether our memory/executable/dependency model composes naturally
across this whole application, and where it exposes a better API alternative.

Select a project-owned, deterministic synthetic training/reference fixture. An
external pretrained photographic model would add model licensing, framework and
dataset dependencies before testing composition; a hand-written blur would not
exercise a learned executable. Synthetic training is a deliberate limit: success
does **not** demonstrate photographic denoising quality or a competitive network.

The fixed network is a residual CNN: 3x3 convolution (1 to 8 channels), ReLU,
1x1 convolution (8 to 1), add the noisy input, clamp to [0,1]. All 89 parameters
are trained. Inputs, weights, activations and GPU arithmetic are FP32. Use
replicated-edge padding and pixel-major, channel-interleaved activations. The
first layer reads input minus 0.5. Weight order is output channel, kernel y,
kernel x; then eight hidden biases, eight output weights and one output bias.
There is no quantization, matrix acceleration or numerical variant selection.

Training uses fixed-seed piecewise-smooth scenes with edges, independent additive
noise (sum of twelve centered uniforms, scale 0.1), and clean targets. Commit the
recipe, seeds, training configuration, FP32 weights and provenance. Training
seeds and held-out seeds must be disjoint. The fixed training run must not choose
weights or hyperparameters using acceptance outputs. No external data or model
license is needed; these project-authored assets use the repository MIT license.

Resize uses half-pixel coordinates and replicated edges. Apply RGB =
(0.95*g, 0.8*g+0.05, 0.6*g+0.15), alpha 1, then UNORM conversion. This simple
display palette is not a color-management or HDR claim. An independent scalar
CPU implementation owns numerical/reference semantics, not a GPU readback used
as its own oracle.

## Gates fixed before results

- Eight held-out scenes at 33x25 and 65x47: pooled denoised MSE must improve on
  noisy input by at least 3 dB and be no more than 0.25 dB worse than a replicated
  3x3 box filter. Every scene must improve on noisy input. Compare all pixels,
  including borders, using the stored FP32 inputs/weights. Failure means revisit
  the fixture openly, not silently lower the gate or train on acceptance scenes.
- GPU inference and pre-quantization processing must match the scalar FP64
  reference within `2e-5 + 2e-5*abs(reference)` per value, with finite outputs.
  Final RGBA8 differs by at most one code value per RGB channel; alpha is 255.
- Include 1x1, 1x17, 19x1, odd 33x25 and 65x47 inputs, with both downsize and
  upscale outputs. Tiny shapes are correctness tests, not quality evidence.
- Reuse allocations and executables for A/B/A frames. Poison outputs, check
  guards and unchanged inputs/weights, and require the repeated A result to match.
- The production-shaped path has one input upload and final image readback per
  frame, no intermediate host waits or reads. Weights/draw state are setup costs.
  A separately labeled diagnostic mode may copy intermediates **after** the
  complete chain, for the scalar checks. Count GPU representation copies
  explicitly; GPU residency is not a claim that no GPU copy occurs.
- Validate on the existing Radeon and llvmpipe. Record setup, transfers, host
  execution and optional device batch time separately; no speedup threshold or
  general native-Vulkan performance claim. No new hardware is a prerequisite.

## Implementation sequence and boundaries

1. Freeze and test the training/reference fixture, including quality results and
   provenance. This is a runnable CPU checkpoint, not GPU milestone acceptance.
2. Implement the full Vulkan application through the public C API. Start with
   address-based convolution executables and explicit dependencies. Choose the
   image handoff deliberately and account for copies. No runtime tensor objects,
   graph scheduler, hidden migration or model loader.
3. Run numerical, reuse and validation gates; document rough edges and decide
   whether they belong in the application, compiler workflow or runtime. Stop
   with one reproducible application and an honest result, not a tuning campaign.

Compiler-generated interfaces for the richer workload are the following large
step, not a prerequisite to hand-code the first executable contracts. Running
this same graphics workload on Metal and packaging an experimental release are
later milestones. Metal currently has compute acceptance, not graphics support;
this work does not trigger another Mac validation round trip.

## Working status

Acceptance brief selected; training/reference implementation is next. No learned
image result or GPU execution has been accepted yet.
