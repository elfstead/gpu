# Ordinary and packed foundation formats — 2026-10-16

Requested prerequisite pulled forward from broader graphics coverage. **Implemented
format vocabulary and bounded operation evidence, not universal format/usage support.**

## Scope and contract

The public C header now declares 118 format IDs instead of 19. Existing IDs retain
their values. Coverage is the Vulkan 1.0 core uncompressed formats with component
widths through 32 bits: R/RG/RGB/RGBA integer, normalized, scaled and floating-point
families, BGR/BGRA and sRGB variants, packed 4/5/6/8/10-bit colors, packed unsigned
floats/shared exponent, and all seven core depth/stencil formats. This intentionally
includes native formats that a particular device may support only for vertex fetch,
or not support at all. Absence from an earlier consumer is not an exclusion policy.

Names ending in PACK8/16/32 specify packed-word bit order; other color names specify
component memory order. The catalog follows the native definitions and compatibility
classes in [Vulkan formats](https://docs.vulkan.org/spec/latest/chapters/formats.html).
These are OGPU IDs, not arbitrary backend enum passthrough. No format conversion,
component expansion, software decompression or shader rewrite is implicit.

`foundation::types::image_formats` exposes named Rust constants. One Rust catalog
supplies those IDs and native metadata; host checks compare the C declarations and
verify unique native round trips. Native lookup is a static indexed table. Discovery
and presentation use the complete catalog; unsupported native presentation formats
still remain visible with an unmapped OGPU ID. Discovery is cold-path allocation;
recording adds no allocation or format search.

Format features remain physical per-use capabilities, not a guarantee that any
shape, usage combination, shader capability or view is legal. Applications query
the count, allocate storage, inspect features, and query exact image profiles and
requirements. C fixture, M4 scene and libplacebo enumeration now use returned counts
instead of fixed 32/64-entry arrays. No record layout or function signature changed;
the experimental source/header/runtime still need to be revision-matched.

## Operation changes

- Blits distinguish float-valued, signed-integer, unsigned-integer and depth/stencil
  domains. Signed and unsigned integer blits cannot mix, integer filtering is nearest,
  and depth/stencil still requires identical formats and nearest filtering. See the
  [native blit rules](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBlitImage.html).
- All integer color attachments select the integer sample-count mask, not just R32_UINT.
- Graphics `depth_format` describes whichever depth/stencil aspects the selected
  format actually has. S8 declares only the native stencil attachment format and
  stencil sample mask; depth test/write with S8 rejects. Stencil-only clears do not
  validate the unused depth value. Combined-format transitions still require both
  aspects until separate depth/stencil layout enabling is implemented.
- Copies use actual color texel sizes and per-aspect depth/stencil byte sizes,
  including three-/six-byte RGB and D16S8 depth. Transfer-only queue address checks
  intersect texel alignment with the four-byte requirement; taking their maximum
  was insufficient for non-power-of-two texel sizes. See
  [address/image copy alignment](https://docs.vulkan.org/refpages/latest/refpages/source/VkCopyDeviceMemoryImageInfoKHR.html)
  and [queue restrictions](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdCopyMemoryToImageKHR.html).

## Verification

Same Radeon RX 5700 XT/RADV 26.2.4 and llvmpipe 26.2.1, loader/Khronos synchronization
validation 1.4.357 and pinned Slang 2026.14.1 as the prior checkpoints.

| Public-C format matrix | Radeon | llvmpipe |
|---|---:|---:|
| Catalog entries queried | 118 | 118 |
| Executed image profiles | 67 | 69 |
| Exact requested image profile unsupported | 51 | 47 |
| Explicitly unverified combined depth/stencil profiles | 0 | 2 |
| Standalone S8 clear/copy/readback | Pass | Pass |

The matrix checks creation/dedicated placement, an ordinary view when its requested
usage is available, padded rows/slices across two array layers, exact color upload/
readback bytes, depth/stencil clear/readback, and unchanged host inputs/padding/guards.
Undefined padding in 24-bit depth output is not compared. Unsupported counts describe
the matrix's exact copy-plus-view profile, not every possible use of those formats.
All attempted supported cases must pass; the fixture also requires at least 40
executed profiles and standalone S8, so all-skipped execution cannot pass.

The first Radeon test exposed COPY-versus-CLEAR stage mistakes in the new fixture;
its explicit barriers were corrected before acceptance. Final Radeon runs have no
validation/synchronization errors. llvmpipe's unrestricted matrix reproduced the
previous combined-plane problem: D24S8 stencil readback returned zero instead of
the clear value 90. This is not newly diagnosed as a driver bug by these checks.
The existing explicit `OGPU_FOUNDATION_STENCIL_RASTER_ONLY=1` diagnostic excludes
D24S8/D32S8 plane-copy checks, prints `FORMAT_UNVERIFIED`, and allows remaining
coverage to proceed. Their capability bits are not rewritten and default testing
still fails on the observed mismatch. D16S8's requested image profile is unsupported
on both devices. Standalone S8 is not excluded.

Further checks:

- 58 host unit tests and two host integration checks pass, including ID parity,
  metadata, numeric blit rejection and non-power-of-two address alignment.
- Both drivers pass 19 headless GPU unit tests, native Rust consumer execution and
  three live Wayland integration tests (including pixel/no-readback scene modes).
  Graphics preparation now exercises native S8-only pipelines and rejects S8 depth testing.
- Full public-C fixture passes on Radeon. llvmpipe passes with the explicit
  combined-plane diagnostic exclusion above, not full combined-plane acceptance.
- Radeon M4 native-matched textured scene passes at 257x193 and 640x360, with
  18 frames per extent and exact comparisons.
- Rebuilt Radeon libplacebo SDR A/B/A and both 36-frame scheduling modes pass,
  including exact native-reference comparisons. No expansion of that adapter's
  own published five-format subset is claimed.
- Strict foundation Clippy, formatting and C/Rust ABI checks (1,122 layout values)
  pass. Final accepted GPU runs have no native validation errors.

Reproduce the matrix with `cargo xtask foundation`, selecting one ICD and enabling
Khronos synchronization validation. Run `cargo test -p ogpu --no-default-features`
for host checks and add `-- --ignored --nocapture` for GPU/Wayland integration.
The diagnostic llvmpipe invocation must explicitly set the environment variable
above; it is not a default runtime fallback.

## Remaining scope and proof

Transfer/view acceptance does not prove sampling, every integer/packed render
output, blending, vertex fetch, storage shaders or multisample use for every new
format. Those uses still obey exact capabilities and shader requirements.
Correction from the [storage-image follow-up](foundation-storage-images-2026-10-16.md):
the initial receipt incorrectly described extended/formatless shader capabilities
as missing device enables. On our Vulkan baseline no such extra enables are needed.
Typed storage needs the corresponding STORAGE format support; formatless reads and
writes additionally need their independent per-format flags, now exposed by that
follow-up alongside matching compiler metadata/artifacts. No new performance or
Metal execution claim.

The [roadmap](../roadmap.md#scheduled-hardware-coverage) now makes the remaining
batches explicit: M8 block compression (BC, ETC2/EAC, ASTC LDR), including block-aware
copies/views and sampling; extension-specific raster formats in M8; plane-aware
formats and 64-bit-component typed profiles in M11. Plane-aware work may move earlier
without waiting for codecs. Missing local hardware leaves native acceptance open;
it does not justify a silent fallback or permanent format omission.
