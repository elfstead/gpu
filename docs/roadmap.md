# Roadmap: from runtime candidate to a usable GPU programming foundation

Proposed 2026-09-18; sequence revised 2026-10-30. The [working plan](plan.md) owns completed/active status;
this document owns the sequence and coverage of the remaining project work.
**C1, C2 and C3 Vulkan/Linux acceptance complete. Active: remaining M8,
then M9 → M10 → M11. External backend acceptance remains open.** C1–C3
are defined [below](#minimal-core-revision). Windows correctness fixes can proceed
independently; unavailable Windows/Mac acceptance does not block later local
coverage. M6 stays deferred until a Mac is available; M7 remains ongoing.
The DX12 branch is implemented through reported scene execution but remains
unmerged with [open review findings](dx12-review-handoff-2026-10-30.md).

Earlier sequence checkpoints: on 2026-10-08 the [whole-API design](whole-api-design.md) selected a
coordinated foundation revision before completing the remaining M4/M5 consumers.
As of 2026-10-13, M4/M5 and [foundation-contract consolidation](core-contract-review.md)
are complete for their declared scopes. Sequence revised 2026-10-15: next is
[P1 presentation](presentation-plan.md), followed by [M6](m6-entry.md), now including
presentation. P1 opens with extraction of the native Rust core below the C ABI;
native Rust, optional Wayland setup and bounded swapchain lifecycle execution are
implemented. The textured Rust scene passes bounded multi-slot rendering and resize
checks. As of 2026-10-17, isolated native Rust packaging passes and human checks
confirm controls, resize, pause and close; resume and real minimize/restore remain
unverified. M6's [native mapping proposal](m6-native-mapping.md) is drafted;
foundation backend implementation and native acceptance have not started.
By user decision on 2026-10-17, defer M6 implementation until a Mac is available
and proceed to M8, starting with block-compressed textures. M6 stays incomplete.
M8–M11 below schedule the remaining hardware
families; completed M1–M5 gates are not reopened.
The 2026-10-17 [DX12 candidate handoff](dx12-candidate-plan.md) adds a parallel
Windows-agent implementation branch, followed by native evidence and review.
It does not replace M6, block local M8 work or declare Windows support in advance.
This is a concrete plan, not authorization to publish releases, provision machines
or coordinate native testing. The foundation can be implemented across the
milestone boundaries below; each capability does not need its own research gate.

## Target and priorities

Deliver a low-level foundation that an independent application can use for
graphics, general compute and ML, with explicit memory/dependencies and compiler-
supplied executable interfaces. Demonstrate useful-sized applications, not only
small correctness fixtures. Keep the C boundary language-neutral and the Rust
runtime free of tensor operators or an application scheduler.

Do not measure completion by matching every Vulkan/CUDA feature. Broader graphics,
numeric profiles and concurrency need explicit native mappings and verification;
named consumers exercise the integrated result but are not prerequisites to
designing or implementing a clearly useful hardware capability. Apply the
[minimal composable core rule](design.md#minimal-composable-core): prefer one
mechanism, derive user-level alternatives, and add another primitive only for a
concrete capability or efficient-execution gap that composition cannot resolve.
Native API vocabulary and existing implementation weight are not reasons to keep
duplicate mechanisms. API changes
are welcome when evidence exposes a better API alternative, whether or not the
existing API could be made to work. Do not add legacy fallback implementations.

Current base: Vulkan compute/images/offscreen raster, native Metal 4 compute,
bounded GGML/libplacebo consumers, a learned-image application with generated
interfaces, and a revision-pinned Linux SDK. These are completed checkpoints,
not new tasks to repeat. Source-language design, platform breadth and stable
compatibility are not completed by those results.

## Sequence and acceptance

| Milestone | Concrete outcome | Dependency / execution location |
|---|---|---|
| M1 — Useful-scale execution | Learned-image workload at video-sized extents, bounded memory, matched Vulkan measurements | Complete; Radeon measurements and smaller llvmpipe correctness controls |
| M2 — Compiler/programming contract | Documented device-code contract; generated structured arguments and heap interfaces; explicit language-direction decision | Complete 2026-09-26; [pinned Slang + contract](language-direction.md), not a new frontend |
| M3 — Resource and submission maturity | Sustained multi-frame reuse, measured submission costs, consumer-side allocation assistance, better diagnostics | Complete 2026-09-28; [installed handoff](resource-reuse-handoff.md) closes the bounded reuse brief |
| Foundation revision — Whole-API contract and implementation | Explicit lifetimes, placement, concurrency, queues/timelines, argument alternatives and coherent graphics/ML capability surface | [Setup/backing/commands/images/descriptors, compute and offscreen raster implemented](foundation-implementation.md): prepared executables and caller-owned caches, inline/device-root arguments, mixed heaps, draw variants, multisampling/color resolves, depth/stencil, explicit queries and image copies; broader profiles and consumer cutover remain |
| M4 — Substantial graphics consumer | Textured scene with depth, indexed geometry, mipmapped sampling and blending; separate presentation boundary | Offscreen complete at `43bb9f0`; [native-matched and independent SDK acceptance](textured-scene-plan.md#clean-m4-offscreen-acceptance); presentation separate |
| M5 — Substantial ML consumer | One specified transformer block and one quantized linear variant through a broader GGML subset | Complete: [clean SDK acceptance and cost/error/storage observations](m5-results.md); no general LLM or matrix-acceleration claim |
| Foundation-contract consolidation | Native strategy, ownership/safety and remaining-gap dispositions | Complete: [review](core-contract-review.md); not blanket performance or stabilization approval |
| P1 — Native Rust and interactive graphics | Shared Rust core with thin C adapters, standalone Rust consumer, optional Linux presentation and interactive scene | Native Rust, swapchain lifecycle, textured three-slot scene and isolated packaging pass; [human acceptance partial](presentation-plan.md), resume/minimize/restore unverified |
| C1–C3 — Minimal composable foundation revision | Derive the minimal contract, revise runtime/tooling/consumers together, then accept available native mappings | C1 selected/consolidated; C2 [public argument cutover complete](minimal-core-migration.md#c2-argument-public-cutover) on Vulkan 2026-11-03; public descriptor/secondary cutover complete 2026-11-05; generated argument layouts complete; public dependency cutover complete; public unified submission/outcomes complete; public presentation/frame-loop cutover complete with bounded Wayland checks; C2 audit and [local C3 acceptance](results/c3-linux-acceptance-2026-11-06.md) complete |
| M6 — Mixed-workload Metal parity | Foundation migration, learned-image/heap consumers, generated interfaces and P1 interactive presentation | [Native mapping proposed](m6-native-mapping.md); implementation deferred until a Mac is available |
| DX12 candidate — Parallel native backend | Correct native Windows mapping of the selected foundation and compute/offscreen/presentation acceptance | Implemented candidate with reported Windows scene execution; [review corrections](dx12-review-handoff-2026-10-30.md) and C1 contract decisions remain before acceptance/merge |
| M7 — Experimental release and adoption | Versioned source/install workflow, contract audit, reproducible consumer acceptance | Ongoing packaging track; first [checkpoint](release-checkpoint.md) complete; refresh with new capabilities, publication/adoption separate |
| M8 — Advanced raster and GPU-driven execution | Block compression, task/mesh, generated commands and justified advanced raster primitives | Active after local C3; compressed, task/mesh and initial generated-command tranches implemented; fixed-executable ordinary/indexed/nested-count graphics/culling passes; vertex/index-state tokens now pass locally in revision 8; richer consumer and advanced raster remain |
| M9 — Ray tracing | Acceleration structures, ray queries, ray pipelines and shader tables | After M8 by default; shares memory/executable requirements, native validation may remain open |
| M10 — Memory and interoperability | Mixed backing, native resource/synchronization sharing, VM/sparse memory and residency | After M9 by default; pull prerequisites into P1/M6/M8/M9 when needed |
| M11 — Advanced compute and media | Richer launch/artifact/numerical profiles, hardware matrix consumer and video sessions | After M10 by default; compute and media are independently accepted subprofiles |

Current local implementation order: **completed M1–M5/foundation and consolidation →
P1 (partial human acceptance) → C1 → C2 → local C3 → remaining M8 → M9 → M10 → M11**, with **M6 deferred until
a Mac is available** and **M7 an ongoing packaging/adoption track**,
not the end of functional development. Prerequisites can move earlier without
claiming the entire later milestone. Missing Mac or feature hardware must not
stall useful local work: keep native acceptance open and advance the next
implementable batch. A Vulkan-scoped experimental package does not require Metal
or M8–M11 acceptance; native validation remains a separate evidence track.
No calendar estimates are assigned by this document.
Each milestone ends in a committed result and decision, not another open-ended
list of experiments.

## Minimal-core revision

Execution decision, 2026-10-31: **Vulkan/Linux first.** Work on `minimal-core`,
branched from Vulkan `master` (`2374635`), carrying the design/review documents but
no DX12 implementation commits. Keep `dx12-backend` as the implementation reference
and future Windows handoff. Its reviewed mapping constraints inform C1; updating
or accepting that backend is not a prerequisite for C2 or local C3.

C1–C3 revise the foundation after DX12/NGPU review and the 2026-10-30 design
decision. They do not restart M1–M5, erase accepted consumer results or make the
existing public header stable. Do not add parallel native profiles simply to
preserve every earlier representation. Keep hardware capability coverage while
reducing mechanisms that users can compose efficiently themselves.

### C1 — Derive and select the minimal contract

Started 2026-10-31: [first contract draft](minimal-core-contract.md), whole-surface
dispositions, public user compositions and a DX12 mapping table are written.
On 2026-11-03, [A/B argument updates and reuse](minimal-core-arguments.md) select
one range update, atomic native address fields and explicit storage/compatibility
metadata, with C/Rust record sketches. [D descriptor storage](minimal-core-descriptors.md)
also selects native-unit locations, explicit backing/copy rights and accessible
native spans, with record sketches. [P presentation lifecycle](minimal-core-presentation.md)
selects borrowed frames, native availability and explicit acceptance/resize effects.
[Dependency/submission decisions](minimal-core-dependencies.md) preserve justified
ranged/split/ownership scopes and select one submission operation with outcomes.
The [combined surface and migration inventory](minimal-core-migration.md) now cover
all installed operations and cross-family records. **C1 complete, 2026-11-03.**
This is a selected foundation design, not implemented support or native acceptance.

Produce one revised whole-API draft, including C/Rust operation correspondence,
shader/artifact ABI, lifetimes, observation points, invalidation, capabilities and
failure outcomes. Start with one logical argument interface, one descriptor-storage
model and one command model. These are design candidates, not a decision to force
pointer-only roots, CPU-only descriptor mutation or one-shot commands.

Maintain a disposition table for existing and proposed mechanisms: **core
primitive / user composition / capability-dependent primitive / unresolved**.
Every additional primitive needs a concrete counterexample to efficient composition;
every removal needs a replacement recipe. Each row records native mapping,
unavoidable work, safety obligations and any remaining validation question.

| Decision | Starting point and required analysis | Existing assumption to reconsider |
|---|---|---|
| Arguments | One logical root; show independent data groups, changing scalars, GPU-written data and repeated launches. Compare inline versus referenced observation/lifetime semantics and native access. | Multiple roots, separate binding APIs and a universal byte bank are not independently mandatory. Pointer-only roots are not preselected either. |
| Descriptors | One explicit storage/update/bind model; derive slot allocation, caching and transient tables. Check GPU-selected copies, placement and disjoint pending updates. | Raw bytes and opaque heaps need not both be public. Preserve needed capabilities through the smallest sufficient mechanism. |
| Commands and dependencies | One command-storage model serving recording/reuse; preserve GPU-selected work and needed scheduling freedom. Show fixed inference replay and a fragment-only cross-queue dependency. | Do not duplicate command models for convenience. Global versus ranged barriers, exclusive sharing and layout controls require individual justification. |
| Presentation | Independent surfaces with explicit availability, completion, reuse and error semantics; derive a single-window loop and multi-window use. | Do not copy every Vulkan lifecycle operation or add a backend-specific public profile by default. |
| Memory, transfer and failures | Explicit placement/requirements, representable copy footprints and honest submission outcomes; derive staging, reuse and deferred deletion outside core. | Automatic residency is not rejected merely because it is automatic; establish whether caller control is needed. No silent copy expansion or invented atomicity/retry guarantees. |

Show user-side recipes through the public contract for an argument arena, descriptor
allocator, repeated inference, GPU-culling/rendering and a multi-frame/multi-window
loop. Reuse existing consumers as evidence where applicable. These are composition
examples, not a commitment to ship general allocator/render-graph/ML libraries.
Compare Vulkan and D3D12 source mappings plus documented Metal constraints; Mac
execution is not a prerequisite for selecting a proposal with explicit unknowns.
Maintain a compact DX12 mapping table for each selected primitive, its native
constraints and unresolved questions. Do not emulate DX12 on Linux or infer
native acceptance from source inspection.

**Exit:** one chosen contract and migration/deletion map, exact remaining questions
and targeted checks. Mark proposals separately from implementation. Structural
reasoning is sufficient for clear cases; do not insert a benchmark gate for each
operation. Keep already justified hardware batches in the complete draft rather
than restarting an experiment-per-feature design process.

### C2 — Implement one coordinated foundation revision

Completed for Vulkan/Linux, 2026-11-06; see the [installed contract/deletion audit](results/c2-contract-audit-2026-11-06.md). Historical [cutover sequence](minimal-core-migration.md#c2-implementation-sequence).
The argument and descriptor families are implemented through public C/direct Rust,
compiler metadata and consumers under foundation schema 7. Old argument and
descriptor adapters are deleted. The [descriptor cutover](minimal-core-migration.md#c2-descriptor-public-cutover)
includes native-unit locations and secondary heap inheritance. The
[generated-layout cutover](minimal-core-migration.md#c2-generated-argument-layout-cutover)
also passes native compute execution with explicit layouts and address tokens.
The [public dependency cutover](minimal-core-migration.md#c2-dependency-public-cutover)
exposes independent scopes, one caller scratch span and enabled-domain limits in
C/direct Rust. Old dependency translators and nested scratch paths are deleted;
consumers pass through the selected records. The [public submission cutover](minimal-core-migration.md#c2-submission-public-cutover)
adds one counted C/direct Rust call, explicit acceptance outcomes, direct binary
arrays and one scratch span. Old single/batch/binary adapters are deleted and
consumers migrated. C2 step 4's [public presentation cutover](minimal-core-migration.md#c2-presentation-public-cutover)
now exposes borrowed frames and explicit lifecycle effects in C/direct Rust. The
old index-based wrappers are removed; two-window and three-slot consumers pass
bounded Wayland checks. The installed-surface and deletion audit completes C2
locally; [C3 Linux acceptance](results/c3-linux-acceptance-2026-11-06.md) is complete. C1 drafts remain historical
design evidence; the installed header/core are the implemented contract.

Update the shared Rust operations, thin C adapters, Vulkan mapping,
generated Slang interfaces, installed headers and consumers as one coherent batch.
Use the DX12 mapping table to check the design without importing its backend into
this branch. Defer DX12 implementation updates to a later native Windows handoff;
R2/R3/R8 then use the selected contract. Correctness work on that branch can proceed
independently and does not gate the Linux revision.

Move convenience policy into consumers/helpers and delete redundant production
paths once replacements work. Do not retain a second implementation merely to
preserve an experimental signature. Preserve the separate legacy Metal path until
its replacement has native acceptance; it must not constrain the new foundation.
Document source/artifact migration and capability differences. No hidden emulation
or capability loss may be disguised as simplification.

**Exit:** the selected surface is implemented for Vulkan/Linux;
changed host/ABI/compiler/lowering checks pass, affected Linux consumers work and
the mapping/migration table identifies deferred Windows work. This is an implementation
checkpoint, not an assertion of Windows or Metal acceptance.

### C3 — Accept mappings and refresh the consumer handoff

**Vulkan/Linux complete 2026-11-06:** [clean revision-7 receipt](results/c3-linux-acceptance-2026-11-06.md), including isolated C/native Rust SDKs, GGML/libplacebo and bounded Wayland checks. Windows/Metal remain unaccepted; remaining M8 resumes.

Run the changed-contract tests and the relevant offscreen, ML, replay, GPU-driven
and presentation regressions on available hardware. Inspect mandatory work first;
measure only unresolved tradeoffs. Refresh the isolated C/native Rust SDK workflow,
support matrix and disposition table. These local checks do not require a Windows
implementation update. Later Windows acceptance requires native validation and
closure of each DX12 review finding; Mac acceptance remains M6.

**Exit per backend:** exact revision, contract checks, native evidence and unresolved
limits are recorded. Linux completion permits resuming M8 while external backend
acceptance stays open. Do not mark all C3 complete from Linux results or block the
rest of the roadmap on an unavailable machine. No new GPU is assumed.

Historical sequence clarification 2026-09-30 (ordering superseded above): finish M4's **draw-contract decision** before
the remaining graphics slices; begin M5 with an executable-capability/numerical
contract, not kernels alone. After the consumer milestones, complete the
[core-contract review](core-contract-review.md) before considering stabilization.
Experimental packaging may continue earlier. Consumer usefulness, native strategy
preservation and stabilization are separate gates; none silently passes another.

Historical sequence amendment 2026-09-19: the [performance-expressibility gate](performance-expressibility.md)
pulls the contract audit, repeated small-dispatch and streaming-slot experiments
forward from M3 before M2 implementation. M1's matched-policy result remains
accepted; it does not settle whether a stronger native strategy is expressible.
This gate applies to M2's compiler work and later graphics/ML contracts as well.
The [small-compute](performance-frontier-small.md) and
[streaming](performance-frontier-stream.md) probes are complete (2026-09-20).
They expose a storage-lifecycle opportunity and demonstrate independent slots;
the [storage-retirement alternative](command-storage-review.md) subsequently passed
its first [bounded-cache experiment](command-storage-results.md), then the
[explicit caller-owned storage comparison](recording-storage-results.md).
[Fixed-command replay](command-list-results.md) and
[direct HOST access/independent ranges](host-view-results.md) subsequently passed
bounded acceptance at ABI 15/16. The subsequent
[dependency-scope review](dependency-scope-review.md) exposed missing scheduling
freedom; [public split endpoints and explicit replay mode](split-dependency-results.md)
pass bounded acceptance at ABI 17. Proceed to M2's compiler/programming contract.
Changing commands/timed replay, total command-memory budgets, noncoherent hardware
and the other expressibility concerns remain explicit rather than being passed by
these workloads. Backend encoding optimization is distinct from a contract decision.

## M1 — Make the existing application useful-sized

**Deliverables**

1. Extend learned-image execution to 1280x720, 1920x1080, 3840x2160 and an odd
   1919x1079 input, with both upscale/downscale processing. Keep the 89-parameter
   model, arithmetic policy and existing small fixtures frozen.
2. Remove the harness's 1024-per-dimension ceiling and single-X launch limitation
   using checked size arithmetic and a documented grid/tile mapping within queried
   limits. The runtime already supports XYZ dispatch; a larger image is not by
   itself evidence for a new dispatch API.
3. Add a bounded-memory reference path. Cross-check any accelerated scalar FP64
   reference against the existing Python oracle on all small cases; stream/chunk
   large reference work so Python object arrays and diagnostic dumps do not
   dominate memory. Check full intermediate/final outputs once per correctness
   case, separately from repeated timing frames. Define border/halo rules before
   using chunks; do not silently substitute sample-only checks.
4. Add resident and end-to-end timing modes with explicit setup, upload, command
   recording, wait/readback and device-batch accounting. Use 10 warmups and 30
   measured frames per mode, three separate Radeon runs; report distribution and
   peak allocated bytes, not just a best FPS number. Validation runs are separate.
5. Implement a benchmark-only native Vulkan control using the same generated
   SPIR-V, algorithm, memory placement, stage boundaries, transfers and queue
   policy. If a native optimization changes those conditions, label it as a
   separate comparison, not isolated API overhead.

**Done when:** existing gates remain unchanged; all declared extents pass on Radeon;
small controls pass on llvmpipe; reuse/memory use is bounded; measured costs identify
the next bottleneck. No mandatory speedup and no claim of photographic quality.
Record failures before revising a bound. If an extent exceeds a reported device
limit, it is an explicit unresolved case, not a passing skip.

**First implementation slice:** checked extents, generated multidimensional launch
mapping, and full-reference correctness at 720p/1080p. Then complete 4K/odd extents,
then measure/control. No kernel tuning campaign or public API widening first.

Progress 2026-09-18: [both correctness slices accepted](learned-image-scale.md),
first at `86dd16e`, then 4K/odd-ratio coverage at `6d36e93`. All six declared
scale groups pass full intermediate/final checks and real multi-row dispatch.
A first-slice resize precision defect was corrected with unchanged gates; no
further shader/runtime changes were needed for 4K or non-integer ratios.
Progress 2026-09-19: the [corrected warmed OGPU baseline](learned-image-measurement.md)
is accepted at `958b831`, with resident/end-to-end distributions, all 720 samples
and separate allocation/free traces retained. An odd-edge fragment-pointer read
defect was fixed and full small/scale regressions pass. At that checkpoint the
matched native control remained pending; OGPU-only clocks do not identify API overhead.

Native-control progress 2026-09-19: [workload correctness](learned-image-native-control.md#accepted-workload-correctness--2026-09-19)
is accepted at `219254e`: generated compute/raster output and allocation-policy
parity pass on the selected small/large cases. That checkpoint did not yet include
native warmed timing.

**M1 complete — 2026-09-19:** at `a819f6c`, the
[fresh matched comparison](learned-image-native-control.md#accepted-comparison-and-m1-decision--2026-09-19)
passes: 1,440 retained measured samples, 16 separate allocation controls, and
fresh full-output validation on Radeon/small llvmpipe. Native/OGPU allocations
match and are freed; median latencies are close across the eight selected cases
(OGPU 0.04% lower to 1.14% higher), with explicitly retained tail differences.
This is workload/policy/device-specific latency evidence, not isolated API cost.
Retain the runtime/API and one-shot execution model. Transfer/synchronization
dominates the mode difference; resident latency is mostly GPU work. Earlier
six-group correctness plus this comparison satisfies M1; M2 is next.

## M2 — Define the programming contract and broaden compiler tooling

**Deliverables**

1. Write a device-code contract describing address spaces, scalar widths and
   alignment, structure/array layout, root versus pointed-to data, allowed aliasing,
   workgroup memory/barriers, atomics/subgroup requirements, numerical permissions,
   and compute/graphics stage interfaces. Distinguish existing runtime guarantees,
   compiler conventions, required capabilities and intentionally undefined behavior.
2. Extend the compiler adapter for concrete nested structs, fixed arrays, vectors
   and FP32 scalar arguments used by the applications; add a reflected pointed-to
   argument block fixture. Generate explicit padding and reject unsupported layouts.
   Do not assume a host C struct matches a shader vector layout by accident.
3. Bring image/sampler heap shaders into the generated workflow and add stage
   input/output checks needed by M4. Resource indices remain non-owning values;
   descriptor contents, formats, bounds and heap mutation policy remain explicit.
4. Turn current single-file source compilation into an application build step
   with explicit inputs/outputs and declared include/module dependencies. Detect
   stale source/dependency artifacts. Do not invent a general binary package,
   disk cache or runtime reflection system unless needed by the selected sources.
5. Decide the language direction in a committed design record: continue with
   Slang plus an OGPU device contract, add a small supported library/profile, or
   build a project-owned frontend. Compare expressiveness for pointer structures,
   reduction/subgroups, matrix kernels and graphics with their actual compiler
   output and host integration. If existing tooling is sufficient, say explicitly
   that a new language is not being built. If a frontend is selected, it becomes
   its own scoped milestone before claiming a general new language.

**Done when:** compute, structured-pointer and image/graphics examples build through
the installed tool; field/nesting/local-size mutations preserve unchanged semantic
host code; malformed layouts, missing requirements and stale dependencies reject;
existing M1 outputs remain valid. Publish the supported subset and restrictions.
Native Metal generation is M6, not inferred from SPIR-V success here.

Accepted [language direction and evidence](language-direction.md) complete M2.
The original workgroup/subgroup compiler probes did not widen the legacy installed
profile. Foundation now exposes explicit subgroup queries/requirements and matrix
tuple discovery. M5 deliberately uses ordinary workgroup FP32/storage16; its
consumer acceptance is not accelerated matrix or general subgroup acceptance.

This decision point prevents the initial language ambition from being silently
replaced by a runtime, while also avoiding a new frontend merely to rename Slang.

## M3 — Resource reuse, submission costs and diagnostics

The [execution and acceptance brief](resource-reuse-plan.md) selects the existing
learned-image stream and separates already accepted frontier work from the new
range-helper, sustained integration, diagnostics and independent-handoff gates.

**Deliverables**

1. Run 1,000 A/B frames with two and three in-flight slots against a single-frame
   reference. Exercise wraparound, premature-reuse rejection, failure/drain paths
   and terminal retirement. Keep output checks enabled outside timing runs.
2. Add a consumer-side arena/range helper and completion-associated reuse example;
   measure allocation count and peak bytes against dedicated allocations. Keep
   pointer ownership explicit. Decide separately whether any runtime allocation
   primitive would be better; do not silently move an allocator into the runtime.
3. Use the performance-expressibility controls to choose the submission change.
   Compare reusable native command sequences with the best expressible one-shot
   strategy, including host-sensitive workloads, explicit argument updates and
   resource lifetimes. A GPU-heavy matched-policy result cannot close replay.
   Separate backend pool reuse from public repeated-encoding costs. Adopt a better
   contract when exposed; no mandatory copy of a native API abstraction.
4. Add application/executable/batch labels and a reproducible failure report
   containing revision, backend, enabled requirements and failing operation.
   Start with SDK/consumer diagnostics; extend runtime diagnostics only where it
   provides otherwise unavailable information. Add an allocation/resource-lifetime
   trace for this workload, not a full GPU debugger.

**Done when:** long runs have bounded live memory and objects, slot reuse agrees
with the serial reference, relevant injected failure paths drain safely, and the
submission/allocator decision has measurements. Caller obligations are documented
alongside helpers. No cancellation or automatic dependency inference is implied.

Multi-queue scheduling, concurrent host recording and external memory are not
hidden inside this milestone; their entry criteria are below.

M1 input to this decision: matched host recording+submission medians overlap
(native roughly 74–116 microseconds, OGPU 78–107), while resident GPU work and
end-to-end transfer/synchronization dominate. Begin with sustained slot/staging
reuse, but examine stronger native strategies before retaining one-shot recording
as a fundamental contract. Retain M1's host-tail observations for diagnosis; they do
not establish a driver/scheduler cause or mandate a replay API.

## M4 — Implement a meaningful graphics subset

**Consumer:** a small deterministic textured scene, initially offscreen. Use
overlapping geometry for depth, shared indexed geometry, a mipmapped texture and
a transparent overlay. Produce reference frames and explicit feature probes.

**Deliver in slices:** (a) indexed geometry plus depth attachment/state;
(b) mip levels, views and sampled-image selection;
(c) blending and configurable viewport/scissor; (d) narrow inter-stage varying
reflection/link checks. Specify formats/usages and state before each slice.
Use a matched native Vulkan rendering of the same scene as a control and
analytical probes for depth/blend outcomes. Predeclare pixel tolerances for
filtering and edge coverage; do not demand unrealistic universal bit equality.

Slice (a) ends in an explicit retain/change decision for indirect ranges,
GPU-generated counts and shader-visible draw identity. Matched timing measures
costs, but is not a prerequisite for adopting a better contract justified by a
structural restriction. Preserve the strongest identified native strategy rather
than treating N single-record commands as the only baseline. See the
[decision and measurement brief](indexed-depth-performance-plan.md).

**Done when:** the scene and deliberately invalid combinations pass/fail as
specified, allocation reuse and compute-written inputs remain valid, and the
learned-image path has no regression. No generic fixed-function state object
copied wholesale from Vulkan is assumed as the final design.

**Presentation follow-on:** now scheduled as [P1](presentation-plan.md), before
M6. First extract the native Rust core and validate both host interfaces; then
implement one Linux window-system path and an interactive Rust consumer; native
display testing is an acceptance requirement, not a prerequisite for design or
implementation. Offscreen M4 remains complete. Native window concepts stay out of
the compute core.

## M5 — Implement a broader, still bounded ML consumer

**Consumer:** one fixed transformer block with residuals, normalization, attention,
softmax and feed-forward computation, through a declared GGML operator/layout
subset. Select exact shapes and reproducible weights in its initial brief; include
odd dimensions and more than one sequence length. This is not a general LLM backend.

**Deliverables:** CPU-reference-tested kernels for that block; FP32 baseline and
FP16 weight storage with explicit FP32 accumulation; a separate signed-INT8-weight
linear variant with defined scale/dequantization and error policy. Keep all
intermediates resident and scheduling in the consumer. Use independent CPU
references and predeclared tolerances, not only final classification agreement.
Quantized arithmetic may be implemented with ordinary instructions on the Radeon;
do not claim accelerated dot products when none are exposed.

**Opening gate:** specify queried/enabled executable requirements for the selected
workgroup/subgroup operations, layouts, scalar/storage types, accumulation and
numerical permissions. Add unsupported/mismatch rejection and generated-interface
checks before selecting kernels that depend on them. Separate tooling restrictions
from runtime/backend restrictions. No implicit subgroup size, matrix support or
precision permission follows from the backend version. Hardware matrix variants
remain separate from the baseline consumer.

**Done when:** the complete block executes without CPU operator fallback, all
intermediates meet the declared numerical contract, supported/rejected layouts
are explicit, and throughput/memory/error tradeoffs are recorded. No automatic
selection of reduced precision merely because a device capability bit is true.

**Accelerated matrix profile:** specify shape/type/subgroup/layout requirements
and variant selection using this workload. M11 schedules implementation of the
hardware matrix consumer; native acceptance requires supported hardware. The local
Radeon's lack of exposed cooperative-matrix support cannot be fixed by host API
design; mocks are rejection tests, not acceleration evidence.

## M6 — Metal parity for a mixed application

The [native mapping proposal](m6-native-mapping.md) supplies the first design
checkpoint: native argument profiles, explicit residency, conservative one-shot
scope pending replay guarantees, and native queue/presentation semantics. These
are design dispositions, not support claims or a completed migration.

Target the learned-image application first, not every Vulkan graphics feature:
RGBA8 offscreen targets, compute-to-fragment address reads, indirect fullscreen
draw, transfer/readback, preservation and compute/render dependencies. Add image
and sampler heaps next using the existing heap-image consumer. Treat optional
GPU timing as an independently supported capability.

Generate native MSL/metallib host interfaces and check native argument layouts
against their actual compiler artifacts; do not assume the Vulkan generated layout
is universal. Keep source semantics common where possible, artifact metadata
backend-specific where necessary. Preserve explicit ownership and native Metal 4
encoding rather than mimicking Vulkan internals.

Then map the [P1 presentation contract](presentation-plan.md) to native Metal
drawable/layer behavior and run the interactive consumer on a Mac. Window/input
creation remains external; acquisition and presentation completion must retain
their native semantics. Pull explicit residency/interop prerequisites forward
from M10 where needed, without treating that as whole-package acceptance.

**Done when:** native Mac execution passes the same fixtures, reuse/error tests
and generated-interface mutations, plus interactive multi-frame presentation,
resize/minimize/restore and safe teardown. Linux type checks/mocks can prepare a handoff
but leave this milestone incomplete. Batch native validation into a coherent
handoff when a Mac is available; do not request a round trip per small change.
Any subsequent M4 feature claims require their own native acceptance.

## M7 — Experimental release, then stabilization evidence

M7 is an ongoing track refreshed after P1, M6 and later capability batches. It is
not a claim that the full hardware coverage roadmap is finished. Publication and
stabilization remain separate decisions.

After M1–M3, prepare a Vulkan-scoped source release candidate with the working SDK,
installation/upgrade instructions, current compiler requirements, examples,
compatibility rules and support matrix. Keep Metal compute support and missing
graphics explicit; waiting for M6 is not a Linux release requirement. Audit public
ownership, error/unsupported behavior, integer-overflow checks and ABI changes.

Run a fresh-checkout build and relocated consumer test; use a clean host/container
when available and distinguish it from cached local reproduction. Ensure CI
definition and actually observed CI results are reported separately. Add tested
consumer-facing failure cases and an issue template requesting revision, driver,
requirements and minimal reproduction. Publishing/tagging or requesting external
testing requires separate approval; preparing artifacts does not imply publication.

**Done when:** another developer has a self-contained, revision-identified route
to build/run/debug supported workloads and a written upgrade policy. A real
independent integration is sought as evidence, not manufactured by naming our
own copied example independent. Lack of external adopters does not block local
packaging, but it does block claiming that feedback has occurred.

Stabilization is a later decision: broader physical GPU/vendor/memory coverage,
native mixed-backend evidence, performance comparisons and contract tests are
required before stronger guarantees. No stable API promise follows from a tag.
The [core-contract review](core-contract-review.md) is an explicit prerequisite
to considering that decision, not an additional claim made by experimental M7
packaging. Native Metal and broader hardware evidence remain independently scoped.

## Scheduled hardware coverage

M8–M11 are hardware-capability batches; only the explicitly recorded M8 tranches
are implemented so far. Each batch first applies the composition rule to the
C1-selected foundation: separate user/shader compositions from genuinely missing
hardware primitives, with exact native mappings and capability limits. Then implement the coherent API,
backend and compiler/artifact changes together. Consumers verify integration;
they are not prerequisites to implementing an already understood native strategy.
Use structural cost analysis first; measure when a lowering or tradeoff is uncertain,
not as a mandatory research gate for each operation. No silent emulation, hidden
scheduler or mandatory resource walk may substitute for a missing native mechanism.

Track each profile as **specified / implemented / natively validated**, with
backend/device scope. A batch can reach an implementation checkpoint while native
acceptance remains open. Unsupported local hardware is neither a passing skip nor
a reason to leave the contract indefinitely undesigned. Do not label a milestone
fully complete until its declared native acceptance passes; continue other work
when only unavailable external hardware remains. No hardware acquisition is implied.

### M8 — Advanced raster and GPU-driven execution

**Format prerequisite pulled forward (2026-10-16):** the ordinary/packed foundation
catalog now has 118 formats, with explicit numeric/aspect rules and native-capability
queries; [bounded evidence and exclusions](results/foundation-formats-2026-10-16.md).
This is not M8 completion. The [2026-10-17 compressed tranche](results/m8-compressed-formats-2026-10-17.md)
adds BC, ETC2/EAC and ASTC LDR vocabularies (172 total formats), exact block-aware
copy pitches/bounds, view compatibility and native sampled artifacts. Both drivers
execute 16 BC copy/view profiles and two BC1 sampling profiles; the other 38
requested compressed profiles remain locally unsupported. Broader sampling and
block reinterpretation remain explicit follow-ups. No automatic decompression or RGBA expansion.
Audit extension-specific ordinary/packed formats alongside advanced raster.
The [storage-image follow-up](results/foundation-storage-images-2026-10-16.md) also
pulls forward extended typed and formatless shader metadata plus independent
per-format read/write permissions. No new device enables are needed on our Vulkan
baseline. Typed storage uses STORAGE support and compatible shader/view formats;
formatless operations also require their corresponding read/write format flags.
The bounded 2D compiler profile is not every dimensional/atomic storage profile.

The [task/mesh tranche](results/m8-task-mesh-2026-10-17.md) implements separate
mesh/task enabling, physical limits, executable stage composition and native
direct/address-indirect/count draws. llvmpipe passes the initial mesh-only and
task-payload pixel fixture; Radeon has no native mesh support. Physical-GPU
execution/performance, richer stage interactions and the GPU-culled scene remain
open. No fallback or readback-based dispatch is installed. This is not M8 completion.

The [generated-command tranche](results/m8-generated-commands-2026-10-17.md)
implements optional native token layouts, executable sets, caller-owned preparation
storage, explicit dependencies and separate/integrated preprocessing. Both drivers
pass GPU-produced compute choice/arguments/counts and sequence identity across
20 submissions; malformed recordings reject before submission. Graphics/mesh and
nested-count actions have native lowering and layout coverage, not execution
acceptance at that checkpoint. The [2026-11-06 generated-graphics consumer](results/m8-generated-graphics-2026-11-06.md)
and its [indexed/nested-count extension](results/m8-generated-indexed-counts-2026-11-06.md)
now pass all four graphics action forms, GPU culling/compaction/counts/identity,
signed index offsets and native nested-count clamping under both preprocessing
modes on Radeon (48 submissions, 98,304 pixels, guards). Graphics pipeline
selection remains unavailable locally and independently gated. Generated mesh
execution, richer scene/raster interactions and performance evidence remain open.
The [revision-8 buffer-state token tranche](results/m8-generated-buffer-state-2026-11-06.md)
adds GPU-selected vertex/index bindings and explicit static/dynamic vertex stride;
96 combined graphics submissions and 196,608 pixels pass locally. Foundation
revision 9 replaces the exhausted public mask with
[counted feature-ID query/request lists](feature-selection.md), including device,
shader/compiler and native Rust consumers. Native enables stay independent;
private encoding and list preparation are cold.
[Clean C and Rust SDK acceptance](results/m8-feature-selection-2026-11-06.md)
now passes alongside source/ABI and local Vulkan regression.
Revision 10 now implements [multiview](multiview.md): explicit native enabling,
physical limits, pipeline/render/inheritance masks, shader requirements and
[local sparse/offset-layer correctness](results/m8-multiview-2026-11-06.md).
The clean revision-10 C and native Rust SDK checks also pass at `2f316e6`.
Color/MSAA/resolve, generated/mesh multiview and corrected-layer full timestamp
validation remain acceptance work. Revision 11 implements [fragment shading rate](shading-rate.md)
with independent enables, limits/tuples, static/dynamic state, attachment use and
compiler requirements. [Local evidence](results/m8-shading-rate-2026-11-06.md) covers
host/compiler and regression checks; this GPU has no native shading-rate inputs,
so positive rendering acceptance remains open. Clean revision-11 C/GGML/libplacebo
and native Rust/Wayland SDK acceptance passes at `8a57dc4`. [Pass continuation](render-continuation.md) now implements native suspend/resume
through existing flags and ordered submission lists, with [local D32/MSAA/resolve
acceptance](results/m8-render-continuation-2026-11-06.md).
[Framebuffer-local reads](local-read.md) now implement explicit input/output mappings,
input-attachment root/heap bindings and in-render region dependencies. The
[Radeon consumer](results/m8-local-read-2026-11-06.md) passes color plus same-pixel
storage reads with both native layouts and global/resource barriers. Depth/MSAA,
secondary/multiview combinations and generated compiler handoff remain acceptance
work. Clean C/GGML/libplacebo and isolated native Rust/Wayland SDK acceptance
passes at `64e0988`, including the new consumer against the installed release
runtime. [Tessellation/geometry](raster-stages.md) now implement independent enables,
physical limits, patch/origin state, adjacency and shared argument/stage scopes.
[Native acceptance](results/m8-raster-stages-2026-11-06.md) passes ordinary,
adjacency and tessellated/combined pipelines with changed-data replay and statistics.
Clean C/GGML/libplacebo, isolated native Rust/Wayland and installed-release raster
consumer checks pass at `385e215`.
The [textured advanced-raster consumer](../tools/sdk/foundation-advanced/README.md)
now integrates geometry/tessellation with generated indexed geometry, texture
heaps/mips/filtering, depth, blending, scissors and continuation. The [clean installed matrix](results/m8-advanced-textured-2026-11-06.md)
passes 180 frames at 257×193 and 640×360, including continuation and independent
CPU/reference checks. The [M8 coverage audit](m8-coverage.md) schedules the remaining native
raster/fetch controls, distinct raster mechanisms, geometry capture/dynamic patch
state, and contract/composition consolidation before M9. The [first control batch](raster-controls.md) now implements large points,
independent clipping, divisors and list/patch restart; ten supported profiles pass
locally, with patch restart still unavailable. [Clean installed acceptance](results/m8-raster-controls-2026-11-06.md)
passes C/native Rust and the optimized fixture. The [raster mechanisms tranche](raster-mechanisms.md)
now adds conservative coverage, line/stipple modes, provoking vertex, depth
convention and independent dynamic controls, with [clean installed C/native Rust
and local GPU acceptance](results/m8-raster-mechanisms-2026-11-07.md) at `92c1184`.
[Programmable sample positions](sample-locations.md) now implement explicit
patterns, independent dynamic enable, compatible-depth images and ordinary/split
transition metadata in revision 12; [local evidence](results/m8-sample-locations-2026-11-07.md)
checks coverage and depth preservation. Clean installed C/native Rust and
GGML/libplacebo acceptance passes at `73090dd`. [Complete dynamic vertex input/divisors](dynamic-vertex-input.md)
now add direct recording and generated-buffer-token composition without a new
fetch schema or pipeline permutations. [Clean installed C/native Rust acceptance](results/m8-dynamic-vertex-input-2026-11-07.md)
passes at `1089406`. [Core dynamic raster controls](core-dynamic-raster.md) now add
eight direct setters and topology-class discovery, with local static/dynamic
color/depth evidence. [Clean installed C/native Rust acceptance](results/m8-core-dynamic-raster-2026-11-07.md)
passes at `a16805c`. [Extended dynamic raster controls](extended-dynamic-raster.md)
now add eleven independent capabilities for clamp, polygon, multisample and
blend/logic/write state, with [local and clean installed C/native Rust evidence](results/m8-extended-raster-2026-11-07.md)
at `ebf6abb`.
[Viewport/scissor arrays](viewport-arrays.md) now add explicit range/count updates
and shader routing, with local 1/4-sample output checks.
[Extended depth/color-write profiles](extended-depth-color.md) now preserve native
ranges, bias units and independent attachment enables. The [format inventory](format-inventory.md)
adds eighteen IDs through existing mechanisms and accounts for every pinned native
definition. [Dynamic patch count/origin](dynamic-tessellation.md) now preserve native
per-draw selection. [Geometry capture/streams](geometry-capture.md) now implement native
ordered output, counters, queries and byte-count draws in revision 13. Contract and
[Consumer composition and contract consolidation](m8-consolidation.md) is implemented;
[Clean installed acceptance](results/m8-consolidated-acceptance-2026-11-07.json)
completes the scheduled M8 checkpoint. M9 is next.
Native combinations and general stage-interface tooling remain
explicit acceptance work rather than blanket milestone completion.

**Deliver:** task/mesh executable stages and direct/indirect/count launch paths (initial tranche above);
explicit GPU-generated command layouts, executable selection, preparation/storage
and execution; capability-specific multiview, shading-rate, tile-local/input-
attachment and pass-continuation strategies. Audit remaining vertex-fetch,
tessellation and geometry-stage gaps and implement native profiles where absent.
Add compressed-texture format/copy/sampling profiles with explicit block layout
and usage queries, rather than assuming every native format combination works.
Prefer a composition through the revised primitives; add an operation only for
hardware execution or scheduling that cannot be preserved that way. A GPU culling
algorithm and render-graph scheduler stay in the consumer. Preserve task/mesh and
generated-work capability even if their entry points consolidate.

**Consumer:** a GPU-culled scene with generated draw/mesh work, plus small raster
fixtures for the additional profiles. A conventional draw path can be a separately
selected control, not automatic mesh emulation.

**Acceptance:** generated artifacts and stage linking; GPU-generated count/identity
and bounds; exact preparation/execution dependencies and lifetimes; image checks
for supported raster profiles. Inspect lowering for CPU readback or forced per-draw
host work. Local execution covers supported profiles; unsupported mesh/generated
command features retain explicit native-test debt.

### M9 — Ray tracing

**Deliver:** the non-derivable hardware boundary for acceleration structures and
ray execution, reusing ordinary memory, arguments, commands and dependencies.
This includes acceleration-structure requirements, caller-controlled backing and
scratch, build/update/copy/compaction and query operations; shader ray queries;
ray executable groups, shader tables, and direct/indirect trace profiles. Keep ray
queries independently usable without a ray pipeline. Scene graphs, build scheduling
and acceleration-structure allocation policy stay outside core. Preserve native capability
differences rather than assuming every backend implements every execution model.

**Consumer:** a deterministic scene using both inline ray queries and a ray-pipeline
variant where supported, including geometry updates and compacted structures.

**Acceptance:** hit/miss and instance/geometry identity against a CPU reference;
build-to-trace, update and compaction dependencies; scratch/table alignment and
lifetime rules; malformed/unsupported profile rejection. Inspect the lowering for
hidden builds, allocations or CPU traversal. No software tracing fallback counts
as hardware acceptance. Pull M10 backing/interop prerequisites forward if needed.

### M10 — Memory and interoperability

**Deliver:** mixed buffer/image backing where native placement allows it; typed
owned/borrowed device/resource handles; external memory and synchronization
import/export with device identity and initial/final state contracts; virtual
address reservation, map/unmap/access, sparse image binding and explicit residency
operations only where the composition audit establishes a missing primitive.
Allocators, streaming/eviction policy and interop convenience wrappers stay outside
core. Audit raw-address command endpoints so foreign resources are not forced
through unnecessary allocation or registration strategies.

**Consumers:** an external native presenter or GPU producer sharing resources with
OGPU without CPU readback, plus sparse/virtual resource streaming and residency
fixtures. Pull subsets earlier for P1, Metal or advanced execution as needed.

**Acceptance:** zero-copy handoff with explicit synchronization, ownership and
teardown; aliasing/map/unmap ordering; external-handle ownership/error paths;
bounded streaming reuse and native residency behavior. Separate OS/handle types
and hardware profiles in results. Multi-device peer execution remains a distinct
consumer/validation track, not inferred from single-device import/export.

### M11 — Advanced compute and media

**Compute deliverables:** richer shared-memory/launch profiles, cooperative or
cluster/device-enqueue execution where native APIs support it, pipeline artifact/
binary interfaces, and remaining numerical/subgroup/matrix profiles. Extend
generated artifact requirements and implement a hardware matrix variant of the
bounded ML consumer with explicit layout, accumulation and numerical permissions.
Audit conditional rendering and primitive-generation query variants here as
independent execution/counter profiles; capture counters and ordinary indirect
counts do not establish their equivalence. First use shader/toolchain capability declarations and the existing launch model;
add host records only for a native facility not expressible that way. Do not add
host tensor operators, graph execution or an automatic precision selector to the core.

**Media deliverables:** explicit codec/session capabilities, plane-aware images,
bitstream ranges, parameter/reference records, decode/encode operations and status
queries. Select and document the initial codec/profile set in the implementation
brief from native mappings; no universal codec promise. Use M10 sharing for external
media integration where needed.

Plane-aware formats (including NV12/P010-style layouts) need per-plane extents,
copy pitches/aspects, binding and sampling/conversion contracts. They form a coherent
sub-batch that can move earlier without implementing codecs. M11 also owns remaining
64-bit-component typed format profiles and their independent shader/vertex/storage
requirements; existing FP64 arithmetic is not their acceptance evidence.

**Acceptance:** compute outputs versus declared numerical bounds and launch/
artifact contract tests, with actual native acceleration distinguished from scalar
controls. For media, deterministic bitstreams and reference-frame checks for decode,
declared quality/error checks for lossy encode, explicit reference-picture lifetime
and synchronization tests. Compute and media are separate acceptance subprofiles;
neither blocks implementing the other, and neither becomes a core device prerequisite.

## Coverage assignments and remaining horizon

Scheduled capabilities below refer to the batches above. Other items retain an
explicit entry criterion; they are not silently included in milestone completion.

| Gap | Planned treatment / entry criterion |
|---|---|
| New source language | M2 decision; a frontend only if a concrete expressiveness/tooling gap warrants it |
| Structured arguments, heap interfaces, graphics linking | M2, then the varying subset exercised by M4 |
| Replay and allocation assistance | M3 measurement/contract decision; application helpers first |
| Concurrent host recording / multiple queues | Implemented in foundation; parallel host workers and Radeon cross-family transfer pass. Actual overlap benefit and broader topology/hazard evidence remain open |
| Timed waits / cancellation | Timeline timeout/poll and recording cancellation are implemented. Timeout/cancellation do not retire pending GPU work; GPU cancellation/recovery remains a separate semantic decision |
| External memory/import-export | M10; boundary design in P1, prerequisite subsets earlier when needed; ownership/device matching and native synchronization required |
| Multi-device execution/communication | Deferred; needs a selected workload and multiple real devices. Never a prerequisite for current local milestones |
| More formats, arrays, 3D textures, MSAA | 122 ordinary/packed plus 68 compressed formats and existing image shapes/resolve profiles implemented; bounded BC execution, other compressed profiles locally unverified; M8 now records the complete pinned format inventory; M11 adds plane-aware media and 64-bit-component profiles. Query exact native combinations; no universal format claim |
| Mixed backing, virtual memory and residency | M10, with required subsets pulled into M6 or other consumers; explicit native placement/ordering, no hidden migration |
| Generated commands and richer artifact/launch profiles | M8 generated topology; M11 richer launch and binaries; P1 implements the narrow optional-extension negotiation prerequisite |
| Presentation | P1 Linux before M6; M6 Metal; real window-system acceptance required |
| Native Rust host interface | P1 opening refactor: shared core below C adapters, typed zero-policy interface and standalone Rust consumer; subsequent milestones extend both host interfaces |
| Ray tracing / mesh and other specialized stages | M8 mesh/advanced raster, M9 rays, M11 video; implementation scheduled independently of hardware availability, native acceptance remains mandatory for support claims |
| General GGML backend / GPU training | M5 remains bounded inference. Broader operator coverage or forward/backward/optimizer training is a separate consumer milestone, not a new host tensor API |
| Hardware matrix acceleration / wider numeric types | M11 extends existing requirements/discovery to remaining execution profiles and a hardware matrix consumer. Unsupported tuples stay unaccepted; M5 scalar Q8 is not acceleration |
| Fine profiling / capture-replay / GPU debugging | Foundation exposes explicit timestamp/occlusion/statistics pools. Consumer logs exist; foundation native labels and full address-aware capture/debugging remain separate tooling work |
| Windows and other platform builds | DX12 candidate implemented on its branch with open review corrections and C1–C3 acceptance; other platforms need their own maintained build/loader/consumer lane. Public C does not imply tested platform support |
| macOS installation/package support | After M6 artifact workflow, or a separately selected compute-only packaging pass; native install/link tests required |
| Portable binary bundles | M7 starts source/local-build distribution; bundles need a chosen libc/deployment policy and tests on that target, not repackaged Nix binaries |
| Independent adoption, cross-vendor coverage | M7 evidence track; record actual external results, no hardware acquisition or outreach assumed |

Host tensor operators, automatic memory migration, hidden graph scheduling and
ownership inferred from GPU pointers remain outside the core. They are not
completion debts. Higher-level libraries can implement their own policies on top.

## Working discipline

- One coherent active implementation batch with explicit acceptance. Pull needed
  prerequisites forward and record the change; do not serialize every capability
  behind a separate experiment. Missing external validation leaves acceptance open,
  not all further implementation blocked. Later rows remain scoped plans, not
  permission for unrelated actions or publication.
- Commit coherent implementation steps and acceptance records. If gates fail,
  retain the failed result and discuss semantic/tolerance changes explicitly.
- Keep the installed consumer and existing bounded applications as regressions;
  scale test depth to changed runtime/compiler contracts rather than rerunning
  every historical benchmark for every documentation change.
- A milestone can close with a documented retain/reject design decision; it
  cannot claim implementation or hardware validation that did not happen.
- Revisit ordering after each milestone, without silently losing the language,
  graphics, ML or portability goals from the coverage map.
