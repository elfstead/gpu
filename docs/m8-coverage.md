# M8 coverage and remaining work

2026-11-07. M8 remains active. Implemented representation, local execution and
complete native coverage are separate claims. The public core exposes native
mechanisms; applications own culling, render graphs, texture policy and scheduling.

| Area | Current implementation and evidence | Remaining disposition |
|---|---|---|
| Compressed formats | 54 BC/ETC2/EAC/ASTC formats, block copies/views; bounded BC execution and sampling | Keep exact format/usage queries; broaden sampling where locally supported. Other native formats remain acceptance debt, without decompression fallback. |
| Mesh/task | Independent enables, limits, executable stages and direct/indirect/count commands; initial llvmpipe execution | Physical GPU mesh execution and richer generated/multiview combinations remain unaccepted. No local Radeon prerequisite for other implementation work. |
| Generated commands | Compute and graphics actions, counts, identity, vertex/index tokens, explicit preprocessing and scratch; Radeon execution | Pipeline selection unavailable locally; mesh and advanced-stage combinations need native receipts. Consumer culling stays outside the core. |
| Multiview | View masks, limits, native enabling, sparse/layer-offset checks | MSAA/resolve and advanced stage combinations need coverage, not another view abstraction. |
| Shading rate | Pipeline, primitive and attachment capabilities, state, limits and compiler requirements | Local native inputs unavailable. Keep host checks separate from positive execution claims. |
| Local read | Attachment maps, input-attachment root bindings, region dependencies; color/storage fixture | Depth/MSAA, secondary/multiview and integrated consumer coverage remain. General artifact generation remains a tooling task. |
| Continuation | Native suspend/resume flags and ordered submissions; depth/MSAA/resolve fixture | Textured stage composition added by this step. Cross-list and other stage combinations retain their own evidence requirements. |
| Tessellation/geometry | Independent stages, patch/origin state, adjacency, shared argument/synchronization scopes and statistics | Textured/depth integration added. Line/point, quad/isoline, unequal patch sizes, multiview, secondary and generated combinations remain acceptance work. |

The new [textured consumer](../tools/sdk/foundation-advanced/README.md) composes
GPU-produced indexed draws, sampled texture heaps, mip views, filtering, depth,
blending and scissors with geometry, tessellation and continuation. It uses five
explicit profiles. No runtime operation, public layout or hidden policy was added.
[Clean installed acceptance](results/m8-advanced-textured-2026-11-06.md) passes
180 frames at both extents. Its fixed offline shader recipe is deliberately separate from the bounded VS/FS
`ogpu-graphics` linker. It is not evidence that arbitrary stage interfaces are
automatically checked.

## Implementation sequence before M9

The remaining implementation work is not just more permutations of tests. The
first control batch is now implemented; its unavailable patch-restart profile
remains execution debt:

1. **Existing raster/fetch controls — implemented.** [Large points, independent
   depth clipping, instance divisors and list/patch restart](raster-controls.md)
   now have independent enabling, physical limits and direct lowering. Ten native
   profiles pass, including existing dynamic stride with non-unit/zero divisors
   and nonzero first instance. Patch restart is unsupported on the local GPU and
   remains explicitly unverified. No alternative vertex binding mechanism added.
2. **Distinct raster mechanisms — partly implemented.** [Conservative coverage,
   line/stipple modes, provoking vertex and depth convention](raster-mechanisms.md)
   now have independent capabilities, physical properties, static records and
   direct dynamic controls, including independent depth clip enable. Local
   [execution evidence](results/m8-raster-mechanisms-2026-11-07.md) separates
   supported profiles from unavailable dynamic extra overestimation and stipple
   variants. **Next:** programmable sample positions together with depth-image
   compatibility and transition metadata; complete dynamic vertex input/divisor
   descriptions and remaining native dynamic raster controls; inventory the pinned
   registry's extension-specific raster formats. These remain in this batch, not
   silently deferred to M9. No draw-time synthesis or shader substitute for a
   distinct native mechanism.
3. **Resolve geometry capture and dynamic patch state.** Transform feedback,
   geometry streams and associated counters/queries need their own native
   capability/profile; address storage writes are not assumed equivalent. Dynamic
   patch count should use the existing dynamic-state model if it preserves native
   execution and avoids forced executable permutations. Record backend constraints
   instead of embedding a stream-management runtime.
4. **Consolidate the contracts and bounded composition evidence.** Integrate local
   reads into a substantial consumer, exercise available depth/MSAA and stage
   combinations, and distinguish artifact-tool limitations from core omissions.
   Use existing CPU/reference checks and native validation. Collect timing only
   for an unresolved mechanism/cost question; do not make every state addition a
   benchmark project. Publish one M8 disposition table before starting M9.

These are implementation batches, not a promise that every native combination
will be executable on the available GPU. Unsupported hardware and external
backend validation remain explicit evidence debt. A newly discovered native
mechanism that cannot be expressed without lost execution/scheduling capability
belongs in the audit; this list is not a claim to cover every vendor extension.

## Assumptions and proof obligations

Static state may be prepared in executables when the native API also requires
that preparation. It is not an excuse to suppress a meaningful native dynamic
path. Each optional capability must reject unsupported use independently; adding
one cannot silently enable the others. Changing tessellation must not create CPU
geometry generation, shader rewrites, hidden passes or resource tracking.

For the textured consumer, differing triangulation can round interpolants to
opposite sides of nearest-texel ties. The existing CPU oracle's narrow boundary
mask is retained and reported. Every depth value and linear-filter color remains
compared against the original profile; geometry after tessellation and continuation
must preserve image bytes exactly. Correctness here is not a performance-parity
claim and the control is OGPU's VS/FS path, not an independent Vulkan implementation.

M9 ray tracing, M10 memory/interoperability and M11 remaining execution/media
profiles retain their existing order. M6 and native Metal/DX12 acceptance remain
deferred rather than blocking this Linux work.
