# Working status and next milestone

Updated 2026-10-15. **M1–M5 and foundation-contract consolidation are complete for
their declared scope. Next is P1: presentation and an interactive Linux consumer,
before M6 Metal foundation. P1's initial native Rust operation extraction is complete; presentation
and M6 implementation have not started.**
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

## Next: P1 presentation and an interactive consumer

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
shader layout coverage is claimed. No C ABI revision is changed. Next: P1's native
presentation mapping and selected Linux window path, exposed through both APIs.

The [P1 mapping](presentation-contract.md) now selects Wayland/external GLFW and
KHR swapchain maintenance with distinct render/presentation retirement. Local
Radeon supports the queried extension/feature baseline; no window or swapchain
execution is accepted yet. Generated WSI declarations reproduce and pass C/Rust
ABI comparison (1,009 layout values); optional setup, public records and runtime
implementation remain next.

Expanded-tranche checks: 48 unit and two host integration tests, 17 GPU unit tests plus the native Rust
consumer integration test on Radeon and llvmpipe, standalone Cargo consumer on
both drivers, full public-C foundation fixture on Radeon, and strict Clippy.
GPU runs use Khronos synchronization validation. This is one physical GPU plus
software evidence, not cross-vendor or Metal acceptance; no new timing claim.

## Then: M6 Metal foundation

Follow the [M6 entry handoff](m6-entry.md):

1. Establish exact Metal mappings/profiles, especially arguments, replay,
   synchronization, placement and residency.
2. Implement the explicit resource/command foundation without hidden copies,
   waits, retained resource graphs or Vulkan-shaped emulation.
3. Generate native executable interfaces and run the mixed learned-image and
   heap-image fixtures with native lifetime/reuse/error validation on a Mac.
4. Map P1 presentation to Metal and run the interactive consumer, preserving
   explicit drawable availability and presentation synchronization semantics.

Existing Metal 4 compute remains the legacy path with earlier native evidence,
not current foundation parity. No Mac is available here. A coherent implementation
handoff can be prepared when work resumes, but native acceptance cannot be inferred
from Linux tests. M6 work has not begun in this checkpoint.

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
