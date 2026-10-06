# ABI-20 implementation checkpoint, not performance acceptance

2026-10-06. M4's argument-contract experiment remains open. The public range-update
prototype was implemented at `23c0508`; `caa3555` balances the four timing paths.
The C API can now express unchanged arguments across changing operations and exact
four-byte updates, without immutable public objects, full-root resupply, shader
indirection or automatic compute/graphics restoration. See the
[contract](recording-arguments-prototype.md).

## Evidence before the recording-time deduplication correction

Clean `23c0508` correctness passes 72 cases × 992 frames on Radeon and 72 × 64 on
llvmpipe: **76,032 frames**. Native/public full/partial controls pass eight cases
per driver, checking output, allocation budgets and exact update ranges. There
are 35 passing GPU tests per driver, 49 ordinary tests, 31 scene-runner tests,
847 C/Rust layout checks, formatting, Clippy and mock-loader checks.

The ABI-20 installed scene passes both index widths on both drivers, including
regenerated/reordered shader interfaces with unchanged host code. The strict
llvmpipe counted→fixed diagnostic still fails five of eight native cases; its
installed range consumer still fails 12 configurations. These failures are not
waived or fixed by the argument change. Radeon passes both checks.

Retained local reports (raw files remain under `target`, not a release artifact):

| Check | Report under `target/graphics-scene/` |
|---|---|
| Radeon reuse | `argument-reuse-wpny0r9a/report.json` |
| llvmpipe reuse | `argument-reuse-zmg6gjlx/report.json` |
| Radeon partial updates | `argument-patch-cimk_d0z/report.json` |
| llvmpipe partial updates | `argument-patch-u90rqj27/report.json` |
| Radeon grouped/scoped regressions | `range-reuse-d8apm7_d/report.json`, `range-reuse-5g_2iizu/report.json` |
| Native count→fixed, Radeon/llvmpipe | `count-followup-i5qpfgo8/report.json`, `count-followup-_4bfj7io/report.json` |
| First four-path timings | `argument-timing-5ahsx6kx/report.json` |

## Timing discrepancy retained, not explained away

The clean timing matrix has 288 processes, 285,696 measured frames and 28,800
warmups. At 512 draws, one slot, 256-byte arguments it measured median wall times
of **0.206 ms native once / 0.382 ms public convenience / 0.289 ms public current**.
This is not enough to claim performance acceptance: it is worse than the previous
checkpoint and varies substantially in subsequent focused runs.

Diagnostic controls committed at `448c44d` narrow the question:

- Native pushes before versus after pipeline binding measure 0.2050 versus
  0.2045 ms; this bounded test does not explain the regression.
- A six-way, balanced same-session comparison measures 0.2033 ms native,
  0.2599 ms old public, 0.2838/0.2797 ms new installed/workspace convenience,
  and 0.2363/0.2378 ms new installed/workspace current-bank operations.
- The old installed runtime's source is unchanged between `4671f67` and
  `befc5a8`. The runner uses matching installed headers, checks actual library
  resolution and hashes, and checks every final slot. No ABI mismatch bypass.
- These diagnostics are development runs, not clean acceptance. They do not
  establish a build-artifact cause or explain the first matrix's larger wall
  times. Do not select only the faster series or infer isolated GPU time from
  host wait intervals. The API strategy is expressible; total parity is not proved.

The [checkpoint manifest](results/recording-arguments-checkpoint-2026-10-06.json)
records report/log hashes, environments, revision/dirty status and focused timing
ranges. It indexes local evidence, not an independently redistributable archive.
Local diagnostic reports are `argument-order-m6xe_tit/report.json`,
`argument-runtime-8pveeo2n/report.json` (four paths) and
`argument-runtime-be750phj/report.json` (six paths).

## Correction and next stopping condition

The first implementation copied each convenience root into a new update step,
then elided identical pushes during encoding. The correction compares supplied
bytes against the last owned snapshot during recording, avoiding duplicate
allocations, copies and steps. Changed ranges still copy at call time; failed
operations still append nothing. Current-bank operations inspect no payload.
The separate encoding-time argument cache is removed, not retained as a second
implementation. A CPU test verifies 512 redundant updates across vector growth
reuse one snapshot and that changed ranges/bytes still own independent copies.

This correction passes all 35 GPU tests on each driver and the native/public
partial-update controls on both, plus ordinary tests and Clippy. Those are
development-tree checks, not fresh clean-revision acceptance. It has **no accepted
fresh performance result yet**. Next: clean-revision reuse/partial-update and installed-consumer
checks, then a bounded matched timing rerun retaining the earlier discrepancy.
Close the retain/change decision with explicit remaining implementation costs;
do not turn this into an indefinite tuning milestone or claim universal Vulkan
parity. Then resume M4's mip/view and blend/dynamic-state slices. No new GPU or
Mac is needed for that local sequence.
