# Native Rust foundation consumer

This standalone Cargo package uses `ogpu::foundation::rust` directly, below the C
ABI. It needs the same modern Linux/Vulkan baseline as the foundation SDK, but no
C compiler, C bindings, shader compiler or window system for this transfer example.
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

Handles are non-owning, pointer-sized values. Copying/dropping them neither retains
nor frees anything; destruction is explicit and unsafe. The consumer performs
ordered cleanup only after successful completion. On error or uncertain completion
it exits without freeing potentially pending resources; this is a small executable,
not a general application recovery/cleanup framework.

This is the first extracted native surface: discovery, device, memory, timeline,
arena and transfer/submission operations. Some descriptions still use versioned
records and raw embedded pointers from `foundation::types`; shader, image,
descriptor and broader command extraction is not complete. No fully safe wrapper,
extra allocation/retention layer or implicit scheduling is promised.
