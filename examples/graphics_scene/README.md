# M4 matched indexed/depth scene

The direct Vulkan control links only libc/libdl. Its ABI-19 public counterpart
includes only `ogpu.h` and standard C headers. Both use identical shader binaries,
allocation sizes, guarded GPU-generated data and scene modes. Existing native
loader, allocation, shader-module and drain helpers are reused only by the native
control; no runtime implementation is imported into either consumer.

Predeclared scene: two indexed quads (eight unique vertices, twelve index uses),
far opaque red at z=0.75 and near opaque green at z=0.25. Compute writes vertices,
indices and two 20-byte native indirect records into guarded DEVICE buffers.
Indices have two poison prefix entries; `firstIndex=2`, `vertexOffset=-1/+3`,
`firstInstance=0`. Vertex pulling uses the **native indexed vertex ID**, not a
non-indexed shader emulation of index lookup. The near rectangle shifts between
-0.125 and +0.125 on X in A/B/A frames.

Default indices are UINT32. `--index16` selects a compute variant that packs pairs
of UINT16 indices with ordinary uint arithmetic, without newly requiring shader
16-bit arithmetic/storage features. Native and public draws bind UINT16 and a
16-byte range instead of 32 bytes. Allocations stay the same size; the unused
16 bytes remain poison and are checked. Positions and indirect records are unchanged.

RGBA8 color + D32_SFLOAT depth, single sample/mip/layer, no culling/blending,
full-target viewport/scissor. Both exact image format/usage combinations must be
supported; no fallback format. The index buffer explicitly has INDEX_BUFFER usage:
[address binding still requires it](https://docs.vulkan.org/refpages/latest/refpages/source/VkBindIndexBuffer3InfoKHR.html).
The native [indexed indirect address command](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdDrawIndexedIndirect2KHR.html)
consumes GPU-written parameters without CPU readback between compute and draws.
Each original-scene indirect call has drawCount=1. The public ABI-19 graphics
profile enables multiDrawIndirect/count/draw-parameters; the stronger controls
below exercise those operations explicitly.

Before the first run, the modes and expected outcomes are fixed:

| Mode | State and expected overlap |
|---|---|
| 0 | LESS + depth writes, far then near: green |
| 1 | Same, near then far: green, full output identical to 0 |
| 2 | Depth test disabled, near then far: red; depth remains 1 |
| 3 | LESS, depth writes disabled, near then far: red; depth remains 1 |
| 4 | ALWAYS + writes, near then far: red; depth becomes 0.75 |
| 5 | LESS + writes, clear depth 0: black; depth remains 0 |
| 6 | Near pass clears both; far pass LOADs both: green, identical to 0 |
| 7 | Near pass clears both; far pass clears color but LOADs depth: black overlap, depth 0.25 |
| 8 | Near pass clears both; far pass LOADs color but clears depth: red overlap, depth 0.75 |
| 9 | Zero index counts: black, depth 1 |

Outside the rectangles color is black/alpha255 and depth is the selected clear.
The CPU analytic oracle checks every pixel except a one-pixel band around rectangle
edges (raster quantization is not assumed universal). Interior colors are exact;
depth tolerance is 1e-6 over binary32 readback. Full images including edges must be
byte-identical on A repeat, reversed-order modes 0/1 and split-pass mode 6. Geometry,
index/indirect payloads and every guard are also checked exactly. The CPU oracle
does not derive its expected image by rasterizing or copying shader code.

Use `python3 examples/graphics_scene/run.py --check` for build/oracle tests;
`run.py` for 257x193 and 640x360; `run.py --scale` also covers 1280x720 (Radeon).
Add `--public` to build/run both consumers. Acceptance requires each consumer to
pass the independent oracle, then byte-for-byte native/public equality of every
image and geometry file, including edges excluded by the analytic oracle and all
guards. Negative tests reject truncated, changed and missing comparison files.
Device identities must match throughout. Public roots contain GPU addresses;
there is no CPU readback between generation and drawing. Both consumers use fresh
recordings, explicit per-frame image discard and stable allocations. Public image
copies use the runtime's copy dependencies; explicit generation-to-draw and
between-scope dependencies remain visible in the consumer.
UINT16 GPU acceptance additionally requires `--reference32 PATH/TO/report.json`
pointing to a clean accepted matched UINT32 run at all requested extents on the
same driver. Every color/depth image byte must agree across index widths, after
checking the reference artifact hash and independently accepting the new geometry.
For example, run `run.py --public` first, then
`run.py --public --index16 --reference32 target/graphics-scene/matched-.../report.json`.
Use `--scale` on both runs to include 720p.
Select one ICD and enable Vulkan + synchronization validation. Pinned Slang
2026.14.1 and SPIRV-Tools compile native shaders. The installed compiler now also
checks the range consumer's vertex roots and draw identity. Fresh report directories retain shaders, full
color/depth/geometry output, sources/artifact hashes and logs. Failures stay incomplete.

This first control uses serialized fresh recordings and stable allocations; it is
not the later two-slot/replay or performance comparison. It does not measure vertex
cache efficiency or settle the public indexed/depth design. No invalid native GPU
addresses/indices are deliberately executed. Depth-format rejection is a setup
failure, not a successful skipped test. All accepted work drains before teardown.

## Stable scene reuse

`reuse.py` implements the first correctness increment from the
[performance brief](../../docs/indexed-depth-performance-plan.md), not timing.
It uses one shared device/queue and shared prepared executables, with one/two
independent slots. Each slot has fixed attachments and buffers plus either owned
reset/re-record storage or one serial immutable list. Compute reads phase/empty
flags through an immutable pointer to guarded HOST control storage. Each slot
cycles A/B/empty/A; only retired slots are read or mutated. No list-per-phase,
extra generation submission, CPU-generated geometry or successful queue-idle path.

The matrix is UINT16/UINT32 × modes 0/6/7/8 × one/two slots × reset/replay ×
native/public. These modes cover grouped draws and independent color/depth LOAD
preservation. Mode 8 hides movement in final pixels; the geometry must still change.
Native and public use identical reuse shader binaries and allocation-size/type
multisets. Slot resources live through drain and list destruction. All slots drain
before shared context or pointer-referenced allocations are destroyed on failure.

Provide accepted same-driver reports from both index widths:

```sh
python3 examples/graphics_scene/reuse.py --check
python3 examples/graphics_scene/reuse.py --reference32 PATH32/report.json --reference16 PATH16/report.json
# --preflight: 8 frames/configuration (development, not sustained acceptance)
# --software: 64 frames/configuration; default: 1,000 frames on Radeon
# --scale: also 1280x720, 8 frames/configuration (A/B/empty/A on each of two slots)
```

The runner rechecks reference hashes and analytically validates all 30 serial
reference frames at each selected extent/index width. Consumers then compare
**every retired image/geometry/guard byte** against the appropriate accepted
reference, plus the entire control buffer. The analytic expectations are cached
through these byte comparisons, not recomputed per repeated frame. Large output
frames are not dumped repeatedly; per-frame generation records and process logs
are retained. CPU tests reject corrupt inputs/outputs and false/incomplete reports.

A test-only loader traces actual recording/scope/indexed-draw/submission/reset/
pool calls and allocation lifetimes. Replay must have no hot recording or pool
changes; reset must have bounded pool creation, not per-frame allocation. All
native pools and buffer/image memory allocations must be released. Requested
bytes, traced allocation bytes and command-object counts are reported separately;
private driver command-memory size remains unknown. Two unretired receipts are
not a claim of physical GPU overlap. Keep validation/tracing out of future timing.

`reuse_failures.py --reuse-report PATH/TO/accepted-reuse/report.json` builds a
diagnostic public consumer with ASan/UBSan and explicit counting of its own C
allocations. A test-only interposer rejects HOST writes during first/second slot
setup, before the first frame and while the other slot has an accepted submission.
Reset and replay both include success controls. Expected failure exits must not
accept output, and every counted consumer/native allocation and command pool must
be released. This directly probes cleanup around the retained Clang leak warning;
it does not disable the warning or claim exhaustive allocation-failure coverage.
LeakSanitizer stays disabled; CPU counting covers these C sources, not all library
allocations. Fault injection never invents GPU completion or device loss.
## Native indexed frontier

`frontier.py` checks a stronger native Vulkan control before timing or public-API
decisions. It compares N single-record commands, one N-record command, and one
GPU-counted command at capacities 1/64/512. Each runs eight changing-input frames,
including zero, one, partial, full and above-capacity counts. Counted execution
keeps all records nonempty, so masking cannot conceal a failure to read the count.
Only the last record draws the near surface (except the one-record far-only case),
so full and partial counts visibly differ even at capacity 512.
Records and the count are generated by compute, then consumed in the same
submission; only final outputs are read back. The requested count originates in
host input: this is not a GPU culling workload.

With the same compiler/ICD/validation prerequisites as the scene runner:

```sh
python -B examples/graphics_scene/frontier.py --check
python -B examples/graphics_scene/frontier.py \
  --reference32 target/graphics-scene/matched-hx0vfkhq/report.json --scale
```

Use an accepted same-driver UINT32 reference containing the requested extents;
the path above is the local Radeon reference. Omit `--scale` for llvmpipe.
The runner revalidates the analytic references, compares all image bytes, checks
every generated vertex/index/record/count/guard byte, and verifies actual native
draw calls and allocation balance. It also reruns the original native ten-mode
scene as a regression control. Reports/logs/dumps stay in a fresh target directory.

This native executable opts into `multiDrawIndirect` and `drawIndirectCount`, with
a checked `maxDrawIndirectCount`. Ordinary native controls keep their original
profile; ABI-19 public graphics explicitly enables these features. All three frontier strategies enable the same
features and use equal allocation budgets. The scene has no shader-visible draw
ID; it does not establish a general fusion mapping. This initial probe uses one
serial slot and fresh per-frame command storage, **not the planned reset/replay
timing policy**. No performance or public API acceptance follows from it.

### Draw identity control

Add `--identity` to the same command to use a vertex shader reading `SV_DrawIndex`
and a fragment shader encoding draw identity/surface in exact RGBA8 bytes. The
diagnostic executable alone queries/enables `shaderDrawParameters`; baseline
executables keep their original feature profile. SPIR-V must contain both
`DrawParameters` and `BuiltIn DrawIndex` in this variant, and neither in the
ordinary vertex variant. Root/geometry/record/count/allocation shapes are unchanged.

The oracle recolors the analytically checked scene coverage and retains every
depth/guard byte. The runner also requires multi/count equality and single/multi
inequality in full-capacity frames at N=64/512, with equality in the one-record,
empty and partial cases. This is an intentional semantic counterexample, not a
failed matched-output benchmark. ABI 18 did not enable draw parameters; the probe
identified the semantic choice now exposed in ABI 19. No timing claim.

### Public ranges and matched storage/replay

`ranges.py --native PATH/report.json` executes the same nine identity-sensitive
cases through ABI 19 and checks complete output/budget equality. Its one-shot
storage is diagnostic, not a matched performance policy.

`range_reuse.py --native PATH/report.json` uses the same accepted, same-driver
257×193 identity report and unchanged native shader binaries for the next gate.
The existing native/public slot scheduler now has a range variant: capacities
1/64/512 × single/fixed/count × one/two slots × owned reset/serial replay × both
backends = 72 cases. Each slot independently cycles the eight changing-count
inputs. Checks compare every image, generated geometry/record/count and guard byte;
traces establish native draw capacity, binding/root counts, pools, resets,
submissions and balanced equal allocation budgets. `--preflight` uses 16 frames
per case, `--software` 64, and the default 1,000. No timing is performed yet.

The native controls bind raster/index/root once per range/group; separate public
calls currently repeat that state. The trace preserves this implementation cost
rather than making the native control artificially repeat public work. The
[counted→fixed driver regression](../../docs/draw-count-followup.md) remains a
separate strict test; these scenes do not mix count/fixed operations in one scope.
