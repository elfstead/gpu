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

M10's primitives and clean SDK are complete within the recorded local scope. M11's
implementation starts with the numeric format and execution-control packages above.
The remaining rows are scheduled work, not implemented claims or vague examples.
