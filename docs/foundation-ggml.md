# GGML on the explicit foundation

Migration checkpoint, 2026-10-11. The pinned MNIST consumer now selects the
implemented foundation contract by default on Linux. No tensor operators, graph
ownership or GGML-specific policy were added to the public runtime.

## Boundary

`integrations/ggml/backend.cpp` still owns the same graph preflight, supported
tensor profile, allocator alias checks, CPU token ranges, registration and
transfer accounting. A build-time-selected private consumer helper now supplies
the small synchronous GPU workflow. `runtime-foundation.h` includes only
`ogpu_next.h`; `runtime-installed.h` includes only `ogpu.h`. They do not call
each other, use native Vulkan or select a runtime fallback. The latter remains a
migration control and the existing Metal path, not a second implementation of
the GGML graph translator. Native Metal revalidation has not been performed.

The foundation helper explicitly owns one selected compute queue, one timeline,
one reserved command arena and submission scratch. Tensor allocations choose a
compatible LOCAL or HOST_VISIBLE memory type from public requirements and queries.
HOST tensors and the grow-to-fit transfer buffer remain persistently mapped;
flush/invalidate is explicit. LOCAL does not imply that memory is unmappable or
physically discrete VRAM. No tensor mirror, automatic placement or host mapping
is advertised to GGML.

Each synchronous graph callback records one one-shot list after complete preflight.
Each DEVICE copy callback records one copy list. Both wait on caller-owned timeline
values, then reset the arena to discard borrowed references before GGML can free
allocations. The next successful callback needs no extra reset or command-pool
creation. A local recording error can reset at the next begin. Submit/wait failure
aborts the consumer rather than unwinding resources whose completion is uncertain;
transfer callbacks still cannot return an error through GGML.

Graph barriers cover compute/copy read/write hazards, including the previous
submission and GGML's legal dead-intermediate aliases. HOST read visibility is
recorded before completion; invalidation precedes CPU copies. There are no host
waits between graph nodes. Prepared executables retain the original 40-byte inline
root, 64 invocations and 512-byte matrix shared-memory requirement; setup checks
the profile's maximum dispatch/argument/local/shared limits. The frozen SPIR-V,
FP16-storage/FP32-arithmetic contract, model, data and tolerances are unchanged.

This intentionally retains GGML's synchronous callback policy and bounded graph
profile. It does not promise competitive GEMM, asynchronous scheduling, cached graph
lists, arbitrary tensors or a native-Vulkan performance comparison. The public
foundation permits other policies; this adapter does not constrain its API.

## Reproduce

Use the prerequisites and pinned upstream revision in
[the consumer README](../integrations/ggml/README.md). No download or training occurs
inside the acceptance runner:

```sh
bash integrations/ggml/run.sh target/ggml-source 0 device f16 foundation
bash integrations/ggml/run.sh target/ggml-source 0 host f32 foundation
bash integrations/ggml/run.sh target/ggml-source 0 host f32 installed
```

The fifth argument explicitly selects the API. Separate build directories avoid
cached CMake choices switching a run. A standalone CMake build can select
`-DOGPU_GGML_FOUNDATION=ON/OFF`; ON is Linux-only. Both paths build the same graph,
matrix and lifetime tests. Use the matching header/library/shaders from the exact
source commit; foundation revision 1 is not a cross-commit compatibility promise.

## Acceptance and remaining work

Foundation runs pass all four HOST/DEVICE × F32/F16 combinations on Radeon/RADV,
plus HOST/F32 and DEVICE/F16 on llvmpipe. The installed HOST/F32 migration control
also passes on Radeon. Each run checks
all 10,000 test images in six configurations: direct/scheduled at batch sizes
1, 17 and 64. All use synchronization validation. The scheduled graph remains one
OGPU split with exactly five dispatches per call; no CPU fallback is accepted.

Both two-byte-aligned matrix cases, unchanged-weight transfer checks, unsupported
graph/type/layout/alias rejection before execution, failed-init cleanup, repeated
registration and live-child destruction rejection pass. Steady-state DEVICE staging
does not reallocate; uploads/downloads remain exactly the declared input/poison/
logit operations. FP16 runs retain 9,801/10,000 correct predictions, no prediction
changes against the original model and maximum FP32-model drift `0.00321006775`.
These are local observations, not new thresholds or cross-driver guarantees.

A focused consumer test rejects overlapping and out-of-range copy recordings
without advancing the timeline, then successfully records/submits again and checks
unchanged destination bytes. A two-byte copy changes exactly its selected bytes.
It tests the adapter's local-error recovery and arena retirement, not fault-injected
device loss; fatal submit/wait failure remains deliberate consumer policy.

The default SDK and libplacebo adapter have since moved to foundation. Remaining
legacy workflows are tracked separately. The installed helper can be removed when its migration
control is no longer needed and Metal has a validated foundation implementation.
This checkpoint does not retire ABI 20 or claim native Metal parity.

## Independent source handoff

The default installer also exports this adapter and its tests as
`share/ogpu/examples/foundation-ggml`, with supplied SPIR-V and frozen model.
`build.py --ggml-source ...` gets the public header/library from `ogpu-foundation.pc`
and builds only C/C++ consumer/upstream code. CMake's `OGPU_INCLUDE_DIR` and
`OGPU_LIBRARY` parameters make this boundary explicit; the in-tree runner pins both
to its freshly built checkout, while the copied wrapper pins them to the SDK.

The standalone runner checks HOST/DEVICE matrix, lifecycle and the new bounded
[transformer profile](transformer-consumer-plan.md) by default;
`--data-dir` explicitly adds both full-dataset precision modes. It validates input
digests, preserves partial logs and complete/incomplete receipts, and records
SDK/header/library, executable and source hashes. It downloads/trains nothing and
does not require Cargo, Slang, Vulkan headers or OGPU checkout fixtures. The pinned
upstream checkout and optional MNIST data remain explicit caller prerequisites.

Local relocated-SDK acceptance passes with a separate local clone of upstream
GGML: Radeon runs both full-dataset precision modes; llvmpipe runs the default
HOST/DEVICE matrix/lifetime handoff. Compiler dependencies include the relocated
foundation header and no OGPU checkout/native/legacy headers. Actual dynamic
resolution selects the relocated library. Missing-loader failure leaves an
incomplete report with no accepted runs. The Linux import uses `-L/-l` for the
Rust library without SONAME, rather than an absolute `DT_NEEDED` filename.

A no-GPU hosted CI lane builds the same exported source graph and checks failure
handling without shader tools or MNIST downloads. It is configured, not remotely
executed by this checkpoint. Local package builds used available dependencies and
driver/tool caches; these results do not establish clean-machine portability.

## M5 profile expansion

The explicit foundation-only transformer selection adds full-shape residual ADD,
RMSNorm, finite-scale unmasked softmax and a tightly checked 2D transpose/CONT pair.
Metadata-only transpose borrows exactly its source storage; CONT materializes it
with an ordinary shader. Reductions reject input/output aliasing. Graph preflight
rejects unsupported masks/sinks, nonfinite parameters, layouts and ownership before
recording any node. Ordinary operations retain the original MNIST behavior.

All ten shape/precision cases pass in HOST/DEVICE placement on both local drivers
in the [clean M5 acceptance](m5-results.md), with every intermediate independently checked and no transfer
between nodes. The block uses no subgroup operation or optional arithmetic feature;
only baseline FP32 and storage-only FP16 weights. The new SPIR-V gate checks the
40-byte root, 64-invocation local size, baseline capabilities and absence of reduced
precision. The separate explicit QuantizedLinear profile adds Q8_0 weights with
FP32 accumulation and five linear shapes, including two-byte alignment, guards,
representation-error bounds and ten rejection/retry controls. It does not change
default MNIST behavior or automatically quantize the transformer. Both profiles
now pass clean SDK acceptance on both drivers; separate Radeon host-wall observations
record the bounded cost/storage/error tradeoffs. This completes M5, not a general
LLM backend, tuned performance, hardware matrix acceleration or native Metal parity.
