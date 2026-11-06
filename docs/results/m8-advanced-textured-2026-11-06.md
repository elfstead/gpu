# M8 advanced raster composition acceptance

2026-11-06, foundation revision 11. This step adds a public C/installed SDK
consumer composing the existing textured scene with geometry, tessellation and
render continuation. No runtime or public layout changes are needed.

## Validation status

Initial implementation `da5919a` passed the CPU image checks at 257×193, but its
first installed run correctly failed validation: the continuation profile set
SUSPENDING without recording its RESUMING segment. The quick probe had inspected
only stderr and missed diagnostics on stdout. Its apparent success is not native
acceptance. The installed runner checks both streams and retains an incomplete
receipt at `/tmp/ogpu-foundation-external-04ordo9t/independent advanced-raster/run-vj92n7ll/report.json`.

The correction ends the first segment immediately after the far draw, begins the
matching RESUMING segment, then records the overlapping near draw. There is no
action or synchronization between segments. The corrected 18-frame 257×193 probe
has no validation error on either stream and is byte-identical to uninterrupted
tessellation/geometry. Clean installed two-extent acceptance remains pending.

## Scope

Five profiles cover VS/FS control, geometry, tessellation, both stages and both
stages with continuation. Each runs six texture/mip/filter/viewport cases with
A/B/A replay. Base tessellation factor is two and overlay factor four. Generated
geometry/indices/indirect draws, heaps, depth, blending and scissors all use the
existing consumer mechanisms. The fixed offline Slang recipe does not claim
general stage-interface linking.

Different triangulation can select opposite texels at nearest-filter ties.
Comparisons retain the existing CPU oracle's boundary mask and count those
differences; linear-filter color and every depth value remain compared. Geometry
after tessellation and continuation must preserve image bytes exactly. The control
is OGPU's VS/FS path; this is not a pure Vulkan or performance-parity comparison.

See the [M8 coverage audit](../m8-coverage.md) for the next implementation batches
and separate native/compiler evidence gaps.
