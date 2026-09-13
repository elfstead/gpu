# Second consumer: libplacebo image processing

Approved 2026-09-13. Status: G0 source/compiler/reference gate in progress.
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
