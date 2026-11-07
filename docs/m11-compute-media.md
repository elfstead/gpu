# M11 advanced compute, artifacts and media

Implementation starts 2026-11-07 after [M10 consolidation](m10-consolidation.md).
The hardware batch is optional. C and direct Rust advance together; no feature
becomes a device baseline merely because it is useful to this milestone.

## Work order and concrete exits

| Work package | Native mapping and selected interface | Exit evidence |
| --- | --- | --- |
| 64-bit component formats | Append the twelve pinned core formats to the ordinary catalog; preserve exact image/vertex/storage capability queries | Complete registry disposition, byte-accurate transfer/view tests where native format use exists; shader use needs its own declared numeric requirements |
| GPU conditionals and primitive counters | Native predicate-address begin/end, explicit secondary inheritance; extend existing query pools with primitive-generation kind and independent feature variants | Predicate zero/nonzero/inversion, secondary propagation, no CPU decision/readback; primitive counts independent of transform feedback, discard/nonzero-stream variants separately queried |
| Compute shader profiles | Audit remaining narrow storage, subgroup/control-flow, explicit workgroup layout, derivatives and matrix profiles against the pinned Vulkan/Slang contract | Supported/enabled/required distinction, exact shader declarations and output checks; specialization remains explicit and cold |
| Launch and matrix execution | Use existing specialization, subgroup choice, address arguments and dispatch first; add records only for native cluster/cooperative/device-enqueue semantics that cannot compose efficiently | A bounded hardware matrix consumer with exact tuple/layout/accumulation permissions; no scalar substitution or automatic precision selector |
| Pipeline artifacts | Explicit native binary identity/data/import/capture and executable inputs; audit ray libraries and capture/replay alongside it | Cold compile/capture/recreate equivalence, mismatched identity rejection, ownership/failure paths; no hidden disk cache, recompilation fallback or invented cross-driver binary compatibility |
| Plane-aware images | NV12/P010-class two-plane and corresponding three-plane/packed/padded catalog entries; per-plane extent/aspect/copy/view/placement and native conversion contract | Plane/pitch/alignment/alias validation, guarded copies and conversion/reference output where supported; no hidden repacking or color conversion |
| Initial codec sessions | H.264/AVC 8-bit 4:2:0 decode and encode as independent profiles; native capabilities, explicit session backing/parameters/reference slots, bitstream ranges and status | Deterministic decoded frame/reference checks, encode bitstream decode-back with declared error bounds, explicit reference lifetime/queue dependencies; no container demux, stream parser, player, automatic rate control or implicit DPB ownership |
| Consolidation | Whole-surface structural-cost review and clean C/direct-Rust SDK | Every selected deliverable has implementation status plus native acceptance status; unavailable hardware is a recorded gap, not a successful test |

The first codec selection is deliberately explicit: H.265/AV1, additional bit
depths/chroma formats and vendor media engines remain subsequent capability
profiles. Plane format support does not imply codec support. Codec-specific syntax
must stay representable without a runtime parser; any choice of standardized
parameter records and external header dependency is recorded before implementation.
A universal video-session object must not hide different native reference,
allocation or parameter-update rules.

## Boundary decisions

Native conditional execution cannot generally be replaced by a CPU predicate
readback without a synchronization/cost change. Primitive-generation queries are
also distinct from transform-feedback counters: forcing capture merely to obtain
a count adds unrelated work/state. Both warrant direct native mechanisms.
[Conditional rendering](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_conditional_rendering.html),
[primitive-generation queries](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_primitives_generated_query.html).

Pipeline binaries let an application own individual compiled artifacts rather
than only an opaque aggregate cache. This is a distinct native persistence/creation
choice; retain the existing cache mechanism only where it expresses a different
native facility. Binary import must preserve exact compatibility and ownership,
not advertise portable machine code.
[Native pipeline binaries](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_pipeline_binary.html).

The initial media profile maps to native queue/session/parameter/reference
operations, not host tensor/image processing. It must compose with M10 interop and
explicit queues. Same-device and external image use do not imply an automatic
ownership transfer or plane layout conversion.
[H.264 decode mapping](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_video_decode_h264.html).

## Assumptions and proof obligations

The current Vulkan backend can expose only capabilities its driver advertises and
its selected device enables. A format ID or compiler-accepted shader does not prove
native execution. Local RX 5700 XT lacks advertised hardware matrix and native ray
profiles; implement/query unsupported paths honestly without inventing acceleration.
Metal/Windows acceptance remains deferred. No second GPU is assumed for progress.

Richer launch facilities require a native mapping audit before choosing records.
Per-launch dynamic workgroup memory, cooperative grids, workgroup clusters and
GPU-created work are not interchangeable. Compilation specialization is an adequate
expression only where the native operation itself fixes that property at pipeline
creation. It must not be presented as equivalent to a native per-dispatch change.
Backend limitations remain explicit capabilities, not restrictions imposed on all
future implementations.

Performance review asks whether a direct native implementation of the contract can
preserve the native workload and control. Reject compulsory extra GPU operations,
allocation, staging, scheduling, tracking or host synchronization. Use targeted
experiments for uncertain mappings and correctness; do not benchmark every obvious
translation step before implementing the coherent surface.

## Working status

M10's primitives and clean SDK are complete within the recorded local scope. The twelve numeric format IDs, metadata and local transfer/clear consumer are
implemented; three image profiles pass and nine are natively excluded. Typed
shader/vertex execution remains separate. GPU conditionals and primitive-generation
queries are implemented through C and direct Rust. Local primary compute/draw
predication and primitive queries without transform feedback pass; inherited
conditional execution is unavailable on this device. The remaining shader profiles,
launch/matrix audit and artifacts are next.
The remaining rows are scheduled work, not implemented claims or vague examples.

## GPU execution controls (2026-11-07)

Feature IDs 143–147 independently expose conditional execution, inherited
conditional execution, primitive-generation queries, query support with rasterizer
discard, and nonzero query/rasterization streams. Two recording operations borrow
a four-byte predicate range; an optional inheritance extension preserves the
existing secondary-record ABI. One new query kind uses the existing pool,
reset/begin/end/resolve and explicit stream argument. The primitive feature does
not require enabling transform feedback. Its extension dependency and stream limit
come from native transform-feedback *extension* metadata, not an active capture.

The public contract names exactly which operations are predicated. Draws, compute
dispatches and generated-command execution are covered; copies/fills, barriers,
queries, rendering load/store, acceleration builds and direct ray dispatch remain
unconditional. Predicate values stay stable within a scope, with explicit
CONDITIONAL stage/read dependencies. Local scope checks prevent unmatched/nested
begin/end; discard and rasterization-stream restrictions remain documented unsafe
caller obligations. No extra work, CPU decision, retention or implicit dependency
is introduced. Recording has constant-time local checks and one native call;
query slots are reserved with the arena, not allocated while recording.

The independent [Rust consumer](../examples/foundation_rust/src/bin/controls.rs)
enables `[RASTER, CONDITIONAL_RENDERING, PRIMITIVES_GENERATED_QUERY,
PRIMITIVES_GENERATED_DISCARD]`, deliberately excluding TRANSFORM_FEEDBACK. GPU
fills produce predicates; zero/nonzero and inversion produce four checked compute
outputs, four primitive counts and four unconditional fill markers, with surrounding
4096-byte guards and five local rejection cases. Vulkan synchronization validation
is clean. Conditional inheritance is implemented and its native chain has host
coverage, but RADV reports that feature unavailable: the consumer's secondary
execution branch is **not accepted**. Nonzero stream emission, predicated mesh/
generated work and attachment clears likewise need separate execution evidence.

An explicit `OGPU_CONDITIONAL_RESET_DIAGNOSTIC=1` run reuses a native slot after
abandoning an open predicate and resetting the pool. It produces one
`VUID-vkCmdBeginConditionalRendering2EXT-None-13063` diagnostic despite OGPU resetting
its local scope. Matching VVL 1.4.357 source at
`e4786f7ce8f1319215eff0d938f4be4651cbb85d` leaves `conditional_rendering_active` out of
`CommandBuffer::ResetCBState()` (cmd_buffer_state.cpp:305); it is set/cleared only by
conditional begin/end (2519/2526). This supports a stale validation-state diagnosis.
The normal negative-case fixture uses separate reserved slots; it does **not**
establish abandoned-scope same-slot validation acceptance. No workaround or extra
native end command is inserted into OGPU. The raw diagnostic is kept separately
from the clean execution receipt.

[Receipt](results/m11-execution-controls-2026-11-07.json).
