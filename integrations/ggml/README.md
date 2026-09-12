# GGML MNIST consumer

A pinned GGML backend adapter for the upstream FP32 fully connected MNIST forward
graph. Read the [brief and API decisions](../../docs/consumer-ggml.md) first.
This is not a general GGML backend or a replacement for `mnist-eval`'s full
loss/optimizer graph. The Rust GPU runtime and public API are unchanged.

## Reproduce

Run from the repository root on Linux x86-64. Requirements: the runtime's Rust
toolchain/loader, C/C++17 compilers, CMake 3.20+, Ninja, Git, Bash, curl, gzip,
sha256sum, ripgrep, and SPIRV-Tools. Vulkan 1.2 compute plus buffer device address
is required; neither graphics nor timestamps is required. GGML's configured CPU
reference currently requires AVX2/FMA/F16C, even though the GPU kernels use FP32.

```sh
git clone --no-checkout https://github.com/ggml-org/ggml.git target/ggml-source
git -C target/ggml-source checkout --detach 7840aaba1989c6deeefede1d77d5aaf8f52b947e
bash integrations/ggml/prepare.sh target/ggml-source
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 \
    bash integrations/ggml/run.sh target/ggml-source
```

`prepare.sh` explicitly downloads MNIST into ignored `target/ggml-data`, verifies
the [dataset checksums](dataset.sha256), builds upstream CPU tools and trains a
model **only if no saved model exists**. It does not overwrite an existing model.
Training uses the unmodified upstream 30-epoch FC example with its 95/5 train/
validation split. Random initialization/shuffling means a fresh preparation is
not bitwise reproducible. The saved weights are fixed across all acceptance runs;
the runner prints their SHA-256. Keep that file to reproduce an exact model result.
New preparations must meet the same numerical, prediction and accuracy gates;
they need not match the recorded accuracy or model hash. Training support is not
part of the OGPU adapter. Preparation logs are in `target/ggml-data/training.log`.

Source: [GGML MNIST example](https://github.com/ggml-org/ggml/tree/7840aaba1989c6deeefede1d77d5aaf8f52b947e/examples/mnist).
Data: [MNIST](https://yann.lecun.com/exdb/mnist/), by Yann LeCun, Corinna Cortes and
Christopher J. C. Burges, downloaded from the `cvdf-datasets` Google storage mirror.
Consult the dataset's own attribution/terms; this repository's MIT license does
not replace them. GGML is MIT licensed. Neither dataset nor trained weights are
redistributed in this repository.

`run.sh` performs no downloads or training. It verifies test-data checksums, builds
the release Rust library, validates the checked-in SPIR-V, independently builds
the consumer with CMake, and runs all acceptance checks. It rejects a wrong GGML
revision or tracked upstream edits. It fails on process errors or Vulkan validation
errors and leaves a uniquely named acceptance log under `target/ggml-integration`.
Use matching header/library/shaders from the same OGPU checkout, not an arbitrary
ABI-1 shared library. Leave `CARGO_TARGET_DIR` unset.

An optional second argument to `run.sh` selects the OGPU probe's device index
(default 0). To select a software ICD use the loader's `VK_DRIVER_FILES`; set
`OGPU_VULKAN_LIBRARY` and `VK_LAYER_PATH` if needed by your installation. Validation
must be installed/enabled to claim a validated run; the script cannot prove that
a requested layer was successfully activated on every loader configuration.

To regenerate shaders (glslang 16.4.0 used for this checkpoint):

```sh
for shader in integrations/ggml/shaders/*.comp; do
    glslangValidator -V --target-env vulkan1.2 "$shader" -o "$shader.spv"
    spirv-val --target-env vulkan1.2 "$shader.spv"
done
```

## Integration boundary

[backend.cpp](backend.cpp) translates GGML buffer callbacks and graph nodes into
the public C ABI. Its C++ is consumer glue for GGML's C++ application/backend
interface, not a second GPU runtime. It contains no host-side Vulkan calls and
does not include private Rust/backend headers. The runtime remains Rust.

[mnist.cpp](mnist.cpp) calls the unmodified upstream model loader and graph builder,
then sends each graph directly to a named backend. The upstream constructor prints
fallback-backend messages, but its scheduler is **never executed** by this driver.
GGML CPU runs a separate reference graph. GPU dispatch counts must be exactly five
per call; unsupported graphs fail before submission. Four parameter nodes may
also appear in GGML's graph and require no dispatch.

Supported tensor profile: contiguous, non-view FP32 1D/2D tensors with both leading
extents in 1..1024 and remaining extents equal to 1. Operations are out-of-place
`MUL_MAT` with matching inner dimensions, row-broadcast bias `ADD`, and unary ReLU.
This is exercised on finite trained weights/normalized inputs, not a complete
NaN/Inf arithmetic conformance suite. Unsupported types/layouts/ops are rejected;
out-of-range, foreign-buffer and overlapping input/output addresses also fail
graph preflight. Copy callbacks have no GGML error return, so transfer failures
abort loudly rather than return stale data.

Each graph records one batch with compute read/write dependencies before each
dispatch, including the first (previous submissions). It submits and waits once.
The graph, weights and input allocations remain live throughout. All host calls
and backend instances share one externally serialized execution device; async,
events, host mapping/import and cross-device copies are not advertised.

Root layout is 40 bytes: three 64-bit GPU addresses at 0/8/16, followed by u32
M/N/K/operation at 24/28/32/36. Both shaders use `main` and 64 invocations.
Matrix workgroups produce 8×8 tiles with 512 bytes of shared memory; GGML's layout
is A[K,M], B[K,N], C[M,N], first dimension fastest. Element workgroups process 64
values, guarding the tail. These bounds fit Vulkan's minimum core limits, so no
new optional feature or workgroup-limit query is needed for this profile.

## Deliberate costs and remaining friction

- GGML suballocation fits stable GPU addresses and checked offset copies. But its
  pointer-arithmetic ABI requires a distinct aligned host token range: this simple
  adapter allocates an extra host byte array per GPU buffer, not a data mirror.
  It also uses `alloc_ctx_tensors`, not GGML's lifetime-reusing graph allocator.
- Bias and ReLU need shader code, not host-side tensor operators. There are two
  prepared shaders, five dispatches, and deliberately conservative global barriers.
  This is correctness/integration evidence, not competitive GEMM performance.
- The upstream constructor assumes registry ordering leaves CPU last. The driver
  unregisters/re-registers the statically linked CPU backend before constructing
  models. It neither changes upstream sources nor runs a fallback scheduler.
- Explicitly unregister/free the OGPU state after models, buffers and backends,
  before process static teardown. The initial process-exit-only cleanup exhibited
  a fault with validation enabled; explicit lifecycle management eliminated it in
  repeated local runs. The underlying driver/layer teardown cause is not isolated.
- No claim yet about arbitrary schedulers, allocation aliasing/reuse, asynchronous
  callbacks, larger/quantized models, other GPU vendors, or graphics consumers.
