# M2 stage-interface acceptance

Accepted 2026-09-25 at clean `efcbeba`. Together with the
[heap checkpoint](compiler-heap-results.md), this completes
[M2 slice 3](compiler-contract-plan.md). M2 itself, P7 and the language-direction
decision remain open. No public API or runtime change; ABI remains 17.

## Result

The generator now cross-checks a bounded flat scalar/vector stage interface against
the compiler's SPIR-V. A separate offline pair command validates each stage, matches
fragment input locations and exact types against vertex outputs, and only then
publishes a combined C header containing both original native artifacts.
Generated words are checked byte-for-byte against the compiler binaries. Nothing
is rewritten, converted, repacked or uploaded by the tool at execution time.

Matching does not depend on source struct/field names. Extra producer outputs are
allowed. Interpolation comes from checked native decorations because Slang JSON
does not expose it; fragment interpolation need not match vertex decorations in our
monolithic pipeline. These choices follow
[Vulkan's stage matching rules](https://docs.vulkan.org/spec/latest/chapters/interfaces.html#interfaces-iointerfaces),
not an invented requirement for identically spelled source structs. Integer
fragment inputs must be flat. The bounded adapter supports Smooth/Flat/NoPerspective,
uint32/FP32 scalar/vector whole-location varyings, eight contiguous locations and
the existing builtins. It does not cover component packing, stage arrays/matrices,
centroid/sample interpolation, arbitrary vertex attributes or all legal native
interfaces. Those are tooling limits, not fundamental API restrictions.

`stage_vertex.slang` emits a fullscreen triangle with interpolated coordinates
and a flat integer. `stage_fragment.slang` reconstructs pixel coordinates, checks
them against FragCoord and checks the integer before producing an RGBA8 coordinate
pattern. The public C consumer compares every channel exactly, checks 64-byte
guards, reuses each target for three draws, and overwrites root bytes after recording
to verify the existing snapshot contract. No tolerance was introduced.

## Evidence

Pinned Slang 2026.14.1, Rust 1.97.1; Radeon RX 5700 XT/RADV Mesa 26.2.1 and
llvmpipe/LLVM 21.1.8, Vulkan and synchronization validation enabled:

- Six extents × three draws × four variants = **72 draws per driver**, all pixels
  and guards exact. Variants are original, both varying/root orders changed,
  fragment field names changed, and fragment-only NoPerspective interpolation.
  All variants use the same C consumer. Constant clip-space W makes the two
  interpolation policies equivalent for this oracle; this is a transport/profile
  test, not a perspective-correct rendering quality claim.
- Fifteen stage tests pass: matching, root/location mutations, names, interpolation,
  extra/missing producer outputs, type/width mismatch, metadata/native disagreement,
  unsupported decorations/types/builtins, original embedded words and a real
  separately valid but mismatched shader pair. Invalid pairs are never submitted.
- The clean SDK relocates to paths containing spaces. Original/reordered pairs run
  on both drivers. A deliberately half-updated pair rejects and leaves the prior
  header unchanged; updating both shaders causes the stale check to reject until
  regeneration. Installed transform, affine, aggregate and heap examples also
  pass their original/mutated execution and regeneration paths.
- Existing generated headers reproduce. Transform (16), affine (6), aggregate (20)
  and heap (11) tests pass. Both drivers rerun 48 original/reordered heap cases
  and all 38 small learned-image cases in original/mutated, normal/diagnostic modes
  (152 frames per driver). Learned-image maxima remain `1.71016513e-7` float error,
  `0.00667210166` error/bound and RGB delta 1; original/mutated files are byte-identical.
- Forty ordinary tests, strict Clippy and formatting pass. Runtime/API did not
  change; the full 28-case runtime GPU suite and ABI suite were not rerun here.
  No new Metal or other physical GPU claim.

The native-code identity and absence of extra runtime work establish the bounded
performance-expressibility result: pair checking itself imposes no GPU strategy
or conversion cost. This is not a timing comparison, a compiler optimization
quality verdict or approval of the entire fundamental API.

## Reproduce

```sh
python3 examples/compiler/stage_workflow.py --check
python3 examples/compiler/stage_workflow.py
python3 tools/install.py --prefix /absolute/new-prefix
python3 tools/test-install.py --prefix /absolute/new-prefix --stage-pair --heap-image --structured --affine --shader-check
```

Select `SLANGC`, `VK_DRIVER_FILES` and validation layers explicitly as in the
[heap receipt](compiler-heap-results.md). Installed usage is in the
[quickstart](quickstart.md); no shader tools or Cargo are needed merely to build
and execute the supplied generated consumer.

Accepted local SDK `/tmp/ogpu-sdk-stages-m2`; relocated tests
`/tmp/ogpu-external-whcs4s9d` (Radeon) and `/tmp/ogpu-external-59n2v3ef` (llvmpipe).
Local diagnostic log receipts:

| Log under `/tmp/` | SHA-256 |
|---|---|
| `ogpu-stages-radv.log` | `5669a2d3e96a3d366dc8d75d7031e9a1098ac9209ded7258871a8c6d0c381e90` |
| `ogpu-stages-lvp.log` | `fa260dd71a8a1ab8c947232f51a19e023b971d4c040da97f19bfa9234846345f` |
| `ogpu-stages-m1-radv.log` | `1a2f8f933480c82badc2d640bff8672edb5fa7c327dd5a85e0da54cce091ae49` |
| `ogpu-stages-m1-lvp.log` | `f219a2022e7f2c2e6730802673b4c7808e9df8352b3261c9c91e366d58d771c3` |
| `ogpu-stages-heap-radv.log` | `7fdc04f46868871301b0f86cf5cb9841c8ffe19e11597ae5d09200ab6c52b66f` |
| `ogpu-stages-heap-lvp.log` | `015015ded6fabadf103999e26415e279fdf3c759284d2f78394e97419d83ba62` |
| `ogpu-stages-install.log` | `0fe060e14060b773449d43104691b3d18182cf768e35a001f741950b53d75a54` |
| `ogpu-stages-sdk-radv.log` | `74c5ae7717393222f3d6d6d54444f6afa63ebb0e2face6f44a0fbd6ad2683511` |
| `ogpu-stages-sdk-lvp.log` | `cbb6318b3b3c2ee0939d4a8efd9a3dce2522dcac8879a90471a0269e4528edc6` |

Next: slice 4, compiler-reported transitive include/module inputs and generated
output declarations, tested through installed checks. Slice 5 then records the
language direction from actual native output. Neither is complete yet.
