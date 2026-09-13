# Linear memory and compute baseline

The synchronous experiment below now runs on the [one-shot batch implementation](batches.md).
The asynchronous API separates recording, submission, and completion; this page
describes the implemented buffer/shader baseline and its blocking convenience example.
See [the design overview](design.md) for direction and [the experiment ledger](experiments.md)
for the current evidence and gaps.

The initial Rust round trip allocates one host-visible, GPU-addressable
buffer, uploads 4099 integers, dispatches `x = x * 3 + 7` twice, makes a partial CPU
update, and reads back and verifies every result. The count deliberately isn't a
multiple of the shader's 64-thread workgroup size.

```sh
cargo xtask compute
cargo test --locked gpu_roundtrip -- --ignored --nocapture
```

The C example runs three dispatches on the first eligible device and verifies a
partial CPU update between dispatches. It destroys the probe and device handles
before dispatch to test retained ownership. The Rust test runs on every eligible
device. Both need a Vulkan loader/driver (hardware or Mesa lavapipe) and fail if
none can execute. Discovery still requires only a Vulkan 1.1
instance implementation; execution requires the [modern Vulkan baseline](modern-baseline.md).
Descriptor heaps and address commands are required; matrix acceleration is optional.

## C API and ownership

The baseline uses these opaque types; batches/completions and graphics objects are
documented in their respective contracts:

| Object | Created from | Operations | Retains |
|---|---|---|---|
| `OgpuDevice` | Probe + stable device index | Create buffers and kernels | Vulkan instance |
| `OgpuBuffer` | Device + byte size | Write, read, query GPU address | Device |
| `OgpuKernel` | Device + SPIR-V + push byte count | Dispatch and wait | Device |

Each handle is destroyed exactly once. Destroying a parent handle does not destroy
the implementation object while children retain it. Device creation uses the
probe's original physical handles, not a fresh enumeration with potentially changed
indices. Independent devices may be used concurrently, but all calls involving a
device or any of its children must be externally serialized, including destruction.

The kernel API accepts copied argument bytes rather than a hard-coded integer
operation. The entry name remains `main`; ABI 5 accepts explicit
`groups_x, groups_y, groups_z`, with unused axes set to 1. Each count must be
nonzero and within its per-axis device limit. The shader declares its own local
workgroup size. The original experiment introduced execution at ABI 1; use the
current matching header/library after subsequent breaking changes. No
execution-facing Vulkan types are exposed.

## Memory and execution contract

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
  kernel ABI; the backend supports a caller-specified root-data byte count.
- Each blocking dispatch records host/prior-compute → compute and compute → host memory
  dependencies, then submits and waits on its device-owned timeline value. Arguments are
  copied into the command buffer. Readback invalidates non-coherent CPU caches.
- Buffers and kernels retain their device, and devices retain their Vulkan instance.
  A caller must keep every allocation referenced by shader addresses alive during
  dispatch. The runtime cannot infer those references from arbitrary argument bytes.
- The blocking helper has no timeout. If completion waiting
  reports a transient error, the backend continues draining until completion or
  device loss before reporting an error. It must not release pending command
  buffers or return permission to destroy allocations prematurely. Persistent
  wait failures can therefore block indefinitely. Device loss poisons execution;
  resource destruction and draining existing completions remain allowed.

SPIR-V and shader memory accesses are trusted inputs. Checking a SPIR-V header is
not full validation or sandboxing: programs must use only enabled device features,
respect their declared root layout, and make aligned, in-bounds, race-free accesses.
The backend creates layout-free descriptor-heap pipelines and copies roots with
`vkCmdPushDataEXT`. Existing descriptor-free Vulkan 1.2 SPIR-V remains valid input;
the executable's minimum SPIR-V environment is not the runtime's device baseline.

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

## Validation

With Khronos validation layers installed:

```sh
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation cargo xtask compute
```

The C compute task fails on reported validation errors, not just API return codes.
Hosted CI validates the checked-in SPIR-V; the manual modern-driver workflow runs
execution when a runner is provisioned. The modern backend now passes on both
RX 5700 XT / RADV and llvmpipe; see [hardware validation](hardware-validation.md).
The non-coherent memory selector and wait-error draining have unit coverage;
actual non-coherent cache behavior still needs a device exposing a suitable memory
type. Tests can exercise the explicit flush/invalidate calls on coherent memory,
but that is not equivalent to hardware coverage of non-coherent memory.

This is a correctness experiment, not a performance baseline. The later
[memory checkpoint](memory-transfers.md) adds device-local placement and staging
copies; runtime suballocation remains deferred. Executable/command preparation
lacks caching and persistent pools. This blocking example does not overlap
submissions; the asynchronous batch API permits multiple outstanding submissions.
