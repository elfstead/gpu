# C3: Vulkan/Linux acceptance of foundation revision 7

2026-11-06. **Local C3 is complete** at
`fc11fdaa467bcbeecadf77ceed91cab420ff026f` on `minimal-core`, following C2 audit
`80b5c1b` and the output-initialization correction. The clean SDK manifest records
`source_dirty=false`, foundation 7, no default/legacy Cargo features and runtime
SHA-256 `5d668ce51c535073615823d2a34b63d9ee3d5f817788c60415ce5f9a1290132b`.
C3 remains unaccepted on Windows/Metal. Remaining M8 work can resume locally.

## Environment and checks

Linux x86-64, AMD Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan loader 1.4.363,
validation layers 1.4.357 with synchronization validation, and a live Wayland
session. Rust 1.98.1 and GCC 16.2 build the runtime and consumers. Pinned Slang
2026.14.1 reproduces the foundation shader/compiler artifacts in the C2 audit.
Validation-enabled execution here is correctness evidence, not a timing campaign.

| Check | Result |
|---|---|
| Host and direct Rust correspondence | 78 library tests and two correspondence tests pass; strict Clippy and formatting pass |
| Installed C/Rust contract | 151 signatures, 123 records, 76 selected constants / 1,063 values and exact foundation exports agree; combined ABI fixture checks 2,029 values |
| Native GPU tests | All 28 library tests pass; injected failures cover selected rejection/uncertain-acceptance cases |
| Direct Rust/native window checks | One headless GPU test and three bounded Wayland tests pass |
| Public C fixture | Passes on Radeon, including generated compute, arguments/descriptors, dependencies/submission, image/raster/numeric and supported compressed profiles |
| Relocated C SDK | Six public-only example families pass source/dependency graph, exact runtime identity, missing-loader handling and execution checks |
| Independent GGML adapter | Pinned upstream `7840aaba1989c6deeefede1d77d5aaf8f52b947e`; HOST/DEVICE transformer and F32/F16/Q8_0 linear matrices, plus both full-dataset MNIST precision modes pass |
| Independent libplacebo adapter | Pinned upstream `3330a515d62139259c26239014f286e233bd3a5c`; all five declared runs pass, including compute/raster checks; source graph, stale-build rejection and runtime identity pass |
| Isolated native Rust source SDK | Fresh metadata/build/link/host checks, missing-loader/GLFW handling, headless compute/image, surface/present and bounded scene checks pass without linking the C SDK library |

The C relocation runner checks the learned-image, indexed-scene, range/replay and
textured-scene outputs, including 32 indexed-scene and 48 range replay cases and
18 textured frames at each of two extents. These remain bounded consumers, not a
general engine, tensor runtime or third-party adoption claim.

The initial pre-correction external run stopped at upstream libplacebo's missing
Python/Jinja dependency. The final clean-SDK run above supplies the installed
Jinja/MarkupSafe packages and passes. No OGPU runtime change or test waiver was
used to address that build environment failure.

## Reproduction

With the declared tools, Vulkan loader/ICD, validation configuration and GLFW
library available:

```sh
cargo test --offline --no-default-features -p ogpu
cargo test --offline --no-default-features -p ogpu -- --ignored --test-threads=1
cargo xtask abi
cargo xtask foundation
python3 tools/install.py --prefix /tmp/ogpu-c3-r7-final-sdk
python3 tools/check-foundation-surface.py --library /tmp/ogpu-c3-r7-final-sdk/lib/libogpu.so
python3 tools/test-foundation-install.py --prefix /tmp/ogpu-c3-r7-final-sdk \
  --ggml-source target/ggml-source --ggml-data target/ggml-data \
  --libplacebo-source target/libplacebo-source \
  --vulkan-registry vendor/Vulkan-Headers/registry/vk.xml
python3 tools/test-rust-install.py --prefix /tmp/ogpu-c3-r7-final-sdk --presentation
```

Use a new prefix when repeating installation. Source checkouts must contain the
pinned upstream commits; MNIST data and upstream build dependencies are separate
consumer inputs. The SDK is a local-build/source handoff, not a self-contained
binary distribution for arbitrary Linux machines. The adjacent JSON records
command outcomes and local log hashes; temporary raw logs are not distributed.

## Limits and next work

Radeon does not execute the optional mesh/task or matrix fixture profiles. The
float16 atomic profiles, float32/64 buffer-add and float64 shared-add profiles
are also explicitly unexercised. Native graphics-generated commands have not
been accepted by this receipt; the C fixture's generated execution is compute.
Earlier llvmpipe results remain historical, not a fresh revision-7 run.

No real device loss, noncoherent physical memory, second physical GPU/vendor,
Windows/DXGI or Metal acceptance is claimed. Human resume and actual
minimize/restore remain unverified. Synthetic lifecycle faults and bounded
windows do not close those gaps. No new performance parity claim or ABI
stabilization follows from these checks.

Next: M8 generated graphics execution and a consumer-owned GPU culling/compaction
scene, followed by remaining generated token and advanced raster profiles. Reuse
the installed mechanisms first; add host operations only for non-derivable native
capability. M9 rays, M10 memory/interoperability and M11 compute/media follow.
