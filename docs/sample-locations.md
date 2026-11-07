# Programmable sample locations

Foundation revision **12**, added 2026-11-07. C and native Rust share this native
mechanism below the C boundary. Revision 12 appends `ImageBarrier.next`: rebuild
all callers against the matching header/runtime/source commit. Set it to NULL for
ordinary barriers. Do not mix revision-11 barrier arrays with revision 12.

## Pattern and raster state

`SAMPLE_LOCATIONS` independently enables custom positions. Its physical query,
`QUERY_SAMPLE_LOCATION_LIMITS`, reports a sample-count bitmask, maximum grid,
coordinate clamp range, subpixel precision, whether patterns may vary within a
rendering instance, and seven per-count grid limits. `grids[i]` describes sample
count `1 << i`; unsupported counts have zero grids. Discovery enables nothing.

`SampleLocations { header, samples, grid, count, locations }` contains an array of
`SampleLocation { x, y }`. Its header has `next=NULL`. Grid dimensions are nonzero
and evenly divide the queried grid for that sample count. The array length is
exactly `samples * grid.width * grid.height`, with checked arithmetic. Index
`((y * grid.width + x) * samples + sample)` selects a location. Coordinates are
finite, measured from the pixel's upper-left corner. Values outside the physical
range are passed through for native clamping; OGPU does not normalize, quantize,
reorder or copy the coordinate array. Native precision still applies.

`SampleLocationState { header, enable, reserved, locations }` is another optional
member of `RasterOptions.header.next`, alongside the [raster mechanisms](raster-mechanisms.md).
`enable` is boolean, `reserved=0`. Static enabled state needs a valid pattern whose
sample count equals the executable's raster samples. Disabled state ignores its
pattern. The existing executable mask gains:

- `DYNAMIC_SAMPLE_LOCATIONS`: select a pattern with `set_sample_locations`; this
  requires `SAMPLE_LOCATIONS`, without another feature ID.
- `DYNAMIC_SAMPLE_LOCATIONS_ENABLE`: select enable with `set_sample_locations_enable`;
  this requires its independent feature plus `SAMPLE_LOCATIONS` explicitly.

The two controls are independent. Only selected dynamic fields ignore static
values; headers and reserved fields remain checked. Initialize dynamic values and
observe existing static/dynamic bind-order rules. Draw-time sample compatibility,
pass/secondary/continuation pattern consistency when `variable=0`, shader
`InterpolateAtSample` restrictions and shading-rate interaction rules remain
caller obligations. There is no bound-pipeline or last-pattern tracker.

## Depth images and dependencies

Depth attachments used with custom positions need the explicit image flag
`IMAGE_SAMPLE_LOCATIONS_DEPTH`. Image-profile queries and creation pass that flag
to the native API; it requires the sample-location capability and a depth-bearing
format. The actual format/sample/usage combination still needs its ordinary query.

Preserving the depth contents of such an image across a layout transition needs
the pattern that last wrote those subresources. Put that `SampleLocations.header`
on **each corresponding `ImageBarrier.next`**. It must be a single record with
`next=NULL`, a matching sample count and a compatible-depth image. Ordinary
barriers, split release/acquire and ownership-transfer descriptions share this
metadata path. Split endpoints still require matching descriptions and explicit
event-cycle ordering. Coordinates are borrowed only through the native call.

Supplying a pattern is a declaration of the existing contents, not a request to
convert them. The caller proves it matches the last write. Omitting metadata on a
layout transition of compatible depth leaves depth contents undefined under the
native rule; OGPU deliberately allows that instead of inferring a pattern.
Changing the pattern used to render existing depth requires clearing depth first.
Discard still requires synchronization against prior uses. Ordinary depth images
and barriers without custom locations retain their existing behavior.

Dynamic rendering has no automatic subpass transitions and needs no new render-pass
object or begin/end metadata. The explicit image dependencies are the complete
transition boundary. Color images do not need historical pattern metadata.

## Implementation and cost

Vulkan lowering uses `VK_EXT_sample_locations`, its image flag and native
sample-location records. Static state extends native multisample state. The
pattern setter issues one native command; dynamic enable uses its independent
extended-dynamic-state3 feature/command. Pattern validation is linear in the caller's
coordinate count. It introduces no allocation, pattern rewrite, pipeline variant,
implicit transition or per-draw work.

Existing barrier/split scratch queries reserve native metadata storage when
`SAMPLE_LOCATIONS` is enabled. They still inspect counts and immutable capabilities,
not nested resource/coordinate arrays. Disabled devices retain their original
scratch sizes. Native metadata is written only for supplied records, and all local
validation finishes before a barrier/wait is emitted. The coordinate array stays
caller-owned; no native packet storage or pointer survives the call.

This is a direct mapping argument, not a measured universal performance-parity
claim. A backend must preserve custom coverage and depth-content semantics or
report the capability unavailable. Metal/DX12 implementation and native acceptance
are not implied by the public records.

## Verification and remaining work

The [C fixture](../examples/foundation_sample_locations.h) and
[receipt](results/m8-sample-locations-2026-11-07.md) cover static/dynamic patterns,
independent dynamic enable, 2×2 grids, mixed per-sample coverage, sloped depth,
explicit barrier and split transitions, LOAD preservation, replay and guards.
Unavailable counts, fixed-pattern devices, shading-rate, secondary/continuation,
multiview and external backends remain separate acceptance work.

M8 next implements complete dynamic vertex input/divisor descriptions, remaining
native dynamic raster controls and the extension-format inventory, followed by
capture/dynamic patch and consolidation. See the [coverage audit](m8-coverage.md).

Native semantics follow the pinned registry and the primary
[sample-location extension](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_sample_locations.html),
[pattern description](https://docs.vulkan.org/refpages/latest/refpages/source/VkSampleLocationsInfoEXT.html),
[coordinate clamping](https://docs.vulkan.org/refpages/latest/refpages/source/VkSampleLocationEXT.html)
and [image dependency contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkImageMemoryBarrier2.html).
