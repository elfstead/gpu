# Independent foundation indexed/depth scene

Copy this directory outside the OGPU checkout. It needs a C11 compiler, Python
3.11+, pkg-config and the matching Linux foundation preview. Embedded generated
shader artifacts remove any build-time Slang, Vulkan-header or Rust dependency.

```sh
export PKG_CONFIG_PATH=/path/to/preview/lib/pkgconfig
python3 build.py
export VK_DRIVER_FILES=/path/to/one/icd.json
python3 run.py
```

The runner checks 30 A/B/A frames for each of UINT32 and UINT16: GPU-generated
geometry/indirect arguments, depth/order/clear/load/empty-draw modes, independent
analytic color/depth results, guards, repeatability and exact cross-width images.
Default extent is 257x193; `--scale` additionally runs 1280x720. There is no CPU
readback between geometry generation and drawing. Memory placement, views,
transitions, recording storage and timeline completion are application-owned.

The build produces `consumer` (fresh recording) and `reuse` (reset/replay).
After accepting each width's fresh outputs, the runner checks eight A/B/empty/A
frames for every combination of four render modes, one/two slots and reset/replay.
That adds 32 cases / 256 frames by default, compared byte-for-byte with the freshly
accepted images, geometry and guards. Replay records once per slot; reset records
after completion. Mapped control-pointer arguments change between submissions.
Each slot owns its memory, arena and timeline; no CPU readback precedes drawing.

Fresh `run-*` directories retain outputs, logs, source/binary/runtime hashes and
complete/incomplete receipts, including failures. Validation layers must be
provided and enabled by the caller; these scripts do not install them. Match the
header, library and generated artifacts from one source revision; revision 1 is
mutable, not a stable ABI promise. The relocation check verifies the loaded
runtime and public-only compiler dependencies, including missing-loader failure.

This standalone package does not include the native Vulkan reference or timing
controls. In-tree acceptance separately compares the exact embedded shader bytes
and every output byte against direct Vulkan. No performance, general graphics,
native Metal parity or independent outside adoption claim follows.
