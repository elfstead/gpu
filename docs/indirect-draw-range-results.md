# ABI-19 indirect range migration checkpoint

Accepted 2026-10-01 at clean implementation `2c20170`. This implements the
[selected contract](indirect-draw-ranges.md); it is not complete M4 acceptance.

The old one-record argument pair is replaced, not kept as a second encoder.
Fixed ranges lower to one native address-based indirect command; counted ranges
lower to one native count command. Neither reads counts on the CPU or rewrites
producer records. Graphics creation explicitly requires/enables multi-draw,
GPU counts and draw parameters. Compute requirements are unchanged. Count/record
buffers and index/raster objects follow existing retained recording/list/receipt
ownership; zero capacity validates and retains backing without issuing a draw.

## Checks

- 42 ordinary Rust tests, Clippy with warnings denied, formatting, mock loader
  scenarios and 847 C/Rust ABI layout values pass.
- All 33 Vulkan GPU tests pass with validation/sync validation on RX 5700 XT
  (RADV) and llvmpipe. The new range suite covers non-indexed, UINT16 and UINT32;
  padded/nonzero offsets; shared/separate count backing; capacity zero/three;
  changing zero/partial/full/above-capacity counts; rejected descriptors/foreign
  count objects; handle destruction and serial replay lifetime. Existing failure
  tests now retain separate count backing through preparation/submission failures.
- The public scene matches the accepted native draw-identity control in all
  nine 257×193 cases per driver: 1/64/512 records × separate/fixed/count commands,
  eight changing-count frames each. All 144 frames match complete color/depth
  and geometry/count/guard bytes, independent analytic oracles and allocation
  shapes/peak bytes. Multi-record identity remains distinct from separate calls.

Raw reports and native-call/allocation traces:
[RADV](results/indirect-ranges-2026-10-01/radv/report.json),
[llvmpipe](results/indirect-ranges-2026-10-01/lvp/report.json).
Reports identify native reference hashes, source/artifact hashes and retained
local binary dumps; committed text exports preserve the original reports/logs.
The two `gpu-tests.txt` files alongside them preserve clean-revision GPU runs.

These diagnostic public scenes use one-shot batches. At 512 separate draws the
existing bounded cache deliberately declines admission, so pools churn; the
trace parser records and verifies this, rather than presenting it as matched
storage-policy timing. There are no timing samples in this checkpoint.

## Remaining acceptance

Generated draw-identity requirement checking and installed range/count consumption
remain, as do direct identity checks across holes/padded strides/multiple ranges
and matched one/two-slot reset/replay controls and timing. The existing scene
regressions and earlier installed results remain evidence for their recorded
revisions, not automatically ABI-19 acceptance. No new Mac or other physical GPU
validation is claimed. M4 mip/view/blend work follows this bounded draw slice;
this migration does not approve the entire fundamental API.
