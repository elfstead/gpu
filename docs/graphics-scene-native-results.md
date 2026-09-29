# M4 first control: native indexed geometry and depth

Accepted 2026-09-29 at clean **`5da5aaf`**, completing step 1 of the
[offscreen scene brief](graphics-consumer-plan.md). This is a direct Vulkan
reference, not public indexed/depth support or completion of M4. The runtime,
public ABI 17 and installed compiler subset are unchanged.

## What the control establishes

The [source and predeclared expectations](../examples/graphics_scene/README.md)
use two shared-vertex indexed quads, compute-written positions/indices/indirect
records and native index binding/drawing. Each surface uses four unique vertices
and six index references. A poison prefix, nonzero first index and signed base
vertex exercise native index semantics. Vertex pulling consumes the native indexed
vertex ID; it does not replace indexed execution with a non-indexed shader lookup.

The attachments are RGBA8 color and D32_SFLOAT depth with explicit format/usage
support checks. Ten modes cover LESS/ALWAYS, depth testing/writing disabled,
reversed draw order, clear depths 0/1, zero index counts and independent color/depth
CLEAR/LOAD across two rendering scopes. The near quad shifts in A/B/A frames.
No CPU reads generated geometry or draw arguments between compute and rendering.
Compute-to-index/vertex/indirect dependencies and attachment/readback dependencies
are explicit. Accepted submissions drain before native resources are destroyed.

The independent CPU oracle checks analytic rectangle coverage and occlusion,
exact interior RGBA and depth error <=1e-6. It conservatively excludes a one-pixel
band along the full lines containing rectangle edges; it is not a triangle
rasterizer or universal edge-rule oracle. All computed vertices, indices, native
indirect records and guard bytes match exactly. Full color/depth images, including
edges, must match on A repeat and between normal, reversed-order and split-LOAD
modes. Disabled-test/write controls deliberately produce different expected output.

## Observed results

| Driver | Extents | Frames | Analytically checked pixels |
|---|---|---:|---:|
| Radeon RX 5700 XT / RADV | 257x193, 640x360, 1280x720 | 90 | 35,187,870 |
| llvmpipe | 257x193, 640x360 | 60 | 8,017,950 |

All color/depth/geometry/guard, repetition and draw-order/load checks pass with
Vulkan and synchronization validation enabled. Analytic coverage is approximately
93%, 96% and 98% of pixels at the three extents; full-frame repeat comparisons
cover the excluded edges without claiming cross-device raster equivalence.

Four CPU oracle tests cover expected occlusion/clear modes, movement, native record
layout and rejection of altered colors/depth, nonfinite depth, guards and truncated
outputs. The native executable has no undefined OGPU symbols. Pinned Slang
2026.14.1 output is SPIR-V-validated; reflection checks the native root offsets,
sizes and compute local size without accepting vertex roots in the installed
adapter. Clang static analysis passes; its only warnings are unused linker flags
from the Nix wrapper. No sanitizer result is claimed for this new control.

The only shared test-infrastructure change adds an optional usage argument to the
native buffer allocator; existing callers still pass zero through the original
wrapper. Both drivers pass the existing public/native mapped learned-image
preflight: 25 configurations x 12 frames each, including the serial baseline,
dedicated/arena layouts and one/two/three slots. Allocation counts balance and no
live allocations remain. This is a bounded regression, not a new sustained M3 or
full M1 video-scale acceptance.

Retained evidence:

- [RADV report](results/graphics-scene-native-radv-2026-09-29/report.json) and
  per-extent stdout/stderr alongside it.
- [llvmpipe report](results/graphics-scene-native-lvp-2026-09-29/report.json) and
  per-extent stdout/stderr alongside it.
- [Validation/regression receipt](results/graphics-scene-native-validation-2026-09-29.json),
  commands, original report hashes, clean revision and limitations.

Source/shader/executable/output hashes and complete frame matrices were checked
before retention; original A/B/A and equivalent-mode outputs were compared again.
Large raw image/depth/geometry files remain in the local report directories listed
in the receipt. Git retains their hashes and execution logs, not the raw binaries.
Reproduction regenerates the artifacts. This is local Nix tooling on the existing
two drivers, not a fresh-host, second-physical-vendor or Metal result.

## Consequences for the public contract

1. **Real indexed execution must remain expressible.** An index address and format
   do not make all native storage eligible: address-based Vulkan index binding
   still requires index-buffer usage. The implementation must provide eligible
   storage and the contract must define its lifetime, bounds and ownership.
   [Native binding requirements](https://docs.vulkan.org/refpages/latest/refpages/source/VkBindIndexBuffer3InfoKHR.html)
2. **Color and depth load/clear choices are independent.** Modes 7/8 distinguish
   these choices; one shared “clear target” flag would hide required semantics.
   Depth format/compare/write state and actual attachment support need explicit
   treatment, including clear value validity and preservation dependencies.
3. **Do not mandate a rendering-scope break per draw.** This control records two
   draws in one scope as well as two scopes with preservation. Before extending
   the public draw calls, compare explicit begin/end attachment scopes against
   per-draw attachment descriptions that a native implementation could combine.
   The latter is not automatically inferior, but equivalent grouping must be
   expressible without a mandatory load/store or synchronization penalty.
4. **Generated stage support is a separate gate.** The reference uses a native
   vertex root pointing to geometry plus a flat integer varying. A proposed public
   slice must keep the compiler/runtime contracts aligned; successful direct
   Slang compilation is not acceptance of a widened installed adapter.

Follow-up: the [bounded indexed/depth proposal](indexed-depth-proposal.md) now
selects a candidate resolving those choices; the [ABI-18 migration](indexed-depth-migration.md)
implements the runtime slice. Compare the full public scene against this reference
next. Choose the better contract even if the old
draw API could be extended. This control does **not** measure cache efficiency,
rendering-scope cost, replay, two-slot scene execution or public/native overhead;
none is silently approved by passing pixel checks. M4's remaining graphics slices
and the fundamental native-performance criterion remain open.
