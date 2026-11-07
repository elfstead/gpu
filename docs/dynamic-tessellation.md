# Dynamic tessellation state

M8, 2026-11-07. Independent feature IDs 119 and 120 enable dynamic input patch
control-point count and tessellation-domain origin. Executable dynamic bits 48/49
select these fields; `set_patch_control_points` and `set_tessellation_origin` are
available through C and direct native Rust. The existing `TessellationState` record
still supplies static values and is required for paired tessellation stages.

Dynamic fields ignore their static values, including zero input count or an invalid
origin enum. Record headers remain validated. A recorded count must be nonzero and
within `RasterStageLimits.max_patch_size`; origin is upper-left or lower-left.
Shader output patch size, tessellation domain/spacing, stage linkage and consumed
input array elements remain shader/compiler and caller obligations. Input patch
size need not equal shader output patch size.

The setters work inside or outside a render scope on a graphics-capable recording.
Each directly emits its native command, without allocating, retaining a pipeline,
creating variants or tracking current state. The caller establishes state before
every consuming draw and observes native static/dynamic invalidation and secondary
command rules. Selecting either dynamic feature does not implicitly select the
other or enable tessellation shaders.

Native mappings are [patch control points](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetPatchControlPointsEXT.html)
from extended dynamic state 2 and [domain origin](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetTessellationDomainOriginEXT.html)
from extended dynamic state 3. Only requested optional features and commands are
enabled/loaded. Existing physical limits and the dynamic-state availability query
cover discovery; no new description or query record is needed.

## Local evidence

The C raster-stage fixture now runs 22 profiles / 44 submissions / 2,816 pixels.
The sixteen added profiles compare static, dynamic-count-only, dynamic-origin-only
and both-dynamic pipelines across three/four input control points and both origins.
Back-face culling makes the origin observable: static reference pairs alternate
between full coverage and clear pixels, and dynamic output matches byte-for-byte.
The shader emits three output control points in every case. Changed tessellation
levels, stage statistics, direct/indexed/indirect draws, changed-data replay and
byte guards remain covered. Seven existing malformed preparations reject.

119 host tests, two C/native Rust correspondence tests and all 31 native library
tests pass, including devices enabling each dynamic feature independently. Full C
and native library runs report no Vulkan validation errors and four pre-existing
unused-output warnings each. Strict Clippy and the surface check pass: 192
operations, 160 records, 301 selected constants and 1,536 representation values.
No shader artifact or existing public layout changed. The corrected operation
inventory is tested; the initial correspondence failure is retained in the receipt.

This proves bounded native behavior, not general performance parity. Quad/isoline,
other spacing modes, secondary/generated and multiview compositions remain native
acceptance work, as do Metal/DX12. Geometry capture and M8 contract/composition
consolidation are next. The [receipt](results/m8-dynamic-tessellation-2026-11-07.json)
records the local checks; a refreshed clean SDK acceptance follows consolidation.
