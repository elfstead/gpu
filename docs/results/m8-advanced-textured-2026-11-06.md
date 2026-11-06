# M8 advanced raster composition acceptance

2026-11-06, foundation revision 11. This step adds a public C/installed SDK
consumer composing the existing textured scene with geometry, tessellation and
render continuation. No runtime or public layout changes are needed.

## Validation correction

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
tessellation/geometry. Clean installed acceptance below supersedes the failed run.

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

## Clean installed acceptance

Implementation `8d52135cb73be8d1b92dcccbf815f95936d595fc`, installed with
`source_dirty=false` into `/tmp/ogpu-m8-advanced-fixed-sdk`. Radeon RX 5700 XT,
RADV/Mesa 26.2.5, Vulkan validation layers 1.4.357 with synchronization validation,
Slang 2026.14.1. Release runtime SHA-256 is unchanged:
`1b86e9093f29602e5a669f79b7ba74d45fdf3bd62f1d17554ddcfffb044d2674`.

All **180 frames** pass across five profiles at **257×193 and 640×360**. The
independent CPU oracle checks **24,073,530 pixels** of 25,200,090 image pixels;
its pre-existing polygon/nearest-filter boundary exclusions remain explicit.
Generated geometry, indices, indirect records, texture bytes and guards match;
A/B/A replay and view-relative mip equivalence are exact. There are **zero
validation errors or warnings** on either output stream.

Geometry-only output is byte-identical to VS/FS. Tessellation versus the control
has at most one byte of color difference in the checked region and linear-filter
cases; all depth values agree within 1e-6. Nearest-filter boundary differences
above the two-byte tolerance occur at 1,298 pixels across the 18 frames at 257×193
and 27 pixels at 640×360 (maximum byte deltas 93 and 173). These are reported, not
silently counted as exact agreement. Adding geometry after tessellation and then
continuation preserves every image byte exactly at both extents.

The isolated build uses C11/Python/pkg-config plus the installed header and
library, with Cargo and Slang disabled for normal build/run. Dependency and
runtime resolution checks pass. Missing-loader execution leaves an incomplete
receipt with no accepted runs. Optional regeneration with the pinned compiler
reproduces all three artifacts and passes SPIR-V validation.

The unchanged textured consumer separately passes 36 supplied-artifact and 36
reordered/regenerated-root frames at `da5919a`. The corrected SDK's macro-disabled
preprocessed source matches after normalizing only diagnostic line numbers.
The surface audit passes with 154 operations, 138 records, 185 constants and
1,284 correspondence values. No runtime changes warranted repeating the unrelated
GGML/ML, native Rust or full library suites from the prior accepted checkpoint.

The [machine receipt](m8-advanced-textured-2026-11-06.json) records exact identities,
checks, per-profile comparisons and evidence hashes. Optional 720p, other hardware,
external backends and unlisted advanced combinations remain unaccepted.

Reproduce with the local Vulkan loader/driver and validation environment configured:

```sh
python3 tools/install.py --prefix /absolute/new/sdk
python3 tools/test-foundation-install.py --prefix /absolute/new/sdk \
  --consumer advanced-raster --shader-check
```
