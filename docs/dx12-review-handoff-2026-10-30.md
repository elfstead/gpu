# DX12 review handoff: corrections before merge

Review date: 2026-10-30. Reviewed branch: `origin/dx12-backend`, commit
`34930b0` (against `master` at `2374635`). **Disposition: do not merge yet.**
This is a source-review handoff, not a claim of Windows reproduction or an
exhaustive audit. All findings below are open at the reviewed commit. Line numbers
refer to that commit and will move as fixes land; function names identify targets.

This review is also carried on the Vulkan-only `minimal-core` branch. Source links
are pinned to the reviewed DX12 commit; that implementation is not in this branch.

Design follow-up: the [cross-backend API-shape assessment](cross-backend-api-shape-2026-10-30.md)
and [fresh NGPU implementation comparison](ngpu-comparison-2026-10-30.md) explain
which corrections need a contract revision rather than preservation of existing
Vulkan semantics. The subsequent [minimal-core decision](design.md#minimal-composable-core)
and [C1–C3 roadmap](roadmap.md#minimal-core-revision) supersede the prescription
to add parallel profiles: first derive alternatives through one simple mechanism.
Resolve the selected contract before declaring R2/R3/R8 closed;
correctness-only fixes can proceed independently. These documents do not close
any finding or prescribe copying NGPU's narrower feature/policy choices.

The branch contains useful native compute, graphics and presentation work and a
reported successful Windows M4 scene check. Preserve that work. However, the
current argument, descriptor and presentation mappings introduce emulation or
contract mismatches that the [candidate plan](dx12-candidate-plan.md) explicitly
asked us to avoid. Correct those mappings and the concrete bugs below before
expanding feature coverage. API changes are welcome when they **expose a better
API alternative**; compatibility with the provisional mapping is not a goal.

## Evidence boundary

The reviewer inspected the branch and its Windows receipts in an isolated source
snapshot, without merging or modifying implementation code. Independently run on
Linux at the reviewed commit:

- `cargo test -p ogpu --no-default-features --locked`: 67 host unit tests and two
  native-Rust surface checks passed; GPU/window tests remained ignored there.
- `cargo test -p ogpu --no-default-features --lib -- --ignored`: all 21 headless
  Vulkan tests passed on the local Radeon, with an explicit Vulkan loader/ICD.
  This run is not claimed as a validation-layer run.
- Rust/Cargo 1.98.1. No Windows execution, Windows ABI check, full C foundation
  fixture or interactive presentation rerun was performed by this reviewer.

The Windows scene result is implementor-reported evidence, not independently
recertified here. The existing D2 receipt also reports intermittent first-slot
indirect-draw failures. Their cause remains unresolved; the bugs below are not
proof of the cause of that intermittency.

## R1 — P1: global dependencies can emit no barrier

Location: [commands.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/commands.rs),
`List::translate_dependency`, lines 459–511; `global_groups`, lines 862–873.

For `STORAGE_WRITE -> INDIRECT_READ`, the before-side has a nonzero grouped access
but the after-side has only a layout-free access. The nested loop skips every
pair; the fallback requires **both** grouped sides to be empty. No global barrier
is emitted. The reverse shape, such as copy-write to shader-read, has the same
problem. Valid GPU-producer/consumer dependencies therefore disappear.

Fix the lowering for grouped/free combinations on either side. Also audit the
depth-write special case that substitutes `NO_ACCESS` for requested memory
dependencies: do not assume callers supplied per-image barriers to rescue a
global dependency. Reject an unrepresentable operation or expose its native
alternative instead of silently weakening it.

Acceptance: deterministic lowering tests for grouped/grouped, grouped/free,
free/grouped, free/free and execution-only dependencies; native compute-written
indirect arguments/counts consumed with only the declared global dependency;
copy-to-shader and depth-related cases. Check exact scopes/accesses, not just
whether a native barrier call occurred.

## R2 — P1: descriptor binding rebuilds and mutates the entire range

Location: [commands.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/commands.rs),
`List::bind_heap`, lines 1427–1450; recipe encoding/materialization in
[descriptors.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/descriptors.rs).

Every bind scans recipe memory and creates native descriptors for every slot in
the bound range. This imposes O(heap-size) recording work and rewrites slots that
earlier submitted work may still reference. Updating recipe bytes after recording
does not update the native heap unless another bind materializes them. The two
representations do not preserve the existing independent-update contract.

Replace this with the C1-selected descriptor-storage contract: cold heap/storage
creation, explicit descriptor writes/copies to caller-selected slots and cheap
binding. Do not disguise recipes as execution-visible descriptor bytes, scan
whole heaps during recording or infer pending-resource ownership. An opaque native
heap is a strong candidate; public byte access is retained only if the composition
analysis justifies it. Update Vulkan consumers coherently if the contract changes.

Acceptance: disjoint slot updates while other slots are pending; updated slots
observed at the documented execution boundary without rebinding/rebuilding;
independent host encoders; no native descriptor writes during bind and no
heap-size-dependent bind cost. Declare native lifetime and synchronization rules.

## R3 — P1: argument restoration corrupts independent pipeline state

Location: [commands.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/commands.rs),
`bind_executable` (1062–1080), `emit_arguments` (1088–1127), `inline`
(1158–1191), `apply_descriptor_state` (1477–1528).

Each executable bind sets both native compute and graphics root signatures.
Restoration then visits previously bound compute and graphics executables and
writes both namespaces with each executable's own parameter indices/layout.
Different signatures can receive incompatible root writes. The cached bound
executable state no longer reliably describes the native state.

The shared-byte-bank emulation also adds mandatory shadowing/restoration,
`Vec` allocation in heap-state application, and allocation when inline updates
overlap root slots. COM clones and unconditional trace output add further hot-path
work. This is not the native-profile alternative requested by the handoff.

Use C1's selected argument interface with explicit invalidation/preservation
semantics, separating pipeline classes correctly. One logical root is the starting
point; do not add multiple slot/constant profiles merely to preserve earlier
signatures. Do not advertise `SHARED_BYTES` unless its full semantics have a suitable
native mapping. Prefer a better common mechanism over automatic restoration.
Remove hot allocation and unconditional logging; put diagnostics behind an
explicit opt-in.

Acceptance: alternating compute and graphics with deliberately incompatible native
interfaces, generated layouts and observation/reuse cases allowed by the selected
contract; heap/pipeline ordering and compute-only queues. Old multi-root/partial-
update cases must have a documented replacement or retained primitive in C1's
migration table, not be silently dropped. Inspect emitted calls and
test that ordinary binding/update/dispatch adds no OGPU heap allocation.

## R4 — P1: surface ownership leaves a dangling adapter pointer

Location: [presentation.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/presentation.rs),
`Surface::create` (110–113), `Swapchain::create` (314–322).

A surface stores a raw pointer into discovery's adapter array; swapchain creation
later dereferences it. The shared contract lets native surface ownership survive
discovery destruction. Creating device/surface, destroying discovery and then
creating the swapchain can therefore dereference freed host memory.

Retain the required native factory/adapter ownership independently at cold surface
creation, with explicit identity checks. Do not silently impose a new discovery
lifetime requirement on existing callers.

Acceptance: create device/surface, destroy discovery, create/use/destroy the
swapchain and surface. Test cross-discovery/device rejection where the chosen
profile requires it, and ensure failures do not alter outputs or native state.

## R5 — P1: presentation failure becomes success; preflight is partial

Location: [commands.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/commands.rs),
`Queue::present`, lines 1963–2007; shared C adapter
`queue_present` in [presentation_api.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/presentation_api.rs).

A failed native present returns `Ok(false)`, which the C adapter translates to
`OK`. With no results array, the failure is lost entirely. Validation happens
inside the execution loop, so an earlier chain can already be presented when a
later invalid entry returns a nominal preflight error.

Preflight the complete request before native side effects. Preserve aggregate
and per-chain errors and distinguish rejection from partially queued work. The
batch also needs honest native queue/wait/fence semantics: do not assume a loop
over DXGI presents reproduces Vulkan's single-batch retirement contract.

Acceptance: inject failure with/without a results array; invalid second chain
must enqueue nothing; multi-chain waits/fences; partial native failure; loss and
recovery state; wrong queue/device and acquired-index checks. Test the C return
status as well as the Rust result.

## R6 — P1: device removal is reported as completed work

Location: [presentation.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/presentation.rs),
`Fence::poll` (660–664), `Binary::retired` (572–575); `Timeline::poll` in
[mod.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/mod.rs) (931–936).

`GetCompletedValue()` returns `UINT64_MAX` on device removal. Direct comparison
against the target reports successful retirement, while timeline polling returns
it as an ordinary value. See Microsoft's
[documented sentinel](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12fence-getcompletedvalue).

Recognize the sentinel, propagate terminal loss and apply the foundation's
conservative recovery contract. Audit waits and every completion-dependent reuse
path, not just the two public poll functions. Do not call this successful work.

Acceptance: injected sentinel across timeline, binary and fence paths, including
wait/reuse checks and subsequent device calls. Distinguish synthetic coverage from
any actual device-removal run; do not intentionally destabilize the host merely
to satisfy this handoff.

## R7 — P1: tight-row copy expansion generates invalid footprints

Location: [images.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/images.rs),
buffer/image copy lowering, lines 944–1018.

Any non-256-aligned caller pitch is expanded into per-row copies with native
`RowPitch = 256`, even if each row contains more than 256 bytes. For example,
100 RGBA8 texels need 400 bytes, so the declared native pitch is too small.
Per-row offsets can also violate placement alignment when unrestricted-copy
support is unavailable. See native
[row-pitch requirements](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ns-d3d12-d3d12_subresource_footprint)
and [placement alignment and capability exception](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ns-d3d12-d3d12_placed_subresource_footprint).

Query and expose the native copy-layout constraints and unrestricted-copy
capability. Use the direct native strategy when supported; otherwise reject
incompatible layouts or require explicitly selected preparation outside the
fundamental copy command. Do not hide per-row command expansion or staging.

Acceptance: both copy directions, nonzero offsets, rows below and above 256
bytes, layers/mips, compressed block edges where advertised, guards, and
capability-on/off lowering tests. Native validation must reject no emitted copy.

## R8 — Presentation profile and lifecycle redesign

These are additional merge requirements, not permission to keep the current
presentation behavior once R4/R5 are fixed:

- Add an explicit Win32 surface/profile. Passing HWND through
  `surface_wayland_create` with the Wayland kind misrepresents the public contract.
- `Swapchain::acquire` ignores timeout and returns the current back-buffer index
  with queue signals. Establish the actual DXGI availability/backpressure and
  ordering guarantee; expose it directly instead of pretending it is a Vulkan
  acquire. Define release and repeated acquire behavior accordingly.
- The `old_swapchain` path clears old image wrappers before `ResizeBuffers`.
  That invalidates borrowed image pointers before old-chain destruction and can
  leave the old object changed on failure. Use an explicit native resize/lifetime
  contract, including views and failure state, rather than claiming the existing
  replacement-chain semantics.
- Resolve render completion, presentation payload retirement and image reuse
  separately. A generic queue fence is not automatically proof of every DXGI
  presentation obligation.

Acceptance: typed C/Rust Win32 entry points, timeout/backpressure behavior,
resize success/failure with borrowed views, repeated acquisition, multi-frame
reuse and close with pending work. Update the header and consumers together;
preserve Vulkan capabilities and valid lifetimes through the selected common
contract, without freezing the old presentation signatures.

## R9 — Validation and evidence gate

The current `dump_validation` helper in
[tests.rs](https://github.com/elfstead/gpu/blob/34930b0/crates/ogpu/src/foundation/dx12/tests.rs) prints only the first ten
stored messages and does not fail on validation errors. Debug-layer enabling is
best-effort, including unconditional runtime enabling. Green pixel output is not
proof of valid native calls.

Make validation an explicit test configuration: report whether the debug layer
and GPU-based validation are active, inspect all relevant error/corruption messages
and fail the run on them. Keep normal runtime diagnostics opt-in and timing runs
separate. Record skips/unavailable validation honestly. After the fixes, rerun the
indirect-draw soak and investigate any remaining intermittency rather than
accepting a usually-green test.

Consolidate the argument/presentation profiles and D1/D2 receipts: several earlier
entries still say operations are unimplemented or describe superseded behavior.
Keep history labeled as history, with one authoritative current profile, capability
matrix, API-change ledger and native evidence receipt.

## Return handoff

Continue on `dx12-backend`, committing coherent changes. Suggested grouping:
R1/R5/R6 correctness; R2/R3 selected binding/storage contract; R4/R8 presentation
lifetime and API; R7 copy requirements; R9 evidence consolidation. Dependencies
may reorder these groups. No feature-expansion or timing campaign is needed before fixing
structurally incorrect mappings.

For every R-number, return a disposition, fixing commit, regression test names,
exact reproduction commands and remaining limitations. Include tested Windows,
GPU/driver, SDK/Agility, Rust and shader-toolchain versions. Run host/ABI/C/Rust
consumer checks, native validation, offscreen fixtures, the M4 scene and lifecycle
tests appropriate to the revised profiles. The Linux reviewer will rerun shared
regressions; Windows-only builds do not establish Vulkan preservation.

Do not merge, claim full backend parity or mark a finding closed merely because
the scene renders. Return the branch for another review once these corrections
and their declared evidence are complete.
