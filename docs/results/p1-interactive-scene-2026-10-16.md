# P1 native Rust textured scene — 2026-10-16

Follow-up to [swapchain lifecycle](p1-swapchain-lifecycle-2026-10-15.md).
**Interactive implementation and bounded native execution, not complete P1 acceptance.**
No public API, runtime implementation or C layout changes in this checkpoint.

## Consumer and policy

The standalone [Rust consumer](../../examples/foundation_rust/README.md#interactive-textured-scene)
now renders indexed textured quads with depth testing, four mip levels and a
scissored alpha-blended overlay. WASD/Q/E/1–4 control camera and mip selection;
Space pauses and Escape closes. It reuses M4's exact checked-in shader artifacts
through a build-time extraction/layout check; no C/shader compiler or runtime
reflection is introduced. This is not a general Rust shader-interface generator.

The application selects MAILBOX, SDR RGBA8/BGRA8 UNORM, D32 depth, one compatible
graphics/present queue and three reusable slots. These policies are not built into
OGPU. Each slot has explicit command/geometry/depth storage, acquisition/render
binaries, a render-completion timeline value and an independent present fence.
Ordinary slot reuse retires only that slot; replacement, suspension and close drain
all pending slots. Replacement recreates extent-dependent storage/views, and
pipelines only if the surface format changes. No device-idle wait, hidden resource
retention, application per-frame heap allocation or mandatory pixel copy is added.
Native present can block; this is not a nonblocking event-loop guarantee.

Framebuffer size, not requested logical window size, determines the next extent.
Out-of-date/suboptimal triggers replacement. Zero extent and Space pause stop
acquisition. Uncertain failures exit without unwinding potentially pending GPU or
native-window ownership; this small executable is not a loss-recovery framework.

## Evidence

Live Sway/Wayland (`wayland-1`), external GLFW 3.4, Vulkan loader and Khronos
synchronization validation 1.4.357. Same Radeon RX 5700 XT/RADV 26.2.4 and llvmpipe
26.2.1 as the preceding checkpoint. One physical GPU plus software, not cross-vendor
physical coverage. All GPU logs are free of VUID, validation-error and synchronization-
hazard diagnostics.

| Bounded run | Radeon | llvmpipe |
|---|---:|---:|
| `--scene-check` rendered frames | 18 | 18 |
| Sampled pixels checked | 64,776 | 57,717 |
| Swapchain replacements | 3 | 3 |
| Requested framebuffer resizes observed | 2 | 2 |
| Peak application-unretired slots / slots pending at close | 3 / 3 | 3 / 3 |
| `--scene-smoke` frames, no readback | 18 | 18 |

Both modes exercise scripted pan/zoom/mip changes, two observed framebuffer-size changes,
one synthetic zero extent through the real suspension branch and normal close with
pending slots. Both retire render and presentation work before successful destruction.
Sample/replacement counts can vary with compositor configuration events and sizing;
they are observations, not fixed acceptance thresholds. Pending slots do not prove
simultaneous GPU execution.

The check mode adds explicit COPY_SRC usage and per-slot readback, compares sampled
pixels against CPU geometry/UV/nearest-texture/depth/blend equations (two-byte
channel tolerance), avoids raster-edge/texel-tie ambiguities and checks a 64-byte
guard. The texture recipe is shared with upload; the raster/blend expectation is
computed on the CPU. Near geometry is drawn before far geometry, so correct overlap
depends on depth rejection rather than painter's order. It is not an independent replacement for all M4 offscreen
checks or proof of display scanout. The smoke mode omits readback storage, image
copies and copy transitions, exercising the normal rendering path instead.

Regression checks:

- 55 foundation host unit tests and two native-Rust surface/boundary checks pass.
- Two standalone scene host tests check suspension policy, known-clear acceptance
  in both channel orders and pixel/guard corruption rejection.
- Both drivers pass 19 headless GPU unit tests, the native Rust consumer and three
  Wayland integration tests (setup, two-window lifecycle, textured scene). The
  textured integration test runs both bounded modes as main-thread subprocesses.
- Foundation and standalone strict Clippy and formatting pass.
- The preceding runtime checkpoint's 1,122 C/Rust ABI comparisons and public-C fixture
  acceptance remain the relevant evidence: neither the runtime nor header changes here.

Reproduce with a trusted GLFW path, chosen ICD and enabled Khronos synchronization
validation using the consumer instructions, then:

```sh
cargo test -p ogpu --no-default-features
cargo test --manifest-path examples/foundation_rust/Cargo.toml --all-targets
cargo test -p ogpu --no-default-features -- --ignored --nocapture
```

The integration harness applies GNU `timeout` (30 seconds plus two-second kill
grace) to each window subprocess. These tests need a live Wayland session; they
remain ignored in ordinary headless host runs.

## Remaining proof and next step

No automated keyboard-event injection, human visual/input acceptance, real window
minimize/restore, scanout verification, actual loss recovery, cross-family
presentation or performance claim. GLFW on Wayland cannot report iconification or
restore a minimized window; its iconified attribute does not supply this state.
See the [GLFW window guide](https://www.glfw.org/docs/latest/window_guide.html).
Do not turn the synthetic zero extent into a claimed compositor minimize test.
The manual test must use the compositor's own hide/minimize/restore facilities and
observe continued event-loop responsiveness and rendering after restoration.

Next package the native Rust consumer and checked shader dependencies for an
isolated build/run, with an explicit human acceptance checklist. The source package
currently needs sibling M4/compute artifacts; it is not yet installed by the SDK.
M6 remains after P1 and has not started. No Metal acceptance follows from these runs.
