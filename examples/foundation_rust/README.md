# Native Rust foundation consumer

This standalone Cargo package uses `ogpu::foundation::rust` directly, below the C
ABI. The default compute/image executable needs the same modern Linux/Vulkan baseline as the foundation SDK, but no
C compiler, C bindings, shader compiler or window system. Compute uses the existing
checked-in `examples/shaders/foundation-compute.comp.spv` artifact (embedded at
build time), whose source is `foundation-compute.slang` in the same directory.
It is separate from the repository workspace; the path dependency can be replaced
with an exact Git revision of the public repository (`default-features = false`).
OGPU is not published on crates.io and does not promise a stable Rust API.

```sh
cargo run --manifest-path examples/foundation_rust/Cargo.toml
```

The consumer explicitly selects a queue and compatible host-visible memory type,
maps backing, records fill/copy operations and transfer/host dependencies, submits
with caller-owned scratch, waits on a timeline, invalidates and checks all bytes.
Three A/B/A phases reuse the same arena/backing; malformed creation, poisoned
recording, timeout and insufficient submission scratch exercise errors and retry.
Additional A/B/A phases cover prepared compute with specialization, two address
roots, partial inline updates, direct/indirect dispatch and guarded CPU-reference
output checks. Cache export/import/merge, dedicated image placement, views,
descriptor/sampler encoding, explicit transitions and clear/readback are exercised
through native Rust, including malformed executable/view/descriptor requests.
Headless synchronization phases mix binary and timeline dependencies, including a
two-batch fill/copy handoff carried only by the binary semaphore. They check A/B/A
data, rejected-submit retry, explicit reuse after retirement and initial-signaled/
unsignaled fence poll/wait/reset/destruction (`NATIVE_RUST_SYNC_PASS`). The separate
windowed probe below exercises acquisition/presentation signal association.

Handles are non-owning, pointer-sized values. Copying/dropping them neither retains
nor frees anything; destruction is explicit and unsafe. The consumer performs
ordered cleanup only after successful completion. On error or uncertain completion
it exits without freeing potentially pending resources; this is a small executable,
not a general application recovery/cleanup framework.

The native surface now has equivalents for every currently exported foundation C
operation, including graphics, queries, split dependencies and secondary lists.
An integration test compile-checks that correspondence against the C header and
guards against routing native operations through C entry points. This is operation
coverage, not a claim that this consumer exercises every operation: query,
secondary and split execution still rely on the existing shared-backend/C fixtures.
The windowed scene below exercises indexed/depth/textured/blended raster.
Some descriptions use versioned records and raw embedded pointers from
`foundation::types`; ergonomic typed descriptions and generated Rust shader layouts
remain possible follow-ups. This example explicitly asserts its known root layout.
No fully safe wrapper, extra allocation/retention layer or implicit scheduling is
promised. It is not yet packaged by the C SDK installer.

## Optional Wayland surface probe

`--bin surface` is a separate, bounded hidden-window probe. It dynamically loads
external GLFW 3.4, explicitly selects Wayland and borrows its native display/surface.
GLFW remains an application dependency; OGPU has no GLFW or Wayland-library link.
Before creating a window it creates/destroys devices with presentation disabled
and explicitly enabled through the ordinary feature mask, checking reported state.
Use a trusted GLFW path if it is not on the system loader path:

```sh
OGPU_GLFW_LIBRARY=/path/to/libglfw.so.3 cargo run \
  --manifest-path examples/foundation_rust/Cargo.toml --locked --bin surface
```

This needs a live Wayland session and the optional presentation instance profile
in [the P1 contract](../../docs/presentation-contract.md). It runs the window
lifecycle on the process's main thread, checks surface capabilities, exact format/
color-space and present-mode lists, per-mode image counts/scaling/compatibility,
queue compatibility, swapchain-maintenance
availability, rejection/output semantics, and instance lifetime after discovery
destruction. It does **not** create a swapchain,
render, present, or test interactive behavior. `SURFACE_SETUP_PASS` means only
surface setup/query acceptance. The ignored integration test launches this
standalone executable (rather than running GLFW on a Rust test worker):

```sh
cargo test -p ogpu --no-default-features --test presentation -- --ignored --nocapture
```

Headless regressions can explicitly select `--lib --test native_rust` when using
`--ignored`; they still need no window system. The default binary remains headless.

## Bounded live presentation probe

```sh
OGPU_GLFW_LIBRARY=/path/to/libglfw.so.3 timeout --kill-after=2s 30s cargo run \
  --manifest-path examples/foundation_rust/Cargo.toml --locked --bin surface -- --present
```

This opens two small Wayland windows, selects MAILBOX explicitly, and clears/presents
eight two-swapchain batches. It checks borrowed-image/view protections, acquire
binaries, separate render timelines and present fences, replacement of both chains,
unused-image release after acquisition retirement and independent binary consumption,
and scratch-failure retry. Submission/presentation scratch and entry storage are
reused. The fixture serially retires frames; it is not a multi-frame scheduler,
textured scene, resize/minimize or visual/scanout correctness test.

`PRESENTATION_PASS` reports the bounded lifecycle and whether unified image layouts
were enabled. No implicit fallback: MAILBOX must be supported. Native FIFO stalled
inside presentation in the initial two-window run, consistent with compositor
throttling; the cause is not conclusively diagnosed. Native present may block,
and event-loop/mode policy belongs to the application. The ignored integration
test uses GNU `timeout` to bound the entire Cargo/consumer process group.

On uncertain GPU errors this executable exits without tearing down potentially
pending objects/native windows. Ordinary successful teardown explicitly retires
all work; no wait-on-drop or device-idle teardown is introduced into OGPU.
See the [receipt](../../docs/results/p1-swapchain-lifecycle-2026-10-15.md).

## Interactive textured scene

```sh
OGPU_GLFW_LIBRARY=/path/to/libglfw.so.3 cargo run \
  --manifest-path examples/foundation_rust/Cargo.toml --locked --bin surface -- --scene
```

WASD pans, Q/E zooms, 1–4 selects a mip, Space pauses rendering, Escape closes.
Two indexed quads use depth testing, mipmapped nearest sampling and a scissored
alpha-blended overlay. This is an application example, not an engine or a runtime
frame scheduler. It explicitly chooses MAILBOX and a graphics/present-capable queue,
RGBA8/BGRA8 UNORM with SDR color space, D32 depth and three reusable frame slots.
Those are consumer choices, not OGPU requirements or silent backend fallbacks.

Each slot owns its geometry/depth storage, command arena, acquire/render binaries,
render timeline value and present-retirement fence. Reuse waits only for that slot;
resize, suspension and normal close explicitly drain all slots. A new swapchain
generation replaces the old chain after retirement. Zero framebuffer extents and
Space pause suspend acquisition. Out-of-date/suboptimal causes replacement; uncertain
errors terminate without freeing potentially pending objects. This does not implement
surface/device-loss recovery. Normal rendering has no pixel readback, per-frame
application heap allocation or device-idle wait; native presentation can still block.

`build.rs` embeds the existing M4 SPIR-V arrays from
`../graphics_scene/textured_scene.generated.h` and checks its known 24-byte argument
layout against the Rust record. No C/shader compiler or runtime reflection is used.
Keep that sibling file (and the default compute shader above) when copying this
package. This narrow bridge is not general generated Rust shader-layout support.

For bounded automated runs, replace `--scene` with:

- `--scene-check`: 18 frames, scripted pan/zoom/mip changes, two observed framebuffer
  resizes, one synthetic zero-extent suspension, and close with unretired slots.
  Copies rendered images into per-slot buffers and checks sampled pixels against
  CPU texture/depth/blend equations, plus guard bytes.
- `--scene-smoke`: the same lifecycle using the normal no-readback recording path.

Both are included in the ignored `presentation` integration tests above; GNU
`timeout` bounds subprocesses. `SCENE_PASS` reports application-unretired slots,
not simultaneous GPU execution or displayed/scanout correctness. Pixel sample and
replacement counts can vary with compositor sizing. Interactive keyboard input
is not synthesized by these checks. GLFW on Wayland cannot report iconification
or restore a minimized window; synthetic zero extent is **not** evidence of actual
minimize/restore. See [GLFW's window guide](https://www.glfw.org/docs/latest/window_guide.html)
and the [scene receipt](../../docs/results/p1-interactive-scene-2026-10-16.md).
