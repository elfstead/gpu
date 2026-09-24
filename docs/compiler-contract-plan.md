# M2 acceptance brief — programming contract and compiler interfaces

Selected 2026-09-24 after [bounded P4 acceptance](split-dependency-results.md).
This milestone addresses P7 of the performance-expressibility audit. It does not
declare a new language, freeze the C ABI or wait for native Metal validation.

## Decision and stopping condition

Can a documented device-code profile plus existing compiler tooling expose the
mechanical interfaces and optimization freedom our graphics/compute/ML consumers
need? Compare a pinned Slang profile, a small project-owned device library/profile,
and a project-owned frontend. Decide after compiling the selected examples, not
from surface syntax alone. Unsupported adapter input is a tooling limitation;
an API contract that prevents an otherwise legal native strategy is more serious.

M2 ends when the following slices pass through the installed tool, old application
results remain valid, and a committed decision names the selected language path
and restrictions. A compiler success does not establish native Metal support or
prove a universal performance result.

## Concrete slices

1. **Contract and scalar root:** document address/layout/ownership/stage/numerical
   obligations. Add FP32 root values to the existing scalar adapter with a guarded
   scale/bias example, original/reordered roots and changed local size. Use exactly
   representable data so this slice tests transport/layout, not a new tolerance.
2. **Structured data:** recursively describe nested structs, fixed arrays and
   vectors, including a pointer to a parameter block containing further addresses.
   Cross-check reflected offsets, sizes, alignments and element strides against
   SPIR-V. Emit explicit C padding and compile-time assertions. Mutate field order
   at both root and pointed-to levels without changing semantic host code. Reject
   recursive/opaque layouts, unsupported widths and layouts that cannot be verified;
   do not guess pointee layout from a name-only reflection field.
3. **Resource/stage interfaces:** migrate one existing image/sampler heap fixture
   to generated artifacts; add a bounded producer/consumer graphics interface.
   Check resource kinds, heap requirements and stage IO matching. Keep indices
   non-owning and heap contents/formats/extents application-owned. Native heap and
   image support already exists; this is not permission to add a rendering engine.
4. **Application build integration:** declare transitive include/module inputs and
   generated outputs; detect changed/missing dependencies in installed-tool checks.
   Demonstrate an out-of-tree consumer with no access to the source checkout.
   No runtime reflection service, hidden compiler download or generic package/cache.
5. **Language-direction record:** inspect actual compiler output for the structured
   pointer fixture, workgroup reduction, subgroup operation, matrix kernel and
   graphics pair. Record any hidden copies, address indirections, synchronization
   or forbidden native strategies. Separate unsupported capabilities from compiler
   profile restrictions. Use a matched native control if generated host glue adds
   work; perform a targeted timing comparison if code inspection exposes a cost.

Each slice is committed and tested before starting the next. Slang remains pinned
to 2026.14.1 for comparable output. A compiler bump needs an explicit reason and
regression run; “latest” is not part of this milestone's acceptance condition.

## Gates shared by every slice

- `spirv-val` and independent reflection/SPIR-V checks, malformed metadata tests,
  reproducible generated headers, C size/alignment/offset assertions, and explicit
  unknown-capability/extension rejection.
- Full output and guard comparisons on Radeon and llvmpipe, with Vulkan and sync
  validation; original/mutated layouts use the same C consumer source. No passing
  skip of a declared fixture, silently raised tolerance or fake completion.
- Generated arguments are ordinary application-owned bytes. Do not add mandatory
  repacking, heap allocation, GPU copies, scheduling or per-dispatch reflection.
  Host code chooses direct roots versus pointed-to blocks and obeys lifetime rules.
- Existing transform and learned-image generated headers still reproduce unless
  an intentional format change is documented. Preserve all small M1 numerical,
  guard and A/B/A cases; rerun useful-scale/native controls if arithmetic, shader
  optimization, memory layout or execution policy changes those paths.
- Installed-tool and relocated-SDK tests must include the new subset, not only the
  in-tree build. A public runtime change requires a separate rationale and ABI/test
  migration; generated helper types are not a stable public ABI.

## Current status

The [device-code contract](device-code-contract.md) describes the current boundary
and targeted extensions. [Slice 1 is accepted](compiler-scalar-results.md) at
`291eb1f`: both driver/layout fixtures, independent installed-tool mutations and
unchanged learned-image regressions pass. Structured arguments, generated heap/stage interfaces,
transitive build dependencies and the language-direction decision remain unaccepted.
Slice 2 is next. This brief is a bounded work queue, not completed M2 evidence.
