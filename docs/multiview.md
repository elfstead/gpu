# Native multiview — foundation revision 10

Multiview is one graphics execution over an explicitly selected set of image-array
views. It uses the existing executable, image/view, command and argument model;
there is no camera object, view-data layout, hidden draw loop or separate rendering
API. The application supplies its own view-indexed data.

## Contract

Request `MULTIVIEW` explicitly. `graphics_state.view_mask` is prepared with the
pipeline. The same mask belongs in `render_desc.view_mask` and, when recording a
render-continuation secondary, `render_inheritance.view_mask`. Zero retains ordinary
layered rendering. A nonzero mask enables multiview; `render_desc.layers` is then
ignored. Set bits select indices relative to each attachment view's `first_layer`.

`graphics_limits.max_multiview_views` bounds the highest selected index plus one,
not the number of set bits. The same span must fit every attachment and resolve
view. Thus `0b1010` needs four layers and activates indices 1 and 3. The native
shader builtin is `ViewIndex` (`SV_ViewID` in Slang), distinct from shader-written
`Layer`. Existing caller-selected view dimension/format/usage rules still apply.
The generator now checks vertex/fragment ViewIndex and emits the feature ID.

`max_multiview_instance_index` bounds generated instance indices. Direct and indexed
draws validate the range without 32-bit overflow; GPU-written indirect and generated
records retain their caller-side bounds obligation. An ordinary executable and a
multiview render scope cannot be silently combined. Generated executable-set
updates likewise require matching masks.

Mesh multiview has a separate `MESH_MULTIVIEW` enable, requires `MULTIVIEW` and
`MESH_SHADER` (which itself requires `RASTER`), and uses
`mesh_limits.max_mesh_multiview_views`. No feature dependency is enabled implicitly.
Geometry/tessellation multiview enables belong with those later stage profiles.

Queries and timestamps inside multiview consume consecutive slots equal to the
mask's population count. Occlusion/statistics totals are obtained by summing those
slots; the distribution is native-defined. Queries begin and end inside the same
multiview scope. Slot ownership, reset, availability, dependencies and reuse remain
explicit. These are [native Vulkan query rules](https://docs.vulkan.org/spec/latest/chapters/queries.html),
not an OGPU query aggregation service. See also the
[rendering mask rules](https://docs.vulkan.org/refpages/latest/refpages/source/VkRenderingInfo.html)
and [physical limits](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceMultiviewProperties.html).

## Representation and cost

Revision 10 appends the mask to `graphics_state` and the two physical limits to
`graphics_limits`: 184 and 120 bytes respectively on the supported 64-bit ABI.
The existing render/inheritance masks become usable. Header, runtime, generated
interfaces and native Rust consumers must be rebuilt from the same revision.
Counted feature selection is unchanged; no operation is added.

Vulkan lowering writes the native feature/property structures and pipeline,
dynamic-rendering and inheritance masks. Draw/secondary compatibility checks are
constant work per existing operation. Instance range validation is scalar arithmetic;
query span validation uses a population count. There is no command-time feature-list
walk, allocation, descriptor rewrite, CPU view loop, query aggregation, barrier or
GPU work inserted. Dynamic-rendering begin preserves previously set native bindings.

This retains the native scheduling choice and instruction count. It is a structural
cost argument, not a new throughput or timing claim. Cross-backend view-mask,
instancing and query semantics remain mapping/acceptance work; unsupported native
combinations must reject rather than become a hidden multipass implementation.

## Evidence and next work

The [local receipt](results/m8-multiview-2026-11-06.md) identifies the tested profile
and the installed validation layer's timestamp-tracking defect. The D32 fixture
checks sparse masks, an offset array view, clears, all inactive/outside layers,
scissored direct/indexed/indirect draws, primary/secondary recording, changed-root
replay, query totals and malformed combinations.

Color, multisample/resolve, generated-command multiview, physical mesh-multiview and
additional-device execution remain explicit verification work. The native paths
share the mask/attachment validation; that does not establish their execution.
Shading rate is the next M8 implementation batch, followed by tile-local/input-
attachment and pass-continuation profiles and remaining tessellation/geometry work.
M9 owns rays. Metal remains deferred until a Mac is available.
