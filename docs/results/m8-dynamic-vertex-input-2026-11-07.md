# M8 complete dynamic vertex input acceptance

2026-11-07, foundation revision 12. The [contract](../dynamic-vertex-input.md)
adds optional feature 98, dynamic-state bit 24 and two C/native Rust operations.
Existing records and ABI layouts are unchanged. Explicit caller scratch packs
native input descriptions for one Vulkan command; no allocation, driver capability
query, pipeline compilation, vertex conversion or retained layout is added to
recording. This is correctness and native-lowering evidence, not performance parity.

Local hardware: AMD Radeon RX 5700 XT, RADV NAVI10, Mesa 26.2.5, Vulkan 1.4.
`VK_EXT_vertex_input_dynamic_state` is supported and explicitly enabled. The
available divisor limit is `UINT32_MAX`; nonzero first instance is supported.

## Executed cases

The raster-control fixture now passes 16 profiles, 32 submissions and 2,048 pixels,
with six rejected preparations, seven rejected recordings and byte guards. Ten
profiles are prior raster/fetch coverage; the six new complete-input profiles are:

| Profile | Fetch description | Evidence |
|---|---|---|
| 11 | Binding 3, float color, instance divisor 2 | Nonzero first instance, changed-data replay |
| 12 | Binding 5, offset 16, stride 32, divisor 0 | Sparse binding and repeated instance element |
| 13 | Divisor 1, offset 16; input stride 16 overridden to 32 by address binding | Complete input composes with independent dynamic stride, first instance 1 |
| 14 | Binding 5, vertex rate, zero stride, offset 16 | Constant fetch with no divisor extension |
| 15 | Divisor 1 then divisor 0 between two draws | Same executable, changed fetch state and nonzero first instance |
| 16 | RGBA8 normalized color, offset 4, stride 8, divisor 2 | Native format conversion compared to exact expected colors |

Each new profile first records an empty input description, then installs its
complete layout. Deliberately malformed static input is ignored by dynamic
executables. Invalid counts, missing binding, depth format, insufficient/null
scratch, reserved divisor metadata and non-instance divisor state poison recordings;
none is submitted. Unsupported local patch restart remains unverified.

The generated-graphics fixture adds `state_tokens=2`: complete dynamic input with
GPU-produced vertex/index tokens and direct/indexed/count actions, implicit and
explicit preprocessing, culling, compaction, GPU counts and guards. It passes 48
submissions and 98,304 pixels. The two prior generated profiles also pass. Native
executable selection remains unavailable on this device.

The first new fixture run exposed a fixture-data error: divisor one with four
instances starting at one needs five color entries. The input upload was corrected;
no implementation repair or implicit bounds handling was added. The initial log
is retained in the machine-readable receipt's evidence hashes.

## Checks and limits

- 109 host library tests and two C/native Rust correspondence tests pass.
- All 31 native GPU library tests pass, including independent dynamic-input,
  non-unit-divisor and zero-divisor enables and command availability.
- Strict Clippy passes with the crate's Rust 1.85 MSRV constraint.
- C/Rust ABI audit: 2,304 values. Surface audit: 166 operations, 151 records,
  242 constants, 1,425 representation values.
- Pinned Vulkan bindings reproduce; shader artifacts are unchanged.
- 21 tooling tests pass, including SDK/source packaging checks.
- Full C GPU regression passes with synchronization validation: zero errors,
  four previously known unused-output warnings outside the new profiles.

Commands use `/tmp/ogpu-c2-tools.sh` for the pinned build environment; the GPU
fixture additionally enables `VK_LAYER_KHRONOS_validation` and synchronization
validation. Relevant commands:

```sh
cargo test --offline --no-default-features -p ogpu
cargo clippy --offline --no-default-features -p ogpu --all-targets -- -D warnings
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
python3 -m unittest discover -s tools -p 'test_*.py'
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --nocapture
cargo xtask foundation
```

Secondary, multiview, tessellation/geometry combinations, generated executable
selection, other hardware and Metal/DX12 remain unverified. No performance parity,
new presentation/human acceptance or external ML/video integration acceptance is
claimed by this step. Remaining M8 implementation is dynamic raster controls and
the extension-format inventory, then geometry capture/dynamic patch state and
consolidation before M9.

## Clean installed acceptance

Implementation `1089406d066ed1b961eeaa1dc9dd322accc78a16` installed with
`source_dirty=false` into `/tmp/ogpu-m8-dynamic-vertex-sdk`. Runtime SHA-256:
`6752f614246fbff87417c71aa6bf8b591e301e1438c71c490c68ab909e1b5e02`.

The optimized C fixture passes against the installed header and shared library,
including all six new fetch profiles and the dynamic-input generated scene. Exact
shared-library exports match the reviewed surface. The separately relocated native
Rust source bundle builds and passes correspondence, missing-loader rejection and
headless execution without using the C SDK. The bundle also contains the new
contract. These checks retain the verification scope and limitations above; this
step does not repeat unrelated external integration acceptance.

The [machine-readable receipt](m8-dynamic-vertex-input-2026-11-07.json) records the
source/runtime identity, counts and evidence hashes, including the initial fixture
failure and successful corrected run.
