# M11 ray libraries and handle capture/replay

Implemented on Vulkan/Linux, 2026-11-07. Normal local RADV reports these ray
profiles unavailable. Explicit `RADV_EXPERIMENTAL=emulate_rt` exercises linkage
and replay with clean validation; this is diagnostic evidence, not native
ray-hardware acceptance.

## Contract

Feature IDs 195–198 independently expose ray libraries, stable library group
handles, opaque group-handle capture/replay, and mixed replay/new-handle creation.
Libraries and capture require the ray-pipeline profile; stable library handles
require libraries, and mixed creation requires capture/replay. Query 38 reports
the opaque capture-handle size, separately from ordinary shader-table handle size.

`COMPILE_LIBRARY` on the existing executable descriptor creates an unbindable
library. The current supported executable kind is ray tracing. `RayLibrariesInfo`
extends `RayState.header.next` and supplies a borrowed input-library array and
explicit payload/hit-attribute interface sizes. It is required even for an initial
library with no imports. Libraries share the executable destruction operation;
they cannot be bound or inserted into generated execution sets.

Local stages precede imported stages, in library order. Local groups may reference
that complete stage list; imported groups follow local groups. Nested linkage and
zero local stages/groups are supported when imports provide the contents. Inputs
must belong to the device, match recursion/interface limits, and honor the parent
ray flags. Compatible argument mappings need not use the same layout object.

**The caller retains input libraries and their argument layouts until all derived
libraries/executables and their uses are destroyed/retired.** This preserves native
library lifetime requirements. There is no reference counting, automatic retention,
dependency destruction or hidden wait. An early draft consumer destroyed libraries
after linking; validation exposed that invalid lifetime assumption. That run is
rejected, and the corrected consumer destroys dependents first.

`RAY_CAPTURE_HANDLES` enables opaque handle capture and replay on a ray pipeline.
`ray_capture_handles` returns those bytes; ordinary `ray_group_handles` still
returns live shader-table handles. Querying either on libraries requires the
stable-library-handles profile. `RayReplayInfo`, also on `RayState.header.next`,
supplies one optional captured byte string per **local** group. The two extension
records can appear in either order and compose with executable binary inputs.

Opaque capture bytes retain native provenance and compatibility requirements. When
mixed creation is unavailable, the caller obeys native device-wide sequencing and
exclusion rules. OGPU adds no global lock or history. This package does not capture
acceleration structures, resource addresses, scenes or complete shader tables.

## Cost and evidence

Cold creation translates native linkage/interface/replay records and stores stage
and group metadata for later bounds checks and nested links. Handle queries map to
one native call after bounds checks. Existing dispatch/table submission is unchanged;
there is no extra GPU work, shader rewrite, scheduling or persistent-cache policy.

The public Rust `ray_artifacts` consumer checks:

- A source library, a nested library, and a final executable without local shaders.
- A local group referencing the second imported shader; two imports of one library
  preserve native group ordering.
- Stable ordinary handles across library links, plus capture/replay on a fresh
  device whose first native ray-pipeline creation uses captured bytes. Mixed
  creation is deliberately not enabled.
- Three ray dispatches in each phase, exact integer outputs, unchanged allocation
  guards/table bytes, and explicit dependent-before-source destruction.
- Rejection of library binding, malformed replay bytes and undersized capture output.

Local emulation enables profiles 195–197 and reports mixed creation unavailable.
Capture handles are 56 bytes. Both fresh compilation and replay pass with zero
Vulkan/synchronization validation errors. The existing four ray consumer variants
remain regression checks. The consumer never selects emulation itself.

SDK bundles include this source consumer; `test-rust-install.py --ray-artifacts`
requires successful library execution, records the externally selected ray
environment, and preserves optional replay exclusion in its raw log. A pass with
emulation is still diagnostic, and a library-only pass is not replay acceptance.

Remaining acceptance: native ray hardware; mixed replay/new-handle creation;
nontrivial hit/miss library linkage; binary-backed ray/library execution; other
native backends. Existing M9 traversal and combined-usage gaps remain independent.
[Machine-readable receipt](results/m11-ray-artifacts-2026-11-07.json).

Native references: [ray pipeline creation](https://docs.vulkan.org/refpages/latest/refpages/source/VkRayTracingPipelineCreateInfoKHR.html),
[pipeline library lifetime](https://docs.vulkan.org/spec/latest/chapters/pipelines.html),
[stable library handles](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_pipeline_library_group_handles.html),
[capture handles](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetRayTracingCaptureReplayShaderGroupHandlesKHR.html).
The implementation uses the pinned registry's EXT group-handle profile; it does
not depend on a newer alias absent from that registry.
