# Graphics → compute → graphics experiment

## Hypothesis and scope

The existing batch and address-based memory model should express a rendered-image
processing loop without intermediate CPU work or new API entry points. This tests
composition, not direct compute access to images or texture sampling.

The implementation uses the existing fixed RGBA8 offscreen profile:

1. Compute generates triangle vertices and an indirect draw record.
2. Graphics draws an asymmetric, coordinate-colored triangle into a target.
3. An explicit image-to-buffer copy produces tightly packed RGBA8 pixels.
4. Compute vertically flips the pixels, swaps red/blue, and inverts green into a
   separate linear allocation.
5. A fullscreen triangle's fragment shader reads that allocation by device address
   and writes a second RGBA8 target.
6. Copy the final target to a buffer, submit once, and wait once for verification.

The image copies manage their known image-layout dependencies. The caller records
COMPUTE_WRITE → VERTEX_READ | INDIRECT_READ before the first draw,
TRANSFER_WRITE → COMPUTE_READ before processing, and
COMPUTE_WRITE → FRAGMENT_READ before the second draw. Both draws use the generated
indirect record. Raw-address allocations remain alive until completion.

## Representation and checks

Pixels use one uint32 per RGBA8 texel on the currently supported little-endian
Linux x86-64 target. Byte order is R, G, B, A. Rows follow target coordinates;
the processor explicitly maps destination `(x, y)` to source `(x, height-1-y)`.
The output fragment shader indexes using `gl_FragCoord` and unpacks UNORM bytes.
There is no filtering, sampler, image upload, descriptor, or implicit image/array
equivalence. The two copies and separate allocations are real costs, not zero-copy.

After completion, the C example compares every first-render pixel against a procedural color or
opaque black; check known interior/background pixels to reject a vacuous draw.
Compare every compute output against a byte-oriented CPU transform of the first
render, and every final pixel against that same reference. The test includes
odd/non-square dimensions, dispatch tails, tiny images, buffer guards,
and reuse of targets across submissions. It avoids exact expectations at triangle
edges, where rasterization rules are not the subject of this experiment.

## Run and observed result

Run `cargo xtask image-loop`. The [C example](../examples/image_loop.c) is both the
public-API demonstration and expanded test harness; it visits every device that
supports graphics execution. Its sizes are 1×1, 2×3, 63×65, 64×64, 65×63, and 97×65.
Each runs twice with the same targets/allocations. Before each submission, host
code poisons all three pixel buffers; stale output cannot satisfy the next check.
One final wait per loop precedes all intermediate and final reads. A reuse barrier
orders prior buffer accesses before subsequent GPU writes. The processor root is
overwritten immediately after recording to exercise copied arguments.

All 12 loops per device passed locally on 2026-09-12 on the RX 5700 XT (RADV) and
llvmpipe, with Vulkan synchronization validation enabled and no reported validation
errors. Explicit dependencies are still caller obligations: validation alone does
not prove arbitrary buffer-device-address accesses are free of hazards. Existing
Rust GPU tests and C compute/batch/graphics/reduction examples, ABI/mock checks,
Clippy, unit tests, and the release build also passed. CI is configured to run the
new example and validate its shader binaries; no remote run is claimed.

No public header, Vulkan bindings, or runtime implementation changes were needed.
The four new shaders were compiled with glslang 16.4.0 and validated with
SPIRV-Tools 1.4.357.0. They reuse the existing triangle producer/vertex binaries.
Normal builds use checked-in SPIR-V; to regenerate the new files:

```sh
for shader in image-pattern.frag image-process.comp fullscreen.vert image-read.frag; do
    glslangValidator -V --target-env vulkan1.2 "examples/shaders/$shader" -o "examples/shaders/$shader.spv"
    spirv-val --target-env vulkan1.2 "examples/shaders/$shader.spv"
done
```

The processor root has addresses at offsets 0 and 8, then uint32 width/height at
16 and 20 (24 bytes). The final fragment root has a pixel address at 0, uint32
width at 8, and reserved padding at 12 (16 bytes). Each processor workgroup has
64 invocations; out-of-range lanes return without touching memory. This is not a
workgroup-cooperation test; [reduction](reduction.md) covers that separately.

## Questions deliberately left open

- Would direct storage-image access or sampled images make a materially better
  workload? This buffer-mediated path does not answer their API design.
- Where should format, row pitch, and representation conversions live for real
  consumers? This experiment has only one fixed packed format.
- Can known-resource dependencies reduce caller burden without hiding aliasing
  through arbitrary addresses? The current buffer dependencies remain explicit.
- What do copies, host-visible allocations, and fragment address fetches cost?
  Correctness alone provides no throughput or portability claim.

See [graphics](graphics.md), [batches](batches.md), and the
[experiment ledger](experiments.md) for the surrounding contracts and evidence.
