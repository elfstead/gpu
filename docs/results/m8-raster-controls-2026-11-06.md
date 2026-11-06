# M8 raster/fetch control acceptance

2026-11-06, foundation revision 11. The [contract](../raster-controls.md) adds six
independent enables, a physical limits query, explicit depth clipping and instance
divisor records. Existing layouts, draw commands and recording state are unchanged.

## Local execution

Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan loader 1.4.363 and validation layers
1.4.357 with synchronization validation. Rust 1.98.1, Slang 2026.14.1 and the pinned
Vulkan-Headers registry generate the checked native bindings/artifacts.

| Case | Observation over two changed-data replays |
|---|---|
| Shader point size | Centered size-four and size-six points cover exactly the expected pixels |
| Four clip/clamp combinations | Negative and positive Z distinguish independent clipping from the coupled default |
| Triangle-list restart | All-ones index discards an incomplete triangle; one completed primitive by query and expected pixels |
| Patch-list restart | Native capability unavailable; execution explicitly unverified |
| Divisor two and zero, static stride | Four instances fetch repeated pairs or one constant element; changed colors replay correctly |
| Divisor two and zero, dynamic stride | Stride 32 overrides prepared stride 16; nonzero first instance offsets the fetched elements without changing placement |

Ten supported profiles pass **20 submissions and 1,280 RGBA8 pixels**. All caller
input bytes and guards remain unchanged. Six malformed preparations reject before
native pipeline creation: invalid clipping boolean/reserved/chain, missing divisor
binding, per-vertex divisor rate and nonzero divisor reserved field.

Physical limits report point range `[0, 8191.875]` with granularity `0.125`, maximum
divisor `UINT32_MAX`, nonzero-first-instance support and polygon point-size support.
Positive shader point sizes still obey native clamping/granularity. Querying these
properties does not enable a feature.

The full public C fixture passes without validation errors. Its four pre-existing
unused-output warnings remain in the null/missing color-slot fixtures. The new
control cases produce no warning or suppressed diagnostic. The 31 native library
tests also pass, including independent enabling and unavailable-feature rejection.

## Host and correspondence

100 host library tests and two native Rust correspondence tests pass. New checks
cover independent restart/divisor/clip enables, bounds, duplicate/rate errors and
malformed chains. Strict Clippy passes. C/Rust ABI passes 2,194 layout values;
surface audit passes 154 operations, 142 records, 194 constants and 1,315 values.
Pinned Vulkan bindings and the six shader artifacts reproduce and validate.

This is direct-mapping and bounded correctness evidence, not performance parity.
Patch restart, further topology/stage/secondary/generated combinations and native
Metal/DX12 acceptance remain unverified. General stage-interface compiler support
and dynamic divisor/input/clip state are not added by this batch.

```sh
cargo test --offline --no-default-features -p ogpu
cargo clippy --offline --no-default-features -p ogpu --all-targets -- -D warnings
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
cargo xtask foundation-shaders --check
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --test-threads=1 --nocapture
cargo xtask foundation
```

Clean installed acceptance is recorded separately after the implementation commit.
