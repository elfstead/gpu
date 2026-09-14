# GGML FP16-weight / FP32-arithmetic checkpoint

Selected 2026-09-14. This is the next bounded D1 experiment, not a general ML
backend or performance milestone. Keep the existing pinned GGML revision
`7840aaba1989c6deeefede1d77d5aaf8f52b947e`, trained fixture, upstream MNIST
forward graph and public C interface. Do not add tensor operators, accelerated
matrix instructions, training, new numeric families or asynchronous scheduling.

## Audit and API decision

Three alternatives were considered: enable native 16-bit buffer storage in the
baseline; add an explicitly requested optional execution profile; or use packed
32-bit loads and software half unpacking. Select **native storage in the baseline**.
Our existing Vulkan 1.4 requirement already mandates physical support for
`storageBuffer16BitAccess`; enabling it adds no new conformant-hardware exclusion.
An optional profile would introduce a selection branch without a hardware
distinction here. Packed loads impose unnecessary padding/alignment obligations.
This does not decide how genuinely optional arithmetic/matrix features should be
selected later. See the [Vulkan promotion requirement](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_16bit_storage.html).

Storage is not arithmetic. The compiler audit (shaderc 2026.1 / glslang 16.4)
emits `StorageBuffer16BitAccess`, an aligned-2 half load, then `OpFConvert` to
FP32; it does not emit `Float16`. This is the documented
[storage-only conversion pattern](https://docs.vulkan.org/samples/latest/samples/performance/16bit_storage_input_output/README.html).
Enable only `storageBuffer16BitAccess`; keep `shaderFloat16`, uniform/push/I/O
16-bit storage and other numeric/matrix features disabled. Report the enabled
storage bit in the existing capability query. Version the changed baseline as
ABI 10; no new creation function, tensor API or reflection mechanism.

## Numerical contract and independent reference

Only the two matrix weights become IEEE binary16, rounded once using pinned
GGML's FP32-to-FP16 conversion. Biases, activations, shared tiles, products,
accumulation and outputs remain FP32. Matrix source A may be F16 or F32; source
B and output must be F32. ADD/ReLU remain F32-only. No hidden conversions or
CPU fallback during GPU execution. Native F16 weight payloads must be half the
F32 matrix payload, with no FP32 weight mirror on the GPU.

Pinned GGML CPU's F16 matrix path selects an F16 dot-product input type and
rounds the FP32 activation operand to F16 (`src/ggml-cpu/ggml-cpu.c`,
`type_traits_cpu` and `ggml_compute_forward_mul_mat`). It is therefore **not** the
oracle for FP16 weights with unrounded FP32 activations. The semantic reference
is a separate GGML CPU F32 graph whose weights were rounded to F16 and then
widened exactly back to F32. A second, original-fixture CPU graph measures the
effect of weight rounding. Keep the existing OGPU F32 runs as regression controls.
Neither reference executes on the GPU, and neither changes upstream code.

Declare these acceptance limits before running the mixed GPU workload:

- Against the rounded/widened CPU reference: finite logits, per-element absolute
  error <= `1e-4 + 1e-4 * abs(reference)` and identical top-1 predictions.
- Against the original FP32 model: finite logits, absolute drift <=
  `0.05 + 0.002 * abs(reference)`; at most 10 changed predictions out of 10,000
  and at most 10 fewer correct predictions. Report all three measurements, not
  just dataset accuracy. These are fixture acceptance limits, not universal
  numerical guarantees or a performance claim.
- Retain the original >=90% accuracy floor, full test set, batch sizes 1/17/64,
  direct/scheduled execution, padded tails, five dispatches per graph, allocator
  reuse, no fallback and stable steady-state transfer counts. Test HOST and DEVICE
  placement on llvmpipe and physical RADV with validation.
- Add a small deterministic matrix check with odd K/M/N, positive/negative values,
  zero and small finite normal half values, and activations not representable
  in F16. Compare to a double-accumulated host reference using widened weights,
  tolerance `1e-4 + 1e-4 * abs(reference)`. Check output poisoning and rejection
  without dispatch for unsupported F16 activations, outputs and elementwise ops.
  Full NaN/Inf/subnormal behavior remains outside this workload contract.
- Inspect the actual mixed SPIR-V for storage-only half usage; baseline creation
  and capability tests must confirm that half arithmetic remains disabled.

If a gate fails, report it and investigate; do not loosen these limits to fit a
GPU result. Completion means the contracts, implementation, reproducible commands
and paired-driver evidence agree, followed by a retain/revise discussion. It does
not automatically select another numeric type or a larger model.

## Implementation and acceptance — 2026-09-14

Complete. The scope and numerical gates were committed before mixed GPU results
at `53403d2`; baseline implementation is `6ca1d19`, consumer implementation
`5900679`. No numerical limit was changed after observing results.

All **24 mixed cases and 24 original-F32 controls** pass: each precision runs
the full dataset at batches 1/17/64, direct/scheduled, HOST/DEVICE, on llvmpipe
and physical RX 5700 XT / RADV. Every case reports 9,801 correct predictions.
Mixed results match all semantic-reference top-1 predictions and change **zero**
predictions from the original FP32 model. Across mixed cases, maximum logit error
against the rounded/widened GGML CPU reference is `2.67028809e-5`; maximum drift
from the original model is `0.00321006775`. Original F32 controls retain maximum
reference error `3.43322754e-5`. The two error figures use different weight sets;
their ordering does not mean half storage improves arithmetic accuracy.

The two matrix payloads shrink from 1,588,000 to 794,000 bytes. Bias payload stays
2,040 bytes. Model-upload counters verify these exact totals, and steady-state
checks retain resident weights/intermediates, reusable staging, five dispatches
per graph and three scheduled intermediate aliases. There is no GPU-side widened
weight allocation. Timings remain diagnostics, not a performance gate or speedup
claim.

The 13×11×3 diagnostic passes with maximum error `1.31609067e-6`; the 1×1×1
activation-precision sentinel is exact against its double reference. Both pass
three repeats and bit-exact two-byte-aligned weight transfers under both placements
on both drivers. F16 activations/outputs/elementwise operations and an in-range
odd weight address reject without dispatch or output changes. The pre-existing
graph rejection tests still ensure that a late unsupported operation causes no
partial execution. Lifecycle checks include repeated creation/destruction, failed
initialization and live-child death tests.

ABI 10 also passes 27 ordinary tests, 745 C/Rust ABI layout checks, mock-loader
and binding-reproduction checks, Clippy, all 20 runtime GPU tests and all eight C
execution examples on both drivers. Device-creation interception checks the actual
feature chain: buffer16 enabled; half arithmetic and other 16-bit storage disabled.
Shader validation checks the mixed SPIR-V's exact capability set and absence of
relaxed precision. Libplacebo still matches its same-driver reference for all
286,488 intermediate/final bytes on each driver. Validation/synchronization
validation reported no errors. These are local checks, not a remote-CI claim.

Reproduce using the [consumer commands](../integrations/ggml/README.md). Local
acceptance logs below live under ignored `target/ggml-integration/`:

| Driver / placement | Mixed log | Original F32 control log |
|---|---|---|
| llvmpipe HOST | `acceptance.HtTt0MB4.log` | `acceptance.vetSbqoX.log` |
| llvmpipe DEVICE | `acceptance.ktTHwrbb.log` | `acceptance.LrHbkFVQ.log` |
| RADV HOST | `acceptance.IClIuERe.log` | `acceptance.9xUe3HCU.log` |
| RADV DEVICE | `acceptance.j0U70Xtl.log` | `acceptance.nhQxGbrB.log` |

Additional odd-address checks on RADV are in
`storage16-radv-alignment-{host,device}.log`. Libplacebo comparison logs are
`target/libplacebo-integration/storage16-{llvmpipe,radv}.log`; runtime/C-example
logs are `target/storage16-*`. Logs are local artifacts; commands and fixture
provenance, not those filenames, are the reproducibility contract.

All four conversions produced the same derivative SHA-256 values:

```text
74b76688dc7388b9cee7147ca30c1b412b9e72377fae0a3baaf7196d93d4a4b5  half.gguf
7c41d857d2212993082644fcc11e45dd2433cb6e0ae219db411928eecff46b4c  widened.gguf
```

## Retain/revise discussion

Retain native 16-bit buffer storage in the baseline. It expresses this consumer's
data layout directly and adds neither a fallback path nor a genuinely new
hardware requirement beyond Vulkan 1.4. No better optional-creation API alternative
emerged from this workload; inventing one now would not exercise a real distinction.
Storage type, activation precision and accumulation precision must remain separate
in executable contracts: the GGML CPU audit demonstrates why a generic "FP16"
label is insufficient.

This checkpoint establishes narrow storage and conversion through the existing
memory/shader boundary, not native FP16 arithmetic, matrix acceleration or modern
ML coverage generally. Keep those questions open, along with sustained asynchronous
execution and backend/hardware portability. The API remains experimental; completing
this gate does not authorize the next expansion or make stabilization imminent.
