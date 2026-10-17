# M8: block-compressed format foundation

Date: 2026-10-17. **Implemented Vulkan format/copy/view support with bounded BC
execution evidence.** Metal implementation is deferred until a Mac is available;
this is the first Linux M8 tranche, not completion of advanced graphics.

## Contract

The catalog adds 54 IDs to the previous 118: 16 BC1–7, 10 ETC2/EAC and 28 2D ASTC
LDR formats. Existing IDs stay unchanged. C and native Rust expose the same names
and metadata. ASTC HDR/3D, vendor compression and plane-aware formats are not
included. Query exact native format/usage/image support; an ID is not a support
promise and there is no fallback decoder, transcoder or RGBA expansion.

`ogpu_next_format_info` / `FormatInfo` now includes `copy_block_extent[3]` and
`copy_block_bytes[3]`. The latter gives independent COLOR/DEPTH/STENCIL copy sizes,
zero for absent aspects. This describes copy footprints, not native optimal-image
allocation sizes. Ordinary formats use a 1x1x1 extent; compressed copies count
whole blocks, including at partial mip edges. The mutable revision-1 record grows
from 32 to 56 bytes. Rebuild consumers against matching headers/runtime/Rust source;
old query element sizes reject without writing output. No legacy ABI20 change.

Per-format queries suffice; OGPU does not add coarse compression feature enables.
Vulkan explicitly permits querying individual formats when the all-family
compression feature is not enabled. See
[native feature semantics](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceFeatures.html).

Runtime changes:

- Memory/image copy footprints round texel extents up to block counts. Byte row
  pitches count block rows; byte slice pitches count block slices. Native pitch
  fields are converted back to texel units with checked arithmetic.
- Offsets must be block-aligned. Nonmultiple extents are legal only at the mip
  edge, including mips smaller than one block. Queue transfer granularity is scaled
  by block extent; zero granularity still means whole-mip copies. Address alignment
  uses block bytes. See [queue granularity](https://docs.vulkan.org/refpages/latest/refpages/source/VkQueueFamilyProperties.html)
  and [copy addressing](https://docs.vulkan.org/spec/latest/chapters/copies.html).
- Image copies accept equal block sizes and equal block extents, preserving raw
  bits; view compatibility instead uses the native format compatibility class.
  Equal byte sizes alone do not make BC1 and BC4 compatible views. See
  [format definitions and classes](https://docs.vulkan.org/spec/latest/chapters/formats.html).
- Compressed clears reject before native encoding, as required by the
  [native clear contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdClearColorImage.html).
  Native sampled-image and explicit blit behavior continue to depend on queried
  capabilities; no shader specialization or conversion is inserted by OGPU.

All checks use immutable metadata and local arithmetic. No recording-time format
query, allocation, resource retention, synchronization or extra native operation
is introduced to implement compression. Native sampling does the decode. This is
a structural mapping statement, not a newly measured performance claim.

## Verification

The public-C fixture now includes `foundation_compressed.h` and a reproducible
Slang sampling shader. It enumerates all 54 compressed formats and requests the
exact sampled + copy-source + copy-destination, mutable, 2D-array profile.

| Check | Radeon RX 5700 XT / RADV | llvmpipe |
|---|---:|---:|
| Compressed formats with copy/view execution | 16 BC | 16 BC |
| Requested profiles unsupported | 38 ETC2/EAC + ASTC | 38 ETC2/EAC + ASTC |
| BC1 RGBA UNORM/sRGB sampled profiles | 2 | 2 |
| Rejected malformed copy / unsupported clear recordings | 12 | 12 |

For each executed format: three mips, two array layers, odd base extents, padded
host rows/slices, exact upload → image copy → readback bytes and untouched guards.
Image copies include an aligned interior and partial edge region. Views use both
members of each native compatibility pair. BC1 samples every pixel in both layers
through per-mip views, using valid encoded red/green blocks that change by layer
and mip. Endpoint colors do not test intermediate sRGB transfer accuracy or
filter precision. Other BC families have copy/view evidence, not shader decoding
or numerical acceptance. ETC2/EAC and ASTC remain locally execution-unverified.

An initial run reported synchronization hazards between the fixture's split image
copy writes. The fixture now explicitly orders those writes with a caller-issued
COPY-write dependency. The runtime does not add it. Final runs are validation-clean;
this is not evidence of independent parallel writes to distinct compressed blocks,
nor a diagnosis of whether the initial report reflected native or validator limits.

Further checks pass:

- 60 host unit tests and two native-Rust host integration checks.
- A pinned-registry test cross-checks all 172 formats' block extents, color block
  sizes and compatibility-class equivalence; added to CI (no hosted result claimed).
- 19 existing GPU unit tests per driver and the full public-C fixture on both.
  llvmpipe retains its explicit pre-existing combined depth/stencil plane-readback
  exclusion; it is not a new compression exclusion or full depth/stencil acceptance.
- C/Rust ABI comparison: 1,130 layout values, including the extended format record.
- Strict Clippy, formatting, C/C++ syntax checks and pinned Slang 2026.14.1 shader
  reproduction/validation. Local Nix syntax checks suppress only the wrapper's
  unused linker-argument warning.

Local logs: `/tmp/ogpu-m8-compressed-radeon.log`,
`/tmp/ogpu-m8-compressed-llvmpipe.log`, `/tmp/ogpu-m8-unit-radeon.log` and
`/tmp/ogpu-m8-unit-llvmpipe.log`. Environment is the existing Rust 1.98.1, explicit
ICD selection, Vulkan loader/Khronos synchronization validation 1.4.357, RADV 26.2.4
and llvmpipe 26.2.1. Reproduce with `cargo xtask foundation`,
`cargo test -p ogpu --no-default-features`, `cargo xtask abi`,
`cargo xtask foundation-shaders --check` and `python3 -B tools/test_format_catalog.py`.
The llvmpipe full fixture explicitly sets `OGPU_FOUNDATION_STENCIL_RASTER_ONLY=1`.

## Remaining coverage

Cross-block-shape image copies, compressed/uncompressed block reinterpretation
views, ASTC HDR/3D, 3D compressed execution, transfer-only native queue execution,
compressed blit consumers and broader sampling/filter/numerical fixtures remain.
These are explicit scope limits, not rejected design directions. No new package
release, independently adopted consumer, Metal or cross-vendor physical result is
claimed. Next is M8's advanced raster/task-mesh and GPU-generated command tranche;
missing hardware must remain distinct from unimplemented API coverage.
