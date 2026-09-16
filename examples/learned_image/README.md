# Learned-image flagship

A project-owned, synthetic-data residual denoiser, scalar reference and complete
[mixed ML/graphics Vulkan application](../../docs/learned-image.md).

## Vulkan application

```sh
cargo xtask learned-image
cargo xtask learned-image --check
```

Requirements: the normal Rust/C environment, Python 3 standard library, `glslc`
(recorded shaderc 2026.1 / glslang 16.4), and SPIRV-Tools supporting Vulkan 1.4.
The runner compiles and validates all shaders into `target/learned-image/shaders`,
builds the release runtime/C application, and checks the CPU fixture. No shader
compiler is downloaded automatically; `GLSLC`, `CC`, and `CARGO` can select tools.
`--check` builds/checks without GPU execution. The normal command needs the
modern Vulkan graphics profile. Select a driver with `VK_DRIVER_FILES`; enable
`VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` and `VK_LAYER_VALIDATE_SYNC=1`
for acceptance. Leave `CARGO_TARGET_DIR` unset and serialize application runs
sharing this checkout's generated files.

The runner executes 38 cases in both final-image-only and diagnostic modes,
checks scalar references and repeated A/B/A frames, and rejects Vulkan validation
errors. GPU files are under `target/learned-image/gpu`; `last-run.txt` records the
latest successful run, replacing the previous receipt. Save it before testing a
different driver if both receipts are needed. Failure is not a successful receipt.

`app.c` owns allocation/lifetime, command order and semantic parameter packing.
Three compute executables implement hidden convolution, residual output, and
resize/palette. The fragment shader reads the compute-written RGBA float buffer
directly and renders a fullscreen triangle. There are no intermediate image
copies or heap bindings in this representation. All five numerical allocations
are DEVICE buffers. Only input upload, final readback and constant draw state
need HOST memory; weights upload once at setup. Explicit dependencies cross each
stage and each reused frame. Public owners stay alive until completion.

Diagnostic mode adds GPU NaN poisoning and end-of-chain copies of guarded buffers;
there is still only one submission/wait per frame, never a mid-pipeline host read.
Normal mode records exactly three compute dispatches and one draw, one guarded
input upload and one final image copy, with no intermediate copies. Both modes
must produce identical final pixels. Guard/input/weight checks and scalar
intermediate comparisons belong to diagnostic mode, not a claim that normal
execution continuously validates shader memory accesses.

Per-frame instrumentation separates CPU staging/readback, record-submit-wait,
query retrieval, optional whole-device-batch duration and transfer byte counts.
Device batch includes input/final copies and barriers; diagnostic mode additionally
includes poisoning/copies. Host-write time includes readback poisoning. Setup
includes device/executable creation, allocation and weight upload. These cold,
validation-enabled correctness runs are **not benchmarks** or an isolated transfer
cost measurement. File I/O/reference comparisons are outside execution timings.

The hand-maintained C/GLSL roots are deliberate first-implementation scaffolding;
extending the compiler-generated interface to this workload is the next milestone.
The application does not add a public model/tensor/operator API. It is Linux
Vulkan acceptance, not a Metal graphics implementation or portability claim.

## CPU fixture and training

```sh
python3 examples/learned_image/check.py
python3 examples/learned_image/check.py --retrain
```

Python standard library only; no downloads, NumPy, PyTorch or GPU needed. The
second command also repeats the fixed training run and requires a byte-identical
`model.json`. Recorded training environment: CPython 3.14.7 on Linux x86-64.
Bit-identical training across Python versions/platform math libraries is not
promised. Normal checks use the frozen weights and do not retrain or overwrite
them. An intentional new model requires `train.py --output <path>` and a new
acceptance record; do not replace the model just to make reproduction pass.

`reference.py` defines scenes, the FP64 scalar oracle over FP32 inputs/weights,
replicated padding, bilinear resize, affine palette and RGBA8 conversion.
`train.py` implements fixed-step Adam in binary64, then rounds all 89 parameters
to FP32. It trains on 32 scenes of 16x16 pixels, seeds 41–72, independently sampled
minibatches of 32 patches for 4,096 steps. The final checkpoint is used, with no
held-out selection. Architecture, noise, seed/configuration, source hashes and
FP32 weight hash are recorded in `model.json`. All data, code and weights are
project-authored and covered by the repository MIT license.

The checker runs analytical/rejection tests, verifies provenance and fixed
held-out quality gates, and exports raw files under
`target/learned-image/reference`. `manifest.json` specifies extents, grouping,
little-endian encodings and hashes. It includes eight quality cases plus 30
lifecycle cases: five input shapes, two resize extents and A/B/A frames per group.
FP32 input and weights are executable inputs; FP64 hidden, denoised and processed
values plus RGBA8 final pixels are diagnostic oracles. Never feed those outputs
to GPU inference. Clean targets are evaluation-only data.

`preview.ppm` shows clean / noisy / denoised for held-out seed 1005 using the
application palette. It is generated for inspection, not a separate acceptance
gate. `quality.json` contains all per-scene metrics. Generated files are not
versioned; their source recipe and the frozen learned model are.

Quality applies only to this synthetic piecewise-smooth/noise distribution.
Neither the small fixture sizes nor successful denoising establish performance,
photographic quality or Metal graphics support. CPU checks alone are not GPU
acceptance; that requires the application command above.
