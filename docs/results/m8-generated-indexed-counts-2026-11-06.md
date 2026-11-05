# M8: generated indexed and nested-count graphics

2026-11-06, following `52592a6` and the
[first generated-graphics receipt](m8-generated-graphics-2026-11-06.md).
The existing foundation-7 API now has local execution evidence for generated
DRAW, DRAW_INDEXED, DRAW_COUNT and DRAW_INDEXED_COUNT. No runtime or public
contract change was necessary; culling, compaction and draw-list construction
remain consumer code.

## Expanded consumer

The same procedural grid fixture now uses padded 64-byte sequence records.
Indexed records use a nonzero first index and a negative base vertex, with a
leading index guard. Nested-count records contain GPU-written addresses, strides
and counts for separate padded draw lists. Two nested draws render distinct
left/right halves of a tile through their first-instance values, so ignoring a
nested draw or its first instance changes the checked image.

Ordinary and indexed cases each cover five visible sets. Each nested form covers
seven cases: empty sequence counts, zero nested counts, one draw, a requested
count above the two-draw maximum, mixed per-object zero/one/two counts, a zero
execution maximum and a one-draw maximum. Full visibility also exercises the
absent optional sequence-count span. The native count limit is a clamp, as specified
by [VkGeneratedCommandsInfoEXT](https://docs.vulkan.org/refpages/latest/refpages/source/VkGeneratedCommandsInfoEXT.html).
The [native nested wire record](https://docs.vulkan.org/refpages/latest/refpages/source/VkDrawIndirectCountIndirectCommandEXT.html)
contains an indirect-buffer address, byte stride and command count; OGPU forwards
these GPU bytes without scanning or rewriting them.

Both explicit and integrated preprocessing execute all four forms. Explicit
producer dependencies cover preprocessing and indirect reads of the generated
and nested records. Host-written indices are flushed before submission. The
consumer queries preparation requirements after every executable-set update and
for the exact work limits, then destroys that case's preparation storage only
after completion and arena reset. This also corrects the earlier graphics fixture's
query-before-set-update ordering; no runtime allocation policy is introduced.

## Findings

The first indexed run produced an empty image despite matching generated records.
An ordinary indexed control using the same shader and index bytes failed the same
way. Inspection of the pinned Slang output showed that `SV_VertexID` subtracts the
base vertex and `SV_InstanceID` subtracts the first instance. The fixture needed
the native indices, so it now uses `SV_VulkanVertexID` and `SV_VulkanInstanceID`,
consistent with the existing ordinary graphics fixtures. The negative-base and
split-tile cases exercise both distinctions. No driver workaround or OGPU lowering
change was needed; the temporary diagnostic control is not retained as a fallback.

Graphics executable selection remains independently gated and unavailable on the
local Radeon driver. Its branches compile, but graphics set-update execution is
not accepted by this receipt. Every action also has its own native token-support
gate; skipping a token prints an explicit unverified result.

## Verification and remaining scope

On Radeon RX 5700 XT, RADV/Mesa 26.2.5, with Vulkan and synchronization validation:

- **48 submissions, four action forms, two preprocessing modes, 98,304 pixels pass.**
- Whole 32 KiB allocations match for every submission: sequence records, nested
  records, count, indices, image readback and untouched guards.
- The full public C fixture passes without validation errors. Existing unrelated
  shader-output warnings remain. C11 compilation treats warnings as errors.
- Slang 2026.14.1 shader reproduction and SPIR-V validation pass. The runtime and
  public ABI are unchanged, so no new native performance or backend-parity claim
  is made.

Reproduce with `cargo xtask foundation-shaders --check` and
`cargo xtask foundation` under the declared loader/ICD/validation environment.
The final local log is `/tmp/m8-nested-gpu-final.log`; reproduction is authoritative,
not availability of that temporary file.

Next are native generated vertex/index-state tokens and remaining advanced raster
profiles. Generated mesh and graphics pipeline selection still need suitable
native hardware evidence. Larger textured/depth scenes, separate consumer
packaging and any unresolved performance questions remain outside this bounded
fixture. M8 as a whole is not complete.
