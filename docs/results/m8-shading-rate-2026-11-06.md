# M8 fragment shading rate: implementation and local checks

2026-11-06. Foundation revision 11 implements the [shading-rate contract](../shading-rate.md)
through C and direct Rust. Pipeline, primitive and attachment inputs have separate
feature IDs; mesh primitive output has a further independent enable. Physical
limits and rate/sample tuples, static/dynamic state, explicit attachment pipeline
flags, image/view usage, render attachment and dependency scopes lower to native
Vulkan structures and commands. The bounded vertex/fragment compiler handles
Slang shading-rate built-ins and preserves the fragment input's alternative-enable
requirement in generated compatibility checks.

This is implementation plus host/compiler and regression evidence. **Positive
native shading-rate rendering is not accepted.** Radeon RX 5700 XT / RADV Mesa
26.2.5 exposes pipeline=false, primitive=false, attachment=false, mesh=false and
zero rate/sample tuples. The tests do not emulate support or count rejection as
shaded-pixel execution. No additional GPU is assumed.

## Local evidence

Linux x86-64, Rust 1.98.1, Slang 2026.14.1, Vulkan loader 1.4.363, validation layers
1.4.357 and synchronization validation:

- 89 host library tests and two Rust correspondence tests pass. New coverage
  includes independent enabling, IDs above 64, native hard rules versus final-rate
  reduction, texel/aspect limits, offset extent/overflow and sparse/broadcast layers,
  and explicit attachment compilation policy. Core and standalone Rust strict
  Clippy pass.
- 29 native library tests pass. The new probe queries actual capabilities, tests
  independent requested features and rejects unavailable inputs. A separately
  intercepted setter checks exact size/combiner arguments and rejection before
  lowering. That intercepted call **does not execute a native shading-rate command**.
- The complete public C fixture passes with validation, including adapter/device
  shading-rate query correspondence. Existing compute, graphics, generated-command
  and multiview checks pass. Multiview timestamp readback retains the previously
  documented [first-slot validation boundary](m8-multiview-2026-11-06.md).
- ABI checks pass for 2,109 layout values. Surface audit covers 152 operations,
  131 records, 161 constants and 1,208 values. Pinned bindings reproduce exactly.
- Shader reproduction and six compute compiler tests pass. Seven graphics compiler
  tests pass, including actual Slang-to-SPIR-V shading-rate shaders, validation,
  legacy/missing-capability rejection and compiled C alternative-enable checks.
- 47 scene tests, eight installer tests and two textured-handoff tests pass.
  Current consumers and SDK version gates migrate to revision 11.

The first C regression run found the fixture's old allowed-format mask (8191)
rejecting the newly reported shading-rate bit. The mask is now 16383 and the full
rerun passes. No runtime workaround or validation-error suppression was added.
The scalar/state review also removed an unnecessary single-mip restriction from
rate attachment views and fixed the disabled-feature dynamic-state gate.

Clean installed-SDK acceptance passes at
`8a57dc4e780966f42f4fd5ae5e15ffce1864c038`. The manifest reports foundation 11,
`source_dirty=false`, no legacy/default Cargo features and runtime SHA-256
`3fc75276b72c4e26a4297c363160761118dea85bbd2fc87ef560cfc2ccaeea36`.
All six relocated C example families, pinned GGML (transformer/quantized plus both
full-dataset MNIST precision modes), pinned libplacebo, and isolated native Rust
headless/Wayland checks pass. Installed source graphs, runtime identity, expected
failures and exact exports pass. These establish revision-11 compatibility; they
do not establish positive shading-rate execution. The [adjacent JSON](m8-shading-rate-2026-11-06.json)
records the exact source/runtime identities, capability observations and log hashes.

## Reproduce and remaining acceptance

```sh
cargo test --offline --no-default-features -p ogpu
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --test-threads=1 --nocapture
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
cargo xtask foundation-shaders --check
cargo xtask foundation-compiler --check
cargo xtask foundation-graphics-compiler --check
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 cargo xtask foundation
python3 tools/install.py --prefix /tmp/ogpu-m8-8a57dc4-sdk
python3 tools/test-foundation-install.py --prefix /tmp/ogpu-m8-8a57dc4-sdk \
  --ggml-source target/ggml-source --ggml-data target/ggml-data \
  --libplacebo-source target/libplacebo-source \
  --vulkan-registry vendor/Vulkan-Headers/registry/vk.xml
python3 tools/test-rust-install.py --prefix /tmp/ogpu-m8-8a57dc4-sdk --presentation
```

Use the pinned toolchain and correct local loader/ICD/layer paths. The complete
[native verification plan](../shading-rate.md#verification-still-needed) covers
static/dynamic observed rates, independent inputs/combiners, attachment production
and synchronization, sample/depth interactions, layered/multiview and generated/
secondary execution, mesh output and native controls. No positive pipeline,
attachment or mesh shading-rate execution, cross-backend equivalence or performance
parity is inferred from these checks. M8 remains open; tile-local/input-attachment
and pass-continuation implementation is next. M6 remains deferred without a Mac.
