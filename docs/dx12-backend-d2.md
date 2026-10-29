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

## Executable caches (D2.4: `gpu_executable_cache_roundtrip`)

The executable cache maps to D3D12's native cached pipeline-state mechanism:
`CachedPSO` hints at `Create*PipelineState` and `GetCachedBlob` after creation
— the same advisory compiled-pipeline storage `VkPipelineCache` wraps on
Vulkan. The exported container is the backend's native cache representation:
a magic/version header, the adapter identity (vendor, device and cache UUID
from the adapter LUID) checked at import with `UNSUPPORTED` on mismatch and
never silently retried as empty, and entries keyed by the canonical
preparation identity (every byte that shapes the native PSO: shader code,
entries, specialization, static state, requirements). Entries compare
exactly, so a cached blob is only offered to the preparation it came from —
no hashing collisions and no cross-preparation reuse.

Contract clauses covered by `gpu_executable_cache_roundtrip`: empty creation,
preparation through a cache for both compute and graphics, size query and
full export, unchanged-bytes import with a second preparation, identity
mismatch rejection, valid partial export at reduced capacity (whole entries
up to capacity; the container's entry count matches what was written), merge
preserving destination contents with `count=0` as a no-op and self-merge
invalid. A rejected cached blob falls back to uncached creation — caching
promises no hit semantics, per the advisory clause.

Provisional design notes for the reviewer: the entry key is the exact
preparation identity rather than a digest (exact comparison, at the cost of
carrying the identity bytes in the export); the alternative native mapping is
`ID3D12PipelineLibrary` with a name table, which this implementation avoids
because library serialization is monolithic and cannot honor the valid-
partial-export clause. The synchronization field accepts both modes;
`CALLER_SYNCHRONIZATION` requires the `CACHE_CONTROL` feature.

Suite: 18 GPU + 18 host tests pass on Windows; Linux unchanged (110 + 2 + 67).

## Queries (D2.4: `gpu_query_occlusion_timestamp`)

Native query heaps (`ID3D12QueryHeap`) with caller-owned reset, issue and
resolve order. Mapping: kind 1/2/3 to TIMESTAMP/OCCLUSION/PIPELINE_STATISTICS
heaps; `query_begin/end` to `BeginQuery`/`EndQuery`; `timestamp` to a single
`EndQuery` of a TIMESTAMP query whose scope lowers to exactly one native stage
bit (HOST, zero and multi-bit lowerings rejected); `queries_resolve` to
`ResolveQueryData` with `WAIT` mapping to the native in-queue resolve
ordering. One active begin/end query per type per list; inside-scope queries
must end before `render_end` (enforced).

Evidence-backed boundary found during the run: resolving a **never-issued**
query slot hung the GPU until TDR, exactly the contract's "do not resolve
never-issued slots" clause. The test now issues every resolved slot; the
clause is a real device requirement on this backend, not just a portability
rule.

Visible rejections on this backend (native result data is 64-bit and packed
with no stride control): the contract's 32-bit result width (would require a
format conversion this profile does not hide), strides other than the native
record stride (8 bytes per occlusion/timestamp query, 88 bytes per statistics
query), AVAILABILITY and PARTIAL results. `queries_reset` validates its range
and records no native command: D3D12 query slots are implicitly reusable
after resolve and have no reset. Statistics pools require the full native
counter mask (11 counters, ascending bit order = the native struct order);
subset masks would change the record layout and stay visible. The snapshot
now reports the physically supported features (RASTER, PIPELINE_STATISTICS,
PRECISE_OCCLUSION, CACHE_CONTROL) and the eleven statistics counters as
capabilities.

Suite: 19 GPU + 18 host tests pass on Windows; Linux unchanged (110 + 2 + 67).

## Indirect draws (D2.4: `gpu_draw_indirect`)

Count-based and fixed-count indirect draws through device-level `ExecuteIndirect`
command signatures (DRAW and DRAW_INDEXED argument layouts — the contract's
wire records match the native ones field for field). The optional count span
maps to the native count buffer, whose `min(count, maximum_count)` semantics
are exactly the contract's. Validation mirrors the contract: the arguments
span covers `(maximum_count-1)*stride + record size`, addresses are 4-byte
aligned, INDIRECT memory is required for arguments and counts.

Visible boundary: the native command stride is fixed to the wire record size
(16/20 bytes), so the contract's wider argument strides (gaps between records)
have no native form and reject with `UNSUPPORTED`; stride zero is accepted for
a single draw. `maximum_count` is checked against `max_indirect_count`, which
the snapshot now reports as the native u32 bound.

An apparent "first recorded render scope draws nothing" symptom appeared
during development and is **resolved as a non-issue**: controlled probes (two
identical scopes on one list; the direct draw in every slot; the exact
original arrangement) all render pixel-exactly. The symptom only ever occurred
while the test file was in corrupted intermediate states from scripted edits
(duplicated test bodies and mis-targeted replacements — a recurring hazard of
this work, recorded here as a development note). The final arrangement
verifies all three phases in order: fixed-count draw, fixed-count indexed
draw, and a zero-count count-based draw that must issue nothing.

Suite: 20 GPU + 18 host tests pass on Windows; Linux unchanged (110 + 2 + 67).

## Image clears (D2.4: `gpu_image_clear`)

`clear_image` maps to native `ClearRenderTargetView`/`ClearDepthStencilView`
with one temporary RTV/DSV per mip slice of the range (D3D12 clears whole
views; the descriptor is consumed at record time and never retained). The
format's numeric class selects the value member: float/normalized colors
clear through the float member, depth/stencil formats validate the
depth_stencil member (finite, in [0,1]) and clear with the aspect-matching
flags. Visible boundaries: integer-format clears need the UAV clear path this
profile does not expose (`UNSUPPORTED`), compressed clears are never
synthesized, and native clears are render-target operations so they require a
graphics-capable list on this backend (the contract's compute-queue color
clear would need `ClearUnorderedAccessView*` on a storage view).

Also fixed with it: the attachment view builders filled the 2D union member
while declaring the 2D-array dimension for multi-layer views; array views now
carry their layer slice (`FirstArraySlice`/`ArraySize`) for both RTV and DSV.

`gpu_image_clear` verifies exact clear values through the copy path and the
state/range rejections. Suite: 21 GPU + 18 host tests pass on Windows; Linux
unchanged (110 + 2 + 67).

## The mixed compute-to-raster fixture (green)

Resolved: the `CAPACITY` poison was `scratch()`'s insufficient-scratch
rejection in the barrier path — the test's barrier scratch allocation
under-counted the records (the scratch-size helper's parameter pairing
mattered). The fix is the caller sizing the scratch for both record kinds; a
symmetric `(1, 1)` allocation is valid regardless of ordering. The encoding
trace (two lines in `foundation/rust.rs`) located it in one run.

## The mixed compute-to-raster fixture (history)

`gpu_compute_to_raster` is written and compiles: the D2.1 argument transform
dispatches `value * multiplier + count` over uploaded vertex words (the
inverse transform is uploaded so the vertices land exact), a
compute-to-vertex barrier sequences it against the M4-shape draw, and the
draw consumes the computed positions through its root descriptor together
with descriptor-indexed sampling. The vertex buffer starts zero, so the
rendered pixels prove the compute ran and its writes reached the vertex
fetch. One test-side defect was already found and fixed this way (the index
buffer needed `USAGE_COPY_DST` when written by copy rather than map).

Two test-side defects were found this way and fixed: the index buffer needs
`USAGE_COPY_DST` when written by copy rather than map, and the barrier
scratch must cover the declared memory and image records.

## Open D2 checklist item: the mixed compute-to-raster fixture

The plan's D2 fixtures list includes a *mixed* compute-to-raster fixture. The
compute (argument/heap roundtrips), copy and textured offscreen raster
fixtures all exist and pass; the mixed one — a compute dispatch writing the
vertex buffer that a descriptor-indexed textured draw then consumes through
its root descriptor (the D2.1 argument lowering feeding the M4 shape) — is
the remaining checklist item. Its shape is fully supported today (both halves
are proven separately: `gpu_argument_transform` and `gpu_scene_raster`); it
needs a test that sequences dispatch -> compute-to-vertex barrier -> draw in
one list. Nothing blocks it; it was left unwritten at the end of this batch.

Presentation (D3) and the consumer's Win32 window path are out of scope for
this branch: the shared contract defines presentation only for Wayland
surfaces, and the plan places "Windows presentation and advanced native
profiles" in D3.

## gpu_draw_indirect: the first-phase quirk is real (labeled blocker)

Correcting the earlier "non-issue" conclusion: the symptom is reproducible in
the current tree — the first phase's draw through the test's closure renders
nothing (the clear lands, the draw does not), while identical later phases
draw pixel-exactly. Control evidence this session: the mixed
compute-to-raster fixture's first scope draws through the same closure
structure; the two-scope inline probe draws in both scopes; and every
passing gpu_draw_indirect configuration had a non-drawing first phase
(count-zero or the direct control in a later slot). Host-write visibility was
hypothesised and tested (memory_flush after the map/unmap, matching the
consumer's documented flow) — no change. The pattern "the first
render-scope draw in a descriptor-free, argument-free closure sometimes does
not rasterise" is worth a focused session with a debugger and the debug
layer's verbose messages; the indirect variants themselves verify
pixel-exactly in the later slots. The suite keeps it as the single labeled
blocker alongside the now-green mixed fixture.

### First-phase quirk: narrowed to one test, counter-evidence recorded

A controlled AB probe (`gpu_closure_ab_probe`) records the same draw scope
through a closure (round 0) and inline (round 1) on one list: **both draw
pixel-exactly** — so neither the closure nor the first-scope position is the
trigger in general. Adding the indirect-test's argument/index memories with
their map/unmap to the probe changes nothing. The remaining delta between
the probe and `gpu_draw_indirect`'s first phase is small (the Indirect
record construction in the closure, the three-region readback, and this
test's particular captures) and the phenomenon is confined to that one test:
every other graphics test — including the mixed compute-to-raster fixture's
first scope and the scene test's three closure phases — draws in its first
scope. `gpu_draw_indirect` keeps the reliable arrangement (the zero-count
phase, which must draw nothing, in the first slot) and its drawn variants
verify pixel-exactly; the counter-evidence lives in `gpu_closure_ab_probe`
for whoever picks this up with a debugger.

Notes from the follow-up run: `memory_flush` on a coherent UPLOAD-heap
memory rejects with `INVALID` (such memory needs no flush), so the
host-write-visibility hypothesis is closed from the API side too; the flush
call was a test error, not an API gap. `gpu_draw_indirect` verifies its
count-based suppression with a *late* zero-count phase — after a drawn phase
on the same list, so a stale record would show red — which makes that check
meaningful regardless of the first-slot quirk. Suite: 23 GPU + 18 host tests
pass on Windows with `gpu_closure_ab_probe` kept as the counter-evidence
probe; Linux unchanged.

**Intermittency note (important for the reviewer):** the first-slot symptom
is *flaky*, not deterministic — after removing the bad `memory_flush` calls
the suite passed 23/23 in three consecutive full runs, while one earlier full
run in the same session failed exactly one phase-0 assertion in
`gpu_draw_indirect`. A roughly 1-in-N first-draw-in-a-list intermittency on
this AMD driver would be worth ruling out before D3's interactive work; the
`gpu_closure_ab_probe` and the mixed fixture are the instruments (run them in
a loop under load). Nothing else in the suite has shown flakiness across the
session's hundreds of runs.

**Soak data:** 8 runs each of `gpu_draw_indirect`, `gpu_closure_ab_probe` and
`gpu_compute_to_raster` in isolation (24 total) and 5 consecutive full-suite
runs — all green. The single observed failure remains the one full-suite run
earlier in the session (one phase-0 assertion in `gpu_draw_indirect`), i.e. a
low single-digit-percent intermittency under this driver that a dedicated
soak (hundreds of runs, ideally under GPU load) should characterize before
D3's interactive work. The instruments and the counter-evidence probe are
checked in for exactly that purpose.

**Second soak data point:** another full-suite run failed exactly one
gpu_draw_indirect phase-0 assertion (and the immediately following runs were
clean), bringing the observed rate to roughly 2 in 15 full-suite runs. The
intermittency reproduces across sessions of work and remains confined to that
one test's first-slot draw.

## The M4 scene on Windows: one resource-lifetime bug remains

The scene consumer now runs through its entire setup and frame recording on
the D3D12 backend: Win32 window, surface and every capability query, swapchain
with borrowed images, the scene and overlay pipelines (DXIL variants), the
geometry/index/descriptor-heap resources, all state transitions (including
the PRESENT state and the transfer-to-host readback barrier), and the full
render and copy recording. Four real defects were found and fixed on the way:

1. `image_placement` rejected committed dedicated backing — the exact kind it
   exists to adopt (a binding-order bug).
2. `bind_indices` demanded a bound executable; the contract says index
   binding persists across binds and needs none. (`bind_vertices` keeps its
   executable because the contract's "pipeline static strides are left
   unchanged" makes the bound pipeline the stride source.)
3. Host-side barrier parts (`STAGE_HOST`, host accesses) were rejected; they
   lower to no native sync bit, which is the D3D12 truth (map and fence order
   host access). Valid contract readback barriers now record.
4. `Present(0, 0)` is invalid in DXGI: immediate present requires
   `ALLOW_TEARING` on both the swapchain and the Present flags; the mapping is
   now mode-driven.

The remaining failure is precisely characterized: `ID3D12GraphicsCommandList::
Close` rejects with E_INVALIDARG because "An ID3D12Resource object ... was
deleted prior to closing the command list" — one recorded resource dies
before the list closes. The debug layer names the object address; no Memory
or Image drop precedes it, so the owner is a temporary reference in the
recorded path (or a consumer-side binding). This is the last line to the M4
scene executing on Windows.

Tight image-copy pitches (the contract's `row_pitch: 0` form) are now
accepted and forwarded to the native footprint: the consumer's tight uploads
recorded without driver complaint, and the 256-alignment check remains
removed pending a driver-level verdict during that final debugging.

## Presentation validated end to end; four more defects fixed

The consumer's presentation path executes on the driver: the swapchain
presents successfully (`Present` returns S_OK with the tearing mapping),
binary signals retire, and the interactive scene banner appears. Four more
real defects were found and fixed, each with a regression test in the suite:

1. **Global barrier lowering.** The native global barrier expresses one
   implied layout per side (validated by the debug layer): a mask mixing
   layout-implying accesses splits into the cross product of its layout
   groups, each record's sync bits pairing with its own accesses. The one
   native exception discovered empirically: a DEPTH_STENCIL_WRITE before-side
   is accepted only against a DEPTH_STENCIL_READ after-side; other
   depth-write pairs lower to sync-only records whose cache ordering rides
   on the caller's per-image texture barriers (the contract's own model).
   `gpu_layout_access_dependency` covers the consumer's exact shape.
2. **Binary semaphores in submissions** lower to native queue wait/signal
   pairs around the submitted lists (the frame-sync extension records).
3. **Copy-direction memory rules** (native D3D12): textures exist only on
   non-host-visible types, UPLOAD-heap resources are copy sources only and
   READBACK-heap resources copy destinations only. `gpu_memory_compatibility_rules`
   pins the report.
4. **Command-list closing** (from the previous batch's diagnosis): only open
   recordings are closed; `gpu_list_reuse_after_retirement` pins it.

The debug layer is now enabled best-effort at device creation and native
failures dump the info queue (validation evidence per the review gate), with
resources named at creation so reports identify them.

### Clip-space Y (Vulkan NDC on D3D12)

ogpu clip space is Vulkan's: `+Y` is down in framebuffer space. D3D12's native
viewport maps `+Y` up. `RSSetViewports` therefore uses a negated height and a
shifted `TopLeftY` so the same clip-space positions produce the same pixels as
Vulkan. Scissor stays in framebuffer pixels (top-left origin) and is not
flipped. Winding is computed after the viewport transform, so matching
framebuffer positions also match Vulkan front-face without a PSO invert.
`gpu_scene_raster` samples the asymmetric second quad at Vulkan top-right
`(30, 2)` rather than D3D bottom-right. Tight `row_pitch: 0` copies whose pitch
is not 256-aligned record one native `CopyTextureRegion` per row (D3D12's
placed-footprint rule) instead of silently changing the layout.

### M4 `--scene-check` on Windows (green)

`examples/foundation_rust --bin surface -- --scene-check` completes:
`SCENE_PASS frames=18 slots=3 peak_pending=3 resize=2 checked_pixels=36729`.
The remaining hops after clip Y, each reduced to a suite pin before the fix:

1. **Heap-then-executable tables.** The consumer binds descriptor heaps before
   the graphics executable. `bind_executable` applied tables from the stored
   pointers *before* storing the new bind, so graphics tables never landed and
   samples were unbound (`[1,0,3,255]`). Store the pointer, then
   `apply_descriptor_state`. `gpu_scene_consumer_clip_sample` uses the consumer
   bind order.
2. **Binary wait consumes the signal.** A waited binary stayed `pending`, so
   the fourth acquire (same slot) returned `INVALID`. `wait_on` clears pending
   (Vulkan unsignal). `gpu_swapchain_present` acquires and presents twice with
   the same objects.
3. **Recording bind state dies with the list.** `commands_begin` reuses a slot
   after `arena_reset` without clearing executable/heap pointers, so `bind_heap`
   on a fresh list called `SetComputeRoot32BitConstants` with no root signature.
   Begin now clears bind state.
4. **`ResizeBuffers` flags and GetBuffer refs.** Immediate chains are created
   with `ALLOW_TEARING`; resize must pass the same flags. DXGI also forbids
   `ResizeBuffers` while `GetBuffer` references remain: the old chain drops its
   image vec first.
5. **480-wide readback pitch.** D3D12 placed-footprint pitch is 256-aligned.
   640×480 is already aligned; 480×320 is not (`1920 % 256 != 0`). The consumer
   oracle now uses `row_bytes(w) = (w*4).next_multiple_of(256)` for copy pitch,
   buffer size, and sampling. 64- and 128-wide unit tests are unchanged.

### Image-to-image copy (native) and blit (honest subset)

`copy_image` lowers to `CopyTextureRegion` between textures (`gpu_copy_image_and_blit`).
Same-size, unreversed, nearest, same-format `blit_image` uses that copy; scaling,
axis reversal, LINEAR filter, format conversion and MSAA resolve stay
`UNSUPPORTED` (no shader blit, no resolve profile). `present` moved out of
`stubs.rs` onto `Queue` next to submit (stub-shadowing).

Linux `cargo test -p ogpu`: 110 + 2 host tests pass (the Windows-only
`presentation::{Surface, Swapchain}` re-export was cfg-gated).

### Remaining honest gaps

| Surface | Status |
| --- | --- |
| `Split` / split barriers | `UNSUPPORTED` (enhanced barriers cover the recorded dependency model) |
| `GeneratedLayout` / `GeneratedSet` / preprocess | `UNSUPPORTED` (plan: do not force Vulkan generated-command objects onto D3D12) |
| `draw_mesh*` | `UNSUPPORTED` on this RDNA1 adapter |
| Learned-image / nested GPU pointers | `UNSUPPORTED` (ROOT_DESCRIPTORS profile; toolchain has no DXIL nested pointers) |
| M5 transformer consumer | not attempted; same argument-profile limit |
| Secondary bundles (`execute` lists) | `UNSUPPORTED` |
| `gpu_draw_indirect` first-slot | ~1-in-10 intermittency under load, labeled |
| `PRESENTATION_WIN32` | reviewer: HWND still travels in the Wayland surface record |

Initial-acceptance evidence for this branch: D1 core, D2 compute/descriptors/
offscreen graphics, D3 DXGI presentation, M4 `--scene-check` on the RX 5700 XT.
