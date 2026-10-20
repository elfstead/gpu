# D3D12 argument profile: provisional decision

Status: **PROVISIONAL — the reviewer may replace this profile.** Decided
2026-10-17 on the `dx12-backend` branch to unblock D2 (executables/compute).
Nothing in this record is merged, accepted, or reflected in the common C
header. The compiled probe evidence below is final for these toolchain
versions; the *choice* among valid profiles is what stays open.

## The question

The Vulkan foundation exposes an unrestricted pointer strategy: shaders
receive physical GPU addresses (`memory_address`, `set_root`), compute on them,
and may dereference addresses loaded from GPU memory (nested pointers,
GPU-produced references). The plan asks which of those guarantees D3D12 can
match, and forbids faking the rest (no giant virtual buffer, no
address-to-resource registry, no silent substitution of
descriptor-index-plus-offset for physical addresses).

## Probe evidence (not inferred from syntax)

Toolchain: pinned Slang 2026.14.1 (`slangc`), dxc 1.6 from the Windows SDK
10.0.22621 redistributable (`dxcompiler.dll`, `dxil.dll` placed beside
`slangc`). Sources are checked in under `tests/dx12-probes/`. Reproduction:

```
slangc tests/dx12-probes/<probe>.slang -target dxil  -profile cs_6_6 -entry main -o out.dxil
slangc tests/dx12-probes/<probe>.slang -target spirv -profile cs_6_6 -entry main -o out.spv
```

| Probe | SPIR-V | DXIL |
| --- | --- | --- |
| `flat-binding.slang`: raw buffer + uniform bytes | compiles | compiles (3548-byte artifact) |
| `pointer-arithmetic.slang`: `&data[i]`, pointer indexing, dereference | compiles | **rejected by Slang**: `error[E36107]: unavailable features in entry point ... entrypoint 'main' uses features that are not available in 'compute' stage for 'hlsl' compilation target` (note points at the `&` address-of) |
| `nested-pointer.slang`: 64-bit address loaded from GPU memory, cast to `Ptr<float>`, dereferenced | compiles | **rejected**: Slang emits the raw pointer type into HLSL and dxc rejects it: `error: use of undeclared identifier 'Ptr' ... expected '(' for function-style cast` |

Conclusion for the matrix: flat binding is a direct mapping (1); address
arithmetic/nested pointers/GPU-produced references are **unsupported on this
backend with the exact missing guarantee** — HLSL/DXIL has no general pointer
type, at the language and artifact level, independent of runtime design (3).
This is not a runtime gap an emulation could honestly close.

## Provisional profile chosen: native root arguments

1. **`set_inline` (shared inline bytes) → root constants.** Direct mapping
   (1). One native difference: root constants share the 64-DWORD root budget
   with root descriptors/tables, so the usable inline size is a property of the
   prepared signature, not a single device constant. The snapshot keeps
   `max_inline_size = 0` until prepared executables report exact budgets.
2. **`set_root` (root addresses) → root descriptors (CBV/SRV/UAV GPU virtual
   addresses).** Direct mapping (1) for *flat buffer addresses* with native
   alignment/range rules checked at preparation. A root address is not and
   will not be silently redefined as a descriptor index.
3. **Address arithmetic, nested pointers, GPU-produced references →
   `UNSUPPORTED`** (3) with the guarantee above, per probe evidence. Consumers
   relying on them cannot run on D3D12 under this profile; consumer coverage
   claims must say so.
4. **`bind_heap` (descriptor heaps as memory) → native descriptor heaps and
   root descriptor tables.** Explicit native alternative (2): descriptor
   storage is a native heap object, not GPU-addressable linear memory, and
   descriptor bytes stay device-specific (already the contract). Allocation
   and mutation rules will be specified when D2 lands the descriptor profile.

## Alternative kept open (not merged into this profile)

**Descriptor-index-plus-offset** using SM 6.6 dynamic resources: shaders store
`(descriptor index, offset)` references instead of addresses. This is a
*different artifact ABI and consumer data layout*; if the reviewer selects it,
it must become a separately declared profile (artifacts declare which they
use), not a reinterpretation of `set_root`. It would unlock descriptor-chasing
workloads the root profile cannot express, at the cost of per-backend consumer
strategies and a second compilation target.

## Reversal map (what changes if the reviewer decides differently)

- This document and the matrix rows in `docs/dx12-backend-d1.md`.
- Future D2 code only: the root-signature builder and argument-interface
  validation in the executables module, and the snapshot's
  `execution_limits.max_inline_size` / `argument_flags` (currently zero = no
  claim).
- If descriptor-index becomes primary or parallel: one more artifact-ABI
  dimension in executable preparation and shader metadata; Slang target
  flags/pipeline choices in `tools/xtask`.
- **Not affected in any case:** D1 implementation, the Vulkan pointer
  strategy, and `include/ogpu_next.h` (the root profile fits the existing
  `set_inline`/`set_root`/`ArgumentInterface` records; a descriptor-index
  profile would extend, not break, them).

## Why this direction is provisional-safe

It is the D3D12-idiomatic mapping (root constants, root descriptors), it
over-claims nothing (the pointer gap is explicit and probed), it does not
distort the shared API, and every place a different decision would touch is
listed above. The reviewer can accept, replace, or run this profile in
parallel with a descriptor-index profile without reworking D1.

## Consumer-fixture evidence (M4 foundation Rust consumer attempt)

Probe (slangc 2026.14.1, `-target dxil -profile cs_6_6`): the consumer's
`examples/shaders/foundation-compute.slang` fails to compile for DXIL. Slang
lowers the pointer member `uint* values` to generated HLSL
`Ptr<uint, ...> values_0;` and dxc rejects it (`expected member name or ';'
after declaration specifiers`) — HLSL has no general pointers, language-level,
matching the profile's probe table.

The fixture's ABI is exactly the ROOT_DESCRIPTORS shape (`byte_size: 24,
root_count: 2`): inline bytes are root constants at the same namespace
offsets, the two `*Root` pointer members are root slots (raw UAV root
descriptors). The D2.1 `argument-transform` fixture is the proven DXIL
lowering. Consumer work therefore compiles DXIL artifact variants through
that lowering rather than reusing the SPIR-V-oriented sources unchanged;
`SHADER_DXIL` already exists in the artifact format enum and the consumer's
`Shader.code` is byte-based, so artifact plumbing is byte-exact DXIL.

## Consumer attempt: foundation Rust consumer on Windows (evidence)

The standalone consumer (`examples/foundation_rust`) was built and run against
the D3D12 backend on the RX 5700 XT machine. It discovers and creates the
device through the foundation API (`NATIVE_RUST_DEVICE=AMD Radeon RX 5700
XT`) and then reports `NATIVE_RUST_FAIL status=-2` — a clean `UNSUPPORTED`
from its first unsupported operation, with no panic and no device fault. The
consumer's own contract treats unsupported-device runs as failures rather
than skips, which is the honest outcome under this profile.

Coverage claim (rule 3): the consumer's compute workload
(`examples/shaders/foundation-compute.slang`) depends on nested pointers
(`source->values[i]`, `DataRoot { uint* values; ... }`), which the profile
marks `UNSUPPORTED` and which the toolchain rejects at the language level
(see the probe table above). The consumer therefore does not run to
completion on D3D12 under the ROOT_DESCRIPTORS profile, and no substitute
strategy is hidden for it. The M4 textured-scene argument shape it carries is
covered separately by the `gpu_scene_raster` fixture (shared inline bank,
flat root-descriptor vertex buffer, descriptor-indexed sampling, indexed
draws with depth rejection), and the executable-cache path it exercises is
covered by `gpu_executable_cache_roundtrip`.

## Consumer wiring: the M4 scene's DXIL variant (work in progress)

The consumer's M4 scene (the `surface` binary, `--scene-check` mode) now has
the D3D12 variant wired end to end: `examples/graphics_scene/textured_dxil.slang`
(the documented lowering of `textured.{vert,frag}.slang`: shared inline bank,
the vertices address as root descriptor u0, descriptor-indexed
textures/samplers through bound heap tables), its checked-in `textured.{vs,fs}.dxil`
artifacts, a `build.rs` that emits both artifact families, and per-target
selection in `scene_resources.rs` (`SHADER_DXIL` + the root slot + `vsMain`/
`fsMain` on Windows, `SHADER_SPIRV` + inline pointer bytes elsewhere).

The binary builds and runs against the D3D12 backend and reaches the scene
setup, currently failing at a further `UNSUPPORTED` before any case executes.
Candidate checks for the next session, in order: the scene's `topology: 3`
against this backend's topology mapping; the graphics state's dynamic-state
bits; the vertex-input record; the blend enum values (`src_color: 6`,
`dst_color: 7`). Each is a small, visible rejection to either accept or
document — nothing here is emulated.

### The scene consumer's actual gate: presentation, not graphics

The remaining `UNSUPPORTED` before any scene case executes is the very first
call: `discover_presentation(PRESENTATION_WAYLAND)`. The M4 scene consumer is
a Wayland-window application — even its `--scene-check` mode opens a surface
— and this backend's presentation boundary is unimplemented (D1 deferred DXGI
swapchains to the window-system profile). The graphics argument work it needs
is complete and wired (DXIL variant, root slot, descriptor tables); running
the consumer to completion on Windows therefore requires (a) the DXGI
swapchain/surface implementation on this backend and (b) a Win32 window path
in the consumer alongside its Wayland one. Neither is an arguments-profile
gap — both are named, visible work items.
