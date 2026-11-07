# M8 core dynamic raster acceptance

2026-11-07, foundation revision 12. The [contract](../core-dynamic-raster.md) adds
eight direct C/native Rust setters and a topology-class query. Existing public
record layouts remain unchanged. Selected static fields are ignored; preparation
and recording share numerical validation. Recording adds no allocation, pipeline
creation, inferred state or GPU conversion pass. This is bounded correctness and
native-lowering evidence, not a performance-parity measurement.

Hardware: AMD Radeon RX 5700 XT, RADV NAVI10, Mesa 26.2.5, Vulkan 1.4.
`unrestricted_topology=0`: dynamic topology must stay within its prepared class.

## Local execution

Ten profiles pass, each comparing two static executables with one dynamic
executable that changes state between draws. Both recordings replay twice with
changed input. Total: **40 submissions, 2,560 color/depth pixel pairs**, exact
static/dynamic output comparison, numerical depth checks and input/output guards.

| Profile | Verified behavior |
|---|---|
| 0 | Triangle list/strip changes within the triangle class |
| 1 | Indexed strip restart disabled/enabled, including a restart sentinel |
| 2 | Discard disabled/enabled between draws; attachment clear/output preserved |
| 3 | Line width one/three, with wider coverage in the second half |
| 4 | Depth-bias enable changes on sloped D32 depth |
| 5 | Positive/negative slope-bias values |
| 6 | Positive/negative bias clamps |
| 7 | Bounds ranges accepting/rejecting stored depth |
| 8 | Bounds enable changes independently of the range |
| 10 | Line widths outside the queried range passed unchanged in both paths |

Profile 9 (cross-class triangle/line topology) is explicitly skipped on this GPU.
The discard reference omits its discarded draw, so it verifies attachment effects,
not earlier shader-stage side effects. Dynamic preparation deliberately receives
invalid/NaN static counterparts to catch accidental validation/use. Eight malformed
recordings fail before submission, one for each new setter.

Review found that static line width previously rejected values outside the queried
range. Native supported-width selection permits them, so that restriction was
removed for both paths. Host tests and profile 10 cover the correction. Widths
remain finite/positive and values other than one require `WIDE_LINES`.

## Verification

- 111 host library tests and two C/native Rust correspondence tests pass.
- All 31 native GPU library tests pass. A raster-only device additionally checks
  baseline setters, disabled optional wide-line/clamp/bounds capabilities, and
  adjacency/patch feature rejection without submitting invalid work.
- Strict Clippy passes with the crate's Rust 1.85 MSRV constraint.
- ABI audit: 2,306 layout values. Surface: 174 operations, 152 records,
  251 constants, 1,437 representation values.
- Pinned Vulkan bindings reproduce. All foundation shader artifacts reproduce
  with Slang 2026.14.1 and validate, including the two new artifacts.
- 21 tooling tests pass.
- Full C regression passes under Vulkan synchronization validation: zero errors
  and four previously known unused-output warnings outside the new profiles.

Commands use `/tmp/ogpu-c2-tools.sh`; the C GPU run additionally enables the
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

Cross-class topology, constant-term bias rendering, other depth formats, advanced
stage/secondary/generated combinations, other hardware and Metal/DX12 remain
unverified. No new performance, presentation/human or external ML/video consumer
acceptance is claimed. M8 continues with the [named extended raster control groups](../m8-coverage.md#remaining-raster-control-groups),
format inventory, geometry capture/dynamic patch state and final consolidation.
