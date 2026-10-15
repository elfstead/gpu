# P1: presentation and an interactive Linux consumer

Scheduled 2026-10-15, before M6. **Native Rust operation extraction implemented;
presentation is not implemented or natively validated.**
M4's offscreen acceptance remains complete. P1 supplies a missing application
boundary; it does not turn OGPU into a windowing library or game engine.

## Current implementation checkpoint

`ogpu::foundation::rust` now has equivalents for every current foundation C
operation: setup/resources, executable/cache preparation, descriptors/images,
recording/raster, queries, split dependencies and submission. Both languages share
backend validation/execution and recording-error state; Rust bypasses C entry
points. Handles are transparent non-owning tokens with explicit unsafe destruction,
no additional retention or waits. Some descriptions retain versioned records and
raw nested pointers. Operation coverage is not complete ergonomic typing, a safe
wrapper, generated Rust shader interfaces or native evidence for every Rust call.

The [standalone Cargo consumer](../examples/foundation_rust/README.md) is separate
from the workspace, uses public Rust only, and checks A/B/A fill/copy output,
malformed creation, poisoned recording, timeout and insufficient-scratch retry.
It now also checks direct/indirect compute with specialization, independent root/
inline updates, cache export/import/merge, image binding/views, descriptor encoding
and A/B/A image clear/readback. Broader raster/query/secondary/split execution is
covered by existing shared-backend/C fixtures, not this Rust consumer.
It is not yet an installed Rust SDK package or independent third-party adoption.

Expanded-tranche local validation (2026-10-15): 48 host unit tests plus two host
integration tests for compile-checked C/Rust surface correspondence and absence of
C adapter calls; 17 GPU unit tests
and one native Rust integration test on Radeon RX 5700 XT/RADV and llvmpipe;
standalone Cargo consumer on both drivers; full C foundation fixture on Radeon;
strict Clippy for all foundation targets. Synchronization validation enabled.
The unwind test checks Rust propagation, C containment and first-error retention.
No runtime performance measurement, new hardware or presentation support is claimed.

## Contract and implementation batch

The [native presentation mapping](presentation-contract.md) selects Wayland with
external GLFW window/input handling and the KHR swapchain-maintenance profile.
Local Radeon extension/feature discovery is confirmed; surface/window execution
is not. Generated WSI declarations reproduce and pass C/Rust ABI comparison;
opt-in setup, exact public records and native retirement paths remain to implement.

### Opening step: first-class native Rust, then presentation

Extract a public native Rust interface implementing the same foundation contract
below the C ABI. C entry points adapt arguments/results and contain panics; Rust
callers do not route through them. Keep backend representations private and share
operation validation/execution rather than maintaining two implementations.

Use typed handles, slices and Results where appropriate, with explicit unsafe
GPU-lifetime/address/synchronization obligations. No mandatory reference counting,
resource retention, automatic waits, scheduler or additional allocation is added.
This is not a fully safe convenience wrapper or an ABI stabilization project.

Move the existing surface in coherent batches, preserving C behavior and tests.
Add a standalone native Rust consumer covering explicit resource lifetimes,
submission and errors; verify that its calls bypass C adapters. Then expose new
presentation operations through both interfaces and write the interactive consumer
in Rust. Subsequent hardware milestones extend Rust and C together. M6 builds on
this shared contract rather than treating the C boundary as the runtime core.

### Presentation implementation

1. Specify an optional, versioned presentation interface and implement the narrow
   extension negotiation needed to discover it. Headless device creation must
   remain independent of surfaces. Query native surface/queue compatibility;
   never assume queue zero can present. Select one Linux window-system path after
   inspecting the available display and windowing dependencies; record that choice
   before implementation. Do not claim all Linux window systems from one path.
2. Accept typed native window/display handles supplied by an external window/input
   library or application. Define owned versus borrowed lifetimes. Expose surface
   capabilities, image format/color space, extent, image count and present-mode
   selection explicitly; do not insert a hidden frame scheduler or pacing policy.
3. Define acquisition outcomes, timeout/nonblocking behavior and explicit GPU
   dependencies. Acquired images use ordinary OGPU rendering operations with
   documented ownership/state boundaries. Keep render completion, image reuse and
   presentation completion distinct. Expose native limitations honestly; do not
   promise timeline presentation semantics that require hidden host waits.
4. Implement present, resize/recreation, out-of-date/suboptimal outcomes, suspended
   zero-extent windows, surface loss and teardown. Define how acquired but unused
   images are retired and when old swapchain resources may be destroyed. No
   per-frame device-idle wait, mandatory CPU pixel readback or implicit copy path.
5. Specify the accompanying native interop boundary: device/queue identity,
   owned/borrowed objects, image states, imported synchronization and handoff rules.
   Existing presenters must eventually be able to share resources without CPU
   readback. Full external-memory/import/export implementation belongs to M10;
   pull a subset forward only if the chosen native path actually needs it.
6. Adapt the textured scene into a small interactive Rust consumer with camera/input,
   caller-managed multiple frames in flight and bounded reusable storage. Package
   its source and dependency/build instructions with the experimental SDK.

## Acceptance and remaining proof

- Native Rust and C exercise the shared core; existing C rejection/output semantics
  and GPU regressions remain intact. Verify the standalone Rust consumer, document
  unsafe obligations, and inspect for extra allocations, retention and waits.
- On the selected real Linux display path: render a textured/depth/blended scene,
  interact, resize repeatedly, minimize/restore, and close with work in flight.
  Exercise available present modes and explicitly report unsupported modes.
- Verify acquire/present ownership and synchronization under native validation;
  cover nonblocking/timeout and recreation outcomes with deterministic tests where
  possible. Injected tests are not proof of actual surface/device-loss recovery.
- Inspect the steady-state path for hidden waits, copies, per-frame allocation
  and forced serialization. The application controls frames in flight. Reuse
  existing offscreen image checks; no broad new benchmarking campaign is required
  before implementing the straightforward native mapping.
- Check incompatible surface/queue choices, invalid lifetimes and format requests
  to the extent the trusted C boundary can diagnose them. Document caller-owned
  safety obligations rather than claiming comprehensive pointer/lifetime safety.
- Run the installed interactive consumer independently of the source tree. Record
  exact compositor/window system, driver and modes; distinguish visible interactive
  acceptance from headless tests or a virtual display.

No display is assumed available. Contract, implementation and headless lifecycle
tests can proceed without it; native presentation acceptance stays open. Do not
add a CPU display fallback to manufacture completion. M6 maps the same concepts
to Metal with explicit native differences, not Vulkan-shaped emulation.

## Scope boundaries

Input, audio, assets, scene management and window creation remain consumer/library
responsibilities. HDR/color-management breadth, additional window systems and
advanced presentation timing need separately declared profiles and evidence; the
API must leave room for them without claiming their implementation in P1. Mesh
shaders and ray tracing are not prerequisites for this consumer. P1 completion is
a practical experimental Linux game-rendering path, not production readiness or
a stable ABI promise.
