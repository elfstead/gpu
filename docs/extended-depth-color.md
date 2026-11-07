# Extended depth and independent color-write profiles

M8, 2026-11-07. Seven independently selected feature IDs (112–118), three static
records and three C/native Rust setters expose native depth range/units and
whole-attachment write control. Existing foundation revision 12 layouts remain
unchanged. No shader replacement, state history, implicit enabling, allocation or
pipeline creation is added during recording.

| Feature | Native role |
|---|---|
| `UNRESTRICTED_DEPTH_RANGE` | Finite viewport endpoints, depth bounds and user clamp ranges outside [0,1] |
| `DEPTH_CLAMP_CONTROL` | Select viewport-derived or user-defined clamp range, statically or dynamically |
| `DEPTH_BIAS_CONTROL` | Explicit native bias representation and extended setter |
| `DEPTH_BIAS_UNORM` | Format bit-size/mantissa-derived constant bias unit |
| `DEPTH_BIAS_FLOAT` | Constant bias unit of one, independent of depth format |
| `DEPTH_BIAS_EXACT` | Disallow implementation scaling for minimum resolvable difference |
| `COLOR_WRITE_ENABLE` | Whole-attachment enable independent of component masks |

Bias subfeatures require explicitly selecting `DEPTH_BIAS_CONTROL`; none implies
another. Clamp range selection does not itself enable clamping (`DEPTH_CLAMP` and
its static/dynamic enable remain separate). Nonzero bias clamp still requires
`DEPTH_BIAS_CLAMP`. Depth bounds still require the existing test-enable capability
when actually enabling the test. Default ranges retain their prior restrictions.

## State and lowering

Chain `DepthClampControl` (`DEPTH_CLAMP_CONTROL_STATE`, 143) and
`DepthBiasRepresentation` (144) to `RasterOptions`. Clamp mode zero derives the
range from the viewport and ignores min/max. Mode one uses a finite ordered range,
bounded to [0,1] unless unrestricted range is selected. The representation enum is
format-derived (0), forced UNORM unit (1), or float unit (2), with an independent
0/1 exactness flag. Static bias factors remain in `RasterOptions`.

`DYNAMIC_DEPTH_CLAMP_RANGE` (bit 46) selects `set_depth_clamp_range(mode,min,max)`.
The existing `DYNAMIC_DEPTH_BIAS` flag selects all bias parameters, including
representation/exactness set by `set_depth_bias_control(constant,clamp,slope,
representation,exact)`. The old `set_depth_bias` operation remains the native
format-derived/nonexact shorthand. Ignored static clamp/bias values are not
validated; record headers/reserved fields remain checked. The extended bias setter
uses two stack native structures and one command. Static chains and dynamic state
both use the same units; no unit conversion or numerical normalization occurs.

Chain `ColorWriteState` (145) to `GraphicsState` for static whole-attachment enables;
its count matches `color_count`. Without it, every attachment is enabled.
`DYNAMIC_COLOR_WRITE_ENABLE` (bit 47) ignores that static count/pointer and uses
`set_color_write_enable(enables)` (slice in Rust, count/pointer in C). Values are
0/1; count does not exceed max colors. A zero-count call is a no-op, emitting no
native command. Caller establishes every consumed target's state and observes
native static/dynamic invalidation, secondary/generated and replay rules.

The Vulkan mappings are `VK_EXT_depth_range_unrestricted`,
`VK_EXT_depth_clamp_control`, `VK_EXT_depth_bias_control`, and
`VK_EXT_color_write_enable`. Each device feature is queried and enabled explicitly;
only selected optional commands are loaded. Static preparation uses direct native
extension structures and command-time arrays are borrowed without packing.

## Why color-write enable is retained

A false attachment enable suppresses every component; a true enable preserves the
component mask. If dynamic component masks are supported and the application knows
the current mask, it can build this behavior by setting zero and restoring the
mask. That helper does not supply a native enable path on devices exposing
`colorWriteEnable` without `extendedDynamicState3ColorWriteMask`. Using the existing
mask API alone would either lose that path or force pipeline variants, state
tracking or an implied stronger capability. Core therefore retains the independent
native enable profile. It does not add a mask-restoration helper or automatically
switch between implementations. Cross-backend availability remains explicit.

## Local evidence and boundaries

RX 5700 XT / RADV Mesa 26.2.5 passes five depth profiles: unrestricted viewport
mapping, unrestricted bounds acceptance/rejection, user clamp ranges, exact float
bias and exact forced-UNORM bias on D32. There are 20 submissions / 1,280
color-depth pixel pairs with changed input, numerical depth checks and guards.
The viewport profile checks the ordinary dynamic viewport path against CPU values;
the other four compare static/dynamic state. Five malformed recordings fail.
Ignored static values deliberately include invalid enums/NaNs.

The existing extended-control fixture adds a twelfth profile for whole-attachment
write enables over different component masks, comparing static/dynamic output at
1/4 samples on two targets. Its new total is 96 submissions / 12,288 pixels and
12 rejected recordings. This is direct-lowering and functional evidence, not a
performance comparison. Native devices also exercise independent enables and
reject unrequested neighboring capabilities.

Other depth formats, bias/sample-shading combinations, geometry/mesh, secondary/
generated composition and Metal/DX12 remain separate acceptance work. The format
inventory and dynamic tessellation/capture mechanisms are next in M8.

Native references: [bias representation capabilities](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceDepthBiasControlFeaturesEXT.html),
[depth bias units](https://docs.vulkan.org/spec/latest/chapters/primsrast.html#primsrast-depthbias-computation),
[clamp ranges](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetDepthClampRangeEXT.html),
and [independent color-write state](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineColorWriteCreateInfoEXT.html).

Verification: 118 host tests, two native Rust correspondence tests and 31 native GPU
library tests pass. Full C regression passes with zero Vulkan validation errors and
four existing unused-output warnings. Strict Clippy, 2,347 ABI values, the surface
inventory (190 operations / 160 records / 297 constants / 1,532 values), pinned
binding reproduction and 21 tooling tests pass. All shader artifacts are unchanged
from the preceding validated viewport commit; the fixture still takes 72 inputs.

Clean committed SDK acceptance at `4b6b2ca` passes the optimized installed C fixture
and isolated native Rust build/correspondence/missing-loader/headless checks. The
[machine-readable receipt](results/m8-depth-color-2026-11-07.json) records provenance
and hashes; no new native warnings were introduced.
