# Modern Vulkan baseline

Decision, 2026-09-12: prefer one direct implementation of a coherent modern GPU
model, not compatibility paths for older drivers. The Vulkan 1.2 feasibility
backend is a migration source, not a supported fallback to preserve.

Decision/audit: `3fefc48`. Backend migration: `39b8c16`; timeline completion:
`05a1857`, locally verified on llvmpipe. Physical-GPU and remote execution-CI
verification remain pending.

## Selected requirements

The execution target is Vulkan 1.4, buffer device addresses, synchronization2,
timeline semaphores, maintenance5, `VK_EXT_descriptor_heap`,
`VK_KHR_shader_untyped_pointers` and `VK_KHR_device_address_commands`.
Graphics additionally requires dynamic rendering, `VK_KHR_unified_image_layouts`
and a shared graphics/compute queue. Mesh shading, presentation and accelerated
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
  permit efficient ordinary use in GENERAL. Initialization, presentation and real
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
There is no `/dev/dri` here, so no new physical-GPU result is claimed. Earlier
RX 5700 XT results validate the old backend, not this selected baseline.
Installed tools include glslang 16.4.0 and SPIRV-Tools 1.4.357.0; no shader Slang
compiler was found. Existing descriptor-free SPIR-V/root blocks are the first
migration input. Heap-indexed texture shaders still need a compiler/tooling test;
extension availability alone does not establish that end-to-end path.

## Migration and stopping condition

| Existing mechanism | Replacement | Expected removal |
|---|---|---|
| Pipeline layouts and push-constant ranges | Descriptor-heap pipelines and push data | Per-executable layout allocation/destruction and compatibility state |
| Legacy barriers/submission/timestamps | Synchronization2 commands | Old synchronization structures and command loading |
| Render passes and framebuffers | Dynamic rendering and GENERAL images | Persistent pass/framebuffer objects and ordinary layout transitions |
| Buffer handles in indirect draws and image readback | Address-based commands | Command-side buffer/offset translation (owning allocations still exist) |
| Per-submission fences | One device-owned timeline semaphore with monotonically increasing values | Centralized polling/retirement queue and public polling semantics |

The first four replacements are implemented with no legacy execution branches.
Vulkan buffer allocations still exist underneath owning allocations, and their
usage flags must satisfy address-command validity rules. The old STORAGE_BUFFER
flag was unnecessary for physical-pointer shaders and was removed. INDIRECT_BUFFER
and TRANSFER_DST remain required for the commands we use. Address-based commands
do not erase every Vulkan backing-allocation requirement.

The fixed graphics operation still clears on every draw: it explicitly discards
with UNDEFINED → GENERAL before dynamic rendering, and orders color writes before
readback. These preserve the existing public clear/draw/copy contract; they are not
a newly generalized image API. Completion handles retain their command resources and
wait on a device-owned timeline value. A centralized retirement queue and public
polling semantics remain deliberately deferred.

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
  cover timeline-signal/wait and the new object lifecycle; deleted pass/layout/framebuffer
  objects have no remaining allocation/cleanup paths.
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
