# Framebuffer-local reads

M8 adds explicit local reads to foundation revision 11 without changing existing
record layouts. Enable `FEATURE_LOCAL_READ`; query `QUERY_LOCAL_READ_LIMITS` for
physical input-index, depth/stencil and multisample support. The Vulkan backend
uses Vulkan 1.4 dynamic rendering local read, already part of the baseline API
version. It adds no legacy render-pass implementation or automatic subpass graph.

## One render instance, explicit dependencies

A producer draw can write attachments or address-backed storage, followed by a
framebuffer-region barrier and a consumer draw in the same render instance.
`ACCESS_INPUT_ATTACHMENT_READ` distinguishes input attachments from sampled
images. Existing global, memory-range and image barriers remain the mechanisms;
`DEPENDENCY_BY_REGION` supplies the framebuffer locality. `DEPENDENCY_VIEW_LOCAL`
is required for a scope with multiple views. Storage addresses are not magically
pixel-local: the application must make the dependency's region guarantee true.
Reads across arbitrary pixels require an appropriate non-local dependency, outside
the render instance in this Vulkan profile.

In-render barriers require LOCAL_READ and inline command contents. Their stages
are fragment shader, early/late depth/stencil tests and color output. A nonzero
source stage requires BY_REGION. Layout changes, discards and ownership transfers
are rejected inside rendering. An image barrier referring to a currently bound
attachment must keep it in GENERAL or LOCAL_READ; the application proves this
relationship. Non-attachment resources are also permitted. There is no retained
attachment registry, inferred hazard tracking or implicit transition.

LOCAL_READ is an explicit native image state. It requires STORAGE usage, or
INPUT_ATTACHMENT together with COLOR_ATTACHMENT or DEPTH_STENCIL_ATTACHMENT. An
attachment-only output uses GENERAL or its ordinary attachment state instead.
The shared layout guard checks this condition for barriers, rendering and descriptors.
GENERAL remains valid. Image/view
usage, shader input types, descriptor state and attachment compatibility must all
match. Depth/stencil and multisample reads depend on their queried support, not
merely on the base enable. Combined depth/stencil transitions retain the existing
combined-aspect rule. Split events retain their outside-rendering, zero-flags
contract; they do not become a second local-dependency mechanism.

## Attachment mappings and shader resources

`RenderLocations` maps color attachment slots to shader output locations.
`RenderInputs` maps attachment slots to input attachment indices. Null color
arrays use identity; ATTACHMENT_UNUSED disables a slot. Color indices are unique
and cannot alias depth/stencil input indices. Depth and stencil may share an
index. A null depth/stencil pointer denotes an input with no InputAttachmentIndex
decoration; a pointer to ATTACHMENT_UNUSED disables that aspect.

Put a `RenderMapping` on `GraphicsState.header.next` for executable preparation,
or on `RenderInheritance.header.next` for secondary recording inheritance.
On graphics state it may be chained with one TessellationState, in either order;
unknown/duplicate/cyclic chains reject. On render inheritance it remains the only
extension and its own next pointer is null.
The two recording setters update output locations and input indices independently.
Counts must match the current render scope. The application proves agreement
between executable, current mapping, shader, attachments and secondary inheritance
at use. No array survives preparation/recording and no per-draw mapping comparison
is added. Render begin initializes native input/output mappings, including a
resumed render: reissue the selected mappings after begin before drawing. A primary scope marked SECONDARY cannot use inline setters/barriers;
render-continuation secondary lists can record their own local commands.

There is one necessary native shader-interface detail: Vulkan descriptor heaps
still require binding mappings for input attachments. Ordinary bindless image
access cannot express this resource. `InputAttachmentBinding` identifies a
fragment shader space/binding and a four-aligned offset in the existing argument
bytes. That uint32 contains the caller's resource-heap location; array_stride is
an explicit native-location-unit step between elements, including zero. Vulkan
locations are bytes, and array strides must satisfy image descriptor alignment.
The application proves actual heap bounds, descriptor alignment, input-attachment
kind, view/layout and array accesses. No descriptor is allocated, copied, selected
or retained by this mapping.

Only executable preparation accepts bindings; secondary inheritance does not.
Preparation combines these mappings with the existing argument layout. Native
image resource classification depends on SPIR-V NonWritable, which the pinned
Slang compiler omits for SubpassInput; the mapping accepts either classification.
This does not authorize a writable input attachment. The byte offset and array
stride are a native Vulkan shader ABI, not a promise of an identical Metal/DX12
input-attachment mechanism. Future native backends must accept a truthful profile
or expose the efficient hardware alternative rather than emulate a hidden pass.

## Compiler and consumer

[The Slang source](../examples/shaders/foundation-local-read.slang) supplies a
fullscreen vertex stage, a producer and a consumer fragment stage. The caller
supplies the input attachment's declared binding, root offset and shader feature
requirements explicitly. `cargo xtask foundation-shaders --check` reproduces and
validates the three checked-in SPIR-V artifacts with the pinned toolchain.
The bounded `ogpu-graphics` generated-interface tool does **not** yet generate
input-attachment binding metadata. Raw prepared artifacts are usable now; a
compiler-generated local-read handoff remains an explicit follow-up.

[The public C consumer](../examples/foundation_local_read.h) keeps two RGBA8 targets
bound. The first draw writes target zero and per-pixel address storage; the second
writes target one while reading both. It remaps output location zero, uses a
nonzero descriptor location/root value, checks both layouts and both global and
resource-specific barriers, and replays each list with fresh host storage.
Four malformed executable mappings and sixteen malformed recordings test rejection
before native work. See the [acceptance receipt](results/m8-local-read-2026-11-06.md).

## Verification still needed

The available Radeon establishes ordinary color and same-pixel storage
correctness. It does not establish tile retention or performance parity: Vulkan
chooses the physical implementation. Structurally, commands map to native
mapping setters and one barrier, with borrowed caller arrays/scratch and no extra
submit, render break, staging or hot-path allocation.

The [advanced textured consumer](../tools/sdk/foundation-advanced/README.md) now
composes color/D32 local reads with texture heaps, mip/filtering, generated indexed
draws, tessellation, geometry, blending, depth and suspend/resume. It checks all
local-output pixels against independently read-back source color/depth. A separate
four-sample probe supplies distinct per-sample color/depth values and validates a
weighted reduction through multisample input attachments followed by resolve.
Its fixed artifact recipe reflects root offsets; it does not expand the general
`ogpu-graphics` linker. Pinned Slang emits invalid Vulkan SPIR-V for scalar
`SubpassInput<float>` in this probe; `SubpassInput<float4>` with `.x` is validated.

Remaining combinations include stencil reads, input arrays/alternate compiler
decorations, secondary execution, multiview, shading rate and mesh/generated-command
local reads (GPU-produced ordinary indexed arguments are covered).
LOCAL_READ resolve destinations are representable but not newly executed by this
fixture. Native Metal/DX12 mappings and compiler-generated input binding metadata
remain unaccepted. These do not block the next planned M8 tessellation/geometry
implementation batch.

Native rules are checked against the repository's pinned Vulkan registry. Design
references: [dynamic rendering local read](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_dynamic_rendering_local_read.html)
and [descriptor heaps, including input attachment restrictions](https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_descriptor_heap.html).
