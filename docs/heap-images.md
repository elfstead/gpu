# Direct image access and sampling

Historical compiler/consumer record for `cbc6528`. Its immutable table wrapper and
same-batch initialization rule have since been removed. See
[image preservation](image-preservation.md) and [independent heaps](descriptor-heaps.md)
for the replacement heap contract and evidence (introduced in ABI 3; the current
[memory checkpoint](memory-transfers.md) uses ABI 4). Commands below now run the follow-up
shaders/example; use the recorded source revision to reproduce this original variant.

Bounded D3 compiler/consumer experiment, 2026-09-13. This follows the modern-baseline
migration and [retirement comparison](retirement.md). It tests the missing end-to-end
path, not a general texture API or a choice of the project's eventual source language.
Implementation checkpoint: `cbc6528` (ABI 1 with additional symbols; use matching
headers/library from this source revision, not an arbitrary older ABI-1 library).

## Result and API direction

Native heap-indexed images work through the public C ABI. The workload is:

```text
draw coordinate pattern → compute image load/store → fragment sampling → draw
                                                               ↓
                                             final diagnostic copies and wait
```

The compute shader flips the image vertically, swaps red/blue, and inverts green.
It reads a sampled-image descriptor with integer texel coordinates, writes a typed
RGBA8 storage image, then graphics samples that image with normalized coordinates.
There is no image-to-buffer copy between stages. Diagnostic copies of the processed
and final images happen after the chain; every pixel and surrounding guard is checked.

This supports separate **image allocations, descriptor indices, and sampler indices**,
with explicit dependencies in the existing batch. Buffer data still uses addresses.
It does not support treating opaque images as ordinary addressable byte arrays.

The experiment adds an immutable `OgpuImageTable`: the application supplies an ordered
array of target/kind pairs, and entry positions become shader indices. Binding retains
the table, its two heap allocations and referenced images through completion destruction.
Subsequent draws/dispatches use the same table until another bind. No executable binding
layout or descriptor-set mapping is created.

This is a usable experimental adapter, **not the selected final heap-management API**.
It couples the resource heap to one fixed nearest/clamp sampler and retains all entries,
including unused ones. Those constraints keep this compiler/access test bounded but
would be restrictive for streaming, large persistent heaps, or independent sampler reuse.

| Alternative | Benefit | Cost / evidence |
|---|---|---|
| Buffer-mediated image loop | Ordinary addresses; existing regression workload | Copy and linear intermediate allocation needed between image and compute |
| Immutable image table (implemented) | Direct image access, copied metadata, clear retained ownership, no slot-update races | Coarse retention and fixed sampler; new tables allocate new heaps |
| Application-owned resource/sampler heaps | Independent lifetime, slot allocation and updates; closer to Aaltonen's prototype | Caller must manage descriptor reuse and implementation reservations; not exercised here |

The last alternative may expose a better API even though this example succeeds.
Compatibility with the table wrapper is not a reason to reject it. Next is a focused
API review of independent heap/view/sampler ownership and image initialization—not
another unrelated workload or a declaration of API stability. The resulting
[ownership/preservation proposal](image-ownership-review.md) is now available;
it does not change the implementation described here.

## Initialization and synchronization

The experiment exposed a better primitive: `ogpu_batch_discard_image` (originally
named `ogpu_batch_discard_target` before ABI 6). A compute-written
image should not require a dummy draw merely to become usable. Discard orders previous
uses and initializes GENERAL without clearing texels. Every accessed texel must be written
before it is read. Draw continues to discard and clear, using the same backend initializer.
Copy now accepts an earlier draw **or discard** in the same batch and orders prior image
writes before transfer reads. The example exercises copying a compute-only-produced image.

All image uses remain in GENERAL. Color-write → compute-read and compute-write →
fragment-read dependencies are explicit. Binding a table adds no data dependency,
initialization, recursive pointer tracking, or permission to sample an active attachment.
The bounded contract requires a same-batch draw/discard before shader image access;
the runtime does not inspect arbitrary shader indices to prove that obligation.
Preserving image contents across batches remains an unresolved API limitation.

Heap descriptor bytes are written and flushed only during construction. After a bind,
the host does not touch either heap, including whole-allocation cache operations.
Heap alignments, descriptor strides, size limits and implementation reservations come
from device properties. Exact reserved ranges stay alive until binding command buffers
are reset or freed (originally pool destruction, now also reset at ABI 13), not merely
GPU completion, following the
[resource heap binding contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBindResourceHeapEXT.html).
Sampler bindings use their separate heap and reservation. No public sampler or shader
image-view Vulkan objects are allocated; the attachment view is still needed for rendering.

## Shader/toolchain gate

The two new sources use **Slang 2026.14.1**, direct SPIR-V 1.5 output, C layout and
native `spvDescriptorHeapEXT`. Shader roots are four uint32 fields (16 bytes).
Resource and sampler heaps are separate index namespaces. Integer image loads use
`Texture2D<float4>`; storage writes use `RWTexture2D<float4, 0, 10>` (RGBA8).
A local format attribute did not specialize the heap-fetched type in this compiler;
the explicit format type argument removes unwanted formatless-storage capabilities.
See Slang's [texture format parameters](https://shader-slang.org/stdlib-reference/types/0texture-01/index.html).

Generated modules contain native `ResourceHeapEXT`/`SamplerHeapEXT`, untyped heap access
and device-sized descriptor strides. They contain no descriptor-set bindings and require
only Shader, UntypedPointersKHR and DescriptorHeapEXT capabilities. No shaderInt64,
formatless storage, or other optional arithmetic capability was enabled to make them work.
The [SPIR-V heap extension](https://github.khronos.org/SPIRV-Registry/extensions/EXT/SPV_EXT_descriptor_heap.html)
defines this direct indexing mechanism. This confirms a compiler path without adopting
Slang as the project's source-language design or runtime dependency.

To reproduce (normal builds use the checked-in binaries):

```sh
SLANGC=/path/to/slangc cargo xtask heap-shaders --check
cargo xtask heap-image
cargo xtask gpu-tests
```

Omit `--check` to regenerate. The task requires the exact compiler version, validates
SPIR-V using a Vulkan 1.4-capable SPIRV-Tools, checks capabilities/native heap access,
and compares both binaries byte-for-byte. Tested SPIRV-Tools: 1.4.357.0.
Official [Slang release](https://github.com/shader-slang/slang/releases/tag/v2026.14.1),
Linux archive `slang-2026.14.1-linux-x86_64-glibc-2.27.tar.gz`, SHA-256:
`427d9985aac9e88912429bf9e7c1a3543bf35a61aaa855c890eeb96920175e0c`.
No compiler download occurs in an ordinary build or test.

## Verification and limits

Mesa 26.2.1 llvmpipe, loader/validation 1.4.357.0, Vulkan and synchronization validation:

- Six image sizes (1×1 through awkward 97×65), each with two descriptor permutations,
  table rebinding, reused images, exact processed/final pixels and guards.
- Public table and image handles released before submission on the second pass.
- Rust ownership tests cover discarded recordings, retained tables after wait,
  wrong-device/kind/empty inputs and post-submit recording rejection. Injected failures
  in resource and sampler descriptor writes release both partially constructed heaps.
- 22 ordinary tests, 10 execution tests, 691 C/Rust ABI checks, pinned bindings/shader
  reproduction, formatting, Clippy and loader mocks pass. The buffer-mediated image loop
  and scratch-retirement comparison still pass. Both matrix kernels pass all 50 shapes
  in timed/untimed modes. GGML lifecycle checks and all six direct/scheduled MNIST cases
  pass (10,000 images per case; log `acceptance.w1ZrRjR2.log` under the local target tree).

This verifies one-layer/mip/sample RGBA8, texel loads, compute stores and nearest sampling.
It does not establish performance gains, nonuniform per-lane indices, filtering quality,
mutable slots, arbitrary formats/views, general upload, preserved cross-batch images,
or images on a compute-only queue. Graphics-profile target creation now checks the combined
attachment/readback/sampled/storage usage; unsupported combinations are rejected.
Physical-GPU and provisioned remote execution-CI evidence remain pending. The manual
execution workflow includes shader validation and this experiment; its runner is not
provisioned here. Hosted Ubuntu SPIRV-Tools is too old for this extension, so it does not
validate these two modules or reproduce Slang output.
