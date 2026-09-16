# Matrix executable requirements experiment

Selected 2026-09-16, after merging the validated Metal compute/GGML checkpoint.
This brief precedes implementation/results. This is a numerical-capability and
variant-selection experiment, not a general ML backend or GEMM performance claim.

## Hardware audit and selection

Local Vulkan discovery: RX 5700 XT (RADV NAVI10), Mesa 26.2.1. `shaderFloat16`,
`shaderInt8`, subgroup extended types, subgroup size control and integer dot
product support are true. The default subgroup size is 64. No KHR/NV cooperative
matrix extension is exposed. All reported integer-dot-product acceleration
properties are false. FP16 preserve/flush and RTE/RTZ properties are supported.
These are physical capabilities, not OGPU's enabled-device contract.

Select paired FP16 multiplication with FP32 accumulation. Do not implement a
cooperative-matrix path that cannot be tested here. Compare against a matching
paired FP32 kernel, reusing the matrix harness, explicit row strides, guards,
poisoning, independent references and separate execution timings. Keep the
original naive/tiled FP32 experiment unchanged as a control.

## Decision and alternatives

Question: can caller-owned executable metadata plus enabled capabilities select
a numerical variant cleanly, without a host tensor/operator API?

- Keep optional arithmetic disabled: cannot execute the selected candidate.
- Add a device-request/profile API now: explicit, but introduces a configuration
  surface before we have evidence for interactions between optional features.
- Enable supported FP16 arithmetic at creation, report it, and let callers choose
  compatible executables: selected for this bounded experiment. Unsupported
  devices keep the existing baseline. Metal already reports native FP16 support.

Only FP16 arithmetic is added to Vulkan enablement; do not enable every queried
feature. It remains optional, not a new hardware requirement. No C layout change
or host matrix operation is needed. The experiment owns a small variant record
with a semantic name, artifact and required capability. Test missing-capability
selection without submitting an incompatible shader. Requirements are trusted
caller metadata, not runtime SPIR-V reflection or validation. Keep numerical
semantics distinct from a hardware-support bit.

Vulkan [FP16 arithmetic](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceShaderFloat16Int8Features.html)
is separate from 16-bit buffer storage. This experiment keeps FP32 buffers/root
layout; only workgroup arithmetic/storage and products differ. It is not the
existing GGML FP16-weight/FP32-product contract.

## Numerical gates (before results)

Both variants compute C[M,N] from identical inputs already exactly representable
in FP16, with FP32 output and FP32 accumulation in ascending K order. Candidate
products are explicitly FP16, widened before accumulation. The control multiplies
in FP32. Prevent contraction/reassociation across the product/accumulate boundary.
Paired columns exercise vector arithmetic; odd N and tile tails must work.

Use finite bounded test inputs: zeros, identity, signed dyadic fractions,
cancellation and mixed scales. Nonzero input magnitudes are at least 1/16 and at
most 8, so nonzero products are normal FP16 values without overflow. This does
not claim NaN/Inf/subnormal or general float-control conformance.

Use a FP64 CPU product/sum oracle and S=sum(abs(a*b)). Control error must be at
most `1e-6 + 8*max(K,1)*FLT_EPSILON*S`. Candidate error must be at most
`1e-6 + (2^-10 + 8*max(K,1)*FLT_EPSILON)*S`: the extra term allows native FP16
product rounding without claiming a specific rounding mode. Check every output,
untouched padding/guards and unchanged inputs. Include values whose products
are not FP16-exact; require a precision sentinel to distinguish the two semantics.
Do not widen these limits after results.

Inspect generated SPIR-V: candidate must actually contain half-vector multiply
and widening into FP32 accumulation; control must not require Float16. Validate
both modules. Cover zero K, singleton, odd/rectangular/tile-aligned shapes and a
long reduction, plus unsupported-capability selection.

## Measurements and stop condition

Use the existing fixed benchmark shapes, alternating variant order, warmups and
nine measured samples. Report host execution separately from optional device
batch time, creation, transfers and query overhead. Correctness runs use Vulkan
validation; performance comparisons use a separately identified non-validation
run. No speedup threshold: an unsuccessful optimization is a valid result.

Stop after numerical/memory gates, actual shader requirements, capability tests
and Radeon measurements agree. Record whether caller-owned requirements remain
sufficient or expose a better API alternative. Do not automatically add numeric
families, a compiler framework, tensor operators or a larger model. A later
matrix-capability profile will need shape/type/subgroup constraints, not just a
Boolean; this experiment does not settle that richer contract. No new Metal
execution claim is made for the new candidate.

## Result — 2026-09-16

Completed at `f663a4b`; numerical gates were fixed in `5548584` before execution.
No tolerance was changed. Radeon and llvmpipe pass all 50 shape/pattern cases per
variant in both timed and untimed modes, plus every benchmark sample. The
precision sentinel distinguishes the two multiplication semantics. Maximum error
over all cases is 0.0157013 for FP32 and 0.299042 for FP16 products; maximum
error/bound is 0.0110872 and 0.242544 respectively. These maxima are not relative
error claims. Inputs, row padding and allocation guards remain intact.

Three additional Radeon runs disabled validation after the agent's other
regression jobs finished. Each run uses two warmups and nine samples per mode,
alternating variant order. The table reports the median of the three per-run
medians (milliseconds). Clocks were not locked; these are bounded observations,
not a general performance ranking.

| M × N × K | FP32 device batch | FP16-product device batch | FP32 untimed host execution | FP16-product untimed host execution |
|---|---:|---:|---:|---:|
| 128 × 128 × 128 | 0.0541 | 0.0561 | 0.2000 | 0.2030 |
| 257 × 193 × 129 | 0.0968 | 0.0998 | 0.2476 | 0.2511 |
| 256 × 256 × 256 | 0.1597 | 0.1735 | 0.3122 | 0.3247 |

Device intervals include batch-boundary barriers; host execution includes
recording, submission, wait and cleanup, excluding transfer and query retrieval.
The candidate's device-batch medians are roughly 3–9% slower. Smaller workgroup
storage and narrower products did not deliver a speedup for these kernels/shapes.
This does not isolate the cause or predict an optimized GEMM implementation.
Raw per-run min/median/max, creation and transfer measurements:
[run 1](results/ml-fp16-radv-2026-09-16-run1.txt),
[run 2](results/ml-fp16-radv-2026-09-16-run2.txt),
[run 3](results/ml-fp16-radv-2026-09-16-run3.txt).

**Decision:** retain optional FP16 enablement, explicit enabled-capability queries,
and caller-owned variant requirements. Hardware permission, numerical permission
and performance preference are three separate decisions. Do not select this
candidate automatically or change GGML's FP32 arithmetic. No host matrix API,
general device-feature negotiation API or new executable package is justified by
this experiment. The richer matrix shape/type/subgroup contract remains open.

The first run exposed a pre-existing xtask problem: Cargo's inherited loader
path could pick an old debug library instead of the newly built release library.
The enabled-feature check rejected the candidate, so it was not executed under
an incompatible device. The runner now puts its selected library directory first
on the platform loader path. All results above are from the corrected runner.
Earlier xtask-based host timings are not retroactively certified as release
measurements; this does not imply the independent consumer runners had that issue.

Regression receipt: 37 ordinary tests, strict workspace Clippy, 749 C/Rust layout
checks and loader mocks pass. All 21 GPU tests pass with validation on Radeon and
llvmpipe. The feature-chain interceptor covers both present and synthetically
absent Float16 on compute and graphics creation, while leaving Int8 disabled.
The original FP32 matrix experiment passes on Radeon. GGML DEVICE/F16 passes its
lifecycle, two matrix and six inference cases on llvmpipe. Shader inspection
confirms actual half-vector multiply, widening and FP32 addition with
NoContraction, and no Float16 capability in the control. No new Metal run is claimed.

## Reproduce

Use the normal Rust/C build environment, ripgrep and SPIRV-Tools supporting Vulkan
1.4. Checked-in binaries were generated with shaderc 2026.1 / glslang 16.4:

```sh
glslc --target-env=vulkan1.4 examples/shaders/matmul-paired.comp -o examples/shaders/matmul-paired.comp.spv
glslc --target-env=vulkan1.4 -DHALF_PRODUCTS=1 examples/shaders/matmul-paired.comp -o examples/shaders/matmul-half.comp.spv
bash examples/check-matmul-shaders.sh
VK_DRIVER_FILES=/path/to/radeon_icd.x86_64.json \
  VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 \
  cargo xtask matmul-half
```

For measurements, run separately without other regression jobs:

```sh
env -u VK_INSTANCE_LAYERS -u VK_LAYER_VALIDATE_SYNC \
  VK_DRIVER_FILES=/path/to/radeon_icd.x86_64.json \
  VK_LOADER_LAYERS_DISABLE=VK_LAYER_KHRONOS_validation cargo xtask matmul-half
```

Repeat three times; retain full output. `cargo xtask matmul` remains the original
FP32 control. Missing Float16 is reported as an unsupported candidate while the
FP32 control still runs; that result is not acceptance of the half variant.
