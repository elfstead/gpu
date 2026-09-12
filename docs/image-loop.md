# Graphics → compute → graphics experiment

## Hypothesis and scope

The existing batch and address-based memory model should express a rendered-image
processing loop without intermediate CPU work or new API entry points. This tests
composition, not direct compute access to images or texture sampling.

The first implementation will use the existing fixed RGBA8 offscreen profile:

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

After completion, compare every first-render pixel against a procedural color or
opaque black; check known interior/background pixels to reject a vacuous draw.
Compare every compute output against a byte-oriented CPU transform of the first
render, and every final pixel against that same reference. Expanded tests should
include odd/non-square dimensions, dispatch tails, tiny images, buffer guards,
and reuse of targets across submissions. Avoid exact expectations at triangle
edges, where rasterization rules are not the subject of this experiment.

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
