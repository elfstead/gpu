# M8 sample-location acceptance

2026-11-07, foundation revision **12**. The [contract](../sample-locations.md)
implements explicit sample patterns, independent dynamic enable, compatible-depth
images and ordinary/split transition metadata. `ImageBarrier.next` changes the
record layout from 88 to 96 bytes on the tested target; all consumers must rebuild.
C/Rust callers, SDK revision gates and the libplacebo adapter are migrated together.

## Local execution

Radeon RX 5700 XT, RADV/Mesa 26.2.5, loader 1.4.363 and Vulkan validation 1.4.357
with synchronization validation. Rust 1.98.1, pinned Slang 2026.14.1 and the checked
Vulkan registry produce the runtime, bindings and shader artifacts.

The device advertises sample counts 1, 2, 4 and 8, 2×2 grids for each, coordinate
range `[0, 0.9375]`, four subpixel bits and variable patterns. Dynamic enable is
also available. Execution here uses **four samples**, with color average resolve
and depth sample-zero resolve. Other counts are discovered, not execution-certified.

| Profile | Observed behavior |
|---|---|
| Static left/right positions | A half-pixel edge distinguishes the two patterns with exact resolved color coverage |
| Dynamic left/right patterns | Matches the corresponding static color and depth bytes |
| Dynamic enable, static pattern | Selected enable ignores static poison; output matches the static control |
| Dynamic enable and pattern | Both independently selected fields initialize and match the static control |
| 2×2 pattern grid | Alternating pixel-grid positions change edge coverage as specified |
| Mixed sample positions | Two covered samples resolve to the expected half-coverage value; sample-zero depth is checked |
| Split dependency | Matching release/acquire pattern metadata preserves depth exactly like the ordinary transition control |

Each profile records once and replays with two different input depth offsets.
A sloped-depth producer is followed by attachment→GENERAL→attachment transitions
and a LOAD pass. The later pass tests depth without writing it. Both resolved
depth images match byte-for-byte, and every depth value is checked against the
sample-position plane equation within `1e-6`. Color coverage is exact RGBA8.

**Nine profiles pass 18 submissions and 2,304 color/depth pixel pairs.** Five bad
preparations and five bad recordings reject: wrong count, zero grid, invalid
boolean/reserved data, chained patterns, metadata on an incompatible image and
mismatched image/pattern sample counts. Caller inputs and output guards survive.

An initial split fixture failed synchronization validation because it omitted
reset-to-signal execution ordering and sufficient source access for the subsequent
transition. The fixture now supplies those explicit dependencies. The backend
adds no automatic barrier or event repair. The final full C run has **zero
validation errors**, no new warnings, and the four pre-existing unused-output
warnings in old null/missing color-slot cases.

## Host, migration and correspondence

107 host library tests, two native Rust correspondence tests and 31 native library
tests pass. Host checks cover pattern shape/count/feature rules, native clamping
without coordinate rewriting, selective dynamic fields, compatible-depth metadata
and conditional scratch sizing/alignment. Strict Clippy passes, including the
existing Rust 1.85 MSRV constraint.

The ABI audit passes **2,304 layout values**. Surface correspondence passes
**164 operations, 151 records, 240 constants and 1,423 values**. Native bindings
and shader artifacts reproduce; shaders validate. SDK/tooling tests pass 21 cases;
scene/oracle/revision-migration tests pass 47 cases. An old hard-coded revision in
one test was corrected; historical receipts retain their original revisions.

```sh
cargo test --offline --no-default-features -p ogpu
cargo clippy --offline --no-default-features -p ogpu --all-targets -- -D warnings
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
cargo xtask foundation-shaders --check
python3 -m unittest discover -s tools -p 'test_*.py'
python3 -m unittest discover -s examples/graphics_scene -p 'test_*.py'
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --nocapture
cargo xtask foundation
```

This establishes native lowering and bounded correctness, not performance parity.
Fixed-pattern devices, other counts, coordinate-clamping execution, shader
interpolation restrictions, secondary/continuation/multiview combinations,
ownership transfers and Metal/DX12 remain unverified. The shared barrier path
represents those combinations but this fixture does not claim their acceptance.

Next M8 work is complete dynamic vertex input/divisors, remaining native dynamic
raster controls and the extension-format inventory, then capture/dynamic patch
and consolidation before M9.
