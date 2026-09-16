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

## CPU fixture checkpoint

CPU fixture completed at `f28bd28` on 2026-09-17. The first fixed
training run passed unchanged gates over 15,520 held-out pixels: 6.0236 dB gain
over noisy input and 2.4932 dB over the box control, with improvement in every
scene. All 13 analytical/provenance/quality tests pass. Repeating training on
CPython 3.14.7 / Linux x86-64 reproduced `model.json` byte-for-byte. Training never
evaluates held-out scenes; there was no hyperparameter/checkpoint selection after
these results. This is synthetic-distribution evidence only.

The [fixture and reproduction instructions](../examples/learned_image/README.md)
include 38 exported CPU cases: eight quality scenes and ten A/B/A reuse groups
covering all five input shapes and both resize directions. The source recipe and
89 FP32 weights are versioned; generated inputs, scalar intermediate/final oracles
and a viewable preview live under `target/learned-image/reference`. This checkpoint
preceded GPU implementation and did not itself establish GPU correctness.

## Vulkan result — 2026-09-17

Complete at `21dc090`, against the unchanged gates committed in `b4e4aee`.
`cargo xtask learned-image` executes 38 cases in each of two modes on both
RX 5700 XT / RADV and llvmpipe, with Vulkan and synchronization validation.

The application owns five DEVICE allocations (input, weights, hidden features,
denoised pixels and processed RGBA floats), a final RGBA8 render target and HOST
staging/readback/draw buffers. Three compute dispatches produce hidden features,
residual output and resized/color-processed pixels. A fullscreen fragment shader
reads the last allocation directly; rasterization writes the final image.

Per frame: **input upload → convolution/ReLU → residual output → resize/palette
→ raster → final readback**. Weights upload once during setup. The ordinary path
uses one submission and final wait, with no intermediate host access, heap binding,
buffer-to-image conversion or other GPU representation copy. This is a useful
address-based handoff, not a claim that arbitrary texture workloads need no
images/samplers. Resize here is explicit shader arithmetic, not hardware filtering.

Diagnostic mode additionally poisons all scratch allocations with NaNs on the
GPU and appends guarded buffer copies after rendering. It checks every hidden,
denoised and processed scalar against the independent FP64 reference. Input and
weights remain byte-identical; guards are intact. Ordinary and diagnostic final
pixels agree exactly with each other. All ten A/B/A groups distinguish B and
reproduce A exactly, including diagnostic intermediates. Neither mode has an
intermediate CPU wait/read.

| Driver | Maximum scalar error | Maximum error / allowed bound | Maximum RGB code difference |
|---|---:|---:|---:|
| Radeon / RADV | 2.2891e-6 | 0.08458 | 1 |
| llvmpipe | 1.1085e-6 | 0.03342 | 1 |

Alpha is exactly 255. The scalar maxima cover hidden features, denoised values and
processed colors; they are not relative-error claims. The quality gates remain
those of the frozen synthetic fixture, not a new photographic-quality claim.
Raw receipts: [Radeon](results/learned-image-radv-2026-09-17.txt) and
[llvmpipe](results/learned-image-llvmpipe-2026-09-17.txt).

Setup, host staging/readback, record-submit-wait and optional whole-batch timing
are reported separately, with upload/final/diagnostic byte counts. These runs
include validation, instrumentation and cold first-use costs, with no performance
acceptance threshold. They are not a native-Vulkan comparison, steady-state
benchmark, or isolated GPU transfer/kernel timings. Diagnostic mode follows the
normal mode and therefore is not a valid overhead comparison. Readback poisoning
is included in host-write time. File I/O and CPU references are outside execution
timings; device batch time includes transfers/barriers and any diagnostic work.

Regression receipt: 13 reference/provenance tests, four numerical-checker rejection
tests, byte-identical retraining, shader validation, no-GPU build/check, 37 ordinary
Rust tests, strict Clippy, formatting and 749 ABI layout checks pass. All 21 existing
GPU tests pass on both drivers. Tool versions: CPython 3.14.7, shaderc 2026.1 /
glslang 16.4, SPIRV-Tools 2026.3. Runtime/header unchanged; GGML/libplacebo were
not rerun for this application-only addition. No new Metal run or support claim.

**Decision:** keep the existing runtime model. This application has not exposed
a better host operator/resource API alternative. Model knowledge, storage choices,
dependencies and reuse fit in the consumer without a tensor API or scheduler.
This bounded success does not stabilize the API or establish general performance.

The visible remaining duplication is mechanical: handwritten C/GLSL root layouts,
local sizes, executable requirements and artifact wiring. The next large milestone
is to carry this application through the compiler-generated workflow, preserving
the frozen fixtures, public ownership rules and all acceptance gates. Prefer that
over further network/kernel tuning. Same-application Metal graphics and an
externally usable experimental release remain later milestones.
