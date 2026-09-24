# Compiler-facing workflow experiment

Selected 2026-09-16 after the numerical executable experiment. This brief precedes
implementation and results. No new source language, runtime reflection system,
public API, or Metal round trip is selected.

## Question and bounded consumer

Can a compiler supply the mechanical host/shader interface while leaving memory
ownership, synchronization and semantic policy with the application?

Use the existing integer transform (`x = x*3 + 7`) from `examples/compute.c`:
4,099 integers, three dispatches, partial upload, and independent CPU results.
Add prefix/suffix guards. Preserve parent-handle destruction coverage. This is
deliberately a small existing workload, not a new algorithm benchmark.

Pinned Slang 2026.14.1 already emits JSON reflection for push-constant pointer
fields, scalar fields, root size/alignment, entry point and local dimensions.
Its [reflection interface](https://shader-slang.org/slang/user-guide/reflection.html)
does not relieve us of checking the executable or handling unsupported layouts.
Use direct SPIR-V plus C layout, as in the existing heap compiler path.

## Boundary and alternatives

- Hand-maintained C roots/requirements: current control; duplicates compiler data.
- Runtime reflection and dynamic argument packing: unnecessary runtime dependency
  for this workload; deferred.
- Offline generation from reflection with artifact checks: selected.

Generate one C header containing the argument struct, explicit padding, static
layout assertions, workgroup dimensions, shader description, required enabled
capabilities and embedded SPIR-V. Embedding couples metadata and executable; do
not invent an independently versioned package format or cache. A source digest
and compiler version identify inputs, not compatibility across toolchain versions.

Cross-check reflection field types/offsets, entry point and local size against
the emitted SPIR-V. Requirements come from the actual module capabilities with
an explicit small allowlist. Reject unknown capabilities, resources/bindings,
specialization, shared memory, nested/array/vector roots and unsupported layouts;
do not guess or emit an incomplete contract. Device limits and capability checks
must happen before kernel creation. Pointer fields become uint64_t GPU addresses,
not CPU pointers or owners. The supported module has no shared memory.

The application still supplies pointer validity, allocation bounds, buffer
lifetimes, inter-dispatch dependencies, logical dispatch extent and algorithm
semantics. Compiler workgroup size does not tell us the problem size. This
integer-only workload does not infer numerical permission from capability bits.

## Gates and stop condition

One command regenerates and checks the interface, builds the C consumer and runs
the established workload. A no-GPU check rebuilds and byte-compares generated
output. Generator tests corrupt reflection/entry/local-size/types/capabilities
and verify rejection. C static assertions check the host layout. A deliberately
missing capability rejects compatibility before dispatch. Changing field order
and local size must regenerate a matching interface and run the same host code.
Original compute controls and ordinary/ABI checks must still pass.

Stop with this consumer running on Radeon and llvmpipe, reproducible commands,
an explicit supported generator subset and an assessment of whether a better
runtime API alternative emerged. No performance target, Slang adoption decision,
cross-language universal ABI or new Metal execution claim follows.

## Result — 2026-09-16

Completed at `66854db`, with the brief committed first at `0d58358`. The same C
consumer passes on RX 5700 XT / RADV and llvmpipe with Vulkan/synchronization
validation, both for the original source and for a generated field-order/local-size
mutation. Each executes 4,099 integers across three passes, checks all results and
guards, performs a partial upload and destroys parent handles before child work
finishes. The original `cargo xtask compute` control passes on both drivers.

The original root has a pointer at offset 0 and count at offset 8, with local size
64. The mutation moves count to offset 0 and the pointer to offset 8, with local
size 32. Both roots occupy 16 bytes with different explicit padding. No host source
change is needed. Generated static assertions verify field offsets, size and
alignment; the host derives group count from generated local size and its own
logical element count. Both modules require only Shader and physical addresses.

Sixteen generator tests pass, including offset/type/entry/local-size disagreement,
unmapped capability/extension, descriptors, shared storage, specialization,
unexpected builtins/execution modes, duplicate roots and inconsistent reflection.
The optional Float16 mapping is a synthetic metadata test, not a new arithmetic
execution claim. `--check` reproduces the checked-in header byte-for-byte and
compiles both layouts without GPU execution. The 37 ordinary Rust tests, strict
Clippy and 749 C/Rust layout checks also pass. Runtime and public API are unchanged;
the full consumer/GPU suites were not rerun for this build-tool-only change.

**Decision:** keep this adapter outside the runtime. The experiment removes
duplicated layout, entry, local-size and capability declarations without exposing
a better runtime API alternative yet. It supports a compiler-owned mechanical
interface and application-owned execution policy. Do not generalize it into a
universal shader package, promote its generated helper names to public API, or
treat the pinned reflection JSON as a stable format.

## Reproduce and use

Requirements: the normal Rust/C environment, Python 3 (standard library only),
Slang **2026.14.1**, and SPIRV-Tools with Vulkan 1.4 validation. The existing
[Slang archive/version/checksum record](heap-images.md#shadertoolchain-gate) applies.
No compiler is downloaded automatically. Leave `CARGO_TARGET_DIR` unset, and
serialize runs sharing this checkout's generated/build files.

```sh
SLANGC=/path/to/slangc cargo xtask compiler-workflow
SLANGC=/path/to/slangc cargo xtask compiler-workflow --check
```

The first command compiles `examples/compiler/transform.slang`, emits reflection,
validates SPIR-V, regenerates `transform.generated.h`, runs rejection tests,
compiles the release runtime/C consumer, then runs original and mutated layouts.
The second checks the checked-in header instead of replacing it and does not
execute GPU work. Generated intermediates and the mutated fixture live in
`target/compiler-workflow`; only the original combined header is checked in.
Use `VK_DRIVER_FILES` and the normal Vulkan validation environment to select and
validate a driver. The runner selects its release library ahead of Cargo's
inherited debug-library directories.

For generator-only use:

```sh
SLANGC=/path/to/slangc python3 examples/compiler/generate.py
SLANGC=/path/to/slangc python3 examples/compiler/generate.py --check
```

The [consumer](../examples/compiler/consumer.c) uses `TransformArguments`,
`transform_shader()`, `transform_local` and `transform_compatible()`. It names
fields by their semantic role (`arg_data`, `arg_count`), not their byte offsets.
It explicitly creates buffers, obtains GPU addresses, records dependencies,
retains allocations and waits for completion. There is no generated hidden
allocation, migration, dispatch, graph scheduler or numerical policy.

## Exact limits and remaining obligations

The original experiment above covered one compute entry, a flat root of uint32
fields/device uint32 pointers and dispatch ID input. The subsequent
[learned-image migration](learned-image-compiler.md) extends this same adapter to
FP32 pointers, independently named artifacts, a rootless fullscreen vertex entry
and a display fragment entry. It does not introduce another parser or runtime
dependency. That page owns the current exact stage/interface subset.
M2's [scalar-root slice](compiler-contract-plan.md) additionally supports FP32
scalar fields through `python3 examples/compiler/affine.py [--check]`. It emits
host representation assertions and validates original/reordered layouts with
exact binary32 scale/bias results; aggregate roots remain the next slice.

Supported capability mappings remain Shader, PhysicalStorageBufferAddresses and
Float16; Shader maps to the corresponding enabled compute/graphics profile.
Unknown capabilities/extensions are rejected. Float16 permission is not a claim
about precision semantics. Shared storage, descriptor bindings, specialization,
nested/array/vector roots and other scalar/pointer types remain unsupported.

SPIR-V field offsets/types, entry and local size are cross-checked against Slang
reflection. Trailing struct padding comes from the reflected C layout, checked
against supported field alignment and then by the C compiler; SPIR-V alone does
not provide that complete host struct size. Embedded words tie the checked
artifact to its declarations. The source hash is provenance, not a security or
cross-version compatibility mechanism. Generation assumes trusted compiler output
and uses `spirv-val`; the adapter itself is not a full SPIR-V parser or sandbox.

The caller must still establish allocation extent, pointee alignment, lifetime,
aliasing, access order, logical extent and algorithm semantics. The compatible
predicate checks the bounded capability/root/workgroup requirements, not all
possible shader correctness or resource obligations. This narrow success does
not prove a general compiler workflow for matrices or native Metal artifacts.
The later graphics subset is bounded, not a general vertex/fragment linker or
graphics profile. Those limits do not keep the original experiment open.
