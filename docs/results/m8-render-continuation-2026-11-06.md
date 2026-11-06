# M8 native render-pass continuation

2026-11-06. The [continuation contract](../render-continuation.md) adds suspend and
resume flags to existing C/Rust render descriptions. Record layouts and foundation
revision 11 are unchanged. One native render flag word carries the choice; no pass
object, resource retention, hidden synchronization or additional submit is added.

## Acceptance

Linux x86-64, AMD Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan loader 1.4.363,
validation layers 1.4.357 with synchronization validation, Rust 1.98.1.

The public C depth fixture adds modes 20–25:

- One list with suspend/resume between two draws.
- Two primary lists in one submit entry, with suspension/resumption at their boundary.
- Three primary lists, with an empty resumed-and-suspended middle segment.
- Each pattern repeated with four-sample depth, SAMPLE_ZERO resolve and source DISCARD.

All six cases pass two changed-root replays: 12 submissions, 768 checked D32 output
values, unchanged input bytes and host guards. A CLEAR attachment is unchanged
across segments; the left half survives resume while the right half is drawn later.
The resolve cases preserve early sample values until the final resolve. Unknown
render flag bit 8 rejects with UNSUPPORTED before native recording and poisons the
encoder. All existing C fixture cases also pass with no validation errors.

Host library tests (89), native Rust correspondence tests (2), strict core Clippy
and the surface audit pass. The audit covers 152 operations, 131 records and 164
constants (1,211 values), including all three C/Rust render flags. No new operation
or layout migration is involved.

```sh
cargo test --offline --no-default-features -p ogpu
cargo clippy --offline --no-default-features -p ogpu --all-targets -- -D warnings
python3 tools/check-foundation-surface.py
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1 cargo xtask foundation
```

Use the pinned toolchain and local loader/ICD/layer paths. Logs are recorded in the
adjacent JSON. This is native correctness/structural-lowering evidence, not measured
performance parity. Cross-list parameter matching and same-batch legality remain
caller proof; no runtime rejection guarantee is inferred for malformed chains.
Color/stencil, secondary composition, multiview, shading-rate and other hardware/
backend combinations remain unverified. Tile-local/input-attachment implementation
is next; M8 as a whole remains open.
