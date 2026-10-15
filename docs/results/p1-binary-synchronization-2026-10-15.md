# P1 explicit binary submission and fence lifecycle — 2026-10-15

Follow-up to [device enabling and mode discovery](p1-presentation-profile-2026-10-15.md).
**Headless synchronization acceptance, not swapchain or frame-presentation acceptance.**

## Implemented contract

The native Rust interface and optional C table now create/destroy binary semaphores
and create/poll/wait/reset/destroy fences. These native primitives do not require
PRESENTATION enabling, a surface or a display. Devices must outlive every object
and pending use; destruction never waits. Binary pending-state validity and reuse
remain explicit caller obligations, not a mirrored runtime state machine.

One `BinarySubmitDesc` extension adds stage-scoped binary waits/signals to an ordinary
`SubmitDesc`. Single and batched submission share the existing translation path;
caller scratch is sized for combined timeline/binary counts. One batch call lowers
to one `vkQueueSubmit2`, preserving its submission boundaries. There is no hidden
bridge submission, host wait, retention or submission-path allocation. Native
binary values are zero; stages and object identity are preserved. See
[native semaphore fields](https://docs.vulkan.org/refpages/latest/refpages/source/VkSemaphoreSubmitInfo.html).

Only this root accepts the new extension. Short/unknown/nested/flagged records,
count overflow, insufficient scratch, null objects, foreign-device objects and
invalid stage masks are rejected before native submission. Array validity,
scratch disjointness, duplicate-free binary wait/signal arrays and valid pending
signal/wait ordering remain caller obligations. No quadratic duplicate scan or
resource graph is introduced. A rejected batch does not consume one-shot lists.
Native failure retains the existing submission uncertainty rules; see
[native submission](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueueSubmit2.html).

Fences expose initial state and lifecycle only at this checkpoint. Poll maps
signaled/not-ready; timed wait preserves timeout; reset is explicit, non-waiting
and allowed on an idle unsignaled fence. Polls/waits may run concurrently, while
reset/destruction exclude other host accesses and pending signals. See
[fence status](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetFenceStatus.html),
[wait](https://docs.vulkan.org/refpages/latest/refpages/source/vkWaitForFences.html)
and [reset](https://docs.vulkan.org/refpages/latest/refpages/source/vkResetFences.html).
Acquire/present signal association and presentation retirement are not implemented.

## Evidence

Same Radeon RX 5700 XT/RADV 26.2.4 and llvmpipe 26.2.1 environment as the preceding
receipts, with loader/validation 1.4.357 and synchronization validation enabled:

- 54 host unit tests and two host C/Rust-operation/boundary tests pass.
- 18 headless GPU unit tests, the native Rust consumer and the separate GLFW
  Wayland setup probe pass on each driver. No validation errors, VUID diagnostics
  or synchronization hazards were emitted in the captured regression logs.
- The independent Cargo consumer passes mixed binary/timeline A/B/A fill/copy
  phases on both drivers: three phases, one two-batch binary-only handoff, two
  fence lifecycle cases and 384 checked words. Caller scratch is reused; malformed
  stages, insufficient scratch and a bad second batch are rejected before retry.
- The C-table/native-lowering GPU test checks binary/fence creation, initial fence
  status, timeout/reset, output clearing, foreign-device rejection, short-record
  rejection and capacity failure. Interception proves rejected batches issue zero
  native calls; accepted two-batch submission issues exactly one, with unchanged
  semaphore handles, stage scopes and timeline/binary values. The accepted call
  forwards to Vulkan and its completion is explicitly awaited.
- Injected native creation failure does not adopt unspecified output handles.
  Injected submit OOM permits retry without poisoning the device. These are injected
  failure checks, not real OOM/device-loss recovery evidence.
- Strict all-target foundation and standalone-consumer Clippy pass; C/Rust ABI
  comparison passes 1,084 layout values; the full public-C foundation fixture passes
  on Radeon, retaining explicit unsupported-profile reports.

Commands use `cargo test -p ogpu --no-default-features --offline`, with host tests
separate from `--lib --test native_rust --test presentation -- --ignored
--test-threads=1`; independent consumer via
`cargo run --manifest-path examples/foundation_rust/Cargo.toml --locked --offline`;
`cargo xtask abi` and `cargo xtask foundation` cover ABI and C regression checks.
The optional Wayland probe additionally needs its documented trusted GLFW library
and live session; binary/fence tests do not.

No new timing measurement, second physical vendor, Metal implementation, native
fence GPU-signal acceptance, swapchain borrowing, acquire/present, resize/minimize
or interactive acceptance is claimed. Next is the complete borrowed-image and
acquire/present lifecycle, using these dependencies without implicit scheduling.
