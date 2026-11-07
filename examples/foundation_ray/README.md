# Explicit ray consumer

Uses only OGPU's public native Rust interface. The checked-in Slang/SPIR-V fixture
consumes a native acceleration-structure address via ordinary argument bytes.

From the repository, with the normal toolchain/loader environment:

```
cargo test --offline --no-default-features -p ogpu --test ray -- --ignored --nocapture
```

This test requires an advertised AS/query profile and fails if unavailable. A
standalone build of `main.rs` exits 77 for unsupported. For an explicitly labelled
RADV emulation diagnostic, prefix the command with `RADV_EXPERIMENTAL=emulate_rt`.
The program never enables emulation itself. Run with Vulkan synchronization
validation enabled. Passing under emulation is correctness evidence only, not
hardware traversal/performance acceptance.

The fixture uses a bounded 1 MiB caller arena and checks native requirements before
using its fixed placements. It exercises build/update, clone, compact, serialized
round-trip, native version compatibility, property queries and five invalid
recordings. Every submitted list is explicitly waited before host reads or reuse.
On an uncertain submission/wait failure, it exits without freeing pending objects.

The small source-level AS conversion intrinsic works around an unnecessary
ray-pipeline extension emitted by Slang 2026.14.1's built-in constructor. It performs
no shader rewriting and does not add a runtime feature dependency. Reproduce the
artifact with `cargo xtask foundation-shaders --check`.
