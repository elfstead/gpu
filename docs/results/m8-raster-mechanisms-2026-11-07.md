# M8 raster mechanism acceptance

2026-11-07, foundation revision 11. The [contract](../raster-mechanisms.md) adds
independent conservative rasterization, line/stipple modes, provoking vertex,
depth convention and eight dynamic setters. Existing public layouts and draw
commands remain unchanged. This is one tranche of the remaining M8 mechanisms,
not completion of M8.

## Local execution

Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan loader 1.4.363 and validation layers
1.4.357 with synchronization validation. Rust 1.98.1, Slang 2026.14.1 and the pinned
Vulkan registry supply the runtime and reproducible shader/native bindings.

| Case | Observation over two changed-data replays |
|---|---|
| First/last provoking vertex | Exact different flat RGBA8 colors; dynamic last equals static last |
| Zero-to-one/negative-one-to-one depth | Negative Z is clipped only by the zero-to-one convention; positive Z is visible in both. Dynamic negative convention equals static. |
| Conservative disabled/over/under | Two triangle shapes produce 0/1 normal, 1/8 overestimated and 0/0 underestimated pixels. Bounds and white values checked. Dynamic conservative mode equals static overestimation. |
| Rectangular/Bresenham/smooth lines | Horizontal segment covers six pixels in each mode. Smooth coverage uses spatial/channel bounds, not a universal exact antialiasing oracle. |
| Bresenham stipple | Alternating pattern covers three pixels; dynamic mode, stipple enable and parameters equal static output |
| Independent dynamic depth clip | Negative and positive Z both visible when explicitly disabled |
| Combined records | All five record types in one chain prepare and render; last-vertex color and negative-depth visibility checked |

**17 supported profiles pass 34 submissions and 2,176 pixels.** Twelve malformed
preparations reject: invalid enums/booleans, cyclic records, reserved fields,
nonfinite overestimation and invalid stipple factor. Input bytes and guards remain
unchanged. Static poison values are ignored only for selected dynamic fields.

Physical properties: primitive overestimation, extra maximum and extra granularity
are zero; underestimation, degenerate triangle coverage, fully-covered input,
post-depth coverage and per-pipeline provoking modes are supported; conservative
point/line and degenerate-line coverage are not. Line subpixel precision is four
bits and `strict_lines` is false. Queries do not certify shader built-in execution.

This GPU lacks **dynamic extra overestimation, stippled rectangular lines and
stippled smooth lines**. Four dependent fixture profiles explicitly skip. Dynamic
conservative mode is tested independently rather than skipped with unavailable
extra-overestimation control. No fallback is installed.

The full C regression has zero validation errors and the same four pre-existing
unused-output warnings in null/missing color-slot cases. New raster cases have no
validation warnings. All 31 native library tests pass, including independent
enabling and rejection of unsupported requests; deliberate unavailable-device
requests produce four expected feature-not-present warnings (the three new
unsupported features plus patch restart).

## Host and correspondence

103 host library tests and two native Rust correspondence tests pass, including
bounded chain parsing, feature independence, local bounds and selective dynamic
field ignoring. Strict Clippy passes. ABI checks cover 2,255 layout values;
surface correspondence covers **162 operations, 147 records, 232 constants and
1,389 values**. Pinned Vulkan bindings and shader artifacts reproduce; all
foundation artifacts validate.

```sh
cargo test --offline --no-default-features -p ogpu
cargo clippy --offline --no-default-features -p ogpu --all-targets -- -D warnings
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
cargo xtask foundation-shaders --check
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --nocapture
cargo xtask foundation
```

No universal performance-parity claim, external backend acceptance or unsupported
native execution is inferred. Nonzero extra overestimation, other primitive and
line shapes/widths, shader built-ins, secondary/generated/multiview composition
and broader hardware remain unverified. Programmable sample positions and depth
compatibility, complete dynamic vertex input/divisors and remaining native raster
controls are still implementation work in the [M8 audit](../m8-coverage.md).
