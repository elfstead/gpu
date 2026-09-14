# Two-consumer experimental checkpoint

Updated 2026-09-15. A working low-level host/runtime interface for compute and
offscreen graphics, with a Rust implementation and C ABI 10. This is not a GPU
source language, stable release, portable standard or general consumer backend.
The [design](design.md) describes direction; the [plan](plan.md) owns selected work.

## What can be used today

- Explicit HOST/DEVICE buffers, GPU addresses, copies, prepared SPIR-V executables,
  specialization, X/Y/Z dispatch, dependencies, batches, wait/poll and optional timing.
- Shared compute/raster execution with independent image/sampler heaps, preserved
  images and indirect draws. Images: 1D/2D RGBA8/R32F; raster: fixed-state RGBA8,
  offscreen only. Exact limits and supported combinations are queryable.
- GGML MNIST direct and scheduled inference: FP32 or FP16 matrix weights with
  FP32 arithmetic, HOST/DEVICE placement, independent CPU comparison.
- libplacebo EWA Lanczos compute plus nearest raster: upstream-generated processing,
  independent native reference, two-slot reuse and consumer-owned frame batching.

Use matching header/library/shaders from one source revision. Calls on a device
and its children are externally serialized. Shaders and pointer-reachable data
are trusted; callers own pointee lifetimes and explicit dependencies. Completion
wait/poll retires submitted resources; it is not pointer tracing or a sandbox.
See [the public header](../include/ogpu.h) for exact obligations.

Not included: presentation, arbitrary raster state/formats, general GGML/libplacebo
backends, FP16 arithmetic/matrix acceleration, multi-queue/multi-device execution,
another runtime backend, stable cross-version ABI or broad hardware support claims.
The optimized-shaderc diagnostic fails validation with current descriptor-heap
shaders; it is not enabled in the ordinary compiler path.

## Build and acceptance from a fresh checkout

Prerequisites are external: Linux x86-64, Rust/Cargo (declared minimum 1.85),
C/C++ compilers, Git, Bash, ripgrep, CMake/Ninja, Meson, pkg-config, Python with
Jinja2/MarkupSafe, shaderc development files and modern SPIRV-Tools. libplacebo
needs C++20 upstream and Vulkan headers/registry; GGML's CPU reference needs
AVX2/FMA/F16C. Validation requires an installed loader, ICD and validation layer.
See [development](development.md) and the two integration READMEs for exact tools.

Normal Rust builds use checked-in bindings and shader fixtures. Header regeneration
is separate; the headers submodule is needed for ABI/mock and libplacebo checks.

```sh
# From a fresh checkout of this repository, without copying an old target directory:
git submodule update --init vendor/Vulkan-Headers
unset CARGO_TARGET_DIR
export CARGO_INCREMENTAL=0
cargo build --locked
cargo test --locked
cargo xtask abi
cargo xtask mock

# Select one installed ICD exposing the intended device; do not copy this placeholder.
export VK_DRIVER_FILES=/absolute/path/to/one/icd.json
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
export VK_LAYER_VALIDATE_SYNC=1
cargo xtask baseline
cargo xtask gpu-tests

git clone --no-checkout https://github.com/ggml-org/ggml.git target/ggml-source
git -C target/ggml-source checkout --detach 7840aaba1989c6deeefede1d77d5aaf8f52b947e
bash integrations/ggml/prepare.sh
for placement in host device; do
    for precision in f32 f16; do
        bash integrations/ggml/run.sh target/ggml-source 0 "$placement" "$precision"
    done
done

git clone --no-checkout https://github.com/haasn/libplacebo.git target/libplacebo-source
git -C target/libplacebo-source checkout --detach 3330a515d62139259c26239014f286e233bd3a5c
bash integrations/libplacebo/run-consumer.sh target/libplacebo-source
bash integrations/libplacebo/run-perf.sh target/libplacebo-source verify
```

Run builds sequentially; scripts use checkout-local `target/` paths. On a second
available ICD, repeat execution commands with that ICD. No second physical GPU
is required for this checkpoint. `verify` checks all three libplacebo policies
and all benchmark extents/modes without timing. To measure performance separately,
omit `verify`; do not compare validation-enabled timing with measured results.

If needed, set `OGPU_VULKAN_LIBRARY` to a trusted real loader and `VK_LAYER_PATH`
to the installed layers. Native libplacebo uses its own loader path, so ensure it
resolves the same loader build. Do not carry diagnostic loader/environment overrides
into acceptance. `CC`, `CXX`, `PKG_CONFIG_PATH`, `LIBCLANG_PATH` and tool `PATH`
may need installation-specific setup; scripts do not provision these tools.

For offline execution, clean pinned local source clones and checksum-verified
MNIST test inputs can replace downloads. Cargo's source cache can be reused.
Record this reuse explicitly: an empty build directory is not an empty machine
or an independently reproduced dependency installation.

## Evidence and coverage

Runtime image fix: `1f41d7e`; performance acceptance: `5b7acd2`. Existing evidence
is RX 5700 XT / RADV Mesa 26.2.1 and llvmpipe, with 30 ordinary tests, 20 GPU tests
per driver and 745 ABI checks. [Fresh native comparison](libplacebo-diagnosis.md#runtime-correction--2026-09-15):
near-4K resident grouped OGPU 563.54 vs native 593.17 fps. Smaller workloads retain
larger overhead gaps; these are workload-specific local measurements, not a general
Vulkan comparison or isolated API cost.

Clean-checkout reproduction is in progress; its exact revision, commands, reused
dependencies, results and limitations will be recorded here before closing the
checkpoint. No additional physical GPU is available. Cross-vendor and physical UMA
coverage remain unproven; synthetic selectors, mocks and llvmpipe are complementary
checks, not substitutes. No remote runner has been provisioned or remote workflow
execution demonstrated by this local checkpoint.
