# P1 native presentation mapping

Selected 2026-10-15. **Implementation design, not an implemented public contract.**
The initial native Rust operation extraction is complete; P1 presentation remains
in progress. This records the next coherent implementation batch and assumptions.

Prerequisite checkpoint: pinned Vulkan 1.4.357 Wayland/swapchain/fence declarations
are generated and reproduce exactly; C/Rust ABI checks pass for 1,009 layout
values, including the added WSI records. Foundation host tests still pass (48 unit
plus two surface-coverage integration tests). This does not enable WSI extensions
or implement any public presentation operation. No window has been opened yet.

## First platform and native baseline

Use **Wayland** for the first Linux path. The current environment reports a
Wayland session with a live `wayland-1` socket (and XWayland); GLFW 3.4 libraries
are installed. The consumer will use a Wayland-enabled GLFW build with no OpenGL
context, obtaining borrowed `wl_display` and `wl_surface` handles via
[GLFW native access](https://www.glfw.org/docs/latest/group__native.html).
OGPU does not depend on GLFW or own its event loop. No X11 fallback is selected;
unsupported/missing Wayland is explicit. Library availability and socket existence
are not a successful window/surface or interactive acceptance test.

Require `VK_KHR_surface`, `VK_KHR_wayland_surface`,
`VK_KHR_get_surface_capabilities2`, `VK_KHR_surface_maintenance1`,
`VK_KHR_swapchain` and enabled `swapchainMaintenance1` for this Vulkan profile.
Use the KHR maintenance path, not an old-swapchain retirement workaround or
automatic EXT compatibility branch. Headless discovery/device use must keep its
existing baseline and must not require any WSI extensions.

A read-only local Vulkan query on 2026-10-15 reports all of these extension names
and `swapchainMaintenance1 = true` on Radeon RX 5700 XT/RADV. The query created no
window or surface. No surface format, present mode, queue compatibility, successful
swapchain, compositor behavior or llvmpipe WSI acceptance is inferred from that.

The maintenance requirement supplies explicit resource-retirement fences and
release of unused acquired images; these are useful native operations, not
additional frame scheduling. See
[KHR swapchain maintenance](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_swapchain_maintenance1.html).

## Operation and ownership model

| Boundary | Proposed contract / native mapping |
|---|---|
| Optional setup | Negotiate a typed/versioned presentation function table, including an opt-in discovery configuration for native WSI instance requirements. Surfaces are separate from device construction; enabling presentation on a device must not choose a window or queue. Plain headless setup stays unchanged. |
| Surface | Borrow typed platform handles; create/destroy the Vulkan surface while preserving their required lifetime. Query physical-adapter/surface queue support before selecting queues. Surface identity must match discovery/instance and device. |
| Capabilities | Query exact formats/color spaces, extent bounds, usage, image count, transform/alpha and supported present modes. Caller selects; no silent format/mode substitution. Start with SDR consumer output, not an HDR claim. |
| Swapchain | Caller supplies configuration, sharing/queue policy and any old-chain replacement reference. Expose swapchain images as borrowed ordinary OGPU images; neither image destruction nor memory rebinding is allowed through this borrow. Explicit swapchain destruction does not wait. |
| Acquisition | Caller chooses a reusable acquisition synchronization object and timeout. Return image identity plus success/suboptimal/timeout/not-ready/out-of-date/surface-lost outcomes without fabricating a usable image on failure. Do not promise an unlimited wait when the native surface cannot guarantee forward progress. |
| Render submission | Expose native acquire waits and render-finished signals through an explicit submission extension, alongside existing timeline waits/signals. Preserve stage scopes and multiple submissions/queues; do not hide an extra submission converting binary semaphores into timelines. |
| Presentation | Explicit queue + acquired image + ready dependency; independent present-retirement token. Present success means queued, not visibly displayed. A present-retirement fence permits defined resource reuse/destruction, not a scanout-complete or image-reacquisition guarantee. |
| Retirement/reuse | Caller polls/waits retirement and decides slot reuse. Render completion, acquire synchronization reuse, present semaphore retirement and image reacquisition remain distinct. No mandatory N-frames scheduler, resource retention walk or per-frame allocation. |
| Unused acquisition | Explicit release after all image use is retired. Releasing the image does not by itself consume/reset a signaled acquisition semaphore; define separate synchronization retirement/reset/replacement rules. |
| Replacement/teardown | Explicit out-of-date, minimized/zero-extent and surface-loss handling. Retain old-chain resources until their own work/presentation obligations are retired. Native failure after naming an old swapchain can retire it even without a replacement; do not promise unconditional rollback. |

The exact C/Rust records, submission-extension representation and borrowing checks
are the first implementation work, not a frozen ABI in this document. Expose
binary synchronization as an explicit optional native mechanism, not by pretending
that a timeline value is acceptable everywhere. Keep caller choice of synchronization
objects separate from swapchain-image indexing or an implicit frame ring.

The distinction between queue completion and present semaphore reuse is required by
the [Vulkan semaphore reuse guidance](https://docs.vulkan.org/guide/latest/swapchain_semaphore_reuse.html).
The precise retirement semantics come from
[present fences](https://docs.vulkan.org/refpages/latest/refpages/source/VkSwapchainPresentFenceInfoKHR.html),
which may signal before presentation completes. Releasing an acquisition follows
[vkReleaseSwapchainImagesKHR](https://docs.vulkan.org/refpages/latest/refpages/source/vkReleaseSwapchainImagesKHR.html);
neither API is a substitute for retiring unrelated GPU resource access.

## Implementation batch and verification

1. Add pinned generated native WSI/fence declarations (no new runtime dependency).
   Implement opt-in instance/device negotiation and the public extension table;
   add unsupported-profile and output-atomicity tests. Keep plain headless creation
   and existing C/Rust entry points working.
2. Implement surfaces, capability queries, explicit binary submit dependencies and
   their caller-owned scratch sizing, borrowed swapchain images and acquire/present
   retirement. Reconcile image destruction/binding/query invariants before allowing
   ordinary rendering to use a swapchain image. No presentation-state transition
   may be elided merely because unified image layouts are enabled.
3. Build the external-GLFW Rust consumer using the existing textured rendering
   pipeline. Exercise caller-managed multiple frames, camera input, resize,
   minimize/restore, unused acquisitions and close with pending work. Add a bounded
   automated mode; visible interactive acceptance is a separate recorded result.
4. Validate synchronization and lifetime behavior on the selected display path;
   use headless/injected checks for failure transitions without claiming actual
   device/surface-loss recovery. Inspect steady-state lowering for hidden waits,
   submissions, copies or allocations. Keep offscreen and Rust/C regressions.

## Portability and deferred scope

Metal M6 must map drawable availability/presentation and retirement honestly, not
emulate Vulkan swapchains or promise Vulkan binary semaphore behavior as universal.
Capability-specific alternatives are allowed; common operation names do not prove
identical native completion guarantees. A rendering queue need not be the present
queue; exact surface-supported strategies remain visible.

Native presenter interoperability remains required in M10: typed device/resource
identity, owned/borrowed handles, image states and native synchronization handoffs.
P1 surface-handle borrowing alone does not implement external GPU-memory sharing.
Additional window systems, HDR policy, present timing/scanout feedback and arbitrary
native encoding remain separate profiles, not implicit support from this batch.
