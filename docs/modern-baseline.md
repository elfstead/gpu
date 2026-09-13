# Modern Vulkan baseline

Decision, 2026-09-12: prefer one direct implementation of a coherent modern GPU
model, not compatibility paths for older drivers. The Vulkan 1.2 feasibility
backend is a migration source, not a supported fallback to preserve.

Decision/audit: `3fefc48`. Backend migration: `39b8c16`; timeline completion:
`05a1857`, locally verified on llvmpipe. The subsequent
[ABI-3 hardware run](hardware-validation.md) verifies modern compute and GGML on
RX 5700 XT / RADV. At `a5a609d`, optional unified layouts also enable verified
graphics/heaps and preservation on that card. Remote execution CI remains pending.

The subsequent [independent-heap checkpoint](descriptor-heaps.md) implements native
image/sampler bindings and preserved image use on this same baseline, without a
legacy table or descriptor-set path. The later [memory checkpoint](memory-transfers.md)
uses this same baseline and brings the public interface to ABI 4.

## Selected requirements

The execution target is Vulkan 1.4, buffer device addresses, synchronization2,
timeline semaphores, maintenance5, `VK_EXT_descriptor_heap`,
`VK_KHR_shader_untyped_pointers` and `VK_KHR_device_address_commands`.
Graphics additionally requires dynamic rendering and a shared graphics/compute queue.
At ABI 8 this is an explicit creation choice; images and heaps also work on ordinary
compute devices. [Enabled capabilities](execution-capabilities.md) are reported
separately from the physical support snapshot.
`VK_KHR_unified_image_layouts` is optional, enabled when supported for its
layout-efficiency guarantee. GENERAL-only recording is identical without it.
Mesh shading, presentation and accelerated
matrix instructions are additional facilities, not prerequisites for compute.
Unsupported execution devices must be rejected with a named missing requirement;
discovery remains available to diagnose them. Feature bits and limits matter, not
just API version or advertised extension names.

Why these requirements:

- [Descriptor heaps](https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_descriptor_heap.html)
  remove pipeline/binding layout objects and provide a single push-data interface.
- [Address commands](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_device_address_commands.html)
  extend the pointer model to transfers and indirect command inputs.
- [Untyped pointers](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_shader_untyped_pointers.html)
  support that descriptor model and reduce IR type-reconstruction constraints.
- [Unified image layouts](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_unified_image_layouts.html)
  optionally guarantee efficient ordinary use in GENERAL; our current operations
  are legal in GENERAL without the extension. Initialization, presentation and real
  access dependencies do not disappear. Global image barriers are not guaranteed
  to be the fastest choice on every implementation.

This follows the important structural choices of Aaltonen's
[prototype at 8e414bd](https://github.com/sebbbi/NoGraphicsAPI/blob/8e414bd0a8010b9f721d06d470860e27aa69c071/docs/no-graphics-api-comparison.md),
without making mesh shaders mandatory. It does not settle our source language,
GPU-resident roots, allocator ownership, or texture API.

## Deployment audit

`cargo xtask baseline` builds a standalone C feature/limit query against the
pinned headers. It uses the normal loader or `OGPU_VULKAN_LIBRARY`, does not change
driver configuration, and returns 2 if no device meets the compute feature set.
It reports capabilities only: device creation and execution are separate gates.

In the current sandbox, llvmpipe (LLVM 21.1.8, Mesa 26.2.1), Vulkan 1.4.354,
reports every selected compute/graphics feature and maxPushDataSize=256.
The initial sandbox audit had no `/dev/dri`; it did not establish host GPU absence.
A later approved host audit found the RX 5700 XT and verified the modern compute
backend there; see [hardware validation](hardware-validation.md). The driver lacks
unified image layouts. Removing our unnecessary hard requirement subsequently
allowed the unchanged image execution path to pass graphics/heap/preservation tests
on that card too. The initial compute-only result is historical, not a current limit.
Installed tools include glslang 16.4.0 and SPIRV-Tools 1.4.357.0; no shader Slang
compiler was found. Existing descriptor-free SPIR-V/root blocks are the first
migration input. Heap-indexed texture shaders still need a compiler/tooling test;
extension availability alone does not establish that end-to-end path. The subsequent
[heap-image experiment](heap-images.md) closes that local compiler/execution gate
with pinned Slang 2026.14.1 and native SPIR-V heap indexing.

## Migration and stopping condition

| Existing mechanism | Replacement | Expected removal |
|---|---|---|
| Pipeline layouts and push-constant ranges | Descriptor-heap pipelines and push data | Per-executable layout allocation/destruction and compatibility state |
| Legacy barriers/submission/timestamps | Synchronization2 commands | Old synchronization structures and command loading |
| Render passes and framebuffers | Dynamic rendering and GENERAL images | Persistent pass/framebuffer objects and ordinary layout transitions |
| Buffer handles in indirect draws and image readback | Address-based commands | Command-side buffer/offset translation (owning allocations still exist) |
| Per-submission fences | One device-owned timeline semaphore with monotonically increasing values | Per-submission fence allocation/destruction and wait commands |

All five replacements are implemented with no legacy execution branches.
Vulkan buffer allocations still exist underneath owning allocations, and their
usage flags must satisfy address-command validity rules. The old STORAGE_BUFFER
flag was unnecessary for physical-pointer shaders and was removed. INDIRECT_BUFFER
and TRANSFER_DST remain required for the commands we use. Address-based commands
do not erase every Vulkan backing-allocation requirement.

The fixed graphics operation still clears on every draw: it explicitly discards
with UNDEFINED → GENERAL before dynamic rendering, and orders color writes before
readback. These preserve the existing public clear/draw/copy contract; they are not
a newly generalized image API. Completion handles retain their command resources and
wait or poll a device-owned timeline value. The [retirement experiment](retirement.md)
keeps range reuse in the application and supplies optional allocation retention.
A centralized runtime retirement queue remains unselected.

Migrate one path and delete its predecessor; do not add version-switching execution
branches. Keep current public lifetime/numerical contracts unless explicitly revised.
Regression gates are binding reproduction, C/Rust ABI, ordinary/mock tests,
validated compute/graphics/image-loop and the pinned GGML consumer. Check failure
cleanup against the new objects rather than preserving tests for removed objects.
CI must use a driver that meets the selected baseline, or explicitly report the
execution lane unavailable; skipping every device is not a successful GPU test.

The migration is complete only when the chosen path executes those workloads,
obsolete backend mechanisms are removed, and deployment requirements agree with
the implementation. The local runtime gate is met; deployment coverage is narrower
than before, rather than silently supported by a second path. Lifetime/range API
experiments follow this work, rather than motivating retention of the old backend.

## Verification — 2026-09-12

Loader/validation 1.4.357.0, Mesa 26.2.1 llvmpipe, Linux x86-64:

- 21 ordinary tests, seven Vulkan-backed tests, formatting and Clippy pass.
- Pinned generated bindings reproduce; 604 C/Rust ABI layout/constant values match.
- Discovery mocks still report older devices and now check named execution rejection.
  Unit coverage rejects each missing queried core requirement.
- Compute, batch, graphics, six-size repeated image loop, reduction, and both matrix
  kernels pass with validation and synchronization validation enabled. Failure tests
  cover the new object lifecycle; deleted pass/layout/framebuffer objects have no
  remaining allocation/cleanup paths. This record describes the original migration;
  the September 13 timeline and retirement verification is recorded separately below.
- All 15 existing shader binaries reproduce byte-for-byte with glslang 16.4.0 and
  pass SPIRV-Tools 1.4.357.0. Their Vulkan 1.2 SPIR-V target remains sufficient for
  these descriptor-free programs; it does not lower the backend's device baseline.
- GGML lifecycle and all six direct/scheduled MNIST cases pass through the public
  ABI: 10,000 images per case, 9,801 correct, identical CPU top-1 predictions,
  maximum logit error 0.0000343322754. No shader or adapter changes were needed.

No performance improvement, new physical-GPU result, heap-indexed texture shader,
or remote CI execution is claimed. The existing Ubuntu 24.04 package-based execution
lane is not an adequate declared environment for this profile. Ordinary hosted CI
continues build/mock/ABI/SPIR-V checks; GPU and GGML execution move together to
`.github/workflows/gpu.yml`, manually invoked on a provisioned runner labeled
`ogpu-modern-vulkan`. It requires the baseline audit and fails if no device qualifies.
The runner is **not provisioned by this change**. Manual trusted invocation avoids
running arbitrary pull-request code on self-hosted hardware. Until that runner is
available, automatic hosted CI is not an execution-regression gate.

## Timeline follow-up — 2026-09-13

The full Cargo workflow is restored after recovering its dependency cache. The
review fixed missing pending-value limit enforcement and tested semaphore creation
failure cleanup, rejected/unknown submissions followed by successful submissions,
out-of-order waits, and exhaustion without wrapping. `e82cfff` records that work.
`3c23fef` corrects the earlier loader-version clamp: the application's Vulkan 1.4
ceiling must remain independent of a Vulkan 1.1+ instance implementation, as
specified by [VkApplicationInfo](https://docs.vulkan.org/refpages/latest/refpages/source/VkApplicationInfo.html).
The old mock assertion caught the capability-reporting regression and now passes.
GGML's six acceptance cases and lifecycle checks pass after timeline hardening.
See [retirement](retirement.md) for the subsequent API experiment and final checks.
