# M5 signed-INT8 linear variant

Selected 2026-10-13 before implementation. This is a separate GGML linear consumer,
not automatic quantization of the transformer or a new public tensor API.

Use pinned GGML `Q8_0`: contiguous rows, K divisible by 32, each block exactly
34 bytes (little-endian FP16 scale followed by 32 signed INT8 values). Decode
weight `float(scale) * float(q)`; activation, product and accumulation are FP32.
No zero point, activation quantization, Int8 arithmetic feature, integer dot
product or matrix acceleration is needed. Packed 16-bit storage loads access
the existing two-byte-aligned blocks; sign extension uses ordinary 32-bit integer
instructions. Scales/inputs must be finite; device-resident data validity remains
the caller's responsibility, not a hidden CPU readback.

An explicit foundation-only QuantizedLinear consumer profile accepts F32/F16/Q8_0
left-hand matrix weights and F32 activation/output. It does not broaden the default
MNIST profile or enable quantization based on a capability flag. Other quantized
types, non-contiguous/strided layouts, views, in-place matrices and half activations
are rejected during whole-graph preflight. Q8_0 K must be a multiple of 32, 1..1024
for M/N and 32..1024 for K. This layout restriction belongs to Q8_0/the adapter;
the public GPU API imposes no tensor block size.

## Frozen fixtures and numerical gates

Shapes (K,M,N): (32,1,1), (32,11,3), (64,13,17), (96,17,5), (256,128,64).
Both HOST and DEVICE placements, F32/F16/Q8_0 weights, A/B/A activations; no weight
upload between graph executions. Include zero-scale blocks, positive/negative
values, cancellation, all signed-byte extremes including -128, and weights placed
at offset two in their backing allocation. Block bytes and allocation guards must
remain unchanged. Compare every output to an independent FP64 dequantized oracle:

`abs(actual - reference) <= 1e-5 + 8*K*FLT_EPSILON*sum(abs(dequantized_weight*x))`.

Compare each variant to the unquantized F32-weight reference separately. The
allowed drift is the actual representation bound
`sum(abs(stored_weight-original_weight)*abs(x))` plus the arithmetic bound above.
Report observed error and drift; the representation bound does not promise model
accuracy. Host fixture quantization uses rounded FP16 `max(abs(w))/127` scales,
nearest integer with halfway cases away from zero, clamped to [-127,127]. A named
sentinel block separately exercises -128 and 127. Do not loosen bounds after runs.

Reject invalid graph/type/layout/alignment/alias cases before dispatch, preserve
outputs, and retry successfully. Exactly one dispatch per linear graph, no graph
transfer/fallback/staging growth. The SDK must carry its shader/test and verify
all shape/precision/placement results, including incomplete failures.

## M5 cost observations

After correctness, use separately identified non-validation Radeon runs for both
the transformer and linear consumers: two warmups, nine samples of ten synchronous
graph calls, no intermediate readback during timing. Report minimum/median/maximum
host wall time per graph and corresponding items/second, allocation and weight
bytes, and numerical drift from correctness. This includes preflight/record/submit/
wait/reset, not isolated GPU time. Preserve F32/F16/Q8_0 choice; no speedup threshold,
claim of optimal kernels or general Vulkan parity. Local matched native graphics
evidence does not certify these ML kernels. Record untested hardware profiles and
remaining optimization opportunities explicitly; do not start Metal implementation
as part of this milestone.
