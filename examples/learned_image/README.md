# Learned-image flagship

A project-owned, synthetic-data residual denoiser, scalar reference and complete
[mixed ML/graphics Vulkan application](../../docs/learned-image.md).

## Vulkan application

```sh
SLANGC=/path/to/slangc cargo xtask learned-image
SLANGC=/path/to/slangc cargo xtask learned-image --check
SLANGC=/path/to/slangc cargo xtask learned-image --scale
```

Requirements: the normal Rust/C environment, Python 3 standard library, pinned
Slang **2026.14.1**, and SPIRV-Tools supporting Vulkan 1.4. The runner compiles and
validates all six Slang stages, generates checked C interfaces with embedded
SPIR-V, builds the release runtime/C application, and checks the CPU fixture.
No compiler is downloaded automatically; `SLANGC`, `CC`, and `CARGO` select tools.
`--check` byte-compares the checked-in headers and builds/checks without GPU
execution. Ordinary runs regenerate the headers. The normal command needs the
modern Vulkan graphics profile. Select a driver with `VK_DRIVER_FILES`; enable
`VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` and `VK_LAYER_VALIDATE_SYNC=1`
for acceptance. Leave `CARGO_TARGET_DIR` unset and serialize application runs
sharing this checkout's generated files.

The runner executes 38 cases in both final-image-only and diagnostic modes for
two interface variants: original and mutated (reversed root fields, local size
64 to 32). Both compile the same host source and must produce identical outputs.
It checks scalar references and repeated A/B/A frames and rejects Vulkan validation
errors. GPU files are under `target/learned-image/gpu/small/{original,mutated}`;
`last-run-small-original.txt` and `last-run-small-mutated.txt` record the latest successful
variant runs, replacing previous receipts. Save them before testing another
driver if both receipts are needed. Overall acceptance also requires the final
cross-variant equality check; a stale/partial receipt is not a successful run.

`--scale` additionally checks six video-sized A/B/A groups, in both modes and
both interface variants (18 cases / 72 frame executions):

| Input | Output(s) |
|---|---|
| 1280x720 | 2560x1440 |
| 1920x1080 | 960x540 |
| 3840x2160 | 4097x2305 and 1919x1079 |
| 1919x1079 | 2561x1441 and 1277x719 |

Allow at least 16 GB of disk space for a fresh scale run and several minutes for
full-output CPU comparisons. Existing artifacts are overwritten, not pruned. Large
fixtures are in `reference-scale`, GPU outputs in `gpu/scale/{original,mutated}`,
and receipts in `last-run-scale-{original,mutated}.txt` beneath the same build
directory. `--check --scale` generates/cross-checks references and builds without
GPU execution. Large acceptance targets Radeon; keep the default small run for
llvmpipe. This remains correctness work, not a performance benchmark.

Every run builds `reference_stream.c` with binary64 arithmetic and contraction
disabled, and requires all six files from each of the 38 Python cases to match
byte-for-byte. This row-based reference then supplies the large oracles without
whole-image Python number arrays. Comparisons and hashing stream through files
and check every value. The input generator/model/border policy and numerical
gates are unchanged. See the [scale acceptance brief](../../docs/learned-image-scale.md).

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

The generated headers in `generated/` supply C roots/padding/assertions, embedded
artifacts, push sizes, stage, local dimensions and enabled-capability predicates.
`generate_interfaces.py` selects the application sources and mutation fixture;
the shared [compiler adapter](../compiler/generate.py) checks the reflected
mechanics against SPIR-V. Compiler scratch/reflection files and mutated headers
live in `target/learned-image/compiler`. The C code uses semantic field names and
derives X/Y dispatch grids from generated dimensions and device limits, with an
application row policy of at most 1024 workgroups in X (clipped to the device
limit). This ensures large acceptance exercises Y/tail addressing even on GPUs
with very wide X limits; it is not a claim of optimal launch geometry. An
explicit generated root field carries the invocation-row stride; shaders flatten
`id.x + id.y * dispatch_width`. Logical and padded invocation counts, guarded
buffer sizes and signed coordinates are checked before allocations/submissions.
Resize uses an integer quotient/remainder to select the source pixel and converts
only the interpolation fraction to FP32. Each axis must satisfy
`(2 * output_extent - 1) * input_extent <= UINT32_MAX`; unsupported products reject
before allocation rather than wrapping. This covers the planned video extents.
The current shader contract keeps local Y/Z equal to one; a different local
decomposition needs an explicit policy. No handwritten root layouts
or shader file loading remain. See [migration acceptance and exact supported
subset](../../docs/learned-image-compiler.md). The old GLSL implementations remain
in git history, not as a maintained alternate path.
The application does not add a public model/tensor/operator API. It is Linux
Vulkan acceptance, not a Metal graphics implementation or portability claim.

## Warmed measurement

```sh
SLANGC=/path/to/slangc cargo xtask learned-image-benchmark --check
SLANGC=/path/to/slangc cargo xtask learned-image-benchmark
```

Also requires Vulkan development headers and `pkg-config` for a diagnostic-only
allocation loader. Start with `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation`
and `VK_LAYER_VALIDATE_SYNC=1`, an explicitly selected Radeon ICD, and the real
`OGPU_VULKAN_LIBRARY` (or the normal system loader). The runner performs separate
validated A/B/A checks, then disables layers for ordinary timing; it never reports
traced/validation timings as benchmark samples. `--validate-only` omits timing
and native allocation runs; `--check` needs no GPU.

The [measurement protocol](../../docs/learned-image-measurement.md) defines the
four selected extents and limitations. Resident mode alternates two pre-uploaded
DEVICE inputs, with final readback outside timing. End-to-end mode includes CPU
staging, upload and final readback, but not disk I/O. Both serialize one frame at
a time: 10 warmups, 30 samples, three fresh processes per extent/mode. This is an
OGPU baseline, not maximum throughput or a native Vulkan comparison.

Each run preserves a new `target/learned-image/measurement-*` directory containing
revision/artifact hashes, raw samples, per-run statistics, validation outputs and
allocation traces in `report.json` and per-process logs. Only `complete: true`
means the entire measurement/trace sequence passed. A validation-only result has
`validation_complete: true` instead. Traces report successful Vulkan allocations,
not driver-private memory, physical residency or total process memory. Setup and
requested CPU/HOST/DEVICE payload bytes are separate from native peak bytes.
Large references/outputs require substantial disk space; nothing is auto-deleted.
Use `python3 examples/learned_image/benchmark.py` for live progress (xtask captures
its child output until completion). Serialize runs sharing generated files.

Export a completed report into a new directory for review or a committed receipt:

```sh
python3 examples/learned_image/export_measurement.py \
  target/learned-image/measurement-IDENTIFIER/report.json /path/to/new-receipt
```

The export retains all 720 measured samples in CSV, per-run statistics, revision/
artifact hashes, validation summaries and all eight native allocation traces.
It rechecks sample/count/accounting consistency and refuses incomplete reports
or an existing destination. Full GPU diagnostic outputs remain in the local run
directory; exporting does not rerun correctness or establish a native comparison.

After `run.py --check`, with synchronization validation and an explicitly selected
ICD, `python3 examples/learned_image/check_display_edges.py` repeats the odd-edge
65x47 -> 131x95 A/B/A regression in 20 fresh processes across both interfaces.
The display shader bounds fragment coordinates before indexing its raw pointer,
including invocations outside the visible target. Guard words check writes, not
out-of-bounds reads. See the [failure analysis](../../docs/learned-image-measurement.md#acceptance-interruption--2026-09-19).

## Native Vulkan control

```sh
python3 examples/learned_image/run_native.py --check
VK_DRIVER_FILES=/path/to/selected_icd.json python3 examples/learned_image/run_native.py
SLANGC=/path/to/slangc python3 examples/learned_image/run_native.py --workload
SLANGC=/path/to/slangc python3 examples/learned_image/run_native.py --scale
```

The default runs the standalone setup/address-copy smoke test. `--workload` adds
the complete learned-image small odd-edge A/B/A case; `--scale` also checks the
four selected measurement extents on Radeon. These are correctness runs; use the
separate paired runner below for performance. It needs a C11
compiler, Python 3, `nm`, the repository's pinned Vulkan headers and a dynamic
loader. GPU execution requires the same synchronization-validation environment
as above and exactly one physical device from the selected ICD. Workload runs
regenerate/check both interfaces and fixtures, and build fresh OGPU controls;
they need the same Slang/SPIRV-Tools/Rust environment as the ordinary application.
The transfer smoke alone does not need a Slang rebuild.

`--check` runs injected host policy/cleanup tests and verifies that the executable
has no OGPU runtime symbol dependency. GPU execution additionally performs exact
A/B/A transfer checks and validates an allocation trace with zero live allocations
at exit. Artifacts and revision/source hashes remain under
`target/learned-image/native-control/`. The [native-control brief](../../docs/learned-image-native-control.md)
tracks the implementation and paired-measurement work still pending.

Workload acceptance checks the full CPU reference once per extent, then exact
equality of all 21 output files across native/OGPU, resident/end-to-end and the
small-case original/mutated interfaces. All these processes have validation and
allocation tracing enabled. Their memory-size/type sequences must agree and all
allocations must be freed. The report retains output hashes and full traces.
Allow substantial additional disk space for full diagnostics (about 10 GB for
one scale workload run); outputs are never auto-deleted. Serialize builds/runs
that share generated files. `--workload --check` or `--scale --check` builds and
checks without GPU execution.

For a committed correctness receipt, export a complete report to a new filename:

```sh
python3 examples/learned_image/export_native_workload.py \
  target/learned-image/native-control/workload-IDENTIFIER/report.json /path/to/new-receipt.json
```

The exporter rechecks the exact run matrix, all retained output hashes, per-extent
output equality and allocation traces before writing. It refuses incomplete
reports or an existing destination. It does not rerun the CPU oracle or export
the large diagnostic buffers themselves.

### Matched native/OGPU measurement

```sh
SLANGC=/path/to/slangc python3 examples/learned_image/compare_native.py --check
SLANGC=/path/to/slangc python3 examples/learned_image/compare_native.py
python3 examples/learned_image/export_comparison.py \
  target/learned-image/native-control/comparison-IDENTIFIER/report.json /path/to/new-receipt-directory
```

Start with the same synchronization-validation environment and selected Radeon
ICD as above. The runner performs fresh full-output validation, then 48 timing
processes with validation/tracing disabled and rotated control order, then 16
independent allocation-trace runs. Every timing process uses 10 warmups and 30
samples and checks its final B image against the full validated result. Native
timestamps, memory and one-shot submission policy match the OGPU control.
Resident final snapshot is outside timing; end-to-end includes HOST write/upload/
readback, not disk I/O. No intermediate CPU work is introduced.

This measures instrumented serialized latency, not peak throughput or isolated
API overhead. Interpret recording+submission together because OGPU lowers commands
at submit while the direct control records immediately. Allow roughly 11 GB of
additional space per complete run; it retains full validation buffers, all 1440
ordinary samples, per-process statistics and native allocation traces. The exporter
keeps reviewable statistics, CSV samples and correctness/hash/trace evidence, not
the large GPU dumps. Existing results are never overwritten or auto-deleted.

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
