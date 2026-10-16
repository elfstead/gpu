# Typed and formatless storage-image contract

Date: 2026-10-16. Follow-up to the 118-format catalog at `5141374`.
This completes the pulled-forward ordinary storage-image metadata batch, not M8
or every storage-image shader profile. P1 packaging/human acceptance remains next.

## Native contract correction

The previous format receipt and proposed next step incorrectly treated extended
and formatless image capabilities as missing device enables. That would add
artificial coarse gates to the current Vulkan baseline:

- `shaderStorageImageExtendedFormats` guarantees a set of physical format support;
  enabling it has no operational effect. Individual format queries remain valid.
  See [the feature definition](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceFeatures.html).
- SPIR-V `StorageImageExtendedFormats` is allowed by Vulkan 1.0, and
  `StorageImageReadWithoutFormat` / `StorageImageWriteWithoutFormat` are allowed
  by Vulkan 1.3 (also by older feature/extension paths). Our modern baseline needs
  no extra toggle for declaring them. See [the capability table](https://docs.vulkan.org/spec/latest/appendices/spirvenv.html).
- Formatless operations still need the corresponding native per-format
  `STORAGE_READ_WITHOUT_FORMAT` / `STORAGE_WRITE_WITHOUT_FORMAT` permissions,
  compatible numeric class and native texel component rules. This does not make
  arbitrary storage views compatible. See [format capabilities](https://docs.vulkan.org/spec/latest/chapters/formats.html).

Discovery now chains `VkFormatProperties3` through
`vkGetPhysicalDeviceFormatProperties2`; truncating to the old 32-bit mask would
lose the native write-without-format bit. C exposes bits 2048/4096 in the existing
64-bit optimal/linear masks, and native Rust exposes matching constants in
`types::format_features`. Existing bit values and record layouts are unchanged.
Image-profile/placement queries remain separate. No hot-path parsing, descriptor
scanning, allocation, conversion or synchronization policy was added.

## Offline compiler boundary

The pinned Slang 2026.14.1 adapter accepts 39 typed SPIR-V storage formats with
32-bit shader scalars (including normalized/packed views), plus Unknown-format
float/signed/unsigned images in its existing non-MS, non-array 2D heap subset.
It reports shader format identity and per-kind format-feature unions separately
from executable device requirements. Legacy output rejects the new kinds and
capabilities explicitly; the existing legacy heap headers still reproduce.

Slang declares both formatless capabilities even for one-direction use. The
adapter checks actual image reads/writes to avoid imposing write permission on
read-only usage, or vice versa. Storage descriptors must flow directly into those
operations in this bounded adapter: unexamined calls/phi/copies/texel pointers
fail closed. In particular, typed image atomics cannot silently bypass their own
format requirement. This is tooling scope, not a new runtime API restriction.

The generated `compatible()` predicate checks only device/static limits. The
caller separately selects and checks view formats, extents, descriptors, indices,
lifetimes and dependencies. Per-kind unions are not per-index dataflow analysis;
different indices of one kind may have narrower individual needs than the union.

## Verification

Run the public-only fixture with the usual Vulkan loader/layer environment:

```sh
python3 -B examples/compiler/storage_workflow.py --check
python3 -B examples/compiler/storage_workflow.py
```

`--check` compiles all device artifacts, validates Vulkan 1.4 SPIR-V, checks
metadata, builds strict C11 consumers and runs six compiler/negative tests.
The workflow creates generated artifacts only under `target/compiler-storage`.
The vocabulary test additionally compiles all 39 typed shader formats; this is
not an execution claim for all corresponding native view formats.

| Native view | Shader scalar | Typed, unknown-read, unknown-write, both unknown |
|---|---|---|
| RGBA8_UNORM | float | 4/4 on Radeon; 4/4 on llvmpipe |
| RGBA8_SINT | signed integer | 4/4 on Radeon; 4/4 on llvmpipe |
| RGBA8_UINT | unsigned integer | 4/4 on Radeon; 4/4 on llvmpipe |
| RG32_FLOAT | float | 4/4 on Radeon; 4/4 on llvmpipe |

All 16 variants per driver executed; none were excluded. Each uses **zero optional
device enables**, explicit backing/native descriptors/barriers, two changed-input
serial replays, exact read/transform/write/copy-back results and host guards.
Capability/profile exclusions are printed separately and never counted as execution.
The drivers were RADV on RX 5700 XT and Mesa llvmpipe 26.2.1, with Khronos
1.4.357 synchronization validation. This is one physical GPU plus software,
not cross-vendor or Metal evidence, and not a performance comparison.

Regression checks: 59 foundation host unit tests and two host integration tests;
19 existing GPU unit tests per driver; full public-C foundation fixture on Radeon;
11 existing heap-generator tests and original/reordered header reproduction/build;
six existing foundation-generator tests/build; 16 legacy-generator and 14
transitive-build-input tests plus original/reordered builds; strict Clippy, formatting,
reproducible pinned bindings and 1,122 C/Rust ABI layout values. The first full C
run correctly caught its obsolete 11-bit format-mask expectation; it now accepts
the 13 defined bits and passes. The checked dependency receipt now records the
new adapter hashes; its shader inputs/artifact did not change. Existing llvmpipe combined depth/stencil-copy
limitations are unchanged and were not reclassified by this work.

Local logs: `/tmp/ogpu-storage-{host,radeon,llvmpipe}.log`,
`/tmp/ogpu-storage-gpu-{radeon,llvmpipe}.log`,
`/tmp/ogpu-storage-foundation-radeon.log`, `/tmp/ogpu-storage-rust-host.log`,
`/tmp/ogpu-storage-heaps-regression.log`, `/tmp/ogpu-storage-foundation-compiler.log`.

## Still open

The 2D read/write fixture does not prove every typed format, sampled integer
operation, array/3D/MS profile, image atomic, raster-stage storage write or
64-bit-image capability. Runtime support outside the adapter's subset must carry
its own correct artifact/requirements contract. Compressed textures and advanced
graphics remain M8; plane-aware and 64-bit typed profiles remain separately
scheduled. No permanent exclusion or silent fallback is implied.
