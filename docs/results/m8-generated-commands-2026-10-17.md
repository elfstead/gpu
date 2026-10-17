# M8 GPU-generated commands: initial native tranche

Date: 2026-10-17. Linux Vulkan implementation, not M8 completion or Metal parity.

## Contract and implementation

- Optional `GENERATED_COMMANDS` enabling and `QUERY_GENERATED_LIMITS`: native
  stages, pipeline-selection stages, token support and layout/sequence/table limits.
  Availability is not enabling. Compute does not require the raster feature.
- Cold, immutable layouts describe caller-chosen offsets and stride, optional
  executable selection, inline argument bytes, sequence identity and one final
  dispatch/draw/indexed/mesh action. Nested draw-count actions are represented.
  Zero/overlapping source strides are preserved; nothing repacks GPU records.
- Executable sets expose explicit slot updates. Only executables prepared with
  `COMPILE_INDIRECT_BINDABLE` may be inserted. There is no implicit recompilation.
  Fixed-executable work does not need a set. Compatible graphics state/outputs
  across set members are a caller obligation, not shader reflection at recording.
- Explicit requirements query reports preprocessing size, alignment and compatible
  memory types. The caller intersects these with ordinary backing requirements,
  allocates storage and owns its lifetime. No mandatory prepared-plan object or
  hidden storage cache sits between the query and recording.
- Separate preprocessing and native integrated preprocessing are distinct choices.
  Both take explicit storage and optional GPU count spans. Preprocess stage/access
  bits expose the dependencies; OGPU inserts no barriers or queue submissions.
- C and native Rust expose the same operations below the C boundary. Recording
  checks fixed metadata/ranges and emits one native command per operation: no host
  allocation, requirements query, CPU command expansion or GPU readback. Cold
  layout/set creation and updates may allocate host storage.
- The caller guarantees execution-time counts, indices, dimensions, storage
  requirements, state compatibility and resource lifetimes. OGPU checks host-known
  structure, same-device usage, enabled features and span bounds without scanning
  GPU data. Invalid recording poisons the list under the existing error model.
- Executable selection invalidates the tracked pipeline binding; subsequent work
  must explicitly bind again. Generated inline updates leave argument contents
  undefined afterward. No implicit state restoration is recorded.

This maps to native
[device-generated commands](https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_device_generated_commands.html),
not a general GPU scheduler or arbitrary GPU command buffer. The Vulkan generated
mesh-token profile requires **both** mesh and task features enabled, even for a
mesh-only executable, per the native
[token validity rules](https://docs.vulkan.org/refpages/latest/refpages/source/VkIndirectCommandsLayoutTokenEXT.html).
That requirement does not change ordinary mesh draws.

Ordinary direct/indirect commands remain available: generated command preparation
is a genuinely different native strategy, not the default implementation of every
draw. Preserving these strategies is the performance argument here; this tranche
does not provide timings or establish speed parity with direct Vulkan.

## Evidence

`examples/foundation_generated.h` and `foundation-generated.slang` are included in
`cargo xtask foundation` and pinned shader reproduction/validation. A producer
shader writes executable indices, pointer/inline payloads, dispatch dimensions and
counts. Generated work consumes these without intermediate host readback.

Four modes combine fixed/table-selected executables with separate/integrated
preprocessing. Five runs each cover zero, one and two GPU-selected sequences,
an absent count span, specialization-dependent output, native sequence identity
and changed set contents. Independent expected bytes cover all 4 KiB of storage,
including the generated wire records, outputs and untouched guards.

Duplicate/out-of-range set writes reject; later GPU output also checks that the
rejected duplicate update did not partially change a slot. Twenty poisoned
recordings cover missing bindings, short argument/count spans, zero sequence
capacity and mismatched preprocessing modes. No invalid list is submitted.

| Local driver | Discovery and execution |
| --- | --- |
| Radeon RX 5700 XT / RADV | Generated compute/vertex/fragment stages; executable selection **compute only**. All 20 compute submissions and 20 rejection cases pass. |
| llvmpipe 26.2.1 / LLVM 21.1.8 | Generated compute/vertex/fragment/task/mesh stages and corresponding executable selection. Same compute fixture passes. This is software-driver evidence, not physical mesh execution. |

Both full foundation regressions pass with Vulkan validation and synchronization
validation enabled. llvmpipe uses the existing
`OGPU_FOUNDATION_STENCIL_RASTER_ONLY=1` exclusion for unrelated combined
depth/stencil plane copies; no generated-command check is excluded. Fixture marker:

```
GENERATED_PASS submissions=20 modes=4 gpu_choice=1 gpu_count=1 gpu_arguments=1 sequence_identity=1 set_update=1 guards=1 rejected_recordings=20
```

Host checks cover layout/action families, offsets/argument overlaps, stride,
optional enabling, scopes/accesses, exact query schema and C/Rust correspondence.
Native setup covers disabled-feature rejection and layout creation for advertised
compute, graphics, mesh and nested-count profiles. That is layout evidence only,
not graphics/mesh execution. Final checks: 67 host tests plus two surface checks;
21 headless native tests on each driver; 1,342 C/Rust/native ABI layout values;
pinned bindings/shaders, Clippy, formatting and C11/C++17 header syntax.

## Remaining work and proof

1. Execute generated graphics, indexed/mesh and nested-count actions, starting with
   the GPU-culled scene. Check render state, identity, attachments, heap/root data
   and pipeline-selection invalidation. Radeon cannot validate graphics executable
   selection; fixed graphics work is a separate supported profile.
2. Add vertex/index-state tokens alongside the remaining vertex-fetch/raster work.
   Current indexed generated work uses explicitly bound index state. Ray tokens
   belong with M9; shader-object selection is not implemented or emulated.
3. Exercise distinct preprocessing/state encoders, cross-queue dependencies,
   storage reuse, repeated execution, query interactions and richer graphics set
   compatibility. The current fixture uses one primary encoder/queue and fresh
   preprocessing storage per run.
4. Compare representative generated workloads with the same native Vulkan strategy
   and inspect any measured difference. No claim about dispatch throughput,
   preprocessing amortization or driver-independent performance follows from the
   small correctness fixture.
5. Metal foundation remains deferred until a Mac is available. Unsupported native
   strategies must remain explicit, not replaced with host expansion or readback.

The complete experimental contract is in `include/ogpu_next.h`; pin the source,
header, library and shader artifacts together. This is not a stable ABI promise.
