# DX12 candidate backend: Windows implementation handoff

Decision: 2026-10-17. **Candidate approved for branch implementation and review;
not implemented, validated or accepted as a supported backend yet.**

The Windows implementor should start a `dx12-backend` branch from the commit
containing this plan, commit coherent implementation batches and return the branch
with native test evidence for review. Do not merge to `master` as part of this
handoff. Linux M8 work continues independently; Metal M6 remains deferred until
a Mac is available. DX12 is a parallel backend track, not a renumbering of M6–M11.

## Objective and design rules

Implement a modern native D3D12 foundation backend in Rust, exposing the common
C and native Rust interfaces. Target explicit graphics, compute and ML building
blocks, not a scene runtime, operator library or WebGPU compatibility layer.

Follow the [whole-API design](whole-api-design.md) and
[foundation contract review](core-contract-review.md). In particular:

- Preserve efficient native strategies, not every detail of the Vulkan mapping.
  Change or extend the API when the mapping **exposes a better API alternative**,
  even when the existing API could technically be emulated. Document the change
  and its consequences for Vulkan and the proposed Metal profile.
- Query native capabilities and explicitly enable optional profiles. Require
  modern facilities where appropriate; do not build legacy fallback machinery.
  Record exact Windows SDK, Agility SDK, shader/compiler and driver requirements.
  A published specification or compiler target is not evidence of driver support.
- No hidden copies, staging, pointer lookup tables, host command expansion,
  pipeline compilation, automatic barriers, queue bridging, retention walks or
  implicit waits to manufacture parity. Preparation and optional helpers may
  allocate; ordinary recording/submission must have explicit storage policy.
- Unsupported strategies remain visible. Do not remove a Vulkan strategy merely
  because D3D12 lacks it, or advertise a D3D12 capability with weaker semantics.
- Keep object-local host synchronization and explicit asynchronous lifetimes.
  Rust types, generated artifact metadata and cold validation should improve
  safety without making ownership/scheduling policy mandatory on the hot path.

## First decision: shader addresses and arguments

This is the largest unresolved mapping, not a detail to hide behind a backend trait.
The Vulkan foundation supports physical shader pointers, pointer-containing data,
address arithmetic, independently updated root addresses and shared inline bytes.
D3D12 GPU virtual addresses used in root descriptors do **not**, by themselves,
establish equivalent arbitrary shader-side pointer support.

On the actual Windows toolchain, establish which native artifact/binding profiles
are available. Compile small probes for a root binding, nested pointer access,
GPU-produced references, indexed descriptors and partial argument updates. Inspect
generated shader code/binding metadata; do not infer support from source syntax.

For each operation, record one of:

1. Direct native mapping with matching guarantees.
2. Explicit native alternative with different artifact/API semantics.
3. Unsupported, with the exact missing guarantee.

A descriptor-index-plus-offset reference can be a useful native profile, but is
not a physical address and must not silently replace one. Do not introduce a
giant virtual buffer or address-to-resource registry to pretend otherwise. Preserve
the unrestricted pointer strategy on backends that support it. A native D3D12
resource-binding profile may still enable substantial useful implementation;
label its consumer coverage honestly rather than claiming pointer-based consumer
parity. If a viable profile is unresolved, continue independent setup/resource
work and return the specific design question instead of inventing emulation.

## Required mapping matrix

Write the resolved matrix in this branch before claiming backend acceptance.
The following are candidates, not assertions of semantic equivalence:

| OGPU area | D3D12 candidate | Decision to establish |
| --- | --- | --- |
| Discovery and enabling | DXGI adapters, D3D12 feature queries | Honest profile-specific baseline, limits and optional features; no Vulkan baseline prerequisites |
| Queues and completion | Direct/compute/copy queues, fences | Queue-class/count model, signal/wait ordering, host concurrency and loss; no fabricated physical queue-family topology or stage-scoped waits |
| Command storage | Allocators, command lists, bundles | Reset/retirement, replay and simultaneous-use guarantees, bundle restrictions; no replay by host re-encoding |
| Backing and images | Heaps, placed resources, allocation queries | Native alignment/types, buffer/image placement, aliasing, mapping/cache rules and residency granularity; no forced all-resource mega-buffer |
| Arguments | Root signatures, constants and root descriptors | Limits, stage namespaces, partial updates and invalidation across executable changes; no emulated shared-byte bank |
| Descriptors | Native descriptor heaps and direct heap indexing | Native allocation/encoding/mutation/copy rules and lifetimes; no promise of portable or directly writable descriptor bytes |
| Dependencies | Enhanced barriers, queue waits/signals | Exact sync/access/layout rules, aliasing and split operations; reject or explicitly profile incompatible scopes |
| Executables | Native shader artifacts and prepared pipeline/state objects | Actual compiler ABI, specialization strategy, caches, error behavior and static/dynamic state; no recording-time compilation |
| GPU-driven work | Command signatures and ExecuteIndirect; newer facilities only when available | Supported tokens, counts, identity, root/state updates, executable choice, preparation and post-execution invalidation |
| Presentation | Optional DXGI swapchains with externally supplied windows | Acquire/backpressure, render versus presentation retirement, resize, modes, borrowed images and teardown |
| Residency and interop | Explicit native residency and sharing operations | Caller-visible obligations and ownership; pull needed M10 prerequisites forward without claiming all of M10 |

Use one efficient mechanism for equivalent operations; preserve genuinely different
strategies as explicit alternatives. A modern root-signature/heap profile is
preferable to implementing every historical binding tier. Compute must not acquire
an unnecessary mesh/ray/window requirement just to simplify a global baseline.

## Implementation batches

These are coherent delivery batches, not a request for a separate benchmark or
approval round trip for each function. Implement as far as the native mapping and
available hardware support, keeping unresolved contracts explicit.

### D1 — Foundation separation, discovery and resource/command core

- Start from foundation revision 1 (`include/ogpu_next.h`), not legacy ABI20.
  Current foundation modules are Linux/Vulkan-coupled: `lib.rs` gates foundation
  to Linux, while `foundation/mod.rs` owns Vulkan instance/adapter types. Extract
  genuinely shared records, boundary checks and host contracts as needed.
- Use target-specific native modules beneath both host interfaces. Native Rust
  must not call the C adapters. Avoid duplicating a second public API policy layer
  or forcing a generic Vulkan-shaped backend trait before mappings are known.
- Use narrow, pinned Rust Windows/COM bindings as appropriate. Avoid introducing
  a C++ implementation layer. Native COM ownership must be explicit; acquiring
  references on cold construction is different from mandatory per-command retention.
- Implement discovery, requested queues, fence/timeline operations, explicit
  memory/images, copy/readback, arenas/lists, dependencies and submission errors.
  Add backend/profile discovery where the current records assume Vulkan semantics.
- Preserve existing Linux builds and the legacy Metal path; do not delete them
  during extraction. Build checks alone are not native regression acceptance.

### D2 — Native artifacts, compute, descriptors and offscreen graphics

- Resolve the address/argument profile above and implement it through C and Rust.
  Prefer Slang tooling that produces verified native artifacts and host layout
  metadata. Record any necessary toolchain/profile alternatives; source portability
  is not proof of identical artifact layout or native strategy.
- Implement prepared compute and conventional graphics executables, explicit
  descriptor heaps, images/views/samplers, indexed drawing and indirect/count work
  supported by the selected native profile. Add formats and optional numeric
  capabilities only with honest queries and contract coverage.
- Run compute/copy, textured offscreen raster and mixed compute-to-raster fixtures.
  Reuse existing CPU oracles and semantic tests where possible; do not reuse
  Vulkan shader ABI assumptions or make a different workload look identical.
- Attempt the foundation Rust consumer, learned-image/heap workload and M4/M5
  consumers as their native profiles become expressible. List unsupported paths
  individually. A small compute fixture does not establish graphics or ML parity.

### D3 — Windows presentation and advanced native profiles

- Implement the optional native presentation boundary and run an interactive
  multi-frame scene. Window/input handling stays external. Do not implement a fake
  Vulkan acquire/present protocol if DXGI exposes a better native API alternative.
- Map task/mesh and generated commands where available, then report each token,
  stage and execution mode separately. Our latest Vulkan generated-command tranche
  is not a universal backend contract: do not force its preprocessing object model
  onto an API without the corresponding native operation.
- Audit ray tracing, advanced raster and numerical features against M8–M11. Pull
  useful shared prerequisites forward; do not make full hardware-family parity
  a prerequisite to reviewing a useful, honest initial backend.

## Verification and review gate

Return reproducible commands, checked-in fixture sources/artifacts or generation
instructions, and a result receipt containing:

- Base and tested commit, Windows build, GPU/driver, SDK/Agility version, Rust,
  Slang and any native shader compiler versions. State required native features.
- A per-profile table: specified / implemented / natively exercised / unavailable /
  unverified. A skipped test is not a pass. WARP is useful software-driver evidence,
  not physical-GPU performance or coverage of unsupported hardware features.
- Host tests and C/Rust ABI checks on Windows, including native layout/calling
  conventions and export/link checks. Exercise C and direct Rust consumers.
- D3D12 debug-layer and GPU-based-validation runs where supported. Check all output
  bytes/pixels and guards with independent expected results. Include malformed
  input, feature-off rejection, poisoned recording and failure-output ownership.
- Multi-frame reuse, changed arguments, descriptor mutation of independent ranges,
  explicit aliasing, replay where advertised, concurrent independent recording,
  cross-queue handoffs where supported and destruction only after retirement.
- Separate tests for ordinary rejection versus device removal/unknown submission
  outcome. Never treat an uncertain submission failure as proof that resources
  are safe to destroy. Synthetic loss tests are not real-device-loss evidence.
- Presentation checks: multiple frames in flight, resize, pause/resume,
  minimize/restore and close with pending work; distinguish automated and human
  observations and render completion from presentation retirement.
- A structural lowering review: no added mandatory copies, shader indirections,
  global locks, retention walks, command-time allocation or hidden waits. Compare
  with the efficient native D3D12 strategy, not just today's Vulkan implementation.
  Measurements investigate uncertain overhead; obvious structural disadvantages
  are already grounds to change the mapping. Keep validation runs separate from
  timing and do not claim universal Vulkan-equivalent performance from samples.
- An API-change ledger, remaining gaps, proposed follow-up scope and any required
  Linux verification after rebase. The reviewing Linux agent will run available
  regressions; Windows-only compilation cannot certify preservation of Vulkan.

Initial acceptance means a reviewed native core plus useful compute/offscreen
graphics and reproducible evidence for its declared profiles. Presentation and
advanced profiles have their own acceptance entries; neither partial acceptance
nor merging a branch implies all consumers, hardware families or platforms work.
No stable ABI, release publication or universal performance promise is authorized.

## Reference starting points

Verify against the SDK actually used; these describe native mechanisms, not
guaranteed availability on the Windows test machine:

- [D3D12 resource binding](https://microsoft.github.io/DirectX-Specs/d3d/ResourceBinding.html)
  and [dynamic resource indexing](https://microsoft.github.io/DirectX-Specs/d3d/HLSL_SM_6_6_DynamicResources.html).
- [Enhanced barriers](https://microsoft.github.io/DirectX-Specs/d3d/D3D12EnhancedBarriers.html).
- [Indirect drawing/execution](https://microsoft.github.io/DirectX-Specs/d3d/IndirectDrawing.html)
  and [newer Work Lists specification](https://microsoft.github.io/DirectX-Specs/d3d/WorkLists.html).
- [Slang pointer types and target limitations](https://docs.shader-slang.org/en/stable/external/slang/docs/language-reference/types-pointer.html).
- [Metal mapping proposal](m6-native-mapping.md), for shared design questions,
  not an implemented abstraction to copy.
- [Current generated-command contract and evidence](results/m8-generated-commands-2026-10-17.md).

WebGPU is not part of this implementation brief. A browser-compatible restricted
layer would be a separate product/profile decision, not a reason to weaken OGPU's
native foundation.
