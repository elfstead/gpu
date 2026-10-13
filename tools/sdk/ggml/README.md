# Independent foundation GGML consumer

This copied source example uses only the installed foundation C header/library
plus the pinned upstream GGML source. It does not build OGPU, need Rust, compile
shaders or import an OGPU checkout. The bounded adapter is C++ consumer glue; the
runtime remains Rust. No general GGML, competitive GEMM or Metal claim is made.

Requirements: Linux x86-64, C/C++17, CMake 3.20+, Ninja, Git, Python 3.11+, pkg-config,
the matching foundation preview and a compatible modern Vulkan driver. GGML's
configured CPU reference additionally needs AVX2/FMA/F16C. Pin headers, library and
artifacts to exactly one SDK source revision; revision 1 alone is insufficient.

```sh
git clone https://github.com/ggml-org/ggml.git /path/to/ggml
git -C /path/to/ggml checkout --detach 7840aaba1989c6deeefede1d77d5aaf8f52b947e
export PKG_CONFIG_PATH=/path/to/preview/lib/pkgconfig
python3 build.py --ggml-source /path/to/ggml
python3 run.py
```

CMake rejects the wrong upstream commit or tracked modifications. Builds force
foundation selection and obtain include/library paths from `ogpu-foundation.pc`.
The installed helper is present only as source migration context; it is not a
fallback and is not selected by this build script.

The default run checks HOST/DEVICE odd-sized FP16-storage/FP32-arithmetic matrices,
two-byte transfers, rejected operations, repeated session initialization, forbidden
live-child teardown, and local recording failure/recovery. It also runs the opt-in
foundation transformer profile: five (width, hidden width, sequence length) shapes
in F32/F16-weight modes, A/B/A reuse, all 15 intermediates against an independent
FP64 reference, and 11 invalid-graph/retry controls. These deterministic fixtures
need no downloaded model. The profile is single-head, bidirectional, pre-RMSNorm
attention plus residual/ReLU feed-forward, not a general LLM backend.
A separate QuantizedLinear profile checks F32/F16/Q8_0
weights, F32 accumulation, five linear shapes, A/B/A, two-byte weights, guards and
ten rejection/retry cases. Q8_0 K is a multiple of 32; scales are FP16 and signed
values are packed INT8. No hardware integer-dot/matrix acceleration is assumed.
It does **not** run the MNIST dataset by default. To additionally execute all six direct/scheduled batch
configurations for both F32 and F16 weights:

```sh
python3 run.py --data-dir /path/to/already-extracted-mnist-test-data
```

Supply `t10k-images-idx3-ubyte` and `t10k-labels-idx1-ubyte`. The bundled digest file
is checked before inference. Test data is not redistributed or downloaded by these
scripts. MNIST is by Yann LeCun, Corinna Cortes and Christopher J. C. Burges; obtain
it under its applicable terms. GGML is MIT licensed. The bundled trained fixture
has project-authored MIT provenance in `fixtures/README.md`. Half/widened models
are derived into each fresh report directory; originals are never overwritten.

All gates are unchanged from the in-tree consumer. Reports preserve output and
complete/incomplete status in fresh `run-*` directories, including failed runs.
Enable `VK_LAYER_KHRONOS_validation` and synchronization validation when installed
to collect validated evidence. This handoff uses local tools/driver caches; it is
not a clean-machine, universal-binary or outside-adoption claim.

For separate cost observations, after correctness acceptance:

```sh
env -u VK_INSTANCE_LAYERS -u VK_LAYER_VALIDATE_SYNC \
    VK_LOADER_LAYERS_DISABLE=VK_LAYER_KHRONOS_validation \
    VK_DRIVER_FILES=/path/to/selected/icd.json python3 measure.py
```

This measures both placements (or choose `--placement device`), records every
sample and SDK/source/device identity, and reruns numerical checks outside timed
intervals. Nine samples of ten synchronous graph calls follow two warmups. Costs
include graph preflight/record/submit/wait/reset, not transfer or GPU-only timing.
Allocation bytes exclude runtime objects, CPU tokens and transfer staging. No
native-performance equivalence or model-quality guarantee follows from these data.
