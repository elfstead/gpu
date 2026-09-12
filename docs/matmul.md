# FP32 matrix multiplication experiment

## Hypothesis and workload

Ordinary address-based allocations, copied roots, and compute dispatch should
express both a scalar-per-output matrix product and a shared-memory tiled version
without a host matrix operation or API additions. Implement `C = A × B` for
row-major FP32 A[M,K], B[K,N], and C[M,N], with explicit element row strides.
There is no batching, transpose flag, alpha/beta accumulation, or accelerated
matrix instruction in this first experiment. M and N are positive; K may be zero,
in which case every logical output is zero. Inputs and outputs do not alias.

The baseline assigns one output to each of 64 workgroup invocations. The tiled
variant uses an 8×8 output tile, 64 invocations, and two shared 8×8 input tiles.
It flattens the two-dimensional tile grid into the API's existing 1D dispatch.
All lanes participate in both barriers per K tile, including boundary lanes.
Each kernel accumulates FP32 products in ascending K order; results need not be
bitwise identical across kernels or devices.

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

## Measurement contract

Use a monotonic host clock. Report device creation and kernel/pipeline creation
separately from per-shape allocation, initial upload, warmed execution, and CPU
readback. Offline GLSL-to-SPIR-V compilation is not timed. Warm each kernel twice,
then report median and min/max across nine samples, with validation/readback
outside the execution interval. Alternate kernel order to reduce ordering bias.

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
