# GGML MNIST consumer

A pinned GGML backend adapter for the upstream fully connected MNIST forward
graph. Read the [brief and API decisions](../../docs/consumer-ggml.md) first.
This is not a general GGML backend or a replacement for `mnist-eval`'s full
loss/optimizer graph. The consumer uses only the public C API. The
[memory checkpoint](../../docs/memory-transfers.md) compares host-accessible buffers
with device-local tensors and reusable staging. The [mixed-precision checkpoint](../../docs/ggml-mixed-precision.md)
adds FP16 matrix weights with FP32 activations and arithmetic; F32 remains the control.

## Reproduce

Run from the repository root on Linux x86-64. Requirements: the runtime's Rust
toolchain/loader, C/C++17 compilers, CMake 3.20+, Ninja, Git, Bash, curl, gzip,
sha256sum, ripgrep, awk, and SPIRV-Tools. The modern execution baseline is required;
neither graphics nor timestamps is required. GGML's configured CPU
reference currently requires AVX2/FMA/F16C, even though the GPU kernels use FP32.

```sh
git clone --no-checkout https://github.com/ggml-org/ggml.git target/ggml-source
git -C target/ggml-source checkout --detach 7840aaba1989c6deeefede1d77d5aaf8f52b947e
bash integrations/ggml/prepare.sh
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 \
    bash integrations/ggml/run.sh target/ggml-source
```

`prepare.sh` downloads only MNIST test data into ignored `target/ggml-data` and
verifies the [dataset checksums](dataset.sha256). Routine local/CI acceptance
uses the [checked-in trained fixture](fixtures/README.md), whose exact SHA-256
is verified by both preparation and execution. It never trains a new model or
uses an arbitrary model left in `target/`. Optional `train.sh target/ggml-source`
separately exercises upstream CPU training and saves a different output filename.
Training is not part of the OGPU adapter.

Source: [GGML MNIST example](https://github.com/ggml-org/ggml/tree/7840aaba1989c6deeefede1d77d5aaf8f52b947e/examples/mnist).
Data: [MNIST](https://yann.lecun.com/exdb/mnist/), by Yann LeCun, Corinna Cortes and
Christopher J. C. Burges, downloaded from the `cvdf-datasets` Google storage mirror.
Consult the dataset's own attribution/terms; this repository's MIT license does
not replace them. GGML is MIT licensed. Dataset images/labels are not redistributed;
the locally trained regression weights are included with provenance and MIT license.

`run.sh` performs no downloads or training. It verifies test-data/model checksums, builds
the release Rust library, validates the checked-in SPIR-V, independently builds
the consumer with CMake, and runs all acceptance checks. It rejects a wrong GGML
revision or tracked upstream edits. It fails on process errors or Vulkan validation
errors and leaves a uniquely named acceptance log under `target/ggml-integration`.
Use matching header/library/shaders from the same OGPU checkout, not an arbitrary
older shared library. The current checkout uses ABI 10 (native 16-bit buffer storage);
GGML retains its ABI-5 dispatch signatures with Y=Z=1 and explicit allocation placement.
Leave `CARGO_TARGET_DIR` unset.

An optional second argument to `run.sh` selects the OGPU probe's device index
(default 0). A third argument selects `host` or `device` (default `device`, to
exercise staging, not because it is universally faster). To compare both:

```sh
bash integrations/ggml/run.sh target/ggml-source 0 host
bash integrations/ggml/run.sh target/ggml-source 0 device
```

A fourth argument selects `f32` (default) or `f16` matrix weights. For mixed runs:

```sh
bash integrations/ggml/run.sh target/ggml-source 0 host f16
bash integrations/ggml/run.sh target/ggml-source 0 device f16
```

`convert-weights.cpp` deterministically derives two GGUF files from the pinned
original: F16 matrix weights with unchanged F32 biases, and those same rounded
weights widened back to F32 for the CPU semantic reference. Files go to a unique
ignored `target/ggml-integration/weights.*` directory; hashes are logged. The
original fixture is never edited. The upstream loader and graph builder handle
both files unchanged. No conversion occurs inside GPU graph execution.
The CPU's native F16 matrix path would also round activations, so it is not used
as the FP32-activation oracle. A second CPU graph with the original weights
measures model drift; see the brief for predeclared logit/prediction limits.
Matrix upload payload is checked: 1,588,000 bytes F32 versus 794,000 bytes F16;
the 2,040 bias bytes remain F32. This is payload reduction, not a speedup claim.

Both precision modes also run `ogpu-ggml-matrix-check`: odd 13×11×3 dimensions,
a 1×1×1 activation-precision sentinel, two-byte-aligned weights/transfers, output
poisoning, three repetitions and rejection of F16 activations/outputs/elementwise
operations without dispatch or output changes. `check-shaders.sh` validates all
binaries and checks storage-only half capabilities and absence of relaxed precision;
that shader gate also runs in hosted CI.

Both run the same six full-dataset and lifecycle checks. `memory_stats` reports
setup upload bytes/transfer time separately from steady-state upload/download
counts, bytes, transfer time and graph time. Times are adapter CPU wall milliseconds,
including synchronous submission/wait/destruction, not isolated GPU kernel time;
graph time excludes graph preflight, CPU reference execution and output checks.
The driver verifies two uploads and one readback per inference: input, deliberate
output poisoning, then logits. It rejects unexpected weight/intermediate movement
or staging reallocation during repeated inference. Setup includes model loading
and rejection checks, not just weight transfer. HOST reports CPU-copy bytes, not
PCIe traffic; DEVICE reports explicit GPU-copy payload bytes, not bus transactions.

To select a software ICD use the loader's `VK_DRIVER_FILES`; set
`OGPU_VULKAN_LIBRARY` and `VK_LAYER_PATH` if needed by your installation. Validation
must be installed/enabled to claim a validated run; the script cannot prove that
a requested layer was successfully activated on every loader configuration.

To regenerate shaders (shaderc 2026.1 / glslang 16.4 used for this checkpoint):

```sh
for shader in integrations/ggml/shaders/*.comp; do
    glslc --target-env=vulkan1.2 "$shader" -o "$shader.spv"
done
glslc --target-env=vulkan1.2 -DF16_WEIGHTS=1 integrations/ggml/shaders/matrix.comp \
    -o integrations/ggml/shaders/matrix-f16.comp.spv
bash integrations/ggml/check-shaders.sh
```

## Integration boundary

[backend.cpp](backend.cpp) translates GGML buffer callbacks and graph nodes into
the public C ABI. Its C++ is consumer glue for GGML's C++ application/backend
interface, not a second GPU runtime. It contains no host-side Vulkan calls and
does not include private Rust/backend headers. The runtime remains Rust.

[mnist.cpp](mnist.cpp) calls the unmodified upstream model loader and graph builder.
It checks both direct backend execution and execution through GGML's scheduler
and lifetime-based graph allocator, at batch sizes 1, 17 and 64 (six full test-set
runs). GGML CPU runs a separate reference graph. In scheduled mode, the driver
checks support before allocation, verifies every operation is placed on the primary
OGPU backend and the graph is one split, and verifies placement before execution.
It does not force per-node placement. The constructor prints its normal fallback
list, but this acceptance workflow forbids fallback. GPU dispatch counts must be
exactly five for **each** call; unsupported graphs fail before submission.
Four parameter nodes may also appear in GGML's graph and require no dispatch.

Supported tensor profile: contiguous, non-view FP32 1D/2D tensors with both leading
extents in 1..1024 and remaining extents equal to 1. Operations are `MUL_MAT` with
matching inner dimensions, row-broadcast bias `ADD`, and unary ReLU. Matrix source
A also accepts F16; B, outputs, bias and ReLU remain F32. F16 no-op declarations
are accepted for resident weights, not evidence of general F16 operations. ADD/ReLU may
exactly alias source 0 (same start and byte extent); every invocation reads and
writes its own element, and shaders do not use `restrict`. Matrix aliases,
partial overlaps and broadcast-bias overlaps are rejected. GGML, not OGPU, decides
when an input tensor is dead and eligible for overwrite.
This is exercised on finite trained weights/normalized inputs, not a complete
NaN/Inf arithmetic conformance suite. Unsupported types/layouts/ops are rejected;
out-of-range, foreign-buffer and unsafe overlapping addresses also fail graph
preflight. Copy callbacks have no GGML error return, so transfer failures
abort loudly rather than return stale data.

Each graph records one batch with compute/transfer dependencies before each
dispatch, including the first (previous submissions). It submits and waits once.
The graph, weights and input allocations remain live throughout. All host calls
and backend instances share one externally serialized execution device; async,
events, host mapping/import and cross-device copies are not advertised.

Root layout is 40 bytes: three 64-bit GPU addresses at 0/8/16, followed by u32
M/N/K/operation at 24/28/32/36. All shaders use `main` and 64 invocations.
Matrix workgroups produce 8×8 tiles with 512 bytes of shared memory; GGML's layout
is A[K,M], B[K,N], C[M,N], first dimension fastest. Element workgroups process 64
values, guarding the tail. These bounds fit Vulkan's minimum core limits, so no
new optional profile or workgroup-limit query is needed. The mixed matrix variant
requires the ABI-10 `storage_buffer_16bit_access` baseline bit. It loads aligned-2
half values, widens them into FP32 shared tiles, and uses FP32 multiplication and
accumulation. It does not require or enable `shader_float16`. Session creation
checks the enabled capability contract before preparing shaders.

## Deliberate costs and remaining friction

- GGML suballocation fits stable GPU addresses and checked offset copies. But its
  pointer-arithmetic ABI requires a distinct aligned host token range: this simple
  adapter allocates an extra host byte array per GPU buffer, not a data mirror.
  The direct control uses `alloc_ctx_tensors`; the scheduled path uses GGML's
  graph allocator. Both reuse their allocations over repeated calls; the latter
  also reuses dead intermediate storage within each forward pass.
- Placement is fixed per session. DEVICE keeps all tensor allocations device-local
  and stages synchronous callbacks through one grow-to-fit HOST buffer. It adds
  one copy submission/wait per callback, with no automatic migration or tensor mirror.
  HOST is the direct-copy control. Usage-specific placement, batched/asynchronous
  transfers and persistent mappings are not part of this checkpoint.
- Bias and ReLU need shader code, not host-side tensor operators. There are three
  prepared shaders (two matrix variants), five dispatches, and conservative global barriers.
  This is correctness/integration evidence, not competitive GEMM performance.
- The upstream constructor assumes registry ordering leaves CPU last. The driver
  unregisters/re-registers the statically linked CPU backend before constructing
  models. It does not change upstream sources. The scheduled path uses this
  scheduler with placement checked; unsupported fallback is rejected by the driver.
- `OgpuGgmlSession` owns registration and GPU state; backend streams and buffers
  retain that state explicitly. Destroy the session after those children, before
  static teardown. Live-child destruction aborts with a diagnostic. Initialization
  failure destroys partial resources without publishing a registration.
  The [teardown investigation](../../docs/ggml-hardening.md) isolates the original
  fault to validation-layer static destruction ordering, also with direct Vulkan.
- No claim yet about arbitrary GGML graphs, general tensor views, asynchronous
  callbacks, larger/quantized models, other GPU vendors, or graphics consumers.
  See [the follow-up results and API alternatives](../../docs/ggml-hardening.md).
