# Execution capability contract

Implemented 2026-09-14, **ABI 8**, following the two-consumer D1/D3/D4 review.
This closes the capability-reporting and compute-image coupling issues. It does
not stabilize the API or add general feature negotiation, formats or scheduling.

## Supported, enabled, required

| Question | Source | What it does not establish |
|---|---|---|
| What does the physical device support? | `ogpu_probe_device_info` | Features enabled on a logical execution device |
| What does this execution device enable? | `ogpu_device_capabilities` | All requirements of an arbitrary shader or image |
| What are its execution ceilings? | `ogpu_device_limits` | Validity of specialized code or every image combination |
| Is this exact image description supported? | `ogpu_image_check_support` | Available memory or infallible creation |
| Is this executable compatible? | Caller checks code/root/stage interfaces, specialized local/shared sizes and enabled features | Runtime reflection, a shader package format or sandboxing |

The enabled query reuses `OgpuCapabilities`, with the same 0/1 fields as discovery.
Compute, addresses, timeline, synchronization2, heaps, address commands and untyped
pointers are enabled by the fixed [modern baseline](modern-baseline.md).
`graphics_queue` is 1 only for graphics creation: it means graphics execution is
enabled, not merely that the physical queue has a graphics flag. ABI 10 also enables
`storage_buffer_16bit_access`, already mandatory for physical Vulkan 1.4 support.
The subsequent [numerical executable experiment](ml-executable-requirements.md)
also enables optional `shader_float16` when supported, without strengthening the
baseline or changing C layouts. Other numeric, narrow-storage and matrix fields
remain 0 on Vulkan. See the [storage/arithmetic distinction](ggml-mixed-precision.md).
The existing fields are not an exhaustive Vulkan feature list;
the baseline document and header define additional fixed requirements.

Enabled capabilities and limits are cached, externally serialized queries. Their
outputs are unchanged on error and remain readable after device loss. Neither
query enables features or submits work. Future optional executable profiles remain
a design decision, not an implicit promise from discovery bits.

## Compute images are not rasterization

Ordinary `ogpu_device_create` enables compute, images and independent image/sampler
heaps. It does **not** enable rasterization, including when the selected queue also
supports graphics. `ogpu_device_create_graphics` additionally requires/enables
dynamic rendering and a shared graphics/compute queue. Color images, raster
creation and graphics access masks reject on an ordinary device.

This deliberate behavior change increments the ABI, even though no existing
structure layout or function signature changes. Callers that depended on
opportunistic rasterization must choose graphics creation explicitly. ABI 1–7
callers fail the version gate; rebuild with matching header/library. No old
behavior alias, second image implementation or compatibility fallback remains.
Production queue selection is unchanged; this is not a performance policy change.

Non-color images use the same format/storage, GENERAL-layout, native-heap,
dependency and ownership rules in either profile. Unified image layouts can be
enabled on either profile when available; both use identical commands without
the extension. Removing the graphics prerequisite does not relax initialization,
image usage, heap mutation or shader-address lifetime obligations.

## Exact image checks

`ogpu_image_check_support(device, desc, error)` and image creation share one
preflight implementation: description validation, required enabled operations,
format/dimension/usage support, linear sampling and exact extent/sample checks.
The query creates no image, GPU allocation or descriptor and submits no GPU work.
Creation repeats the check, then attempts allocation normally.

- `SUCCESS`: the exact description passes support checks; allocation can still fail.
- `INVALID_ARGUMENT`: malformed description, unsupported enum values, invalid
  usage combinations or the existing global dimension ceilings.
- `UNSUPPORTED`: unavailable raster capability, format/usage combination, filtering,
  or the format-specific extent/sample support.
- Driver errors remain errors, not a false unsupported answer. Device loss rejects
  this query, unlike the two cached device queries.

`SAMPLED` still promises nearest **and** linear filtering. Query and creation both
reject sampled formats lacking linear support. Splitting sampleability from
filterability may expose a better API alternative later; this checkpoint does not
silently change that promise or add a new format catalog.

The libplacebo adapter now checks enabled compute/raster/heaps and its bounded
RGBA8/R32F image combinations before advertising formats (extended at ABI 11 to
RGBA16F and sampled/transfer RGBA16 UNORM for the HDR consumer). Small startup descriptions
establish combination support, not support for all extents; actual creation checks
each image's exact extent and usages. Its `rg32f` format is a host vertex-record
layout, not a newly supported OGPU image. The adapter remains deliberately bounded
and synchronous, with unchanged processing math and compiler lowering.

## Verification

27 ordinary tests, 19 GPU tests on each of RX 5700 XT/RADV and llvmpipe, Clippy,
pinned binding reproduction, mock tests and 745 C/Rust layout values pass.
Vulkan and synchronization validation were enabled for execution.

The C-boundary sampling test runs 1D/2D R32F nearest/linear sampling and storage,
A/B/A uploads and readback through both device constructors. Tests check enabled
flags, invalid/query/create agreement and raster rejection on a compute-created
device. Injected missing dynamic rendering, missing linear support, unsupported
formats, extent/sample restrictions and driver/allocation failures test the
boundaries. Cached capabilities survive simulated device loss; image queries do not.

A test-only restriction selects a real compute-only queue when available without
changing its Vulkan family index. Image discard and native image/sampler-heap
bindings submit successfully on RADV family 1 with dynamic rendering disabled.
llvmpipe has no dedicated compute family and uses its shared family 0. Full sampling
execution uses ordinary public device creation on each driver; these are distinct
checks, not a claim of dedicated-queue performance or device-wide compute-only hardware.

The libplacebo reference/consumer rerun compares 286,488 intermediate/final bytes
per driver with **zero differences**, nine compute and nine raster passes, and
zero live child objects at teardown. Rejection/specialization checks also pass.
Ignored artifacts under `target/libplacebo-integration/`:

- llvmpipe: `reference.tvNCiBbA` / `ogpu.iBa7fh6u`, `capabilities-llvmpipe.log`.
- RADV: `reference.tXMGrsrY` / `ogpu.RbBwEjs1`, `capabilities-radv.log`.

All eight C execution examples pass on both drivers. GGML passes all 24 acceptance
cases (six direct/scheduled cases × two placements × two drivers), including
lifecycle/rejection checks. Each case processes 10,000 images, matches CPU top-1
predictions (9,801 correct) and has maximum logit error 0.0000343322754.
Ignored logs under `target/ggml-integration/`:

- llvmpipe HOST/DEVICE: `acceptance.H1agoi2k.log` / `acceptance.ryZ6ig0j.log`.
- RADV HOST/DEVICE: `acceptance.EtyCYZY3.log` / `acceptance.hVuiAoji.log`.

These are local correctness regressions, not new performance measurements or
remote-CI results. No shader binaries or pinned upstream revisions changed.

## Subsequent D4 review

The [completion-resource follow-up](completion-resource-review.md#implementation-abi-9)
now implements the result/timing receipt split at ABI 9. It retires submission
resources on safe terminal observation, with no background collector, mutable
in-flight heaps, asynchronous adapter scheduling or generalized runtime allocator.
