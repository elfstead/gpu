# Working status and next milestone

Updated 2026-09-20. This page owns current status and selected work. The
[design](design.md) describes the model; the [ledger](experiments.md) records
evidence. The [roadmap](roadmap.md) covers the remaining project work and proposed
sequence. The [historical plan](plan-history.md) preserves earlier milestones.

## Current phase

Initial feasibility, two bounded consumer integrations and a mixed learned-image
application are complete. We have
an **experimental runtime/API candidate**, not a stable API, a portable standard,
a GPU source language or a general graphics/ML platform. Breaking changes remain
allowed when evidence exposes a better API alternative; compatibility is not a veto.

The runtime is Rust over the modern Vulkan baseline, plus an experimental native
Metal compute backend. The language-neutral C boundary is now **ABI 15**, adding
optional immutable command lists on Vulkan after ABI 14's explicit recording-storage
ownership; ABI 13 relaxed retirement to allow bounded empty command storage. The bounded macOS arm64
compute/GGML path was verified at ABI 12; Metal remains destruction-based and is
not natively revalidated for this revision (new owner/list calls return UNSUPPORTED).
The validated Metal branch is merged
into `master`. Use matching header/library/shaders
from one source revision. No release/tag or cross-version stability is implied.

| Area | Current evidence | Important boundary |
|---|---|---|
| Core model | Explicit HOST/DEVICE buffers, addresses, prepared executables, X/Y/Z dispatch, batches, dependencies, terminal completion retirement | One queue, externally serialized calls, trusted shaders and caller-owned pointer lifetimes |
| Graphics/images | Compute and raster share batches; independent heaps, preservation, uploads/readbacks and indirect draws | Narrow offscreen state and formats; no presentation or general rendering backend |
| GGML | MNIST direct/scheduled inference, FP32 and FP16 weights with FP32 arithmetic, both memory placements | Bounded operators/layouts; no FP16 arithmetic or accelerated matrix profile |
| libplacebo | EWA compute, nearest raster and bounded HDR-to-SDR processing match upstream; two-frame reuse and batching remain | Static scene-linear BT.2020 to sRGB conversion, not a general media backend |
| Learned-image application | Residual CNN, resize/palette and raster share DEVICE buffers; 38 small cases on both Vulkan drivers plus six full-reference video-scale A/B/A groups through 4K/odd extents on Radeon | Tiny synthetic-trained model; no photographic quality or Metal graphics claim |
| Performance | ABI-15 fixed-command replay cuts repeated host work about 88% at 512 dispatches; compiled/native-replay wall ratios 0.967–1.044 | Replay does not win every throughput case; not approval of the fundamental API or equal total memory budgets |
| Validation | 25 GPU tests on each Linux driver, 38 ordinary tests, 749 ABI checks, independent installed consumer; historical Apple M4 compute/GGML acceptance | Metal has no ABI-15 revalidation or graphics acceptance; synthetic failures are not real device-loss evidence |

The [performance diagnosis and correction](libplacebo-diagnosis.md) are complete.
Keep runtime image-memory preference and consumer-side batching. No allocator
framework, runtime scheduler, new public placement flag or further performance
target was selected. The failed shaderc optimized-heap diagnostic remains a known
toolchain limitation; ordinary shader compilation is unchanged.

## Completed M1: useful-scale execution

[Roadmap M1](roadmap.md#m1--make-the-existing-application-useful-sized) is complete.
All six declared 720p/1080p/4K/odd scale groups pass full intermediate/final
correctness with checked multidimensional dispatch and a streaming binary64
oracle. The resize-coordinate and display-pointer defects found during scale work
were corrected without relaxing numerical gates. The frozen model is unchanged.

At clean implementation revision `a819f6c`, the
[matched native comparison and decision](learned-image-native-control.md#accepted-comparison-and-m1-decision--2026-09-19)
accept 1,440 measured samples from 48 fresh timing processes, 16 separate
allocation controls and fresh full-output correctness on Radeon/small llvmpipe.
Native/OGPU outputs, allocation sizes/types and clock policy match; all tracked
allocations are freed. Earlier [scale acceptance](learned-image-scale.md) supplies
the complete six-group extent coverage. See the [M1 receipt](results/learned-image-m1-2026-09-19.txt).

The M1 decision retained the runtime/API, generated interfaces and one-shot batches
provisionally; the stronger frontier below reopens command-storage lifetime. Across the
eight extent/mode cases, OGPU's median of per-process medians is 0.04% lower to
1.14% higher than native. For 4K downscale, resident latency is 3.984 ms OGPU versus
3.979 ms native; end-to-end is 12.071 versus 12.057 ms. Some OGPU tail samples are
noisier; no uniform tail parity, isolated API overhead or general GPU performance
claim follows. Transfer/synchronization costs dominate the mode difference; GPU
execution dominates resident latency. M3 should test sustained slot/staging reuse
and investigate tails if material, not assume replay or a runtime allocator.

## Active work: performance-preserving expressibility

The [contract audit and acceptance brief](performance-expressibility.md) adds the
fundamental gate: could a native implementation preserve Vulkan's attainable
performance under the same correctness and resource/latency requirements?
M1 remains complete, but its matched-policy retain decision is provisional, not
proof that the API excludes no better strategy. Pull forward the small-dispatch
and streaming-slot comparisons from M3. Separate API-imposed disadvantages from
backend costs and unresolved hardware-dependent questions; do not assume replay,
mapping or concurrency is unnecessary because M1 is GPU-heavy.

The first [small-compute result](performance-frontier-small.md) is accepted at
`c437fb9`: 48,000 timing samples, full per-frame Radeon/software correctness and
allocation cleanup. Native reset/re-record is substantially faster than OGPU's
fresh-pool lifecycle here. Separate reusable command storage from replay, and
review the contract's native-destruction promise before calling pooling a backend-
only fix. The [streaming result](performance-frontier-stream.md) is also complete
at `ed50e00`: 72,000 timing samples, 1,000-frame Radeon correctness at both extents
and one/two/three slots, smaller llvmpipe controls, and matched allocation cleanup.
Two slots improve throughput about 25–27% at higher memory/latency; three show no
useful additional throughput. Native reset/re-record remains faster than OGPU by
about 1–3% here, versus the much larger host-sensitive gap. Other audit concerns
remain open; this is not approval of the fundamental API.
The [command-storage review](command-storage-review.md) separates storage,
recording and submission lifetimes and defines the next heap/error/retirement gates.
The [bounded storage-reuse result](command-storage-results.md) is accepted at
`01697a0` (ABI 13): 22 runtime GPU tests on each Linux driver, 48,000 new timing
samples and an independent relocated SDK consumer. The four small-compute OGPU/
native-reset wall ratios are 1.050, 1.041, 1.022 and 0.994; most of the previous gap
was storage policy, not demonstrated unavoidable API overhead. Retain the cache
as an experiment, not a fundamental storage interface. It has size/admission
limits, no caller release control and no exact native-byte budget.

That result selected caller-owned storage versus the cache under heterogeneous,
long-lived use and explicit release. It did not settle executable replay or pass
other audit items; the follow-up below supplies the storage evidence.

The [explicit owner result](recording-storage-results.md) is now accepted at
`7fa0763` (ABI 14): 23 GPU tests on both drivers, 90,000 new samples, 30,000 Radeon/
1,920 software correctness frames and a relocated SDK consumer. Explicit storage
removes the cache's admission cutoff and adds caller-driven trim; wall ratios are
0.991–1.037 of native reset in the six selected cases. Retain it as the preferred
controllable candidate and the device cache as a convenience/comparison path.

The [reusable executable result](command-list-results.md) is accepted at `ae69055`
(ABI 15): 72,000 timing samples, 24,000 Radeon/1,536 software correctness frames,
mixed compute/render replay and an independent SDK consumer on both drivers.
At 512 dispatches, host record/submit falls from 112–122 µs to 13–14 µs, near
native replay's 13 µs. Compiled/native-replay wall ratios are 0.967–1.044; replay
loses to re-recording in the five-slot/64-dispatch case. Retain both strategies.
Fixed-command replay is now expressible, with copied roots, mutable pointed-to
data, persistent ownership and independent receipts. This closes that bounded
P2 experiment, not the complete performance gate or final owner ergonomics.

Next execute the [P3 direct-access/range brief](mapped-streaming-review.md): native
controls separate CPU copying from whole-buffer exclusion under explicit storage/
latency budgets, before selecting a new public mapping/range contract. Replay
timing and changing roots/dimensions remain separate open P2 questions. Direct
encoding/vector optimization is optional backend work, not the next API milestone.
No fixed native-byte budget or other audit item is settled. Do not widen unrelated
surface or treat this result as stabilization.

## Following milestone: M2 — programming/compiler contract

Start with a bounded acceptance brief and a device-code contract that separates
runtime guarantees, compiler conventions, capability requirements and caller
obligations. Then cover structured/root/pointed-to arguments, generated image/heap
interfaces and explicit source-build dependencies through the installed tool.
Finish with a committed language-direction decision grounded in actual compiler
output and host integration. Existing M1 outputs remain a regression gate.
M2 has not been implemented or accepted; see its [deliverables](roadmap.md#m2--define-the-programming-contract-and-broaden-compiler-tooling).

After M2: sustained resource reuse (M3), an experimental release checkpoint (M7),
and substantial graphics/ML consumers (M4/M5). Metal parity (M6) remains a separate
native-validation track, not a prerequisite for local progress. The roadmap
assigns remaining gaps either a milestone or an explicit deferral with entry
criteria; the experimental API is not frozen.

## Completed work: reproducible two-consumer checkpoint

1. Reconcile current design, support boundaries and build instructions, keeping
   chronology in the ledger/historical plan rather than the active status page.
2. Build from a fresh source checkout with a separate, initially empty target
   directory. Run ordinary tests, ABI/mock checks, GPU tests and both consumers
   through documented commands. Record dependencies, pins, reused input caches,
   exact coverage and any manual setup; do not call this a clean-machine test.
3. Finish with a version-identified checkpoint and explicit remaining limits,
   without freezing the API or automatically widening its surface.

The original checkpoint used the Radeon and software driver; later Apple M4
compute acceptance supplies bounded second-backend/UMA evidence. Further hardware
coverage is not a blocker for local work. No runner provisioning,
hardware acquisition, remote CI deployment or external coordination is selected.
Use the existing Radeon and software driver where useful; do not manufacture
portability claims from software execution or mocks.

Complete: [checkpoint and reproduction](checkpoint.md), tested at `798e186` with
later documentation-only reconciliation. The fresh checkout rebuilt the runtime
and both consumers without old build artifacts. All 30 ordinary/745 ABI/mock
checks pass; all 20 GPU tests and libplacebo acceptance pass on both available
drivers. GGML's full four-variant matrix passes on Radeon, with a representative
DEVICE/F16 six-case cross-check on llvmpipe. Installed tools, source/data caches
and driver caches were reused explicitly; this is not clean-machine provisioning.
No runtime/build-script changes were needed to complete that checkpoint.

## Decisions blocking the candidate

None for the current bounded checkpoint. D0–D5 are resolved or explicitly deferred;
their [original decision history](plan-history.md#decisions-blocking-the-candidate)
is not a new work queue.

- D0: GGML and libplacebo are the selected consumers, each with a bounded brief.
- D1: Explicit baseline/enabled capabilities and caller-checked executable
  requirements; general reflection and optional accelerated profiles remain deferred.
- D2: Explicit buffer placement/copies; image backing is runtime policy. No
  implicit migration or centralized allocator.
- D3: Shared image/graphics model with documented narrow support. Broader formats,
  views, raster state and concurrent heap mutation need a selected use case.
- D4: Consumer-managed scheduling/reuse; terminal observation retires submission
  resources. No hidden collector or runtime-owned graph scheduler.
- D5: Source revision plus matching ABI/header/library/shaders identify this
  experimental checkpoint. Breaking C layouts/signatures increment the ABI.

## Completed next use case: HDR-to-SDR processing

The [bounded HDR brief](libplacebo-hdr.md) selects a native-first libplacebo capture,
then the smallest coherent extension for floating-point resize and tone mapping
to SDR. It declares input representation, tone/gamut policy, numerical gates and
scope exclusions before results. This uses the existing Radeon and llvmpipe;
additional physical hardware is not a prerequisite.

Completed with ABI 11: RGBA16F images/rendering, explicit executable target format,
and genuine sampled/transfer RGBA16 UNORM support to preserve upstream's clipping
policy. The adapter packs 208 upstream raster parameter bytes plus the vertex
address without new public resource abstractions. The user-approved intermediate
alpha tolerance is explicit; the initial exact-alpha failure remains recorded.
All nine HDR cases match native intermediate/final pixels exactly on both drivers,
with identical upstream parameter bytes and checked cleanup. See the brief for
the retain/revise decision and regression receipt. No new performance target or
general HDR profile is selected.

## After the checkpoint

Selected follow-up completed: `metal-backend` uses common contracts and native
Metal 4 execution. Shared checks/retirement, explicit backend identity,
format-tagged native artifacts, optional translation, argument tables, residency
sets, command allocators and explicit barriers are implemented and validated on
Apple M4 at `8704f62`. A subsequent focused review identified cross-submission
barrier scope, missing commit-error feedback, nullable internal allocations and
allocator retention by consumed batches. Corrections and regression tests passed
native revalidation on Apple M4, followed by the pinned GGML DEVICE/F16 lifecycle,
mixed-matrix and six-case dataset acceptance. See [the native acceptance record](metal.md).
The historical Vulkan milestones above remain evidence for their recorded revisions.

Refactor regression receipt (2026-09-16): 37 ordinary Rust tests, Clippy, 749
C/Rust layout checks and the mock-loader checks pass. All 21 Vulkan GPU tests
pass on Radeon/RADV and llvmpipe with validation. On llvmpipe, GGML DEVICE/F16
acceptance and libplacebo SDR/two-frame and nine-case HDR acceptance pass after
the ABI migration (HDR intermediate/final results match exactly).
Apple-target Rust checks pass with and without the SPIR-V adapter, including the
native test code, using `DOCS_RS=1` to skip the cross-compiled C++ dependency.
No native Apple linking, MSL compilation or GPU execution was possible here.

Metal 4 review-fix receipt (2026-09-16): 37 ordinary Rust tests, Linux Clippy,
21 llvmpipe GPU tests with validation, and GGML DEVICE/F16 lifecycle, two matrix
cases and all six inference cases pass. Apple-target Rust checks include the new
tests with and without the SPIR-V adapter. These are Linux regressions and
cross-checks, not native evidence for the corrections.

## Completed follow-up: executable numerical requirements

The matrix experiment now compares paired FP16 products and FP32 accumulation
against a matching FP32 implementation. The local Radeon supports
`shaderFloat16`, but exposes no cooperative-matrix extension and reports no
accelerated integer dot products. Numerical/memory gates pass on Radeon and
llvmpipe, but this candidate is about 3–9% slower in measured Radeon device batch
time. Retain optional FP16 enablement and caller-owned requirements; do not adopt
the candidate as a default or change GGML arithmetic. No new C API or ABI change.
The [brief and result](ml-executable-requirements.md) record the predeclared
contract, measurements, regression coverage and the release-library runner fix.
No additional hardware or new Mac acceptance round was needed.

This experiment is closed. A further matrix-acceleration profile needs actual
shape/type/subgroup evidence; additional numeric types or tuning are not selected
automatically.

## Completed follow-up: compiler-facing workflow

The [bounded compiler workflow](compiler-workflow.md) is complete at `66854db`.
Pinned Slang reflection plus checked SPIR-V generates the C root layout, embedded
artifact, local dimensions and enabled-capability predicate for the existing
integer transform. The unchanged host consumer passes on Radeon and llvmpipe
after reordering shader fields and changing local size. Sixteen generator tests,
no-GPU reproduction/build, original compute controls, 37 ordinary Rust tests,
Clippy and 749 ABI checks pass. No runtime/public API change or Metal round trip.

Keep compiler-derived mechanics outside the runtime and application-owned memory,
synchronization and numerical policy explicit. This is not a general compiler
framework or a source-language selection. The experiment is closed; broader
shader types/workloads and graphics portability remain separate milestone choices.

## Completed milestone: one mixed ML/graphics application

The [learned-image flagship](learned-image.md) is complete at `21dc090`: a small
residual denoiser, conventional resize/color processing and offscreen rendering,
with GPU-resident intermediates and repeated allocation reuse. The project-owned
training/reference fixture was frozen first; the Vulkan path passes 38 cases in
each of final-only and diagnostic modes on Radeon and llvmpipe. It meets the
unchanged quality, numerical and lifecycle gates with no runtime/API change,
intermediate representation copy, new hardware or Mac round trip.

## Completed milestone: compiler workflow for the application

The [compiler migration](learned-image-compiler.md) is complete at `e363377`.
Six Slang executables use generated C roots, stage/launch requirements and embedded
artifacts. All 38 cases pass on Radeon and llvmpipe, in normal/diagnostic modes,
with both original interfaces and reversed-field/32-thread variants built from
unchanged host source. Outputs are byte-identical between interface variants.
The frozen model and acceptance gates are unchanged. Duplicated application GLSL
and handwritten host interfaces were removed; runtime/API remains unchanged.

## Completed follow-up: independent Linux consumption

Completed at `6d34967`: a revision-identified local Linux installation, standalone
quickstart, optional installed shader generator and an out-of-tree build/run gate
consuming only installed files. A clean-revision package built in a separate Cargo
target directory passes relocation, space-containing paths, pkg-config discovery,
actual dynamic-link resolution, absent-loader rejection and original/regenerated
shader execution on Radeon and llvmpipe. Initial consumer builds need neither
Cargo nor a shader compiler. Existing installations are never merged/overwritten.

The [acceptance receipt](results/installed-sdk-2026-09-17.txt) records exact scope,
hashes and retained artifacts. Six installer safety tests, 37 ordinary tests,
Clippy, formatting and 749 ABI checks pass. A no-GPU hosted CI lane is defined but
has not been run remotely here. No new GPU abstraction, registry publication,
portable binary release or API stabilization. This validates a dependency boundary,
not actual independent adoption or a clean-machine installation.

The [quickstart](quickstart.md) owns the consumer instructions. Preserve matching
header/library/artifact revisions and the documented modern Vulkan requirements.

## Separate track: the same application on Metal graphics

[Roadmap M6](roadmap.md#m6--metal-parity-for-a-mixed-application) brings the same
application to Metal graphics when native validation is available. Start with the
executable/artifact boundary and the narrow compute-to-render-to-readback path
this application needs, not a general renderer. Preserve its fixtures, explicit
ownership/dependencies and generated-interface boundary. Native acceptance remains
necessary before claiming support; Linux checks cannot substitute for it. This
track does not block a Vulkan-scoped experimental release. No machine provisioning
or new Mac coordination is selected by this plan.

Any new experiment needs a named design decision, alternatives, a discriminating
check and a stopping condition. Pure tuning needs its own scope. Stabilization
requires stronger portability, safety/contract and compatibility evidence; see
[the gates](design.md#evidence-required-before-stabilization). Another physical GPU
or backend may eventually supply that evidence, but is not a prerequisite for
finishing the selected local checkpoint.
