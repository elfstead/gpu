# M5 bounded transformer consumer

Completed with the separate quantized-linear profile; [M5 acceptance and cost observations](m5-results.md)
own the final result. The initial contract and development checkpoint below are
retained as chronology, not open work.

Selected 2026-10-13, before implementation. Use the pinned GGML checkout already
used by MNIST. The application owns its graph, tensor placement, precision choice
and synchronization. No tensor operation or graph scheduler enters OGPU's C API.

## Workload

One bias-free, single-head, bidirectional pre-RMSNorm transformer block:

```
N = RMSNorm(X, epsilon=1e-5)
Q = Wq N; K = Wk N; V = Wv N
P = row_softmax(K^T Q / sqrt(D))
A = V P
R = X + Wo A
Y = R + Wdown ReLU(Wup RMSNorm(R, epsilon=1e-5))
```

GGML's first dimension is contiguous. X/Q/K/V/R/Y have shape [D,T], P is
[T,T], and the feed-forward hidden result is [H,T]. Attention rows are queries;
softmax reduces over keys. Materialize a transposed V for the existing tiled
matrix kernel; this is an explicit consumer strategy, not an API-required copy.
No causal mask, positional encoding, normalization gain, KV cache, dropout,
batch/head dimension or training is claimed.

Initial (D,H,T) cases: (1,1,1), (13,29,3), (13,29,17), (64,128,16),
(65,129,17). Deterministic bounded inputs/weights require no model download.
Execute A/B/A input changes without re-uploading unchanged weights. Keep every
intermediate on the GPU until the whole graph completes; diagnostic readbacks
afterward check each intermediate, not only the final tensor.

The existing MNIST profile remains the default. An explicit foundation-only
transformer profile adds full-shape residual ADD, RMS_NORM, unmasked SOFT_MAX
with finite scale and zero max-bias, and a bounded 2D transpose/materialization
pair. Reject other views, masks, sinks, layouts, parameter values and aliases
before any dispatch. Preserve the original MNIST/Metal profile unchanged.

## Executable and numerical contract

Use 64-invocation workgroups and ordinary FP32 instructions. Matrix kernels
retain their 512-byte workgroup storage; row reductions use 256 bytes. No subgroup
width, subgroup operation, Float16 arithmetic, integer-dot acceleration or matrix
hardware is assumed. Query/check local/group/shared/argument limits. FP16 weight
storage uses the baseline's 16-bit storage access with FP32 products/accumulation;
the caller explicitly chooses F32 or F16 weights. Activations remain F32.

Compute an independent FP64 reference for every intermediate from the actual
stored F32/F16 inputs. Require finite outputs and absolute error at most
`2e-4 + 2e-4 * abs(reference)` for this bounded block. This allows FP32 reduction
and exp/inverse-square-root rounding, not arbitrary model-level agreement.
Softmax probabilities must be nonnegative and each row sum within 2e-5 of one.
Transpose copies and repeated A outputs must be bit-exact. Report maximum error
per intermediate and F16-vs-F32-weight drift separately; F16 is never selected
automatically. Zero-input/singleton cases must remain finite.

These are finite bounded inference fixtures, not NaN/Inf/subnormal conformance or
a general stability theorem. Thresholds are fixed before GPU execution.

## Acceptance and follow-on

Require exactly 15 GPU dispatches per block, one synchronous graph submission,
no CPU operator fallback or intermediate transfer between nodes. Check HOST and
DEVICE placement, unchanged input/weight bytes, whole-graph rejection without
output mutation, and successful retry. Retain MNIST matrix/lifecycle regression.
Record diagnostic memory/transfer counts and separate any later timing runs
from validation. The baseline materialization and separate kernels do not imply
a fundamental API cost; fusion, tiling and reuse remain caller choices.

After the block works, add the roadmap's separate signed-INT8-weight linear
variant with explicitly specified scales/layout/error policy, then integrate
the broader consumer into the independent SDK. Throughput/memory/error tradeoffs
and independent handoff remain M5 gates; the first block result alone does not
complete M5. Hardware matrix acceleration and Metal acceptance remain separate.

## First implementation checkpoint

The opt-in `OgpuGgmlProfile::Transformer` now executes the entire block through
the foundation, retaining the default MNIST profile and rejecting transformer
selection in legacy builds. The new workgroup shader adds row RMSNorm/softmax,
residual addition and explicit transpose materialization. Existing tiled F32/F16
matrix kernels and ReLU remain unchanged. No runtime or public C API change.

`bash integrations/ggml/run-transformer.sh target/ggml-source` builds the selected
public runtime and consumer, checks SPIR-V capabilities/root offsets/workgroup
size, then runs both placements without model downloads. All five shapes in both
precision modes pass on Radeon and llvmpipe in development: every intermediate,
probability row, exact transpose, A/B/A output, unchanged input/weight bytes and
bounded staging. Eleven invalid-parameter/layout/alias cases per shape/precision
reject before dispatch, preserve every intermediate and permit a valid retry.
There are 15 dispatches per valid block, no uploads/downloads or staging growth
inside a graph. F16-vs-F32 final-output drift is reported, not an acceptance
threshold or permission to choose reduced precision automatically.

The SDK now includes this consumer in its default HOST/DEVICE GGML checks. Its
receipt parser requires all 150 intermediate error summaries, all ten case results,
the five precision-drift records and a complete marker per placement; host tests
reject missing/duplicated/malformed results. Clean independent-package acceptance
is still pending at this development checkpoint. No timing, INT8, matrix hardware
or Metal result follows from it.
