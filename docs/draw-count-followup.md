# Counted → fixed draw regression

The ABI-19 compiler/installed-consumer work adds a strict edge-case gate:
an indirect count operation followed by an independent fixed range. The second
operation must not inherit the first operation's GPU count or draw identity.
This is not optional batching behavior and needs no API workaround.

Development runs on 2026-10-01 found a local llvmpipe Mesa 26.2.1 failure: after a
zero-count operation the following fixed range leaves the clear color intact.
The public consumer reproduces this for indexed/non-indexed, both index widths,
separate/shared count storage, and reset/serial replay. Radeon passes.

`examples/graphics_scene/count_followup.py --native <accepted-identity-report>`
builds a direct Vulkan control with no OGPU linkage. It emits counted then fixed
indexed draws in the same rendering scope, using the native frontier's unchanged
shaders and GPU producer. Every fixed range should restore full-capacity output.
On llvmpipe, all eight images instead match the count-only reference; five differ
from the required full image. Radeon passes all eight. Validation reports no
error, and native-call traces establish that both commands were issued. Reports
retain `complete` separately from `passed` and the diagnostic exits nonzero on
pixel mismatch. This isolates the observed failure outside OGPU; it does not
identify the exact driver source defect or establish behavior on another version.

Keep the ordinary consumer strict. Do not silently split scopes, inspect counts
on the CPU, substitute masked records or add driver-specific runtime branches.
Retain the failure and reproducer for a future driver fix/recheck. Do not claim
software-driver acceptance for this sequence. Unaffected local checks and Radeon
consumer/performance work can continue; no new GPU or Mac is required.

The native diagnostic is a correctness control, not part of the timing strategy
matrix. The larger scene's matched reset/replay timing and installed handoff remain
separate from the small generated range consumer.
