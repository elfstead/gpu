# P1 partial human interaction acceptance

Date: 2026-10-17. Consumer built from the clean `04fdfa3` source bundle described
in the [packaging receipt](p1-rust-package-2026-10-17.md). These observations add
human evidence; they do not replace or retrospectively change its automated reports.

The agent launched the copied consumer with `surface --scene` on the user's
Wayland desktop, selecting Radeon/RADV, external GLFW 3.4 and Khronos
synchronization validation. The executable was under
`/tmp/ogpu-rust-external-4f08mvd9/independent Rust source/examples/foundation_rust/target/release/`.
Other toolchain/driver details are in the packaging receipt. Compositor version,
display scale and multi-display behavior were not recorded. The consumer selects
MAILBOX explicitly.

## Observations

| Action | Evidence / disposition |
|---|---|
| Visible scene | User observed colorful rectangles. This is the intended small textured-quad/blended-overlay scene, not a game or realistic rendering demo. No independent visual-quality or scanout claim. |
| Controls | User reported that controls work. Individual key/mip/release actions were not separately itemized. |
| Resize | User reported that resizing works. Maximize/unmaximize and display/scale transitions were not separately reported. |
| Pause | User explicitly confirmed that pause worked on the second run. |
| Resume | User explicitly did not test resume; remains unverified. |
| Minimize/restore | User reported no compositor support; remains unverified, not a pass or a rendering failure. |
| Close | User reported close works. Both runs exited successfully; Escape and window-close routes were not separately identified. |

The first process exited with status 0 and no validation diagnostics:

```text
SCENE_PASS frames=364226 slots=3 peak_pending=3 resize=41 resize_observed=[false, false] readback=false checked_pixels=0 pending_close=3 synthetic_suspend=false
```

The second process also exited successfully, without validation diagnostics:

```text
SCENE_PASS frames=50151 slots=3 peak_pending=3 resize=1 resize_observed=[false, false] readback=false checked_pixels=0 pending_close=0 synthetic_suspend=false
```

`resize_observed` refers to scripted resize targets, which interactive mode did
not request. Its false values do not contradict the user's resize observation.
These are interactive no-readback runs, not pixel checks or performance samples.
Three pending slots on first close support the teardown observation; the second
run does not add a pending-work close case.

## What remains

The [full checklist](../presentation-acceptance.md) is only partially accepted.
Resume, exact per-control observations, both close routes and any supported
maximize/scale transitions remain unverified. Real minimize/restore needs a
compositor that supports it; no second display or new machine is required here.
Do not keep relaunching or block M6 design/unrelated Linux work to manufacture
those observations. They can be added when the user next exercises the consumer.
