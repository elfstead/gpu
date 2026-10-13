# M5 acceptance: bounded transformer and quantized linear

Completed 2026-10-13. Contracts were fixed before execution in the
[transformer brief](transformer-consumer-plan.md) and
[quantized-linear brief](quantized-linear-plan.md). No numerical threshold changed.

## Correctness and independent consumption

Clean `de697109093e3b17153c5aa639ec4dad4c2b8fab` SDK acceptance passes on Radeon
RX 5700 XT/RADV 26.2.4 and llvmpipe 26.2.1. Each runs both HOST and DEVICE placement:

- Transformer: five shapes × F32/F16 weights, A/B/A, all 15 intermediate FP64
  oracles, probability sums, exact transpose/repeat, unchanged inputs/weights,
  bounded staging, 11 invalid-graph controls and a successful retry per case.
- Linear: five shapes × F32/F16/Q8_0 weights, A/B/A, independent dequantized FP64
  and representation-error bounds, two-byte weights, allocation guards, unchanged
  bytes, bounded staging, ten rejection controls and a successful retry per case.
- Original mixed-matrix/lifecycle/recovery cases and all six small SDK consumers,
  including learned-image, indexed/range replay and textured graphics.
- Radeon additionally executes both full-dataset MNIST precision modes, all six
  direct/scheduled batch configurations per mode. No CPU operator fallback in the
  M5 graphs; diagnostic intermediate readbacks happen only after whole-graph completion.

The SDK is relocated into a space-containing directory with a separate local
clone of pinned upstream GGML. Compiler dependencies and loaded runtime identify
the installed public boundary, not OGPU implementation headers. Missing-loader
failure leaves an incomplete report. This is our independent-build test, not an
external adopter or a clean-machine claim. Validation and synchronization validation
are enabled for correctness, not for the observations below.

Local full receipts:

| Driver | SDK test log | SHA-256 |
|---|---|---|
| Radeon | `/tmp/ogpu-m5-sdk-radv.log` | `5c411fa304c5bf30768ed07a8a4b6d2ddc6f1d96822625bbf5e89320ebfee0ef` |
| llvmpipe | `/tmp/ogpu-m5-sdk-lvp.log` | `80c5351358b7d0459b532cf4992c89d70cd4808291bc2a802a2b58a97239e705` |

The subsequent measurement runner/device identity commit
`5c2f8997951846ae26dbbfc01f1df35c06c03a2a` also passes the complete installed GGML
HOST/DEVICE acceptance on both drivers at `/tmp/ogpu-m5-measured-sdk`. Reports are
`share/ogpu/examples/foundation-ggml/run-f2l4djxx/report.json` (Radeon) and
`run-k3zdmkvu/report.json` (llvmpipe). Both packages record clean source commits.

## Bounded cost and accuracy observations

The clean `5c2f899` installed consumer runs on Radeon, without concurrent regression
jobs or validation. Two warmups precede nine samples of ten synchronous graphs
per case. All 50 shape/precision/placement combinations pass checks and report raw
samples. Each interval includes GGML preflight, command recording, submit, wait
and arena reset; it excludes tensor transfer/readback. These are **host wall times**,
not GPU timestamps, isolated kernel throughput or a direct-Vulkan comparison.

Representative larger cases; time is median [min,max] in milliseconds per graph:

| Workload / precision | HOST ms | DEVICE ms | DEVICE items/s | Weight bytes | Tensor allocation bytes |
|---|---|---|---:|---:|---:|
| Transformer D65 H129 T17, F32 | .1862 [.1647,.2289] | .1400 [.1252,.1731] | 121,461 | 134,680 | 208,896 |
| Same, F16 weights | .1755 [.1594,.2195] | .1401 [.1274,.1736] | 121,324 | 67,340 | 141,568 |
| Linear K256 M128 N64, F32 | .0952 [.0921,.1048] | .0755 [.0730,.0780] | 847,320 | 131,072 | 229,700 |
| Same, F16 weights | .0958 [.0914,.1012] | .0735 [.0718,.0767] | 870,994 | 65,536 | 164,162 |
| Same, Q8_0 weights | .0974 [.0915,.1046] | .0790 [.0752,.0834] | 810,206 | 34,816 | 133,442 |

Items mean input token vectors for this bidirectional block, or activation vectors
for linear—not autoregressive generation tokens/s. Tensor allocations include
fixture padding/guards but exclude staging, CPU token storage and runtime objects.
HOST/DEVICE name requested memory properties, not universally distinct physical heaps.
F16 halves weight payload; Q8_0 uses 26.5625% of F32 payload including scales.

Maximum observed transformer intermediate error is `0.000146000206`; maximum
final-output F16-vs-F32 drift over the five shapes is `0.00099158287`. For the
largest linear case, F32/F16/Q8_0 arithmetic errors are respectively
`8.44735066e-6`, `6.31309155e-6`, `5.96722997e-6`; drift from original F32 weights
is `8.44735066e-6`, `0.00417685928`, `0.0826777446`. All shapes satisfy both frozen
bounds; maximum arithmetic-error/bound ratio over all cases is `0.00285914546`.
These are absolute errors on deterministic fixtures, not a model-quality guarantee.

Samples trend downward during warmup/clock ramping despite the specified warmups.
For example, DEVICE transformer F32 samples are
`.1731335,.1633056,.1520626,.1453267,.1399628,.1344878,.1316558,.1286579,.1251619` ms.
Variant order is fixed, clocks are not locked and the workload is small. Do not
infer a significant F16 speedup, a universal placement ranking or an inherent Q8
penalty. This observation establishes storage/accuracy/cost tradeoffs for the
implemented kernels, not tuned GEMM/attention performance. No speedup gate was set.

Local measurement receipt:
`/tmp/ogpu-m5-measured-sdk/share/ogpu/examples/foundation-ggml/measure-1oh699hr/report.json`,
SHA-256 `71ba79af2e5d262c97e5c9146e2d82eeace85308e2e698ca5969784908f019df`.
It retains all samples and SDK/runtime/header/source/executable/device identities.
Local paths are evidence references, not redistributed artifacts.

## Decision and remaining scope

M5 is complete for the declared single-head, finite bounded inference block and
separate quantized linear profile. Retain explicit executable requirements and
caller-selected precision/storage. Both consumers fit the existing public API;
no tensor host API, precision auto-selection, CPU fallback or graph scheduler is
added. The synchronous callback, separate kernels and V transpose materialization
are this adapter's choices, not OGPU requirements. Fusion, asynchronous scheduling,
GPU roots and more efficient tiling remain available implementation alternatives.

This does not complete a general GGML/LLM backend, causal/KV-cached attention,
training, competitive kernels, matrix acceleration, universal Vulkan equivalence
or Metal foundation support. Complete the [foundation-contract consolidation](core-contract-review.md),
then start M6 as a separate native-backend migration/validation task.

Reproduce from an exact matching SDK and pinned GGML checkout using the exported
`build.py`, `run.py` and `measure.py` instructions. In-tree correctness is
`bash integrations/ggml/run-transformer.sh target/ggml-source`; shader checks and
receipt-parser negative tests are configured in CI, not claimed remotely executed.
