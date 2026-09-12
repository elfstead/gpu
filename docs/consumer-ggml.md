# First consumer: GGML MNIST forward inference

## Brief and acceptance (C1)

Use [GGML](https://github.com/ggml-org/ggml) revision
`7840aaba1989c6deeefede1d77d5aaf8f52b947e`, specifically its unmodified
`examples/mnist/mnist-common.cpp` fully connected model builder. GGML is MIT
licensed. The adapter uses its backend extension interface, including GGML's
internal backend header: this is pinned, not a stable GGML ABI. On our side it
uses only `include/ogpu.h` and the shared library.

The useful result is handwritten-digit classification with the upstream
784 → 500 → 10 FP32 network, trained weights, and actual MNIST test images
normalized by dividing each byte by 255. Training is fixture preparation on the
upstream CPU backend, not part of the GPU integration. Exact reproduction and
dataset/model provenance accompany the implementation.

Acceptance, stated before GPU results:

- Execute two matrix multiplies, two broadcast bias adds, and ReLU through OGPU.
  No CPU execution or scheduler fallback inside the forward graph.
- Compare every logit to GGML CPU: `abs(gpu-cpu) <= 1e-4 + 1e-4*abs(cpu)`;
  require finite values, identical top-1 predictions, and at least 90% accuracy
  on the full 10,000-image test set. Accuracy checks fixture usefulness, not
  numerical precision. CPU metrics and normalization are allowed.
- Exercise batch sizes 1, 17, and 64, including tails, repeated execution with
  weights and graph allocations reused, and full teardown/recreation.
- Reject unsupported operations, FP16, and unsupported layouts explicitly.
  Reject an unsupported graph before recording/submitting any of it.
- Independently build the consumer, validate shaders, and run with Vulkan and
  synchronization validation on available hardware and software drivers.
  No performance gate.

Excluded: CNNs, training through OGPU, quantization, arbitrary GGML graphs,
asynchronous scheduling, upstream contributions, and graphics-consumer evidence.

## Candidate decisions (C2)

These retain/defer decisions do not add a tensor API.

| Decision | First-consumer contract |
|---|---|
| D0 | GGML revision/workflow and acceptance above |
| D1 | Retain trusted Vulkan 1.2 SPIR-V: `main`, descriptor-free, core-required features plus buffer device address. Successful compute-device creation guarantees this enabled baseline. Probe numeric flags only report support, not enabled optional arithmetic. Consumer metadata defines root layout, local size and dispatch math. Offline validation of checked-in modules is required; runtime header checks are not reflection or validation. No optional executable variants needed. |
| D2 | Retain dedicated host-visible allocations and explicit blocking CPU copies. GGML suballocates tensors in those buffers. GPU addresses are stable until destruction; GGML data tokens are not host mappings. All graph/weight/input allocations outlive the completion wait. Device-local/staging policy remains deferred. |
| D3 | Retain the optional fixed offscreen RGBA8 profile, unchanged and unused here. Sampling/storage images remain deferred. |
| D4 | One externally serialized device, synchronous GGML graph callback, one one-shot batch per graph, explicit compute read/write dependencies between nodes and across repeated calls. Reuse kernels and tensor allocations, not batches/completions. Drain before return, free or CPU copies. Advertise no events or async capability. |
| D5 | Pin consumer and OGPU source revisions. ABI version 1 detects layout/signature mismatch at probe creation, but does not promise later additive symbols in arbitrary ABI-1 builds. Use matching header/library/shaders from one checkout. Shader contract is the fixed profile in `ogpu.h` plus adapter root declarations, not a portable package. Breaking changes update contracts, consumers and tests; existing C layout/signature changes increment ABI version. No release/tag or stable cross-version promise. |

Unsupported GGML graphs return failure. Buffer callbacks without error returns
must fail loudly on transfer errors. Incompatible trusted SPIR-V is a caller
contract violation, not a guaranteed clean `UNSUPPORTED` result. Runtime graph
errors must not trigger a hidden fallback.

## Working status

C1–C3 are complete. C2 retained the API in the initial checkpoint. The experimental C4 source checkpoint is
`375f33398d88b85db2ed6dd244d58e519850e328` (ABI 1), with the clarified compatibility
policy above. This is a source checkpoint, not a release/tag or a stable ABI.
See [build instructions and the integration friction report](../integrations/ggml/README.md).
The active [hardening and scheduler follow-up](ggml-hardening.md) evaluates better
API alternatives, not just compatibility with that checkpoint.

## Acceptance record — 2026-09-12

The saved FP32 model used for the paired-driver runs has SHA-256
`da1c16099ee705ac4445cf460cd89212acf30eca166db47225f5d38935b097ea`.
It was prepared with the pinned upstream CPU `mnist-train`, 30 epochs, default
random initialization, 57,000 training and 3,000 validation images. No test images
were used for training. Originally saved only in `target/`, these exact weights
are now [checked in and checksum-pinned](../integrations/ggml/fixtures/README.md).
Routine regression no longer trains fresh weights; the historical fresh-training
result below records the original preparation procedure.

Both the RX 5700 XT (RADV NAVI10) and llvmpipe (LLVM 21.1.8, 256 bits) passed with
Vulkan/synchronization validation. CPU reference: Ryzen 9 5900X, GGML CPU, four
threads. All rows below passed on both drivers with identical recorded results.

| Batch size | Images | Graph calls | GPU dispatches | Correct predictions | Maximum absolute logit difference |
|---|---|---|---|---|---|
| 1 | 10,000 | 10,000 | 50,000 | 9,801 (98.01%) | 0.0000343322754 |
| 17 | 10,000 | 589 | 2,945 | 9,801 (98.01%) | 0.0000343322754 |
| 64 | 10,000 | 157 | 785 | 9,801 (98.01%) | 0.0000343322754 |

Every logit, including padded tail rows, met the predeclared tolerance; every real
image's top-1 prediction matched CPU. Outputs were poisoned before calls. Weights,
input and intermediate allocations were reused across calls, then models, buffers,
kernels and devices were destroyed/recreated for the next batch size. Rejection
tests verified unsupported operations, FP16, strided layouts and aliased matrix
outputs fail without submitting earlier graph nodes or changing a sentinel output.
No host Vulkan calls, private Rust access, or GPU-graph CPU fallback was used.

Toolchain: Rust 1.97.1, Clang 21.1.8, CMake 4.3.4, glslang 16.4.0,
SPIRV-Tools/loader/validation layers 1.4.357.0. Release consumer checks remain
active under `NDEBUG`. ShellCheck, Rust formatting/Clippy, 20 ordinary tests, all
seven GPU tests across both drivers, 550 ABI layout checks and loader mocks passed.
CI includes a consumer job; no remote CI run is claimed.

A fresh local clone of the source checkpoint also completed the documented
download → checksum → CPU training → build → llvmpipe acceptance procedure.
Its independently prepared model hash was
`0f65ae33a84d59abf328cf00fb517d513d15a6a7d1b9e56953663ca150f3f59c`;
all three batch sizes produced 9,792/10,000 correct predictions, identical CPU/GPU
top-1 results, and maximum logit difference 0.0000305175781. This confirms the
procedure does not depend on the original checkout's generated artifacts.
Missing-shader and invalid-device consumer startup checks also exited with clear
errors, without the process-exit cleanup fault.

An exploratory sanitizer build completed inference without an AddressSanitizer
error (leak detection disabled), but UBSan reported two diagnostics in pinned
upstream GGML: null-pointer offset calculation in graph sizing and an indirect
CPU-kernel function-type mismatch. This is **not** a sanitizer-clean claim for the
dependency or the entire stack, and does not replace the validated release runs.

The result supports this small forward-inference integration. It does not settle
device-local transfers, scheduler-driven allocation reuse, asynchronous callbacks,
accelerated numeric profiles, graphics-consumer usability, or performance against
existing GGML GPU backends. These are future scope choices, not unfinished gates
for this checkpoint.
