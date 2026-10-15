# P1 swapchain ownership and live lifecycle — 2026-10-15

Follow-up to [binary synchronization](p1-binary-synchronization-2026-10-15.md).
**Bounded real Wayland presentation, not complete P1 interactive acceptance.**

## Contract and implementation

Rust and the optional C table expose exact swapchain creation/destruction, borrowed
image enumeration, acquire, unused-image release, present scratch sizing and
multi-swapchain queue presentation. Both interfaces use the same backend.
Creation is cold: explicit configuration is checked against surface capabilities,
compatible modes and device/image support. No implicit format/mode/extent choice,
fallback, queue selection or frame count is introduced. The initial profile supports
modes 0–3, exclusive/concurrent sharing and optional mode lists, not scaling,
runtime switching, mutable/protected images or shared-present/latest-ready profiles.

Image addresses remain stable within their chain. They use ordinary commands/views,
but are already bound and cannot acquire separate backing; dedicated allocation and
rebinding reject them. `image_destroy` is a no-op on these borrows. Destroying the
chain invalidates every borrow; callers first retire image uses and destroy views.
Borrowed metadata never calls `vkDestroyImage`. STATE_PRESENT is an explicit
PRESENT_SRC transition; unified layouts do not erase it.

Acquisition returns an index only for OK/SUBOPTIMAL. Timeout, not-ready, out-of-date,
surface-lost and other errors preserve output. Caller chooses a same-device binary,
fence or both and proves native pending-state/forward-progress rules. Release of
unique acquired indices does not consume an acquisition binary or change layout.
Caller retires acquisition/device access and separately consumes or retires/destroys
that binary. See [native release](https://docs.vulkan.org/refpages/latest/refpages/source/vkReleaseSwapchainImagesKHR.html).

Presentation translates caller arrays into caller-owned scratch and invokes one
native batch, preserving per-image statuses and optional retirement fences. No
hot allocation, wait, timeline conversion, image transition or submission is hidden.
Duplicate-free swapchains/waits/fences and pending ownership remain caller obligations;
there is no quadratic validation walk or retained-resource graph.

Three native distinctions remain explicit:

- Replacement can retire the old chain even if native creation fails; preflight
  rejection does not call native creation. See
  [creation semantics](https://docs.vulkan.org/refpages/latest/refpages/source/VkSwapchainCreateInfoKHR.html).
- Out-of-date/surface-lost presentation can still enqueue semaphore waits; OOM has
  an unchanged-state guarantee, not arbitrary errors. Present may itself block.
  See [native presentation](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueuePresentKHR.html).
- Present fences retire native presentation payload references and semaphore use,
  not scanout or reacquisition. Rendering/acquisition retirement remains separate.
  See [present fences](https://docs.vulkan.org/refpages/latest/refpages/source/VkSwapchainPresentFenceInfoKHR.html).

## Evidence

Same Radeon RX 5700 XT/RADV 26.2.4 and llvmpipe 26.2.1 setup as prior receipts,
Vulkan loader/Khronos synchronization validation 1.4.357, external GLFW 3.4 Wayland:

- Live Rust probe: eight two-swapchain batched clear/presents, replacement of both
  chains, two unused-acquisition releases. Binary acquire waits, a shared binary
  render-finished wait, independent timeline completion and per-chain present
  fences are explicitly retired/reused. Both drivers pass without validation errors.
- llvmpipe enables unified image layouts (`unified=4`); Radeon reports `unified=0`.
  No Radeon unified-layout execution claim. Both execute PRESENT transitions.
- Stable borrow enumeration, short-capacity preservation, no-op image destruction,
  rebinding/dedicated-allocation rejection, ordinary view creation/destruction and
  present scratch rejection/retry are exercised on live swapchains.
- Headless injected WSI test checks C callbacks, failure-output preservation even
  when native acquisition scribbles its output, OK/SUBOPTIMAL index adoption,
  invalid/foreign objects, invalid release indices, per-image present results,
  exactly one native call for a two-chain batch, zero calls on rejected input and
  terminal device-loss propagation. Injected WSI calls do not submit fake objects
  to Vulkan. These are not real surface/device-loss or OOM recovery observations.
- 55 ordinary unit tests plus two host integration checks pass. Both drivers pass
  19 headless GPU unit tests, the native Rust consumer, separate hidden-window
  setup and live presentation tests. Full public-C foundation fixture passes on
  Radeon with its existing explicit unsupported-profile reports.
- Strict foundation/standalone Clippy and formatting pass; C/Rust ABI comparison
  passes 1,122 layout values. GPU regression logs contain no VUID, validation-error
  or synchronization-hazard diagnostics.

The initial two-window FIFO probe stalled in native `vkQueuePresentKHR` after
successful earlier frames; bounded processes were terminated. Switching only the
consumer's explicit mode to MAILBOX completes on both drivers. Occluded-window
compositor throttling is a plausible explanation, not a proven diagnosis. OGPU
does not substitute MAILBOX or add hidden scheduling. The integration test now
uses GNU `timeout` (30 seconds plus two-second kill grace) for the process group.

Reproduce with the [consumer instructions](../../examples/foundation_rust/README.md)
and `cargo test -p ogpu --no-default-features --offline --lib --test native_rust
--test presentation -- --ignored --test-threads=1`, selecting the intended ICD and
trusted GLFW library. Host checks run separately without `--ignored`.

## Remaining acceptance

This is serial clear/present lifecycle evidence, not pixel readback, visual/scanout
verification, sustained performance, textured interactive graphics, multiple frames
in flight, resize/minimize/restore or real loss recovery. Concurrent sharing and
cross-family ownership are specified but not executed by this same-family probe.
The live consumer is Rust; C uses injected WSI contract checks and existing shared
core tests, not a separate live C window application. Native replacement failure
after retiring an old chain is documented but not induced on the compositor.

Next: adapt the textured scene into the interactive Rust consumer with explicit
frame slots, input/event-loop policy and resize/minimize/close lifecycle handling.
M6 remains after P1; no Metal acceptance is inferred.
