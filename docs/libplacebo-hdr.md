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
  confirm some components exceed 1, all are finite, and alpha remains 1 within
  `2^-10` (the explicitly accepted revision after the native checkpoint below).
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

2026-09-15 at `77ca4c5`: native-only capture completed on RX 5700 XT/RADV and llvmpipe.
At that revision no runtime/API or existing adapter change had been made. The runner
returned **2**, because the original exact intermediate-alpha gate failed. See the
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
The original exact-gate failure is preserved in that commit and receipt. The user
subsequently accepted a numerical tolerance; the historical result is not relabeled PASS.

## Completed implementation and retain/revise decision

The user accepted these revisions. ABI 11 now adds `OGPU_FORMAT_RGBA16_FLOAT`
and a target-format argument to raster creation. Images retain their exact support
queries; draws validate executable/target format equality. Transfers use eight
bytes per RGBA16F texel and require eight-byte offsets. No FP16 arithmetic feature,
buffer-placement change, or additional raster state is introduced.

Runtime checkpoint: 30 ordinary tests, 745 ABI layout checks, clippy, and 21 GPU
tests on each driver pass. The added GPU test checks packed HDR/negative/alpha
values through offset transfers with guards, range/alignment rejection, and actual
RGBA16F raster output above 1 and below 0. Rebuild `hdr.frag.spv` with
`glslc --target-env=vulkan1.4 examples/shaders/hdr.frag -o examples/shaders/hdr.frag.spv`.

The adapter now keeps the upstream raster parameter block at its original offsets
and appends the vertex-buffer address at the next eight-byte boundary. This workload
uses 208+8=216 push bytes. It advertises at most 240 upstream bytes, reserving the
address within the queried OGPU limit, and rejects unsupported root declarations
or metadata. Both stage interfaces and processing statements remain upstream-owned.
Tests cover preserved offsets/body, overlapping or wrong-size members, unsupported
types, duplicate blocks and unsupported image declarations. Inline data fits on
both drivers, so an addressed parameter allocation or public uniform-buffer API
was not needed. Lifetime/retirement and frame scheduling are unchanged.

An additional capability dependency emerged during comparison: pinned upstream
`src/shaders/colorspace.c` queries a linearly filterable RGBA16 **UNORM** format,
then unconditionally switches to saturation gamut mapping if it is absent, even
for the selected clipping path that needs no 3D LUT. Before the fix, intermediates
matched but a final colored pixel differed by 11/255; captured color matrices
proved generation had selected different processing, not merely rounded differently.

`OGPU_FORMAT_RGBA16_UNORM` therefore adds genuine 1D/2D sampled/transfer support,
eight-byte texels and offsets; storage/color usages are intentionally not admitted
in this bounded profile. The adapter checks exact support before advertising it.
A dedicated test uploads/downloads packed UNORM16 values and linearly samples
black/white into black/mid-gray/white RGBA8 output, then checks cleanup. C-boundary
transfer/ownership tests now exercise all four image formats, both dimensions and
HOST/DEVICE sources. No fake capability, hidden CPU color conversion or upstream
patch is used. The accepted alpha bound is now explicit in both harness and comparator.

**Retain:** the existing buffer/address/heap/batch model, shared raster root data,
fixed raster state and the two evidenced image-format additions. **Revise:** the
RGBA8-only raster format assumption and the adapter's parameter-free raster shape.
No new scheduler, uniform-buffer object or general color-management API is justified.

On both RADV and llvmpipe all nine HDR A/B/A cases match native intermediate and
final pixels **exactly**, within the unchanged comparison tolerances. Per-run
upstream parameter bytes also match exactly, guarding the selected color policy.
Six passes are prepared, twelve completed-receipt reuses occur, and no child,
bank, pending operation, texture or staging allocation remains at shutdown.
The accepted bounded HDR milestone is complete; no timing or broader HDR claim.
See the [final receipt](results/libplacebo-hdr-2026-09-15.txt) for regression coverage.

## Reproduce acceptance

Use the dependencies and pinned unmodified checkout from the
[integration instructions](../integrations/libplacebo/README.md). The HDR fixture
additionally uses compiler `_Float16` support for CPU binary16 conversion, tested
with Clang 21.1.8; known-bit conversion checks run before Vulkan creation.

```sh
VK_DRIVER_FILES=/path/to/one/icd.json \
  VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 \
  bash integrations/libplacebo/run-hdr.sh /path/to/pinned/libplacebo
```

Expect exit 0. `run-hdr-reference.sh` remains available for native-only checks and
uses the accepted alpha tolerance too; reproduce the original failure at `77ca4c5`.
Execution/check failures exit nonzero. The runner creates fresh ignored
capture directory containing input bytes, intermediate/final images, generated
shaders, per-run push bytes, a requirement manifest and a log. Generated upstream
shader bodies are not checked into this MIT repository. Native uploads use an
explicit reusable HOST staging buffer, as in the existing native benchmark.
