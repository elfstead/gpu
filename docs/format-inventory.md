# Pinned format inventory

M11, 2026-11-07. The catalog now exposes 202 formats: 134 ordinary/packed and
68 compressed. M8 established the preceding 190-format inventory. IDs 173–190 add A1B5G5R5, alpha-only A8, both A4-first 4444
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

## 64-bit component tranche

IDs 191–202 add UINT/SINT/FLOAT variants of R64, RG64, RGB64 and RGBA64.
Native view-compatibility classes remain byte-width classes, including new 192-
and 256-bit classes. All existing image/transfer/view/vertex declarations can name
them; per-format capabilities and exact image-profile queries decide usability.
No arithmetic feature is implicitly enabled, and byte copies do not require shader
FP64/Int64 merely because the storage format has 64-bit components. Typed shader
access and 64-bit vertex inputs retain their independent executable requirements
and execution acceptance in M11's compute/shader package.

The existing clear payload retains its ABI. For native 64-bit component clears,
each pair of 32-bit words carries the representation of one 64-bit R/G value.
The header specifies copying scalar bits into that payload rather than converting
them to 32-bit numeric values.
[Native clear semantics](https://docs.vulkan.org/refpages/latest/refpages/source/VkClearColorValue.html).

Local RADV executes R64_UINT, R64_SINT and R64_FLOAT uploads, padded two-layer
readback and full-width clears, checking every payload/padding/guard byte. The
remaining nine wider image descriptions are natively unsupported and excluded;
that does not imply their vertex-buffer interpretation is unsupported. The whole
ordinary matrix now executes 74 and excludes 60 profiles. No shader arithmetic,
filtered sampling or typed-image/vertex execution acceptance is inferred from
these transfer/clear results. [Receipt](results/m11-component64-2026-11-07.json).

## Remaining definitions and schedule

| Family | Count | Disposition |
|---|---:|---|
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

## Earlier M8 evidence

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
