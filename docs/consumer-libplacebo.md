# Second consumer: libplacebo image processing

Approved 2026-09-13. Status: G0 source/compiler/reference gate passed. G1 adapter/API
implementation is next; G2 OGPU execution is not implemented yet.
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
| Sampled RGBA8 input, storage RGBA8 intermediate, RGBA8 fragment target | Existing image heaps and rendering model fit; add a deliberate image upload contract rather than CPU texture mirrors. |
| 256-entry R32F, linearly sampled 1D filter LUT | Current target is fixed 2D RGBA8. Decide format/dimension/usage description together; do not quantize LUT weights to fit RGBA8. Upstream also has dimension-selection logic worth evaluating before adding every image dimension. |
| Seven compute specialization constants, some sizing shared arrays; one raster constant | Values are captured, not optional. Compare adapter-side SPIR-V specialization with a small executable specialization contract. Do not execute placeholder GLSL defaults. |
| 56 compute push bytes; zero raster push bytes | Fits current root size model. If adapter metadata is added, define offsets explicitly and preserve the upstream layout. |
| Four-vertex strip with two vec2 vertex attributes | Compare adapter-side vertex pulling/strip expansion with a small raster topology contract. No depth, blending, indexed draws or arbitrary raster state is needed for this pipeline. |
| Repeated passes and texture/LUT reuse | Preserve upstream dispatch caching and image lifetimes. Translate dependencies at the adapter boundary; no automatic pointer tracing in core. |

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

Next: image creation descriptions and explicit upload, including floating-point
filter data, followed by executable specialization and vertex-input translation.
G2's upstream image comparison remains pending.
