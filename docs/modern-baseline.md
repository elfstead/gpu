# Modern Vulkan baseline

Decision, 2026-09-12: prefer one direct implementation of a coherent modern GPU
model, not compatibility paths for older drivers. The Vulkan 1.2 feasibility
backend is a migration source, not a supported fallback to preserve.

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
| Per-submission fences | Timeline-backed completion | Deferred: coordinate separately with completion/polling and retirement design |

Migrate one path and delete its predecessor; do not add version-switching execution
branches. Keep current public lifetime/numerical contracts unless explicitly revised.
Regression gates are binding reproduction, C/Rust ABI, ordinary/mock tests,
validated compute/graphics/image-loop and the pinned GGML consumer. Check failure
cleanup against the new objects rather than preserving tests for removed objects.
CI must use a driver that meets the selected baseline, or explicitly report the
execution lane unavailable; skipping every device is not a successful GPU test.

The migration is complete only when the chosen path executes those workloads,
obsolete backend mechanisms are removed, and deployment requirements agree with
the implementation. Lifetime/range API experiments follow this work, rather than
motivating retention of the old backend.
