# Foundation indexed/depth scene

The matched scene now has a public-only `ogpu_next.h` caller alongside the
unchanged direct Vulkan reference and historical installed-API caller. Select it
with `python3 examples/graphics_scene/run.py --foundation` (`--public` is implied).
`--check` compiles and runs host oracle/runner checks without using a GPU;
`--scale` includes 1280x720. Shader compilation needs the existing pinned Slang
toolchain and SPIRV-Tools. Execution needs one selected ICD and synchronization
validation, just as the original control does.

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

This ports the fresh-recording scene, not its separate range/frontier/reuse and
timing controls or generated installed-SDK handoff. Those still retain ABI20
dependencies. Next migrate the generated interface and useful replay/application
workflows, then export the scene through the foundation SDK and remove their
last legacy dependencies. No native Metal or second physical GPU result is claimed.
