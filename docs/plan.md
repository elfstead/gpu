# Working status and next milestone

Updated 2026-10-17. **M1–M5 and foundation-contract consolidation are complete for
their declared scope. P1 presentation has reached partial human acceptance;
M6 Metal foundation mapping is now underway. P1's native Rust operation extraction and opt-in
Wayland surface setup, explicit device enabling, mode queries and binary submission
dependencies are implemented. Borrowed swapchain images and acquire/present/release
now pass a bounded two-window lifecycle probe. The interactive textured Rust scene
is implemented; bounded pixel/lifecycle checks pass with three frame slots and
observed framebuffer resizes. The SDK now packages a self-contained native Rust
source/consumer/shader bundle and a manual acceptance checklist. Partial human
checks confirm controls, resize, pause and close; resume and real minimize/restore
remain unverified. M6's initial mapping proposal is complete; backend implementation
and native acceptance have not started.**
The earlier stop at the M6 entry checkpoint is complete. Scheduled work is not
implemented or natively validated unless explicitly reported below.

The [current design](design.md) describes the model; the
[implemented surface](foundation-implementation.md) gives exact profiles and
evidence. The [roadmap](roadmap.md) owns broader coverage. The [whole-API
design](whole-api-design.md) is a target, not an assertion that its remaining
[sketch](drafts/ogpu_next.h) is implemented.

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

## Current: M6 Metal foundation mapping

Follow the [M6 entry handoff](m6-entry.md):

1. Initial [native mapping/profile proposal](m6-native-mapping.md) complete:
   native address-binding arguments, conservative one-shot scope pending replay
   guarantees, explicit residency and precise synchronization/presentation profiles.
   These are proposed contracts, not implemented support.
2. Implement the explicit resource/command foundation without hidden copies,
   waits, retained resource graphs or Vulkan-shaped emulation.
3. Generate native executable interfaces and run the mixed learned-image and
   heap-image fixtures with native lifetime/reuse/error validation on a Mac.
4. Map P1 presentation to Metal and run the interactive consumer, preserving
   explicit drawable availability and presentation synchronization semantics.

Existing Metal 4 compute remains the legacy path with earlier native evidence,
not current foundation parity. No Mac is available here. A coherent implementation
handoff can be prepared here, but native acceptance cannot be inferred from Linux
tests. Next is the profile/record vocabulary and resource/command implementation
tranche; unresolved native guarantees remain explicit in the mapping.

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

This is the default order after M6, not a ban on pulling prerequisites forward:
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
