# Indexed/depth graphics: candidate contract

Proposed 2026-09-29 against the [accepted native scene](graphics-scene-native-results.md)
at `5da5aaf`. **Design proposal; implementation status is tracked separately** in the
[ABI-18 migration checkpoint](indexed-depth-migration.md). This is the second slice of
[M4](graphics-consumer-plan.md), not completion of that milestone or a stable ABI.

## Decision and alternatives

Separate rendering attachment scope from draws. A scope owns the attachment
load/store decisions; an immutable raster owns executable and depth-test state;
each draw supplies its root, indirect record and, when indexed, an index range.
Retain address-based vertex pulling, real fixed-function indexing/depth testing,
explicit dependencies and caller-controlled allocation/submission reuse.

| Choice | Candidate | Alternative and reason |
|---|---|---|
| Draw grouping | Explicit begin/end rendering | Per-draw attachments can permit legal native fusion; they are not inherently slower. Explicit scope makes grouping and preservation intentional without requiring a backend to rediscover scope boundaries. |
| Depth state | Immutable raster description | Dynamic setters may reduce variant preparation cost. Keep that question open; no pipeline compilation or creation is allowed to become mandatory per draw/frame. |
| Index ownership | Ordinary buffer plus checked byte range | A bare address is smaller but leaves eligibility, bounds and retention entirely to the caller. Handles do not prevent native address commands or suballocation within one buffer. |
| Index eligibility | Opt-in allocation capability | Making every buffer index-capable is simpler, but needlessly couples compute-only allocations to another usage. No cross-device evidence establishes that adding usage is universally free. |
| Attachment preservation | Independent load and store choices for each attachment | The current always-store draw hides an avoidable requirement for disposable depth/color results. |

These choices do not settle the final allocation or pipeline abstraction. In
particular, explicit scopes are a better API alternative for this consumer even
though a more sophisticated implementation of the old draw API could fuse draws.

## Proposed C surface

The sketch below motivated ABI 18; the header is authoritative for the implemented
signatures, constants and rules. Use fixed-width
integer fields/constants, not C enums or compiler-dependent bitfields. Exact
numeric assignments and layout assertions live with the implementation.

```c
typedef struct OgpuBufferDesc {
    uint64_t size_bytes;
    uint32_t placement;
    uint32_t extra_usage;
} OgpuBufferDesc;

OgpuResult ogpu_buffer_create(OgpuDevice *device, const OgpuBufferDesc *desc,
    OgpuBuffer **out_buffer, OgpuError *out_error);

typedef struct OgpuRasterDesc {
    uint32_t push_size_bytes, topology, color_format, depth_format;
    uint32_t depth_test, depth_write, depth_compare, reserved;
} OgpuRasterDesc;

OgpuResult ogpu_raster_create(OgpuDevice *device,
    const OgpuShaderDesc *vertex, const OgpuShaderDesc *fragment,
    const OgpuRasterDesc *desc, OgpuRaster **out_raster, OgpuError *out_error);

typedef struct OgpuColorAttachment {
    OgpuImage *image;
    uint32_t load, store;
    float clear[4];
} OgpuColorAttachment;
typedef struct OgpuDepthAttachment {
    OgpuImage *image;
    uint32_t load, store;
    float clear;
    uint32_t reserved;
} OgpuDepthAttachment;
typedef struct OgpuRenderingDesc {
    OgpuColorAttachment color;
    OgpuDepthAttachment depth;
} OgpuRenderingDesc;

OgpuResult ogpu_batch_begin_rendering(OgpuBatch *batch,
    const OgpuRenderingDesc *desc, OgpuError *out_error);
OgpuResult ogpu_batch_end_rendering(OgpuBatch *batch, OgpuError *out_error);

/* Replaces the attachment-bearing ABI-17 signature. */
OgpuResult ogpu_batch_draw_indirect(OgpuBatch *batch, OgpuRaster *raster,
    OgpuBuffer *indirect, uint64_t indirect_offset,
    const void *arguments, uint32_t argument_bytes, OgpuError *out_error);

typedef struct OgpuIndexRange {
    OgpuBuffer *buffer;
    uint64_t offset, size_bytes;
    uint32_t format, reserved;
} OgpuIndexRange;
typedef struct OgpuDrawIndexedArguments {
    uint32_t index_count, instance_count, first_index;
    int32_t vertex_offset;
    uint32_t first_instance;
} OgpuDrawIndexedArguments;

OgpuResult ogpu_batch_draw_indexed_indirect(OgpuBatch *batch, OgpuRaster *raster,
    const OgpuIndexRange *indices, OgpuBuffer *indirect, uint64_t indirect_offset,
    const void *arguments, uint32_t argument_bytes, OgpuError *out_error);
```

There is no new owning render-pass, index-buffer or attachment object. Descriptions
and roots are copied on successful recording; buffers/images/rasters are retained.
Changing to descriptions removes positional allocation/raster arguments without
adding version chains or a native Vulkan structure to the boundary.

### Buffers and indexed execution

`extra_usage=0` preserves today's address/copy/indirect capabilities and placement
rules. `OGPU_BUFFER_INDEX` additionally permits fixed-function index reads; unknown
bits are invalid. This is allocation eligibility, not a binding type: compute can
write the same buffer, and index/vertex/indirect ranges can share one allocation.
Vulkan adds `INDEX_BUFFER` only when requested and uses the resulting actual memory
requirements. Missing compatible memory remains `UNSUPPORTED`, never a hidden
copy or placement fallback. Creation is valid on a Vulkan compute-only device;
executing graphics still requires the graphics profile. Metal initially rejects
the extra capability explicitly; its existing zero-extra-usage path is unchanged.

The eligibility is real even for address commands: Vulkan's
[index-address binding](https://docs.vulkan.org/refpages/latest/refpages/source/VkBindIndexBuffer3InfoKHR.html)
requires index usage on the originating buffer. This does not prove a measured
allocation penalty; the opt-in preserves the choice without assuming one. The
existing base usage and dedicated allocation policy remain open under P6; this
slice does not claim to expose every native allocation strategy.

Support `OGPU_INDEX_UINT16` and `OGPU_INDEX_UINT32`, not uint8 or primitive restart.
Require a nonempty retained range inside the eligible buffer; offset and size are
multiples of the index element size. Resolve/check native address alignment as
well. A subrange is not a separate allocation or a CPU buffer view.

The indirect record is exactly 20 bytes, four-byte aligned, with the signed field
at byte 12. `first_index` is relative to the supplied index range, not the buffer
allocation. Validate record/range bounds with overflow-safe arithmetic, device
identity, reserved fields, formats and root size before appending a step. The
caller guarantees valid GPU-produced contents, including
`(first_index + index_count) * index_size <= size_bytes` without overflow,
`first_instance == 0`, and valid shader addresses for every resulting vertex and
instance. Zero counts are legal but do not waive the supplied range/record checks.
Do not inspect GPU data on the CPU. These fields map directly to
[native indexed indirect arguments](https://docs.vulkan.org/refpages/latest/refpages/source/VkDrawIndexedIndirectCommand.html).

Apply the signed vertex offset through native indexing; do not substitute shader
index fetching in a non-indexed draw. The native control's negative/positive offsets
and poisoned prefix remain acceptance cases. UINT16 needs new evidence: the first
accepted reference tested UINT32 only. This slice still issues one indirect record
per call; multi-draw/count-buffer and nonzero-first-instance strategies remain
unresolved, not passed by an efficient series of single draws.

### Images, depth and attachment operations

Add `OGPU_FORMAT_D32_FLOAT` and `OGPU_IMAGE_USAGE_DEPTH`. The bounded combination
is 2D, one mip/layer/sample, DEPTH with optional COPY_SRC/COPY_DST; reject color,
sampled or storage usage on D32 in this slice. COLOR remains RGBA8 or RGBA16F.
Both creation and allocation-free support query check the exact combination on
the device. Depth copies use the depth aspect and tightly packed four-byte float
texels; do not accidentally reuse the old hard-coded color aspect. R32 color and
D32 depth are distinct formats even though both have four-byte float storage.

One color attachment is required initially; depth is optional (`depth.image=NULL`,
all other depth fields zero). Present attachments must share device and extent.
This subset does not approve a permanent color requirement: depth-only rendering,
multiple targets, stencil, multisampling, partial render areas and attachment
feedback/local reads need their own contracts/tests. Keep these limitations
explicit instead of emulating them with extra attachments or passes.

For each present attachment independently:

- Load `CLEAR` initializes every texel to the supplied value; `LOAD` preserves
  previously defined contents; `DISCARD` makes prior contents unspecified without
  promising zeroes. Load DISCARD requires writing before any value-dependent read.
- Store `STORE` preserves defined results after the scope; `DISCARD` permits
  their loss. No later copy/sample/LOAD may rely on discarded results. STORE is a
  preservation requirement, not a requirement to perform redundant memory traffic.
- Clear color components must be finite (normal format conversion applies);
  clear depth must be finite and in [0,1]. Validate clear values only for CLEAR.
  Reserved fields are always zero; unknown load/store values are invalid.

Map these to native attachment operations, not clear/copy dispatches between draws.
Vulkan exposes independent [attachment load/store fields](https://docs.vulkan.org/refpages/latest/refpages/source/VkRenderingAttachmentInfo.html).
An empty scope can still clear/store and must not be removed blindly. DISCARD store
can use native DONT_CARE; STORE on a provably read-only LOAD attachment may use NONE
while preserving its previous contents. Do not treat DONT_CARE as read-only: native
[store semantics](https://docs.vulkan.org/refpages/latest/refpages/source/VkAttachmentStoreOp.html)
allow it to invalidate prior results even when no draw writes the attachment.
Conservative STORE encoding is an implementation choice to measure, not a public
requirement to forgo read-only attachment optimization.

Raster depth format is D32_FLOAT or a new distinct `OGPU_FORMAT_NONE` sentinel
(RGBA8 already has value zero). Both attachment formats must match the scope,
even with testing disabled. Depth test/write fields are exactly 0 or 1. Support
the eight ordinary comparisons NEVER, LESS, EQUAL, LESS_EQUAL, GREATER, NOT_EQUAL,
GREATER_EQUAL, ALWAYS. No bounds test, bias, stencil, shader depth output or depth
clamp in this slice. Viewport depth is [0,1]. With testing disabled, no fragment
depth writes occur even if `depth_write=1`; with no depth format require both
flags zero and compare ALWAYS. Test-disabled and write-disabled are different
cases, as in the native reference. State is prepared at raster creation, not in
the render loop. Dynamic-state/variant costs remain a separate stronger-native
comparison before claiming general pipeline expressibility.

## Recording, initialization and dependencies

Rendering is batch state, not a nested batch or a submission boundary:

```text
outside scope -- begin(valid attachments) --> inside scope
inside scope  -- draw / indexed draw ------> inside scope
inside scope  -- end ----------------------> outside scope
outside scope -- submit / compile --------> consumed (existing preparation rules)
```

Draw outside a scope, nested begin, end without begin, and incompatible draws
return `INVALID_ARGUMENT` without changing the recording. Submit/compile with an
open scope rejects without consuming it, like unmatched split-dependency endpoints;
the caller can end the scope and retry. Allocation/validation failure while
recording must not append a partial begin/draw or leak retained objects.

Inside a scope, permit draws, image/sampler heap binding and explicit buffer
retention; reject dispatch, copies, discard-image, barriers and split-dependency
endpoints. Draws can select another compatible raster/root/index range without
ending the scope. Heap binding still follows existing immutable-while-retained
rules. A split dependency may span a complete scope with both endpoints outside;
existing serial/simultaneous list restrictions remain. These are bounded command
rules, not a claim that Vulkan forbids every in-rendering barrier: its
[barrier rules](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdPipelineBarrier2.html)
are more specific. Local rendering dependencies are deferred, not emulated by
mandatory scope breaks and declared performance-equivalent.

**Separate initialization from clear.** All attachments must already be initialized
to the runtime's GENERAL-layout contract, normally with `discard_image` before
first use. CLEAR only initializes values, not layout; begin/end insert no implicit
global data barrier. LOAD additionally requires defined prior contents. This is
an intentional change from ABI 17's CLEAR draw, which calls discard internally.
It allows image initialization once, followed by repeated clears and multiple
draws/scopes without forced per-draw discard/transition barriers. Do not add a
host-side initialization tracker whose answers fail on replay or abandoned batches.

Add `OGPU_ACCESS_INDEX_READ`, `OGPU_ACCESS_DEPTH_READ`, and
`OGPU_ACCESS_DEPTH_WRITE`; VERTEX_READ remains shader reads, not fixed-function
index input. Native mapping is INDEX_INPUT/INDEX_READ and EARLY|LATE_FRAGMENT_TESTS
with depth/stencil attachment read/write respectively. The caller places explicit
dependencies outside scopes for preceding/following accesses; CLEAR/DISCARD do
not excuse write-after-write/read hazards. Load/store operations count as accesses,
not just shader/raster operations. Between ordered draws in one scope, ordinary
attachment depth/color ordering is native rendering behavior, not a reason to
insert a global barrier per draw. Feedback through shaders remains prohibited.

For the selected scene: compute writes -> INDEX_READ | VERTEX_READ | INDIRECT_READ;
render; for the two-scope cases, COLOR_WRITE | DEPTH_WRITE -> COLOR_READ |
COLOR_WRITE | DEPTH_READ | DEPTH_WRITE before the second begin. Existing known
image-copy operations handle their attachment-to-copy dependency, extended to the
depth aspect/access classes. Explicit initialization may be outside a reusable
list; its completion/dependencies must precede executions using those images.

The begin step retains attachments, draws retain rasters/index/indirect buffers,
and pointer-reachable vertex allocations still need caller lifetime or explicit
`retain_buffer`. Discarding a recording releases these references; accepted work
holds them until retirement/drain; compiled lists hold them between executions.
Destroying public handles after recording is legal, not a wait. No new alias
tracking, automatic pointee retention, host waits or per-frame allocation policy.

## Implementation and acceptance sequence

1. **ABI and runtime in one coherent migration.** Bump to ABI 18 when changing the
   header, not for this proposal. Add checked native index bindings, buffer extra
   usage, D32 support/copies, raster depth state and Begin/Draw/End steps. Extend
   access validation and open-scope rejection. Migrate all current non-indexed
   callers: explicit initialization, begin with the old clear/load and STORE,
   draw, end. Update Metal exports/stubs, ABI tests, installed support/version
   metadata, compiler example emitters, GGML/libplacebo adapters and diagnostics,
   and bindings checks together. Remove the
   old attachment-bearing draw path rather than keeping a compatibility wrapper.
2. **Contract/lifetime gates.** Test all invalid state transitions and retry after
   rejection; malformed flags/formats, clear NaN/infinity/range, extent/device
   mismatch, uint16/uint32 alignment, offset/size overflow and 20-byte record
   boundaries. Verify destroy-after-record, abandoned/open scopes, failed
   preparation, list retention/replay and existing loss/drain behavior. Never
   submit invalid GPU indices merely to test CPU validation that cannot exist.
3. **Matched public scene.** Reuse the accepted shader artifacts and analytic
   oracle, not runtime code in the native control. Repeat all ten modes and A/B/A
   frames at the accepted extents on both drivers. Add UINT16 equivalents, empty
   clear scopes, all compare operations, invalid-description/support queries,
   non-black clear, depth upload/readback, and disposable depth followed by explicit
   reinitialization (never assert discarded bytes). Test read-only LOAD depth
   preservation and grouped draws. Preserve raw logs/artifact identities.
4. **Reuse and fundamental performance gate.** Add two-slot and serial-list replay
   with stable attachments, index ranges and GPU-written records; mutate data only
   after the slot's conflicting execution completes. Compare native grouped draws,
   matched OGPU grouping, and an explicitly labelled per-draw-scope control at
   1/64/512 small indexed draws with identical pixels/state, plus the larger scene.
   Use three rotated Radeon processes, 100 warmups and 1,000 measured frames per
   case; validation/correctness is separate, llvmpipe is correctness only. Report
   host recording/submission, wall throughput, storage and scope/barrier counts.
   Compare fresh recording and replay at equal slot/storage budgets. Any measured
   API-imposed loss of a native strategy reopens this proposal; parity alone does
   not close untested multi-draw, dynamic-state, depth-only or transient-memory cases.
5. **Generated/installed handoff and decision.** Extend the offline stage-pair
   checker for the scene's vertex root and flat integer varying, with rejection
   tests, before claiming generated consumer support. Exercise the scene against
   an installed SDK without source-private interfaces; run existing learned-image,
   ABI, binding, bounded integration and lifetime regressions. Record what is
   accepted, deficient or unresolved; only then move to the M4 mip/view slice.

The first native scene proves correctness for its specified native subset, not
this new contract, its replay/performance, all compare/index formats or the shader
adapter changes. No new GPU or Mac is a prerequisite for these Linux gates.
