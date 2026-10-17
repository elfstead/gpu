# Native Rust OGPU source bundle

Copy this **whole directory**, not just the example, to a writable location.
It contains a revision-matched runtime source dependency, a standalone consumer,
its Cargo.lock and embedded compute/textured-scene shader inputs. `source.json`
records the revision and whether the installation was made from a dirty checkout.
The parent SDK's `share/ogpu/manifest.json` hashes every packaged source/input.
No source-tree paths or OGPU shared library are needed at build or execution time.

This experimental bundle currently targets Linux x86-64, not Metal. It is neither
a crates.io publication nor a stable Rust API. Build from the consumer manifest:

```sh
cargo build --locked --release --manifest-path examples/foundation_rust/Cargo.toml --bins
cargo test --locked --manifest-path examples/foundation_rust/Cargo.toml
./examples/foundation_rust/target/release/ogpu-native-rust-consumer
OGPU_GLFW_LIBRARY=/absolute/path/to/libglfw.so.3 \
  ./examples/foundation_rust/target/release/surface --scene
```

Rust/Cargo 1.85 or newer and a native linker are required. Registry dependencies
are checksum/version-pinned by Cargo.lock, **not vendored**: allow Cargo to fetch
them, or supply a populated cache and add `--offline`. Nothing downloads a driver,
window library or shader toolchain. No C headers/compiler frontend, Vulkan headers,
Slang, SPIRV-Tools, xtask or pre-existing target directory is needed to build this
consumer. A linker is still needed for Rust. The bundle is not the full repository
test suite; use the consumer manifest rather than testing the runtime workspace.

Execution needs OGPU's modern Vulkan baseline and a trusted Vulkan loader
(`OGPU_VULKAN_LIBRARY` selects an explicit library). The default executable is
headless. `surface` additionally needs a live Wayland session, GLFW 3.4 built with
Wayland support, and the optional presentation profile. The scene explicitly
selects MAILBOX; unsupported configurations fail rather than choosing a fallback.
`VK_DRIVER_FILES` can select one ICD. Validation layers are optional system tools;
enable Khronos synchronization validation for acceptance runs.

Bounded display checks, preferably with an external process timeout:

```sh
timeout --kill-after=2s 30s ./examples/foundation_rust/target/release/surface --scene-check
timeout --kill-after=2s 30s ./examples/foundation_rust/target/release/surface --scene-smoke
```

These are automated pixel/lifecycle checks, not human input or minimize/restore
acceptance. Follow [the manual checklist](docs/presentation-acceptance.md).
[The consumer README](examples/foundation_rust/README.md) explains controls,
unsafe lifetime obligations, probes and limitations. Native Rust embeds the
runtime directly and bypasses C entry points; do not exchange handles with a
separately loaded SDK `libogpu.so` instance.
