# Native raster mechanisms

Foundation revision 11, added 2026-11-07. Match exact source/header/artifact
revisions; this additive surface is not a stable-ABI promise. C and native Rust
share the implementation below the C boundary.

## State and capabilities

`RasterOptions.header.next` accepts at most one each of `DepthClipState`,
`ConservativeState`, `LineState`, `ProvokingState` and `DepthConventionState`, in
any order. Preparation borrows the records and rejects unknown, repeated, cyclic,
undersized or malformed entries. Reserved fields remain zero. Existing record
layouts are unchanged. Omission preserves native defaults.

| Record | Native meaning and required enable |
|---|---|
| `ConservativeState` | Disabled, overestimate or underestimate coverage, plus extra overestimation in pixels. An explicit record requires `CONSERVATIVE_RASTER`; underestimation additionally requires its queried property. Extra size is finite, nonnegative and within the physical maximum. |
| `LineState` | Default, rectangular, Bresenham or smooth rectangular rasterization. Each nondefault mode has its own feature. Stipple enable, repeat factor (1–256) and 16-bit pattern use separate per-mode stipple features. Default-mode stipple needs `STIPPLED_RECTANGULAR_LINES` and `strict_lines`. |
| `ProvokingState` | First or last vertex supplies flat-interpolated values. An explicit record requires `PROVOKING_LAST`, including first mode. |
| `DepthConventionState` | Z clip interval `[0,w]` or `[-w,w]`, including its associated viewport depth transform. An explicit record requires `DEPTH_NEGATIVE_ONE_TO_ONE`. This is independent of clip enable and depth clamping. |
| `DepthClipState` | Independent Z clip enable, as in the [previous controls](raster-controls.md). Requires `DEPTH_CLIP`. |

`QUERY_RASTER_MECHANISM_LIMITS` exposes native conservative overestimation size,
maximum extra size and granularity; underestimation, point/line and degenerate
primitive support; fully-covered fragment input and post-depth-coverage support;
pass-wide versus per-pipeline provoking mode; line subpixel precision and strict
line rasterization. These are physical properties, not implicit enables or a
promise of shader-toolchain support for every related built-in. The base profile
already supports Vulkan 1.4, including native line rasterization structures.

## Dynamic state and caller obligations

The existing executable dynamic-state mask gains independently selected conservative
mode, extra overestimation, provoking vertex, clip enable, depth convention, line
mode, stipple enable and stipple parameters. Each has a direct setter in C and
native Rust. `GraphicsLimits.dynamic_states` advertises physical availability;
preparation checks the enabled features. Dynamic mode/extra, provoking, clip and
convention features require their corresponding base enable explicitly; they do
not select it for the caller. Dynamic line mode and stipple enable do not require
an arbitrary particular line mode. Stipple parameters need at least one enabled
stipple feature and the core Vulkan 1.4 command, without another feature ID.

Selected dynamic fields ignore their static values; other fields and record
headers/reserved values are still checked. Initialize every selected field before
drawing and follow existing static/dynamic pipeline bind ordering rules. The
caller proves compatibility at execution time, including:

- The current line mode and stipple feature, default-mode strict-line constraint,
  and native restrictions on sample shading and alpha-to-coverage/alpha-to-one
  for the chosen line rasterizer.
- Conservative primitive/output-mode restrictions, including unsupported
  point/line rasterization. A topology alone does not prove a geometry or mesh
  shader's output primitive type.
- When `provoking_per_pipeline` is false, native pass-wide provoking-mode
  consistency across bound pipelines.
- Shader interfaces, sample counts and any shader built-in requirements for the
  selected raster profile.

Preparation and setters validate local enums, booleans, bounds and enabled
capabilities. They do not keep a current-pipeline cache, inspect generated draws,
or repair incompatible combinations. C recording errors retain the existing
sticky recording status; Rust uses the same checked recording path. Caller proof
obligations are part of the existing trusted low-level contract, not a new runtime
validation layer.

## Mapping and cost

Conservative rasterization, provoking vertex and depth convention map to their
native Vulkan extension structures. Line modes map to Vulkan 1.4 state. Dynamic
controls map directly to their respective extended-dynamic-state3 commands, except
line stipple parameters, which use the core command. Optional commands are loaded
only when selected; unsupported mechanisms never become baseline prerequisites.

All state-chain translation and temporary metadata remain in executable preparation.
Each new setter performs bounded local checks and one native command. No shader
rewrite, extra render pass, automatic pipeline variant, allocation, resource
tracking or per-draw work is introduced. This is lowering evidence, not a measured
claim of performance parity across hardware or backends.

## Evidence and remaining work

The [public C fixture](../examples/foundation_raster_mechanisms.h) checks changed-input
replay, exact flat colors and depth visibility, conservative coverage, line/stipple
coverage, static/dynamic equivalence, combined records, malformed preparation and
input/readback guards. The [receipt](results/m8-raster-mechanisms-2026-11-07.md)
separates supported Radeon profiles from unsupported ones. Smooth-line coverage
is bounded; the fixture does not claim an exact universal antialiasing oracle.

[Programmable sample positions](sample-locations.md) now implement depth-image
compatibility and explicit transition metadata in revision 12. Dynamic complete vertex input/divisors,
remaining native raster controls, the extension-format inventory, geometry
capture and dynamic patch state remain in [M8](m8-coverage.md). No Metal/DX12
implementation, unavailable native profile, general shader-interface tooling or
milestone completion is implied.

Native contracts are checked against the pinned Vulkan registry and the primary
[rasterization chapter](https://docs.vulkan.org/spec/latest/chapters/primsrast.html),
[conservative properties](https://registry.khronos.org/vulkan/specs/latest/man/html/VkPhysicalDeviceConservativeRasterizationPropertiesEXT.html)
and [depth convention extension](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_depth_clip_control.html).
