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
