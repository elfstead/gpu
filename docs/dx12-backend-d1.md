# D3D12 foundation backend: D1 core batch

Branch `dx12-backend`. Updated 2026-10-17. **This batch implements the D1
resource/command core of the [DX12 candidate plan](dx12-candidate-plan.md) and
exercises it natively on one machine. It is not a completed backend: D2
(executables, descriptors, graphics), D3 (presentation, advanced profiles) and
the shader-address question remain open, and nothing here is merged or
accepted.** Review is a separate step; this document is the implementation
receipt the reviewer asked for.

## What is implemented

Scope follows the D1 batch exactly: foundation separation, discovery, requested
queues, fence/timeline operations, explicit memory/images, copy/readback,
arenas/lists, dependencies and submission errors — through both host interfaces
(the shared C ABI and the native Rust API, which never calls the C adapters).

| Area | State |
| --- | --- |
| Foundation separation | Shared C ABI, records, boundary checks and the native Rust layer are backend-neutral; Vulkan implementation moved behind a per-target `native` module, unchanged |
| Discovery | DXGI adapter enumeration, cached immutable snapshots, per-adapter physical capability probes; adapters without a D3D12 device are listed with `baseline_supported = 0` |
| Queues | Direct/compute/copy domains, explicit counts, two native priority classes |
| Timelines | `ID3D12Fence` mapping: host signal, poll, bounded wait with timeout semantics |
| Memory | Explicit heaps with placed whole-allocation buffers (linear) and opaque placement heaps; map/unmap; flush/invalidate are ordered no-ops (all CPU-visible D3D12 memory is coherent) |
| Images | Exact requirements, one-shot placed binding, dedicated committed backing; buffer/image copies in both directions (readback) |
| Command storage | Arena slot pools of allocator/list pairs; one-shot, serial and simultaneous replay modes; capacity counts slots exactly |
| Dependencies | Enhanced-barrier mapping (global/buffer/texture groups) through caller-provided scratch |
| Submission | Waits/execute/signals on the queue, one-shot consumption, scratch-based native list translation, conservative submission failure poisoning |
| Budgets | Live per-heap DXGI video-memory queries with the full count/capacity protocol |
| Formats | All 172 catalog formats listed; per-format native capability probes; vocabulary entries without a DXGI equivalent report zero capabilities and are never substituted |

Everything else returns `UNSUPPORTED` and is listed under
[Open mapping work](#open-mapping-work). No emulation hides those gaps.

## Environment and receipt

| Fact | Value |
| --- | --- |
| Base commit | `2374635` (plan) |
| Windows build | Windows 11 build 26100, MSVC 14.44 (VS 2022 BuildTools), Windows SDK 10.0.22621 |
| GPU / driver | AMD Radeon RX 5700 XT (RDNA1), driver 32.0.21045.5002 (2026-08-17) |
| Rust | 1.99.1 `x86_64-pc-windows-msvc` (tests), 1.92.0-nightly `x86_64-unknown-linux-gnu` (regression) |
| Bindings | `windows =0.62.2`, narrow features (Foundation, Direct3D, Direct3D12, Dxgi, Dxgi.Common, Security, Threading) |
| Agility markers | `D3D12SDKVersion`/`D3D12SDKPath` exports pinned to Microsoft.Direct3D.D3D12 1.613.1; see [incidents](#incidents-and-fixes) for the deployment status |
| Shader compilers | Not used in this batch |

Commands (from a Windows toolchain with the repository visible):

```
cargo test -p ogpu --no-default-features                       # host tests
cargo test -p ogpu --no-default-features -- --ignored          # native GPU tests
cargo test [--no-default-features]                             # Linux regression
```

Results on the machine above: 18 host tests pass on Windows; all 6 native GPU
tests pass (including the shared `gpu_foundation_setup` and
`gpu_native_recording_unwind` suites run against the D3D12 backend); Linux
regression passes (110 + 67 tests, 0 failures, both feature sets).

## Per-profile status

Specified / implemented / natively exercised / unavailable / unverified:

| Profile | Specified | Implemented | Exercised | Unavailable | Unverified |
| --- | --- | --- | --- | --- | --- |
| Discovery/queues/timelines | yes | yes | yes | — | concurrent host access patterns beyond the test matrix |
| Linear memory, map, address | yes | yes | yes | — | over-alignment padding beyond 256 B request |
| Opaque memory, placed images | yes | yes | yes | tier-1 RT/DS placement (reports `dedicated_required`) | multi-image aliasing lifetimes |
| Dedicated image backing | yes | yes | — | — | bind-after-dedicated matrix |
| Buffer copy, memory barrier | yes | yes | yes | — | multi-queue domain handoff |
| Buffer/image copies, readback | yes | yes | yes | depth/stencil copies (separate profile), non-256-B-row pitches (native footprint constraint; no hidden staging) | compressed footprints beyond the tight-pitch path |
| Submit, one-shot consumption | yes | yes | yes | binary semaphore extension (D3) | submit-batch ordering beyond the two-batch case |
| Budgets | yes | yes | yes | — | budget-change notifications |
| Fill (buffer pattern) | yes | — | — | no direct native command; descriptor-based alternative belongs to D2 | — |
| Standalone image clear | yes | — | — | clears need view descriptors (D2) | — |
| Views, samplers, descriptors | yes | — | — | views are descriptor contents on D3D12 (D2 profile) | — |
| Executables, compute, graphics | yes | — | — | D2 | — |
| Secondary lists/bundles, splits, queries, generated commands, mesh/task | yes | — | — | D2/D3 | — |
| Presentation (DXGI) | yes | — | — | D3 (needs the external-window surface record) | — |

## Resolved mapping decisions

These are the D1 rows of the plan's mapping matrix. Each is an explicit
alternative where D3D12 differs, recorded for the reviewer.

1. **Queues and completion.** Direct/compute/copy domains with an explicit
   software bound (16) on requestable queues per domain; D3D12 exposes no
   queue-family count. Priorities map to the two native classes (1.0 selects
   HIGH, [0,1) selects NORMAL). Fences map directly to timeline semantics;
   `max_timeline_difference` is reported as the full 64-bit counter.
2. **Command storage.** Arena slots own allocator/list pairs; replay modes need
   no native flags (D3D12 permits resubmission), and re-recording happens only
   after arena reset, which requires no pending use by contract. Abandoned
   recordings are defensively closed at slot reuse so allocator reset is legal.
3. **Backing.** Linear memory is one placed buffer over one heap, giving the
   exact one-addressable-backing semantics with bounded prefix padding.
   Opaque memory is a heap for placed resources. Host-visible opaque memory
   cannot be mapped: D3D12 has no heap map operation, and a covering mapping
   buffer would occupy the heap. This returns `UNSUPPORTED` instead of being
   emulated.
4. **Dependencies.** Enhanced barriers only; no legacy transition fallback. The
   native sync scope of each barrier side is derived from its declared accesses
   and narrowed to the declared stage scope: D3D12 validates sync/access
   pairings (`SYNC_COPY|SYNC_RESOLVE` with pure copy accesses is rejected by
   native validation), and stages that cannot produce an access are exact to
   drop. Buffer barriers additionally drop texture-only scopes
   (resolve/render-target/depth-stencil). HOST access scopes and descriptor-heap
   accesses are rejected (`UNSUPPORTED`), never dropped. Ownership endpoints
   (non-IGNORED domains) map to ordinary scoped barriers plus the caller's
   queue fence edge, because D3D12 has no resource-ownership state; domains are
   validated but no ownership bookkeeping is added.
5. **Image states.** Explicit states map 1:1 to native layouts; `STATE_GENERAL`
   resolves deterministically from its paired access mask (storage writes →
   UNORDERED_ACCESS, shader reads → SHADER_RESOURCE, copy accesses → the
   matching copy layout, otherwise COMMON). This is a documented difference
   from a fixed layout table and is pinned by unit test.
6. **Submission.** Waits, one `ExecuteCommandLists` and signals per descriptor;
   batch submission is one ordered sequence of native queue operations (D3D12
   has no single batched call) — a recorded alternative, not a hidden
   flattening. Scratch holds only the translated native list pointers; batch
   scratch is zero-size on this backend. Ordinary submission errors poison the
   device (`BACKEND_ERROR`, then `DEVICE_LOST`); recording/close errors are
   local and leave the device usable except for device removal.
7. **Memory types/heaps.** The three standard heap types are the memory types;
   physical heaps are the native memory pools the types actually map to (a UMA
   or software adapter may expose one). Heap sizes are honest lower bounds:
   DXGI exposes no exact physical segment size, so the snapshot reports the
   pool's backing memory with the discovery-time budget as a floor, keeping all
   later budget queries bounded by the reported size.
8. **COM ownership.** Interface values own exactly one reference (released on
   drop); barrier/copy translation borrows scratch-resident interface
   references that are never released. Cold construction queries and creates;
   recording and submission perform no AddRef/Release and allocate nothing
   beyond caller scratch and arena slots.

## Open mapping work

- **Shader addresses and arguments (the plan's first decision)** is untouched:
  no argument/pointer profile is claimed (`argument_flags = 0`,
  `max_inline_size = 0` until D2 resolves root signatures versus the pointer
  strategy).
- Descriptor/graphics/query/executable records zero their limits in the D1
  snapshot (no claim is made); D2 fills them with honest queries.
- The `fill` and standalone `clear` commands need descriptor-backed clears;
  they return `UNSUPPORTED` rather than staging.
- Row pitches for buffer/image copies must satisfy the native placed-footprint
  alignment (256 bytes); other pitches return `UNSUPPORTED` rather than
  staging.
- Depth/stencil copies, multisampled storage, view-format lists and secondary
  bundles are separate profiles.

## Incidents and fixes

Recorded in full because the review gate requires incident honesty:

1. **GPU hang (TDR) during early testing.** A test destroyed its arena, memories
   and device while its submitted copy was still executing — undefined behavior
   in D3D12 that wedged the driver (`DXGI_ERROR_DEVICE_RESET` on later tests,
   an AMD driver-timeout report and a `Kernel_141` WER record). Fix: tests now
   retire submitted work on their timeline before any destruction, exactly as
   the contract requires. The same retirement rule is now pinned in test code.
2. **Texture-barrier rejections.** Enhanced-barrier sync/access pairing
   validation rejected `STAGE_TRANSFER` scopes (copy+resolve) attached to pure
   copy accesses. Fix: per-side sync derivation from declared accesses (see
   decision 4). Bisection was done record-only to avoid executing invalid work.
3. **A borrowed-pointer bug** (`&interface_wrapper` bit-copied instead of the
   COM pointer) put garbage resource pointers in barrier/copy locations;
   caught by native rejection at close, fixed to borrow interface references
   explicitly.
4. **Debug-layer deployment is unresolved.** The Agility SDK markers are
   exported correctly (verified in the PE export table of the test binaries),
   but on this OS build the loader still selects the system D3D12 runtime, so
   `D3D12GetDebugInterface` reports `DXGI_ERROR_SDK_COMPONENT_MISSING` without
   the Graphics Tools optional feature. CPU-side validation runs therefore
   remain unverified; the native test evidence above was produced without the
   debug layer. Note for the reviewer: an array-typed `D3D12SDKPath` export
   (instead of the documented pointer variable) caused injected driver
   components to dereference garbage and hang every new process — the marker
   layout is ABI-sensitive and is pinned by comment in `dx12_sdk.rs`.

## API-change ledger

- `include/ogpu_next.h` is unchanged: this batch implements its Linux-neutral
  declarations on a second native backend.
- Shared `types.rs` gains the missing stage/access/image-state constants from
  the C header vocabulary and formatted-buffer feature bits (`format_buffer_features`);
  values match the header exactly (checked by the existing catalog test for formats).
- `lib.rs` gains `BACKEND_D3D12 = 3` alongside the existing backend constants.
- Foundation internals: `Adapter` now holds backend-native discovery state in a
  per-target `native::AdapterState`; `discover`/`discover_presentation` moved
  behind the native module. The Vulkan implementation and its tests are
  behaviorally unchanged (Linux regression passes).
- Windows builds add the `windows` crate (pinned, narrow features) and the
  Agility SDK deployment markers in `dx12_sdk.rs`.

## Follow-up scope

D2 (address/argument profile decision first, then compute/executables,
descriptors, offscreen graphics) and D3 (DXGI presentation with an external
window record, task/mesh and generated commands) as defined in the candidate
plan, plus resolving the debug-layer deployment for validation runs. None of
this is merge authorization; the reviewer owns the regression pass and the
acceptance entries.
