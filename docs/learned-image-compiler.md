# Compiler-owned interfaces for the learned-image application

Selected 2026-09-17, after the [Vulkan application acceptance](learned-image.md).
This brief precedes migration and results. The model, fixtures, tolerances,
memory placement and application execution policy stay unchanged.

## Scope and decision

Extend the existing pinned Slang/offline C generator to the six executable
stages used by the application: four compute shaders (including diagnostic
poisoning), fullscreen vertex, and display fragment. The required additions are
flat FP32 pointers, multiple independently named artifacts, graphics entry
interfaces, and an executable without push constants. Do not add host tensors,
runtime reflection, shader caching or a general shader package format.

Keep the already accepted GLSL path as a migration control in git history and
the recorded results, not as a second permanently maintained implementation.
Use Slang sources and generated embedded-artifact headers once acceptance passes.
Compiler-derived mechanics include root layout/padding, stage, local dimensions,
enabled requirements, entry and artifact coupling. Application-owned semantics
include pointer extents/lifetimes, logical work size, storage, numerical policy,
barriers and draw target format.

## Gates

- Preserve all 38 normal/diagnostic application cases, intermediate scalar
  checks, quality gates, guards and A/B/A reuse on Radeon and llvmpipe.
- Validate SPIR-V and cross-check reflection against actual root field types,
  offsets, entry stage, inputs/outputs, local size and module requirements.
  Reject unsupported interfaces and unknown capabilities rather than guessing.
- Generate distinct C identifiers for each executable. No hand-maintained C
  root structs, local-size constants, shader file lists or embedded-artifact
  size declarations remain in the application.
- Reorder root fields and change compute local size from 64 to 32, regenerate,
  and run the unchanged host application against the same frozen fixtures.
  The mutation includes graphics roots, not only compute.
- Reproduce checked-in headers byte-for-byte with a no-GPU check. Cover malformed
  reflection/SPIR-V, float-pointer mismatches, graphics-stage disagreement and
  absent enabled capabilities in tests. Preserve the original integer-transform
  compiler consumer and its existing rejection/mutation tests.
- No runtime/public API changes unless this workload exposes a better API
  alternative. No Metal execution claim or new Mac validation round trip.

Stop when the application uses generated executable mechanics and both its
original and mutated interfaces pass. Document the supported compiler subset and
remaining application obligations; do not expand into general language design.

## Result — 2026-09-17

Complete at `e363377`, with the brief committed first in `3e3a041`. The unchanged
model, reference recipes and all numerical/quality/lifecycle gates pass on
RX 5700 XT / RADV and llvmpipe. Each driver executes 38 cases in normal and
diagnostic modes for each interface variant: 152 frame executions per driver.
Original and mutated artifacts produce byte-identical output files within each
driver, including intermediate activations. This does not assert byte equality
between drivers or between different source compilers.

| Driver | Maximum scalar error, both variants | Maximum error / allowed bound | Maximum RGB code difference |
|---|---:|---:|---:|
| Radeon / RADV | 2.2891e-6 | 0.08458 | 1 |
| llvmpipe | 1.1085e-6 | 0.03342 | 1 |

Alpha remains 255; guards, unchanged inputs/weights and all ten A/B/A reuse groups
pass. The maxima equal the earlier GLSL acceptance maxima; acceptance is against
the independent scalar oracle, not newly generated GPU golden outputs. The
[receipt](results/learned-image-compiler-2026-09-17.txt) records the exact summaries.
Vulkan and synchronization validation were enabled. Timings remain incidental
correctness-run instrumentation, not a compiler performance comparison.

The application no longer defines C root structs, hard-codes workgroup dimensions,
loads separate shader files or maintains argument offsets. Generated headers
embed each executable with its checked interface, stage, push size, dimensions
and enabled-capability predicate. They are regenerated/checkable byte-for-byte.
Application arguments use designated semantic field names; it still owns logical
extents, allocation sizes, pointer lifetimes, numerical interpretation, barrier
placement, rendering format and the dispatch/draw sequence. A generated local
size does not infer the desired logical problem decomposition.

All five roots are reversed in the mutation, including the fragment root; all
four compute local sizes change from 64 to 32. The rootless vertex interface has
nothing to reorder. Both variants compile the exact same `app.c`. Missing enabled
address/compute/graphics capabilities reject through generated predicates before
executable creation; the address-free vertex correctly needs no address capability.

Thirty-six compiler/interface checks pass (16 original, 20 added), alongside
13 fixture tests and four numerical-checker tests. The original integer-transform
consumer and its layout/local-size mutation still execute on both drivers.
No-GPU header reproduction/build, 37 ordinary Rust tests, strict Clippy,
formatting, 749 ABI checks and all 21 existing GPU tests on each driver pass.
The frozen model SHA-256 remains
`59ca8473cfce3cc205fce4fc88282e1f99d1883de951b6dfd320161ad0e1a7a0`.
Training, GGML/libplacebo and native Metal were not rerun for this compiler-only
migration. No runtime code, public API or ABI changed.

**Decision:** retain the shared offline compiler adapter and remove duplicated
host executable mechanics. The workload has not exposed a better runtime API
alternative. Five superseded application GLSL sources were removed; their
accepted implementation remains in git history at `21dc090`. The shared GLSL
fullscreen shader used by other examples is unaffected. Do not maintain parallel
GLSL/Slang versions of this application or start tuning its tiny network.

## Supported subset and boundaries

All artifacts have one entry named `main`. Roots, where supported, are flat C-layout
structs of uint32 fields and device pointers to uint32/FP32 scalars. Pointers become
uint64 GPU addresses, not owners. The adapter cross-checks pointed-to SPIR-V scalar
type and four-byte stride as well as field offsets, padding and reflected size.

| Stage | Supported input/output | Root and launch |
|---|---|---|
| Compute | uint3 dispatch ID; no stage output | One root; fixed reflected XYZ local size |
| Vertex | Vulkan vertex index → float4 position | No root, resources, varyings or workgroup size |
| Fragment | float4 fragment coordinates → float4 color at location 0 | One root; upper-left origin, no additional execution modes |

The vertex source uses `SV_VulkanVertexID` to match the original GLSL
`gl_VertexIndex` semantics. An initial `SV_VertexID` compile emitted BaseVertex
and DrawParameters, which are outside this adapter's contract. Slang documents
the [explicit Vulkan index semantics](https://github.com/shader-slang/slang/blob/master/docs/user-guide/a2-01-spirv-target-specific.md).
The final SPIR-V needs only Shader for vertex, and Shader plus physical storage
addresses for the other five modules; no runtime feature enablement was added.

Reject unknown capabilities/extensions, specialization, shared/global resources,
descriptor/heap bindings, extra entry inputs/outputs/builtins, unsupported execution
modes, nested/array/vector roots and other pointer/scalar field types. Optional
Float16 requirement mapping from the original adapter remains, but this workload
does not use it or imply a numerical-policy decision. Stage tags and helper names
are generator-local, not a new public runtime ABI.

This is not a general graphics linker: no arbitrary vertex attributes, inter-stage
varyings, multiple render targets or common push roots across both graphics stages.
It is also not a full SPIR-V parser or shader sandbox. It relies on trusted pinned
compiler output plus `spirv-val`; caller lifetime, aliasing, address bounds and
algorithm semantics remain caller obligations. Slang reflection JSON and generated
C helpers are not promised stable across compiler versions.

## Reproduce

Use Python 3 standard library, Slang **2026.14.1**, SPIRV-Tools with Vulkan 1.4
support, and the normal Rust/C environment. No download is automatic.

```sh
SLANGC=/path/to/slangc cargo xtask learned-image --check
SLANGC=/path/to/slangc cargo xtask learned-image
SLANGC=/path/to/slangc cargo xtask compiler-workflow
```

Select the driver and validation environment as in the
[application instructions](../examples/learned_image/README.md). Ordinary runs
regenerate checked-in original headers; `--check` requires byte-identical originals
without replacing them and compiles both host variants without GPU execution.
Mutation sources/reflection/headers remain in `target/learned-image/compiler`.
Serialize runs sharing generated files. The application-owned source list uses
the shared adapter; there is no second reflection parser or runtime dependency.

The next large milestone is this same application's Metal graphics path, preserving
the frozen fixture and ownership/synchronization model. It needs native validation
before any portability claim. This completed Vulkan/compiler milestone does not
itself trigger another Mac round trip or stabilize a language/runtime standard.
