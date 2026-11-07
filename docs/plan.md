# Working status and next milestone

Updated 2026-11-06. **C1 selection and C2 Vulkan/Linux implementation are complete.
C3 Vulkan/Linux acceptance is complete; remaining M8 is active. External native mappings remain unaccepted.** See the [revised sequence and exit criteria](roadmap.md#minimal-core-revision).
The [first contract draft](minimal-core-contract.md) now records conceptual
decisions, a whole-surface disposition table, user compositions and DX12 mapping
constraints. [Argument update/reuse A/B](minimal-core-arguments.md) now has selected
semantics and exact C/Rust draft records. [Descriptor storage D](minimal-core-descriptors.md)
now selects native-unit locations, explicit backing/copy rights and one authoritative
representation. [Presentation P](minimal-core-presentation.md) now selects borrowed
frames, native availability, explicit outcome effects and replacement/resize lifetimes.
[Dependency/submission decisions](minimal-core-dependencies.md) now preserve justified
scopes and consolidate submission. The [consolidated migration](minimal-core-migration.md)
covers every installed operation, cross-family records and the ordered C2 cutover.
The [C2 argument cutover](minimal-core-migration.md#c2-argument-public-cutover) is complete
on Vulkan: foundation schema 7 exposes prepared layouts and one argument updater
through C and direct Rust, with migrated compiler artifacts and consumers. The old
argument adapters are deleted. The [public descriptor cutover](minimal-core-migration.md#c2-descriptor-public-cutover)
is also complete: C/Rust heap locations, native shape/requirements queries and
secondary inheritance use explicit heap objects. Raw-output/bind-record paths are
deleted. The [generated-layout cutover](minimal-core-migration.md#c2-generated-argument-layout-cutover)
is complete: explicit borrowed argument layouts, native address tokens, cold
mapping checks and equivalent-layout execution pass locally. The [public dependency cutover](minimal-core-migration.md#c2-dependency-public-cutover)
is complete: C/direct Rust use per-entry scopes, explicit shared scratch and enabled-domain
limits. Old shared-stage/nested-scratch paths are removed and consumers migrated.
The [public submission cutover](minimal-core-migration.md#c2-submission-public-cutover)
is complete: one counted C/direct Rust operation, explicit per-batch acceptance,
direct binary arrays and one caller scratch span. Superseded submission paths are
deleted and consumers migrated. The [public presentation cutover](minimal-core-migration.md#c2-presentation-public-cutover)
is complete locally: borrowed frames, native availability, creation/resize state and
batch/per-item effects are shared by C and direct Rust. Old index-based wrappers are
removed; the two-window probe and three-slot scene use the selected contract and
pass bounded Wayland checks. The [complete C2 audit](results/c2-contract-audit-2026-11-06.md)
checks the installed signatures, all public records, selected constants, exports
and deletion map. [Clean C3 acceptance](results/c3-linux-acceptance-2026-11-06.md) now passes on Vulkan/Linux, including isolated C/native Rust SDKs and pinned GGML/libplacebo consumers. Remaining M8 resumes.

Execution is Vulkan/Linux first on `minimal-core`, based on `master` at `2374635`.
Carry the design/review documents without the DX12 implementation. Use that branch
as evidence for the mapping constraints, then hand the selected contract back to
Windows later. Neither DX12 fixes nor unavailable native validation block local
C1–C3 or subsequent M8 work.

M1–M5 and foundation-contract consolidation are complete for
their declared scope. P1 presentation has reached partial human acceptance;
M6 Metal implementation is deferred until a Mac is available. M8 compressed
textures, task/mesh and initial generated-command work is implemented within the
receipts below; its remaining coverage resumes after the revision's local acceptance.
P1's native Rust operation extraction and opt-in
Wayland surface setup, explicit device enabling, mode queries and binary submission
dependencies are implemented. Borrowed swapchain images and acquire/present/release
now pass a bounded two-window lifecycle probe. The interactive textured Rust scene
is implemented; bounded pixel/lifecycle checks pass with three frame slots and
observed framebuffer resizes. The SDK now packages a self-contained native Rust
source/consumer/shader bundle and a manual acceptance checklist. Partial human
checks confirm controls, resize, pause and close; resume and real minimize/restore
remain unverified. M6's initial mapping proposal is complete; backend implementation
and native acceptance have not started.
The earlier stop at the M6 entry checkpoint is complete. Scheduled work is not
implemented or natively validated unless explicitly reported below.

The [current design](design.md) describes the model; the
[implemented surface](foundation-implementation.md) gives exact profiles and
evidence. The [roadmap](roadmap.md) owns broader coverage. The [whole-API
design](whole-api-design.md) is a target, not an assertion that its remaining
[sketch](drafts/ogpu_next.h) is implemented.

On `dx12-backend`, native D3D12 implementation now reaches reported Windows M4
scene execution. The [D1 receipt](https://github.com/elfstead/gpu/blob/34930b0/docs/dx12-backend-d1.md) is an earlier checkpoint;
the [2026-10-30 review handoff](dx12-review-handoff-2026-10-30.md) owns the open
corrections. The branch is unmerged and backend acceptance remains open.

## Completed checkpoints

| Work | Outcome | Evidence boundary |
|---|---|---|
| M1: useful-scale execution | Learned-image scale/reuse and native comparisons | Historical measured configurations; not universal parity |
| M2: compiler/programming contract | Pinned Slang, generated structured arguments and stage/heap interfaces | Bounded tooling contract; not a new frontend or arbitrary shader support |
| M3: resource/submission maturity | Sustained slot reuse and independent consumer handoff | Current foundation replaces the former ownership conveniences |
| Foundation revision | Explicit placement, queues/timelines, object-local concurrency, arenas, independent descriptors, arguments and common graphics/compute profiles | Default Linux SDK; no native Metal foundation |
| M4: substantial offscreen graphics | Textured indexed/depth scene, mips/views/filtering/blending and generated interfaces | [Clean native-matched/SDK acceptance](textured-scene-plan.md#clean-m4-offscreen-acceptance) at `43bb9f0`; presentation separate |
| M5: substantial bounded ML | Full transformer block plus separate F32/F16/Q8_0 linear consumer | [Clean acceptance and cost/error/storage observations](m5-results.md); no CPU fallback, no general LLM/training or hardware matrix claim |
| Foundation-contract consolidation | Native mappings, safety/lifetime rules, structural performance dispositions and explicit gaps reconciled | [Completed review](core-contract-review.md); not a universal performance theorem or stabilization approval |
| M7 first preparation checkpoint | Revision-pinned source/SDK workflow and isolated consumer builds | Already usable experimentally; refreshed release/adoption evidence remains |

The latest M5 implementation checkpoints are `de69710` (quantized consumer) and
`5c2f899` (reproducible installed cost observations), accepted in `4319c98`.
Both local drivers pass HOST/DEVICE transformer and linear matrices. Radeon also
passes the original full MNIST precision regressions. Validation is separate from
cost observations; warmup trends prevent a meaningful variant speed ranking.

The consolidation refresh passes 46 ordinary foundation-build tests, 16 GPU tests
per driver and the public-C fixture on Radeon. llvmpipe retains its explicit
combined depth/stencil copy/readback exclusion. One Radeon plus llvmpipe is not
cross-vendor physical coverage.

## Current: minimal composable foundation revision

The [design rule](design.md#minimal-composable-core) prefers one simple mechanism
when users can derive the alternatives without losing capability or efficient
execution. The core must not absorb abstractions just because applications need
them. Native API alternatives and current implementation weight are insufficient
justification for multiple public representations.

Proceed with [C1–C3](roadmap.md#minimal-core-revision):

1. **C1, completed contract selection:** revise the whole-API draft and add a
   mechanism disposition/migration table. Derive argument grouping, descriptor
   allocation/caching, replay and frame loops from the proposed primitives. Resolve
   root observation semantics, descriptor storage/update capabilities and command/
   synchronization composition before selecting additional profiles. Record actual
   counterexamples and unknowns, rather than assuming all previous alternatives
   must survive.
2. **C2, completed on Vulkan/Linux:** implement the selected contract across Rust/C, compiler artifacts,
   Vulkan/Linux and affected consumers as a coordinated batch; remove redundant
   paths and move policy outside core. Keep a DX12 mapping/migration table and
   defer backend updates to the native Windows handoff.
3. **C3, completed on Vulkan/Linux:** clean revision-7 native, public C and isolated
   SDK checks pass, including GGML/libplacebo and bounded Wayland consumers.
   Windows validation remains with its native lane; Mac work remains deferred.
   Remaining M8 resumes without treating external validation as passed.

M8 → M9 → M10 → M11 remains the hardware-coverage order after this revision, with
prerequisites pulled forward as needed. Each batch applies the composition rule
before adding host operations. Shader/compiler capabilities need no new host
abstraction when the selected primitive already expresses them. M7 packaging
continues throughout; M6 resumes only with native development available.

General safe wrappers, allocators, render graphs, frame schedulers and tensor
runtimes belong outside core. Each removal must identify its replacement and
preserved capability in C1 before implementation.
The complete target remains graphics, general compute and ML, including the
scheduled specialized hardware facilities.

C1 checkpoint: [selected contract](minimal-core-contract.md) plus selected
[A/B](minimal-core-arguments.md), [D](minimal-core-descriptors.md) and
[P records/migration](minimal-core-presentation.md).
One range-update operation replaces separate root/inline operations; native domains/compatibility metadata
describe reuse without shadow restoration. Existing argument reuse/partial-update
evidence is reused, not rerun. Descriptor storage now has one native-unit location
model and explicit CPU-copy/source rights without a mirror. Presentation separates
availability, render completion, native retirement and image reuse without a frame
scheduler. Dependencies now retain justified ranges/split/ownership, independently
scoped entries and one submission operation. The complete signatures/deletion map
is now [consolidated](minimal-core-migration.md). C2 public argument records, prepared
layouts, the updater, compiler metadata and consumers are implemented and locally checked.
A pinned mixed-mapping shader now passes real uniform/storage/inline access,
shared-layout pipeline binding, partial updates and changed-pointee replay with
guards and synchronization validation on RADV.
Argument and descriptor public cutovers are complete, including heap consumers and
secondary inheritance. Next finish explicit generated argument layouts, then
dependencies/submission and presentation before local C3.
No final profile count is assumed.

## P1: implemented, partial human acceptance

Follow the [presentation brief](presentation-plan.md). First extract the shared
native Rust core below the C adapters, preserve C behavior, and add a standalone
Rust consumer with explicit lifetime/submission/error handling. No additional
retention, scheduling or waits; a general safe wrapper is a separate project.
Then implement optional native-surface and
swapchain integration, explicit acquire/render/present dependencies, and a small
interactive Linux scene with multiple frames in flight and resize/minimize
handling. Window creation and input remain in an external library. Specify the
native-interoperability boundary alongside presentation; full external-memory
implementation is not a prerequisite. No display availability is assumed.

Native Rust equivalents now cover every currently exported foundation C operation:
setup/resources, prepared executables/caches, descriptors/images, recording,
raster, queries, split dependencies and submissions. Both interfaces share backend
validation/execution and recording error handling; native Rust never calls C.
The standalone [Rust consumer](../examples/foundation_rust/README.md) exercises
resource/compute/image A/B/A, explicit lifetime, errors and retry. Two host tests
compile-check C/Rust operation correspondence and the no-C-call boundary. Broader
graphics/query/secondary execution relies on existing shared-backend/C tests, not
independent Rust-consumer acceptance for every operation. Some descriptions still
use versioned records/raw nested pointers; no fully safe wrapper or generated Rust
shader layout coverage is claimed. Foundation revision 1 remains mutable; pin the
matching header, runtime and Rust source, including the growing presentation table.

The [P1 mapping](presentation-contract.md) now selects Wayland/external GLFW and
KHR swapchain maintenance with distinct render/presentation retirement. Optional
instance setup, a versioned C table/native Rust equivalents, borrowed Wayland
surfaces and live capability/format/mode/queue queries now work. The bounded
external-GLFW hidden-window probe passes on Radeon and llvmpipe with validation;
it checks headless rejection, cross-instance rejection and surface lifetime after
discovery destruction. Native values outside OGPU's current format/usage vocabulary
remain visible. Headless setup is unchanged. See the
[setup receipt](results/p1-surface-setup-2026-10-15.md) and
[device/mode-query follow-up](results/p1-presentation-profile-2026-10-15.md).

Current checks: 55 host unit plus two host surface-coverage tests, 19 headless GPU
unit tests plus native Rust consumer per driver, three Wayland integration tests
(setup, lifecycle, textured scene) per driver, two standalone scene host tests and
strict Clippy. The preceding runtime checkpoint passed 1,122 C/Rust ABI layout
values and the full C fixture on Radeon; this scene adds no runtime/header changes.
Presentation enabling now uses the ordinary device feature mask; no window
or implicit queue choice is required. Per-mode image-count/scaling/compatibility
queries preserve the caller's strategy: local FIFO/IMMEDIATE need at least three
images, MAILBOX four. These counts require matching explicit mode-list creation.
Binary dependencies now share ordinary single/batched submission and caller-owned
scratch with timelines, without hidden bridge submissions. Reusable fences expose
create/poll/wait/reset/destroy, with native acquire/present signal association.
The headless Rust consumer checks real binary fill/copy handoffs;
injected C-path tests verify lowering and rejection before any native submission.
See the [synchronization receipt](results/p1-binary-synchronization-2026-10-15.md).
The [live lifecycle receipt](results/p1-swapchain-lifecycle-2026-10-15.md) covers
borrowed-image protections, two-swapchain batch presentation, explicit render and
presentation retirement, old-chain replacement and unused-image release. The probe
selects MAILBOX explicitly: native FIFO presentation stalled with two windows;
compositor throttling is suspected, not a proven driver diagnosis. No fallback is
hidden in OGPU. llvmpipe also executes presentation transitions with unified layouts.
The [interactive scene receipt](results/p1-interactive-scene-2026-10-16.md) now adds
native Rust indexed/depth/textured/blended rendering, three caller-owned slots,
observed framebuffer resizing and close with pending slots. Both a guarded pixel
oracle and the normal no-readback path pass bounded runs on both drivers. Input
controls now have [partial human acceptance](results/p1-human-interaction-2026-10-17.md):
controls, resize, pause and close work; resume was not tested. Synthetic zero extent exercises
the suspension branch; Wayland GLFW cannot report iconification or restore a window,
so actual minimize/restore remains a manual compositor exercise.
The SDK now includes the native Rust consumer, runtime source dependency, pinned
Cargo.lock, embedded shader inputs and human acceptance checklist under
`share/ogpu/native-rust`. The isolated checker builds from copied sources with a
fresh target directory, no SDK shared library and no shader compiler; registry
dependencies still need fetching or an existing cache. Clean isolated headless and
bounded display checks pass on both local drivers; see the
[packaging receipt](results/p1-rust-package-2026-10-17.md), the bundle README and
`tools/test-rust-install.py`. Remaining observations stay open in
[the checklist](presentation-acceptance.md); this compositor has no minimize support.
The [M6 mapping proposal](m6-native-mapping.md) now records the next design step,
without blocking on unavailable manual checks.
No scanout, real-loss recovery or cross-family
presentation acceptance yet; no full P1 human-acceptance claim.

Expanded-tranche checks: 48 unit and two host integration tests, 17 GPU unit tests plus the native Rust
consumer integration test on Radeon and llvmpipe, standalone Cargo consumer on
both drivers, full public-C foundation fixture on Radeon, and strict Clippy.
GPU runs use Khronos synchronization validation. This is one physical GPU plus
software evidence, not cross-vendor or Metal acceptance; no new timing claim.

## Pulled-forward foundation format coverage

Before returning to P1 packaging, the requested ordinary/packed format foundation
batch is now implemented: 118 explicit IDs with shared native/Rust metadata,
aspect/numeric-aware validation and capability-sized consumer enumeration.
See [format coverage and evidence](results/foundation-formats-2026-10-16.md).
The [storage-image follow-up](results/foundation-storage-images-2026-10-16.md) now
adds independent per-format formatless read/write flags and compiler-generated
format requirements, with typed/formatless normalized, signed, unsigned and float
2D execution. These are format permissions, not new coarse device enables. The
following P1 checkpoint packages the Rust consumer and human acceptance checklist.
This does not complete M8: block compression remains its own coherent image/copy/
view/sampling batch, alongside mesh/GPU-driven work. Plane-aware formats remain
M11, pullable earlier independently of codecs; no conversion fallback is planned.

## Deferred: M6 Metal foundation implementation

Follow the [M6 entry handoff](m6-entry.md):

1. Initial [native mapping/profile proposal](m6-native-mapping.md) recorded:
   native address-binding arguments, conservative one-shot scope pending replay
   guarantees, explicit residency and precise synchronization/presentation profiles.
   These are proposed contracts, not implemented support. C1 now reassesses their
   representation choices under the composition rule before implementation.
2. Implement the explicit resource/command foundation without hidden copies,
   waits, retained resource graphs or Vulkan-shaped emulation.
3. Generate native executable interfaces and run the mixed learned-image and
   heap-image fixtures with native lifetime/reuse/error validation on a Mac.
4. Map P1 presentation to Metal and run the interactive consumer, preserving
   explicit drawable availability and presentation synchronization semantics.

Existing Metal 4 compute remains the legacy path with earlier native evidence,
not current foundation parity. No Mac is available here. A coherent implementation
handoff can be prepared here, but native acceptance cannot be inferred from Linux
tests. By user decision, defer implementation until a Mac is available. Preserve
the mapping as design input, not a reason to refactor around untested assumptions.

## M8: implemented tranches and next coverage after C1–C3

The first [compressed-format tranche](results/m8-compressed-formats-2026-10-17.md)
is implemented: 54 new BC/ETC2/EAC/ASTC LDR IDs, per-aspect copy-block metadata,
block-aware memory/image copies, compatible views and native BC1 sampling.
Both drivers execute all 16 BC copy/view profiles with odd edges, three mips,
two layers, padded pitches and guards; the requested ETC2/EAC/ASTC profiles are
unsupported locally. Format query records grow from 32 to 56 bytes; consumers
must rebuild against matching sources. No implicit decode/transcode or new runtime
dependency. Remaining compressed scopes are explicit in the receipt.
The [task/mesh tranche](results/m8-task-mesh-2026-10-17.md) now adds explicit
feature enabling, physical workgroup/payload/output limits, prepared mesh and
task+mesh executables, and direct/address-based indirect/count draws through C
and native Rust. llvmpipe passes 896 pixel checks across both stage combinations
and seven launch/count modes, task payload transport, guards and invalid-input
rejection. Radeon reports these native features unavailable. This is software
native execution evidence, not physical mesh-GPU acceptance or a speed claim.
The [generated-command tranche](results/m8-generated-commands-2026-10-17.md)
adds native layouts, executable selection, explicit storage requirements and
separate or integrated preprocessing through C and native Rust. Both drivers pass
20 compute submissions with GPU-written executable indices, arguments and counts,
sequence identity, set updates, whole-buffer guards and 20 rejected recordings.
Graphics/mesh and nested-count actions had only lowering/layout coverage at
that checkpoint; the graphics execution follow-up is recorded below. No host readback or command-time allocation
is introduced; this is not a timing or full-M8 acceptance claim.
The [generated-graphics/culling consumer](results/m8-generated-indexed-counts-2026-11-06.md)
now passes all four ordinary/indexed/nested-count action forms on Radeon through
unchanged revision 7: 48 submissions, explicit/integrated preprocessing, GPU
compaction/count/identity, signed index offsets, nested count clamping and 98,304
pixels with full-buffer guards. Graphics pipeline selection is unsupported locally
and independently gated; no fallback is introduced.
The [buffer-state token follow-up](results/m8-generated-buffer-state-2026-11-06.md)
now implements native vertex/index tokens and explicit vertex stride in foundation
revision 8, with 96 total graphics submissions and 196,608 checked pixels. C/Rust
bindings, SDK revision checks and the installed contract audit migrate together.
[Clean revision-8 SDK acceptance](results/m8-revision8-sdk-2026-11-06.md) passes for
relocated C, pinned GGML/libplacebo and isolated native Rust/Wayland consumers.
Foundation revision 9 now implements [counted feature-ID selection](feature-selection.md)
across discovery, device requests, shader requirements and generated interfaces.
C and native Rust migrate together; the implementation encoding is private and
recording retains direct checks. Source/ABI and local Vulkan regression pass;
[clean installed-consumer acceptance](results/m8-feature-selection-2026-11-06.md)
now passes for C/GGML/libplacebo and native Rust/Wayland. The receipt distinguishes
the two clean acceptance commits and verifies unchanged C artifact identity.
Revision 10 implements [native multiview](multiview.md), including independently
enabled support, limits, pipeline/render/inheritance masks, compiler requirements,
sparse/offset array-layer checks and rejections. The [local receipt](results/m8-multiview-2026-11-06.md)
records primary/secondary replay, the timestamp validation-layer limitation and
remaining color/MSAA/generated/mesh acceptance. Clean revision-10 C/GGML/libplacebo
and isolated native Rust/Wayland SDK acceptance passes at `2f316e6`.
Revision 11 implements [fragment shading rate](shading-rate.md): independent inputs,
limits/tuples, static/dynamic state, attachment usage/scope and compiler requirements.
[Local evidence](results/m8-shading-rate-2026-11-06.md) covers host/compiler rules,
real capability queries/rejections and existing Vulkan regressions. The local GPU
exposes no native shading-rate input; positive rendering acceptance stays open.
Clean revision-11 C/GGML/libplacebo and isolated native Rust/Wayland SDK checks
pass at `8a57dc4`, with source and runtime identity recorded in the receipt.
[Pass continuation](render-continuation.md) now implements native suspend/resume
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
The [advanced textured consumer](../tools/sdk/foundation-advanced/README.md) now
composes the stages with texture heaps, mip/filtering, depth, blending, scissors
and continuation. [Installed acceptance](results/m8-advanced-textured-2026-11-06.md)
records five profiles and the unchanged baseline consumer. The [M8 coverage
audit](m8-coverage.md) separates missing native mechanisms from validation debt.
The [raster/fetch control batch](raster-controls.md) now implements large points,
independent depth clipping, instance divisors and list/patch restart. Ten local
profiles pass; [clean installed C/native Rust acceptance](results/m8-raster-controls-2026-11-06.md)
also passes. Native patch restart remains unavailable. [Conservative rasterization,
line/stipple controls, provoking vertex and depth convention](raster-mechanisms.md)
now include independent dynamic controls and local execution evidence. [Sample positions](sample-locations.md) now add explicit patterns and depth-content
transition metadata, including split dependencies, in revision 12. Next implement
complete dynamic vertex input/divisors, remaining raster controls and the
extension-format inventory. Geometry capture,
dynamic patch and contract/composition consolidation follow before M9 ray tracing. M6 remains incomplete until native development is possible.

## Parallel candidate: native DX12 on Windows

Review checkpoint, 2026-10-30: branch `34930b0` has native implementation and
reported Windows scene evidence, but is **not ready to merge**. Follow the
[DX12 correction handoff](dx12-review-handoff-2026-10-30.md), including barrier,
binding, presentation, loss and copy fixes plus native-profile redesign and
validation. Linux host/headless regression passed during review; Windows
acceptance must be rerun after corrections. The candidate brief below describes
the original scope, not the current implementation status.

The [API-shape assessment](cross-backend-api-shape-2026-10-30.md) and
[NGPU source comparison](ngpu-comparison-2026-10-30.md) identify contract questions,
not an obligation to retain all native representations. C1 now replaces their
earlier profile-expansion prescription: derive a minimal contract, preserve
efficient compositions and add primitives only for demonstrated gaps. Architectural
fixes now follow the [selected C1 contract and migration](minimal-core-migration.md)
for arguments, descriptors, submission and presentation. Copy-footprint and native
correctness findings remain in the handoff. No review finding is closed by these
design documents.

The [DX12 implementation handoff](dx12-candidate-plan.md) is approved for a
separate `dx12-backend` branch implemented and tested by the Windows agent, then
returned for review. It remains a candidate, not an accepted supported backend. Resolve
native shader addresses/arguments and descriptor/synchronization profiles first;
API changes are welcome when they expose a better API alternative. No emulation
to claim Vulkan parity. Deliver the core, compute/offscreen graphics, then native
presentation and available advanced capabilities with separate evidence. Ordinary
Windows correctness fixes can proceed during C1; new coverage follows C2/local C3.
M6 remains deferred. No WebGPU backend is scheduled.

## Packaging and compatibility

Default Linux installation exports only `ogpu_next.h`, `ogpu-foundation.pc` and
foundation runtime symbols. `--legacy` explicitly adds ABI20. Plain Cargo defaults
retain legacy for remaining in-tree/Metal callers; it is not a runtime fallback.
Foundation revision 1 remains mutable: pin the exact source, header, artifacts and
library. See [quickstart](foundation-quickstart.md), [support](support.md) and
[upgrade policy](release-policy.md).

The SDK includes six small independent source examples plus GGML and libplacebo
adapters with separate pinned upstream dependencies. M5 correctness and optional
cost observations need no downloaded model. Full MNIST testing remains opt-in
with separately supplied data. Our isolated builds are not third-party adoption,
clean-machine coverage or portable binary distribution.

## Scheduled hardware coverage and stabilization

The [contract review](core-contract-review.md#remaining-fundamental-surface-gaps)
keeps mixed buffer/image backing, imported/raw-address resources, VM/residency,
device-generated topology, richer launch/artifact profiles and specialized
graphics/video/ray/presentation facilities visible. Some numerical/format profiles
are implemented but lack matching physical hardware. No gap is approved as a
permanent restriction merely because M4/M5 did not exercise it.

The [roadmap](roadmap.md#scheduled-hardware-coverage) assigns implementation batches:

- M8: advanced raster and GPU-driven execution, including task/mesh and generated commands.
- M9: ray queries, acceleration structures and ray pipelines.
- M10: mixed backing, native interoperability, virtual/sparse memory and residency.
- M11: advanced compute/numerical execution and video encode/decode profiles.

This is the default coverage order after C2/local C3, independent of deferred M6,
not a ban on pulling prerequisites forward:
Metal residency or imported presentation resources belong earlier when needed.
Missing hardware blocks native acceptance of a profile, not its specification or
implementable backend work. Track specified, implemented and natively validated
status separately; no emulation or passing skip substitutes for native evidence.

M7's source/install/upgrade/support refresh is an ongoing packaging track, not the
end of functional development and not conditional on completing M8–M11. If Mac
validation is unavailable, keep M6 incomplete and proceed with local hardware
coverage work rather than waiting idle. Independent adopters,
additional physical GPUs, native Metal validation and real-loss/noncoherent evidence
remain separate tracks; no new hardware, outreach, publication or stable ABI promise
is authorized by this plan. General safe Rust wrappers and debugging/capture tools
are additional work, not implicit properties of a Rust implementation. The native
low-level Rust interface is scheduled in P1 and does not depend on a safe wrapper.

## History

[Foundation development history](plan-foundation-history.md) preserves the previous
chronological status page, including superseded ABI20 follow-ups and earlier milestone
receipts. [Older plan history](plan-history.md) and the [experiment ledger](experiments.md)
retain preceding evidence. Historical “next” instructions are not current work.
