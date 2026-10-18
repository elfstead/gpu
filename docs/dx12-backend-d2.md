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

**Open item (blocks `gpu_textured_raster`):** the graphics PSO with a vertex
input layout rejects with `E_INVALIDARG` at `CreateGraphicsPipelineState`
while the same shaders/PSO path without input layout works (the triangle
fixture passes against the extended three-table signature). The compiled VS
input signature names are `TEXCOORD` (DXIL ISG1); the semantic-index/layout
pairing needs a debug-layer run or full signature-chunk decode to resolve —
the top reason to unblock the debug layer (Graphics Tools optional feature).

The textured fixture itself is checked in (vertex-buffer VS with `TEXCOORD<N>`
semantics, sampling PS with snapped texel fetches for exact expectations) and
the test is the acceptance criterion, labeled `blocked` until the pairing is
resolved.

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
