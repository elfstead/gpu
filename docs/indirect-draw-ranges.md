# Draw-contract decision: explicit indirect ranges and GPU counts

Selected 2026-09-30 following the [native frontier](graphics-indexed-frontier-results.md)
and its [draw-identity follow-up](graphics-draw-identity-results.md). **Implementation target, not implemented API**:
the current header/runtime remain ABI 18 until the coordinated migration lands.
This closes the choice of direction in the [M4 decision gate](indexed-depth-performance-plan.md),
not its public implementation, lifetime or performance acceptance.

## Decision

Replace the one-record indirect argument pair with an explicit record-range
description for both indexed and non-indexed drawing. A range executes a fixed
number of records or reads a bounded count from a retained buffer during GPU
execution. Keep rendering scopes, immutable prepared raster state, index ranges,
copied roots and caller-managed dependencies/storage/replay.

Do not add a graph scheduler, GPU command interpreter, hidden culling pass or
CPU count readback. Fixed single-record draws remain the count-one case. Do not
keep a second one-record implementation path merely for ABI compatibility;
migrate in-tree callers and advance the ABI together.

Reasons:

- A count word is information the current public draw call cannot communicate.
  Rewriting producers to zero unused records is a different strategy, not a
  general mapping preserving producer semantics and command costs.
- Native draw identity is local to a multi-record operation. Arbitrarily fusing
  separate operations or splitting a range changes that identity unless the
  implementation proves a semantics-preserving mapping.
- This scene need not show a timing win before adopting the explicit alternative.
  Native strategy preservation is the reason; matched timing still evaluates the
  resulting implementation and does not disappear from acceptance.

The current public profile does **not** enable `shaderDrawParameters`. The identity
probe therefore establishes a native semantic choice to expose, not a fusion bug
in a currently valid ABI-18 shader. A backend could still fuse proven identity-
insensitive calls; that optimization is not a substitute for a first-class range.

## Selected C shape

Names/layouts below are the migration target; implemented header/ABI checks become
authoritative when the change lands. Fixed-width integers, no owning range object:

```c
typedef struct OgpuIndirectRange {
    OgpuBuffer *buffer;
    uint64_t offset;
    uint32_t stride_bytes;
    uint32_t max_draw_count;
    OgpuBuffer *count_buffer; /* NULL: fixed max_draw_count. */
    uint64_t count_offset;   /* Must be zero when count_buffer is NULL. */
} OgpuIndirectRange;

OgpuResult ogpu_batch_draw_indirect(OgpuBatch *batch, OgpuRaster *raster,
    const OgpuIndirectRange *draws,
    const void *arguments, uint32_t argument_bytes, OgpuError *out_error);

OgpuResult ogpu_batch_draw_indexed_indirect(OgpuBatch *batch, OgpuRaster *raster,
    const OgpuIndexRange *indices, const OgpuIndirectRange *draws,
    const void *arguments, uint32_t argument_bytes, OgpuError *out_error);
```

The raster/root/index binding is shared by all records in one call. Per-record
data can be addressed by draw identity; this does not introduce per-record root
updates. Indexed/non-indexed record layouts stay 20/16 bytes. `first_instance=0`
and other trusted record/index/pointer obligations remain unchanged.

### Bounds, empty execution and ownership

- `buffer` is required and belongs to the recording device. Offset is four-byte
  aligned; stride is a multiple of four and at least the record size, even for
  zero/one capacity. Nonzero capacity requires checked
  `(max_draw_count - 1) * stride_bytes + record_size` bytes from offset. Padding
  need not be valid draw data and is not rewritten. Capacity cannot exceed the
  queried enabled limit. Zero capacity permits offset at the end of the buffer.
- Fixed mode executes exactly `max_draw_count` records. Counted mode reads a native
  uint32 word and executes `min(word, max_draw_count)` records. Count offset is
  four-byte aligned and four bytes must fit, including when capacity is zero.
  The count and records may share a buffer. No host inspection of GPU contents.
- Zero capacity is valid and may omit the native draw entirely. It still validates
  scope, raster, root, descriptor objects, index range and optional count word;
  it retains declared backing like any other recorded range. Zero active GPU
  count does not waive capacity-sized allocation bounds. Attachment scope effects
  remain, even when no draw invokes shaders.
- Copy the descriptor and root on successful recording. Retain the record/count
  buffers, index buffer and raster through recording/list/submission lifetimes,
  even if application handles are destroyed. Preserve existing list-versus-
  completion retention rules. Root pointees still require explicit lifetime
  management; they are not discovered by scanning addresses.
- Replaying fixed addresses with updated records/counts is legal under existing
  host access, retirement and visibility rules. GPU generation can precede drawing
  in the same submission with explicit compute-write to indirect-read dependency.
  No dependency is inserted solely because a descriptor contains a count pointer.
- Rejection must leave recording unchanged. Validate arithmetic and descriptors
  before appending a step. Same physical device is not sufficient: objects must
  belong to the same logical device. No legacy fallback or hidden staging copy.

### Draw identity

Identity starts at zero **for each public range operation** and follows the record
ordinal. Zero-vertex/index/instance records do not renumber later records. A
counted operation uses the same ordinals for its active prefix. Buffer offsets,
strides and prior range operations do not add an implicit identity base.

Thus N separate count-one operations expose zero in each operation; one N-record
range exposes its local ordinals. These are different legal programs. Combining
or splitting them is an optimization only with a valid proof preserving every
observable shader result and dependency—not an assumed universal lowering.
No new first-instance/base-instance permission is implied by draw identity.

## Graphics capability policy and native mapping

Select an explicit modern **graphics** baseline with multi-record indirect draws,
GPU counts and shader draw parameters. Query and require the corresponding native
features at graphics-device creation, enable them deliberately, and reject an
unsupported graphics profile rather than looping/emulating silently. Compute-only
device creation keeps its current requirements. This is a scope decision, not a
claim that every physical device supports the expanded graphics profile.

Add physical/enabled capability fields `multi_draw_indirect`,
`draw_indirect_count`, `shader_draw_parameters` and the enabled limit
`max_indirect_draw_count`. Physical support remains distinct from enabled
execution; compute-only and unsupported Metal graphics expose zero enabled
fields/limit. The limit is the actual native bound, not the experiment's 512.
SPIR-V draw-parameter use is valid only in its allowed stages with the feature
enabled. Generated artifact requirement checks must agree with those rules.

Vulkan maps a fixed range to one `vkCmdDraw[Indexed]Indirect2KHR` and a counted
range to one `vkCmdDraw[Indexed]IndirectCount2KHR`, using the exact checked address
span/stride/count range. Bind state/root once per range. The zero-capacity case
may emit no draw. Metal graphics remains explicitly unsupported until native
implementation/validation establishes a mapping; no compute regression is allowed.

## Alternatives rejected or deferred

| Alternative | Disposition |
|---|---|
| Keep one-record calls and assume backend fusion | Cannot communicate arbitrary GPU counts; identity-sensitive fusion needs proof |
| Mask all inactive records | Valid consumer strategy, retained as a control; not the fundamental count operation |
| Separate fixed/count functions with duplicate range arguments | Unnecessary duplication for the same binding/lifetime model; one nullable count source selects the native operation |
| Bare record/count addresses | Smaller shape, but loses checked spans and retained backing; handles still lower to native address commands |
| Require every compute device to support the graphics expansion | Unrelated restriction; graphics creation selects this profile |
| General device-generated pipelines/root/state commands | Deferred; this decision covers record ranges and a count word, not arbitrary GPU command topology |

## Implementation and acceptance order

1. Coordinate header/runtime ABI, feature discovery/enablement, native bindings,
   range validation and retained step representation. Migrate every in-tree
   caller, generator output, mock, ABI test and explicit Metal unsupported entry.
   Keep single-record controls with capacity one; no old encoder duplication.
2. CPU tests: overflow, zero/one capacity, stride/alignment, terminal byte bounds,
   nullable count mode, maximum limit and unchanged output on rejected descriptors.
3. Public GPU tests: fixed/count execution, identity (including zero-count holes,
   multiple ranges and nonzero record offsets), padded strides, shared/separate
   count backing, zero/partial/full/above-capacity count, both index widths and
   non-indexed parity. Reject foreign/mismatched objects without corrupting a
   recording. Test handle destruction, retained count backing, reset/serial replay,
   failed preparation/submission and drain cleanup under existing failure policy.
4. Extend the matched scene to public ranges with identical native shaders,
   producer policies, storage budgets and one/two-slot reset/replay. Retain the
   independent one-record/identity-sensitive comparisons and explicit native-call
   traces. Run the predeclared host-sensitive timing matrix after correctness.
5. Complete generated/installed stage-pair and range/count consumer handoff.
   Record the implemented contract decision and remaining limits, then proceed
   to mip/view/blend slices. No second GPU or Mac prerequisite for Vulkan scope.
