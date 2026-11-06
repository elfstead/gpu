# M8 framebuffer-local read acceptance

2026-11-06, foundation revision 11. Implementation `64e098857185d0ddadd1645e26d4098a31a61920`. The [contract](../local-read.md) adds local-read
feature discovery/enabling, output/input mappings, preparation-time input attachment
bindings, explicit LOCAL_READ image state and framebuffer-region dependencies.
Existing public record layouts are unchanged. There is no subpass graph or implicit
pass break, descriptor copying, resource retention or synchronization inference.

## Local native result

Linux x86-64, Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan loader 1.4.363,
validation layers 1.4.357 with synchronization validation; Rust 1.98.1,
Slang 2026.14.1. Native limits report 8,388,606 input indices, depth/stencil reads
and multisample reads supported. Only ordinary color reads execute in this tranche.

The public C fixture draws twice into two bound 8×8 RGBA8 attachments. Producer
writes attachment zero and same-pixel address storage. Consumer reads both and
writes attachment one; output location zero is explicitly remapped. A nonzero
resource-heap location comes from existing argument bytes through the native
input-attachment shader binding mapping.

Four modes cover GENERAL/LOCAL_READ layouts and global/resource-specific region
barriers. Two serial replays per mode pass: **8 submissions, 1,024 attachment
pixels and 512 storage values**, with fresh host data and untouched guards.
Four malformed executable mappings reject before preparation; sixteen malformed
recordings poison command end without submitting work. They cover root bounds/
alignment, array stride, duplicate bindings, mapping counts/reserved/indices,
command scope, missing region flags, non-framebuffer stages, transitions/discards,
ownership and unknown flags.

The complete existing C fixture also passes, with no validation errors. Four
pre-existing unused-fragment-output warnings remain in the raster fixture's null/
missing color-slot cases; no local-read validation diagnostic remains. The first
development run exposed Slang's omitted NonWritable decoration on SubpassInput;
the corrected native mapping handles either image classification and the final
native run passes. No diagnostic was suppressed.

## Other checks and limits

- 92 host library tests and 2 native Rust correspondence tests pass.
- 30 native library tests pass, including independent LOCAL_READ enabling,
  disabled mapping rejection and inheritance binding rejection.
- Strict core Clippy, C/Rust ABI (2,136 layout values) and foundation surface checks
  pass: 154 operations, 136 records, 172 constants, 1,251 correspondence values.
- Vulkan bindings and all foundation shaders reproduce. The three new SPIR-V
  artifacts validate. Existing compute/graphics compiler checks pass.

This is native correctness and direct-lowering evidence, not a timing claim or
proof of tile residency. Depth/stencil/MSAA reads, arrays, secondary execution,
multiview, continuation and other raster combinations remain unverified here.
The bounded generated-interface compiler does not yet emit the new binding
metadata; the consumer supplies it explicitly. Metal/DX12 acceptance remains open.
The [contract's verification list](../local-read.md#verification-still-needed)
records those follow-ups. Next planned implementation is tessellation/geometry.

Reproduce with the pinned tools and local Vulkan/validation environment:

```sh
cargo test --offline --no-default-features -p ogpu
cargo clippy --offline --no-default-features -p ogpu --all-targets -- -D warnings
cargo xtask abi
python3 tools/check-foundation-surface.py
cargo xtask bindings --check
cargo xtask foundation-shaders --check
cargo xtask foundation-compiler --check
cargo xtask foundation-graphics-compiler --check
cargo test --offline --no-default-features -p ogpu --lib -- --ignored --test-threads=1 --nocapture
cargo xtask foundation
```

## Clean installed acceptance

Installed from the clean implementation commit above (`source_dirty=false`) into
`/tmp/ogpu-m8-64e0988-sdk`. The release runtime SHA-256 is
`bf210310468cc24c5ec5f92a00f827de75624b40563f2eaed78f7eac8a39ffcb`.
Relocated public C consumers, pinned GGML (including transformer/quantized and
full-dataset precision modes), pinned libplacebo, and isolated native Rust/Wayland
checks pass. The new local-read C fixture also passes against that installed
public header and optimized library. Installed surface audit passes. Human
interaction and native features absent on this GPU are not newly accepted.

Exact log paths and hashes are in the [machine-readable receipt](m8-local-read-2026-11-06.json).
The follow-up documentation commit changes no runtime or artifact code.
