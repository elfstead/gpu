# Foundation indexed/depth scene

The matched scene now has a public-only `ogpu_next.h` caller alongside the
unchanged direct Vulkan reference and historical installed-API caller. Select it
with `python3 examples/graphics_scene/run.py --foundation` (`--public` is implied).
`--check` compiles and runs host oracle/runner checks without using a GPU;
`--scale` includes 1280x720. Shader compilation needs the existing pinned Slang
toolchain and SPIRV-Tools. Execution needs one selected ICD and synchronization
validation, just as the original control does.

`--generated` implies `--foundation` and selects compiler-generated typed roots,
capability checks, executable requirements and the linked graphics argument bank.
`generated.py --api foundation --check` reproduces the committed headers. The
acceptance runner rejects stale generated files and requires their embedded SPIR-V
to match the native control byte-for-byte. Independent runs use separate compiler
output directories.

## Preserved workload and acceptance

Compute generates vertices, UINT32 or UINT16 indices, and two native indexed
indirect records. The same native SPIR-V bytes run in both consumers. There is
no CPU geometry/count readback between compute and drawing, nor shader rewriting
or runtime reflection. The runner checks reflected roots, workgroup size, native
capabilities, absence of shared globals, and vertex/fragment linkage before
accepting the foundation caller's executable metadata.

All ten depth/order/clear/load/empty-draw modes retain the original A/B/A inputs,
guard bytes, exact generated geometry and independent analytic pixel/depth oracle.
Every output byte, including edges excluded from the analytic oracle, must match
the same-driver native control. The runner identifies foundation explicitly and
checks its imported OGPU symbols; ABI20 results cannot masquerade as foundation.
This is correctness/migration coverage, not throughput or complete M4 acceptance.

For UINT16, first obtain a **clean committed** matched UINT32 report, then run:

```sh
python3 examples/graphics_scene/run.py --foundation --index16 \
    --reference32 target/graphics-scene/matched-.../report.json
```

Use `--scale` on both commands for the larger extent. The second run independently
checks UINT16 geometry and requires its color/depth bytes to match the hash-checked
UINT32 outputs on the same driver. There is no weaker UINT16 acceptance mode.

## Explicit ownership and assumptions

- One combined graphics/compute queue, one reusable arena slot, one timeline;
  frames record afresh, submit, wait and reset before CPU readback/reuse.
- Separate linear allocations for geometry, index/indirect data and mapped
  staging/readback. Usage and memory types are application-selected. This caller
  requires LOCAL geometry/image memory and HOST_VISIBLE staging, preferring
  non-host-visible LOCAL types when eligible; UMA remains eligible.
- Dedicated image backing, views, discard/layout transitions, compute-to-draw,
  split-render, transfer and host dependencies, and scratch storage are explicit.
  No optional unified-image-layout feature is required by this caller.
- Resources remain alive until proven completion and destruction of recordings;
  recording/submission rejection cleans up. Backend submission uncertainty or a
  failed wait exits without freeing potentially pending operands. Test-only link
  wrappers exercise each case after a genuinely completed upload; they never
  fabricate successful completion. Host tests reject false-success receipts and
  incorrect teardown counts/order.
- Bounded dimensions, 32-entry discovery arrays, fixed RGBA8/D32 formats, one
  sample/layer/mip and trusted shader addresses are this **consumer's** limits.
  Unsupported setup fails; no alternate format, queue or backend is substituted.

## Migration boundary

The opt-in preview now exports `share/ogpu/examples/foundation-indexed-scene`,
including typed shader artifacts and the existing analytic/cross-width oracle.
Copy it outside the checkout, run `python3 build.py`, then `python3 run.py`.
Only C11, Python and pkg-config plus the installed preview are needed; no Rust,
Slang, Vulkan headers or native-reference source is required by that consumer.
The default handoff checks both widths at 257x193, followed by 32 one/two-slot
reset/replay cases; `--scale` adds 1280x720. Fresh
complete/incomplete receipts record source/executable/runtime identity. Relocation
checks exercise public-only header dependencies, actual loaded-library selection
and incomplete missing-loader reports. The shared acceptance code takes an explicit
foundation profile; it cannot accept an installed-ABI log as foundation evidence.

This ports the fresh-recording scene and its generated handoff. The subsequent
[reset/replay caller](foundation-scene-reuse.md) also migrates one/two-slot reuse;
its generated control-pointer interfaces and independent handoff are included.
The [fresh range/count/identity caller](foundation-indexed-ranges.md) is also
migrated, including slot reset/replay, per-draw scopes and explicit argument
bind-once/resupply, call-time snapshots and partial-word updates. Generated range
interfaces/SDK packaging and historical control/timing retirement remain.
Retire redundant historical controls at their last dependency,
then finish default SDK cutover. No native Metal or second physical GPU result is claimed.

## Local checkpoint — 2026-10-12

Radeon/RADV 26.2.4 passes raw UINT32 and generated UINT16 native/cross-width
acceptance at 257x193, 640x360 and 1280x720; generated UINT32 additionally matches
native at the first two extents. llvmpipe 26.2.1 passes raw UINT32 and generated
UINT16 native/cross-width acceptance at 257x193 and 640x360. Each extent/backend
checks all 30 frames, and both drivers pass all four failure controls. The clean
UINT32 reference revision is `72b4018`; generated runs retain their development
source hashes. There are no accepted validation errors or output mismatches.

Independent SDK relocation passes both index widths at 257x193 on both drivers,
alongside the other three preview consumers. A separate no-GPU run passes build,
dependency, loader-failure and incomplete-receipt checks. Both installed and
foundation generated headers reproduce, and strict C11 plus host oracle/profile/
failure/installer tests pass. These are local checks, not external adoption or
performance evidence.
