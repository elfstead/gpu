# Complete dynamic vertex input

M8, 2026-11-07. Foundation revision 12 adds complete dynamic fetch descriptions
alongside the existing static description and dynamic stride. Existing public
record layouts are unchanged. Use matching headers, library and native Rust
source identity; the two new operations need a runtime containing this addition.

`DYNAMIC_VERTEX_INPUT` is optional feature ID 98. Query and explicitly enable it;
`GraphicsLimits.dynamic_states` advertises `DYNAMIC_VERTEX_INPUT` (Rust:
`DYNAMIC_VERTEX_INPUT_STATE`, bit 24). Consuming executables select that bit. Their
entire static `GraphicsState.vertex_input` is then ignored, without reading its
pointer or nested arrays. This avoids one executable per fetch layout where the
native backend supports dynamic descriptions.

The setter uses the existing `VertexInput`: unique binding IDs, strides,
vertex/instance rates and unique attribute locations with format/offset. Its
optional `VertexDivisors` extension supplies instance divisors, with one for
omitted entries. There is no second fetch schema, retained layout object or
vertex-pulling rewrite. Vertex pulling remains ordinary address-based shader code.

```c
ogpu_next_host_requirements required;
ogpu_next_vertex_input_scratch_requirements(binding_count, attribute_count, &required);
/* Allocate aligned host scratch once, outside recording. */
ogpu_next_set_vertex_input(encoder, &input, scratch);
```

Native Rust exposes `vertex_input_scratch_requirements` and `set_vertex_input`
below the C boundary. The query only reads counts and needs no device. The setter
borrows the complete input and disjoint writable scratch through the call. Empty
input accepts empty scratch and still records the native state-setting command.
The operation replaces all descriptions; omitted attribute locations become
undefined, not preserved from a prior call.

## Native rules and caller obligations

- Binding/attribute counts and IDs, stride, attribute offset, format support,
  uniqueness and binding references obey the existing graphics limits. Stride
  zero, sparse IDs and overlapping fetch are legal. Format support comes from the
  immutable physical snapshot, not a driver query during recording.
- Non-unit divisors need `VERTEX_DIVISOR`. Zero additionally needs
  `VERTEX_ZERO_DIVISOR`: the Vulkan dynamic description requires **both** for zero.
  Static divisor state retains its independent zero-divisor behavior. Non-instance
  bindings use divisor one. Divisor limits and `nonzero_first_instance` apply to
  direct, indirect and generated draws; callers prove draw-time compatibility.
- `set_vertex_input` supplies stride. If the executable also selects
  `DYNAMIC_VERTEX_STRIDE`, a later `bind_vertices` with `VERTEX_BIND_STRIDE` replaces
  the specified binding's stride. A subsequent complete input setter replaces it
  again. Vertex addresses remain independently bound with the existing operation.
- The application establishes all shader-consumed attributes and obeys native
  dynamic-state invalidation across pipeline binds and secondary command lists.
  The implementation does not maintain a current-layout cache or scan every draw.
  Shader type/location agreement, address alignment, fetch bounds and asynchronous
  memory lifetime remain caller obligations.
- Generated vertex-buffer tokens still require `DYNAMIC_VERTEX_STRIDE`. With
  static input, cold pairing checks that targets exist in the executable. With
  complete dynamic input, the caller establishes target descriptions in the native
  state used by preprocessing/execution. The requirements query cannot infer that
  future state. Generated commands do not synthesize missing descriptions.

## Lowering, cost and evidence

Vulkan uses `VK_EXT_vertex_input_dynamic_state`, its independent feature enable,
`VK_DYNAMIC_STATE_VERTEX_INPUT_EXT` and one `vkCmdSetVertexInputEXT`. The wrapper
validates local metadata and packs native binding/attribute arrays into caller
scratch. It performs no allocation, format capability query, pipeline creation,
resource retention, vertex conversion or shader rewrite while recording.
Preparation of static input shares the format snapshot and structural checks.
This establishes the native mechanism and its explicit CPU translation cost; it
is not a claim of measured performance parity or zero wrapper cost.

The [fixture](../examples/foundation_raster_controls.h) adds six profiles: non-unit
and zero divisors, sparse bindings and attribute offsets, dynamic stride overriding
the complete-input stride, vertex-rate zero stride, a divisor change between draws
on one executable, and RGBA8 normalized fetch alongside RGBA32 float. Each replays
with changed data and checks every output pixel and input/output guards. Seven
malformed recordings fail before submission; ignored static input is deliberately
malformed to catch accidental reads. Host checks include independent feature
requirements, counts, duplicate locations, rate/divisor compatibility, unsupported
formats, scratch capacity and exact packed values.

The [generated scene](../examples/foundation_generated_graphics.h) adds complete
dynamic input to GPU-produced vertex/index tokens, direct/count draw actions and
explicit/implicit preprocessing. It checks 48 submissions and 98,304 pixels on the
local Radeon. Native executable selection is unavailable locally and remains
unverified. Secondary, multiview and tessellation/geometry composition, other
hardware and Metal/DX12 implementations also remain separate evidence work.

See the [acceptance receipt](results/m8-dynamic-vertex-input-2026-11-07.md).
Native rules were checked against the pinned registry and the primary
[command contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetVertexInputEXT.html),
[binding description](https://docs.vulkan.org/refpages/latest/refpages/source/VkVertexInputBindingDescription2EXT.html)
and [attribute description](https://docs.vulkan.org/refpages/latest/refpages/source/VkVertexInputAttributeDescription2EXT.html).
