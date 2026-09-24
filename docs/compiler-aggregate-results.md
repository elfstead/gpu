# M2 slice 2: aggregate and pointed-to layouts — 2026-09-24

Accepted at clean `c63b0146112e68100b3827f5bffa063c48f9ff94`. The runtime/header
remain ABI 17. Nested structs, fixed arrays, uint32/FP32 vectors and named struct
pointers now pass the bounded [M2 slice-2 gates](compiler-contract-plan.md).
This is not a universal C/device ABI, general shader sandbox or language decision.

## Interface and evidence

`layouts.py` replaces the flat-only layout walker. It checks native types, member
names/offsets, array counts/strides, pointer strides, reflected sizes/alignments and
C representability recursively. Generated array/vector typedefs and aggregate
members have size/alignment/offset/stride assertions; GPU pointers remain uint64
addresses, not C pointers or owners. Existing scalar generated headers reproduce
byte-for-byte, including the learned-image application.

Pinned Slang JSON gives a struct pointer's pointee name but not its layout. The
adapter runs a separate `-no-codegen` query including the source and declaring the
named types for reflection. Original parameter metadata must remain identical.
Query-only parameters never enter the validated original SPIR-V, generated root
or device upload. Actual original pointee offsets and pointer/array strides must
match the reflected query. Missing, recursive, ambiguous or unused metadata rejects.
No name-based layout guess, source-language parser or runtime reflection dependency.

The consumer has two guarded float arrays and two contiguous parameter blocks,
each containing a GPU address, an array of nested coefficient structures, a count
and flags. Roots contain nested controls and an address to those blocks. Three
A/B/A passes change pointed-to coefficients and root selection; every value, both
data guards and the entire parameter allocation/guards are checked after each pass.
All reference values are exactly representable. Both pointee allocations and the
parameter allocation are explicitly retained; no inferred transitive ownership.

| Variant | Root size/alignment | Block stride | Local X | Change |
|---|---|---:|---:|---|
| Original | 32/8 bytes | 48 bytes | 64 | Nested float2/uint arrays and block array |
| Reordered | 32/8 bytes | 48 bytes | 32 | Root, controls, block and coefficient field order |
| Vector3 | 48/8 bytes | 48 bytes | 64 | Wider root vector, explicit reserved fields/native-compatible packing |
| Vector4 | 48/8 bytes | 48 bytes | 64 | Wider root vector, explicit reserved fields/native-compatible packing |

All four variants execute the same C source on Radeon and llvmpipe with Vulkan/
synchronization validation. Wider-vector cases test layout/stride and transport of
the first two lanes used by this algorithm; they are not vector-arithmetic tests.
Twenty new layout/rejection tests pass, including corrupted nested fields, strides,
counts, pointee metadata, cycles, root-index uniformity and illegal native packing.
The corrected direct-Vulkan control independently checks the artifact/root and
second-block pointer stride on both drivers, with 65 values and guards. It has no
OGPU calls/linkage. No timings or general performance parity claim are attached.

The independent SDK is installed at this clean revision, relocated to a path with
spaces and used without the checkout's headers/library/build outputs. On both
drivers its structured consumer builds/runs, reproduces through the installed tool,
rejects a stale header, reverses all four structures' fields and changes local size,
regenerates and executes unchanged C code. Affine/transform compiler mutations and
HOST-view/split paths also pass. This is not an Internet clean-machine install.

## Failure found and corrected

The first fixture used `args.controls.mapping[id.x % 2]`. It passed `spirv-val`
and llvmpipe execution but produced an incorrect odd-lane coefficient on Radeon:
first discrepancy -63.5 versus expected 255. A direct-Vulkan control using the same
artifact reproduced 32/65 mismatches with intact guards.

That access violates Vulkan's requirement for dynamically uniform push-array
indices. It is an invalid fixture, **not evidence of an OGPU or Radeon driver bug**.
Validation silence and software-driver success do not make it valid. The corrected
source reads the two constant-indexed root values, then selects per invocation;
pointed-to coefficient/selector arrays still use divergent indices.
[Vulkan push-constant interface](https://docs.vulkan.org/spec/latest/chapters/interfaces.html#interfaces-resources-pushconst)

The adapter now rejects root access indices whose uniformity cannot be proven by
its bounded constant/root-load/arithmetic analysis. The rejected original is rebuilt
by a negative test and never submitted. This is conservative tooling, not complete
dataflow verification: unknown function/phi/data flow rejects even if a legal native
program could establish uniformity. Keep that limitation separate from the runtime
and final language design. No shader rewrite, driver fallback or tolerance relaxation.

A wider-vector probe also exposed illegal C packing under the current native
layout rules. `spirv-val` rejected it before execution. The positive fixtures use
explicit reserved fields/native-compatible placement; the negative case remains a
test. Neither `scalarBlockLayout` enablement nor a permissive validation flag was
added. “C layout” is not itself proof of valid Vulkan block layout.

## Regression, performance gate and remaining limits

All 38 learned-image small cases pass in normal/diagnostic modes with original and
mutated interfaces on both drivers (152 frame executions per driver). Within each
driver the interface variants produce byte-identical outputs. Maximum scalar error
is 1.71016513e-7, error/bound 0.00667210166 and RGB code delta 1, unchanged from
slice 1. Forty ordinary runtime tests, formatting and strict Clippy pass. Existing
transform/affine headers and shader/interface checks reproduce. No runtime change
selected a fresh full GPU/ABI/mock or useful-scale timing reacceptance.

The generated C arguments are native-compatible storage, with no mandatory host
repacking, per-dispatch interpretation, new allocation, GPU copy or synchronization.
The legal native control uses the same artifact and argument representation.
The shader's source-level temporary coefficient value can introduce private local
array operations in emitted SPIR-V; that is compiler/source behavior, not generated
host glue or a runtime requirement. M2's later compiler-output review must still
consider such costs before the language-direction decision. This slice does not
close P7 or claim equal performance for all aggregate access patterns.

Limits: simple unqualified pointee names, finite nonrecursive C-layout structures,
uint32/FP32 fields and scalar/struct pointers; no matrices, runtime arrays, arbitrary
scalar widths, namespace/generic pointee lookup or arbitrary uniformity proofs.
Trusted compiler input remains assumed. Native Metal, generated heaps/stage pairs
and transitive source dependency tracking remain separate. Next is slice 3's
generated image/sampler and graphics interfaces.

## Reproduction and retained receipts

With Slang 2026.14.1, the documented toolchain and one selected validated ICD:

```sh
python3 examples/compiler/structured.py --check
python3 examples/compiler/structured.py
cargo xtask compiler-workflow --check
python3 examples/compiler/affine.py --check
cargo xtask learned-image
python3 tools/install.py --prefix /new/sdk
python3 tools/test-install.py --prefix /new/sdk --structured --affine --shader-check --split --host-view
```

Direct native diagnostic build (then run `target/structured-native` with that ICD):

```sh
cc -std=c11 -O2 -Wall -Wextra -Werror -Iexamples/compiler -Iinclude \
  -Ivendor/Vulkan-Headers/include -Iexamples/learned_image/generated \
  examples/compiler/structured_native.c -ldl -o target/structured-native
```

SDK `/tmp/ogpu-sdk-structured-m2`; relocated consumers `/tmp/ogpu-external-6sh43upv`
(Radeon) and `/tmp/ogpu-external-vx3gtoj4` (llvmpipe). Intermediates in
`target/compiler-structured` include original reflection, separate query metadata,
merged checked interface and per-variant shader artifacts. Local log hashes with
`/tmp/ogpu-structured-` prefix and `.log` suffix:

```text
radv-final          819ac71e8241671825b017b229a156fb40e7fa0bc0b4a3584c0afad1f8803790
lvp-final           ab84013d13e7e8eae3c8bd250b82a0a93fd91988d1c5fe88bdce98e05ef8361e
native-radv-final   b08e3d306d6d3a37a2fc2f039859e318d70974a3a30c12fed1c368783af688dc
native-lvp-final    befe7a8d23431c731dcbb2409882127b2de2ce2362d9b230e0d06dbb0c944850
m1-radv             d4d819b962efe3f9e67d49c2ac3a1145963418ca9e4fd833ee47f6126ce5f98b
m1-lvp              1802d0b1ce6aadfa3f6bfa6ae9b671b5554cc0f5de15c8194007fc8928afa15c
install             e4339886a8002a5e2520bb21fd7f86409c9fd431c50a7d521405bcba59423acc
radv-sdk            1349c115174c0f8307ccbc225c09908a4045594301dce327a6cb426abedcf247
lvp-sdk             2aa3e0bedb74657295310c2b814b53c66798deb9d26124648d8033ead1aa30b7
radv-diagnostic     8176b473c551394a09227c8ab25370752aed98d85081a7e4e882adb13989a2e5
native-radv         98066c1b643753e6647fd58c956da7e486d092c5f13229d0a30d444d935b239f
```

The last two are rejected-fixture diagnostics, not accepted correctness evidence.
