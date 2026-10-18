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

## Remaining D2 scope

- Descriptor profile: `write_resource_descriptors`/samplers, DESCRIPTOR_HEAP
  memory as native descriptor heaps, `bind_heap` tables (D2.2).
- Offscreen graphics: render scopes, graphics executables/PSOs, viewport and
  dynamic raster state, indexed drawing, image clears (D2.3).
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
