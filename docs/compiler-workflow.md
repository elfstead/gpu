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
