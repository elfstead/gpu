# M4: deterministic offscreen scene

Selected after the first M7 local preparation checkpoint. The first native control
is [accepted at `5da5aaf`](graphics-scene-native-results.md) on 2026-09-29; the
[ABI-18 migration](indexed-depth-migration.md) implements the public indexed/depth
slice. The [matched public scene](graphics-scene-public-results.md) and
[expanded contract matrix](indexed-depth-contract-results.md) now pass; reuse and
performance acceptance remained at that checkpoint. [Slot/replay correctness](graphics-scene-reuse-results.md)
is now accepted; performance remains open. Use Radeon and llvmpipe;
no Mac, second physical GPU, window-system integration or asset download is required.

## Consumer and sequence

Build a deterministic textured scene with overlapping indexed geometry, a depth
attachment, mipmapped sampling and a transparent overlay. Start at 257x193 and
640x360; add 1280x720 on Radeon for useful-sized throughput/memory measurements.
Generate geometry/textures procedurally and retain seeds, artifacts and hashes.
Keep the existing learned-image workload as a regression, not the new oracle.

1. **Specify and implement a direct Vulkan indexed/depth control.** Define a small
   shared-vertex mesh with overlapping near/far surfaces, reversed draw order and
   compute-updated positions/indirect arguments. Use deterministic flat colors at
   first. Record native index/depth state, lifetimes, format support and barriers.
   Validate clear/depth rejection/occlusion analytically at interior probe pixels;
   retain full output and repeated-frame identity. This establishes an executable
   reference and exposes the necessary choices before widening the public surface.
2. **Design and implement the public indexed/depth slice.** Compare explicit state
   alternatives against that control, then extend the C boundary, checked native
   bindings, capability/description rejection and generated interfaces only as
   needed. Support compute-written geometry/arguments, caller-owned buffers,
   reusable recordings and explicit color/depth preservation. Predeclare the
   exact depth format, compare/write/clear behavior and legal combinations in
   the slice proposal. Update the ABI if signatures/layout/behavior change.
3. **Mips/views and sampled-image choice.** Add a procedurally generated texture
   with distinct mip content. Define image subresource/view and sampler LOD
   contracts together; explicit upload and sampled views must preserve native
   mip-selection strategies. Do not conflate mip support with image aliasing.
4. **Blend and viewport/scissor.** Add a bounded transparent overlay with declared
   alpha representation and blend equation. Check analytic interior blends,
   untouched scissor regions and depth/color preservation. Reject undefined or
   unsupported state combinations instead of silently changing them.
5. **Generated stage interfaces and full consumer acceptance.** Widen the existing
   offline varying checker only for the scene's actual needs. Run repeated-frame,
   invalid-input, native-matched and installed-consumer regressions. Record which
   public choices were retained and which remain unresolved.

Each slice gets a committed contract and evidence before the next feature is added.
There is no commitment here to a generic Vulkan-shaped pipeline-state object.

## Fundamental performance gate

For every proposed contract, ask whether a native implementation could use the
same efficient native strategies—not merely whether today's wrapper is fast on
this scene. Preserve real indexed execution and its reuse opportunities; a shader
that fetches indices inside an ordinary non-indexed draw is not automatically an
equivalent substitute. Preserve native depth testing, fixed-function blending,
texture filtering/LOD, attachment reuse and GPU-generated inputs without mandatory
CPU readback, hidden copies, per-frame allocation or forced synchronization.

Use native controls with matching state/work as correctness and implementation-cost
comparisons, then identify stronger native alternatives separately. Include a
small repeated-draw case for host encoding/replay costs; GPU-heavy parity alone
cannot approve the API. Validation/tracing stay outside timing. An API-imposed
loss of strategy is a design failure even if this workload hides the cost; backend
encoding overhead is tracked separately and is not automatically fundamental.

## Acceptance rules and stopping condition

- Before execution, predeclare each probe's expected result and edge/filtering
  tolerance. Use interior analytic probes plus same-device native full-frame
  comparison; neither oracle alone establishes general raster conformance.
- Check odd extents, empty/invalid draws, bounds/overflow, cross-device objects,
  unsupported image/state combinations and post-loss rules. Define validity for
  GPU-produced index/indirect values rather than pretending to inspect them on CPU.
- Explicitly test compute-to-index/vertex/indirect dependencies, repeated depth
  clear/load behavior, resource retention, list lifetime and bounded two-slot reuse.
- Both drivers cover correctness; useful-scale timing belongs to Radeon only.
  Distinguish llvmpipe from physical cross-vendor coverage and preserve raw evidence.
- M4 offscreen acceptance requires the full selected scene, native controls and
  unchanged learned-image results. Presentation is a separate follow-on boundary;
  no window concepts enter the compute core just to complete this milestone.

The [indexed/depth proposal](indexed-depth-proposal.md) now selects a candidate
contract and its implementation/acceptance sequence. The coherent ABI-18 runtime
migration, matched scene and bounded lifetime/description gates are accepted.
Stable slots, stronger native multi-record/count correctness and
[draw-identity controls](graphics-draw-identity-results.md) now pass. The
[range/count contract](indirect-draw-ranges.md) is implemented at ABI 19, with
capability/lifetime gates, generated range handoff, [grouped timing and state
correction](graphics-draw-state-results.md), and [useful-scale correctness](graphics-range-scale-results.md).
The [scope/scale measurements](graphics-scope-scale-results.md) are now accepted.
Next: complete installed scene handoff, then the [argument/state reuse audit](argument-reuse-plan.md)
before the mip/view slice. General API performance remains a gate, not a conclusion
inferred from these bounded comparisons.
