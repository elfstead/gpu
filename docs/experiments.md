# Experiment ledger

Updated 2026-09-13. This page records evidence, not API-stability promises. See
[the current design](design.md) for direction and [development](development.md)
for commands. "Implemented" does not mean production-ready or performance-tuned.

The initial feasibility phase is complete. The [working plan](plan.md) now defines
the API-candidate/integration milestone and its next task. This ledger is evidence,
not a development queue; existing experiments remain regression and diagnostic tools.

## Implemented baseline

The [original heap-image experiment](heap-images.md) established direct compute image
load/store and fragment sampling without intermediate copies. The
[preservation](image-preservation.md) and [independent-heap follow-up](descriptor-heaps.md)
replace its coupled table API with independently owned, exclusively mutable heaps.
Six sizes with four sampler/index variants pass across three submissions, alongside
LOAD/CLEAR, retention/failure tests and pinned Slang/SPIR-V reproduction. ABI 3 is
current; general formats/views and concurrent heap edits remain outside this checkpoint.

The [retirement experiment](retirement.md) adds completion polling and optional
whole-buffer retention. Twelve jobs recycle three scratch ranges through the C API;
a gated Vulkan test verifies reuse while another submission is still pending.

The [modern migration](modern-baseline.md) reran execution tests on llvmpipe, plus all
six GGML acceptance cases. The [ABI-3 hardware follow-up](hardware-validation.md) now
passes the compute examples, seven compute-side Vulkan tests and all six GGML cases
on RX 5700 XT / RADV with software ICDs excluded. Physical graphics/heaps remain
unsupported on that driver (no unified image layouts); remote execution CI is pending.
No new performance conclusion follows from these runs.

| Experiment | Run | Evidence | Still not established |
|---|---|---|---|
| Discovery and C boundary | `cargo xtask smoke`, `cargo xtask mock`, `cargo xtask abi` | Capability reporting, loader failures, C/Rust layouts, version/pointer checks | Portable capability negotiation or feature enablement |
| Address-based round trip | `cargo xtask compute` | CPU upload, repeated dispatch, partial updates, readback, retained device ownership | Device-local staging, suballocation, actual non-coherent hardware coverage |
| Dependent compute kernels | `cargo xtask batch` | Copied roots, intermediate storage, explicit dependency, one submission/wait | Workgroup cooperation, arithmetic acceleration, replay or performance |
| Compute → offscreen graphics | `cargo xtask graphics` | GPU-generated vertices/indirect arguments, image rendering/readback, shared batches | Sampling, direct compute image access, presentation |
| Graphics → compute → graphics | `cargo xtask image-loop` | Explicit image/linear conversion, compute pixel transform, fragment address reads, guarded intermediate/final checks and reuse | Sampling/storage images, conversion costs, filtering, general formats |
| Cooperative integer reduction | `cargo xtask reduction`, `cargo xtask gpu-tests` | Shared memory, uniform workgroup barriers, multi-level partial sums, tails/empty inputs/overflow, guarded intermediate outputs | Floating-point accuracy, subgroup acceleration, scratch reuse, performance |
| FP32 matrix multiplication | `cargo xtask matmul` | Baseline/tiled kernels, FP64 references, guarded row strides, 50 cases per kernel, separate setup/copy/warmed host timings | Isolated GPU timing, accelerated/narrow types, tuned BLAS comparison, performance portability |
| Optional batch timestamps | `cargo xtask matmul`, `cargo xtask gpu-tests` | Queue-specific clock reports, completion-owned queries, wrap arithmetic, state/failure tests, timed/untimed matrix comparison | Per-region attribution, calibrated clocks, real narrow-counter hardware, controlled benchmarks |
| Backend ownership/failure paths | `cargo xtask gpu-tests` | Batch states, retained resources, image reuse, preparation/submission/wait failure injection | Real hardware device loss, arbitrary shader faults, all driver behavior |

These paths have been verified locally on the RX 5700 XT (RADV) and llvmpipe, with
Vulkan synchronization validation for execution tests. The discovery mocks and
ordinary unit tests do not need a GPU. CI is configured to run validation, not
claimed to have run remotely. These are correctness results, not benchmarks.

## First consumer integration — 2026-09-12

The [GGML MNIST checkpoint](consumer-ggml.md) is integration evidence, not another
API-surface experiment. Its unmodified upstream FP32 forward graph uses five
dispatches per batch through the unchanged public API. All 10,000 test digits at
batch sizes 1, 17 and 64 matched GGML CPU predictions on RX 5700 XT and llvmpipe
with synchronization validation: 98.01% accuracy for the recorded saved model,
maximum absolute logit difference 0.0000343322754. Every logit met the predeclared
tolerance. Repeated allocation use, full teardown/recreation and unsupported-graph
preflight rejection passed. See the brief for source/model revisions and checks.

The initial checkpoint validated consumer-side suballocation and direct synchronous
graph integration. The [hardening/scheduler follow-up](ggml-hardening.md) also
verifies GGML placement and safe in-place storage reuse: intermediate buffer bytes
fall from 389,120 to 130,560 at batch 64 with unchanged numerical results on both
drivers. General GGML operations and competitive ML performance remain unproven.
The [friction report](../integrations/ggml/README.md#deliberate-costs-and-remaining-friction)
records host token storage, registry ordering and lifecycle costs. No GPU runtime
or public API changes were necessary; no graphics-consumer conclusion follows.

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
API was added in that initial checkpoint. Workgroup/tile metadata remains a
caller/shader agreement. Its host measurements motivated the timing follow-up below.

## Optional timing outcome — 2026-09-12

Status: implemented and locally verified; [contract, sources, and measurements](timing.md).
Three new functions report the selected queue's clock, opt a recording batch into
timing, and retrieve a duration after an explicit successful wait. Existing layouts
and ordinary execution requirements are unchanged. Unsupported timing leaves
ordinary batches usable; untimed execution creates no query resources.

Both local devices pass timing state/failure tests, existing timed mixed-command
tests, and 50 matrix cases per kernel in both modes. The validation-disabled
256³ RADV device-batch medians were 0.3655 ms baseline and 0.1618 ms tiled, versus
0.5504/0.3408 ms timed host latency. llvmpipe retained a higher tiled median.
Query retrieval is reported separately, and untimed controls expose instrumentation
cost. These intervals do not isolate individual shader/barrier costs or yield an
exact CPU/GPU latency decomposition. The machine was not benchmark-isolated.

## Parked follow-ups, not a work queue

Larger matrix/submission sweeps, resource-reuse tuning, accelerated numeric
variants, and image-representation cost comparisons remain possible follow-ups.
They are not scheduled and do not block the first integration merely because they
are untested. Consumer selection and candidate decisions belong in the
[working plan](plan.md); a second backend is a later portability gate.

Before adding an experiment, name the API decision it can change, the alternatives,
the minimum discriminating check and a stopping condition. If a check is needed,
record its workload, API changes, independent references, hardware/toolchain,
observed result and limitations here. Pure tuning belongs to implementation work,
not an indefinitely extended API-feasibility phase.
