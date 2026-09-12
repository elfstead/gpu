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

C1 is complete. C2 retains the API with these explicit decisions.
C3 implementation and verification are in progress; no acceptance result yet.
