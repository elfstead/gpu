# Cooperative integer reduction

Status: implementation experiment, not an API proposal. The question is whether
the existing address/root/batch model can express workgroup cooperation and a
multi-level reduction without a reduction-specific host API.

Run the [C example](../examples/reduction.c) with `cargo xtask reduction`; run the
boundary/pattern/intermediate checks with `cargo xtask gpu-tests`.

## Workload and numerical contract

Sum an array of uint32 values modulo 2^32. Empty input produces zero. This exact
contract separates synchronization/indexing failures from floating-point rounding;
it is not evidence for floating-point accuracy or accelerated ML arithmetic.

The [shader](../examples/shaders/reduce.comp) uses 64 invocations and 256 bytes of
shared memory per workgroup. Each invocation loads up to two input elements, then
the group reduces up to 128 elements into one partial sum. Out-of-range lanes
contribute zero and still execute every workgroup barrier. Input and output ranges
must not overlap; there is no cross-workgroup barrier inside a dispatch.

One dispatch produces `max(1, ceil(count / 128))` partial sums. Repeat with those
partials as input until one value remains. Empty input still records one workgroup
with count zero and a valid dummy input allocation. The default example uses:

```text
1,048,579 input values → 8,193 partials → 65 partials → 1 result
```

The host records the entire chain into one batch and uses one completion wait.
Every level has a separate output allocation, and COMPUTE_WRITE → COMPUTE_READ
barriers connect levels. The CPU never reads intermediate values to schedule work:
the stage sizes are known from the initial count.

There are two distinct synchronization scopes here:

- Shader `barrier()` synchronizes shared memory among invocations of ONE workgroup.
  All lanes reach it in uniform control flow, including inactive input lanes.
- Batch access barriers make a completed producer level's writes visible to the
  following dispatch. Shader barriers cannot replace this dependency.

The shared-memory rule follows the [GLSL invocation-control contract](https://docs.vulkan.org/glsl/latest/chapters/builtinfunctions.html#shader-invocation-control-functions).

## Host/shader boundary

The application-defined 24-byte root has input/output GPU addresses at bytes 0/8,
count at byte 16, and four initialized padding bytes. Every reachable allocation
stays owned through completion. No descriptor objects, subgroup operations,
specialization, atomics, optional numeric features, or new runtime entry points
are required. The source fixes the workgroup size; 128 is the number of input
elements per group, not the invocation count.

Recording copies each stage's root bytes, so a host-local root can be reused on
the next iteration. Scratch allocations are not implicitly retained through those
copied addresses. They are kept explicitly until the batch completes, including
on error cleanup. This experiment chooses separate per-level scratch allocations
to make dataflow and intermediate validation straightforward; it does not settle
scratch pooling, ping-pong reuse, or an allocator API.

## Checks and limits

The C example compares the final scalar with the low 32 bits of a 64-bit CPU sum.
The Rust test additionally checks every intermediate partial against sequential
CPU chunk sums after the single final wait, guards around each GPU data range,
and the unchanged input. Cases cover empty input, one element, 64/128-element
boundaries, transitions between dispatch depths, and a million-element input.
Patterns include zeros, ones, UINT32_MAX, and deterministic mixed values.

This is a correctness experiment, not a tuned reduction or benchmark. It does not
test subgroup-size control, floating-point association/error, matrix instructions,
device-local linear allocations, general dispatch-limit queries, or reusable
batches. Inputs in the executable tests stay within the baseline dispatch limits;
the example is not a general-purpose library for arbitrarily large arrays.

## Reproduction

The shader binary is checked in. Normal builds do not need a shader compiler.
Regenerate with glslang 16.4.0, targeting Vulkan 1.2, and validate with
SPIRV-Tools 1.4.357.0:

```sh
glslangValidator -V --target-env vulkan1.2 examples/shaders/reduce.comp -o examples/shaders/reduce.spv
spirv-val --target-env vulkan1.2 examples/shaders/reduce.spv
cargo xtask reduction
cargo xtask gpu-tests
```

See [development](development.md) for the toolchain, loader selection, and validation
settings. Results and remaining design questions belong in the [experiment ledger](experiments.md).
