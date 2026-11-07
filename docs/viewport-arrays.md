# Viewport and scissor arrays

M8, 2026-11-07, foundation revision 12. Two direct C/native Rust operations extend
viewport/scissor state to the hardware array model. `Viewport` and `Scissor` are
plain native-layout values; arrays are borrowed for the call, without allocation,
scratch, retained pointers, slot tracking or automatic draw expansion.

`set_viewports(first, values, flags)` and `set_scissors(first, values, flags)` use
Rust slices; C supplies count/pointer pairs. With flags zero, they update a range
for a pipeline using fixed counts. With `SET_COUNT`, they replace the active count
and values: first must be zero and count nonzero. A zero-count range update emits
no native command and does not invalidate previously established count state.

Chain `ViewportCounts` (`VIEWPORT_COUNTS`, 142) to `GraphicsState` to set prepared
counts; both default to one. `DYNAMIC_VIEWPORT_COUNT` / `DYNAMIC_SCISSOR_COUNT`
(bits 44/45) independently ignore those static counts and select the native
with-count states. Keep the existing `DYNAMIC_VIEWPORT_SCISSOR` value-state bit.
The two effective counts must match at every draw; set every consumed slot and
obey native invalidation and secondary/generated-command rules. A fixed-count
range update does not establish dynamic-count state. The old `set_graphics_state`
operation is a slot-zero, fixed-count shorthand for the same native setters.

Query `VIEWPORT_LIMITS` (28): maximum array capacity and viewport subpixel bits.
Dimensions and coordinate bounds remain in `GraphicsLimits`. Explicitly request
`MULTI_VIEWPORT` (110) for counts or indices beyond slot zero and
`SHADER_OUTPUT_VIEWPORT_INDEX` (111) for shader stages requiring that capability.
Stage-specific shader requirements still apply; the latter feature does not imply
the former. Geometry-stage routing uses its own stage capability and multi-viewport
rules. Index values produced by shaders remain caller proof, not host reflection.

Viewports permit negative height and reversed depth endpoints, with finite values
and current native range rules. Scissors allow zero extents, nonnegative offsets,
and no signed offset-plus-extent overflow. Both operations check range overflow,
physical count and requested multi-viewport enabling before reading input arrays.

The previous command-list `viewport_set` boolean has been removed. It proved only
that one combined setter had been called, not that native state survived a bind or
that every slot/count was established. The public contract now treats all complete
dynamic state consistently as caller-owned. Local malformed-value diagnostics and
render/pipeline scope checks remain. This removes a hot-path field/check rather than
adding per-slot state histories. Programs must not rely on diagnostics for an unsafe
draw missing required state. An old negative test that relied on that diagnostic
was changed to reject a malformed viewport value before any native draw.

## Evidence and remaining scope

Five GPU profiles compare array routing to explicit slot-zero draws: fixed counts,
both counts dynamic, viewport count dynamic, scissor count dynamic, and counts
changing from one to two between draws. At 1/4 samples, two shader-selected targets,
changed-data replay and guards produce 40 submissions / 5,120 checked pixels.
The CPU oracle checks colors per pixel. Five malformed recordings are rejected.
Host tests cover ranges/counts, ignored static counts, capability independence,
negative heights/reversed depth and empty scissors. Native device tests independently
enable the two new features and reject unavailable multi-viewport values.

Geometry/mesh viewport routing, layered/multiview combinations, secondary/generated
work and other backends retain separate execution debt. The native Vulkan baseline
supplies both range and with-count commands; no extension or emulation is needed
for the single-viewport count-setting path.

References: [native range/count semantics](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetViewportWithCount.html)
and [prepared count constraints](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineViewportStateCreateInfo.html).

Local verification: 116 host tests, two Rust/C correspondence tests and all 31
native GPU library tests pass. Full C regression has zero validation errors and
four existing unused-output warnings. Strict Clippy, 2,332 ABI values, the surface
inventory (187 operations / 157 records / 280 constants / 1,497 values), pinned
binding/shader reproduction and 21 tooling tests pass. The two new shader artifacts
are pinned; the full fixture now takes 72 artifacts. The first full run exposed
the obsolete missing-viewport negative test described above; the corrected run
passes without weakening a valid-draw obligation or suppressing native validation.
