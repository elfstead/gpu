# M4 integrated textured scene

Selected 2026-10-13. This is the next consumer integration, not another API-design
or timing gate. The existing foundation exposes the necessary image views,
descriptor ranges, filtering, graphics state and explicit lifetimes. No runtime
API addition is proposed. The old indexed/depth and range controls remain intact.

## Workload and acceptance fixed before execution

Reuse the indexed scene's GPU-generated two quads, index/indirect records and A/B/A
geometry changes. Render textured far/near opaque surfaces with LESS depth testing
and writes, then draw the far quad again as a transparent overlay with depth
testing/writes disabled and its own central scissor. The overlay uses straight
alpha: RGB SRC_ALPHA / ONE_MINUS_SRC_ALPHA, alpha ONE / ONE_MINUS_SRC_ALPHA, ADD.
This keeps depth from the opaque pass and final alpha one, including clear pixels.

Upload an explicit 8x8, 4x4, 2x2, 1x1 RGBA8 mip chain. Each level has a distinct
spatial pattern and changes with phase; no runtime mip generation. Use an RGBA8
color target and D32 depth. Interpolated UV and flat surface identity cross the
stage interface. Near-surface tint distinguishes the two indexed surfaces.
Both shader stages deliberately share one 24-byte inline argument bank: vertex
address, image/sampler descriptor offsets, explicit LOD and overlay selector.

Six cases cover nearest level 0, nearest level 1, a view starting at mip 1 with
relative LOD 0, fractional linear mip filtering, inset viewport/scissor, and
fractional filtering through the restricted mip view with inset state. The exact
cases, viewport/scissor coordinates and independent pixel equations live in
`examples/graphics_scene/textured_oracle.py`. Dimensions start at 257x193 and
640x360, with 1280x720 as the larger acceptance case. Each uses A/B/A input data.

The implementation must preserve explicit upload-to-sample, compute-to-indirect/
vertex, attachment-to-readback and mapped-host dependencies. One serial arena and
timeline are sufficient for this consumer; no implicit wait, resource retention,
descriptor registry, texture allocator or workload scheduler enters the runtime.
Use generated shader interfaces and compare the exact same SPIR-V in direct Vulkan.

Acceptance gates:

- Independent CPU image oracle: <=2/255 absolute color error per channel and
  <=1e-6 depth error; reject nonfinite depth. This allows RGBA8 sample/filter/tint/
  blend rounding, not arbitrary visual similarity. At least 85% of all pixels are
  checked; omit only declared polygon-edge and nearest-filter tie bands.
- Exact geometry/index/indirect output and untouched upload/readback guards.
- View-relative mip equivalence for cases 1/2 and repeat equivalence for A frames.
- Same-driver direct-Vulkan comparison using identical shaders/work/state,
  with the same predeclared numerical limits; no benchmark campaign required.
- Changed texture bytes and descriptors are visible only after prior completion;
  no pending mutation/free. Recording failure and unknown-completion teardown
  follow the existing fail-stop checks.
- Missing profile/format support rejects explicitly, not silently substitutes a
  texture format, filtering mode or CPU path. Require the queried linear-filter
  support for the linear cases.

## Working status

The foundation consumer now implements all six cases with generated graphics
interfaces and checked compute artifacts. Run:

```sh
python3 -B examples/graphics_scene/textured.py --check  # build/oracle only
python3 -B examples/graphics_scene/textured.py          # selected ICD + validation
python3 -B examples/graphics_scene/textured.py --scale  # also 1280x720
```

Both local drivers pass 18 frames at each initial extent (257x193 and 640x360),
with the frozen independent image oracle, exact geometry/upload/readback guards,
view-relative mip equivalence and A/B/A repeat checks. Radeon also passes all 18
frames at 1280x720. Four link-interposed failure
cases pass on both: end/submit rejection safely frees all 12 backing allocations;
unknown submission acceptance or failed wait exits without teardown. Initial
upload really completes before injection; no successful wait is fabricated.
The oracle and receipt tests reject corrupted outputs, incomplete files,
nonfinite depth and unsafe cleanup. CI covers the host-only tests. The shader pair
is generated per build; no runtime reflection or shader rewriting is involved.

The consumer reuses the existing scene's setup/allocation/lifetime helpers, not
its workload recording path. Its two prepared raster states select opaque depth
and transparent overlay; textures, views, heap placement, descriptors, transitions
and per-frame retirement are caller-owned. No new runtime/API code is needed.
Receipts identify source/generated/runtime/executable hashes, driver, per-case
checks and incomplete failures. These first runs use a dirty implementation tree;
they are not clean-release or installed-SDK receipts.

Next add the direct-Vulkan rendering of these exact shader artifacts and states,
then export an independent SDK source handoff. Neither is established by the CPU
oracle or by the older untextured native comparison. M4 remains open until the
integrated native comparison and remaining acceptance gates pass.
Presentation remains a separate platform-boundary task; Metal is not inferred
from Vulkan execution. This consumer does not establish implicit-LOD derivatives,
anisotropic quality, sRGB blending, compressed texture support or streaming residency.
