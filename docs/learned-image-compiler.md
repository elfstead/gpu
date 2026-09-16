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
