# Roadmap: from runtime candidate to a usable GPU programming foundation

Proposed 2026-09-18; M1 completed 2026-09-19. The [working plan](plan.md) owns completed/active status;
this document owns the sequence and coverage of the remaining project work.
This is a concrete plan, not authorization to publish releases, provision machines
or coordinate native testing. Only the next milestone is ready to start; later
milestones have bounded deliverables but need a short acceptance brief when begun.

## Target and priorities

Deliver a low-level foundation that an independent application can use for
graphics, general compute and ML, with explicit memory/dependencies and compiler-
supplied executable interfaces. Demonstrate useful-sized applications, not only
small correctness fixtures. Keep the C boundary language-neutral and the Rust
runtime free of tensor operators or an application scheduler.

Do not measure completion by matching every Vulkan/CUDA feature. Broader graphics,
numeric profiles and concurrency must be covered by named consumers. API changes
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
| M2 — Compiler/programming contract | Documented device-code contract; generated structured arguments and heap interfaces; explicit language-direction decision | Next; use M1 findings on Linux, no Mac prerequisite |
| M3 — Resource and submission maturity | Sustained multi-frame reuse, measured submission costs, consumer-side allocation assistance, better diagnostics | M1/M2; existing devices |
| M4 — Substantial graphics consumer | Textured scene with depth, indexed geometry, mipmapped sampling and blending; separate presentation boundary | M2/M3; offscreen Linux first |
| M5 — Substantial ML consumer | One specified transformer block and one quantized linear variant through a broader GGML subset | M2/M3; existing Radeon, no matrix-hardware prerequisite |
| M6 — Mixed-workload Metal parity | Same learned-image application, generated native interfaces, render/readback and common contract tests | M2; native execution conditional on an available Mac validation window |
| M7 — Experimental release and adoption | Versioned source/install workflow, contract audit, reproducible consumer acceptance | First release checkpoint after M1–M3; refresh after M4–M6 |

Default local order: **M1 → M2 → M3 → M7 first checkpoint → M4 → M5**.
M6 can run after M2 when native validation becomes available; it does not block
the Linux sequence or an explicitly Vulkan-scoped experimental release.
No calendar estimates are assigned before measuring each implementation's scope.
Each milestone ends in a committed result and decision, not another open-ended
list of experiments.

Sequence amendment 2026-09-19: the [performance-expressibility gate](performance-expressibility.md)
pulls the contract audit, repeated small-dispatch and streaming-slot experiments
forward from M3 before M2 implementation. M1's matched-policy result remains
accepted; it does not settle whether a stronger native strategy is expressible.
This gate applies to M2's compiler work and later graphics/ML contracts as well.

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

This decision point prevents the initial language ambition from being silently
replaced by a runtime, while also avoiding a new frontend merely to rename Slang.

## M3 — Resource reuse, submission costs and diagnostics

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

**Done when:** the scene and deliberately invalid combinations pass/fail as
specified, allocation reuse and compute-written inputs remain valid, and the
learned-image path has no regression. No generic fixed-function state object
copied wholesale from Vulkan is assumed as the final design.

**Presentation follow-on:** design a separate platform boundary, then implement
one Linux window-system path when the local environment permits native testing.
Test acquire/present, resize, minimization and out-of-date/recreation behavior.
Headless environments do not block offscreen acceptance, but cannot establish
presentation support. Do not add native window concepts to the compute core.

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

**Done when:** the complete block executes without CPU operator fallback, all
intermediates meet the declared numerical contract, supported/rejected layouts
are explicit, and throughput/memory/error tradeoffs are recorded. No automatic
selection of reduced precision merely because a device capability bit is true.

**Accelerated matrix profile:** specify shape/type/subgroup/layout requirements
and variant selection using this workload. Implement and accept a hardware matrix
variant only when supported hardware is available for real execution. The local
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

**Done when:** native Mac execution passes the same fixtures, reuse/error tests
and generated-interface mutations. Linux type checks/mocks can prepare a handoff
but leave this milestone incomplete. Batch native validation into a coherent
handoff when a Mac is available; do not request a round trip per small change.
Any subsequent M4 feature claims require their own native acceptance.

## M7 — Experimental release, then stabilization evidence

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

## Explicit coverage of the remaining horizon

These items are not forgotten or implicitly prerequisites. Each has a disposition
and an entry criterion; adding one requires selecting its consumer and acceptance.

| Gap | Planned treatment / entry criterion |
|---|---|
| New source language | M2 decision; a frontend only if a concrete expressiveness/tooling gap warrants it |
| Structured arguments, heap interfaces, graphics linking | M2, then the varying subset exercised by M4 |
| Replay and allocation assistance | M3 measurement/contract decision; application helpers first |
| Concurrent host recording / multiple queues | Deferred until M1/M3 isolates recording/overlap limits and a two-queue control shows an opportunity; require cross-queue and failure/lifetime tests |
| Timed waits / cancellation | Timed wait only with an interactive responsiveness consumer; timeout must not imply retirement. Cancellation/recovery is a separate semantic decision |
| External memory/import-export | Deferred until an actual media, window-system or other-runtime interoperability consumer; ownership/device matching and native synchronization are acceptance requirements |
| Multi-device execution/communication | Deferred; needs a selected workload and multiple real devices. Never a prerequisite for current local milestones |
| More formats, arrays, 3D textures, MSAA | M4 defines extension points; implement each when the selected scene/volume/antialiasing consumer needs it, not as a full format enumeration sweep |
| Presentation | Separate M4 follow-on; native window-system testing required |
| Ray tracing / mesh and other specialized stages | Deferred, not first-version requirements; needs a concrete renderer and suitable hardware |
| General GGML backend / GPU training | M5 remains bounded inference. Broader operator coverage or forward/backward/optimizer training is a separate consumer milestone, not a new host tensor API |
| Hardware matrix acceleration / wider numeric types | M5 requirements plus actual support/shape evidence; untested hardware paths stay unaccepted |
| Fine profiling / capture-replay / GPU debugging | M3 adds labels/lifetime traces; per-region profiling follows unresolved measured costs. Full address-aware capture/debugging is a separate tooling project |
| Windows and other platform builds | Deferred until a platform-specific build/loader/consumer acceptance lane can be maintained; public C does not imply tested platform support |
| macOS installation/package support | After M6 artifact workflow, or a separately selected compute-only packaging pass; native install/link tests required |
| Portable binary bundles | M7 starts source/local-build distribution; bundles need a chosen libc/deployment policy and tests on that target, not repackaged Nix binaries |
| Independent adoption, cross-vendor coverage | M7 evidence track; record actual external results, no hardware acquisition or outreach assumed |

Host tensor operators, automatic memory migration, hidden graph scheduling and
ownership inferred from GPU pointers remain outside the core. They are not
completion debts. Higher-level libraries can implement their own policies on top.

## Working discipline

- One active implementation milestone with a short brief and predeclared gates.
  Later rows are planning, not permission for unrelated backend/API changes.
- Commit coherent implementation steps and acceptance records. If gates fail,
  retain the failed result and discuss semantic/tolerance changes explicitly.
- Keep the installed consumer and existing bounded applications as regressions;
  scale test depth to changed runtime/compiler contracts rather than rerunning
  every historical benchmark for every documentation change.
- A milestone can close with a documented retain/reject design decision; it
  cannot claim implementation or hardware validation that did not happen.
- Revisit ordering after each milestone, without silently losing the language,
  graphics, ML or portability goals from the coverage map.
