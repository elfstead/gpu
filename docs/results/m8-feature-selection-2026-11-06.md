# M8 foundation revision 9: counted feature selection and SDK acceptance

2026-11-06. [Counted feature selection](../feature-selection.md) is implemented in
`cc7b3a8`. Discovery, device enabling, shader/mesh requirements, generated interfaces
and native Rust use IDs; no public feature-mask alternative remains. Selection is
cold and recording retains private direct checks. This removes the public mask
ceiling without adding GPU work or changing embedded SPIR-V.

Clean C SDK acceptance passes at
`9958934e81142c742ace46623550fd4fefff9be8`; clean isolated native Rust SDK acceptance
passes at `05849f1262276b18d7e15de1ffec20b45554d5fe`. Both manifests report foundation
9, `source_dirty=false`, and no default/legacy Cargo features. Both runtime hashes
are `ffe2554089a0a4ea7a15c1aa25f8741389d197625e7c885503576764e22243d0`.

The last commit fixes/formats only the standalone Rust consumer. All **137**
installed C headers, runtime, compiler-tool and example files are byte-identical
between these SDKs. The C suite was not rerun after that Rust-only change; its
accepted artifact identity is preserved. Source-revision metadata naturally
changes. The adjacent JSON identifies both acceptance commits and log hashes.

## Accepted checks

Linux x86-64, Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan loader 1.4.363,
validation layers 1.4.357 with synchronization validation, Rust 1.98.1, GCC 16.2
and pinned Slang 2026.14.1. These are correctness checks, not new timings.

| Check | Result |
|---|---|
| Host/runtime | 83 library tests and two native Rust correspondence tests pass; strict core Clippy and formatting pass |
| Feature contract | Unordered/duplicate IDs, a 129-element request, unknown/zero IDs, invalid pointer/count/reserved combinations, and independently owned snapshot storage pass |
| Native GPU | All 28 library GPU tests and the full public C fixture pass; generated graphics retain 96 submissions and 196,608 checked pixels |
| ABI and installed surface | 2,047 combined layout values; all 151 C operation signatures, 127 records, 141 constants including all 61 feature IDs, and exact library exports pass |
| Compiler | Compute/graphics generation tests, full indexed-scene and learned-image interface reproduction/mutation, and textured-scene reproduction/build/oracles pass; all 20 embedded SPIR-V arrays in 17 changed generated headers are unchanged |
| Relocated C SDK | Compute, graphics, learned-image, indexed scene/replay, range/count/replay and textured scene pass source-graph, runtime-identity, failure and output checks |
| GGML | Pinned `7840aaba1989c6deeefede1d77d5aaf8f52b947e`; transformer, quantized/linear and both full-dataset MNIST precision modes pass |
| libplacebo | Pinned `3330a515d62139259c26239014f286e233bd3a5c`; declared compute/raster correctness, source graph, runtime identity and failure checks pass |
| Isolated native Rust | Fresh metadata/build/link/host checks, missing-loader/GLFW handling, headless execution, surface setup, presentation lifecycle and bounded scene checks pass without the C runtime library |
| Checkers/packaging | 47 scene tests, two textured-handoff tests, eight installer tests and two surface-audit parser tests pass; standalone Rust all-target build/strict Clippy pass |

Installed acceptance found a stale scene runner version gate, incomplete C++ empty
list initialization, and a leftover Rust presentation-log variable. Follow-up
commits fix those, with a shared scene revision constant/rejection test and strict
standalone C++/Rust checks. Failed attempts are not counted as acceptance. The C
suite passes completely after its corrections; the isolated Rust suite passes
completely after its correction.

## Reproduction

From the declared clean revision, with the named toolchain, native dependencies,
loader/ICD, validation configuration and GLFW available:

```sh
python3 tools/install.py --prefix /tmp/ogpu-feature-sdk
python3 tools/check-foundation-surface.py --library /tmp/ogpu-feature-sdk/lib/libogpu.so
python3 tools/test-foundation-install.py --prefix /tmp/ogpu-feature-sdk \
  --ggml-source target/ggml-source --ggml-data target/ggml-data \
  --libplacebo-source target/libplacebo-source \
  --vulkan-registry vendor/Vulkan-Headers/registry/vk.xml
python3 tools/test-rust-install.py --prefix /tmp/ogpu-feature-sdk --presentation
```

Upstream sources/data, shaderc/SPIRV-Tools and Python/Jinja remain separate inputs.
This is a revision-pinned local-build/source SDK, not a portable binary release.
Temporary raw logs are local; the JSON stores hashes rather than distributing them.

M8 remains open. Next implement multiview, then shading rate, tile-local/pass
continuation and remaining stages. No new native hardware feature, performance
parity, Windows/Metal acceptance, second-GPU coverage or human interaction result
is established by this migration.
