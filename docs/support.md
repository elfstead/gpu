# Current support boundary

Updated 2026-09-29; experimental C ABI 18. Pin a full source revision. This table
distinguishes implementation scope from native validation; it is not a list of
all hardware that might work.

| Area | Linux x86-64 / Vulkan | macOS arm64 / Metal |
|---|---|---|
| Native execution baseline | Vulkan 1.4 and the explicit features below | Apple Silicon/Apple7-family checks and Metal 4, macOS 26+ |
| Buffers, GPU addresses, compute, one-shot batches, barriers, copies, receipts | Implemented; current local contract suites | Implemented; native acceptance remains ABI 12 on Apple M4/macOS 26 |
| HOST views, caller-owned recording storage, compiled lists, split dependencies | Implemented; ABI 14–17 contract and reuse tests | UNSUPPORTED stubs; no current native revalidation |
| Images/heaps and raster | Narrow offscreen subset | UNSUPPORTED |
| Timing | Optional queue support; timed one-shot batches, not compiled lists | UNSUPPORTED |
| Executable inputs | Trusted compatible SPIR-V | Native MSL/metallib; optional SPIRV-Cross adapter |
| SDK installer | Native local-build `.so`, C header, tools, examples and manifest | Not packaged by this installer |

Vulkan requires buffer device addresses, timeline semaphores, synchronization2,
maintenance5, storageBuffer16BitAccess, descriptor heaps, device address commands
and shader untyped pointers with their required bits. Discovery is weaker than
execution: an enumerated device can reject creation. Raster additionally needs a
shared graphics/compute queue and dynamic rendering. Unified image layouts are
enabled when supported, not required. FP16 arithmetic is enabled when supported;
storage16 is not proof of arithmetic, subgroup or matrix support. Query the created
device, not just physical capabilities. There is no legacy emulation path.

Local physical evidence is one AMD Radeon RX 5700 XT with RADV; llvmpipe supplies a
second software implementation, not another GPU vendor. Current reports use Mesa
26.2.1. No new NVIDIA/Intel/other physical GPU, real device-loss, noncoherent hardware
or current Metal validation is claimed. Synthetic error/cache tests cover selected
contract branches, not those missing environments.

Raster supports scoped offscreen color plus optional D32 depth, indexed/non-indexed
indirect triangle list/strip, fixed fill/no-cull/full-target state and one sample;
no stencil or blending. ABI 18 adds explicit load/store and depth test/write/compare
state, plus opt-in index eligibility on buffers (Metal rejects that extra usage).
The migration checkpoint is not full M4 acceptance.
Image formats are RGBA8_UNORM, R32_FLOAT, RGBA16_FLOAT, RGBA16_UNORM and D32_FLOAT with
specific usage restrictions; query the exact description. Images have one mip,
layer and sample. No presentation, general rendering backend or image aliasing API.

The optional installed adapter pins Slang 2026.14.1 plus suitable SPIRV-Tools.
It supports a bounded structured scalar/vector/array/pointer and heap/stage-interface
subset, not arbitrary Slang or a project-owned language. Shader generation is not
needed to build/run supplied embedded artifacts. No global numerical policy,
accelerated matrix profile or general GGML/libplacebo backend is implied.

Calls on each device and its children require external serialization. There is one
queue per created device; no cross-queue scheduler, concurrent recording promise,
cancellation, recovery from loss, pointer tracing, shader sandbox or implicit
dependency inference. GPU-address lifetimes and race-free access are caller-owned.

Rust 1.85 is the declared minimum and a configured CI lane; current local acceptance
uses 1.97.1. Minimum-toolchain and hosted/self-hosted CI results must be reported
separately from configuration. The install path needs Git, Rust/Cargo, native linker
and Python 3; copied examples need C11, pkg-config and Python 3. These are not
bundled tools or a hermetic build environment.
