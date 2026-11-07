# Pinned format inventory

M8, 2026-11-07. The catalog now exposes 190 formats: 122 ordinary/packed and
68 compressed. IDs 173–190 add A1B5G5R5, alpha-only A8, both A4-first 4444
layouts and fourteen 2D ASTC HDR block shapes. C and native Rust use the same
metadata and the existing image, view, copy, descriptor and query operations.
No new operation, device-enable switch, conversion or decompression path is added.

The [complete disposition](../tests/format-disposition.json) accounts for all 297
format definitions in the pinned Vulkan registry. A tooling check verifies the
registry hash, disjoint membership and exhaustive coverage, alongside native
block dimensions, bytes and view compatibility classes. Updating the registry or
catalog requires revisiting the remaining assignments; new definitions cannot
silently disappear from the inventory.

## Capability and compatibility contract

Per-format and exact image-profile queries remain authoritative. An ID does not
promise that a device supports sampling, storage, raster attachment use or any
particular shape. ASTC family features guarantee a family-wide minimum; the native
specification also permits [individual format queries without enabling that guarantee](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceTextureCompressionASTCHDRFeatures.html).
The [4444 formats](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_4444_formats.html)
are optional core formats since Vulkan 1.3. A1B5G5R5 and A8 belong to the existing
maintenance5 baseline. This batch needs no additional extension enable.

Alpha-only A8 has its own native view compatibility class; it is not interchangeable
with R8 merely because both occupy one byte. The existing component-mapping field
can express user-selected swizzles. Each ASTC HDR format shares its native view
class with the same block shape's UNORM/sRGB formats, with sixteen encoded bytes
per block. A view does not transcode or validate that payload. Callers provide
valid encoded data for the interpretation they use.

## Remaining definitions and schedule

| Family | Count | Disposition |
|---|---:|---|
| 64-bit components | 12 | M11: typed vertex/storage/shader requirements, independent of arithmetic FP64 support |
| Planar, packed 422 and 10/12-bit padded formats | 38 | M11: plane extents/aspects, pitches, conversion and compatible views; six padded ordinary formats travel with this coherent format contract |
| PVRTC | 8 | Later candidate: native extension selection plus encoded texture/copy/sampling consumer; no current native lane or portable decoding fallback |
| ASTC 3D | 30 | Later candidate: native extension selection and true 3D block geometry/copy tests; the current catalog assumes block depth one |
| Optical-flow fixed-point | 1 | Later candidate with a selected native optical-flow workload and numerical contract |
| ARM numeric/media formats | 18 | Later candidate: select native tensor/media profiles and shader/format semantics before exposing these IDs |

The last four rows have explicit entry criteria and are not silently part of M8
completion or the initial M11 codec/numeric profile set. They remain possible
extensions, not prohibited use cases or architectural limits. No claim of exposing
every vendor format is made. M11 must select its actual initial advanced-compute
and codec profiles as already required by the roadmap.

## Evidence

The ordinary-format fixture now includes all four new packed/alpha formats, using
padded copy/readback, image views and byte guards. The compressed fixture discovers
all fourteen HDR formats and exercises their native block copies and compatible
UNORM views where supported. Only valid BC1 payloads currently have decoded-value
sampling checks; HDR sampling remains separate native acceptance work.

Local RX 5700 XT / RADV acceptance passes all four added ordinary formats. The
full matrix executes 71 and excludes 51 native profiles; compressed copies execute
16 BC profiles and exclude 52 ETC/ASTC profiles, including all fourteen HDR
additions. Two BC1 profiles also pass sampling checks. Full C regression has zero
validation errors and four existing unused-output warnings. 118 host tests, two
C/Rust correspondence tests, strict Clippy, surface correspondence and 22 tooling
tests pass. No shader or exported function signature changed.

The first run found an obsolete test assumption that ordinary cases occupy
consecutive catalog slots; the fixture now looks formats up by identity. The host catalog check also needed its terminal count updated from 172 to 190.
Those initial failed logs is preserved alongside the corrected result in the
[evidence receipt](results/m8-format-inventory-2026-11-07.json).
