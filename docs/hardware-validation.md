# Modern hardware validation

Updated 2026-09-13, runtime `a5a609d` (still ABI 3), after the
[independent-heap checkpoint](descriptor-heaps.md). Modern compute, graphics,
image heaps and preservation now pass on the RX 5700 XT. Unified image layouts
are optional; the GENERAL-only command path is unchanged. Remote execution CI
is not provisioned or verified by these local runs.

## Correction to the initial audit

At `d93691f`, compute passed but graphics creation rejected this card because our
runtime required `VK_KHR_unified_image_layouts`. That driver capability report was
correct; treating it as necessary for our current image operations was not.
`GENERAL` is already legal for the single-sample RGBA8 attachment, sampled/storage
and copy operations we use. The extension adds a layout-efficiency guarantee, not
the foundation of this execution model. See the [Vulkan layout rules](https://docs.vulkan.org/refpages/latest/refpages/source/VkImageLayout.html)
and [extension rationale](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_unified_image_layouts.html).

`a5a609d` enables the extension only if its feature is supported, without adding
layout tracking, alternate commands or legacy objects. ABI, shaders and the public
image/lifetime model are unchanged. We use neither attachment feedback loops nor
video layouts; future features must be audited separately. No layout-performance
equivalence is promised when the extension is unavailable.

## Device and isolation

The sandbox exposes no `/dev/dri`, but the host has an accessible RX 5700 XT.
The approved out-of-sandbox audit identified AMD Radeon RX 5700 XT (RADV NAVI10),
Mesa 26.2.1, Vulkan 1.4.354. Loader, validation layers and SPIRV-Tools were 1.4.357.0;
Vulkan and synchronization validation were enabled. The RADV ICD was explicitly
selected with `VK_DRIVER_FILES`, excluding llvmpipe from every hardware run.

The device reports buffer device addresses, timeline semaphores, synchronization2,
maintenance5, descriptor heaps, untyped pointers and device-address commands.
`maxPushDataSize` is 256. It also reports dynamic rendering, but does **not** report
`unifiedImageLayouts`. It now meets both compute and graphics requirements because
unified layouts are optional. A Vulkan version number alone still does not establish
the other required features. On llvmpipe, the optional extension is available and enabled.

| Gate | RX 5700 XT / RADV result | Coverage limit |
|---|---|---|
| Compute execution | PASS: round trip, two-kernel batch, retirement, reduction, matmul | One discrete GPU and driver; no portability/performance claim |
| Vulkan tests | PASS: all twelve, with no graphics/image exclusions | Includes optional feature enablement, preservation and heap failures/lifetimes |
| GGML integration | PASS: lifecycle checks and all six direct/scheduled cases | Existing bounded FP32 MNIST consumer only |
| Graphics and image heaps | PASS: triangle, image-loop and heap-image; LOAD/CLEAR and preservation tests | Bounded RGBA8 profile; no layout-efficiency or general performance claim |
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
cargo xtask graphics
cargo xtask image-loop
cargo xtask heap-image
cargo xtask gpu-tests
bash integrations/ggml/run.sh /tmp/ogpu-ggml-consumer
```

Paths are local examples, not portable installation defaults. Use the correct
driver manifest and pinned GGML checkout on another host. `cargo xtask gpu-tests`
now runs the full suite on this device; no test filtering is needed. The earlier
seven-test filtered run remains historical evidence, not the current command.
Execution xtasks scan for Vulkan validation errors as well as process failures.

The round trip checked 4,099 integers over three dispatches, partial writes and
parent-handle destruction. The batch example checked two kernels without intermediate
readback. Both retirement modes passed twelve jobs on three slots with nine safe
reuses. Reduction checked 1,048,579 values across three levels; its Rust test also
checked 68 cases and every intermediate level/guard. Matmul checked 50 shapes/patterns
per kernel in timed and untimed modes. Timings were not a performance acceptance
gate; the host also ran GGML during this validation session.

The original seven compute-side tests passed at `d93691f`. After the optional-layout
change, all twelve Vulkan tests pass independently on RADV and llvmpipe. The added
test inspects logical-device extension names and feature chains, hides the optional
feature on a supporting driver, and checks successful graphics creation and image
initialization. Missing dynamic rendering still rejects graphics before creation.
The full workload covers absent extension support on RADV and enabled support on
llvmpipe. GPU failure tests inject API results around actual resources; this is not
evidence of real device-loss recovery under every driver failure.

All six GGML cases checked 10,000 images, 9,801 correct predictions, identical CPU
top-1 predictions and maximum logit error `3.43322754e-05`. Scheduled cases retained
three intermediate aliases. Lifecycle and intentional misuse checks passed.
The original local log is `target/ggml-integration/acceptance.lNDlLN2T.log`;
the rerun after `a5a609d` is `target/ggml-integration/acceptance.u1GzRSXa.log`
(neither is committed).

Graphics, image-loop and all six four-variant heap-image cases now pass separately
with only RADV and only llvmpipe selected. Checks include processed/final pixels,
guards, sampler/index variation, retained mutation rejection and early handle release.
Vulkan tests verify LOAD/CLEAR, preserved cross-submission contents and rejected or
abandoned discard. All 22 ordinary tests, 700 ABI checks, mock tests, generated binding
reproduction, formatting and warning-free Clippy also pass. The diagnostics added at
`7e3b9b0` remain useful for identifying selected and rejected devices.

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

The current card/driver now passes the local full-profile gate. The next external
decision is whether to use this host and authorize runner registration. No runner
service was installed, credentials created, commits pushed or workflow dispatched.
Keep execution manually triggered on trusted revisions; runner registration and
host administration are separate from editing this repository. Broader hardware and
driver coverage remains future evidence, not a prerequisite for using this host.
