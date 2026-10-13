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
checks and incomplete failures. Clean commit `8cb2253` subsequently passes all 54
frames on Radeon (including 720p) and 36 on llvmpipe, plus four failure cases each.
Local receipts are `target/graphics-scene/textured-f9xq681g/report.json` (Radeon)
and `textured-79swqvea/report.json` (llvmpipe); both record `dirty=false` and
`complete=true`. GCC and Clang strict C checks and all current scene host tests
also pass. This is not installed-SDK acceptance or a published release.

The runner now also builds an independent direct-Vulkan consumer. It extracts
the exact graphics SPIR-V embedded in the generated public pair and uses the
checked compute artifact. Native recording creates its own mipmapped image,
descriptor heaps, sampler/image descriptors, blend/depth pipelines and copies;
it never calls or links OGPU. Both outputs undergo the CPU/geometry/upload gates,
then a complete native/public comparison that includes the oracle's excluded
edge/filter-tie bands. Comparison permits the predeclared two-byte color and
1e-6 depth limits, and separately records whether every output byte is identical.
Malformed extents, guard corruption, nonfinite depth and differences beyond the
tolerances are covered by negative host tests.

The inherited native loader/allocation fixture is shared only with other direct
controls. Its generated legacy host data declarations do not call OGPU. Native
color/depth attachments stay in GENERAL layout; foundation uses explicit
attachment/copy layouts. Descriptor reserved ranges obey queried native alignment,
not an assumed correspondence with heap-base alignment. These paths test output
correctness with the same shaders/draw/state/texture inputs, not identical driver
call costs or throughput. The earlier foundation-only 720p result is not
native-matched 720p evidence; clean integrated acceptance remains pending.

Initial native-matched runs pass both 257x193 and 640x360 on Radeon and llvmpipe:
36 frames per path/driver, both independently accepted, with every image byte
identical across paths and exact geometry/texture bytes. All four foundation
failure controls still pass. These development receipts are dirty-source results;
clean matching and the independent package remain separate checkpoints.

## Independent SDK source handoff

The default SDK now includes `share/ogpu/examples/foundation-textured-scene` with
the public C caller, generated SPIR-V/interface header, shader sources and shared
independent output checks. Copy the whole directory outside the checkout, set
`PKG_CONFIG_PATH` to the installed SDK, then run `python3 build.py` and
`python3 run.py`. C11/Python/pkg-config suffice; no checkout, Cargo, Slang or Vulkan
headers are required. Runs record SDK/runtime/header/source/executable identity,
device, complete/incomplete state and all 36 frame checks. `--scale` adds 720p.

The maintainer test relocates this sixth example, checks its compiler dependency
graph, imported symbols and resolved runtime, and requires a missing-loader error
to leave an incomplete receipt with no accepted runs. `--shader-check` uses the
installed graphics compiler helper to check the supplied header, reorder all root
fields in both stages, reject the stale interface, regenerate and rebuild unchanged
host code. The repeated outputs must remain byte-identical. This establishes a
typed-interface/source-package boundary, not portable binaries or third-party adoption.
The development package passes all six independent consumers and the textured
root-regeneration/output-equivalence check on Radeon and llvmpipe, with validation
and synchronization validation enabled. Host-only receipt tests also reject
missing/duplicate identities, incomplete frame logs, validation failures and oracle
failures; a failed second extent preserves the first result but never marks the
whole run complete. These are dirty-source checks pending a clean package receipt.
Presentation remains a separate platform-boundary task; Metal is not inferred
from Vulkan execution. This consumer does not establish implicit-LOD derivatives,
anisotropic quality, sRGB blending, compressed texture support or streaming residency.

## Clean M4 offscreen acceptance

Completed 2026-10-13 at `43bb9f023d18620aad40edf58ca87a2cec44ff6a`, before
M5 changes. Both native-matched reports record `dirty=false`, `complete=true`:

- Radeon: `target/graphics-scene/textured-ke0pvzj_/report.json`, 54 frames per
  path at 257x193, 640x360 and 1280x720.
- llvmpipe: `target/graphics-scene/textured-oi_6ysc_/report.json`, 36 frames per
  path at the first two extents.

Every native/foundation image byte agrees, including CPU-oracle-excluded edges;
geometry, texture bytes and all guards agree exactly. Independent CPU checks and
all four foundation failure controls pass on both drivers. No threshold changed.

The SDK built from the same clean commit at `/tmp/ogpu-textured-sdk-clean` passes
all six relocated consumers on both drivers, including unchanged learned-image
results and indexed/range reset/replay checks. The textured example passes 36
frames before and after shader-root reordering with unchanged C host code and
identical output bytes. Runtime/header identity and isolated compiler dependencies
are checked; missing-loader failure remains incomplete. Local full SDK logs:
`/tmp/ogpu-textured-sdk-clean-radv.log` and `...-lvp.log`. These paths are local
evidence, not downloadable artifacts; the commands above reproduce the scene and
`tools/test-foundation-install.py --prefix ... --shader-check` reproduces the handoff.

Fresh foundation tests pass 40 ordinary foundation-specific tests and all 16 GPU
tests per driver, covering malformed image/view/descriptor/executable descriptions,
atomic rejection, ownership and failure cleanup. Vulkan and synchronization
validation are enabled throughout acceptance; no diagnostic is accepted as success.

Decision: retain the existing explicit foundation shape for this workload. The
consumer adds no runtime API or hidden scheduling, waits, texture allocation or
descriptor registry. Native indexed draws, depth/blend/filtering, GPU-generated
inputs and caller-selected reuse remain expressible. This closes the bounded M4
offscreen consumer, not general Vulkan performance parity, a new timing campaign,
the known unrelated llvmpipe counted-to-fixed/D32S8 failures, presentation or Metal.
Move to the [M5 transformer and quantized-linear consumer](transformer-consumer-plan.md).
