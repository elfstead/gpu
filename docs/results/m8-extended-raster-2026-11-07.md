# M8 extended dynamic raster acceptance

2026-11-07, foundation revision 12. The [contract](../extended-dynamic-raster.md)
adds eleven independently enabled dynamic controls and a plain blend-equation
record. Existing record layouts are unchanged. C and native Rust share direct
backend setters, with borrowed per-attachment arrays and no allocation, scratch,
current-state tracking or executable compilation during recording.

Hardware: AMD Radeon RX 5700 XT, RADV NAVI10, Mesa 26.2.5, Vulkan 1.4.
All eleven new dynamic capabilities and their exercised base features are available.

## Local output and capability evidence

The new C fixture passes **11 profiles, 88 submissions and 11,264 pixels**. Each
profile independently compares its dynamic control with static pipelines at one
and four samples, using two RGBA8 color attachments, two draws with different
state, changed-input serial replay, exact output comparison and guard bytes.
Every non-count control visibly changes output between its two phases.

| Profile | Control exercised |
|---|---|
| 0 | Depth clamp: clipped versus clamped out-of-range geometry |
| 1 | Polygon fill versus line coverage |
| 2 | One dynamic-count executable reused at 1/4 samples, static partial mask |
| 3 | Dynamic full/partial/empty sample masks |
| 4 | Alpha-to-coverage enabled/disabled with zero shader alpha |
| 5 | Alpha-to-one enabled/disabled |
| 6 | Logic-operation enable, fixed XOR |
| 7 | Per-attachment blend enable |
| 8 | Per-attachment add/reverse-subtract equations, independently ranged updates |
| 9 | Different color write masks for the two attachments, ranged updates |
| 10 | Logic operation COPY versus INVERT |

Ignored static counterparts deliberately contain invalid values. Eleven malformed
recordings fail before submission. Native library tests create devices with each
new dynamic feature independently, call supported setters, reject disabled
neighbors and value-dependent base features, and exercise empty arrays without
emitting invalid native commands. This is explicit enabling evidence, not merely
success on a device with every capability enabled.

Three additional sample-position profiles pass: dynamic rasterization count with
static positions/mask, with static positions/dynamic mask, and with dynamic
positions/static mask. They match the existing static reference, including D32
depth, color/depth resolves, LOAD preservation and explicit depth-transition
metadata. The whole sample-position fixture now passes **12 profiles, 24
submissions and 3,072 color/depth pixel pairs**, plus ten rejected preparations or
recordings. The existing nine profiles retain their coverage.

Review confirmed that executable objects do not cache sample-count compatibility.
Static custom-position preparation did enforce equality with the static count;
that check now applies only when rasterization count is static. Vulkan's ignored
static count still sizes a static sample mask, so lowering preserves both words
of OGPU's `u64` with a native count of 64. Host checks cover preservation of the
high mask word; execution above four samples is not claimed.

## Verification and limits

- 114 host library tests and two native Rust/C correspondence tests pass.
- All 31 native GPU library tests pass under Vulkan validation. Deliberately
  caught panics in the existing unwind test remain expected.
- Strict Clippy passes with the crate's Rust 1.85 MSRV constraint.
- ABI: 2,313 layout values. Surface: 185 operations, 153 records, 273 constants
  and 1,467 representation values.
- Full C regression passes with synchronization validation: zero errors and the
  four previously known unused-output warnings outside the new profiles.
- Pinned bindings and the existing 70 shader artifacts reproduce. No new shader
  compiler recipe or artifact is needed; this fixture reuses graphics/MRT shaders.
- All 21 tooling tests pass.

Commands use `/tmp/ogpu-c2-tools.sh`; GPU execution additionally enables the
Khronos validation layer and synchronization validation:

```sh
cargo test --offline --no-default-features -p ogpu
cargo clippy --offline --no-default-features -p ogpu --all-targets -- -D warnings
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
cargo xtask foundation-shaders --check
python3 -m unittest discover -s tools -p 'test_*.py'
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --nocapture
cargo xtask foundation
```

This is bounded functional and direct-lowering evidence. It does not establish
performance parity, 8–64-sample output, dynamic dual-source rendering, every
format/sample-shading combination, secondary/generated combinations, or Metal/DX12
acceptance. No new external ML/video consumer or human presentation acceptance is
claimed. M8 continues with viewport arrays, extended depth behavior, the independent
color-write-enable audit, format inventory, geometry capture/dynamic patch state,
and final consolidation before M9.

## Clean installed acceptance

Implementation `ebf6abb30669018c314c6f9193213bc1bf20ca6c` installed with
`source_dirty=false` into `/tmp/ogpu-m8-extended-raster-sdk`. Runtime SHA-256:
`b917e792b9869b627cabbaa505832cf6c612ca55761be1bfc81f69b49ff9123e`.

The optimized C fixture passes against the installed header/runtime with all 70
shader inputs, including the eleven new control profiles and all twelve
sample-position profiles. Exact shared-library exports match the reviewed surface.
The separately relocated native Rust source bundle builds and passes correspondence,
missing-loader rejection and headless execution without linking the C SDK. The
new contract is packaged in the installed documentation and Rust source bundle.

The [machine-readable receipt](m8-extended-raster-2026-11-07.json) records source,
runtime and header identity, check counts, evidence hashes and remaining limits.
