# GPU-generated UINT16 indexed scene

Accepted 2026-09-30 at clean **`4e763accc324b4adcc2ded23b1401e7656b0700e`**.
This closes the full scene's index-width gap after the
[UINT32 comparison](graphics-scene-public-results.md), without changing the API.

The same eight vertices, signed vertex offsets, nonzero first index, ten modes
and A/B/A movement now execute with UINT16 index binding on native Vulkan and the
public ABI. Compute packs pairs using ordinary uint operations; no new shader
16-bit requirement is introduced. Allocation sizes are unchanged. The newly unused
16 bytes in the index buffer remain poisoned and are checked by the geometry oracle.

| Driver | Extents | Frames per consumer | Independent pixel checks per consumer |
|---|---|---:|---:|
| Radeon RX 5700 XT | 257×193, 640×360, 1280×720 | 90 | 35,187,870 |
| llvmpipe | 257×193, 640×360 | 60 | 8,017,950 |

All independent color/depth/geometry/guard checks pass with Vulkan/synchronization
validation. Every native/public output byte agrees. Additionally, every color/depth
image agrees with its same-driver accepted UINT32 counterpart at `0999689`,
including raster edges: 576,814,560 bytes compared across widths on Radeon and
134,431,200 on llvmpipe, counting both consumers. Reference artifact hashes, device,
extent and frame identities are checked before acceptance. Geometry is intentionally
not equal across widths; each layout is independently checked instead.

Five oracle tests and two comparison suites pass, including wrong-width geometry,
unused-tail corruption, missing/truncated output, altered reference hashes and
cross-width image mismatch rejection. Both consumers compile with warnings denied;
Clang static analysis also passes. No new sanitizer result is claimed.

## Retained evidence

- [Radeon report and adjacent logs](results/graphics-scene-u16-radv-2026-09-30/report.json)
- [llvmpipe report and adjacent logs](results/graphics-scene-u16-lvp-2026-09-30/report.json)

Source/shader/executable/runtime/output/reference hashes were verified, and the
exported reports/logs match their originals byte-for-byte. Original report hashes:
`dc69b81d75bfed188536b20cbe6a7c79762b59558d6a18b7602862bba083289c` (Radeon),
`f28da5466d607d40259eafbfd924756e9fa7021ed8203f6a1badd0b58abce494` (llvmpipe).
Raw binaries remain in local `target/graphics-scene/matched-25rvtpvm` and
`target/graphics-scene/matched-6a2k3nd6`; git retains hashes/logs, not image dumps.

Reproduce with the [scene prerequisites](../examples/graphics_scene/README.md), a
selected ICD, Vulkan/sync validation and a freshly accepted same-driver UINT32
report. Run `run.py --public --index16 --reference32 PATH/TO/report.json`, adding
`--scale` for Radeon. Both widths have build-only `--check` coverage. The original
UINT32 reports used here remain retained alongside their own acceptance checkpoint.

The independent-index-width correctness gate is complete. Scene two-slot/replay
execution, performance, generated/installed stage support and the remaining M4
graphics features are not implied. Proceed with the
[reuse and native-performance experiment](indexed-depth-performance-plan.md).
