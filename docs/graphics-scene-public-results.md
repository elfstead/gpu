# M4 matched public/native indexed scene

Accepted 2026-09-30 at clean **`0999689`**. The ABI-18 public consumer reproduces
the [native reference](graphics-scene-native-results.md) through `ogpu.h` alone.
This completes the ten-mode matched scene gate, not the complete indexed/depth
contract, performance gate or M4.

Both consumers execute the same SPIR-V: compute writes DEVICE vertex/index/indirect
data, followed by real indexed drawing without intervening CPU readback. Independent
color/depth CLEAR/LOAD scopes, signed vertex offsets, poisoned index prefixes,
zero-count records and A/B/A geometry changes retain the native scene's expectations.
Each frame uses fresh recording and stable allocations; this is not scene replay.

| Driver | Extents | Frames per consumer | Oracle-checked pixels per consumer |
|---|---|---:|---:|
| Radeon RX 5700 XT / RADV | 257×193, 640×360, 1280×720 | 90 | 35,187,870 |
| llvmpipe | 257×193, 640×360 | 60 | 8,017,950 |

Every frame passes the independent analytic color/depth oracle, generated geometry
and guard checks. Native/public files additionally match **every byte**, including
raster edges excluded by the analytic oracle: 288,459,840 bytes on Radeon and
67,250,640 on llvmpipe. A-repeat and equivalent-mode comparisons pass separately.
Vulkan and synchronization validation are enabled; no validation errors occurred.
These are same-driver comparisons, not a universal cross-device rasterization claim.

Four oracle tests and one comparison suite pass. The comparison suite rejects
truncated, altered and missing output files. Both C consumers compile with warnings
denied. Undefined-symbol checks confirm the native control does not link OGPU and
the public consumer uses the indexed OGPU entry point without direct Vulkan calls.
Pinned Slang reflection/SPIR-V checks pass; this still bypasses the installed
adapter's restricted vertex-root/stage-pair subset.

## Evidence and reproduction

- [Radeon report and adjacent execution logs](results/graphics-scene-public-radv-2026-09-30/report.json)
- [llvmpipe report and adjacent execution logs](results/graphics-scene-public-lvp-2026-09-30/report.json)

Reports record clean revision `0999689873019ee740cbb1e10aa18db55888c7c5`, device,
environment, commands, source/shader/runtime/executable hashes, complete frame
checks and output hashes. Exported reports/logs were verified byte-identical to
their originals; every retained source/shader/executable/output hash was checked.
Original report SHA-256 values are respectively
`7dddf75462f9d0996465b9b8eeaf486e90b7d38554611f81c682beacea3a50d2` and
`73b3e63be5b64ad4a381ddcd7bb6318787aca2687fd31cb68a265c61906c8b04`.
Large raw outputs remain locally in `target/graphics-scene/matched-hx0vfkhq` and
`target/graphics-scene/matched-jtx1rlog`; git retains hashes and logs, not binaries.

Select exactly one ICD with `VK_DRIVER_FILES` and `VK_ICD_FILENAMES`, enable
`VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` and `VK_LAYER_VALIDATE_SYNC=1`,
and provide pinned Slang 2026.14.1 plus SPIRV-Tools and the build toolchain. Run:

```sh
python3 examples/graphics_scene/run.py --public --scale # Radeon
python3 examples/graphics_scene/run.py --public         # llvmpipe
```

## Decision

Keep the current scope/index/depth shape for the next experiments. The public
boundary expresses this GPU-generated chain without a CPU inspection or forced
scope break per draw. That is correctness/expressibility evidence, **not** evidence
that an ideal native implementation preserves every Vulkan performance strategy.

Next: the remaining compare/mismatch/failure matrix, then stable two-slot scene
reuse and matched grouped small-draw/replay measurements. The latter must expose
host-sensitive costs, not infer API adequacy from a large GPU-heavy frame. Generated
stage-pair and installed-scene handoff remain required before moving to mip/views.
No Metal, new physical GPU, sanitizer or timing acceptance is claimed here.
