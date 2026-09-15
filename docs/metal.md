# Native Metal backend

macOS arm64 uses Metal directly; it does not route execution through MoltenVK.
Device discovery, addressable HOST and DEVICE buffers, SPIR-V compute kernels,
specialization constants, synchronous dispatch, asynchronous batches, barriers,
buffer copies, polling, waits and explicit buffer retention implement the common
compute subset of the C ABI.

SPIR-V is translated to MSL in-process with SPIRV-Cross and compiled by Metal.
Physical-storage-buffer pointers become native Metal GPU pointers, so existing
root-data layouts and `ogpu_buffer_device_address` remain unchanged. Every live
buffer on a device is declared to a compute encoder for residency because pointer
arguments are opaque to the runtime. Batch barriers end the preceding encoder;
Metal command-buffer encoder order supplies the required dependency.

The backend reports graphics queues, images, image/sampler heaps, raster pipelines,
and queue timestamps as unsupported. Their ABI symbols remain present and creation
outputs are cleared on failure. Vulkan fields in `OgpuDeviceInfo` are zero for a
Metal device; capability fields describe equivalent OGPU behavior rather than
Vulkan extensions.

Run the native acceptance checks on Apple Silicon:

```sh
cargo test -p ogpu
cargo xtask smoke
cargo xtask compute
cargo xtask batch
cargo xtask reduction
cargo xtask retirement
```
