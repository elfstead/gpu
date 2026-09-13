# Modern hardware validation

2026-09-13, runtime `d93691f` (ABI 3), after the
[independent-heap checkpoint](descriptor-heaps.md). This run validates the modern
compute backend on physical hardware; it does not complete the graphics/heap or
remote execution-CI gates. No runtime changes or legacy fallbacks were needed.

## Device and isolation

The sandbox exposes no `/dev/dri`, but the host has an accessible RX 5700 XT.
The approved out-of-sandbox audit identified AMD Radeon RX 5700 XT (RADV NAVI10),
Mesa 26.2.1, Vulkan 1.4.354. Loader, validation layers and SPIRV-Tools were 1.4.357.0;
Vulkan and synchronization validation were enabled. The RADV ICD was explicitly
selected with `VK_DRIVER_FILES`, excluding llvmpipe from every hardware run.

The device reports buffer device addresses, timeline semaphores, synchronization2,
maintenance5, descriptor heaps, untyped pointers and device-address commands.
`maxPushDataSize` is 256. It also reports dynamic rendering, but does **not** report
`unifiedImageLayouts`. Thus it meets our compute baseline, not the optional graphics
profile. A Vulkan version number alone does not establish that full profile.

| Gate | RX 5700 XT / RADV result | Coverage limit |
|---|---|---|
| Compute execution | PASS: round trip, two-kernel batch, retirement, reduction, matmul | One discrete GPU and driver; no portability/performance claim |
| Compute Vulkan tests | PASS: seven tests, including injected failures, gated reuse, timing and timeline limits | Four graphics/image tests explicitly excluded, not passed |
| GGML integration | PASS: lifecycle checks and all six direct/scheduled cases | Existing bounded FP32 MNIST consumer only |
| Graphics and image heaps | UNSUPPORTED: missing unified image layouts | Existing llvmpipe results remain the execution evidence |
| Remote execution CI | Not run or provisioned by this work | Needs a selected runner host and registration |

## Results and reproduction

Set up the build tools and loader/layers as described in [development](development.md).
For this host, the hardware-selection and validation settings were:

```sh
export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/radeon_icd.x86_64.json
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
export VK_LAYER_VALIDATE_SYNC=1
cargo xtask baseline
cargo xtask compute
cargo xtask batch
cargo xtask retirement
cargo xtask reduction
cargo xtask matmul
cargo test --locked -p ogpu -- --ignored --nocapture \
  --skip gpu_graphics --skip gpu_image_preservation --skip gpu_heaps
bash integrations/ggml/run.sh /tmp/ogpu-ggml-consumer
```

Paths are local examples, not portable installation defaults. Use the correct
driver manifest and pinned GGML checkout on another host. The ordinary
`cargo xtask gpu-tests` still runs the full suite and is expected to fail when only
this compute-capable device is exposed. Do not report the filtered command as an
eleven-test pass. Inspect Vulkan diagnostics as well as test exit status; the
direct Cargo invocation does not run xtask's validation-error scanner.

The round trip checked 4,099 integers over three dispatches, partial writes and
parent-handle destruction. The batch example checked two kernels without intermediate
readback. Both retirement modes passed twelve jobs on three slots with nine safe
reuses. Reduction checked 1,048,579 values across three levels; its Rust test also
checked 68 cases and every intermediate level/guard. Matmul checked 50 shapes/patterns
per kernel in timed and untimed modes. Timings were not a performance acceptance
gate; the host also ran GGML during this validation session.

Seven Vulkan tests passed without reported validation errors: round trip, batches,
batch failures, timeline boundaries, retirement, timing and reduction. GPU failure
tests inject API results around actual allocations/submissions; this is not evidence
of real device-loss recovery under every driver failure.

All six GGML cases checked 10,000 images, 9,801 correct predictions, identical CPU
top-1 predictions and maximum logit error `3.43322754e-05`. Scheduled cases retained
three intermediate aliases. Lifecycle and intentional misuse checks passed.
The local acceptance log is `target/ggml-integration/acceptance.lNDlLN2T.log`
(not committed).

With RADV as the only ICD, `cargo xtask graphics` exits unsuccessfully because no
graphics device qualifies. At `7e3b9b0`, the examples now print the runtime's unsupported-feature
diagnostic rather than silently discarding it; heap-image also prints its selected
device name. Graphics, image-loop and all six four-variant heap-image cases were
rerun successfully with only llvmpipe selected after those diagnostic changes.

## Remaining deployment gate

The full manual [execution workflow](../.github/workflows/gpu.yml) needs a trusted
Linux x86-64 runner with the `ogpu-modern-vulkan` label, the build/validation tools
listed in that workflow, and an explicitly selected device satisfying **both**
compute and graphics requirements. A successful `cargo xtask baseline` alone is
insufficient: it succeeds if any device meets the compute feature set. Confirm its
per-device graphics result and actual graphics/heap execution as well.

Before claiming physical coverage, exclude software ICDs and record the selected
GPU/driver and source revision. Otherwise compute can execute on RADV while graphics
executes on llvmpipe in the same apparently successful run. Such a split run is useful
but does not validate physical image heaps or LOAD/preservation.

The current card/driver cannot fill that full-profile hardware role. The next external
decision is which host/device to use and authorize for runner registration. No runner
service was installed, credentials created, commits pushed or workflow dispatched.
Keep execution manually triggered on trusted revisions; runner registration and
host administration are separate from editing this repository. A compute-only lane
on this host is possible, but would need an explicitly separate coverage scope and
would not close the full-profile gate.
