# Extended dynamic raster controls

2026-11-07. Foundation revision 12 adds eleven independent capabilities (99–109),
dynamic state bits (33–43), and matching C/native Rust setters. They expose
native command state for mechanisms already represented in static executables.
There is no current-state cache, executable permutation cache, resource walk,
hidden compilation, allocation, or caller scratch in these setters.

| Feature / dynamic state suffix | Setter arguments after encoder | Ignored static field |
|---|---|---|
| `DEPTH_CLAMP` | `set_depth_clamp(enable)` | `RasterOptions.depth_clamp` |
| `POLYGON_MODE` | `set_polygon_mode(mode)` | `RasterOptions.polygon_mode` |
| `RASTERIZATION_SAMPLES` | `set_rasterization_samples(samples)` | `GraphicsState.samples` |
| `SAMPLE_MASK` | `set_sample_mask(samples, mask)` | `RasterOptions.sample_mask` |
| `ALPHA_TO_COVERAGE` | `set_alpha_to_coverage(enable)` | `RasterOptions.alpha_to_coverage` |
| `ALPHA_TO_ONE` | `set_alpha_to_one(enable)` | `RasterOptions.alpha_to_one` |
| `LOGIC_OP_ENABLE` | `set_logic_op_enable(enable)` | `GraphicsState.logic_op_enable` |
| `COLOR_BLEND_ENABLE` | `set_color_blend_enable(first, count, values)` | Each `ColorState.blend` |
| `COLOR_BLEND_EQUATION` | `set_color_blend_equation(first, count, equations)` | Each color/alpha factor and operation |
| `COLOR_WRITE_MASK` | `set_color_write_mask(first, count, masks)` | Each `ColorState.write_mask` |
| `LOGIC_OP` | `set_logic_op(op)` | `GraphicsState.logic_op` |

The complete feature name is `FEATURE_DYNAMIC_<suffix>`; the pipeline flag is
`DYNAMIC_<suffix>`. Rust uses `feature_ids::DYNAMIC_<suffix>` and
`DYNAMIC_<suffix>_STATE`. Query available IDs and graphics dynamic-state limits,
then explicitly request the features used. An advertised but unrequested feature
cannot be used for preparation or recording. Values of ignored static fields are
not validated or used to infer additional feature requests.

## Values, arrays and draw obligations

Booleans are 0/1. Polygon, logic, factor and equation enums are the existing static
state enums. Enabling depth clamp needs `DEPTH_CLAMP`; line/point polygon modes need
`NON_SOLID_FILL`; enabling alpha-to-one or logic operations needs `ALPHA_TO_ONE`
or `LOGIC_OP`. These base features are value dependent, independently enabled and
never requested implicitly. Secondary-source factors need `DUAL_SOURCE_BLEND`.

`BlendEquation` is six `u32` values: source color, destination color, color operation,
source alpha, destination alpha, alpha operation. Per-attachment operations borrow
arrays for the call. C takes counts/pointers; native Rust takes slices. The native
Vulkan representation matches directly, with size/alignment/offset assertions.
No packing or scratch storage is needed. Zero count is a no-op after feature and
range checks, permits a null pointer, and emits no zero-count native command.
`first + count` must not overflow or exceed the queried attachment limit.

Set every consumed attachment slot and every required scalar before a draw.
Native state invalidation on static pipeline binds, secondary command buffers,
replay and generated work still applies. Callers prove attachment/format, shader,
dual-source output/attachment limits and sample-count compatibility. Dynamic blend
enabling on a format without blending support is invalid; the backend does not
track the current formats to revalidate each setter. Static attachment state still
obeys the native independent-blend restriction; ignored dynamic values are removed
before that check. This work does not add a cross-command state registry.

Sample counts are powers of two in [1,64]. The dynamic mask supplies a `u64` and a
sample count at least as large as the effective rasterization count. Only bits
within that declared count are relevant. Actual draw counts must satisfy image,
rendering, format, sample-shading and custom-position constraints. Render scopes
and secondary inheritance continue to declare their actual attachment counts.
They do not inherit an executable's static sample-count placeholder.

A dynamic rasterization count leaves a static 64-bit mask intact. Vulkan still
uses its static `rasterizationSamples` field to size that mask, even when the draw
count is dynamic. The backend therefore uses 64 when the mask is static and 1
when both states are dynamic. These are cold native representation choices, not
draw restrictions or additional physical sample-count requirements. Two local
`u32` words preserve all 64 mask bits; neither a null-mask extension nor allocation
is required. Static custom positions are checked against their own queried limits;
when count is dynamic, their equality with the effective count is a draw-time
obligation. Explicit depth-transition metadata still matches the actual image.

## Lowering and evidence boundaries

Ten features map to independently selected `VK_EXT_extended_dynamic_state3`
booleans and commands. Dynamic logic-operation selection maps to the separate
`extendedDynamicState2LogicOp` capability. The existing EDS3 feature structure is
extended, not duplicated in the device chain. Command pointers are loaded only for
selected features. Recording performs local value/range/capability checks and one
native setter for nonempty input. Native Rust and C share those backend methods.

The local fixture compares each control independently against static pipelines at
one and four samples, with two attachments, per-slot updates, state changes within
a list, changed-data replay and guard bytes. A single dynamic-count executable is
used with both sample counts and a static partial mask. Separate sample-position
profiles compose dynamic counts with static/dynamic positions and static/dynamic
masks, resolved color/depth and LOAD preservation. Host and native device tests
check isolated enabling, base-feature rejection, empty arrays and invalid values.
See the [execution receipt](results/m8-extended-raster-2026-11-07.md).

This is functional and direct-lowering evidence, not a universal performance
claim. Execution at 8–64 samples, this batch's secondary/generated combinations,
dual-source dynamic equations and additional format/sample-shading combinations
remain unaccepted. Metal/DX12 lowering and hardware validation are still external
backend work. Multiple viewports, extended depth controls and the independent
color-write-enable audit remain in the [M8 sequence](m8-coverage.md).

Native references: [multisample state and static mask sizing](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineMultisampleStateCreateInfo.html),
[dynamic sample count](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetRasterizationSamplesEXT.html),
[dynamic sample mask](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetSampleMaskEXT.html),
[per-attachment blend enable](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetColorBlendEnableEXT.html),
and the pinned Vulkan registry's draw-time mask/position compatibility rules.
