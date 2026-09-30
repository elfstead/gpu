# Indexed scene: stable slots and serial replay

Accepted 2026-09-30 at clean **`637c7db9da19c0ed8d79e698a831f04aeb351056`**.
This completes the first correctness increment of the
[reuse/performance brief](indexed-depth-performance-plan.md), not its timing or
stronger-native-strategy gates. No runtime/API change was needed.

## What is implemented

Both consumers use one device/queue and shared prepared executables. One/two
slots own fixed attachments, guarded vertex/index/indirect/control/readback
buffers and either explicit reset/re-record storage or one serial compiled list.
Phase and empty flags move from copied root values into mutable pointed-to input;
the immutable root retains its addresses. Compute still writes geometry and draw
records on the GPU, with no intermediate CPU readback.

Each slot independently cycles A/B/empty/A, preventing the two-slot stale-input
trap. The consumer submits both slots before observing either, reads/mutates a
slot only after retirement, and drains every slot before destroying shared or
pointer-referenced backing. Native and public use identical shader binaries,
resource sizes/types and dependency/attachment policies. No list-per-input
variant, extra generation submission or successful queue-idle call is introduced.

## Accepted matrix

UINT16/UINT32 × modes 0/6/7/8 × one/two slots × reset/replay × native/public gives
64 configurations per extent. Modes cover grouped draws, LOAD preservation, and
independent color/depth clear. In mode 8 movement is occluded in the image, but
the generated geometry still changes and is checked.

| Driver / extent | Configurations | Frames/configuration | Checked reuse frames |
|---|---:|---:|---:|
| Radeon / 257×193 | 64 | 1,000 | 64,000 |
| Radeon / 1280×720 | 64 | 8 | 512 |
| llvmpipe / 257×193 | 64 | 64 | 4,096 |

All **68,608 reuse frames** pass with Vulkan/synchronization validation and tracing.
Before streaming, the runner rechecks hashes and independently recomputes the
analytic oracle for all 30 serial reference frames per index width/extent.
Each retired stream frame then passes full-byte image/geometry/guard comparison
against the appropriate reference, plus the entire control buffer. Analytic
expectations are cached through exact equality, not recomputed per repeated frame.

The refactored original serial consumers also run all ten modes and A/B/A frames:
240 regression frames on Radeon and 120 on llvmpipe. They match the previously
accepted serial artifacts byte-for-byte, with balanced allocation lifetimes.

## Strategy and memory evidence

Native-call tracing establishes actual behavior, not merely consumer mode labels:

- Serial replay has **zero hot command-buffer encodes, rendering-scope/draw
  recordings, resets or pool creation/destruction**, and one submission per frame.
- Reset/re-record encodes once per frame. Hot pool creation is bounded to one per
  slot, not one per frame. Public retirement resets after each frame; native resets
  before the next recording. Both preserve the chosen reuse policy.
- Every scope and indexed draw is counted. Grouped mode has one scope/two draws;
  preservation modes have two scopes/two draws per encoding. No successful path
  calls queue-idle. All native pools are destroyed at final teardown.
- Each slot has nine tracked buffer/image allocations. Native/public allocation
  size/type multisets and peak bytes agree in every matched configuration. Every
  allocation is freed; final tracked live bytes/count are zero.

| Extent / driver | Requested bytes per slot | Traced peak allocation bytes per slot |
|---|---:|---:|
| 257×193 / Radeon | 795,432 | 1,082,696 |
| 257×193 / llvmpipe | 795,432 | 825,120 |
| 1280×720 / Radeon | 14,747,416 | 15,394,616 |

Two slots double these values. They are not total process/device memory: private
driver command-memory bytes and executable/CPU allocation budgets are not measured
by this tracker. Two unretired receipts establish a legal consumer schedule, not
physical GPU overlap. Validation/tracing and per-frame logging preclude timing claims.

## Verification and evidence

- [Radeon report and per-process logs](results/graphics-scene-reuse-radv-2026-09-30/report.json)
- [llvmpipe report and per-process logs](results/graphics-scene-reuse-lvp-2026-09-30/report.json)
- [Build/CPU/regression/sanitizer logs](results/graphics-scene-reuse-validation-2026-09-30/build.log)
  (other logs are alongside it).

Source, executable, runtime, shader, reference and log hashes were verified.
Reports and per-process exports are byte-identical to their originals. Original
report hashes: `0e4abba446677c392a5597a41507d4190f5b9a515099dd70d2ed54c6df2c7142`
(Radeon), `2d74e16ad35979a94045245a31b6b4c9a66212482d6dec42666747b41da3ae04`
(llvmpipe). Local directories are `target/graphics-scene/reuse-4qbxx5p8` and
`target/graphics-scene/reuse-4fjf6fzu`; serial regression image dumps remain local.

C consumers compile with warnings denied. CPU tests cover numeric parsing,
2,000 generation checks, every control-buffer byte, corrupt images/geometry and
unchanged counters on rejected checks. Parser tests reject false/incomplete frame,
allocation and native-call reports. Full runtime regressions remain **32 GPU tests
per driver**, 41 ordinary Rust tests, Clippy with warnings denied and 824 ABI values.

ASan/UBSan development checks pass on both C consumers for 64 two-slot replay
frames in mode 8, UINT16, Radeon. LeakSanitizer is disabled (`detect_leaks=0`);
these are not CPU leak-detection results. Clang analysis is not a clean acceptance:
it reports a potential leak at the public slot's first buffer write. The source
cleanup frees each slot's CPU buffer, but the diagnostic remains retained in
`analysis.log`; do not describe the sanitizer or native-allocation trace as an
exhaustive proof about CPU allocations. Auxiliary text logs normalize trailing
blank lines; the two reports and their process logs do not.

One development preflight correctly stopped on an overstrong test assumption:
mode 8 need not change final pixels between A/B. The corrected gate requires
changed geometry and preserves every image/geometry/guard comparison. Its failed
report remains local at `target/graphics-scene/reuse-2xara7mq`; it is not acceptance.

Reproduce with the commands and prerequisites in the
[consumer README](../examples/graphics_scene/README.md), using accepted same-driver
UINT16/UINT32 references. Radeon uses default 1,000 frames plus `--scale`;
llvmpipe uses `--software`. `--preflight` is explicitly development evidence.

## Decision and next step

Keep the current immutable-list/mutable-data contract for the next experiment:
stable indexed/depth reuse is expressible and actual native re-encoding is avoided.
This does not approve its fundamental performance. Next implement the
1/64/512 grouped-draw timing controls, distinguish redundant backend bindings from
contract restrictions, and test stronger native multi-record/count-buffer paths.
Then complete generated/installed scene handoff before the mip/view slice.
