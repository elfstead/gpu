# Argument reuse: matched native strategies and host costs

Accepted 2026-10-06 at clean **`955bdd7`**, ABI 19. This completes the declared
bind-once/resupply/current-public matrix, not general Vulkan parity or selection
of a new public argument API.

## Correctness and actual work

All **57,024 frames** pass: 54 configurations × 992 frames on RX 5700 XT / RADV,
plus 54 × 64 on llvmpipe, Mesa 26.2.1. Roots are 8/64/256 bytes; capacities are
1/64/512 separate indexed draws; one/two slots use caller-owned reset/re-record
storage. Per-slot roots are initialized once while draw offsets alternate
forward/reverse on re-recording. This is not whole-list replay. Every shader
payload word contributes to output; DrawIndex remains zero for every call.

All output/geometry/count/control/guard bytes match the independently checked
reference. Native/public shader artifacts match within each root size. Nine GPU
allocations per slot, allocation size/type multisets, peak bytes and final zero
live allocations match across all three paths. Traces verify actual draws,
forward/backward address steps, scopes, pushes, pool reuse, submissions and no
successful queue-idle. Existing native/public dependency differences remain
labelled, not hidden by weakening native. A fresh original range/reset/replay
preflight also passes 1,152 frames; it is not new sustained acceptance.

At 512 draws and a 256-byte root, per recording (graphics root only):

| Path | Root supply calls | Declared bytes supplied | Actual native pushes / bytes |
|---|---:|---:|---:|
| Native bind-once | 1 | 256 | 1 / 256 |
| Native resupply | 512 | 131,072 | 512 / 131,072 |
| Current public | 512 | 131,072 | 1 / 256 |

The separate compute push is 32 bytes on every path. Supply counters describe
call-boundary payloads, not physical loads/cache traffic. Source inspection shows
the current public path copies each successful root into its retained step,
then compares owned content during encoding. Native push deduplication does not
remove this repeated collection work or the contract's call-time value semantics.

## Timings

The Radeon matrix passes **162 fresh processes, 160,704 measured samples and
16,200 drained warmups**. Three rounds rotate path and root-size ordering.
Every final slot is checked after each warmup and measurement window; GPU image,
depth and geometry copies remain inside every measured recording. Validation,
command tracing and supply counters are disabled. Four additional host clock
reads/frame bracket collection/recording and encoding/submission on both paths;
their overhead remains in these timings. See the [predeclared protocol](argument-reuse-plan.md#matched-reuse-controls-and-timing-protocol).

Values below are medians of three process summaries, not pooled independent
samples. Per-process distributions and ranges remain in the receipts; no isolated
GPU duration, confidence interval or universal tail comparison is claimed.

| 512 draws, one slot | Native once wall (ms) | Native resupply wall (ms) | Public wall (ms) | Public/native once |
|---|---:|---:|---:|---:|
| 8-byte root | 0.2047 | 0.2267 | 0.2528 | 1.235 |
| 64-byte root | 0.2037 | 0.3967 | 0.2601 | 1.277 |
| 256-byte root | 0.2033 | 0.4526 | 0.2630 | 1.293 |

Two-slot public/native-once wall ratios are 1.010/1.008/1.006 respectively;
overlap conceals host costs. Native resupply/native-once ratios at two slots are
1.030/1.074/1.140. Root sizes use different vertex shaders, so compare paths
within a size; do not treat cross-size wall differences as isolated copy cost.

| 512 draws, one slot | Native once host total (µs) | Native resupply host total (µs) | Public host total (µs) | Public collection (µs) | Public encoding+submit (µs) |
|---|---:|---:|---:|---:|---:|
| 8 bytes | 39.00 | 56.00 | 74.41 | 27.17 | 46.97 |
| 64 bytes | 38.03 | 140.92 | 76.18 | 29.76 | 45.79 |
| 256 bytes | 37.98 | 163.83 | 77.83 | 32.24 | 46.04 |

Phase medians are not additive. Public collection includes batch creation,
validation, retention, all scene operations and root allocation/copying; it is
not a pure memcpy measurement. Public submit includes deferred native encoding,
finalization and queue submission. Native records commands directly (roughly
25 µs here), then prepares/submits (roughly 13 µs). Subtracting those unlike
phases would not isolate a contractual penalty. Remaining control writes,
destruction and bookkeeping belong to whole host time, outside the subintervals.

## Decision

Keep the existing consecutive-state optimization: repeatedly pushing identical
large roots is materially worse in this local control. Do **not** use that weak
native-resupply path as the parity baseline; native bind-once remains attainable
while command order changes. Also do not assign the whole public/native host gap
to the API: validation, allocations, deferred encoding and other implementation
work are mixed into it. The native host compare/dedup control was optional and
has not been measured, so payload inspection alone has no isolated timing here.

The structural difference is now explicit and exercised: current public calls
cannot express unchanged root values independently of operation count, without
relying on whole-list replay or changing shader/storage representation. Under
the project's performance-preservation criterion, this exposes a better API
direction: explicit argument update/reuse. It does not select its exact surface.
The [immutable-snapshot versus recording-local comparison](argument-reuse-alternatives.md)
remains relevant; a prototype must preserve mutation, failure atomicity,
executable independence and replay ownership without adding a compulsory
allocation/scheduler.

One bounded native capability must be covered before choosing that surface:
updates can target a byte range within existing pushed data, with four-byte
alignment and the device's size bound. A whole-block-only replacement primitive
could retain a structural disadvantage when just one scalar changes.
[Vulkan push-data update description](https://docs.vulkan.org/refpages/latest/refpages/source/VkPushDataInfoEXT.html).
Next compare full resupply with a four-byte native update using the observable
64/256-byte snapshot shaders. Retain identical output and root representation;
this is a correctness/expressibility control, not another broad timing campaign.
Then declare the selected candidate's lifetime, initialization, partial-update,
cross-stage, rejection and replay rules before implementing it. Mip/views remain
behind that bounded argument decision; no additional GPU is required.

## Receipts

[Committed evidence](results/argument-reuse-2026-10-06/) includes both complete
correctness reports, the timing report, all runner logs, baseline-regression log
and an 18-case aggregate summary. Report wrappers preserve every original field,
original path and SHA-256; normalized JSON is not the original serialization.
Per-frame logs/samples and reference binaries remain at recorded local paths
with hashes. No Metal, second physical GPU, general dispatch parity or API
stability is claimed; the strict llvmpipe counted→fixed failure is unchanged.
