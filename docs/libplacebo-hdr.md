# Bounded HDR-to-SDR image processing

Selected 2026-09-15. Extend the existing libplacebo consumer toward useful HDR
processing, not a general media player/backend. Keep upstream revision
`3330a515d62139259c26239014f286e233bd3a5c`, shader generation/math and native Vulkan
reference. No additional GPU, video decoding, presentation, dynamic peak detection,
general color-management framework or FP16 shader arithmetic is required.

## Workload fixed before implementation

Input is opaque, scene-linear BT.2020 RGB in RGBA16F, normalized to libplacebo's
SDR reference white, with explicit 1000-nit peak metadata. Generate deterministic
finite fixtures with bright/neutral/color gradients and values above 1. Resize
through upstream EWA Lanczos compute into RGBA16F, then nearest-sample and map
to BT.709/sRGB using upstream spline tone mapping, explicit clipping gamut mapping,
static metadata and no contrast recovery or dithering. Output is RGBA8.
This is a declared conversion, not an image-quality comparison between tone mappers.

Start with a native-only capture of actual image formats/usages, shaders,
specialization constants, roots and dispatches. Record requirements before changing
the runtime. Prefer exact support queries and one coherent format extension over
a general format catalog or speculative rendering-state API.

## Acceptance and stopping condition

- Three inputs (16x16, 31x17, 64x33), odd output extents (2w+1, 2h+1), three A/B/A
  frames each. Preserve high-dynamic-range values in the FP16 intermediate;
  confirm some components exceed 1, all are finite, and alpha remains 1.
- Native and OGPU intermediate components: absolute difference <=0.005 plus
  relative difference <=0.005*abs(reference), combined as a single error bound.
  Final RGB differs by <=2/255; alpha exactly 255. Repeat A exactly on each backend,
  require B to differ, and exercise deterministic repeated resource reuse.
- Use identical generated input bytes and upstream processing on both backends.
  Add checked-in known-value half-conversion tests and meaningful luminance ramp
  checks; the native reference alone is not independent tone-mapper mathematics.
- Validate on RADV and llvmpipe. Preserve all existing ABI/runtime and SDR consumer
  tests; cover transfer sizing, requirements masks, unsupported usages and cleanup.
  CPU half conversion is fixture/verification work, not hidden GPU fallback.
- Stop with the working bounded consumer, required public contracts and a
  retain/revise decision. No timing target, general HDR support claim or automatic
  follow-up benchmark. If captured upstream requirements materially exceed this
  scope, report the concrete mismatch before expanding it.

Initial status: native capture is the first gate; no runtime/API change yet.
