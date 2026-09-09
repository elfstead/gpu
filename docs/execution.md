# First execution experiment

The initial internal Rust round trip allocates one host-visible, GPU-addressable
buffer, uploads 4099 integers, dispatches `x = x * 3 + 7` twice, makes a partial CPU
update, and reads back and verifies every result. The count deliberately isn't a
multiple of the shader's 64-thread workgroup size.

```sh
cargo test --locked gpu_roundtrip -- --ignored --nocapture
```

This test needs a real Vulkan loader (or Mesa lavapipe). It runs on every eligible
device and fails if none can execute. Discovery still requires only a Vulkan 1.1
instance implementation; execution requires a Vulkan 1.2 device, buffer device
addresses, and a compute queue. No descriptor-heap or matrix extension is required.

## Contract exercised internally

- One logical device owns one compute-capable queue. Operations are serialized.
- Each buffer owns a dedicated allocation, bound at offset zero and mapped for
  its entire lifetime. We prefer ordinary host-coherent memory but support
  non-coherent memory with explicit whole-allocation cache maintenance. Optional
  AMD device-coherent and protected memory types are excluded.
- Upload/readback are checked CPU copies, not transfer-queue operations. No host
  mapping is exposed to the caller. Unwritten buffer contents are unspecified.
- Device addresses are non-owning numbers, never CPU pointers. They remain valid
  only while the owning buffer lives and only on that buffer's device.
- The shader is descriptor-free SPIR-V, with a compute entry named `main`. Its
  16-byte argument block contains a device address at byte 0, an element count at
  byte 8, and four padding bytes. This block is an example contract, not a universal
  kernel ABI; the backend supports a caller-specified push-constant byte count.
- Each dispatch records host/prior-compute → compute and compute → host memory
  dependencies, then submits and blocks until the queue is idle. Arguments are
  copied into the command buffer. Readback invalidates non-coherent CPU caches.
- Buffers and kernels retain their device, and devices retain their Vulkan instance.
  A caller must keep every allocation referenced by shader addresses alive during
  dispatch. The runtime cannot infer those references from arbitrary argument bytes.
- There is no timeout or asynchronous completion handle yet. If queue waiting
  reports a transient error, the backend continues draining until completion or
  device loss before reporting an error. It must not release pending command
  buffers or return permission to destroy allocations prematurely. Persistent
  wait failures can therefore block indefinitely. Device loss poisons execution;
  resource destruction remains allowed.

SPIR-V and shader memory accesses are trusted inputs. Checking a SPIR-V header is
not full validation or sandboxing: programs must use only enabled device features,
respect their pipeline layout, and make aligned, in-bounds, race-free accesses.

## Shader source and regeneration

The small example's source and generated binary are checked in under
`examples/shaders/`. No shader compiler runs during normal builds or tests.
The binary was generated with glslang **16.4.0**, targeting Vulkan 1.2, and validated
with SPIRV-Tools **1.4.357.0**:

```sh
glslangValidator -V --target-env vulkan1.2 examples/shaders/roundtrip.comp -o examples/shaders/roundtrip.spv
spirv-val --target-env vulkan1.2 examples/shaders/roundtrip.spv
```

The choice of GLSL for this test does not select the project's eventual shader
language. The runtime consumes SPIR-V; later experiments can use other compilers.

This is a correctness experiment, not a performance baseline: no device-local
staging, suballocation, pipeline caching, persistent command pools, or overlapping
submissions. Its purpose is to expose ownership and visibility decisions before
growing the public API.
