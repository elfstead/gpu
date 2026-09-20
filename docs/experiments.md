# Experiment ledger

Updated 2026-09-16. This page records evidence, not API-stability promises. See
[the current design](design.md) for direction and [development](development.md)
for commands. "Implemented" does not mean production-ready or performance-tuned.

The initial feasibility phase is complete. The [working plan](plan.md) now defines
the current two-consumer checkpoint and selected work. This ledger is evidence,
not a development queue; existing experiments remain regression and diagnostic tools.

## Implemented baseline

The [compiler-facing workflow](compiler-workflow.md) is complete at `66854db`.
Generated host roots and executable metadata replace manual declarations for
the existing integer transform. Original and field-order/workgroup-mutated
variants pass on Radeon and llvmpipe using unchanged host code. Sixteen generator
tests, byte-for-byte reproduction and C layout assertions establish the bounded
compiler/host interface; runtime/public ABI stay unchanged. This is a pinned
adapter experiment, not a source-language choice or universal shader package.

The [numerical executable experiment](ml-executable-requirements.md) is complete
at `f663a4b`, after merging Metal compute/GGML acceptance. Optional Float16
enablement and caller-owned numerical/capability requirements suffice for the
bounded paired-product variants. Numerical/memory gates pass on Radeon and
llvmpipe; the FP16-product candidate is about 3–9% slower in measured Radeon batch
time, so it is not selected automatically. Existing GGML arithmetic is unchanged.
The brief records the corrected xtask release-library lookup and evidence limits.

The [consolidated two-consumer checkpoint](checkpoint.md) passes fresh-checkout
acceptance at `798e186`, using newly built runtime/upstream/consumer binaries.
All 30 ordinary tests, 745 ABI checks, mock cases and 20 GPU tests per driver pass;
Radeon covers all four GGML placement/precision combinations (24 dataset cases),
llvmpipe repeats DEVICE/F16 (six). Both drivers pass libplacebo controls and all
benchmark correctness configurations. Installed tools and pinned source/data caches
were reused; no clean-machine, minimum-Rust or extra-hardware claim follows.
No runtime/build-script fix, API change or new performance measurement was needed.
See the [receipt](results/checkpoint-2026-09-15.txt) for exact coverage and provenance.

The [image-memory preference correction](libplacebo-diagnosis.md#runtime-correction--2026-09-15)
is implemented at `1f41d7e`, following the diagnosis below. Images prefer eligible
non-host-visible local memory, while visible local/UMA and existing eligibility
rules remain supported. Three selector tests, 30 ordinary tests, 745 ABI checks,
both-driver consumer comparisons and all 20 GPU tests per driver pass. All 54
fresh ordinary timing runs complete without diagnostic controls: near-4K resident
median fps is 593.17 native / 554.75 per-operation / 563.54 grouped OGPU; transfers
45.63 / 63.97 / 65.22 under the declared differing staging/readback policies.
Retain the fix; ABI 10, shaders, barriers and buffer policy are unchanged.

The [resident performance diagnosis](libplacebo-diagnosis.md) identifies image
memory-type preference as the dominant near-4K cost on RX 5700 XT. Pre-fix images
selected a 256 MiB host-visible local heap; a validated diagnostic loader control
selects eligible non-host-visible local memory, changing nothing else. Three rotated
ordinary-harness runs improve median grouped throughput from 104.04 to 562.93 fps,
against a fresh native 591.60 fps. Separate device timestamps fall from 9.497 to
1.672 ms per frame. All sizes/modes still match native exactly under validation.
Residency/eviction is an unmeasured possible mechanism, not an established fact.
The runtime correction above follows this diagnostic checkpoint, which itself
included no runtime fix or API change. An isolated shaderc
optimization variant failed SPIR-V validation and was not timed or adopted.

The [controlled libplacebo performance comparison](libplacebo-performance.md) is
complete at `90dd3d7`, without runtime changes. Native Vulkan, OGPU per-operation
and consumer-side per-frame batches produce byte-identical outputs on RADV and
llvmpipe at three extents in resident/transfer modes, with synchronization validation.
A native upload-path hazard was caught before timing; an explicit public-API HOST
staging policy was documented and committed, with upstream unchanged. Fifty-four
validation-disabled RX 5700 XT runs show batching lowers host recording cost but
does not close the resident-image gap: near-4K median throughput is 553 fps native,
98.9 per-operation, 99.1 per-frame; with transfers it is 45.10/41.83/41.91 fps.
Retain frame grouping in consumer policy, not a new runtime scheduler. At this
pre-diagnosis checkpoint the cause was unproven; the later diagnosis and correction
above supersede that uncertainty. Measurements include different implementation
paths and are not isolated API overhead. Raw results and limits are in the brief.

The [two-frame libplacebo checkpoint](libplacebo-inflight.md) retains ABI 10 with
no runtime changes. Per driver, synchronous and two-slot 36-frame runs each match
1,145,952 reference bytes exactly. Both submit 183 operations; final two-slot runs
make zero explicit waits on llvmpipe and one on RADV versus 183 in each control.
Two frames, fixed receipt arrays and duplicated mutable pass banks bound ownership;
allocations stop after warmup. Pending/error/destruction and queued specialization
tests pass. Keep frame scheduling and reuse policy in the consumer; this is reduced
host synchronization, not a throughput or simultaneous-GPU-execution claim.

The [mixed-precision GGML checkpoint](ggml-mixed-precision.md) adds ABI 10's native
16-bit buffer storage, already required physically by Vulkan 1.4. Twenty-four
FP16-weight/FP32-arithmetic cases and twenty-four F32 controls pass on llvmpipe and
RADV under both placements. Matrix payload halves; predictions remain unchanged.
The CPU oracle widens rounded weights without rounding activations; a separate
original-model reference measures weight-rounding drift. No optional creation
profile, half arithmetic, tensor operator or accelerated matrix facility was added.
Runtime/C examples and the libplacebo byte-for-byte comparison still pass on both
drivers. This is correctness and integration evidence, not a speedup claim.

The [original heap-image experiment](heap-images.md) established direct compute image
load/store and fragment sampling without intermediate copies. The
[preservation](image-preservation.md) and [independent-heap follow-up](descriptor-heaps.md)
replace its coupled table API with independently owned, exclusively mutable heaps.
Six sizes with four sampler/index variants pass across three submissions, alongside
LOAD/CLEAR, retention/failure tests and pinned Slang/SPIR-V reproduction. These
introduced ABI 3; general formats/views and concurrent heap edits remain deferred.

The [memory checkpoint](memory-transfers.md) introduces ABI 4: explicit HOST/DEVICE
allocation and retained byte-range GPU copies. GGML passes all six direct/scheduled
cases under both placements on RADV and llvmpipe. Exact transfer counters verify
resident weights/intermediates and reusable staging; measured transfer overhead
argues against a hidden universally-device-local policy. The contract stays explicit;
no allocator framework, migration or new tensor operation was added.

The [libplacebo G1 grid checkpoint](consumer-libplacebo.md#g1-checkpoint-multidimensional-dispatch)
introduces ABI 5: explicit X/Y/Z workgroup counts, with checked per-axis limits.
Both C dispatch entry points execute 1D/2D/3D grids with unmodified builtin IDs;
tail/guard/argument-copy tests and existing execution regressions pass on RADV and
llvmpipe. That checkpoint established an adapter prerequisite, not consumer acceptance.

The [G1 image checkpoint](consumer-libplacebo.md#g1-checkpoint-image-descriptions-and-upload)
introduces ABI 6: 1D/2D RGBA8/R32F images with explicit usage and retained upload /
readback copies. Native float LUT sampling and repeated uploads exercise the image
requirements observed in the upstream consumer. The target-only API is removed;
the adapter and upstream comparison were deferred to G2.

The [G1 executable checkpoint](consumer-libplacebo.md#g1-checkpoint-specialization-and-vertex-pulling)
introduces ABI 7: copied per-stage 32-bit specialization and triangle-list/strip
selection. The adapter's bounded declaration lowering pulls original vertex records
through an address, leaving processing bodies unchanged. Dedicated GPU tests execute
these contracts on both drivers; all six captured libplacebo executables prepare
with their actual constants. That gate prepared, but did not submit, upstream passes.

The [libplacebo G2 execution gate](consumer-libplacebo.md#g2-bounded-ogpu-execution)
now passes on both drivers: 9 compute and 9 raster submissions, three extents with
A/B/A updates, unchanged upstream generation/math, and exact intermediate/final
matches against upstream (286,488 bytes per driver). A cached device-limits query
is additive to ABI 7; no shader-contract change was needed in the adapter. Explicit
uploads, heaps, barriers and completion cleanup implement the bounded workload.
Operations block individually; this is not an asynchronous scheduling or performance
result, a general libplacebo backend, or an API-stability decision.

The [capability follow-up](execution-capabilities.md) introduces ABI 8: explicit
compute/image versus raster creation, cached enabled capabilities and exact image
support checks shared with creation. The adapter checks its format combinations
before advertisement. Both driver suites and paired libplacebo comparisons pass;
compute-created devices execute native sampling/storage without enabling raster.
The follow-up records the next D4 comparison without implementing reclamation.

The [D4 receipt/resource follow-up](completion-resource-review.md#implementation-abi-9)
implements ABI 9: wait/terminal poll retires command pools and retained objects,
while result/timing receipts survive. Gated tests preserve pending/shared-use safety;
lazy timing remains independent. The C heap example and libplacebo reuse resources
with old receipts alive. No background collector, range allocator or asynchronous
adapter scheduling is added.

The [retirement experiment](retirement.md) adds completion polling and optional
whole-buffer retention. Twelve jobs recycle three scratch ranges through the C API;
a gated Vulkan test verifies reuse while another submission is still pending.

The [modern migration](modern-baseline.md) reran execution tests on llvmpipe, plus all
six GGML acceptance cases. The [ABI-3 hardware follow-up](hardware-validation.md) now
passes compute, graphics/image examples, all twelve Vulkan tests and all six GGML
cases on RX 5700 XT / RADV with software ICDs excluded. At `a5a609d`, unified image
layouts became optional, retaining the same GENERAL-only commands. Full Vulkan and
image regressions also pass independently on llvmpipe; remote execution CI is pending.
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

## Bounded HDR consumer — 2026-09-15

The [HDR brief](libplacebo-hdr.md) and [receipt](results/libplacebo-hdr-2026-09-15.txt)
close the selected RGBA16F resize and static BT.2020-to-sRGB tone-mapping workload.
ABI 11 adds explicit raster target format, RGBA16F images/rendering and sampled/
transfer RGBA16 UNORM. The latter preserves pinned libplacebo's clipping policy:
without it, upstream silently selects saturation mapping. Raster parameters fit
inline with an appended vertex address; no new public uniform-buffer abstraction.

The original native exact-alpha gate failed and is preserved at `77ca4c5`; the user
accepted a one-step binary16 alpha tolerance before OGPU comparison. All nine HDR
cases now match native intermediate/final bytes exactly on both available drivers,
with equal upstream parameter bytes, A/B/A reuse and cleanup checks. Runtime,
existing SDR/scheduling/benchmark-correctness and GGML DEVICE/F16 regressions pass.
Retain the existing ownership/address/heap/batch model and the evidenced format
extensions; no general HDR profile, timing target or automatic expansion follows.

## Mixed learned-image application — 2026-09-17

The [flagship brief and results](learned-image.md) close the first complete mixed
ML/graphics application at `21dc090`. A frozen, reproducible 89-parameter residual
CNN feeds resize/palette compute and offscreen raster through shared DEVICE
buffers. All 38 cases pass in normal and diagnostic modes on Radeon and llvmpipe,
with independent scalar checks, guarded buffers and A/B/A reuse. There are no
intermediate CPU reads/waits or GPU representation copies in the ordinary path.
Runtime/API unchanged. Synthetic quality and small correctness fixtures are not
a photographic or performance claim. Grow compiler-owned executable mechanics
around this application next; do not turn it into a kernel-tuning work queue.

## Application compiler interface — 2026-09-17

The [learned-image compiler migration](learned-image-compiler.md) is complete at
`e363377`. The existing adapter now checks FP32 scalar pointers and bounded
vertex/fragment interfaces, generating six distinct embedded-artifact C headers.
All original application gates pass on both Vulkan drivers with original and
reversed-root/32-thread interfaces, using unchanged C source; outputs between
variants are byte-identical. No runtime/API change. Remove duplicated host layouts
and superseded GLSL sources; keep semantic execution policy in the application.
The next large milestone is the same application on Metal graphics, not automatic
expansion of the compiler subset or network tuning.

## Installed consumer boundary — 2026-09-17

The [Linux quickstart](quickstart.md) and [installation receipt](results/installed-sdk-2026-09-17.txt)
establish standalone consumption at `6d34967`. A clean-revision SDK was installed,
relocated and used to build/run a C application from another directory on Radeon
and llvmpipe. The consumer finds only installed headers/library via pkg-config;
shader regeneration is optional and also works through the installed tool.
No runtime/API change or portable binary-distribution claim. This is a tested
installation boundary, not evidence that an independent third party has adopted it.

## Video-scale correctness — 2026-09-18

The [scale brief and results](learned-image-scale.md) close the first roadmap M1
slice at `86dd16e`. A bounded-row binary64 C oracle matches all 38 Python fixtures
byte-for-byte; both video-scale A/B/A groups pass full intermediate/final checks
on Radeon. Small regressions pass on Radeon and llvmpipe. Both interface variants
and normal/diagnostic modes pass, with real multi-row dispatch and checked size
arithmetic. Initial floating resize-coordinate failures and the unsuccessful
reassociation attempt are retained; integer quotient/remainder pixel selection
passes without changing the model or numerical gates. Runtime/API unchanged.
This is correctness evidence, not a benchmark. The next M1 slice is 4K/odd extents
and general resize ratios; timing/native control follows. See the
[roadmap](roadmap.md) for current sequencing, including the separate Metal track.

## 4K and odd-ratio correctness — 2026-09-18

The [second scale slice](learned-image-scale.md#accepted-second-slice) completes
M1's declared extent/correctness matrix at `6d36e93`. All six A/B/A scale groups
pass on Radeon with original/reversed interfaces and both modes; 38 small cases
also pass on Radeon and llvmpipe. This includes 4K inputs, odd input/output extents,
non-integer up/down ratios and full intermediate comparisons. No new failed gate,
tolerance change or shader/runtime change. Keep the current implementation and
proceed to warmed measurement, allocation accounting and a matched native Vulkan
control; this acceptance is not performance evidence. See the
[receipt](results/learned-image-4k-2026-09-18.txt).

## Learned-image warmed baseline and display-boundary correction — 2026-09-19

The [measurement protocol and results](learned-image-measurement.md) complete the
OGPU half of M1 performance evidence at `958b831`: twelve mode/interface validation
processes, 24 timing processes (720 retained measured frames) and eight independent
allocation traces. Resident and end-to-end modes preserve the frozen arithmetic
and full scalar/pixel gates. The 4K downscale median is 3.988 ms resident and
11.941 ms end-to-end; peak traced Vulkan allocations are 427.667 and 396.026 MiB.
All traced allocations are freed; driver-private memory/residency is not measured.

The initial ordinary llvmpipe regression crashed twice. Core analysis identified
an out-of-range display fragment pointer read beyond an odd image edge. `9430ad3`
adds height to the generated root and bounds coordinates before pointer indexing;
visible pixels and tolerances are unchanged. The original failed acceptance is
retained, not replaced by a passing retry. Corrected small suites, all six Radeon
scale groups/both interfaces and 20 fresh odd-edge llvmpipe processes pass.

Retain the corrected baseline; no runtime/public API change is selected. Next is
the [matched native Vulkan control](learned-image-native-control.md), not kernel
tuning or an overhead claim from OGPU-only clocks. M1 remains active. See the
[receipt](results/learned-image-measurement-2026-09-19.txt) and committed raw data
linked from the measurement result.

## Learned-image native-control foundation — 2026-09-19

At clean commit `de85a07`, the benchmark-only native path builds independently of
the OGPU runtime and passes injected host policy/partial-failure cleanup tests.
The direct modern Vulkan device/buffer/address-copy/timeline path passes a full
260-byte A/B/A round trip under synchronization validation on Radeon and llvmpipe.
Separate allocation traces match requested/native buffer accounting and free all
three allocations; peak tracked bytes are 816 and 780 respectively.

This closes only the native setup/transfer slice. It does not execute the learned
model, establish shader/raster parity or compare performance. Keep the same
generated code/fixtures for the next workload slice. See the
[native-control status](learned-image-native-control.md#implementation-status--accepted-foundation-slice)
and [receipt](results/learned-image-native-foundation-2026-09-19.txt). No runtime,
public API or accepted OGPU baseline change; M1 remains active.

## Learned-image native compute/raster correctness — 2026-09-19

At clean commit `219254e`, the direct Vulkan control executes the same generated
compute/raster programs as OGPU. All 32 traced validation processes pass: small
odd-edge A/B/A on both drivers, both modes and interface variants, plus four
large Radeon groups. All intermediate/final outputs match fresh OGPU outputs
byte-for-byte and pass the unchanged full-reference gates. Allocation sizes/types
match both engines; all 304 tracked allocations are freed. The validation memory
peaks include full intermediate readbacks, unlike ordinary timing allocations.

Keep this as the correctness control. No runtime/API/model/shader change is
selected. Next match timestamp queries and implement native warmed timing before
the fresh paired comparison; output parity is not a performance result. M1 remains
active. See the [command-policy inventory and accepted result](learned-image-native-control.md),
[receipt](results/learned-image-native-workload-2026-09-19.txt) and linked evidence.

## Learned-image matched measurements: M1 complete — 2026-09-19

At clean implementation `a819f6c`, the direct Vulkan control matches OGPU's
timestamp/query-retirement and final-only-readback policy. Fresh full-output
validation passes in 32 processes on Radeon/small llvmpipe (192 frames), with
byte-identical native/OGPU outputs and unchanged gates. Forty-eight fresh timing
processes retain all 1,440 post-warmup samples; 16 separate traced controls match
allocation sizes/types and free all 152 allocations, with no frame-count growth.
All timed/traced final outputs match fully validated B. Tests cover query failures,
pending/failed-completion rejection, wrapping clocks, terminal pool retirement,
balanced control order and export consistency. Host checks include 37 Rust tests,
Clippy, formatting, 749 ABI layouts and native static analysis.

Across the eight selected extent/mode cases, ratios of median-of-three process
medians put OGPU 0.04% lower to 1.14% higher than native. The 4K downscale is
3.984/3.979 ms OGPU/native resident and 12.071/12.057 ms end-to-end. Several OGPU
tails are noisier, with host submit/query or staging/readback spikes; no particular
driver/scheduler cause, uniform tail parity or isolated API-overhead claim follows.
All samples/outliers remain. Memory peaks match, with zero tracked live allocations
after cleanup; driver-private allocations and physical residency are not measured.

Retain the runtime/API, generated interfaces and one-shot batches. GPU work
dominates resident latency; transfer/synchronization dominates the mode difference.
M3 should test sustained slot/staging reuse, not assume replay or a runtime allocator.
The earlier six-group scale acceptance and this matched comparison complete M1.
M2's device-code/compiler contract and explicit language-direction decision are
next, not implemented here. No new hardware, native Metal round trip, model/shader
change or runtime/API widening. See the [result and decision](learned-image-native-control.md#accepted-comparison-and-m1-decision--2026-09-19)
and [receipt](results/learned-image-m1-2026-09-19.txt) for provenance, raw data,
distribution/memory tables, reproduction and exact acceptance boundaries.

## Performance expressibility: small-compute frontier — 2026-09-19

The [standing gate](performance-expressibility.md) compares stronger native strategies,
not only native controls constrained to OGPU's policy. At `c437fb9`, all four
small-compute strategies pass full per-frame correctness at one/three slots and
1/64 dependent dispatches: 16,000 validated Radeon frames and 1,024 llvmpipe frames.
48,000 ordinary timing samples and separate allocation traces are retained; all
tracked allocations are freed. Native reset/re-record wall time is substantially
lower than the current fresh-pool OGPU implementation (about 1.62–8.15x at equal
slots/work). Replay reduces host recording further, without uniformly improving
throughput. This is not a general API-overhead or optimal-native claim.

The [result and contract consequence](performance-frontier-small.md) separates
submission retirement, reusable empty command storage and reusable executable
recordings. Current documented native-pool destruction is a design restriction,
not an unquestioned requirement: a storage alternative must reset native references
before releasing heaps/objects, preserve completion semantics and bound retention.
No runtime/API change yet; streaming and other expressibility concerns remain open.

## Performance expressibility: streaming image frontier — 2026-09-20

At `ed50e00`, the [streaming probe](performance-frontier-stream.md) passes 24,000
validated Radeon frames across 720p/odd extents, one/two/three slots and four native/
OGPU strategies; software controls check 768 small odd frames. Final-image numerical
gates are unchanged, with separate post-window input/weight integrity and intermediate
guard checks. This does not repeat M1's full scalar-intermediate acceptance.

72,000 ordinary timing samples plus separate allocation controls show two slots
improving OGPU throughput about 25–27%, with roughly doubled data allocations and
higher observed frame latency. A third adds no useful throughput here. At equal
slots/work, OGPU process wall times are 1.46–2.73% higher than native reset/re-record;
replay reduces host recording further but does not consistently improve throughput.
All 1,020 tracked allocations across correctness/allocation controls are freed.
Driver-private storage is not measured. Radeon ran clean; llvmpipe recorded pending
documentation edits, with identical committed code hashes and binaries.

Retain explicit independent slots, but do not approve the fundamental API from
these cases. The [next storage review](command-storage-review.md) separates backing
capacity, executable references and submission retirement, with heap/lifetime/failure
and timing-receipt gates. No runtime/API change yet. Cross-queue/concurrent host work,
mapped/range access, dependency precision, descriptor streaming and compiler limits
remain open. The receipt links full statistics, raw samples and checked traces.

## Command-storage lifetime alternative — 2026-09-20

At `01697a0`, ABI 13 explicitly relaxes unconditional native-pool destruction:
reset native references before releasing application objects, while allowing a
bounded empty-storage cache. Batches remain one-shot; no executable replay or new
public object is introduced. The cache holds at most three pools and admits at
most 256 steps/64 KiB roots; it is not a driver-private-byte budget.

The [accepted result](command-storage-results.md) adds 48,000 measured samples.
OGPU/native-reset wall ratios become 1.050, 1.041, 1.022 and 0.994 in the original
four small-compute cases. This closes most of the earlier storage-policy gap,
without establishing universal parity or unavoidable API overhead. All 22 runtime
GPU tests pass on Radeon/llvmpipe; added checks cover cache admission/cleanup,
warm-cache errors, reset-before-reference-release, heap replacement, surviving
timing and pending/error gates. The independent relocated ABI-13 SDK C consumer,
C heap-image and small mixed-workload checks also pass. No native Metal or full
consumer/video-scale performance reacceptance is claimed.

Retain the cache as an implementation experiment. Next compare explicit caller
storage ownership/release and separately executable replay. The cache's hidden
admission thresholds and missing caller budget/control are not the fundamental
interface selected; the remaining performance-expressibility audit stays open.

## Explicit recording-storage ownership — 2026-09-20

At `7fa0763` (ABI 14), [caller-owned storage](recording-storage.md) provides one
reservation per owner, reusable empty capacity and explicit idle trim, without
reviving consumed batches or retaining application objects in old receipts.
Vulkan implements it; Metal's optional entry points explicitly return UNSUPPORTED.

The [accepted result](recording-storage-results.md) adds 90,000 timing samples,
30,000 Radeon and 1,920 llvmpipe correctness frames, plus separate allocation
controls. Explicit/native-reset wall ratios are 0.991–1.037 across one/five slots
and 64/129/512 dispatches. At 129 dispatches, which crosses cache admission,
throughput improves about 31–47% over the implicit cache path. All 270 tracked
application allocations are freed; native command-storage bytes remain unmeasured.
All 23 runtime GPU tests pass on both drivers, including oversized heterogeneous
reuse, trim with old handles/timing alive, heap replacement, pending/error gates
and injected warm-owner failures. An independent relocated ABI-14 SDK consumer
passes both default and explicit-owner paths.

Retain explicit ownership as a preferred controllable candidate, the cache as a
convenience/control, and neither as a settled final API. Next specify/test reusable
executable recordings and distinguish the runtime's CPU step/root buffering from
unavoidable API work. Native replay still saves substantial host work; similar
multi-slot throughput does not settle that question. No native Metal validation,
exact byte budget or general Vulkan parity is claimed.

## Immutable executable replay — 2026-09-20

At `ae69055` (ABI 15), the [command-list experiment](command-lists.md) exposes
compile-once/submit-many with persistent recorded ownership, independent receipts,
simultaneous in-flight uses and explicit retry/poison/loss rules. Timed compilation
and Metal compilation are unsupported; no implicit fallback or native Mac claim.

The [accepted result](command-list-results.md) retains 72,000 timing samples,
24,000 Radeon/1,536 llvmpipe correctness frames and 192 compiled mixed-image frames
per driver. Twenty-five runtime GPU tests on each driver and relocated SDK C
consumers cover ownership between uses, gated concurrent execution, mutable data,
early destruction, heap exclusion and failure cleanup. Host tests, bindings,
749 ABI values and loader mocks pass.

At 512 dispatches, OGPU repeated host work falls about 88%, from 112–122 µs to
13–14 µs versus native replay's 13 µs. Compiled/native-replay wall ratios span
0.967–1.044. Replay loses throughput to re-recording in the five-slot/64-dispatch
case on both sides; retain both strategies. All tracked application allocations
are freed, but native command-memory/CPU-vector budgets are not equated.

Fixed-command replay is now expressible, not a universal native-parity proof.
Next probe P3's copy-only host access and whole-buffer exclusion with separate
native controls under [declared storage/latency constraints](mapped-streaming-review.md).
Do not keep this bounded P2 result open for unrelated tuning; mutable command
variants and per-execution timing remain explicitly unresolved.

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
