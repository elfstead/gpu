# P1 presentation-device enabling and per-mode discovery — 2026-10-15

Follow-up to [surface setup at `cfb2e1f`](p1-surface-setup-2026-10-15.md), using
the same local drivers, loader/validation and external GLFW Wayland path.
**No swapchain, frame rendering/presentation or interactive acceptance yet.**

Presentation device enabling uses the existing device-creation feature mask, not
a second device constructor. `PRESENTATION` is available only when both the
physical device and opt-in discovery instance support the selected profile.
Requesting it enables KHR swapchain and swapchainMaintenance1, with no surface,
implicit presentation queue, frame ring or wait. Leaving the bit unset keeps the
device headless. Physical availability remains independently queryable without
a window, even through headless discovery; availability is not enabling.

`surface_mode_query` adds a native mode selector to cold capability queries. It
reports mode-specific image-count bounds, scaling/gravity and compatible modes;
unknown modes and wrong query kinds are rejected without changing output. Base
unqualified queries remain separate. The public contract requires explicit
matching mode-list creation to use per-mode image counts; querying a lower count
does not by itself authorize an unqualified swapchain creation strategy.

Local observations on both Radeon and llvmpipe:

| Query | Minimum images | Compatible modes |
|---|---|---|
| Base / unqualified | 4 | Not queried without a mode |
| MAILBOX | 4 | MAILBOX, FIFO, IMMEDIATE |
| FIFO | 3 | FIFO, MAILBOX, IMMEDIATE |
| IMMEDIATE | 3 | IMMEDIATE, FIFO, MAILBOX |

The compatible-mode lists are sets, not an ordering promise. Scaling and X/Y
gravity masks are zero on this session; no implicit scaling implementation is
added. These are capability observations, not measured latency or throughput.
They illustrate why the API must preserve per-mode strategies instead of imposing
one frame/image-count policy.

The native compatibility query can silently truncate an array while returning
success. OGPU allocates one spare temporary element and retries full results,
bounded to eight attempts. It never publishes such a potentially truncated result
as a complete list. A deterministic host test covers growth and retry exhaustion.
This is cold discovery storage, not a steady-state frame allocation.

Validation: 53 host unit plus two host surface-coverage tests; 17 headless GPU unit
tests plus the native compute/image consumer on each driver; the separate
main-thread Wayland probe on both drivers, including feature enabling before
window creation, disabled/enabled device queries, headless rejection, unknown-mode
rejection, all reported modes and compatibility output capacity; strict Clippy;
1,068 C/Rust layout values; full public-C foundation fixture on Radeon. GPU runs
enable Khronos synchronization validation. No validation error markers accepted.
Device creation uses the shared Rust/C implementation; there is still no separate
successful C window consumer or Mac acceptance.

Next: borrowed swapchain images and their explicit ownership/layout rules, binary
dependencies in normal submission, acquisition/presentation retirement, then the
interactive textured scene. Nothing here proves those unimplemented lifetimes.
