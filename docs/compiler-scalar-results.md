# M2 slice 1: FP32 root transport and layout — 2026-09-24

Accepted implementation `c7592de`, with the installed-test command correction at
clean `291eb1fa3b4c76fe6722a70be2cf95b71c84fd31`. The runtime/header stay at ABI 17.
This completes slice 1 of the [M2 brief](compiler-contract-plan.md), not the whole
compiler milestone or a general floating-point conformance claim.

## What changed and what passed

The existing offline adapter now accepts flat FP32 scalar root fields alongside
uint32 fields and scalar device pointers. It checks the reflected type/size/
alignment against SPIR-V and emits C assertions for binary32 host representation,
field offsets, padding, aggregate size and alignment. Unknown scalar widths still
reject. The existing transform and six learned-image generated headers reproduce
byte-for-byte; no arithmetic, compiler flag, shader or runtime change to those paths.

The new affine fixture transforms 4,099 values with three different scale/bias
roots. Powers of two and small binary fractions make every reference result exact,
so it checks transport/layout without a new numerical tolerance. Every output and
both guards are checked after every pass. Roots are overwritten on the CPU after
recording, exercising copied argument bytes. Missing enabled address support and
insufficient root size reject through generated compatibility predicates.

| Layout | Field offsets | Root size/alignment | Local X |
|---|---|---|---:|
| Original | data=0, count=8, scale=12, bias=16 | 24/8 bytes | 64 |
| Reordered | bias=0, scale=4, count=8, data=16 | 24/8 bytes | 32 |

The same C source passes both layouts on Radeon and llvmpipe with Vulkan and sync
validation. Six new rejection/layout tests cover real original/reordered metadata,
host assertions, native/reflected scalar disagreement and size/alignment corruption.
Existing transform compiler checks and learned-image compiler/checker tests pass.
Forty ordinary Rust tests, formatting and strict Clippy also pass; no runtime code
changed after the full ABI-17 GPU/ABI/mock acceptance.

Each driver reruns all 38 small learned-image cases in normal/diagnostic modes,
with original and mutated interfaces: 152 frame executions per driver. Independent
numerical, input/weight integrity, guard and reuse gates pass; original/mutated
output files are byte-identical within each driver. The reported maximum scalar
error is 1.71016513e-7, error/bound 0.00667210166 and RGB code delta 1. These are
the current small fixtures, not a rerun of the historical useful-scale timing or
full hardware/Metal acceptance. Unchanged artifacts/policy did not select a fresh
large-scale/native timing sweep.

## Installed use and performance gate

The new SDK ships `share/ogpu/examples/affine` and the updated optional compiler
adapter. Both independently relocated consumers build/run the supplied header,
reproduce it using installed `ogpu-shader`, reject a stale header after field/local-
size mutation, regenerate and execute the unchanged host program. The same runs
also exercise the existing transform, HOST views and split replay. No checkout,
runtime Rust build or shader compiler is needed just to build/run a supplied C
example; shader mutation explicitly requires the pinned compiler tools.

The first installed run failed because the test omitted the wrapper's required
`--stage compute`. It did not reach affine shader regeneration. Commit `291eb1f`
corrects the invocation and quickstart; fresh installs/runs on both drivers pass.
The original failed log remains `/tmp/ogpu-affine-radv-sdk.log`.

Inspected SPIR-V loads scale/bias directly from the push root and performs one
physical-buffer float load/store with `Aligned 4`, `OpFMul` and `OpFAdd`. Generated
C fields add no per-dispatch interpretation, allocation, repacking or GPU copies;
the runtime receives the same ordinary byte-root contract as before. Native Vulkan
can use that artifact/root directly. This supports the bounded mechanical-interface
choice; it is not a timing comparison or proof that all compiler/native optimization
opportunities are preserved. M2's aggregate, workgroup/subgroup, matrix and graphics
output review remains required by P7.

## Reproduce

Use Slang 2026.14.1, the documented SPIRV-Tools/C/Rust environment and one selected
ICD with synchronization validation:

```sh
python3 examples/compiler/affine.py --check
python3 examples/compiler/affine.py
cargo xtask compiler-workflow --check
cargo xtask learned-image --check
cargo xtask learned-image
python3 tools/install.py --prefix /new/sdk
python3 tools/test-install.py --prefix /new/sdk --affine --shader-check --split --host-view
```

Clean SDK `/tmp/ogpu-sdk-affine-m2-checked`; independent applications
`/tmp/ogpu-external-uxahdhf5` (Radeon) and `/tmp/ogpu-external-y3i2iq3g` (llvmpipe).
Artifact/reflection intermediates are in `target/compiler-affine`; original and
reordered headers are checked against actual compiler output, not hand-authored
layout golden files. Local log hashes, with `/tmp/ogpu-affine-` prefix and `.log` suffix:

```text
build             fce1fb2d240ef984e965b13da34e770534bf6c77761ccac1ea945274bdfae0c5
radv              ae302a9a2b754a476b00aae038d11880066b32a1769dfcd0ea91fa81d4e0568c
lvp               c03ab1f4835d9a5606cdb12083659e6368ab74b750840d3d9291d3926df4f27c
transform-check   1850babf11b02cf14081bf5eb84a2237c8bffcd6e61d698629723dfbbe3f927d
m1-check          64263a51bb953b1292175be5bd7d64c4dbed9db09fec4c8318c8a891aa072641
m1-radv           0e0dca88505a4ab6c69b3e9799c0ef8b272c03cfb1a99d8dc8a2a10db1a61290
m1-lvp            fd9dd4276ed68ea390c9bfcf623a9f17dc7c11818a65439a525914c9f7f95bdf
install-final     c9e3e0c7c884a60aab0f48992adf5e33504bbc8ad95e898b3110564e2f96e646
radv-sdk-final    595acf7aab4901825ff56a9138589d09f974d4077165556aaf2721c89af9fe60
lvp-sdk-final     07f00156c6f79362257f4978224de870f3cb3b916d56c6c472fcb79c8831df2d
```

Next: recursive aggregate layouts and pointer-block metadata. A preliminary pinned
compiler probe validates nested struct/fixed-array/vector SPIR-V, but its JSON
represents a struct pointer's pointee only by name. Do not infer layout from that
name or add a mandatory unused inline copy to force reflection. Obtain and verify
the actual pointee layout as part of slice 2; this observation is not its acceptance.
