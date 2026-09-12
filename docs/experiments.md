# Experiment ledger

Updated 2026-09-12. This page records evidence, not API-stability promises. See
[the current design](design.md) for direction and [development](development.md)
for commands. "Implemented" does not mean production-ready or performance-tuned.

## Implemented baseline

| Experiment | Run | Evidence | Still not established |
|---|---|---|---|
| Discovery and C boundary | `cargo xtask smoke`, `cargo xtask mock`, `cargo xtask abi` | Capability reporting, loader failures, C/Rust layouts, version/pointer checks | Portable capability negotiation or feature enablement |
| Address-based round trip | `cargo xtask compute` | CPU upload, repeated dispatch, partial updates, readback, retained device ownership | Device-local staging, suballocation, actual non-coherent hardware coverage |
| Dependent compute kernels | `cargo xtask batch` | Copied roots, intermediate storage, explicit dependency, one submission/wait | Workgroup cooperation, arithmetic acceleration, replay or performance |
| Compute → offscreen graphics | `cargo xtask graphics` | GPU-generated vertices/indirect arguments, image rendering/readback, shared batches | Sampling, direct compute image access, presentation |
| Graphics → compute → graphics | `cargo xtask image-loop` | Explicit image/linear conversion, compute pixel transform, fragment address reads, guarded intermediate/final checks and reuse | Sampling/storage images, conversion costs, filtering, general formats |
| Cooperative integer reduction | `cargo xtask reduction`, `cargo xtask gpu-tests` | Shared memory, uniform workgroup barriers, multi-level partial sums, tails/empty inputs/overflow, guarded intermediate outputs | Floating-point accuracy, subgroup acceleration, scratch reuse, performance |
| FP32 matrix multiplication | `cargo xtask matmul` | Baseline/tiled kernels, FP64 references, guarded row strides, 50 cases per kernel, separate setup/copy/warmed host timings | Isolated GPU timing, accelerated/narrow types, tuned BLAS comparison, performance portability |
| Backend ownership/failure paths | `cargo xtask gpu-tests` | Batch states, retained resources, image reuse, preparation/submission/wait failure injection | Real hardware device loss, arbitrary shader faults, all driver behavior |

These paths have been verified locally on the RX 5700 XT (RADV) and llvmpipe, with
Vulkan synchronization validation for execution tests. The discovery mocks and
ordinary unit tests do not need a GPU. CI is configured to run validation, not
claimed to have run remotely. These are correctness results, not benchmarks.

## Cooperative reduction outcome — 2026-09-12

Status: implemented and locally verified; [contract and source links](reduction.md).
No public API additions or runtime execution changes were needed. A 64-invocation
workgroup reduces up to 128 unsigned inputs using shared memory; explicit batch
barriers connect the partial-sum levels. The C example reduces 1,048,579 inputs
through 8,193 and 65 partials to one modulo-2^32 result, `3717237828`, with one
submission and one wait.

The Vulkan-backed test checks 17 input lengths × 4 patterns (68 cases per device).
Lengths include zero, 64/128-element boundaries, 16,384/16,385 (a dispatch-depth
transition), and a million-element case. Patterns are zeros, ones, UINT32_MAX, and
deterministic mixed values. After one final wait per case, CPU references check
every intermediate partial, prefix/suffix guards, and the unchanged input. A
driver-free planning test also checks uint32-max sizing without allocating it.

Both the C example and 68-case suite passed on the RX 5700 XT (RADV) and llvmpipe
with synchronization validation. The shader was compiled with glslang 16.4.0 and
validated with SPIRV-Tools 1.4.357.0. Existing compute/graphics examples, unit tests,
ABI/mock checks, Clippy, and the release build also passed locally.

Design implications:

- Workgroup cooperation fits in shader code plus existing dispatch dependencies;
  a host `reduce` operation is not required for this workload.
- Padding lanes must participate in shader barriers even when they have no input.
  Workgroup barriers and inter-dispatch dependencies are separate obligations.
- Scratch ownership is still manual. Copied roots do not retain their pointees;
  dedicated per-level allocations prove correctness, not an optimal reuse strategy.
- Host dispatch sizing knows the shader's fixed 128-input tile. Executable metadata,
  specialization, and workgroup variants remain unresolved before tuning variants.
- Empty logical input does not require a zero-byte allocation: the workload uses
  a valid dummy allocation and emits zero. Tensor/collection semantics stay above
  the allocation API.

This does not establish floating-point summation accuracy, subgroup-size control,
matrix acceleration, competitive throughput, device-local transfer performance,
or correctness on additional hardware vendors.

## Image-processing loop outcome — 2026-09-12

Status: implemented and locally verified; [contract and source links](image-loop.md).
No API or runtime changes were needed. Compute-generated geometry produces a
coordinate-colored triangle; a copy exposes RGBA8 pixels as linear memory. Compute
flips rows, swaps red/blue, and inverts green into a separate buffer. A fullscreen
fragment shader reads those bytes by address and renders the result to a second
target. One submission and one final wait cover the complete loop.

The C harness checks every intermediate and final pixel, prefix/suffix guards,
copied processor roots, and target/buffer reuse. Six sizes, including tiny,
non-square, odd, and exact-workgroup cases, run twice on each graphics-capable
device. All 12 loops passed on both the RX 5700 XT and llvmpipe with synchronization
validation. Fresh poison values prevent stale output passing the second run.

Design implications:

- Existing allocations, roots, dependencies, and completions compose across all
  stages. A host image-processing operator is not required for this transform.
- Images remain distinct objects: the image-to-buffer copy is an explicit
  representation boundary, not evidence that an optimal image has a GPU address.
- Fragment address reads can consume processed linear pixels without a texture
  binding. This does not provide filtering, mipmaps, or arbitrary image formats.
- The transfer-write → compute-read and compute-write → fragment-read dependencies
  belong to the caller; completion only becomes relevant at the final host read.
- No performance claim follows: this path pays for separate allocations and two
  image copies. Direct storage/sampled-image alternatives need a measured comparison.

## FP32 matrix outcome — 2026-09-12

Status: implemented and locally verified; [contract and measurements](matmul.md).
One scalar-per-output kernel and an 8×8 shared-memory tiled kernel implement
row-major C=A×B with explicit strides, using the existing API unchanged. Both
passed 50 shape/input cases per device plus every benchmark warmup/sample on
RADV and llvmpipe. Checks use FP64 references, a per-output magnitude-aware FP32
error bound, NaN-poisoned outputs, row padding/guards, and unchanged inputs.

The release runner separates device/pipeline setup, allocation/staging, host
copies, and warmed recording/submission/wait/cleanup. Two validation-disabled runs
showed lower median tiled latency on RADV but higher median tiled latency on
llvmpipe across the three measured shapes. This supports retaining kernel variants,
not selecting a universal tile. Timings include host/driver overhead, and the
machine was not isolated or clock-locked; isolated shader throughput remains unknown.

No matrix host operation, 2D dispatch, optional arithmetic feature, or timestamp
API was added. Workgroup/tile metadata remains a caller/shader agreement. A narrow
optional timestamp experiment is now the next justified measurement step before
attributing performance differences or evaluating accelerated matrix variants.

## Queued experiments

1. Add a narrow optional GPU-timestamp experiment to distinguish device execution
   from host/runtime latency in matrix and mixed workloads; then investigate
   supported accelerated/narrow-type matrix variants against the FP32 baseline.
2. Follow up the image loop with measured representation costs before deciding
   whether to add direct storage-image access or sampled-image bindings.
3. A compiler/runtime consumer and a second backend for the common compute model.

For each new experiment record: hypothesis, exact workload, required API changes,
independent checks, hardware/toolchain used, observed result, untested cases, and
the next design question. A successful example is a reason to continue testing,
not a reason to freeze its temporary constraints.
