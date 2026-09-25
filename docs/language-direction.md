# Language direction: pinned Slang plus an explicit device contract

Accepted 2026-09-26 against clean implementation `8113303`, completing
[M2 slice 5](compiler-contract-plan.md). Runtime and public boundary remain ABI 17.
The [receipt](results/compiler-language-2026-09-26.txt) identifies artifacts,
validation results and all 252 targeted timing samples.

## Decision

Continue with **Slang 2026.14.1 plus the OGPU device-code contract and checked,
offline-generated interfaces**. We are **not building a new language/frontend**
in this roadmap step. Do not add a project device library just to rename native
operations. The existing supported compiler subset is a versioned experiment,
not a stable language standard or a claim that all Slang programs are supported.

The goal remains a general low-level graphics/compute/ML foundation. It is not
reduced to an ML operator library or a Vulkan-only source syntax. Executables,
addresses, roots, explicit dependencies and numerical permissions stay separable
from the source compiler. Native artifacts remain an escape path within the
enabled backend contract, not permission to use unqueried features.

This is an evidence-based current direction, not permanent allegiance. Reopen it
when a named consumer exposes a better API/language alternative: an unavailable
native strategy, irreducible representation or synchronization overhead, necessary
semantic control that cannot be expressed, or a reproducible compiler defect
without an acceptable lowering. A small device library is justified by a concrete
reusable operation with inspectable cost; a frontend needs a separate acceptance
brief explaining what existing compilation cannot preserve.

## What the native output says

The new [probe runner](../examples/compiler/language/README.md) compiles pinned
sources, runs `spirv-val`, checks reflection against native layouts, inspects
storage/scopes/instructions, reproduces accepted headers, and rejects unsupported
installed-generator inputs. These are fixture checks, not an arbitrary-shader
safety verifier or proof of final machine-code quality.

| Idiom | Actual native representation | Preserved choice / limitation |
|---|---|---|
| Structured pointers | 32-byte root, 48-byte block stride; physical pointer loads and address arithmetic; no descriptor lookup or adapter-added transfer | Application chooses root versus pointed-to data. Value-based selector materialization is avoidable in source; see control below. Divergent push-array indexing remains a Vulkan restriction, not a general pointer restriction. |
| Reduction | 24-byte root, 256-byte Workgroup array, 64 invocations; two static control-barrier sites | Explicit shared-memory tree and partial-output placement. Both barriers use Workgroup execution/memory scopes and `AcquireRelease \| WorkgroupMemory` (`2,2,264`), matching the existing GLSL artifact. Static sites are not the dynamic barrier count. |
| Subgroups | Native `OpGroupNonUniformIAdd` twice and `OpGroupNonUniformUMin`, uint32 Reduce at Subgroup scope; lane-index builtin; no workgroup array/barrier | Native arithmetic collective is expressible without fixed width/mapping. Public OGPU cannot yet query the operation mask: diagnostic execution is direct Vulkan, not supported installed-profile execution. |
| Matrix | 48-byte root, two 256-byte Workgroup arrays, 64 invocations; two static barriers matching GLSL | Caller chooses 8×8 tiling, strides, partial-tile guards and FP32 accumulation. This is ordinary matrix arithmetic, **not accelerated/cooperative-matrix support**. |
| Graphics pair | Rootless vertex; 8-byte fragment root; location-0 FP32 float2 with smooth interpolation, location-1 uint32 with Flat | Native stage IO/interpolation, checked offline; no runtime stage adaptation, linking, copies or barriers. The accepted bounded pair is not a full graphics profile. |

All three new compute roots are independently matched to the manually declared
consumer structs, including size/alignment/offsets and physical scalar-pointer
strides. No ShaderInt64 arithmetic is imposed merely by 64-bit physical pointers.
No descriptor-set bindings, function calls, memory-copy instructions or extra
memory-barrier instructions appear in those three artifacts. Function-local
variables do appear; SPIR-V alone does not say whether the driver uses registers
or spills. Absence of `OpCopyMemory` is not a proof that no source value is copied.

The installed adapter still rejects workgroup storage and subgroup capabilities.
That is a tooling limitation, not evidence Slang cannot express them. Workgroup
reduction and matrix artifacts execute through the current native-SPIR-V public
path. The subgroup case also has a **public capability-discovery gap**; do not
hide that distinction behind an adapter limitation or infer arithmetic support
from the Vulkan version. Vulkan exposes subgroup stages/operations through
`VkPhysicalDeviceSubgroupProperties`; arithmetic is not the basic mandatory
operation set. [Vulkan subgroup guide](https://docs.vulkan.org/guide/latest/subgroups.html)

An initial implicit subgroup-profile upgrade warning is resolved by explicitly
selecting `spvGroupNonUniform+spvGroupNonUniformArithmetic`. The runner checks the
emitted capability set and rejects unexpected compiler diagnostics. A profile
label alone is not executable-requirements metadata. Workgroup control/memory
barriers and subgroup operations remain distinct in the device contract, as in
Slang's [barrier](https://docs.shader-slang.org/en/latest/external/core-module-reference/global-decls/barrier.html)
and [wave](https://docs.shader-slang.org/en/latest/external/core-module-reference/global-decls/wave.html) interfaces.

## Structured-value cost control

The accepted structured source copies `Coefficients` to a value before dynamically
indexing its two selectors. Native output loads both selectors, constructs the
array, stores it in Function storage and reads the selected element. This is not
an adapter-generated GPU allocation or transfer, but it is a potential compiler
cost worth checking rather than claiming zero overhead from the interface alone.

An alternate source uses `Block*` and `Coefficients*` and accesses the selected
member directly. It has the same checked layout, three physical pointer-access
instructions, no Function array, and no new feature or runtime change. Both
sources pass the unchanged structured C consumer's non-identity A/B/A cases on
both drivers. Equivalence is claimed for this race-free, disjoint-parameter
fixture, not arbitrary aliasing code with intervening writes.

A separate timing control uses both artifacts with identical public host policy,
runtime identity coefficients and one dispatch per timed batch. Each of three
fresh Radeon processes alternates AB/BA order, with three warmups and 21 samples
per variant/extent. Every sample checks all output bytes, inactive suffix, guards
and parameter bytes outside the device interval. Validation is disabled only in
these timing processes. Median of per-process medians:

| Elements | Value-based source | Direct pointers |
|---:|---:|---:|
| 4,099 | 14.680 µs | 14.640 µs |
| 1,048,576 | 971.160 µs | 975.360 µs |

No value-materialization penalty is demonstrated by this control; removing the
Function array is not a measured optimization here. Differences are small and
this does not isolate register allocation, spills, alias analysis or individual
instructions. No affinity/clock pinning or exclusive-machine control. Do not
substitute this compiler-source comparison for native-Vulkan API timing, general
Slang quality, or a ban on different native implementations. Both strategies are
expressible through the same contract; retain the accepted fixture unchanged.

## Alternatives considered

1. **Pinned Slang + contract + checked adapter — selected.** It expresses these
   pointer, workgroup, subgroup, matrix and graphics idioms while leaving memory
   and scheduling policy with the application. Compiler requirements and native
   restrictions still need explicit checking; version pinning is not correctness.
2. **Project device library/profile — only the explicit contract/profile today.**
   There is no demonstrated missing operation that a wrapper library solves.
   A reusable tiled kernel/helper may be useful for M5, but must expose strategy,
   synchronization and numerical permissions rather than mandate one tiling or
   subgroup size. Extend generated requirement checks when that consumer needs it.
3. **Project-owned frontend — not selected.** These probes expose tooling and
   capability-query work, not an irreducible source-language limitation. A frontend
   would still need native lowering, optimization, reflection and conformance.
   It is not justified by replacing syntax alone, nor ruled out for future needs.

## Acceptance and limits

- Both drivers pass the 1,048,579-element multi-level reduction, exact sum
  `3717237828`, and ten guarded boundary-count cases checking every partial.
- The existing matrix consumer checks 50 shapes/patterns per kernel in both
  timed/untimed modes, including input integrity, padding and guards. Both kernels'
  maximum error/bound is `0.0159155`; no tolerance changes. Its incidental timings
  are not a matched GLSL-versus-Slang or native-API comparison.
- Six subgroup cases per driver include padded launches and A/B/A reuse. The
  native helper queries compute BASIC/ARITHMETIC support first. Reported sizes
  are 64 on Radeon and 8 on llvmpipe. Independent sums, membership counts, unique
  bounded lane IDs, workgroup containment, unchanged input and guards pass without
  assuming contiguous subgroup membership. Unsupported is not a passing skip.
- Original/direct structured consumers pass; accepted structured and graphics
  headers reproduce. All four graphics interface variants pass again. Twelve
  probe tests, sixteen transform checks, forty ordinary Rust tests, strict Clippy,
  formatting and three C static analyses pass. Nix's analyzer wrapper emits only
  unused-linker-option warnings.
- All 38 small M1 cases in both interface variants and normal/diagnostic modes
  pass on both drivers (152 frames each), with byte-identical original/mutated
  output: max float error `1.71016513e-7`, error/bound `0.00667210166`, RGB delta 1.
- Installed/relocated support is inherited from accepted slices 1–4, most recently
  `839edab`; no installed code, runtime, public header or accepted source changed.
  These intentionally unsupported probes are not advertised as an SDK extension.
  The full 28 runtime GPU tests, ABI suite, useful-scale timing and SDK install were
  not rerun for this examples-only change. No new Metal or second physical GPU claim.

This completes the bounded M2 language decision, **not universal P7/performance
approval**. Before M5 selects subgroup algorithms, add an explicit queried/enabled
subgroup requirement vocabulary and its rejection tests, then extend the installed
adapter for the selected workgroup/subgroup subset. Accelerated matrix formats,
shapes, numerical modes and optional enablement belong to a separate M5 profile;
the available GPU need not provide matrix hardware to make progress. Native Metal
source generation and cross-backend semantics remain M6. M4 expands graphics stage
and resource contracts with its selected renderer. M3 resource/reuse work is next.
