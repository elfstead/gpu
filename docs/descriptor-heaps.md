# Independent descriptor heaps

Implemented 2026-09-13 at `d93691f`, completing the
[image ownership review](image-ownership-review.md). This follows
[image preservation](image-preservation.md) at `a835f19` and replaces the original
[coupled table experiment](heap-images.md). This checkpoint introduced **ABI 3**:
`OgpuImageTable` and its functions are removed, without aliases or a second execution
path. Rebuild consumers with matching header/library/shaders from this checkout.
The later [memory checkpoint](memory-transfers.md) introduces ABI 4, rejecting
ABI 1–3 at probe creation without changing these heap operations. This is not a
stable cross-version ABI promise.

## Contract

`OgpuImageHeap` and `OgpuSamplerHeap` independently own their backing allocations.
Creation takes a nonzero capacity, writes take a first slot and copied descriptions,
and separate batch bindings select each heap for subsequent draws/dispatches. Image
and sampler indices are independent uint32 namespaces relative to those bindings.
Changing image membership no longer allocates a new heap or recreates sampler storage.

An image entry retains its target until replacement, clear or heap destruction.
`ogpu_image_heap_clear` releases that ownership and makes the slot invalid; it does
not install a readable null descriptor. New slots are also invalid until written.
Shader indices, descriptor kinds and valid image contents remain trusted caller
obligations, not runtime-checked accesses or recursive pointer tracing.

At ABI 6, views follow the image's RGBA8 UNORM or R32F format and 1D/2D dimension,
with one mip/layer/sample. ABI 11 adds RGBA16F and sampled-only RGBA16 UNORM views.
Sampled/storage descriptors require the matching creation
usage. No view handle or subresource selection is exposed. Sampler
descriptions independently select nearest/linear minification and magnification,
and clamp-to-edge/repeat U and V addressing. Coordinates are normalized, LOD is zero,
W clamps, and comparison/anisotropy are disabled. Sampled image creation checks
linear-filter support for its actual format and rejects UNSUPPORTED if absent;
samplers are format-independent. At ABI 8, both heap types and non-color images
work on compute-created devices; rasterization is no longer a prerequisite for
compute image access. See [the capability follow-up](execution-capabilities.md).

Bindings retain the entire heap through recording and submission retirement,
including earlier bindings superseded later in the batch. At ABI 9, wait/terminal
poll destroys that submission's native command pool before releasing these references.
Editing remains rejected while any other recording or unretired submission retains
the heap. Discard recordings and observe all retaining submissions; the completed
receipts themselves may survive edits. A submitted batch transfers its references to
the submission resources. Public heap/image handles may be released while retained uses exist.

This deliberately gives exclusive mutation rather than slot streaming. Internally,
`Rc::get_mut` enforces the rule at the C boundary. It also ensures native command
pools that used a heap's driver-reserved range have been destroyed before an edit.
No range locks, per-slot completion tracking or extra retirement queue were added.
External host-call serialization still applies.

## Failure and cache behavior

Writes validate the whole request and generate native descriptors into temporary
host storage before touching the live heap. Invalid range/device/kind/reserved fields,
unsupported sampler settings and descriptor-generation failures preserve old entries.
Noncoherent cache invalidation precedes the commit to preserve bytes outside the
edited slots, including the driver reservation, during whole-allocation maintenance.
An invalidation error also leaves the prior entries intact.

After that, image ownership is replaced and descriptor bytes are copied and flushed.
A flush failure poisons that heap: later edits/bindings fail and the caller must
destroy it. New entries remain retained until destruction because copied descriptors
may already reference them. There is no claim that a failed flush rolled back.
Device loss still follows the device-wide terminal contract.

Zero-count writes allow NULL descriptions and `first == capacity`, but still require
a live, non-poisoned, exclusively owned heap. Clear follows the same range/ownership
rules. There is no public raw descriptor mapping.

## Image preservation and executable contract

The existing image workload now spans three ordered submissions, with no intervening
host wait: render source/prepare storage image; compute image load/store; sample and
render, then diagnostic readback. Initialization, dependencies and lifetime are
separate obligations. Binding does not initialize images or establish visibility.
Ordinary uses preserve GENERAL and contents; CLEAR/discard explicitly invalidate
them. LOAD preserves existing texels with COLOR_READ | COLOR_WRITE dependencies.
Rejected/abandoned initialization cannot be used to justify subsequent reads.

The example keeps its 16-byte compute root (four uint32 values). Its fragment root
is now 20 bytes: image index, sampler index, width, height, then a float horizontal
texel offset. This tests sampler behavior without adding runtime shader metadata.
Slang 2026.14.1 emits native heap access; checked-in SPIR-V must reproduce and pass
validation/capability checks. No descriptor-set translation path was added.

## Verification

Local execution used Mesa 26.2.1 llvmpipe (LLVM 21.1.8), Vulkan 1.4.354, loader and
validation layers 1.4.357.0, with Vulkan and synchronization validation enabled.
This is software-driver evidence, not modern physical-GPU coverage.

- `cargo test --workspace`: 22 ordinary tests pass; all 11 opt-in Vulkan tests pass
  under `cargo xtask gpu-tests`, including preservation and independent heaps.
- `cargo xtask heap-image`: six sizes (`1x1`, `2x3`, `63x65`, `64x64`, `65x63`,
  `97x65`), each with four sampler/index cases and three submissions. Two image
  heaps share one sampler heap; slots are cleared/rewritten without reallocation.
  Cases cover nearest/linear texel centers, a one-pixel nearest clamp shift, and a
  half-texel linear blend including repeat at the right edge. Processed pixels,
  nearest samples, alpha and guards are exact; linear RGB permits one byte rounding.
  Public handles are released before the final submission in the last case.
- Heap tests check wrong-device/range/kind inputs, partial allocation/binding failure,
  descriptor generation failing after staging writes, invalidation failure, terminal
  flush failure and unsupported linear filtering. Ownership is checked after clear,
  abandoned/rejected recordings, a deterministically gated pending submission,
  successful wait and completion destruction. The C example also checks superseded
  bindings, retained mutation rejection, reserved fields and empty NULL writes.
- `cargo xtask abi`: 700 layout values agree. Binding and pinned heap-shader
  reproduction, mock tests, formatting and warning-free workspace Clippy pass.
  Existing image-loop, graphics, retirement and timed/untimed matmul regressions pass.
- `bash integrations/ggml/run.sh /tmp/ogpu-ggml-consumer`: lifecycle checks and all
  six direct/scheduled MNIST cases pass against ABI 3. Each checks 10,000 images,
  9,801 correct predictions, maximum logit error `3.43322754e-05`; scheduled cases
  preserve three intermediate aliases. Local log:
  `target/ggml-integration/acceptance.sd2RjJx9.log` (not a committed artifact).

Failure injection exercises the noncoherent maintenance branch; it does not establish
real noncoherent-memory hardware coverage. No performance conclusion follows from
these runs. At this checkpoint physical-GPU validation and a provisioned modern
execution-CI runner were next; the subsequent hardware result is below.
Concurrent slot updates, generalized views/formats, image uploads and broader
raster state remain separately scoped decisions. The local stopping condition from
the ownership review is met, not the project's stabilization gates.

The subsequent [hardware run](hardware-validation.md), after making unified image
layouts optional at `a5a609d`, also passes image/heap/preservation tests on RX 5700 XT
/ RADV. The unchanged GENERAL-only commands work without the extension's
layout-efficiency guarantee. All twelve current Vulkan tests pass independently on
RADV and llvmpipe; the original eleven-test record above remains checkpoint history.

Implementation: [public header](../include/ogpu.h), [heap owners](../crates/ogpu/src/heaps.rs),
[C boundary](../crates/ogpu/src/execution_api.rs), [failure/lifetime tests](../crates/ogpu/src/heap_tests.rs),
[public C consumer](../examples/heap_image.c). See [working status](plan.md) for priorities.
