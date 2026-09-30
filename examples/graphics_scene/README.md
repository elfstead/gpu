# M4 matched indexed/depth scene

The direct Vulkan control links only libc/libdl. Its ABI-18 public counterpart
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
Each indirect call has drawCount=1; multiDrawIndirect is not newly required.

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
2026.14.1 and SPIRV-Tools compile native shaders; this does not widen the installed
compiler's vertex-root subset. Fresh report directories retain shaders, full
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
