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
unsignaled fence poll/wait/reset/destruction (`NATIVE_RUST_SYNC_PASS`). Fence signal
association through acquisition/presentation remains unimplemented.

Handles are non-owning, pointer-sized values. Copying/dropping them neither retains
nor frees anything; destruction is explicit and unsafe. The consumer performs
ordered cleanup only after successful completion. On error or uncertain completion
it exits without freeing potentially pending resources; this is a small executable,
not a general application recovery/cleanup framework.

The native surface now has equivalents for every currently exported foundation C
operation, including graphics, queries, split dependencies and secondary lists.
An integration test compile-checks that correspondence against the C header and
guards against routing native operations through C entry points. This is operation
coverage, not a claim that this consumer exercises every operation: raster, query,
secondary and split execution still rely on the existing shared-backend/C fixtures.
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
