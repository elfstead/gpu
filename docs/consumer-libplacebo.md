# Second consumer: libplacebo image processing

Approved 2026-09-13; updated 2026-09-14. Status: G0/G1 and bounded G2 execution pass
on RADV and llvmpipe. Upstream-generated compute/raster work now executes through
OGPU; intermediate and final images match the independent reference exactly.
The later [two-frame follow-up](libplacebo-inflight.md) also passes: bounded
consumer-managed asynchronous execution, unchanged runtime ABI 10. The G0–G2
sections below record the original synchronous checkpoint.
Pinned upstream: `3330a515d62139259c26239014f286e233bd3a5c` (2026-09-03),
[official mirror](https://github.com/haasn/libplacebo).

## Purpose and scope

Exercise useful upstream image-processing logic through mixed compute and raster
execution. Compare against libplacebo's existing Vulkan backend, not a rewritten
approximation of its algorithms. Candidate workflow: polar image scaling (upstream
compute path) followed by a fragment sampling/output pass, entirely offscreen.
Keep upstream shader generation, dispatch decisions and processing math wherever
possible; a backend adapter may translate resource declarations and execution calls.

No decoding, media player, presentation, external-memory import, HDR pipeline,
general libplacebo backend or upstream contribution. No legacy Vulkan descriptor-set
path in OGPU. The Vulkan reference is independent evidence, not an OGPU fallback.
libplacebo is LGPL-2.1-or-later; build it as a separate dependency with its notices.
The repository's MIT license does not relicense that library or generated shader
logic. Do not vendor generated upstream shader bodies as MIT-only fixtures.

## Decisions this can inform

- D1: generated shader/resource contracts versus our fixed root/entry-point/grid
  forms. Native-heap lowering should preserve the upstream arithmetic, not replace
  it with another hand-written example.
- D3: image formats, sampling, upload and render-target operations for a real
  processing pipeline. Evaluate better API alternatives, not only forced changes.
- D4: repeated-frame resource reuse, temporary-image ownership and synchronization.

Source inspection already identifies a `pl_gpu_fns` backend boundary and public
shader-generation/dispatch interfaces. The backend boundary is internal and pinned,
as with GGML; it is not a promise of upstream ABI stability. Generated GLSL uses
bound sampled/storage resources; polar compute uses two-dimensional workgroups and
a floating-point filter lookup texture. These are concrete questions for G0, not
yet an approved list of general API additions.

## Sequence and stopping conditions

1. **G0 — preserve upstream logic.** Build the pinned library and run/capture one
   real compute-plus-fragment reference pipeline. Inventory its generated shaders,
   bindings, grid, formats and constants. Test a native-heap compiler route. Stop
   and reassess if this entails replacing the processing algorithms or adding a
   broad legacy compatibility layer. Report reference-only evidence as such.
2. **G1 — bounded adapter and contract choices.** Use G0's exact requirements to
   choose the smallest coherent API improvements and integration boundary. Keep
   unsupported operations explicit; do not expand to all libplacebo features.
3. **G2 — OGPU execution acceptance.** Run identical upstream processing and inputs
   through the public OGPU API and the reference. Exercise several odd/even image
   extents, deterministic nonconstant input, repeated frame updates, intermediate
   reuse and teardown. Check both intermediate and final images, actual compute
   and raster execution (no CPU or reference fallback), and synchronization
   validation on RADV and llvmpipe. Declare numerical tolerance before OGPU results.

G0 is not completion of G2. Reference rendering, shader compilation and OGPU
execution are separate gates. No performance target or stability claim.

## G0 evidence — 2026-09-13

The pinned upstream shared library builds without source changes. The
[reference harness](../integrations/libplacebo/README.md) preserves upstream
`pl_shader_sample_polar`, its EWA Lanczos filter/LUT generation, shader dispatch and
Vulkan execution. A nearest-sampled fragment pass consumes the compute output.

Both RX 5700 XT/RADV and llvmpipe pass three input extents (`16x16`, `31x17`,
`64x33`) with three A/B/A frame updates each. Every frame runs one compute and one
raster operation. Intermediate/final images match exactly; alpha is opaque; A
repeats exactly and B changes the output. Only two passes compile per extent;
subsequent frames reuse them. No validation errors were reported with Vulkan and
synchronization validation requested. No window, decoder or CPU processing fallback.

Local artifacts, all ignored:

- llvmpipe: `target/libplacebo-integration/reference.S5Jv5nEz`.
- RADV: `target/libplacebo-integration/reference.tryGFcWP`.
- Compiler gate on llvmpipe captures: `target/libplacebo-integration/compiler.eXC3zpnE`.
- Compiler gate on RADV captures: `target/libplacebo-integration/compiler.umTtYxmI`.

Cross-driver comparison: 143,244 output bytes, 194 differing, maximum channel-byte
difference 1. Checksums are diagnostic, not portable golden values. For G2, declare
an initial maximum absolute RGB difference of **2/255**, alpha exactly 255, for
both intermediate and final images against upstream on the same driver. This is
declared before any OGPU result; failures must be investigated, not hidden by
silently widening tolerance. Preserve exact A/B/A determinism and exact nearest
intermediate-to-final copying within each implementation.

The standalone compiler probe maps known sampled/storage declaration lines to
`GL_EXT_descriptor_heap` arrays, constructing sampled images from separate image
and sampler entries. No processing-body rewrite or descriptor-set backend was
needed. All nine shaders from each driver capture compile under shaderc 2026.1 /
glslang 16.4.0 and validate under SPIRV-Tools 1.4.357.0. The resulting SPIR-V uses
native heaps and contains no descriptor-set/binding decorations. Negative-input
and body-preservation tests pass. This is compiler evidence only: constants,
vertex data and resource binding must still be handled for execution.

## Concrete G1 inventory

| Upstream requirement observed | Current OGPU fit / next decision |
|---|---|
| 32×32 local workgroup; 2D dispatch grid, tail groups | Implemented in ABI 5: explicit X/Y/Z workgroup counts with per-axis bounds checks; unused axes are 1. Local size remains shader-owned, with no builtin-ID remapping. |
| Sampled RGBA8 input, storage RGBA8 intermediate, RGBA8 fragment target | Implemented in ABI 6: explicit image usages and retained buffer-to-image uploads; rendering remains 2D RGBA8. |
| 256-entry R32F, linearly sampled 1D filter LUT | Implemented in ABI 6: native 1D/2D R32F images and format-checked linear sampling. The adapter can keep the upstream LUT dimension and float values. |
| Seven compute specialization constants, some sizing shared arrays; one raster constant | Implemented in ABI 7: copied 32-bit ID/value pairs per stage, applied by the driver at executable creation. Preparation checks use the captured values, not placeholder defaults. |
| 56 compute push bytes; zero raster push bytes | Compute layout stays unchanged. Vertex lowering adds an adapter-owned 8-byte vertex address at offset 0 to the originally empty raster root. |
| Four-vertex strip with two vec2 vertex attributes | ABI 7 exposes triangle strips. Bounded declaration lowering pulls the original 16-byte vertex records through an address; no vertex expansion or processing-body rewrite. |
| Repeated passes and texture/LUT reuse | Implemented in the bounded adapter: upstream dispatch caching, reusable images/staging/vertex storage, explicit dependencies and per-operation completion cleanup. No automatic pointer tracing in core. |

Proceed with a bounded `pl_gpu` adapter, preserving upstream shader generation and
dispatch. Unsupported callbacks/formats must reject rather than fall through to
Vulkan. Keep the reference executable separate so successful reference execution
cannot masquerade as successful OGPU integration. This inventory justifies the
next design/implementation step; it is not a commitment to implement all libplacebo
GPU operations or freeze the current API shape.

### G1 checkpoint: multidimensional dispatch

Both `ogpu_dispatch_wait` and `ogpu_batch_dispatch` now accept X/Y/Z counts.
The backend records those counts and forwards them directly to Vulkan. Zero on
any axis remains an error, not an empty-work convention; limits are inclusive
and checked independently. Invalid calls leave existing recorded work unchanged.
No grid object, flattening adapter or old-signature compatibility path was added.
Existing examples and GGML retain their 1D shaders and pass Y=Z=1; rebuild them
against ABI 5.

The dedicated GPU contract test uses a 4×2×2 local size with pure X/Y/Z, 2D and
asymmetric 3D grids. It checks global/workgroup/local IDs and `NumWorkGroups`,
partial groups, guard words, repeated synchronous/batched execution and argument
copying. Synthetic asymmetric limits test inclusive boundaries (including
`UINT32_MAX`); GPU tests reject zero and representable over-limit counts without
changing the recording. All 24 ordinary tests and 14 Vulkan tests pass; Vulkan
tests and the eight existing execution examples pass on RADV and llvmpipe with
synchronization validation. ABI layout checks, mocks, binding reproduction and
Clippy pass. Rebuilt GGML also passes all six direct/scheduled cases with HOST and
DEVICE placement on each driver (24 full-dataset cases), including its lifecycle
checks. Local logs under `target/ggml-integration/` are `acceptance.p5ioDCpW.log`
and `acceptance.nbHgfqWG.log` (llvmpipe HOST/DEVICE), and
`acceptance.HGM8pMXN.log` and `acceptance.U1e2v6DE.log` (RADV HOST/DEVICE).
This validates the grid contract and preserves the first consumer, not libplacebo
execution or a new performance conclusion.

### G1 checkpoint: image descriptions and upload

ABI 6 replaces `OgpuTarget` and its fixed constructor with `OgpuImage` and a copied
`OgpuImageDesc`. The description specifies 1D/2D, RGBA8 UNORM/R32F, width/height and
sampled/storage/color/copy-source/copy-destination usages. One mip, layer and sample
remain fixed; 1D requires height 1. Color attachments remain 2D RGBA8. Usage is
checked at heap writes, draw recording and copy recording. Only attachment images
inherit framebuffer/viewport limits and allocate an attachment view. Sampled images
require linear-filter support for their actual format; independent samplers no
longer make a hard-coded RGBA8 format query.

Native 1D fits the upstream filter LUT directly. Supporting 2D R32F as well follows
the same description and descriptor path. No shader coordinate rewrite or LUT
quantization is needed. This is a small explicit format set; additional formats,
subresources and feature negotiation remain future decisions.

`ogpu_batch_copy_buffer_to_image` and `ogpu_batch_copy_image_to_buffer` copy whole,
tightly packed images from/to 4-byte-aligned buffer offsets, preserving texel bits.
Both formats use four bytes per texel. The batch retains both operands; HOST and
DEVICE buffers work through the same address-copy commands. There is no hidden
staging allocation. The caller initializes GENERAL with `ogpu_batch_discard_image`
before a first upload, then writes every texel before any reads. Later uploads can
replace contents while preserving GENERAL. Uploads order prior GPU accesses before
the copy; explicit TRANSFER_WRITE barriers precede later shader/attachment consumers.
Host buffers may be updated/reused after completion. Invalid calls do not change
recorded work, and rejected submissions do not apply uploads.

Verification covers 1D/2D × RGBA8/R32F × HOST/DEVICE sources, each with A/B/A
updates, byte-exact readback and guards. Native heap execution samples a 256-entry
1D R32F LUT and a 17×7 R32F image with nearest/linear samplers into an R32F storage
image, checking negative and greater-than-one values against a numerical reference.
Lifetime, wrong-device/usage/range, rejected upload, attachment rejection and
missing-linear-support tests cover the surrounding contract. The existing C graphics,
image-loop and heap-image examples use the new public image API.

All 26 ordinary tests, 16 Vulkan tests, and the three migrated C graphics/image
examples pass locally; GPU coverage includes RADV RX 5700 XT and llvmpipe with
synchronization validation. The C/Rust ABI check covers 719 layout values;
binding reproduction, Clippy and mocks pass. GGML was rebuilt against ABI 6 and
its HOST/DEVICE lifecycle checks pass on both drivers; the full MNIST comparison
was not rerun for this image-only change. Shaderc 2026.1 and SPIRV-Tools 1.4.357.0
compile/validate the native float sampling fixture. Remote CI remains unverified.

### G1 checkpoint: specialization and vertex pulling

ABI 7 makes executable preparation accept an `OgpuShaderDesc` with SPIR-V and
32-bit `{id, bits}` specialization values. Each stage has an independent ID space;
descriptions and values are consumed before creation returns. Missing values keep
shader defaults; unknown IDs are ignored; duplicate IDs are rejected. The driver
specializes the shader, including dependent shared arrays. Present constant types,
specialized resource limits and shader validity remain caller obligations. This
small creation contract fits compile-time algorithm parameters without making
shader reflection or an offline SPIR-V rewrite engine part of the runtime.

Raster creation now chooses triangle list or strip. For the captured libplacebo
layout, adapter-side declaration lowering replaces two vec2 inputs with reads of
the original 16-byte records, indexed by `gl_VertexIndex`. The adapter root contains
one 8-byte-aligned GPU address at byte 0; the original raster root is empty, and
compute's 56-byte root remains unchanged. The lowering rejects other strides,
offsets, inputs or pre-existing push data. It preserves processing statements and
uses the same helper for captured upstream shaders and the independent test shader.
No vertex-layout object or CPU strip expansion is added to OGPU.

The public-API tests execute default and overridden int/uint/float/bool constants,
specialized shared-array sizes, nonsequential IDs, absent IDs and copied values.
Raster tests use different values for the same ID in the two stages and verify
every pixel of an address-pulled four-vertex strip. Invalid descriptions, duplicate
IDs and invalid topology are rejected. All 27 ordinary tests and 18 Vulkan tests
pass; Vulkan tests and all eight C execution examples pass on RADV RX 5700 XT and
llvmpipe with synchronization validation. ABI checks cover 737 layout values;
Clippy, mocks and binding reproduction pass. Remote CI remains unverified.

The refreshed reference still passes all nine frames on each driver and now
captures exact vertex metadata. All nine shaders per capture compile and validate
after declaration lowering. A separate audit creates all six captured executables
through the public OGPU API using their actual specialization bits. This submits
no work: it establishes executable preparation, not image-processing correctness.
Local artifacts under `target/libplacebo-integration/`:

- llvmpipe: `reference.xcWxo67v`, `compiler.9aFdFAGm`, `executables.6kGnUKEv.log`.
- RADV: `reference.o5x8P9Vc`, `compiler.O1FkPHFA`, `executables.ipJCRVRZ.log`.

Rebuilt GGML passes all six direct/scheduled full-dataset cases with HOST and DEVICE
placement on both drivers (24 cases), including lifecycle checks. Logs under
`target/ggml-integration/`: `acceptance.Zxe32fna.log` / `acceptance.YDm1wgiO.log`
(llvmpipe HOST/DEVICE) and `acceptance.Lu9OfJK0.log` / `acceptance.WGsoRkDe.log`
(RADV HOST/DEVICE). This is regression evidence, not a new performance comparison.

That checkpoint prepared the prerequisites for the bounded adapter below; it did
not itself execute upstream image processing.

## G2: bounded OGPU execution

The C adapter implements the pinned `pl_gpu_fns` boundary with public OGPU calls.
The separate C++ shaderc bridge shares declaration-lowering helpers with G0/G1;
it is consumer glue, not part of the Rust runtime. Upstream source remains unchanged.
Reference and consumer executables share the input/generation/dispatch workload;
the consumer creates no upstream Vulkan device and has no reference/CPU fallback.

An additive `ogpu_device_limits` query supplies actual local group dimensions,
invocation/shared-memory bounds, grid bounds and image/root ceilings. This avoids
assuming either tested GPU's properties when upstream chooses its compute layout.
No existing ABI-7 structure or function signature changes. This is not optional
capability negotiation or general executable reflection.

The adapter creates native 1D R32F LUT and 2D RGBA8 images, retains per-image HOST
staging, and uploads upstream-provided bytes unchanged. Prepared passes own SPIR-V,
copied metadata, independent heaps and reusable vertex/indirect buffers. Descriptor
bindings populate fixed native heap slots; four original vertices are address-pulled
as a strip. Compute root bytes and actual specialization values remain upstream-owned
contracts; changed specialization values recreate executables, unchanged values reuse
them. Pass creation is reused across A/B/A frames.

Every operation submits and waits. Previous writes are explicitly ordered before
the next pass; first uploads/storage writes initialize GENERAL, raster uses CLEAR
or LOAD, and readback uses OGPU image copies. Completions and recordings are destroyed
before editing/clearing heaps. There is no in-flight image/vertex reuse and no implicit
core allocator or dependency tracker. This deliberately tests correctness before
asynchronous scheduling; per-operation waits are an adapter policy, not an API need.

The adapter is not a general `pl_gpu` implementation: no general buffers/UBOs/global
uniforms, emulation dispatcher, callbacks, timers, subregions, indexed draws or
arbitrary raster state. Its push-only variable profile does not satisfy upstream's
general-backend guarantee of UBO or global-uniform support. It therefore uses only
the bounded public generation/dispatch paths, not the private general finalizer.
Unsupported requests fail closed and mark the adapter failed; clean destruction
remains available. Live-child device destruction is a caller-contract violation.

### Verified results — 2026-09-14

On each driver: six prepared passes reused across nine compute and nine raster
operations, 12 uploads (including three upstream LUTs), 18 downloads, and zero live
texture/pass objects after teardown. Both intermediate and final images compare
byte-for-byte with upstream: **286,488 bytes, zero differing bytes**, tighter than
the predeclared RGB tolerance of 2/255 (unchanged). Alpha, nearest copying and A/B/A
determinism are exact. Vulkan and synchronization validation were enabled; no
validation errors were reported. Remote CI remains unverified.

Local paired artifacts under `target/libplacebo-integration/`:

- llvmpipe: `reference.hTbDTka3` / `ogpu.q5DhoQs0`,
  adapter checks `backend-checks.wEy9JeV7.log`.
- RADV RX 5700 XT: `reference.zXVkdcBS` / `ogpu.sy8anKWa`,
  adapter checks `backend-checks.CMFjH1Mv.log`.

Ordinary tests (27), Vulkan tests (18 on each driver), Clippy and 745 ABI layout
checks pass. Tests compare cached limits with driver properties and cover query
errors/device-loss readability. Adapter tests exercise partial-transfer, unsupported
image/shader rejection with cleanup, live-child destruction rejection, and actual
A/B/A specialization updates on one live pass with pixel checks. The
compiler-only gate remains independently reproducible.

### Capability follow-up (ABI 8)

The two-consumer review led to [explicit enabled-capability and exact image-support
queries](execution-capabilities.md), plus compute image/heap support independent
of rasterization. The adapter now validates its required combinations before
publishing formats, while exact extents remain checked at creation. Both paired
driver comparisons still have zero differing bytes; processing and scheduling are
unchanged. The follow-up records tests/artifacts and the subsequent D4 completion-resource
comparison. Successful images do not establish stability, general backend coverage
or efficient scheduling.

### Receipt-held resource reuse (ABI 9)

The [completion-resource implementation](completion-resource-review.md#implementation-abi-9)
lets each pass retain its last completed receipt during the next run's specialization,
heap and vertex updates. Every pass still submits and waits; its new receipt survives
the image-heap clear after execution. The consumer verifies 12 old-receipt reuses;
the one-pass specialization A/B/A check verifies two. There is no new scheduling,
shader rewrite or compiler feature. Paired pixel results and failure cleanup remain
the acceptance criteria; retained receipts are not in-flight GPU work.
