# Roadmap: from runtime candidate to a usable GPU programming foundation

Proposed 2026-09-18; M1 completed 2026-09-19. The [working plan](plan.md) owns completed/active status;
this document owns the sequence and coverage of the remaining project work.
Sequence revised 2026-10-08: the [whole-API design](whole-api-design.md) selects a
coordinated foundation revision before completing the remaining M4/M5 consumers.
As of 2026-10-13, M4/M5 and [foundation-contract consolidation](core-contract-review.md)
are complete for their declared scopes. Sequence revised 2026-10-15: next is
[P1 presentation](presentation-plan.md), followed by [M6](m6-entry.md), now including
presentation. Neither has started. M8–M11 below schedule the remaining hardware
families; completed M1–M5 gates are not reopened.
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
designing or implementing a clearly useful native strategy. API changes
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
| P1 — Presentation and interactive graphics | Optional presentation boundary, Linux window integration and an interactive multi-frame scene | Next, not started: [brief](presentation-plan.md); real display acceptance separate from headless tests |
| M6 — Mixed-workload Metal parity | Foundation migration, learned-image/heap consumers, generated interfaces and P1 interactive presentation | After P1, not started: [entry handoff](m6-entry.md); native execution needs a Mac |
| M7 — Experimental release and adoption | Versioned source/install workflow, contract audit, reproducible consumer acceptance | Ongoing packaging track; first [checkpoint](release-checkpoint.md) complete; refresh with new capabilities, publication/adoption separate |
| M8 — Advanced raster and GPU-driven execution | Task/mesh, generated commands and explicit advanced raster profiles | Scheduled after M6; native profile execution needs supporting hardware |
| M9 — Ray tracing | Acceleration structures, ray queries, ray pipelines and shader tables | After M8 by default; shares memory/executable requirements, native validation may remain open |
| M10 — Memory and interoperability | Mixed backing, native resource/synchronization sharing, VM/sparse memory and residency | After M9 by default; pull prerequisites into P1/M6/M8/M9 when needed |
| M11 — Advanced compute and media | Richer launch/artifact/numerical profiles, hardware matrix consumer and video sessions | After M10 by default; compute and media are independently accepted subprofiles |

Current implementation order: **completed M1–M5/foundation and consolidation →
P1 → M6 → M8 → M9 → M10 → M11**, with **M7 an ongoing packaging/adoption track**,
not the end of functional development. Prerequisites can move earlier without
claiming the entire later milestone. Missing Mac or feature hardware must not
stall useful local work: keep native acceptance open and advance the next
implementable batch. A Vulkan-scoped experimental package does not require Metal
or M8–M11 acceptance; native validation remains a separate evidence track.
No calendar estimates are assigned by this document.
Each milestone ends in a committed result and decision, not another open-ended
list of experiments.

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
M6. Implement one Linux window-system path and an interactive consumer; native
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

M8–M11 are planned implementation batches, not implemented features. Each begins
with an exact native mapping and profile list, then implements the coherent API,
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

**Deliver:** task/mesh executable stages and direct/indirect/count launch paths;
explicit GPU-generated command layouts, executable selection, preparation/storage
and execution; capability-specific multiview, shading-rate, tile-local/input-
attachment and pass-continuation strategies. Audit remaining vertex-fetch,
tessellation and geometry-stage gaps and implement native profiles where absent.
Add compressed-texture format/copy/sampling profiles with explicit block layout
and usage queries, rather than assuming every native format combination works.
Expose materially different native strategies explicitly, without duplicating
equivalent mechanisms merely to match another API's naming.

**Consumer:** a GPU-culled scene with generated draw/mesh work, plus small raster
fixtures for the additional profiles. A conventional draw path can be a separately
selected control, not automatic mesh emulation.

**Acceptance:** generated artifacts and stage linking; GPU-generated count/identity
and bounds; exact preparation/execution dependencies and lifetimes; image checks
for supported raster profiles. Inspect lowering for CPU readback or forced per-draw
host work. Local execution covers supported profiles; unsupported mesh/generated
command features retain explicit native-test debt.

### M9 — Ray tracing

**Deliver:** acceleration-structure requirements, caller-controlled backing and
scratch, build/update/copy/compaction and query operations; shader ray queries;
ray executable groups, shader tables, and direct/indirect trace profiles. Keep ray
queries independently usable without a ray pipeline. Preserve native capability
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
profiles. Audit raw-address command endpoints so foreign resources are not forced
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
Do not add host tensor operators or an automatic precision selector to the core.

**Media deliverables:** explicit codec/session capabilities, plane-aware images,
bitstream ranges, parameter/reference records, decode/encode operations and status
queries. Select and document the initial codec/profile set in the implementation
brief from native mappings; no universal codec promise. Use M10 sharing for external
media integration where needed.

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
| More formats, arrays, 3D textures, MSAA | Existing image shapes/resolve profiles remain; M8 adds compressed texture and advanced raster format/usage coverage, M11 multiplanar media profiles. Query exact native combinations; no universal format claim |
| Mixed backing, virtual memory and residency | M10, with required subsets pulled into M6 or other consumers; explicit native placement/ordering, no hidden migration |
| Generated commands and richer artifact/launch profiles | M8 generated topology; M11 richer launch and binaries; P1 implements the narrow optional-extension negotiation prerequisite |
| Presentation | P1 Linux before M6; M6 Metal; real window-system acceptance required |
| Ray tracing / mesh and other specialized stages | M8 mesh/advanced raster, M9 rays, M11 video; implementation scheduled independently of hardware availability, native acceptance remains mandatory for support claims |
| General GGML backend / GPU training | M5 remains bounded inference. Broader operator coverage or forward/backward/optimizer training is a separate consumer milestone, not a new host tensor API |
| Hardware matrix acceleration / wider numeric types | M11 extends existing requirements/discovery to remaining execution profiles and a hardware matrix consumer. Unsupported tuples stay unaccepted; M5 scalar Q8 is not acceleration |
| Fine profiling / capture-replay / GPU debugging | Foundation exposes explicit timestamp/occlusion/statistics pools. Consumer logs exist; foundation native labels and full address-aware capture/debugging remain separate tooling work |
| Windows and other platform builds | Deferred until a platform-specific build/loader/consumer acceptance lane can be maintained; public C does not imply tested platform support |
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
