# Generated indexed/depth scene: independent SDK handoff

Accepted 2026-10-05 at clean **`4671f67`**, ABI 19. This closes the serial
indexed/depth scene's generated/installed handoff, not M4, replay performance or
the fundamental per-operation argument decision.

## What passed

The relocated SDK consumer builds from its copied C source graph, supplied
generated headers and installed public header/library. Dependency and linkage
checks exclude checkout headers and direct Vulkan imports. Missing-loader
rejection passes. Supplied artifacts need no shader compiler or Cargo in the
consumer workflow; optional regeneration uses the pinned Slang toolchain.

On both RX 5700 XT / RADV and llvmpipe (Mesa 26.2.1), the scene passes all ten
modes × A/B/A frames × UINT16/UINT32 index widths at 257×193. The installed
regeneration check reorders the shared compute root, rejects both stale generated
interfaces, regenerates them, and reruns without changing C host source. All
image/depth/geometry/guard outputs remain byte-identical: **240 installed frames**
across the two drivers and supplied/regenerated variants.

Fresh direct Vulkan runs extract the exact original SDK artifacts, execute
**120 additional native frames**, pass the independent analytic oracle and match
every installed output byte. Each driver compares 47,717,760 bytes across both
installed variants. The native renderer does not link OGPU. The regenerated
compute layouts execute through their generated public interfaces; their outputs
are compared with native original-layout outputs, not passed to a raw-root native
renderer with the wrong layout.

The current compute artifact differs from historical scene artifacts following
earlier producer refactoring. This acceptance therefore uses fresh exact-artifact
native controls rather than inferring shader identity from equal scene results.
All runs use Vulkan and synchronization validation. The shared raw-root
reset/replay path also passes a fresh 72-case, 1,152-frame preflight on each driver;
these short regressions are not new sustained or timing acceptance.

## Boundary and next step

This is a relocatable, independently runnable consumer, not evidence that an
unaffiliated third party has adopted it. Optional installed `--scale` execution
exists but is not accepted by this small-extent handoff. Earlier useful-scale and
replay measurements retain their separate provenance. No native Metal result,
second physical GPU, stable ABI or general compiler support follows. The known
llvmpipe counted→fixed failure remains a strict separate regression, not waived
by these passing scenes.

Next execute the [argument-reuse audit](argument-reuse-plan.md), including
observable payloads, same-address mutation and the native bind-once strategy.
Large-workload parity cannot settle whether repeated caller snapshots belong in
the fundamental interface. Mip/views and blending follow that decision.

## Receipts

[Committed evidence](results/scene-handoff-2026-10-05/) contains the clean SDK
manifest, supplied/regenerated installed reports for both drivers, fresh native
comparison reports, generation/install/relocation logs and raw-root preflight
logs. JSON wrappers preserve every original report field, its local path and
SHA-256; their normalized serialization is not the original report bytes.
Per-frame binary outputs remain at the recorded local paths with hashes.

Reproduce with `examples/graphics_scene/generated.py --check`, `tools/install.py`,
`tools/test-install.py --prefix SDK --indexed-scene --shader-check`, then
`examples/graphics_scene/handoff.py --prefix SDK --consumer COPIED_SCENE` under
the selected ICD and Vulkan/synchronization validation. Record the copied scene
path printed by the installer test. No runtime/API change was needed for this
handoff; the argument-contract question remains open.
