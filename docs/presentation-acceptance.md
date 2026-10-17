# P1 manual interaction acceptance

Status: **partially human-accepted**; see the
[2026-10-17 observations](results/p1-human-interaction-2026-10-17.md). Controls,
resize, pause and close were reported working. Resume was not tested;
minimize/restore is unavailable on this compositor. The detailed checklist is
not fully accepted. Automated scripted input, zero extents and
pixel readback are separate evidence. Do not mark this checklist complete merely
because `--scene-check` or `--scene-smoke` prints `SCENE_PASS`.

Use the packaged `native-rust` bundle README to build in a writable directory.
Run `surface --scene` on a real Wayland desktop, with GLFW 3.4 and a supported
Vulkan driver. Use Khronos synchronization validation when available. The scene
explicitly selects MAILBOX; unavailable modes are a reported limitation, not a
reason to silently change the mode. Keep stdout/stderr for the receipt.

Record before starting: OGPU revision/dirty state from `source.json`, Rust version,
GPU/driver, Vulkan loader/layers, GLFW build, compositor/version, display scale,
selected present mode and whether validation is enabled. Describe any environment
overrides. Do not include credentials or unrelated environment variables.

1. Confirm the textured quads and blended overlay are visible and stable.
2. Use WASD to pan and Q/E to zoom. Check immediate response and sensible motion.
   Press 1–4 to select mips; observe texture detail change. Release all keys and
   check that movement stops.
3. Press Space to pause and again to resume. The window should remain responsive;
   resuming should produce valid rendering, not a stale or corrupted generation.
4. Repeatedly resize smaller/larger, maximize/unmaximize and move between available
   displays/scales. Record unsupported compositor operations; no second display
   is required. Check the framebuffer follows the window without corruption.
5. Minimize through the compositor, leave it hidden, then restore and interact.
   If the compositor has no minimize operation, record **unverified**, not passed.
   Wayland GLFW cannot report iconification or restore the window itself; synthetic
   zero extent does not prove this case. Record stalls, unexpected continued work
   or abnormal CPU usage separately from visible recovery.
6. Close while actively rendering, once using Escape and once using the window
   close action. Check timely process exit, no lingering window, no validation
   errors, and no crash during retirement/teardown. Repeat after a resize.

Report each item as pass/fail/unverified with the exact action and observation.
Attach relevant logs and reproduction steps for failures. This is visible
application acceptance, not a scanout/colorimetry proof, latency benchmark or
surface/device-loss recovery test. A failed or unavailable check stays open;
it does not block unrelated Linux work or justify a hidden runtime workaround.
