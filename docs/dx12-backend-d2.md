# D3D12 foundation backend: D2 batch (in progress)

Branch `dx12-backend`. Updated 2026-10-18. **D2 implements native artifacts,
compute, descriptors and offscreen graphics. This document tracks the batch as
it lands; entries below are natively tested but unreviewed and unmerged.**

## D2.1 — root-argument compute (implemented, natively exercised)

The provisional [argument profile](dx12-arguments-profile.md) is now implemented
and executed on the GPU: prepared compute executables from DXIL artifacts,
root-argument recording and direct/indirect dispatch.

| Piece | Mapping |
| --- | --- |
| Artifacts | `SHADER_DXIL` native DXIL containers, per the header's "native artifacts... unimplemented profiles" extension point and its "no runtime shader translation" rule. Slang/DXC tooling produces artifacts per backend; SPIR-V artifacts (pointer-in-bytes ABI) return `UNSUPPORTED` on this backend |
| Argument namespace | Root constants at `b0`, full `byte_size` at identical offsets as the shared-bytes profile |
| Root slots | Raw UAV root descriptors at `u(slot)`, space 0; `set_root` updates the namespace bytes and the binding |
| Byte preservation | Root arguments are invalidated by root-signature changes natively, so binding re-emits the recorded namespace bytes (bounded native re-recording, no allocation) to satisfy "binding preserves bytes at identical offsets" |
| Inline over root bytes | `UNSUPPORTED`: a root descriptor binding cannot follow byte writes to a shared namespace (the Vulkan-only "update a root through inline" trick stays visible as a gap) |
| Specialization | `UNSUPPORTED`: D3D12 has no specialization constants; the strategy is an open profile question |
| Indirect dispatch | `ExecuteIndirect` with a device-level dispatch command signature; arguments stay in caller memory (read at execution) |
| Storage usage | Carries the native unordered-access flag, which UPLOAD/READBACK heaps reject: storage buffers report only the DEFAULT heap as compatible in requirements |

API additions (additive, documented; see the ledger): `OGPU_NEXT_SHADER_DXIL`,
`OGPU_NEXT_ARGUMENT_ROOT_DESCRIPTORS`, and the `HEAP_RESOURCE`/`HEAP_SAMPLER`
kind constants already present in the C header.

Evidence (same machine as the D1 receipt: Radeon RX 5700 XT, driver
32.0.21045.5002, Windows 11 26100, Slang 2026.14.1 fixture generation):

- `gpu_root_argument_dispatch_roundtrip`: dispatches the checked-in fixture
  (`tests/dx12-fixtures/`, DXIL regenerated per its README) with two root
  descriptors and inline bytes; verifies both direct and indirect dispatch
  outputs byte-exact against a CPU oracle; rebinds an executable and a root
  slot and verifies byte preservation.
- `gpu_argument_contract_rejections`: inline-over-root bytes, unknown slots,
  misaligned addresses, dispatch without bind and dynamic shared bytes reject
  exactly as the contract states; SPIR-V artifacts and specialization reject.
- All D1 suites still pass (8 GPU + 18 host tests on Windows; Linux regression
  110 + 67 unchanged).

## D2.2 — descriptor profile (implemented, natively exercised)

The provisional recipe profile is implemented and verified on the GPU, with one
artifact-ABI correction forced by evidence: descriptors bind as an **unbounded
descriptor table at `u0, space1`** (classic descriptor-table indexing), not
SM 6.6 global-heap direct indexing.

| Piece | Mapping |
| --- | --- |
| Descriptor bytes | Opaque validated recipes (`ResourceDescriptor`/`SamplerDesc` records) in linear `DESCRIPTOR_HEAP` memory; `copy_memory` and mutation semantics unchanged |
| Descriptor heap memory | Linear bytes plus preallocated native shader-visible heaps (resource + sampler kinds, D3D12 separates them); host-visible type required (recipes materialize at bind) |
| `write_*_descriptors` | Validate and encode recipes into caller host storage; output sizes bounded by recipe strides (40/80 bytes) |
| `bind_heap` | Materializes the bound range into native descriptors (`Create*View`/`CreateSampler`) and binds the range start as an unbounded UAV descriptor table; table indices are range-relative. Changing heaps invalidates tables/root arguments natively, so binding re-emits both (bounded re-recording) |
| Artifact ABI | `RWByteAddressBuffer heap[] : register(u0, space1)` unbounded arrays (Slang → DXIL, fixture checked in). SM 6.6 `ResourceDescriptorHeap[i]` direct indexing is recorded as an **open alternative**: with root-signature flags verified against the SDK header, correct descriptors and proven shader execution, its writes were silently dead in all probes — the artifact/runtime alignment for that style needs its own investigation |
| Views | Validated interpretation records ("views are descriptor contents"); RTV/DSV slot ownership lands with D2.3 rendering |
| Mutation | Descriptor bytes are captured at bind on this backend; mutate-then-rebind is the profile rule (the review gate's independent-range mutation is exercised across submissions). Submission-time re-materialization stays the reviewer's alternative |

Evidence (same machine as the D1 receipt):

- `gpu_descriptor_heap_roundtrip`: recipes → heap memory → bind → unbounded-table
  dispatch, byte-exact against the CPU oracle on both buffers; a mutated range
  plus rebind redirects the second submission's output and the diagnostic
  root-slot buffer proves each dispatch executed with the expected constants.
- `gpu_descriptor_contract_rejections`: short outputs, unsupported descriptor
  kinds, non-descriptor memory, misaligned ranges and reservations reject
  exactly as documented; non-host-visible descriptor memory rejects.
- Full suite: 10 GPU + 18 host tests on Windows pass (D1 and D2.1 included);
  Linux regression unchanged (110 + 67).

### Incidents recorded (D2.2)

- SM 6.6 direct heap indexing produced silently dead writes despite verified
  root-signature flags (values cross-checked against the Windows SDK header),
  correct descriptor contents and proven shader execution; bisected down to the
  indexing style itself before switching to descriptor tables.
- A test-level copy/dispatch race ("all ones" output) masqueraded as an
  implementation bug; the profile's barrier requirements were already correct.
- Debugging blind without the debug layer cost several GPU-level bisections;
  the debug-layer deployment gap from D1 remains the top tooling follow-up.

## D2.3a — render scopes and attachment views (implemented, natively exercised)

Clear-only and attachment-load render scopes work end to end: attachment views
own native RTV/DSV descriptors (cold creation into device view heaps), render
begin binds targets and applies explicit CLEAR load operations over the
declared area, and pixels verify byte-exact on readback.

| Piece | Mapping |
| --- | --- |
| `View` attachment records | Own native RTV/DSV descriptors (the "views are descriptor contents" decision); slot allocation is a cold-path counter into device view heaps |
| `render_begin` | Validate the full attachment contract (states, one-mip identity views, pairing rules, area, scratch sized per color count) then `OMSetRenderTargets` and explicit `Clear*View` load operations; no transition, initialization or implicit viewport/scissor |
| `render_end` | Clears the recorded scope; D3D12 render targets are state, so no native call is required |
| Load/store operations | LOAD preserves (native state keeps contents), CLEAR clears, DONT_CARE is a no-op; STORE/DISCARD map to native attachment lifetime (no store-op call exists) |
| Depth/stencil | DSV creation and paired-record validation land here; depth clear values are range-checked to [0,1] |

Evidence: `gpu_render_scope_clear_readback` (RGBA clear through a real render
scope, pixel-exact readback) and `gpu_render_scope_contract_rejections`
(secondary scopes, invalid attachment states, out-of-scope render end, views
without attachment usage). Full suite: 12 GPU + 18 host tests on Windows;
Linux regression unchanged.

## D2.3b — graphics executables and rasterization (implemented, natively exercised)

Graphics pipelines work end to end: PSO preparation from the shared
GRAPHICS_STATE/VERTEX_INPUT records, viewport/scissor and the native dynamic
states, and draws that rasterize with pixel-exact verification.

| Piece | Mapping |
| --- | --- |
| Graphics executables | DXIL vertex artifact with optional fragment artifact → `CreateGraphicsPipelineState`; topology, cull/front face, samples, color blend state, depth/stencil state, raster options and vertex input all translate at preparation |
| Vertex input | Location N binds as semantic `TEXCOORD<N>` (the artifact ABI convention; documented in the fixture) |
| Dynamic states | Viewport/scissor, blend constants and stencil reference are native dynamic state; cull/front-face/depth/stencil-masks/ops are PSO state and reject visibly rather than being silently baked (the DYNAMIC bits advertise exactly this) |
| Shared argument namespace | Root-constant/descriptor updates reach both compute and graphics root state, preserving the SHARED_BYTES contract that all stages see one namespace |
| Draws | `DrawInstanced`/`DrawIndexedInstanced`; `bind_vertices`/`bind_indices` translate to input-assembler views with per-binding strides from the executable's vertex input (caller scratch holds the view array) |
| Empty namespaces | The namespace-constants root parameter is omitted when `byte_size == 0` (a zero-length root-constant parameter is invalid) and root indices shift accordingly |
| Artifact sizing | DXIL containers are byte-granular (their header declares the size); the word-granular `size % 4` rule belongs to SPIR-V artifacts. Bytecode length must exactly match the container |

Evidence: `gpu_graphics_triangle_raster` (clear + viewport + draw of a
vertex-id triangle; center pixel red, corner pixels green — pixel-exact) and
`gpu_graphics_contract_rejections` (draw without an executable, PSO-baked
dynamic states, zero-area viewport). Full suite: 14 GPU + 18 host tests on
Windows; Linux regression unchanged.

### Incidents recorded (D2.3b)

- Two intertwined artifact issues: padding a DXIL container to a word multiple
  made `BytecodeLength` disagree with the declared container size (PSO
  rejection), while the unpadded container tripped the SPIR-V size rule. Fixed
  by recording the byte-granular rule for DXIL and keeping artifacts unpadded.
- A root-constants parameter with zero dwords is invalid; empty argument
  namespaces omit the parameter and shift root-parameter indices (now computed
  from the executable, not assumed).

## D2.4 — image/sampler descriptors and textured raster (in progress)

The descriptor profile extends to image and sampler recipes: validated view
records materialize as native SRV/UAV descriptors (`TEXCOORD` component
mapping, mip/layer ranges), sampler recipes as native samplers, and the root
signature grows the profile's tables — a UAV table (u0/space1) for buffers, an
SRV table (t0/space1) for images and a sampler table (s0/space1), each bound
at the heap range start (separate table parameters may share a range start;
ranges within one table must not overlap).

**Open item (blocks `gpu_textured_raster`):** narrowed to the image-descriptor
path by layered diagnostics (fixture variants + the debug layer):

1. **Texture content is correct** — the uploaded pattern reads back exactly
   (`[0,0,100,255, 60,0,100,255, ...]`) through the independently verified
   image-copy path.
2. **Vertex input, input layout and UV interpolation are correct** — a
   diagnostic fragment shader reporting the interpolated UVs matched the CPU
   oracle exactly (this exercised the TEXCOORD<N> convention end to end).
3. **The texture SRV reads only texel (0,0) correctly** — precise finding from
   two-fetch diagnostics: `Load(0,0)`/`[uint2(0,0)]` returns the correct
   pattern values, while `texel(1,0)` returns nondeterministic garbage (0
   across runs, 24 in the next) — while `CopyTextureRegion` reads the same
   texel correctly. So the resource is a correct 4x4 texture and the defect is
   in what the SRV descriptor resolves to: either the bound SRV table
   parameter or the SRV payload view itself. Buffer descriptors through the
   UAV table are proven; the `RWTexture2D` (UAV-table) variant faults the
   device and needs a careful retest.

Next-step hypotheses, in order: (a) the SRV table root-parameter index used
at bind (`table_parameter() + 1`) does not match the signature layout;
(b) the `native_view_desc` SRV payload makes the view behave 1 texel wide;
(c) the materialized slot index for image recipes. A decisive probe: bind the
texture SRV at the *UAV-table* parameter too and read through both tables.)


Also fixed this round: render scopes require the caller's attachment-state
transition (images start UNDEFINED; the clear worked only by driver tolerance —
the debug layer flagged it), and a test-side scratch requirement mismatch
rendered a post-scope barrier CAPACITY-poisoned.

Debug-layer note: `gpu_setup` enables the OS debug layer when installed, so
every GPU test is also a validation run. Two benign messages recur (the
defensive close on already-closed lists and the missing optimized clear value
on render targets) and should be silenced or documented at the review.

The textured fixture itself is checked in (vertex-buffer VS with `TEXCOORD<N>`
semantics, sampling PS with snapped texel fetches for exact expectations) and
the test is the acceptance criterion, labeled `blocked` until the pairing is
resolved.

### Incidents recorded (D2.4)

- The debug layer (Graphics Tools, installed 2026-10-18) named the first
  blocker immediately: the root signature lacked
  `ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT` for vertex-input PSOs. It also caught
  that image recipes were dereferencing a null buffer span before the
  recipe-kind branch (fixed), and that `mod.rs` re-exported the stub
  `vertex_scratch` instead of the real one (fixed — the same stub-shadowing
  class as earlier incidents; worth a naming convention at review).

## Remaining D2 scope

- Standalone `clear_image` (transient-descriptor question) and multisample/
  resolve targets (resolve profiles) remain open; secondary rendering bundles,
  image descriptor recipes and dual-source blending are named rejections.
- The `TEXCOORD<N>` vertex-semantic convention should move into the decision
  record when the reviewer confirms it.
- Image descriptors (sampled/storage image recipes materializing SRV/UAV from
  validated views) — the D2.3 graphics tests exercise them.
- Queries, executable caches (pipeline libraries), count-based indirect work,
  specialization strategy (D2.4).
- Consumer attempts (foundation Rust consumer, learned-image, M4/M5) once the
  above profiles express them.

## D2.2 descriptor profile: provisional decision (implementation next)

Status: **PROVISIONAL — the reviewer may replace this profile.** The question:
the shared contract encodes descriptors into host bytes, lets the caller place
or copy those bytes into linear `DESCRIPTOR_HEAP` memory, and binds a range.
D3D12 descriptors are opaque native values: they exist only inside native
descriptor heaps, are written by `Create*View`, and cannot be materialized into
host memory. The matrix already records "no promise of portable or directly
writable descriptor bytes" for this backend.

### Provisional choice: recipe-encoded descriptor bytes

`DESCRIPTOR_HEAP` memory stays linear memory (so `copy_memory` of descriptor
ranges keeps working exactly as the contract describes) and additionally owns
preallocated native descriptor heaps (resource-kind and sampler-kind, created
at preparation). Descriptor "bytes" are an opaque device-specific **recipe**
encoding. `bind_heap` materializes the bound range into the native heaps and
binds them — bounded record-time encoding into preallocated storage, no
allocation, no copying of resources. Descriptor mutation works exactly as the
contract states: overwrite recipe bytes and rebind. `DescriptorLimits` reports
the recipe sizes/alignments. Artifact-side heap access maps to SM 6.6 dynamic
resources (direct heap indexing), matching the header's "direct heap access"
note.

### Alternative kept open

A write-to-heap API (`write_*_descriptors` targeting a descriptor-heap span
directly) with native `Create*View` encoding — the category-2 alternative where
the heap memory is natively writable and no recipes exist. It changes the
shared API surface; the recipe profile keeps the API unchanged. Either can be
selected without touching D2.1 or D1.

### Related rendering decision (D2.3)

Render-target/depth views are *not* shader descriptors: D3D12 has separate
RTV/DSV heaps and `Set*Targets` binds by handle. A `View` object therefore owns
its RTV/DSV descriptor slot (cold creation) while shader-facing views
materialize as descriptors. `View::create` itself is a validated interpretation
record on this backend — the "views are descriptor contents" item from the D1
matrix.

## M4 scene-shape consumer milestone (gpu_scene_raster)

`gpu_scene_raster` proves the M4 textured-scene argument shape end to end on
the native backend: the 24-byte shared inline bank read by both stages (with
its root-slot words), a flat root-descriptor vertex buffer (D2.1 lowering),
descriptor-indexed image and sampler selection (D2.2 lowering: table slot N is
heap slot N), an indexed two-quad draw with depth rejection (near drawn first,
far rejected where it overlaps) and a scissored overlay phase. Pixels are
exact: solid textures make bilinear sampling constant, and the geometry
avoids pixel-center edge ambiguity. The fixture is
`tests/dx12-fixtures/scene.slang`, the DXIL lowering of
`examples/graphics_scene/textured.{vert,frag}.slang`.

Three findings belong to this milestone:

1. **Contract violation fixed in `inline()`**: the SHARED_BYTES clause of
   `ogpu_next_bind_executable` states "set_root changes only its 8 bytes;
   inline may intentionally change those same bytes". The backend rejected
   inline ranges overlapping root slots (`UNSUPPORTED`); the M4 consumer
   writes its full Root bank through `set_inline` and hits exactly this.
   Fixed: such writes are accepted and update the slot's root descriptor from
   the resulting namespace bytes, exactly as `set_root` would. The
   `gpu_argument_contract_rejections` expectation was updated accordingly.

2. **Interstage signatures on DXIL**: Slang assigns signature registers per
   entry point independently; a fragment stage consuming a trimmed input
   struct (without `SV_Position`) links against a vertex stage that emits it
   and fails PSO creation with "Semantic 'TEXCOORD' is defined for mismatched
   hardware registers". DXIL fixture variants must pass the full vertex
   output struct to the fragment stage (the working textured fixture takes
   this shape). This is a lowering convention for shader variants, not a
   backend restriction.

3. **Buffer barriers cover whole resources on this runtime**: the enhanced-
   barrier validation rejects sub-range buffer barriers ("Size must be
   UINT64_MAX or the width of the buffer"). Sub-range memory barriers should
   be treated as non-portable until a runtime is found that accepts them;
   the backend currently forwards the range unchanged.

Also fixed while landing the depth path: `render_begin` eagerly dereferenced
the absent `stencil` pointer through `unwrap_or` fallbacks (UB that crashed
depth-only scopes), and depth/stencil clear flags are now filtered by the
view format's aspects (a depth-only view no longer requests a stencil clear).

Suite after this milestone: 17 GPU + 18 host tests pass on Windows (RX 5700
XT); Linux regression unchanged (110 + 2 + 67).

Consumer coverage so far: the M4 scene argument shape is covered by fixture;
the `foundation_rust` compute workload (`examples/shaders/foundation-compute
.slang`) depends on nested pointers (`source->values[i]`), which rule 3 of the
arguments profile marks `UNSUPPORTED` and which the toolchain rejects at the
language level — its coverage claim stays "not covered on D3D12 under the
ROOT_DESCRIPTORS profile".
