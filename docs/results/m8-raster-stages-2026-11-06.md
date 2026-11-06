# M8 tessellation and geometry acceptance

2026-11-06, foundation revision 11. The [contract](../raster-stages.md) adds independent
native stage/multiview/point-size enables, physical limits, patch state and topology
coverage. Existing public layouts and operations are unchanged; the existing draw,
argument, dependency and query mechanisms carry the new stages.

## Native correctness

Linux x86-64, Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan loader 1.4.363,
validation layers 1.4.357 with synchronization validation; Rust 1.98.1 and pinned
Slang 2026.14.1. Physical limits report maximum patch size 32, tessellation level
64, geometry output vertices 256 and geometry invocations 32.

The public C fixture passes six pipeline modes with two changed-data replays:

| Mode | Draw | Stages | Observed statistics at tessellation levels 2 → 4 |
|---|---|---|---|
| 0 | Direct | Vertex/geometry/fragment | 1 geometry invocation and primitive in each replay |
| 1 | Indexed | Vertex/adjacency geometry/fragment | 1 geometry invocation and primitive in each replay |
| 2 | Indirect | Vertex/control/evaluation/fragment, upper-left origin | 1 control patch; 7 → 21 evaluation invocations |
| 3 | Direct | Same, lower-left origin | 1 control patch; 7 → 21 evaluation invocations |
| 4 | Indexed | Vertex/control/evaluation/geometry/fragment, upper-left | 6 → 24 geometry invocations/primitives; 1 patch; 7 → 21 evaluations |
| 5 | Indirect | Same, lower-left origin | Same native counts as mode 4 |

All **12 submissions and 768 pixels** pass with unchanged input bytes and guards.
Each stage contributes a distinct value read through the shared caller-owned root,
so correct output exercises the complete stage chain. Both origins execute but
this symmetric fixture does not independently distinguish their winding effect.
Seven malformed preparations reject: zero/oversized patch count, invalid origin,
missing patch state, incompatible topology, cyclic chain and unpaired tessellation.
No malformed work is submitted.

The full existing public C suite passes with no validation errors. Four existing
unused-output warnings remain in the null/missing color-slot raster fixtures;
there is no new tessellation/geometry warning or suppressed diagnostic.

## Other checks

97 host library tests, 2 native Rust correspondence tests and 31 native library
tests pass. Native feature checks enable tessellation, geometry, point size and
the two multiview combinations independently, and reject unsupported argument
visibility. Host tests cover extension-chain composition, stage pairing, patch
bounds, feature prerequisites, native synchronization mapping and generated-stage
mask round trips. Strict core Clippy passes.

C/Rust ABI passes with 2,154 values. Foundation surface audit passes: 154 operations,
138 records, 185 constants and 1,284 correspondence values. Pinned Vulkan bindings
and foundation shader artifacts reproduce and validate; existing compute/graphics
compiler checks pass. The bounded generated-interface tool remains a vertex/
fragment-pair tool, not a general stage-graph linker.

Performance parity, native generated/secondary/multiview draws with these stages,
point/line/quad/isoline output, stage-combination coverage and external backends
remain unaccepted as detailed in the [contract](../raster-stages.md#remaining-verification-and-scope).
M8 remains active; an integrated textured/depth consumer and remaining feature
coverage audit come next.

Reproduction uses the pinned tools and local Vulkan/validation environment:

```sh
cargo test --offline --no-default-features -p ogpu
cargo clippy --offline --no-default-features -p ogpu --all-targets -- -D warnings
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
cargo xtask foundation-shaders --check
cargo xtask foundation-compiler --check
cargo xtask foundation-graphics-compiler --check
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --test-threads=1 --nocapture
cargo xtask foundation
```
