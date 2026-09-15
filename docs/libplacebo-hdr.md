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

## Native checkpoint: captured, acceptance not passed

2026-09-15: native-only capture is complete on RX 5700 XT/RADV and llvmpipe.
No runtime/API or existing adapter change has been made. The runner intentionally
returns **2**, because the original exact intermediate-alpha gate fails. See the
[execution receipt](results/libplacebo-hdr-native-2026-09-15.txt).

Both drivers execute nine compute and nine raster passes, with six prepared
passes total. A/B/A intermediate and final bytes repeat exactly on each backend,
B differs, all intermediate values are finite, final alpha is exactly 255, and
neutral-ramp checks pass. Intermediate maxima range from 4.51953 to 4.84766.
No validation errors were logged with the validation layer and synchronization
validation enabled. These are native checks, not an OGPU comparison or complete
HDR acceptance.

Actual requirements, identical across these captures:

| Pass | Images | Parameters | Execution |
|---|---|---|---|
| EWA resize | sampled RGBA16F source, sampled 256-entry R32F 1D LUT, storage RGBA16F destination | 56 push bytes, 7 specialization constants | 32x32 local size; grids 2x2, 2x2, 5x3 |
| Color/tone mapping | sampled RGBA16F intermediate, sampled 256-entry R32F 1D LUT; RGBA8 target | 208 push bytes, 5 specialization constants | raster triangle strip |

Three important mismatches surfaced before implementation:

1. **Packed host representation:** upstream `rgba16hf` means eight-byte RGBA
   binary16 pixels. Its similarly named `rgba16f` has a sixteen-byte float32
   host representation and emulates transfers. The harness uses `rgba16hf`
   explicitly and checks its texel size/non-emulated property. No half-precision
   shader arithmetic is needed.
2. **Renderable compute destination:** pinned `src/dispatch.c` checks
   `target.renderable` unconditionally in `pl_dispatch_finish`, even when the
   selected shader is compute. The native intermediate therefore has both
   storage and color-attachment usage. The current OGPU image/raster contract
   only admits RGBA8 color targets. Silently advertising RGBA16F as renderable
   in the adapter without supporting it would be dishonest.
3. **Raster parameters:** the existing adapter accepts only parameter-free
   raster passes and adds an eight-byte vertex-address root. The captured
   208-byte block contains color matrices and scalar parameters. This is an
   adapter limitation, not proof of a missing public buffer abstraction. Its
   advertised 128-byte parameter limit also differs from both native devices'
   256-byte limit; generation under that constrained profile needs checking.

The failed alpha gate is separate: EWA float accumulation followed by binary16
storage produces 0.99951171875 at some otherwise opaque pixels. Maximum absolute
alpha error is 0.00048828125 on both drivers. Across all nine frames this affects
3,813 intermediate pixels on llvmpipe and 5,223 on RADV. Final alpha remains 255.
The original exact gate is retained in code; this is not silently relabeled PASS.

## Proposed next decision

Retain this consumer, but revise the initial "format-only" implementation estimate:

- Add RGBA16F image storage/sampling/transfers and a narrowly selected raster
  target format, with exact support queries and matching executable/target
  validation. Keep existing fixed raster state; do not add a general format catalog.
- Extend adapter raster parameter packing to coexist with the vertex address,
  preserving upstream member offsets and processing statements. Compare inline
  push data (checking the actual combined size against OGPU limits) with an
  addressed parameter block if that budget is insufficient. No automatic public
  uniform-buffer API is justified by this capture.
- Revise intermediate alpha acceptance to `abs(alpha - 1) <= 2^-10`, one binary16
  step above 1, while retaining exact final alpha and the original native/OGPU
  comparison tolerances. This acknowledges measured upstream rounding without
  changing shader math or forcing alpha to 1 in the harness.

This is the native-first decision checkpoint, not a completed implementation.
Resolve these choices before widening the runtime or adopting the revised gate.

## Reproduce the native gate

Use the dependencies and pinned unmodified checkout from the
[integration instructions](../integrations/libplacebo/README.md). The HDR fixture
additionally uses compiler `_Float16` support for CPU binary16 conversion, tested
with Clang 21.1.8; known-bit conversion checks run before Vulkan creation.

```sh
VK_DRIVER_FILES=/path/to/one/icd.json \
  VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 \
  bash integrations/libplacebo/run-hdr-reference.sh /path/to/pinned/libplacebo
```

Expect exit 2 for the currently recorded alpha mismatch, not success. Other
execution/check failures exit nonzero as well. The runner creates a fresh ignored
capture directory containing input bytes, intermediate/final images, generated
shaders, per-run push bytes, a requirement manifest and a log. Generated upstream
shader bodies are not checked into this MIT repository. Native uploads use an
explicit reusable HOST staging buffer, as in the existing native benchmark.
