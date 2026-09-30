# Indexed/depth contract matrix

Accepted 2026-09-30 at clean `bad8c83aa7509d520bcaa0a3d3a7e1c0783f4bc3`.
No production runtime or ABI changes were needed for these additional gates.

- All eight depth comparisons × independent test/write enables × UINT16/UINT32:
  64 cases per driver. An oversized indexed triangle has z=0.5; uploaded depth
  stripes contain 0.25/0.5/0.75/1. An explicit CPU truth table checks every color
  and depth texel, plus guards, after two actual-draw LOAD scopes. Read-only depth
  remains byte-identical; disabled testing suppresses writes even when enabled.
- Foreign logical-device color/depth/index/indirect/raster objects, mismatched
  attachment extents/usages, raster color/depth mismatches and root-size errors
  are rejected before native execution. Rejected begin leaves no scope open;
  rejected draws retain no foreign objects. Subsequent valid draws still compile,
  execute, and replay. This uses two logical devices on one physical device.
- Twenty indexed failure configurations per driver cover cached/explicit storage,
  one-shot/list preparation failures, known submit rejection, indeterminate submit
  errors, reset failure and synthetic loss after real completion. Failed recordings
  are consumed; known rejected lists retain resources and retry successfully;
  indeterminate lists reject reuse. Images, index/vertex/indirect backing and raster
  lifetimes are checked with weak ownership references. Native command storage
  creation/destruction balances; lost devices reject new work.
- Existing image/raster creation injection now covers D32 allocations and depth
  pipelines as well as color-only paths, including partial native pipeline creation.

These are synthetic failure controls around real resources, not physical device
loss, exhaustive Vulkan-driver failures or a timing result. Host-written UINT16
coverage does not replace the full GPU-generated scene's narrower UINT32 evidence.

Full regression acceptance: **32 GPU tests on each driver**, 41 ordinary Rust
tests, Clippy with warnings denied, and 824 C/Rust ABI values. Radeon RX 5700 XT and
llvmpipe used one selected ICD, Vulkan validation, synchronization validation and
one test thread. `cargo xtask gpu-tests` runs with uncaptured output and rejects
validation-error diagnostics; both full runs passed.

[Raw logs](results/indexed-depth-contract-2026-09-30/radv.log) are retained alongside
`lvp.log`, `cpu.log`, `clippy.log`, and `abi.log`. Commands after selecting the driver
and setting `RUST_TEST_THREADS=1`, `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation`
and `VK_LAYER_VALIDATE_SYNC=1`:

```sh
cargo xtask gpu-tests
cargo test --locked -p ogpu --lib
cargo clippy --locked -p ogpu --all-targets -- -D warnings
cargo xtask abi
```

The next major gate is stable scene reuse and matched grouped-draw/replay
performance. M4 still needs the generated/installed scene, mip/views and wider
raster state. The fundamental native-performance criterion remains open.
