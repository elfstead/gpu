# M8 multiview: native masks and layered execution

2026-11-06. Foundation revision 10 implements the [multiview contract](../multiview.md)
through C and direct Rust, with native feature enabling/limits, prepared pipeline
masks, render/secondary masks, bounds checks and generated-set compatibility.
The existing operations remain the full execution path; no per-view host loop or
runtime fallback is introduced.

## Local acceptance

Linux x86-64, Radeon RX 5700 XT, RADV/Mesa 26.2.5, loader 1.4.363, validation layers
1.4.357 plus synchronization validation, Rust 1.98.1 and Slang 2026.14.1.

- 86 host library tests and two Rust correspondence tests pass; strict core and
  independent Rust-consumer Clippy pass.
- 28 native library GPU tests pass, including independently requesting multiview
  without enabling raster. The full C fixture passes with validation under the
  timestamp readback boundary below.
- Multiview checks 12 submissions and 4,608 D32 values: sparse masks `5`, `10`, `8`,
  a four-layer view starting at layer one of a six-layer image, inactive/outside
  layers, clears/scissors, direct/indexed/indirect drawing, primary/secondary lists,
  root-data replay, occlusion totals and host guards. Bindings set before dynamic
  rendering remain usable. Seven malformed recording cases reject before native
  invalid commands; preparation also rejects out-of-limit masks when representable.
- Native timestamp readback of every active-view slot passes in the separate
  no-validation-layer run. The standard validated run checks the first timestamp
  slot plus complete occlusion results and timestamp-range rejection.
- ABI: 2,061 layout values; surface audit: 151 operations, 127 records,
  143 constants and 1,155 surface values. Both new feature IDs are covered.
- Vulkan bindings and shader reproduction, compute interface reproduction and
  graphics interface tests pass. Actual vertex/fragment Slang ViewIndex artifacts
  produce `MULTIVIEW` requirements; legacy generation and missing capability reject.
- 47 scene, eight installer and two textured-handoff checker tests pass. Current
  consumers/version gates migrate to revision 10; historical receipts keep their
  original versions.

Clean SDK acceptance is the next check after the implementation commit.

## Timestamp validation discrepancy

The first complete-array run returned the expected data and availability, but
validation emitted `VUID-vkCmdCopyQueryPoolResults-None-08752` for timestamp slot 1.
This was investigated rather than accepted as a clean validated run.

The installed layer's upstream source, tag `v1.4.357`, commit
`e4786f7ce8f1319215eff0d938f4be4651cbb85d`, tracks a single `QueryObject` in
[`CommandBuffer::RecordWriteTimestamp`](https://github.com/KhronosGroup/Vulkan-ValidationLayers/blob/e4786f7ce8f1319215eff0d938f4be4651cbb85d/layers/state_tracker/cmd_buffer_state.cpp#L596)
and marks only that object ended in its
[core query tracker](https://github.com/KhronosGroup/Vulkan-ValidationLayers/blob/e4786f7ce8f1319215eff0d938f4be4651cbb85d/layers/core_checks/cc_state_tracker.cpp#L845).
That misses the remaining slots written by a multiview timestamp. This conflicts
with the [native timestamp rule](https://docs.vulkan.org/spec/latest/chapters/queries.html#queries-timestamps):
all active-view slots are written, either with timestamps or with a first timestamp
and remaining zeros. The observed failure is consistent with that tracking defect.
No upstream issue has been submitted and no patched layer has been accepted here.

The runtime has no workaround. The default fixture explicitly reports first-slot
only timestamp readback. Set `OGPU_FOUNDATION_MULTIVIEW_ALL_TIMESTAMPS=1` to resolve
and check availability for the full array. With this layer it reproduces the
validation error; without it the complete native data/availability/guard oracle
passes. No error filter silently converts that full-array validation run into PASS.
Full-array acceptance with a corrected validation layer remains open.

## Reproduce

With the pinned tools/loader configured:

```sh
cargo test --offline --no-default-features -p ogpu
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --test-threads=1
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
cargo xtask foundation-shaders --check
cargo xtask foundation-compiler --check
cargo xtask foundation-graphics-compiler --check
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 cargo xtask foundation
env -u VK_INSTANCE_LAYERS -u VK_LAYER_VALIDATE_SYNC OGPU_FOUNDATION_MULTIVIEW_ALL_TIMESTAMPS=1 cargo xtask foundation
```

This is correctness/structural-lowering evidence, not native performance parity.
Color/MSAA/resolve and generated multiview interactions, physical mesh multiview,
other hardware and native Metal/Windows execution remain unaccepted. M8 continues
with shading rate and the other raster profiles; M6 remains deferred.
