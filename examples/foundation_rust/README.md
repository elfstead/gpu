# Native Rust foundation consumer

This standalone Cargo package uses `ogpu::foundation::rust` directly, below the C
ABI. It needs the same modern Linux/Vulkan baseline as the foundation SDK, but no
C compiler, C bindings, shader compiler or window system. Compute uses the existing
checked-in `examples/shaders/foundation-compute.comp.spv` artifact (embedded at
build time), whose source is `foundation-compute.slang` in the same directory.
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
Additional A/B/A phases cover prepared compute with specialization, two address
roots, partial inline updates, direct/indirect dispatch and guarded CPU-reference
output checks. Cache export/import/merge, dedicated image placement, views,
descriptor/sampler encoding, explicit transitions and clear/readback are exercised
through native Rust, including malformed executable/view/descriptor requests.

Handles are non-owning, pointer-sized values. Copying/dropping them neither retains
nor frees anything; destruction is explicit and unsafe. The consumer performs
ordered cleanup only after successful completion. On error or uncertain completion
it exits without freeing potentially pending resources; this is a small executable,
not a general application recovery/cleanup framework.

The native surface now has equivalents for every currently exported foundation C
operation, including graphics, queries, split dependencies and secondary lists.
An integration test compile-checks that correspondence against the C header and
guards against routing native operations through C entry points. This is operation
coverage, not a claim that this consumer exercises every operation: raster, query,
secondary and split execution still rely on the existing shared-backend/C fixtures.
Some descriptions use versioned records and raw embedded pointers from
`foundation::types`; ergonomic typed descriptions and generated Rust shader layouts
remain possible follow-ups. This example explicitly asserts its known root layout.
No fully safe wrapper, extra allocation/retention layer or implicit scheduling is
promised. It is not yet packaged by the C SDK installer.
