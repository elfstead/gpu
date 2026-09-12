# FP32 matrix multiplication experiment

## Hypothesis and workload

Ordinary address-based allocations, copied roots, and compute dispatch should
express both a scalar-per-output matrix product and a shared-memory tiled version
without a host matrix operation or API additions. Implement `C = A × B` for
row-major FP32 A[M,K], B[K,N], and C[M,N], with explicit element row strides.
There is no batching, transpose flag, alpha/beta accumulation, or accelerated
matrix instruction in this first experiment. M and N are positive; K may be zero,
in which case every logical output is zero. Inputs and outputs do not alias.

Implemented in [the C harness](../examples/matmul.c),
[baseline shader](../examples/shaders/matmul-naive.comp), and
[tiled shader](../examples/shaders/matmul-tiled.comp). Run `cargo xtask matmul`.
This runner builds the runtime in release mode and the C harness with
`-O2 -DNDEBUG`, without fast-math. The same checks stay enabled in optimized builds.

The baseline assigns one output to each of 64 workgroup invocations. The tiled
variant uses an 8×8 output tile, 64 invocations, and two shared 8×8 input tiles.
It flattens the two-dimensional tile grid into the API's existing 1D dispatch.
All lanes participate in both barriers per K tile, including boundary lanes.
Each kernel accumulates FP32 products in ascending K order; results need not be
bitwise identical across kernels or devices.

The copied root is 48 bytes: GPU addresses A/B/C at byte offsets 0/8/16, followed
by uint32 M/N/K/lda/ldb/ldc at 24/28/32/36/40/44. Strides are in elements, not bytes.
The fixed test sizes keep all uint32 indexing, rounded tile counts, and dispatch
counts in range; these shaders are not arbitrary-size matrix input validators.
Each GPU address skips a four-byte prefix guard. Tiling needs 512 bytes of shared
storage and no subgroup-size assumption or optional matrix feature.

## Numerical and memory checks

Compute independent CPU references in FP64 from the actual FP32 inputs. For each
output, also accumulate `S = sum(abs(A[row,k] * B[k,col]))`. Require finite results
and absolute error no greater than `1e-6 + 8 * max(K,1) * FLT_EPSILON * S`.
This is a conservative workload acceptance bound, not a universal error guarantee
or a claim of correctly rounded results. Report maximum absolute and scaled error.

Test zero, identity-like, signed fractional, cancellation-heavy, and mixed-scale
finite inputs. Cover K=0, singleton, rectangular, tile-aligned and partial-tile
shapes, and a long dot product. Pad all row strides and guard each allocation.
Poison logical outputs with NaNs, check every result, check untouched output row
padding/guards, and verify inputs are unchanged. Test each kernel independently
against the reference, not merely against the other GPU kernel.

The ten (M,N,K) shapes are (1,1,0), (1,1,1), (3,5,7), (7,9,8), (8,8,9),
(9,7,17), (17,19,31), (31,17,33), (65,63,129), and (4,3,1025).
Each uses all five input patterns: 50 cases per kernel per device. The three
timing shapes below also check every output on every warmup and measured sample.
Row strides are K+3, N+5, and N+7 for A, B, and C respectively.

## Measurement contract

Use a monotonic host clock. Report device creation and kernel/pipeline creation
separately from per-shape allocation, initial upload, warmed execution, and CPU
readback. Offline GLSL-to-SPIR-V compilation is not timed. Warm each kernel twice,
then report median and min/max across nine samples, with CPU result checks/readback
outside the execution interval. Alternate kernel order to reduce ordering bias.
Vulkan validation overhead, when enabled, remains inside the execution interval.

The host computes references before timing. Between executions it poisons/uploads
C again and checks the readback, so these are repeated isolated calls, not sustained
queued throughput. The output-reset interval includes CPU poisoning plus the copy
and is reported separately. Allocation timing includes the three GPU allocations,
host staging allocations, and their poison initialization, but not reference work.

The execution interval covers batch creation/recording, submission, completion
wait, and cleanup. It is **not GPU kernel time**: the current API has no GPU
timestamps, and host/driver overhead is included. Upload/readback measure the
current host-visible allocation copies/cache maintenance, not device-local PCIe
staging. Correctness runs with Vulkan validation; comparative timing must also
be collected in a separate validation-disabled run, with device/build conditions
recorded. No timing threshold gates tests.

Keep scratch and arguments alive through completion. Explicit compute-write →
compute-write dependencies cover output reuse. Do not widen the API for timing
yet; use the measurements to decide whether GPU timestamps are the next justified
addition. No comparison against tuned BLAS libraries or accelerated matrix units
is implied by comparing these two deliberately simple shaders.

## Observed result — 2026-09-12

Both kernels passed all 50 cases and all timing-shape checks on the RX 5700 XT
(RADV NAVI10) and llvmpipe (LLVM 21.1.8, 256 bits), both reporting Vulkan 1.4.354.
Validation-enabled runs used Vulkan loader/layers 1.4.357 with synchronization
validation and reported no errors. Existing Rust GPU tests, C compute/batch/
graphics/image-loop/reduction examples, ABI/mock checks, unit tests, and Clippy
also passed. The new runner exercises the release library. CI is configured for
the matrix experiment and SPIR-V validation; no remote run is claimed.

Maximum absolute error across correctness and timing cases was 14.5025 for each
kernel/device. Absolute error alone is misleading for the mixed-scale inputs,
whose products can approach one million. The maximum per-output error divided by
its acceptance bound was 0.0159155, well below the failure threshold of 1. These
matching maxima do not assert bitwise equality between every output. NaNs,
infinities, denormal behavior, FP16/BF16, adversarial conditioning, and application-
specific accuracy requirements remain outside this experiment.

Two separate validation-disabled runs showed the same direction: tiled execution
had lower median host latency on RADV and higher median latency on llvmpipe for
these three shapes. The second run, from implementation commit `8082545`, recorded:

| Device | M × N × K | Baseline ms, median [min,max] | Tiled ms, median [min,max] |
|---|---|---|---|
| RX 5700 XT | 128 × 128 × 128 | 0.3188 [0.3055,0.3742] | 0.2039 [0.1977,0.2191] |
| RX 5700 XT | 257 × 193 × 129 | 0.3649 [0.3576,0.3791] | 0.2636 [0.2524,0.2756] |
| RX 5700 XT | 256 × 256 × 256 | 0.5020 [0.4937,0.5169] | 0.3041 [0.2971,0.3113] |
| llvmpipe | 128 × 128 × 128 | 0.3763 [0.3040,0.4066] | 0.4519 [0.3897,0.5100] |
| llvmpipe | 257 × 193 × 129 | 1.1366 [0.9743,1.4634] | 1.6607 [1.4885,2.0181] |
| llvmpipe | 256 × 256 × 256 | 2.5450 [2.3114,3.6382] | 3.1900 [2.3932,3.8236] |

These are end-to-end execution intervals, **not isolated kernel timings**. On
the 256-cubed RADV case, separate initial allocation/staging and upload took
0.4321 and 0.0844 ms; median output reset/readback were about 0.031/0.009 ms.
The corresponding llvmpipe figures were 0.1730/0.3254 ms for allocation/upload and
about 0.034/0.010 ms for reset/readback. The runner reports all components for all
shapes rather than hiding these costs inside an apparent kernel throughput figure.

Device creation took 1.2137 ms on RADV and 10.7445 ms on llvmpipe. Baseline/tiled
pipeline creation took 0.2078/0.0304 ms and 0.3382/0.7596 ms respectively. Driver
disk caches were not cleared, so these are not cold compilation measurements.
Offline shader compilation is excluded entirely.

Host: Ryzen 9 5900X, Linux x86-64; Rust 1.97.1 and Clang 21.1.8. Shader tools:
glslang 16.4.0 and SPIRV-Tools 1.4.357.0. CPU/GPU clocks were not locked, the
machine was not isolated, and llvmpipe timings varied materially. Nine samples
and two process runs are preliminary evidence, not a statistically controlled
benchmark or evidence of competitive GEMM performance.

## Design implications and next question

- Both kernels fit the existing API; matrix multiplication belongs in executable
  code, not a new host `matmul` entry point.
- Row strides and padding are ordinary root data. Layout conventions must still
  be agreed between caller and shader; no tensor metadata is managed by the runtime.
- A 2D tile grid fits a 1D dispatch, but host code must know each executable's
  workgroup/output tile. Executable metadata and specialization remain real design
  questions; this experiment does not prove that flattening is free.
- Shared-memory tiling is not a universal optimization. Variant selection should
  remain possible, with measurements on the target device rather than one mandated
  tile or assumed subgroup width.
- **GPU timestamps are now a justified next experiment.** Host timing detects a
  difference but cannot partition recording/submission overhead, queue delay, and
  shader execution. Test a narrow optional timing facility before making kernel-
  throughput claims or selecting accelerated variants from these numbers.

## Reproduction

With a working Vulkan loader and installed validation layers:

```sh
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 cargo xtask matmul
env -u VK_INSTANCE_LAYERS -u VK_LAYER_VALIDATE_SYNC \
    VK_LOADER_LAYERS_DISABLE=VK_LAYER_KHRONOS_validation cargo xtask matmul
```

The second command matches the recorded validation-disabled run on the local
1.4.357 loader. Ensure other local layer configurations do not re-enable
instrumentation. The harness reports relevant environment variables, not a list of
active layers. `OGPU_VULKAN_LIBRARY` can select a loader as described in
[development](development.md). The C harness visits every execution-capable device
and fails if none is available; it runs separately from `cargo xtask gpu-tests`.

The two SPIR-V binaries are checked in, so normal builds need no shader compiler:

```sh
for shader in matmul-naive.comp matmul-tiled.comp; do
    glslangValidator -V --target-env vulkan1.2 "examples/shaders/$shader" -o "examples/shaders/$shader.spv"
    spirv-val --target-env vulkan1.2 "examples/shaders/$shader.spv"
done
```
