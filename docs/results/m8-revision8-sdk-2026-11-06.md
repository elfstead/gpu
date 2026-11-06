# M8 foundation revision 8: clean Linux SDK acceptance

2026-11-06. The clean source/SDK handoff passes at
`b5414e541f87c3cd40cb2c04616c6c58ea617eab`, following buffer-state implementation
`d4a566d`. The installed manifest reports foundation 8, `source_dirty=false`,
no default/legacy Cargo features, and runtime SHA-256
`e9fb2e417312782d24bc7ce9b174a0640ee5e35d581d51cf1c4534b609013c0e`.

The [implementation receipt](m8-generated-buffer-state-2026-11-06.md) records the
new contract and 96 generated-graphics submissions / 196,608 pixel checks.
This receipt refreshes packaging and consumer acceptance; it does not complete
M8 or accept Windows/Metal. Native runtime bytes are identical to the first
revision-8 SDK; the follow-up commit corrects consumer/checker revision expectations.

## Accepted checks

Linux x86-64, Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan loader 1.4.363,
validation layers 1.4.357 with synchronization validation, Rust 1.98.1 and GCC
16.2. Validation-enabled results are correctness evidence, not performance data.

| Check | Result |
|---|---|
| Release library / installed contract | Exact foundation exports and all 151 C signatures agree with Rust; 126 records and 80 constants / 1,084 values pass |
| Relocated C SDK | Compute, graphics, learned-image, indexed scene/replay, range/count/replay and textured scene pass source-graph, runtime-identity, failure and output checks |
| GGML | Pinned `7840aaba1989c6deeefede1d77d5aaf8f52b947e`; HOST/DEVICE transformer and F32/F16/Q8_0 linear checks, plus both full-dataset MNIST precision modes pass |
| libplacebo | Pinned `3330a515d62139259c26239014f286e233bd3a5c`; five declared compute/raster checks pass, plus source graph, stale-build rejection and runtime identity |
| Isolated native Rust | Fresh metadata/build/link/host checks, missing-loader/GLFW handling, headless compute/image and bounded Wayland surface/present/scene checks pass without the C runtime library |
| Checker migration | All 47 scene checker/runner tests and two textured-handoff tests pass |

The first installed run found stale revision-7 profile expectations in scene log
checkers, after compute and learned-image had passed. The follow-up migrates both
SDK and source-tree checkers, including adversarial test fixtures. A stale-version
negative test now increments the accepted version rather than accidentally becoming
valid at the next revision. The final clean SDK reruns the entire selected consumer
suite and passes; the failed run is not counted as acceptance.

## Reproduction

With the declared dependencies, loader/ICD, validation configuration and GLFW
available, install from the pinned clean commit into a new prefix:

```sh
python3 tools/install.py --prefix /tmp/ogpu-m8-b5414e5-sdk
python3 tools/check-foundation-surface.py --library /tmp/ogpu-m8-b5414e5-sdk/lib/libogpu.so
python3 tools/test-foundation-install.py --prefix /tmp/ogpu-m8-b5414e5-sdk \
  --ggml-source target/ggml-source --ggml-data target/ggml-data \
  --libplacebo-source target/libplacebo-source \
  --vulkan-registry vendor/Vulkan-Headers/registry/vk.xml
python3 tools/test-rust-install.py --prefix /tmp/ogpu-m8-b5414e5-sdk --presentation
```

Upstream source pins, MNIST data, shaderc/SPIRV-Tools and Python/Jinja dependencies
are separate inputs. The bundle remains a local-build/source SDK, not a portable
binary distribution for arbitrary Linux machines. The adjacent JSON records local
log hashes; temporary raw logs are not distributed.

Graphics pipeline selection, generated mesh, the Vulkan DXGI input-mode branch,
other physical GPUs, Windows and Metal remain unverified. Bounded window checks do
not establish human resume/minimize acceptance or real device-loss recovery.
The next local batch addresses extensible feature selection (61 of 64 public bits
are assigned), then multiview and the remaining advanced raster profiles.
