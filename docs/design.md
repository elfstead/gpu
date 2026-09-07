# Toward a Minimal Open GPU Compute and ML Interface

## Aaltonen’s “No Graphics API” as the basis for a post-OpenCL, open-CUDA-style runtime

### Executive summary

Sebastian Aaltonen’s 2025 essay **“No Graphics API”** argues that much of the complexity in Vulkan, Direct3D 12, Metal, and modern shader frameworks reflects historical GPU architecture rather than the hardware abstraction we would choose today. Modern GPUs have increasingly converged around generic caches, virtual memory, raw memory access, bindless resource addressing, programmable shader cores, and C-like pointer semantics. Aaltonen therefore proposes replacing much of the conventional graphics resource and binding model with a much simpler one:

**memory + pointers + shaders/kernels + queues + synchronization**, retaining special API objects only where hardware genuinely remains specialized.

For graphics, this is still complicated by textures, rasterization, render targets, index processing, blending, depth/stencil state, and presentation.

For **general compute and machine learning**, nearly all of those complications disappear.

The compute subset of Aaltonen’s proposal is consequently strikingly close to a minimal CUDA-like runtime:

```text
device
memory
GPU virtual addresses

kernel/module
dispatch
indirect dispatch

queue
barrier
event/semaphore

copy
capability query
```

A modern ML-capable version would add only a few important machine-level capabilities:

```text
subgroups / SIMD-lane operations
shared/workgroup memory
atomics
cooperative matrix operations
FP16 / BF16 / FP8 / INT8 / perhaps sub-byte types
multi-device / peer memory
```

Crucially, it should **not** make tensors, neural-network operators, attention, convolution, or ML graphs part of this low-level API. Those belong above it.

The resulting architecture would separate:

```text
ML frameworks
    ↓
graph/tensor compilers
    ↓
optimized kernel/math libraries
    ↓
kernel language + portable IR
    ↓
minimal open compute ABI
    ↓
Vulkan initially; native vendor drivers eventually
    ↓
GPU hardware
```

A large fraction of this can already be prototyped over Vulkan. Buffer Device Address gives shaders genuine 64-bit addresses; untyped pointers make the shader memory model substantially more C-like; Synchronization2 supplies low-level execution/memory dependencies; Shader Objects can reduce pipeline machinery; cooperative matrices expose matrix acceleration; FP8 support is explicitly motivated by ML; and newer Vulkan work increasingly allows commands and descriptor machinery themselves to operate in terms of memory addresses rather than traditional resource objects.

The idea is not without precedent. VUDA implements a CUDA-runtime-like interface over Vulkan. Kompute supplies a simpler Vulkan compute abstraction. IREE has a HAL explicitly described by its developers as almost a compute-only Vulkan API. Intel’s Level Zero is remarkably close to the proposed low-level interface philosophically. OpenCL can itself now be layered over Vulkan through clvk/clspv. SYCL has migrated toward pointer-based Unified Shared Memory and even has an experimental Vulkan backend in AdaptiveCpp. Several production and experimental ML systems independently maintain Vulkan compute runtimes.

What appears to remain missing is the **specific combination**:

> A deliberately tiny, public, vendor-neutral, pointer-first GPU compute ABI designed from scratch around modern hardware, useful as a compiler target rather than a framework, with Vulkan serving as its deployable cross-vendor implementation.

That is the potentially interesting project.

---

# 1. Starting point: Aaltonen’s “No Graphics API”

Aaltonen’s thesis is not literally that software can eliminate the interface between applications and GPU drivers. Rather, it is that a modern interface need not expose most of the abstractions accumulated by graphics APIs over several generations.

His prototype API is around 150 lines and is explicitly presented as an experiment in how much conventional graphics API structure can disappear. He notes that a sufficiently extended Vulkan 1.4 implementation can perform the same basic operations in practice, but through considerably more API machinery.

The essential conceptual shift is:

```text
traditional graphics API:

objects
bindings
views
layouts
descriptor sets
pipeline layouts
resource states
command configuration

                    ↓

Aaltonen:

memory
pointers
small opaque descriptors where unavoidable
shader code
commands
hazards
```

## 1.1 Memory becomes fundamental

Aaltonen explicitly takes inspiration from CUDA.

Instead of first creating a buffer object, determining which memory can back it, allocating that memory, binding the two, creating a view, and finally binding that view to a shader, he proposes something approximately equivalent to:

```cpp
void* p = gpuMalloc(bytes);
gpuFree(p);
```

For modern CPU-visible GPU memory, the allocation can have both:

```text
CPU virtual address
GPU virtual address
```

allowing the CPU to populate GPU-visible data structures directly. Large persistent allocations can instead live in private GPU memory and receive data through copies. Aaltonen describes this as combining CUDA’s allocation model with CPU-mapped GPU memory available through UMA or PCIe Resizable BAR.

This is particularly significant for compute because **buffers cease to be a first-class abstraction**.

A buffer is simply:

```text
allocation + address + size
```

The application can construct arrays, graphs, trees, linked lists, argument structures, allocator metadata, and arbitrary pointer-linked structures using ordinary data-layout rules.

## 1.2 Shader arguments become ordinary data structures

Aaltonen’s shader binding model replaces elaborate binding APIs with a root GPU pointer.

For example:

```cpp
struct Arguments {
    float* input;
    float* output;
    uint32_t count;
};

Arguments* args = allocate_gpu_visible<Arguments>();

args->input  = input_gpu;
args->output = output_gpu;
args->count  = count;

gpuDispatch(cb, gpu_address(args), grid);
```

The corresponding kernel conceptually receives:

```cpp
kernel void run(const Arguments* args)
{
    float x = args->input[thread_id];
    ...
}
```

The argument structure may itself contain pointers to other structures.

That gives the GPU an ordinary graph of memory:

```text
root pointer
    │
    ├── input ───────────→ allocation
    │
    ├── output ──────────→ allocation
    │
    └── parameters ──────→ another structure
                                │
                                └── ...
```

Aaltonen argues that root-binding APIs, descriptor sets for buffers, and much of the conventional buffer taxonomy can therefore disappear.

This aspect is much closer to **CUDA or normal systems programming** than to GLSL-era graphics programming. Aaltonen explicitly contrasts pointer-capable CUDA, OpenCL, and Metal with HLSL/GLSL’s historically restricted memory models, and associates CUDA’s composability with its unusually strong library ecosystem.

---

# 2. Extracting the compute subset

A useful way to reinterpret the article is to classify every part of Aaltonen’s prototype as:

- **core compute**
- **optional compute facility**
- **graphics-only**

Aaltonen’s complete prototype ends with an API approximately containing memory, textures, pipelines, graphics state, queues, semaphores, copies, barriers, dispatches, render passes, draws, and mesh dispatches.

The compute version is dramatically smaller.

| Aaltonen concept | Compute/ML disposition | Reason |
|---|---|---|
| `gpuMalloc`, `gpuFree` | **Keep** | Fundamental memory model |
| host→GPU address conversion | **Keep or simplify** | Needed when CPU/GPU virtual addresses differ |
| GPU-only/readback memory classes | **Keep** | Important for discrete GPUs and CPU↔GPU transfer |
| texture allocation/view descriptors | **Optional** | Useful for image workloads, unnecessary for core ML |
| global texture heap | **Optional** | Useful for vision/image processing, not fundamental |
| compute pipeline | **Keep, simplify to module/kernel** | Executable code abstraction |
| specialization constants | **Keep** | Important for optimized kernel variants |
| command queue | **Keep** | Hardware scheduling |
| command buffer | **Probably internal/optional** | Could expose batches rather than Vulkan-style recording objects |
| timeline semaphore | **Keep conceptually** | Host/device and cross-queue synchronization |
| memory copy | **Keep** | Essential |
| generic barrier | **Keep** | Essential memory/execution dependency |
| memory-word signal/wait | **Desirable** | Powerful low-level synchronization primitive |
| dispatch | **Keep** | Core operation |
| indirect dispatch | **Keep** | GPU-driven execution |
| graphics shader stages | **Remove** | Compute has one kernel model |
| vertex/index inputs | **Remove** | Graphics-specific |
| blend/depth/stencil state | **Remove** | Graphics-specific |
| rasterizer state | **Remove** | Graphics-specific |
| render passes | **Remove** | Graphics-specific |
| draw calls | **Remove** | Graphics-specific |
| mesh shaders | **Remove** | Graphics-specific |
| presentation/swapchain | **Remove** | Graphics-specific |

The result is startlingly close to:

```cpp
Device deviceOpen(...);

void* deviceMalloc(Device, size_t);
void  deviceFree(Device, void*);

Module moduleLoad(Device, ByteSpan ir);
Kernel kernelGet(Module, const char* name);

Queue queueCreate(Device);

dispatch(
    Queue,
    Kernel,
    Grid,
    Block,
    const void* argumentData
);

dispatchIndirect(...);

memcpyAsync(...);
barrier(...);

eventSignal(...);
eventWait(...);
```

That is no longer recognizably a graphics API.

It is essentially a **GPU machine interface**.

---

# 3. What ML adds that Aaltonen does not

Simply extracting Aaltonen’s compute functions is not sufficient to make a competitive ML substrate.

CUDA’s basic kernel model succeeds because the language/compiler exposes the GPU’s important computational structure as well as its memory.

A useful open compute interface therefore needs several additional pieces.

## 3.1 Workgroups, subgroups, and local memory

The kernel execution model needs explicit notions corresponding roughly to CUDA’s:

```text
grid
block
thread

warp
shared memory
barrier
```

In vendor-neutral terminology:

```text
dispatch grid
workgroup
invocation

subgroup
workgroup/local memory
workgroup/subgroup synchronization
```

These belong primarily to the **kernel ABI and IR**, rather than to the host runtime API.

## 3.2 Matrix hardware

This is the most important ML-specific addition.

Modern ML performance depends heavily on specialized matrix units:

```text
NVIDIA Tensor Cores
AMD matrix instructions
Intel XMX
mobile GPU matrix hardware
```

An open interface should not expose any one vendor’s instruction names.

Vulkan’s `VK_KHR_cooperative_matrix` demonstrates a plausible abstraction: multiple shader invocations cooperatively operate on matrix fragments and perform matrix multiply-accumulate using appropriate underlying hardware. Khronos explicitly states that ordinary subgroup operations are often insufficient to achieve optimal matrix multiplication and standardized cooperative matrices because multiple vendors had converged on similar facilities.

This suggests an IR-level primitive such as:

```text
matrix_load
matrix_mma
matrix_store
```

with a capability query describing supported:

```text
dimensions
element types
accumulator types
scopes
layouts
```

Later Vulkan extensions already reveal where this abstraction needs to grow: ML workloads frequently require reductions, conversions, dequantization, convolution transformations, and Flash-Attention-like manipulations around matrix operations, not merely an isolated GEMM.

## 3.3 ML numeric types

A modern target must support at least:

```text
FP32
FP16
BF16
FP8 variants
INT8
```

and potentially standardized ways to expose efficient INT4/sub-byte computation.

Vulkan’s `VK_EXT_shader_float8` is explicitly motivated by ML throughput and introduces E4M3 and E5M2 FP8 formats while integrating them with cooperative-matrix usage.

These should again be **kernel-language/IR capabilities**, not high-level ML operators.

## 3.4 Multi-device memory and communication

For serious training, an eventual interface must describe:

```text
device-local allocation
peer accessibility
GPU↔GPU transfers
shared/imported memory
topology information
events across devices
```

Collectives such as all-reduce probably belong in optimized libraries above the base ABI rather than the kernel API itself.

---

# 4. What should *not* be in the low-level ML API

The most important design decision may be what to exclude.

The proposed interface should not contain operations such as:

```cpp
runTransformer(...)
attention(...)
conv2d(...)
gemm(...)
tensorReshape(...)
```

It should arguably not even contain a first-class **Tensor** object.

A tensor is a semantic interpretation of memory:

```text
pointer
+
shape
+
strides
+
element type
+
layout
```

That information is extremely valuable to an ML compiler, but does not necessarily need to be a kernel-driver resource.

This produces an important distinction.

### Machine-level model

```text
allocation
address
kernel
queue
event
matrix instruction
```

### ML/compiler model

```text
tensor
shape
layout
GEMM
attention
fusion
quantization
graph
```

Keeping these separate allows the low-level API to remain useful for:

```text
ML
scientific compute
simulation
compression
cryptography
signal processing
database kernels
rendering support algorithms
anything not yet invented
```

CUDA’s longevity provides a useful precedent: its fundamental execution model remains general even as the dominant workload has shifted dramatically.

---

# 5. Vulkan as the implementation substrate

The reason this idea is substantially more plausible now than it would have been around Vulkan 1.0 is that Vulkan itself has accumulated many capabilities that undermine its original object/binding model.

The public interface can therefore hide Vulkan while using modern Vulkan as a portable machine backend.

## 5.1 Memory and pointers: Buffer Device Address

`VK_KHR_buffer_device_address`, promoted into Vulkan 1.2 and mandatory in Vulkan 1.3, allows an application to obtain a 64-bit GPU address for buffer memory. Khronos itself describes the feature as effectively providing **pointers to buffer memory in shaders**. Addresses can be stored inside other GPU allocations, allowing pointer-linked structures.

Thus:

```text
open_compute_malloc()
```

could internally mean:

```text
VkDeviceMemory
    +
one large VkBuffer
    +
suballocation
    +
VkDeviceAddress
```

while the public application sees only its allocation/address.

This is especially clean for compute because, unlike graphics, it does not need to map arbitrary allocations into renderable image objects.

## 5.2 Device-address commands

A remaining historical wart is that even after shaders gained addresses, many Vulkan commands continued accepting:

```text
VkBuffer + offset
```

rather than addresses.

`VK_KHR_device_address_commands` explicitly identifies this inconsistency and provides variants of existing operations that consume address ranges instead of buffer objects, including transfer, synchronization, indirect-command, and other functionality. Khronos describes choosing device addresses over retrofitting newer functionality back toward buffer handles because addresses are becoming more widely used.

This is unusually strong evidence for the pointer-first direction.

## 5.3 Untyped pointers

Traditional SPIR-V carries strongly typed pointers inherited largely from shader-language assumptions.

`VK_KHR_shader_untyped_pointers` introduces a more flexible pointer model. Its rationale explicitly says physical memory itself is not inherently typed, notes the mismatch with modern LLVM opaque pointers and languages containing byte-addressable memory, and aims to make translation from general-purpose languages easier.

For an open compute language, this matters greatly.

The ideal model is closer to:

```cpp
struct Args {
    SomeStruct* x;
    uint8_t* scratch;
};
```

than to:

```text
descriptor binding 3 is StorageBuffer<SomeStruct>
descriptor binding 4 is StorageBuffer<uint>
```

## 5.4 Kernel arguments

A wrapper can place the root GPU address in a tiny amount of root/push data.

Thus the Vulkan implementation internally performs something akin to:

```text
push 64-bit address
vkCmdDispatch(...)
```

while the public interface simply exposes:

```cpp
dispatch(queue, kernel, root_pointer, grid);
```

The root structure can recursively reference all other data.

This precisely realizes Aaltonen’s argument that data-layout description can disappear from the host API when the CPU and shader share an ABI.

## 5.5 Kernel objects rather than graphics pipelines

A conservative backend can map a kernel to a Vulkan compute pipeline.

A more modern backend can potentially use `VK_EXT_shader_object`, which provides compiled shader-stage objects independently of `VkPipeline`; compute shaders are explicitly supported.

Either way, the public API need expose only:

```text
module
kernel
```

not:

```text
descriptor set layout
pipeline layout
descriptor pool
descriptor set
compute pipeline
```

## 5.6 Synchronization

Aaltonen argues that conventional per-resource synchronization often poorly represents what hardware actually needs: producer/consumer ordering plus appropriate cache visibility. His prototype consequently exposes stage/hazard barriers rather than elaborate resource-state transitions.

Vulkan’s `VK_KHR_synchronization2`, core in Vulkan 1.3, already reorganizes barriers around combined stage/access descriptions and cleaner dependency structures.

For compute-only execution, this mapping is simpler than for graphics because there are no render-target image layouts to manage.

A public abstraction might therefore be as small as:

```cpp
barrier(queue,
        COMPUTE_WRITE,
        COMPUTE_READ);
```

or an even lower-level producer/consumer stage model.

Aaltonen’s more radical memory-address-based signal/wait operations cannot necessarily be mapped perfectly onto portable Vulkan, so this is one area where a Vulkan prototype may be an approximation rather than a proof of the ideal native interface.

## 5.7 Cooperative matrices

`VK_KHR_cooperative_matrix` supplies the essential bridge to tensor/matrix acceleration.

An open compute compiler could therefore lower its generic matrix primitive to Vulkan cooperative-matrix SPIR-V while retaining alternative backends for:

```text
CUDA MMA/WGMMA
AMD native matrix instructions
Intel native matrix hardware
Metal facilities
```

## 5.8 Descriptor heaps become mostly irrelevant for pure ML

`VK_EXT_descriptor_heap` is extremely relevant to Aaltonen’s graphics proposal because it eliminates Vulkan descriptor sets and pipeline layouts, makes descriptor memory explicitly application-controlled, and treats descriptor heaps as memory.

But there is an interesting consequence of focusing on compute:

**we may need almost none of it.**

Ordinary ML tensors can simply be pointers.

Descriptor heaps remain relevant for:

```text
sampled images
storage images
samplers
some hardware-special resources
```

but those could be an optional interoperability/image module rather than part of the fundamental compute ABI.

This makes the compute API even smaller than Aaltonen’s graphics API.

---

# 6. A plausible minimal API

At the conceptual level, the host API could perhaps be no more complicated than:

```cpp
Device device_open(DeviceSelector);

Allocation alloc(
    Device,
    size_t bytes,
    MemoryKind
);

void free(Allocation);

Module module_load(
    Device,
    ByteSpan portable_ir
);

Kernel kernel_get(
    Module,
    String name
);

Queue queue_create(Device);

dispatch(
    Queue,
    Kernel,
    Grid,
    Workgroup,
    const void* args,
    size_t args_size
);

dispatch_indirect(...);

copy(...);

barrier(...);

Event event_create(...);
signal(...);
wait(...);

Capabilities query_capabilities(Device);
```

The notable property is what is **not present**:

```text
buffer creation
buffer views
descriptor pools
descriptor sets
binding layouts
pipeline layouts
tensor objects
operator objects
graphs
graphics state
```

Internally, an allocation object might exist for lifetime and bounds information while its actual kernel-facing identity is simply its device address.

A lower-level C ABI could even separate the allocation handle from the pointer:

```cpp
oc_allocation a;
oc_device_ptr p;

oc_alloc(device, bytes, &a, &p);
```

This avoids forcing implementations where addresses alone are sufficient to carry lifetime information.

---

# 7. The layer model

The project becomes clearer if “open ML API” is decomposed into distinct layers.

## Layer 0 — hardware

```text
NVIDIA GPU
AMD GPU
Intel GPU
Apple GPU
mobile GPUs
eventually other accelerators
```

Vendor-specific ISA and machine architecture live here.

---

## Layer 1 — minimal open compute ABI

This is the proposed missing layer.

Responsibilities:

```text
device discovery
memory allocation
virtual addresses
copies
module loading
kernel lookup
kernel dispatch
queues
events
synchronization
capability discovery
peer memory / device topology
```

It should know almost nothing about ML.

Possible initial implementation:

```text
Open Compute ABI
        ↓
     Vulkan
```

Possible eventual implementations:

```text
             Open Compute ABI
              /    |    |    \
         Vulkan   CUDA ROCm  Metal
```

or ultimately:

```text
Open Compute ABI
        ↓
native vendor driver
```

---

## Layer 2 — kernel ABI, portable IR, and programming language

This layer describes computation inside a dispatch:

```text
invocation IDs
workgroups
subgroups
local memory
pointers
atomics
barriers
numeric types
matrix operations
```

A SPIR-V-derived representation is an obvious initial candidate because Vulkan already consumes it, but the public compute ABI should ideally not be permanently coupled to every historical constraint of Vulkan shader SPIR-V.

Possible compilation flows include:

```text
C++-like compute language
           ↓
         LLVM IR
           ↓
portable GPU IR
           ↓
Open Compute ABI
```

or:

```text
Triton / MLIR
      ↓
portable GPU IR
      ↓
Open Compute ABI
```

This is the layer analogous to the relationship of PTX and CUDA’s compiler ecosystem.

---

## Layer 3 — optimized primitive libraries

This includes:

```text
GEMM
convolution
attention
FFT
reductions
sorting
quantization
collectives
```

Equivalent ecosystem roles today are occupied by things such as cuBLAS, CUTLASS, cuDNN, NCCL, vendor libraries, and specialized open kernels.

A successful open CUDA alternative requires this layer to become excellent.

A beautiful runtime API alone cannot compete with a decade of heavily optimized kernels.

---

## Layer 4 — tensor and graph compiler/runtime

This is where ML semantics belong:

```text
Tensor
shape
stride/layout
operator
fusion
graph scheduling
memory planning
automatic kernel selection
quantization transformations
device placement
```

Projects such as IREE belong approximately here while also reaching into lower layers.

This layer can compile:

```text
StableHLO
ONNX
MLIR
framework-specific graphs
```

into executable kernels and memory schedules.

---

## Layer 5 — ML frameworks

```text
PyTorch
JAX
TensorFlow
ONNX runtimes
higher-level inference/training frameworks
```

Applications should generally live here and never see the low-level ABI.

The complete model is:

```text
┌─────────────────────────────────────┐
│ 5. Frameworks                       │
│ PyTorch / JAX / TensorFlow / etc.   │
├─────────────────────────────────────┤
│ 4. Tensor / graph compiler          │
│ IREE / XLA / MLIR pipelines / etc.  │
├─────────────────────────────────────┤
│ 3. Optimized compute libraries      │
│ GEMM / attention / collectives      │
├─────────────────────────────────────┤
│ 2. Kernel language + portable IR    │
│ pointers / SIMD / matrices / types  │
├─────────────────────────────────────┤
│ 1. Minimal Open Compute ABI         │
│ memory / kernel / queue / event     │
├─────────────────────────────────────┤
│ 0. Vulkan / native vendor drivers   │
├─────────────────────────────────────┤
│ Hardware                            │
└─────────────────────────────────────┘
```

This separation is probably the most important architectural conclusion of the discussion.

---

# 8. Prior art

None of the individual ingredients is unprecedented.

The interesting question is whether an existing project already occupies **Layer 1 in precisely this form**.

At present, the answer appears to be: not quite.

## 8.1 VUDA — CUDA API semantics over Vulkan

VUDA is perhaps the most literal predecessor.

It is a header-only library built on Vulkan that exposes an interface intended to conform, as far as practical, to the CUDA Runtime API.

Its significance is straightforward:

> CUDA-shaped host semantics can be implemented over Vulkan.

But VUDA begins from compatibility with the existing CUDA Runtime API rather than asking what a minimal post-OpenCL interface should look like on current hardware.

It also predates much of the Vulkan evolution that now makes the pointer-first model significantly cleaner.

Thus VUDA is strong **proof of architectural feasibility**, but not necessarily the desired design endpoint.

---

## 8.2 Kompute — making Vulkan compute usable

Kompute was created specifically because projects including ML runtimes repeatedly had to construct large amounts of identical Vulkan compute infrastructure.

Its stated motivation is that Vulkan provides excellent low-level cross-vendor GPU access but requires substantial boilerplate before an application reaches actual compute work. Kompute therefore supplies higher-level concepts for memory, algorithms, sequences, queues, and asynchronous compute.

Its conceptual model remains closer to:

```text
Tensor
Algorithm
Sequence
```

than:

```text
pointer
kernel
queue
```

So Kompute is best interpreted as:

> a friendlier Vulkan compute framework,

whereas the proposed project is:

> an alternative low-level GPU abstraction that happens to use Vulkan internally.

---

## 8.3 IREE — perhaps the most important architectural precedent

IREE has a Hardware Abstraction Layer that manages:

```text
devices
allocations
executables
command buffers
dispatch
synchronization
```

Its own design documentation makes an unusually revealing statement: the IREE HAL is designed **almost one-to-one with a compute-only Vulkan API**. IREE then leverages its compiler to perform allocation planning, lifetime analysis, synchronization, and scheduling that applications would otherwise perform manually.

This makes IREE perhaps the closest serious architectural neighbor.

But the HAL is primarily an internal abstraction serving IREE’s compiler/runtime architecture rather than a deliberately standardized, tiny, human-facing cross-vendor GPU ABI.

A useful interpretation is:

> **IREE demonstrates that a compute-oriented hardware layer below ML compilation is valuable; the proposed API asks whether that layer should exist independently as a public standard.**

---

## 8.4 oneAPI Level Zero — almost the desired API philosophy

Intel’s Level Zero is extraordinarily relevant.

Its specification describes its purpose as providing a **direct-to-metal interface for accelerator devices**, intended to supply explicit low-level controls to higher-level runtimes and libraries.

Its responsibilities include:

```text
device discovery
memory allocation
peer communication
kernel submission
asynchronous execution
synchronization
metrics
```

and the specification explicitly says that higher-level applications generally should not need to use it directly. It was influenced by Vulkan and OpenCL but is intended to evolve independently.

Conceptually, therefore:

```text
Level Zero ≈ the kind of layer being proposed
```

The problem is ecosystem reach. The current Level Zero loader release information lists Intel’s GPU runtime as its supported runtime implementation.

So Level Zero answers:

> What could a modern low-level compute-runtime API look like?

but not yet:

> What low-level API can serve NVIDIA, AMD, Intel, mobile GPUs, and other Vulkan devices today?

That makes Level Zero an important specification to study rather than a complete solution.

---

## 8.5 OpenCL-on-Vulkan: clvk and clspv

The ecosystem has also approached the problem from the compatibility direction.

`clspv` compiles OpenCL C into Vulkan-compatible compute SPIR-V.

`clvk` implements the OpenCL 3.0 host API on top of Vulkan, translating OpenCL queue operations, buffers, synchronization, and kernel launches into corresponding Vulkan facilities.

Khronos specifically notes that modern Vulkan features such as Buffer Device Address and variable pointers help represent OpenCL’s pointer-oriented compute model.

This proves something important:

```text
OpenCL semantics
       ↓
     Vulkan
```

is viable.

But it preserves OpenCL’s existing API rather than asking what should replace OpenCL if legacy compatibility were irrelevant.

---

# 9. SYCL and the return of pointers

SYCL is also informative because its evolution independently moves toward the same underlying machine model.

SYCL 2020 introduced **Unified Shared Memory**, explicitly described as a pointer-based alternative to the older buffer programming model. It permits ordinary pointer arithmetic and distinguishes host, device, and shared allocations.

That is strongly aligned with the Aaltonen/CUDA memory philosophy.

Even more interestingly, AdaptiveCpp now has an **experimental Vulkan compute backend**. Its documentation says Vulkan Buffer Device Address is required specifically so the runtime can implement device USM and return device pointers to users.

This forms another path:

```text
SYCL
 ↓
AdaptiveCpp
 ↓
Vulkan
```

However, SYCL occupies a substantially higher programming-language/runtime layer than the tiny ABI proposed here.

That reinforces the proposed separation:

```text
SYCL-like language/runtime
          ↓
minimal compute ABI
```

rather than making the ABI itself equivalent to SYCL.

---

# 10. Existing Vulkan ML runtimes

Numerous ML projects independently prove that Vulkan is already viable as a compute backend.

## ExecuTorch

PyTorch’s ExecuTorch has an actively developed Vulkan backend using a library of GPU compute shaders. It supports FP32/FP16 inference, dynamic shapes, quantized linear layers, and other ML-oriented functionality, primarily targeting Android but usable wherever the backend is supported.

## ncnn

Tencent’s ncnn has long supported Vulkan inference across multiple GPU vendors and platforms. Its Vulkan implementation maintains its own memory allocators, pipelines, command infrastructure, shader library, and device abstraction—the exact sort of repeated infrastructure that motivates a standardized compute substrate.

## VKML

A newer experimental Rust project, VKML, implements ONNX inference directly on Vulkan 1.4 and experiments with heterogeneous NVIDIA/AMD/Intel execution. Its roadmap explicitly includes cooperative matrices and FP8. Its published project benchmarks also illustrate a central lesson: low dispatch overhead alone is insufficient; matrix-heavy workloads can badly lose to CUDA when the Vulkan path fails to use comparable matrix acceleration.

That is perhaps the clearest practical evidence for the layered design:

```text
runtime simplicity
       ≠
competitive ML performance
```

The compiler and optimized-kernel layers matter at least as much.

---

# 11. Taichi and portable compute languages

Taichi demonstrates a different approach: give the programmer a high-level portable kernel language and compile it to multiple compute backends, including CUDA and Vulkan.

It therefore belongs above the proposed low-level ABI:

```text
Taichi-like language
       ↓
portable compiler
       ↓
minimal compute ABI
```

rather than competing directly with it.

This also suggests an important project criterion:

**the Layer-1 API should be easy for compiler runtimes to target, not merely pleasant for humans to call manually.**

---

# 12. Vulkan’s own emerging ML direction: a competing philosophy

Recent Vulkan development introduces a particularly interesting alternative.

Arm’s `VK_ARM_tensors` adds an actual Vulkan tensor resource type because machine-learning workloads operate on high-dimensional structured data and because explicit tensor semantics may enable efficient interoperability with ML accelerators.

`VK_ARM_data_graph` then introduces data-graph pipelines designed to execute dataflow graphs such as neural networks.

Qualcomm has built on this model with extensions allowing data-graph execution to target QNN models across Hexagon NPUs and Adreno GPUs.

This gives Vulkan a trajectory roughly like:

```text
Buffer
Image
Tensor

Graphics Pipeline
Compute Pipeline
Data Graph Pipeline
```

That is almost philosophically opposite to the minimal compute proposal.

### Vulkan ML philosophy

Expose semantic resource types so drivers and heterogeneous accelerators know what the data represents.

### Aaltonen-derived philosophy

Expose memory and execution primitives; let compilers and libraries provide higher-level semantics.

Neither is universally superior.

For **programmable GPUs**, the pointer-first model is exceptionally attractive.

For **heterogeneous execution involving restrictive NPUs**, a tensor/data-graph representation may carry information that the lower-level hardware genuinely requires.

This suggests that the two models may ultimately coexist:

```text
                ML compiler
                  /     \
                 /       \
     generic GPU path    NPU/data-graph path
             ↓                 ↓
     Open Compute ABI      Tensor/Graph ABI
             ↓                 ↓
            GPU               NPU
```

The mistake would be forcing the GPU path to become graph-specific merely because some accelerators require graph semantics.

---

# 13. Direct relationship to Aaltonen’s article

The proposed compute API is therefore not merely “inspired by” No Graphics API.

It can be understood as a **strict projection of it**.

Aaltonen:

```text
modern GPU
=
generic memory machine
+
programmable SIMD
+
some irreducibly special graphics hardware
```

Compute projection:

```text
modern compute GPU
=
generic memory machine
+
programmable SIMD
```

The special graphics half simply vanishes.

His arguments map almost mechanically:

### Aaltonen: buffers should become pointers

Compute API:

```text
Yes — make this fundamental.
```

### Aaltonen: data bindings should become a pointer to an application-defined root structure

Compute API:

```text
Yes — make this the kernel ABI.
```

### Aaltonen: shaders should use ordinary pointer-capable language semantics

Compute API:

```text
Yes — this becomes the programming-language/IR requirement.
```

### Aaltonen: compute pipeline creation can become essentially shader-IR compilation

Compute API:

```text
Yes — rename this Module/Kernel.
```

### Aaltonen: resource-state barriers should become execution/memory hazards

Compute API:

```text
Yes — and compute avoids much of the image-layout complication.
```

### Aaltonen: one kernel-like programming model is more composable than proliferating shader stages

Compute API:

```text
Already true by construction.
```

### Aaltonen: textures remain special

Compute API:

```text
Make them optional rather than fundamental.
```

### Aaltonen: rasterization remains special

Compute API:

```text
Delete the entire category.
```

Thus the compute version is arguably an even purer realization of his argument than the graphics API itself.

---

# 14. What Vulkan can and cannot prove

A Vulkan implementation would be a legitimate prototype, not merely a toy wrapper.

It can test:

```text
API ergonomics
ABI design
pointer-heavy kernel programming
allocation strategies
CPU dispatch overhead
kernel compilation model
argument passing
indirect execution
cross-vendor portability
cooperative-matrix portability
synchronization abstractions
compiler integration
```

Aaltonen himself now has an experimental Vulkan 1.4 implementation of his ideas, and another project, `no_gfx`, also began as a recreation of the article’s proposed API over Vulkan.

What such a prototype cannot prove is whether a **native** implementation could eliminate all underlying Vulkan costs.

Internally, Vulkan may still require:

```text
VkDeviceMemory bookkeeping
VkBuffer creation
command buffer machinery
pipeline/shader compilation
driver synchronization structures
```

The wrapper can hide these but cannot make them disappear.

Likewise, some hypothetical ideal synchronization operations—particularly waiting on arbitrary GPU memory values—cannot necessarily be represented exactly through portable Vulkan.

So the progression should be interpreted as:

```text
Phase 1:
prove the programming model over Vulkan

Phase 2:
measure where Vulkan's abstraction leaks

Phase 3:
determine whether those leaks justify native driver support
```

---

# 15. What appears genuinely novel

The individual ideas are not novel:

```text
pointer memory               → CUDA / SYCL USM / Vulkan BDA
CUDA-like Vulkan wrapper     → VUDA
simple Vulkan compute        → Kompute
compute HAL                  → IREE
low-level accelerator ABI    → Level Zero
OpenCL implemented on Vulkan → clvk
portable kernel compiler     → Taichi / SYCL / others
matrix abstraction           → cooperative matrices
ML directly on Vulkan        → ncnn / ExecuTorch / VKML / others
```

The potentially novel contribution is **putting the boundary in a different place**.

Specifically:

> Define the smallest modern GPU-compute machine that compiler and runtime authors can agree on, expose it as a stable public ABI, and deliberately refuse to turn it into either a legacy heterogeneous-compute framework or an ML graph API.

That boundary would sit below IREE/SYCL/Taichi/Triton-like systems but above Vulkan/CUDA/ROCm/Metal drivers.

Conceptually:

```text
                  today

        IREE       SYCL       Taichi
          │          │          │
       Vulkan     Level Zero   CUDA/Vulkan/...
          │
        driver


                 proposed

        IREE       SYCL       Taichi       Triton
          \          |          |           /
           \         |          |          /
            ─── Minimal Open Compute ABI ───
                    /    |    \
                 Vulkan CUDA  Metal/...
```

The project succeeds if higher-level systems can target **one tiny compute machine** without inheriting Vulkan’s graphics-oriented object model or OpenCL’s historical API design.

---

# 16. Design principles

The discussion suggests the following principles.

### 1. Memory is memory

Do not create separate API abstractions for every interpretation of linear bytes.

Prefer:

```text
address + size
```

over:

```text
uniform buffer
storage buffer
structured buffer
typed buffer
argument buffer
...
```

### 2. The kernel owns data interpretation

Data layout belongs in shared type definitions and compiler metadata, not duplicated host-side binding declarations.

### 3. Kernel arguments are ordinary structures

One root pointer should be sufficient for arbitrarily complex argument graphs.

### 4. Keep the host ABI tiny

If a feature can live in:

```text
compiler
library
kernel IR
```

it probably should not expand the runtime ABI.

### 5. Make ML important, not fundamental

Support the hardware features ML needs—matrix units and low-precision arithmetic—without embedding today's neural-network architecture into the machine model.

### 6. Separate portable semantics from optimal implementations

A generic matrix operation should have a portable meaning while allowing vendor-specific lowering.

### 7. Make compiler runtimes first-class users

The primary customer may ultimately be IREE, Triton, SYCL, Taichi, graph compilers, and framework backends rather than humans hand-writing dispatch calls.

### 8. Vulkan is a bootstrap mechanism, not necessarily the specification

The API should not merely rename Vulkan functions.

Its semantics should be designed independently and mapped onto Vulkan where possible.

---

# 17. A reasonable prototype

A first implementation could deliberately target a narrow modern Vulkan profile.

## Required baseline

```text
Vulkan 1.3+
Buffer Device Address
Synchronization2
SPIR-V physical storage buffers
timeline synchronization
compute dispatch
```

## Preferred modern extensions

```text
VK_KHR_shader_untyped_pointers
VK_KHR_device_address_commands
VK_EXT_shader_object
VK_KHR_cooperative_matrix
VK_EXT_shader_float8
```

Optional graphics/image interoperability could additionally use:

```text
VK_EXT_descriptor_heap
```

The first milestone should not be running an entire transformer.

It should establish the basic machine:

```text
malloc
pointer-containing structs
kernel load
kernel dispatch
barrier
copy
event
```

Then test:

```text
vector operations
reduction
prefix scan
pointer chasing
GEMM
cooperative-matrix GEMM
fused small operator chains
indirect GPU-generated dispatch
```

Only after that should a thin backend for something like IREE, Triton, or a tiny tensor compiler be attempted.

---

# 18. Final assessment

There is substantial prior art, but rather than making the idea redundant, the prior art clarifies the gap.

**VUDA** shows that CUDA-like host semantics can sit over Vulkan.  
**Kompute** shows demand for a compute-specific simplification of Vulkan.  
**IREE** shows the value of a low-level compute HAL beneath an ML compiler.  
**Level Zero** shows that a direct-to-metal kernel/memory/queue API is a coherent modern design.  
**clvk/clspv** show that a complete existing compute programming model can be implemented over Vulkan.  
**SYCL USM and AdaptiveCpp** show modern heterogeneous programming moving back toward pointers and that Vulkan can support such a model.  
**ncnn, ExecuTorch, VKML, and others** show repeated independent construction of Vulkan ML runtimes.  
**Vulkan’s cooperative-matrix and FP8 work** shows that the API can now expose the hardware features necessary for serious ML.  
**Arm/Qualcomm Vulkan ML extensions** demonstrate a competing higher-level tensor/graph model, especially relevant to NPUs.

And **Aaltonen supplies the simplifying principle** tying it together:

> Once modern hardware supports general memory, pointers, and programmable computation directly, stop representing historical binding mechanisms as fundamental concepts.

For graphics, following that principle leads to “No Graphics API.”

For compute, it leads somewhere even simpler:

```text
             memory
                +
             kernels
                +
        synchronization
                +
      machine capabilities
```

For ML, the missing ingredient is not a larger runtime API. It is a rich compiler and library ecosystem **above that tiny machine**.

The strongest formulation of the project is therefore not:

> **“ML on Vulkan.”**

Nor:

> **“A new OpenCL.”**

Nor even:

> **“An open CUDA clone.”**

It is:

> **A minimal open GPU compute ABI for modern pointer-addressed GPUs, initially implemented over Vulkan, intended as a stable machine target for kernel languages, optimized libraries, and ML compilers.**

That is a considerably more precise—and, based on the current ecosystem, still meaningfully distinct—design space.