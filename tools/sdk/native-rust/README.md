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


## Memory and native interoperability consumers

The independent binaries `native`, `external` and `sparse` exercise borrowed/adopted
Vulkan resources, Linux opaque-FD memory/timeline transport, and explicit sparse
buffer/image page bindings. `external` requires its exact native handle profile;
`sparse` requires sparse binding and separately reports unavailable residency
profiles. They do not select emulation or substitute CPU copies. The `ray_native`
binary is a direct Vulkan diagnostic for the combined preprocessing/ray-storage
observation; it is not an acceleration benchmark or a passing acceptance claim.
See [the M10 contract](docs/m10-memory-interop.md) in the source bundle (repository:
`docs/m10-memory-interop.md`) for capability, lifetime and reproduction details.

From the repository, `tools/test-rust-install.py --prefix SDK --external --sparse`
requires those optional native profiles in the relocated bundle. Select loader,
driver and validation externally. The external SyncVal control is explicitly
selected by `OGPU_EXTERNAL_SYNC_CONTROL=1`; without it, the documented external
signal-tracking diagnostic remains visible and causes the checker to fail.

## Shader and GPU execution profiles

`controls` checks GPU-produced predicates and independent primitive counts without
transform feedback. `shader_profiles` checks subgroup/cluster rotation, linear and
quad compute derivatives, and narrow push storage against exact references. It
prints per-profile native exclusions; it does not emulate unavailable features.
Run relocated acceptance with `--execution-controls --shader-profiles` on
`tools/test-rust-install.py`. [The M11 brief](docs/m11-compute-media.md) distinguishes
implementation, feature enablement, executed artifacts, and remaining acceptance.


`matrix_ml` runs a bounded two-layer 16x16 MLP using an exact native subgroup
16x16x16 half-input/float-accumulator tuple. It explicitly dispatches matrix,
ReLU/half conversion, and matrix, then checks both float intermediates, half
storage, preserved input/root bytes and guards across A/B/A replay. Inputs are
exact binary fractions, so this witness uses exact references rather than claiming
an error bound for arbitrary ML data. It returns exit 77 if native requirements
are unavailable; there is no scalar fallback. The installed checker `--matrix-ml`
requires actual execution and must not be used as a local success claim without
matrix hardware. The binary and shaders are included even on unsupported hosts.


`artifacts` checks native pipeline binary identity, capture/export/import, explicit
release and source-free recreation with normal/disabled driver-cache policies.
Compute outputs and guards are compared for original/recreated executables after
the imported binaries are destroyed. Graphics recreation is preparation-only.
`test-rust-install.py --pipeline-artifacts` requires actual native execution.
The separate `OGPU_BINARY_CAPACITY_DIAGNOSTIC=1` case exercises undersized capture;
VVL 1.4.357 omits threading-object registration on VK_INCOMPLETE and emits known
raw diagnostics. This case is not part of clean validation acceptance; see the
M11 artifact contract for evidence and unresolved scope.


`ray_artifacts` checks nested ray-library linkage, imported shader indices, stable
group handles and optional capture/replay on a fresh device. The caller retains
source libraries and layouts until all derived executables are retired/destroyed.
It exits 77 when ray libraries are unavailable and never selects emulation.
`test-rust-install.py --ray-artifacts` requires library execution; the log separates
optional replay exclusion. Explicit `RADV_EXPERIMENTAL=emulate_rt` results remain
diagnostics, not native ray-hardware acceptance. See the M11 ray-artifact contract.
