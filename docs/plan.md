# Working status and next milestone

Updated 2026-10-13. **M1–M5 and foundation-contract consolidation are complete for
their declared scope. We are at the start of M6; no Metal foundation migration
has been started.** Work stops here as requested.

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

## Next: M6 Metal foundation

Follow the [M6 entry handoff](m6-entry.md):

1. Establish exact Metal mappings/profiles, especially arguments, replay,
   synchronization, placement and residency.
2. Implement the explicit resource/command foundation without hidden copies,
   waits, retained resource graphs or Vulkan-shaped emulation.
3. Generate native executable interfaces and run the mixed learned-image and
   heap-image fixtures with native lifetime/reuse/error validation on a Mac.

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

## Open horizon and stabilization

The [contract review](core-contract-review.md#remaining-fundamental-surface-gaps)
keeps mixed buffer/image backing, imported/raw-address resources, VM/residency,
device-generated topology, richer launch/artifact profiles and specialized
graphics/video/ray/presentation facilities visible. Some numerical/format profiles
are implemented but lack matching physical hardware. No gap is approved as a
permanent restriction merely because M4/M5 did not exercise it.

After M6, refresh M7's source/install/upgrade/support evidence. Independent adopters,
additional physical GPUs, native Metal validation and real-loss/noncoherent evidence
remain separate tracks; no new hardware, outreach, publication or stable ABI promise
is authorized by this plan. General safe Rust wrappers and debugging/capture tools
are additional work, not implicit properties of a Rust implementation.

## History

[Foundation development history](plan-foundation-history.md) preserves the previous
chronological status page, including superseded ABI20 follow-ups and earlier milestone
receipts. [Older plan history](plan-history.md) and the [experiment ledger](experiments.md)
retain preceding evidence. Historical “next” instructions are not current work.
