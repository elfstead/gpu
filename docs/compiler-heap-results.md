# M2 heap-interface checkpoint

Accepted 2026-09-25 at `7b0eced` (implementation `6a8cc69`, installed CLI forwarding
fix `7b0eced`). This completes the heap portion of
[slice 3](compiler-contract-plan.md), **not** its graphics stage-linking portion
or M2 as a whole. Runtime and public ABI remain unchanged at ABI 17.

## Result and performance boundary

The existing `examples/heap_image.c` now uses generated compute/sample roots,
embedded native artifacts, local dimensions, resource-use summaries and capability
predicates. It no longer duplicates those root layouts or hard-codes the workgroup
size. Field-order and local-size mutations use identical C source. Heap contents,
slot validity, dimensions, formats, dependencies and reachable resource lifetimes
are still application-owned. No runtime reflection, implicit retention, argument
repacking, allocation, descriptor conversion or device copy was added.

The original generated binaries are byte-identical to the existing checked-in
heap binaries. The workflow verifies this mechanically. This is structural evidence
that host interface generation preserves the existing native strategy, not a new
timing comparison or a universal Vulkan performance claim.

Select `--native-heaps` explicitly. Without it, pinned Slang emits descriptor-set
bindings and the adapter rejects them. Slang JSON describes the roots but omits
dynamic heap accesses; resource summaries therefore come from checked SPIR-V.
The bounded checker verifies separate heap builtins, direct untyped descriptor
access/load, float-sampled 2D or RGBA8-storage 2D images, samplers, and strides
derived from the correct descriptor type through `OpConstantSizeOfEXT`, following
the [native descriptor-heap specification](https://github.khronos.org/SPIRV-Registry/extensions/EXT/SPV_EXT_descriptor_heap.html).
It rejects descriptor-set bindings, swapped heaps, unsupported image types/formats,
unknown capabilities/extensions, incorrect strides and pointer escapes. This is
a trusted-compiler adapter with `spirv-val`, not a standalone SPIR-V validator.
Sampled float image types do not establish actual view format or slot safety.

## Checks

Pinned Slang 2026.14.1, Rust 1.97.1, ABI 17; Radeon RX 5700 XT/RADV Mesa 26.2.1
and llvmpipe/LLVM 21.1.8, with Vulkan and synchronization validation:

- Six extents × four heap/sampling cases × original/reordered interfaces = **48
  cases per driver**. Each case retains the three-submission pixel, guard, heap
  mutation/retirement and early-destruction oracle. Nearest cases remain exact;
  linear filtering retains its existing one-code tolerance.
- Both roots reorder; compute local size changes 64 → 32. Original native artifacts
  remain byte-identical. Eleven bounded generator tests pass, including a real
  descriptor-set compilation that must reject. Initial GPU receipts contain ten
  tests; the additional pointer-escape rejection test passed before committing.
- A clean revision-identified SDK installs and relocates. Original and independently
  reordered heap consumers execute on both drivers, both stale headers reject, and
  installed regeneration/rebuild succeeds without checkout or Cargo dependencies.
  The first installed test caught missing `--native-heaps` forwarding; its failed
  SDK is not the accepted revision. The corrected installed wrapper is tested.
- Transform (16), affine (6), aggregate (20) tests and generated headers still pass.
  The learned-image build checks and all 38 small cases in original/mutated and
  normal/diagnostic modes pass on both drivers: 152 frames per driver. Maximum float
  error `1.71016513e-7`, error/bound `0.00667210166`, RGB delta 1; original/mutated
  GPU files remain byte-identical within each driver.
- Forty ordinary tests, strict Clippy and formatting pass. The standard
  `cargo xtask heap-image` and `heap-shaders --check` paths also pass. No runtime
  changes: full 28-case GPU and ABI suites were not rerun for this checkpoint.

## Reproduction and local receipts

```sh
python3 examples/compiler/heap_workflow.py --check
python3 examples/compiler/heap_workflow.py
cargo xtask heap-shaders --check
cargo xtask heap-image
python3 tools/install.py --prefix /absolute/new-prefix
python3 tools/test-install.py --prefix /absolute/new-prefix --heap-image --shader-check
```

Use `SLANGC` and the driver's `VK_DRIVER_FILES` explicitly; enable
`VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1` for GPU
checks. Accepted local SDK: `/tmp/ogpu-sdk-heaps-m2-final`; relocated applications:
`/tmp/ogpu-external-s168oeq0` (Radeon), `/tmp/ogpu-external-ks2muyth` (llvmpipe).
Local logs are reproducibility diagnostics, not distributed package dependencies:

| Log under `/tmp/` | SHA-256 |
|---|---|
| `ogpu-heaps-radv.log` | `7e0f041530c0fa4ba0f51826fbb22e7f497d633bfe32b574cedbbd061ea12256` |
| `ogpu-heaps-lvp.log` | `8ffeb7f61b1d45aa7071dc7071c05e82817d46e7b0c8e767b939036d37075ddc` |
| `ogpu-heaps-m1-radv.log` | `df876d6c3579b1ed434f0fa251ec0bad0b54f238ac2b91678be1a8949fc4e751` |
| `ogpu-heaps-m1-lvp.log` | `d03461bdbe7dee6b5f90a123a8cf120eddc1daa80040c158d6d90613910716de` |
| `ogpu-heaps-install-final.log` | `b7ae88fb47485d5325f737febefd01c7093fa230b809172bb15f7546e2108bbe` |
| `ogpu-heaps-sdk-radv-final.log` | `803a29ebeb067feec2ae2794c7350101f84d1cd8014d6fd5ff454b5b28a09a89` |
| `ogpu-heaps-sdk-lvp-final.log` | `274f4feb11f8a10ccdb403352165ca8e56245d0223f7d3a3c56f5c5684b5eabf` |

Next: generated producer/consumer varyings with positive/negative link checks.
The two non-heap raster stages are still supplied SPIR-V inputs. No Metal image,
general varying/attribute, accelerated numeric, language-direction or P7 acceptance
is implied by this checkpoint.
