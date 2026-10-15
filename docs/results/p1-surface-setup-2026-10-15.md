# P1 optional Wayland surface setup — 2026-10-15

Scope: instance opt-in, versioned C extension table, native Rust operations,
borrowed Wayland surface lifecycle, exact live capabilities/formats/present modes/
queue support, and swapchain-maintenance availability. **No presentation-enabled
device, swapchain, rendering, acquire/present or interactive acceptance.**

The standalone [surface probe](../../examples/foundation_rust/src/bin/surface.rs)
uses external GLFW 3.4 with Wayland explicitly selected, `NO_API`, and a hidden
320×240 window. OGPU does not load GLFW or own its event loop. The integration
test launches this executable so GLFW runs on the process's main thread, not a
Rust test worker. All window/native surface work runs within one bounded process.

Environment: Linux x86-64, Vulkan loader and Khronos validation 1.4.357;
synchronization validation enabled. Radeon RX 5700 XT/RADV 26.2.4 is the sole
physical GPU; llvmpipe 26.2.1 supplies software coverage, not a second GPU vendor.
The live Wayland session uses `wayland-1`.

| Observed query | Radeon | llvmpipe |
|---|---|---|
| Surface-supported queue domains | 0, 1; domains 2–4 unsupported | 0 |
| Base image count bounds | Minimum 4; maximum 0 (no explicit upper bound) | Same |
| Extent | Caller-sized; minimum 1×1, maximum 16384×16384 | Same |
| Base usage | OGPU-representable mask 159; full native mask 524447 | Same |
| Native present modes | MAILBOX, FIFO, IMMEDIATE | Same |
| Enumerated format/color-space pairs | 18, including OGPU-unrepresentable formats | 20, including OGPU-unrepresentable formats |
| Swapchain extension + maintenance feature | Available | Available |

These are observations of this session, not guaranteed surface defaults. Enumerated
extended color spaces do not establish HDR support. `format=0` keeps an unknown
format's native identity; no tuple or native usage bit is silently discarded.
No mode, queue, image count, format or frame scheduling policy is selected by OGPU.

Checks passed:

- 52 host unit tests and two host surface-coverage tests; includes table negotiation,
  unsupported platform selection, null/failure outputs, query schemas, initialized
  native output records, count-change retries, retry bounds and error mapping.
- 17 headless GPU unit tests plus the native Rust compute/image consumer on each
  driver. This still uses ordinary headless setup, without WSI requirements.
- The separate Wayland setup probe on each driver: create/query/destroy, queue
  support, malformed query outputs, wrong-instance rejection, headless-profile
  rejection, and surface destruction after discovery destruction. Radeon also
  exercises insufficient queue-array capacity; llvmpipe has only one queue domain.
- Full public-C foundation fixture on Radeon, no Vulkan validation errors. The
  C presentation table's failure paths are host-tested; successful native surface
  execution is through Rust, not a separate C window consumer.
- Strict Clippy for foundation all-targets and both standalone Rust binaries;
  formatting; C/Rust ABI comparison: 1,043 layout values, including every new
  setup record and function-table field.

The surface subprocess test rejects `Validation Error`, `VUID-` and `SYNC-HAZARD`
markers on either output stream. One earlier llvmpipe probe emitted a GLFW/libdecor
plugin-init warning but passed; it is not counted as Vulkan validation output or
proof of working window decorations.

Reproduction after configuring the toolchain, loader, validation and driver:

```sh
export OGPU_GLFW_LIBRARY=/path/to/wayland-enabled/libglfw.so.3
cargo test -p ogpu --no-default-features --offline
cargo test -p ogpu --no-default-features --offline --lib --test native_rust -- --ignored --test-threads=1
cargo test -p ogpu --no-default-features --offline --test presentation -- --ignored --nocapture
cargo xtask abi
cargo xtask foundation
```

Next: explicit device profile enabling, per-mode capability queries, borrowed
swapchain images, binary waits/signals in ordinary submission, distinct present
retirement and then the textured interactive scene. Surface-loss mapping is
host-tested, not actual compositor-loss recovery. No performance or steady-state
frame-allocation claim is made by a cold setup/query probe.
