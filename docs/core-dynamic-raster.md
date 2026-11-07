# Core dynamic raster controls

M8, 2026-11-07. Foundation revision 12 adds eight C/native Rust setters and one
limits query. Existing public record layouts are unchanged. Use the matching
header/runtime or native Rust source identity for the added operations.

| Dynamic bit | Operation | Static counterpart |
|---|---|---|
| `DYNAMIC_TOPOLOGY` | `set_topology` | `GraphicsState.topology`, subject to the class rule below |
| `DYNAMIC_PRIMITIVE_RESTART` | `set_primitive_restart` | `RasterOptions.primitive_restart` |
| `DYNAMIC_RASTERIZER_DISCARD` | `set_rasterizer_discard` | `RasterOptions.discard` |
| `DYNAMIC_LINE_WIDTH` | `set_line_width` | `RasterOptions.line_width` |
| `DYNAMIC_DEPTH_BIAS` | `set_depth_bias` | Constant, clamp and slope factors |
| `DYNAMIC_DEPTH_BIAS_ENABLE` | `set_depth_bias_enable` | `RasterOptions.depth_bias` |
| `DYNAMIC_DEPTH_BOUNDS` | `set_depth_bounds` | Minimum and maximum bounds |
| `DYNAMIC_DEPTH_BOUNDS_ENABLE` | `set_depth_bounds_enable` | `RasterOptions.depth_bounds` |

These bits occupy 25–32. Rust uses the same names with `_STATE` suffix. The current
Vulkan baseline supplies the commands; `GraphicsLimits.dynamic_states` advertises
them without new optional feature IDs. Existing value-dependent feature enables
still apply. Selecting a dynamic bit ignores only its static counterpart during
preparation, so enable and value controls remain independent. Record headers,
extension chains and unrelated static state still undergo validation.

## Topology and draw-time obligations

Query `DYNAMIC_RASTER_LIMITS` (27) for `DynamicRasterLimits.unrestricted_topology`.
Zero means the dynamic topology must remain in the static pipeline's topology
class: point, line, triangle or patch. The static topology remains a required,
valid class selector in that case. One means the static topology is ignored;
preparation supplies a legal native placeholder rather than constraining future
topology changes. The local Radeon reports zero.

The setter uses existing topology IDs. Adjacency and patch topologies require the
existing geometry and tessellation capabilities. The caller proves that each draw
matches its actual shader stages, geometry input, tessellation state and topology
class. Enabling restart does not infer topology or automatically enable list/patch
restart features. Those capabilities are required when the selected draw topology
uses them. Strip/fan restart stays part of the baseline.

Applications establish all required dynamic state before use and obey native
invalidation across executable binds and secondary lists. These requirements also
apply to indirect/generated draws. OGPU adds no current-topology cache, per-draw
compatibility scan, automatic pipeline replacement or primitive conversion.

## Values and effects

Booleans are zero/one. Depth-bias factors are finite, may be signed, and nonzero
clamp requires `DEPTH_BIAS_CLAMP`. Native depth representation determines constant
bias units; slope bias and clamp preserve the native calculation. Bounds are
ordered, finite and within [0,1] in this profile. Enabling the bounds test requires
`DEPTH_BOUNDS`; setting bounds or disabling the test does not. Bounds test existing
attachment depth, not the incoming fragment's depth.

Line width is finite and positive; values other than one require `WIDE_LINES`.
Values pass to the driver unchanged. The rasterizer chooses supported widths and
applies its rounding rules. The queried range is not used to reject widths: this
step also removes that older restriction from static preparation. There is no
wrapper-side normalization. [Vulkan's rasterization rules](https://docs.vulkan.org/spec/latest/chapters/primsrast.html)
define the supported-width selection.

Dynamic discard can toggle on an executable that includes fragment and attachment
state. It suppresses rasterization, while attachment loads/clears/stores still
occur and earlier shader stages may still have effects. The static-discard profile
retains its existing no-fragment-output preparation contract.

## Cost and verification

Each setter does local value/capability checks and emits one corresponding native
command. It allocates no memory, compiles no pipeline, queries no driver property
and synthesizes no GPU work. Topology properties are queried during discovery.
This is an implementation mapping, not measured performance parity.

The [public C fixture](../examples/foundation_core_raster.h) compares static and
dynamic color/depth bytes, changes state between two draws on one dynamic
executable, replays changed input and checks numerical depth plus guards. Local
execution covers ten profiles: same-class topology changes; indexed restart;
discard; narrow/wide lines; bias enable, values and signed clamp; bounds values
and enable; and widths outside the queried range. Cross-class topology is
explicitly skipped on this GPU. Eight malformed recordings fail before submission.
Static fields selected as dynamic are deliberately poisoned to verify that they
are ignored. The discard reference omits the discarded draw: it establishes
attachment output, not vertex-stage side-effect behavior.

See the [acceptance receipt](results/m8-core-dynamic-raster-2026-11-07.md). Cross-class
execution, other devices, advanced stage/secondary/generated combinations and
Metal/DX12 remain unverified. The [M8 audit](m8-coverage.md) names the remaining
extended dynamic and viewport/depth mechanisms; this batch does not complete M8.

Native contracts were checked against the pinned registry and primary references:
[topology](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetPrimitiveTopology.html),
[topology-class property](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceExtendedDynamicState3PropertiesEXT.html),
[restart](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetPrimitiveRestartEnable.html),
[depth bias](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetDepthBias.html),
[bounds](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetDepthBounds.html)
and [bounds enable](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetDepthBoundsTestEnable.html).
