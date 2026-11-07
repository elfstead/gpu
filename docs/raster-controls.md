# Native raster and instance-fetch controls

Foundation revision 11, added 2026-11-06. Exact source/header/artifact revisions
must still match. This batch adds six independent feature IDs, a physical limits
query and two optional state records; existing records and operations keep their
layouts. C and native Rust expose the same values below the C boundary.

| Feature | Selection and native mechanism |
|---|---|
| `LARGE_POINTS` | Shader `PointSize` values other than one; enables native `largePoints`. No host point expansion. |
| `DEPTH_CLIP` | `DepthClipState` on `RasterOptions.header.next`; lowers to `VK_EXT_depth_clip_enable`. |
| `VERTEX_DIVISOR` | Instance divisors greater than one, through Vulkan 1.4 enabling and vertex-input state. |
| `VERTEX_ZERO_DIVISOR` | Independent zero-divisor enable; every instance fetches the same element. |
| `LIST_RESTART` | Existing `primitive_restart` option for point/line/triangle lists, including adjacency. |
| `PATCH_RESTART` | Existing restart option for patch lists, independently enabled. |

`QUERY_RASTER_CONTROL_LIMITS` reports the native point range/granularity, maximum
vertex divisor, support for nonzero first instance with non-unit divisors, and
whether polygon point mode uses shader point size. Discovery does not enable these
features. In particular, zero divisors do not silently enable other divisors and
patch restart does not silently enable tessellation.

## State and caller rules

`DepthClipState { header, enable, reserved }` explicitly controls Z clipping.
`enable` is zero or one and `reserved` is zero. `DEPTH_CLIP` is required even for
explicitly disabled clipping. Omitting the record preserves the native rule that
depth clamping also disables Z clipping; supplying it makes these two controls
independent. The separate `DEPTH_CLAMP` enable still governs clamping. This is
static executable state here; the subsequent [raster mechanisms](raster-mechanisms.md)
batch also exposes independently enabled dynamic clipping.

`VertexDivisors { header, count, reserved, entries }` attaches to
`VertexInput.header.next`. Each `VertexDivisor { binding, divisor }` names a unique
declared instance-rate binding. Omitted bindings use divisor one. Zero requires
`VERTEX_ZERO_DIVISOR`; values greater than one require `VERTEX_DIVISOR`; all values
must fit the physical maximum. Per-vertex bindings cannot carry a divisor entry.
`VertexDivisors` requires `header.next=NULL`. `DepthClipState` participates in the
bounded [raster mechanism chain](raster-mechanisms.md). Unknown or repeated records
fail during preparation. Metadata is borrowed only for that preparation.

For local instance number `i`, the fetched element is
`firstInstance + floor(i / divisor)`. With divisor zero it is `firstInstance`.
When `nonzero_first_instance` is zero, any draw using a divisor other than one
must specify first instance zero. This also applies to indirect/generated data.
The caller proves it, just as it proves vertex/index buffer bounds. There is no
retained current-pipeline tracker or per-draw inspection. Existing static/dynamic
stride, binding offsets and native vertex formats continue to apply.

Restart uses the all-ones sentinel for the bound index type, discarding an
incomplete primitive/patch before restarting assembly. The existing indexed draw,
indirect draw and generated-command paths carry the same state. The new enables
remove the previous list/patch exclusion; they do not introduce another draw API.

Point size is shader output, positive and subject to native range clamping and
granularity. Values other than one require `LARGE_POINTS`; explicit writes in
tessellation/geometry also require `TESSELLATION_GEOMETRY_POINT_SIZE`. The baseline
maintenance profile supplies size one when no point size is written. The fixed
fixture uses Slang `SV_PointSize`; general compiler tooling is not newly certified
for arbitrary point/stage interfaces.

## Cost and verification

Native extension discovery/enabling and state translation happen during snapshot,
device creation and executable preparation. Divisor entries use a temporary cold
array, like existing vertex binding/attribute records. Commands, shader execution,
submission and memory ownership gain no extra work, hidden allocations, shader
rewrites or CPU/GPU conversion passes. This establishes a direct mapping, not a
measurement of universal performance parity.

The [public C fixture](../examples/foundation_raster_controls.h) passes on Radeon
RX 5700 XT with synchronization validation: ten supported profiles, two changed-data
replays each, 1,280 pixels and six rejected malformed preparations. It checks point
sizes four/six; negative/positive Z with all four clip/clamp combinations; list
restart discarding an incomplete triangle with pipeline-statistic confirmation;
and divisors zero/two with static/dynamic stride and first instance zero/one.
Input bytes and guards remain unchanged. Patch-list restart is reported unsupported
on this GPU; its native execution is explicitly unverified.

Host checks cover limits, independent enables, unknown topology, duplicate/rate
errors and malformed extension records. Native device checks exercise independent
enabling and unavailable-feature rejection. Existing C and GPU-library regressions
pass. See the [acceptance receipt](results/m8-raster-controls-2026-11-06.md).

Point/line/adjacency restart execution, point output from other stages, polygon
point mode, generated/secondary/multiview combinations and external backend
execution remain separate evidence work. [Complete dynamic divisor/input descriptions](dynamic-vertex-input.md) are now
implemented with caller scratch. Remaining dynamic raster controls still need work. Independent
dynamic clipping and several distinct [raster mechanisms](raster-mechanisms.md)
are now implemented; see the [coverage audit](m8-coverage.md) for remaining work.

Native semantics are checked against the pinned registry and the primary
[divisor proposal](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_vertex_attribute_divisor.html),
[depth-clip contract](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_depth_clip_enable.html)
and [point rasterization contract](https://docs.vulkan.org/spec/latest/chapters/primsrast.html).
The artifact uses [Slang's documented SPIR-V semantics](https://github.com/shader-slang/slang/blob/master/docs/user-guide/a2-01-spirv-target-specific.md),
including `SV_VulkanInstanceID` for the native instance index.
