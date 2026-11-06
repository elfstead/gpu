# Native tessellation and geometry stages

M8 adds tessellation control/evaluation and geometry to foundation revision 11.
The existing executable, argument layout, draw, query and dependency mechanisms
carry them. Existing public layouts and operation signatures are unchanged.
There is no patch-expansion runtime, geometry conversion, shader rewriting or
additional draw path.

## Preparation contract

Enable TESSELLATION and/or GEOMETRY independently. Traditional graphics shader
order is vertex, optional paired tessellation control/evaluation, optional
geometry, optional fragment. A tessellation pair needs PATCHES topology and a
TessellationState with input patch size and upper/lower-left domain origin.
The shader still declares output control-point count, domain, spacing, winding,
geometry input/output primitive types, invocation count and output bounds.
These are compiler/caller proofs against RASTER_STAGE_LIMITS and the linked stage
interfaces; OGPU does not reflect the artifact at runtime. Input patch size is
not required to equal the control shader's output patch size.

TessellationState goes on GraphicsState.header.next. It may be chained with one
RenderMapping in either order. Unknown extensions, duplicates and cycles reject;
every record retains exact version/size/flags validation. RenderInheritance still
accepts only RenderMapping with a null next pointer: tessellation state belongs to
the executable, not to the attachment inheritance contract.

The existing topology values now also cover triangle fan, line/triangle
adjacency lists and strips, and patches. Adjacency requires GEOMETRY enabling;
shader input compatibility is caller proof. Restart is supported for strips and
fan, not lists or patches in this profile. Fixed rasterizer discard still permits
pre-raster work and requires omission of fragment. Ordinary depth-only graphics
without fragment remains available. Task/mesh pipelines are a separate native
stage family and cannot mix with vertex/tessellation/geometry stages.

TESSELLATION_MULTIVIEW and GEOMETRY_MULTIVIEW are separate enables requiring the
matching base feature and MULTIVIEW. A nonzero pipeline view mask with these stages
requires the corresponding enable. TESSELLATION_GEOMETRY_POINT_SIZE separately
permits explicit point-size writes of one. The native largePoints feature is not
enabled by this profile; values other than one are not accepted caller input.
Shader output mode and interface validity remain compiler/caller obligations. Storage writes/atomics in these stages
use the existing VERTEX_STORES_ATOMICS native feature, which covers the pre-raster
pipeline. Numeric, atomic-type, output-layer and shading-rate requirements remain
independent; none is inferred or silently enabled.

## Existing mechanisms extend to the new stages

Argument fields can name TESS_CONTROL, TESS_EVALUATION and GEOMETRY. All stages
share the same caller-owned argument bytes and descriptor heaps. Per-stage native
binding mappings are prepared once with the layout. Recording does not allocate
or retain shader resources.

The three stage bits also map directly to native synchronization stages, including
barriers, timeline scopes and timestamps. Shader access validation and per-domain
dependency discovery include them. PRE_RASTER and GRAPHICS stay explicit aggregate
choices, not extra scopes automatically added to every dependency. Local-read
barriers inside rendering remain limited to framebuffer stages; these new stages
do not become legal framebuffer-local barrier scopes.

Subgroup stage reporting preserves the new native stage bits. Existing physical
limits and per-shader constraints still apply. Pipeline statistics already expose
control patches, evaluation invocations and geometry invocations/primitives.
Generated-command stage masks and layouts can now describe these traditional
stage combinations, subject to the queried native masks and independent enables;
no separate generated draw command is introduced. Native generated execution with
these stages remains unverified by this tranche.

## Compiler and local evidence

The [Slang source](../examples/shaders/foundation-raster-stages.slang) and seven
reproducible SPIR-V artifacts cover ordinary/adjacency vertex and geometry entries,
hull/control, domain/evaluation and fragment. Every stage reads caller-owned data
through an address in the existing argument bytes. The public C consumer supplies
stage records and requirements explicitly. The bounded `ogpu-graphics` generator
still links vertex/fragment pairs; it does not claim arbitrary stage-graph
reflection or generated tessellation/geometry interfaces.

The [consumer](../examples/foundation_raster_stages.h) executes geometry, triangle
adjacency, tessellation with both origins, and tessellation plus geometry with
both origins. Direct, indexed and indirect draws share the existing command path.
Changed-data replay adjusts tessellation level from two to four: evaluation
invocations increase from 7 to 21 and combined geometry primitives from 6 to 24.
All 768 checked RGBA8 pixels, input data and guards pass over 12 submissions.
Seven malformed preparations reject before native pipeline creation. Independent
feature enabling and argument visibility rejection also pass. See the
[acceptance receipt](results/m8-raster-stages-2026-11-06.md).

This is correctness and direct-mapping evidence, not measured performance parity.
Vulkan owns tessellation and geometry execution efficiency. A future backend must
provide a truthful native profile or expose a better API alternative; it must not
silently emulate these stages with CPU work or multiple draws.

## Remaining verification and scope

Positive drawing with multiview, point output/explicit point size, line adjacency,
fan/restart, quad/isoline tessellation, alternative spacing and unequal input/output
patch sizes remains unverified. So do secondary, generated, local-read, shading-rate
combinations, per-stage storage writes and advanced subgroup
execution. Discovery/enabling and representation are not acceptance of all those
combinations. Large points, transform feedback/geometry streams and dynamic patch-size policy
are not implemented by this change; include them in the remaining M8 coverage audit. Metal/DX12 mappings remain unaccepted.

The [advanced textured consumer](../tools/sdk/foundation-advanced/README.md) now
composes these stages with generated indexed geometry, texture heaps and mip
sampling, depth, blending, scissors and render continuation. The five
profiles pass at 257×193 and 640×360, including exact tessellation versus tessellation/geometry
and uninterrupted versus suspended image agreement. The existing CPU oracle
handles nearest-filter ties explicitly; no runtime lowering change was needed.
[Clean installed acceptance](results/m8-advanced-textured-2026-11-06.md) records
180 frames and the explicit boundary comparison results. The [M8 audit](m8-coverage.md)
schedules remaining native profiles and composition/tooling work before M9;
these bounded fixtures do not imply blanket M8 completion.

Native validity is checked against the pinned Vulkan registry. Shader syntax is
based on Slang's [stage conversion documentation](https://docs.shader-slang.org/en/stable/coming-from-glsl.html);
[hull patch constants are merged for SPIR-V](https://docs.shader-slang.org/en/latest/external/slang/docs/user-guide/a2-01-spirv-target-specific.html).
