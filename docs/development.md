# Building the prototype

The initial backend is Rust with directly generated Vulkan declarations and a small
C ABI. The runtime does not use ash, Vulkanalia, C++, or Kotlin. The optional
[GGML consumer](../integrations/ggml/README.md) has a C++ adapter/application build;
it uses the public C ABI and does not change the Rust runtime. Only Linux x86-64 is currently
supported and tested. The ABI is experimental, not a specification of the eventual
execution interface.

For the first real consumer, see [GGML preparation and acceptance commands](../integrations/ggml/README.md).
Its test-data downloads and CMake build are separate from ordinary Cargo builds.
Acceptance uses checked-in, hash-pinned weights; CPU training is a separate optional
script. Hosted CI covers build/mock/ABI/SPIR-V checks. GPU and consumer execution
are in the manual `gpu.yml` workflow, requiring a separately provisioned
`ogpu-modern-vulkan` runner with the selected baseline, validation layers, build
tools and test-data download access. No runner was provisioned or remote run
performed here. Automatic hosted CI is not currently an execution gate.

## Build and run

`cargo xtask heap-image` runs direct image load/store and sampling through immutable
image tables. New shader binaries are checked in; regeneration uses pinned Slang
2026.14.1 with `SLANGC=/path/to/slangc cargo xtask heap-shaders --check` and modern
SPIRV-Tools. See [the experiment/toolchain record](heap-images.md).

`cargo xtask retirement` compares caller-owned and explicitly retained scratch
allocations through the public C ABI. `cargo xtask gpu-tests` includes the
deterministic gated reuse and polling/error checks; see [retirement](retirement.md).

Requirements: Rust 1.85+ with Cargo, a C11 compiler/linker, and a Vulkan loader with
Vulkan 1.1+ support. A GPU is not required to compile or run the mock tests.

```sh
cargo build --locked
cargo xtask smoke
cargo xtask compute
cargo xtask batch
cargo xtask graphics
cargo xtask image-loop
cargo xtask reduction
cargo xtask matmul
```

The first command builds `target/debug/libogpu.so` using checked-in bindings. It
needs neither the headers submodule nor Clang/libclang. The second builds and runs
[the discovery caller](../examples/probe.c) against [our header](../include/ogpu.h).
`compute` builds and runs [the C execution example](../examples/compute.c), which
requires the [modern Vulkan execution baseline](modern-baseline.md).
Run `cargo xtask baseline` first to inspect the exact feature/limit requirements.
It verifies upload → dispatch → blocking completion → readback, including repeated
dispatches and releasing parent handles before using their children.

`batch` runs [the asynchronous C example](../examples/batch.c): two different
kernels linked by an explicit memory dependency, one submission, and one final
wait/readback. See the [batch contract](batches.md) for lifetime rules and shader
regeneration commands.

`graphics` runs [the offscreen C example](../examples/graphics.c): compute generates
vertices and indirect arguments, graphics draws a triangle, and the same batch
copies the target for CPU verification after one wait. It requires a shared
graphics+compute queue and a supported RGBA8 attachment/readback image. No display
or presentation system is needed. See the [graphics contract](graphics.md).

`image-loop` runs [the mixed-workload C example](../examples/image_loop.c) on every
graphics-capable device: draw → image-to-buffer copy → compute transform → fragment
address reads → draw → final copy, with one submission/wait per loop. The C example
also serves as the expanded test: six image sizes, two runs each, exact intermediate
and final pixel checks, guards, and target reuse. It runs separately from the Rust
`gpu-tests` command. See the [image-loop contract](image-loop.md).

`reduction` runs [the cooperative reduction example](../examples/reduction.c):
1,048,579 uint32 inputs become 8,193 partials, then 65, then one modulo-2^32 sum.
Shared workgroup memory and shader barriers cooperate within a group; batch
barriers connect levels. One final wait precedes readback. See the
[reduction experiment](reduction.md) for the numerical contract and test cases.

`matmul` builds the runtime in release mode and its [C harness](../examples/matmul.c)
with `-O2 -DNDEBUG` (checks remain active). It compares baseline and shared-memory
FP32 matrix kernels against FP64 CPU references on every compute-capable device,
including padded row strides, guards, and awkward shapes. It reports setup/copy
costs and warmed host execution latency separately. It also compares timed/untimed
batches when the selected queue supports timestamps, reporting device-batch duration
and query-read cost separately. Unsupported clocks retain host-only measurements.
Run without validation for comparative timing, and with validation for correctness.
See [the matrix contract and recorded measurements](matmul.md).

`cargo xtask gpu-tests` runs the Vulkan-backed Rust tests, including asynchronous
batch lifetimes, dependencies across submissions, and injected preparation,
submission, and wait errors, plus graphics image reuse, ownership, and partial
creation failures. It also checks reduction stage boundaries, overflow patterns,
every intermediate partial, and buffer guards. Optional [timing tests](timing.md)
cover wrap arithmetic, support fallback, read states, query failures, and resource
cleanup; existing mixed graphics tests also exercise timed submissions.
Like the C runner, it fails on Vulkan validation
errors even when the test process exits successfully. To enable synchronization
validation, set `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` and
`VK_LAYER_VALIDATE_SYNC=1` (requires installed validation layers).

`CC` may select a C compiler executable. Test tools currently use the repository's
`target/` directory; leave `CARGO_TARGET_DIR` unset.

By default the runtime loads `libvulkan.so.1`. For an unusual loader installation:

```sh
OGPU_VULKAN_LIBRARY=/absolute/path/to/libvulkan.so.1 cargo xtask smoke
```

This override loads executable code, so use only a trusted library. Vulkan's normal
driver-selection environment variables remain available through the loader. The
probe does not change driver configuration or require a display/window.

The probe owns one Vulkan instance and an immutable device-information snapshot.
Creation queries devices, queues, and supported feature bits; it creates no logical
device, enables no device features, and submits no GPU work. An empty enumeration
is successful. Optional accelerators never gate startup.

Capability bits are queried features, **not simply advertised extension names**.
Promoted core features are queried using the application ceiling and device API version;
extension features are queried only when advertised. Storage access and shader
arithmetic types remain separate. Cooperative-matrix support does not establish
specific shapes/types, performance, or shader-compiler support; those queries come
with the kernel experiments. This report is preliminary Vulkan support information,
not negotiation of a portable graphics or ML profile.

## Reproduce and test the bindings

```sh
git submodule update --init vendor/Vulkan-Headers
cargo xtask bindings --check
cargo test --locked
cargo xtask abi
cargo xtask mock
cargo xtask smoke
cargo xtask compute
cargo xtask batch
cargo xtask gpu-tests
cargo xtask graphics
cargo xtask image-loop
cargo xtask reduction
cargo xtask matmul
```

`bindings` uses **bindgen 0.72.1**, pinned in the Rust tooling crate and Cargo.lock,
with Khronos Vulkan-Headers **1.4.357**, revision
`e3b1eec08173d6b825cd3ac88c885a63b621504a`, pinned as a submodule. This is the audited
baseline, not a floating request for the latest SDK. The command rejects a different
revision or modified tracked header files.

Only regeneration requires libclang and its C system headers. Formatting uses the
Rust `prettyplease` dependency pinned by Cargo.lock, not an external rustfmt. Set
`LIBCLANG_PATH` if libclang is not discoverable; bindgen also accepts
`BINDGEN_EXTRA_CLANG_ARGS` for system include paths. The initial generation was
tested with Clang 21.1.8. No generator runs during normal library builds.

`cargo xtask bindings` writes generated declarations; `--check` compares without
writing. The allowlist includes the loader/query/execution commands and feature
structures used by the prototype, plus their dependencies. To extend it, edit
[the type list](../tools/vulkan-types.txt) or [command list](../tools/vulkan-commands.txt),
regenerate, inspect the diff, and extend [ABI coverage](../tests/abi.txt).

`abi` independently compiles C and Rust programs and compares sizes, alignments,
and selected field offsets, covering every feature structure used here and every
public data field. Generated assertions also validate the generated declarations.
This is target-specific ABI evidence, not a proof of Vulkan semantic correctness.

`mock` builds a tiny test-only Vulkan loader and exercises the real C boundary.
It checks a Vulkan 1.1 instance with a 1.3 device (visible in discovery but rejected
for modern execution), core promotion without extension
advertisements, an advertised extension
whose feature is false, absent optional extensions, zero devices, loader failure,
and instance cleanup on a Vulkan error. The C caller is compiled with `NDEBUG` to
ensure its checks and API calls also work in release builds. Rust unit tests exercise
changing enumeration counts, bounded `VK_INCOMPLETE` retries, diagnostics, and panic
containment. `smoke` uses the real loader and available drivers.

## Boundary and next step

The C header documents ownership, pointer validity, immutable concurrent queries,
version checks, output behavior, and destruction. Errors are returned directly;
there is no shared last-error buffer. Rust panics are contained at fallible C entry
points, but invalid caller pointers, driver faults, and allocation aborts are not
recoverable API errors.

The [execution experiment](execution.md) now exposes allocation → upload → compute
dispatch → completion → readback through C as well as testing the Rust backend.
The [batch contract](batches.md) extends this with explicit dependencies and
one-shot asynchronous submissions; the blocking helper uses that implementation.
The offscreen graphics experiment now exercises those shared foundations; it does
not settle a complete graphics profile, shader language, executable format, or ML profile.

The project code is MIT licensed. Khronos headers retain their upstream licenses in
the submodule; third-party Rust dependencies retain their own licenses.
