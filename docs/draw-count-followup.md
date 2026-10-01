# Counted → fixed draw regression

Clean-revision evidence: `cd8d0e8` (2026-10-01),
[RADV direct Vulkan PASS](results/range-handoff-2026-10-01/count-followup-radv/report.json)
and [llvmpipe direct Vulkan FAIL](results/range-handoff-2026-10-01/count-followup-lvp/report.json).
The reports and accompanying logs are original byte-for-byte exports.

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

## Compiler/installed checkpoint

At the same clean revision, the [strict generated consumer on RADV](results/range-handoff-2026-10-01/range-handoff-clean-radv.txt)
passes all 60 configurations / 540 checked frames. On
[llvmpipe](results/range-handoff-2026-10-01/range-handoff-clean-lvp.txt), all 48
unaffected configurations / 432 frames pass, including fixed→fixed identity reset;
all 12 counted→fixed configurations fail at their initial zero-count frame.
The overall software consumer correctly reports failure, not partial success.

The [relocated Radeon SDK test](results/range-handoff-2026-10-01/range-sdk-clean-radv.txt)
passes supplied and regenerated range artifacts (1,080 range frames), generated
stage pairs, changed compute-root field order with unchanged host code, stale
artifact rejection, exact dynamic-link/dependency paths and missing-loader errors.
The [manifest](results/range-handoff-2026-10-01/installed-manifest.json) identifies
the clean source, ABI 19 and installed file hashes. Original range builds need
no shader compiler or Cargo. Optional regeneration uses pinned Slang.

The same installation passes [no-GPU range regeneration/build](results/range-handoff-2026-10-01/range-sdk-clean-build.txt)
and [unaffected stage/core execution on llvmpipe](results/range-handoff-2026-10-01/range-sdk-clean-lvp.txt).
These are not a software range pass or independent outside adoption. Five new
generator rejection/requirement tests, the existing 15 stage and 16 core generator
tests, six frontier parser tests, installer safety tests, formatting and Clippy
also pass. No runtime/API change beyond the preceding ABI-19 migration.
