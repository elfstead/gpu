# Open GPU Interface: current design

Updated 2026-09-13. This is an experimental programming model with a working
implementation, not a stable API or standard. Workload evidence should drive API
changes. A small function count alone is not a measure of success.

Initial feasibility and the first bounded consumer checkpoint are complete:
[GGML's FP32 MNIST forward graph](consumer-ggml.md) works through the public API. The
[milestone plan](plan.md) owns working status, scope, blocking decisions and exit
criteria. Experimental still means changes
are allowed, not that a stable or broadly portable interface has been established.

## Purpose and scope

Explore a common low-level foundation for graphics, general compute, and ML:
devices, allocations, GPU addresses, executable code, command batches, explicit
dependencies, completion, capabilities, and diagnostics. Applications and compilers
own data interpretation; the runtime does not own tensor shapes or ML operators.

Graphics is part of the design, not a separate runtime bolted onto compute.
Specialized image storage and rasterization still need explicit concepts. Optional
profiles should expose those facilities without requiring them on compute-only
devices. Presentation belongs behind a separate platform boundary.

The central hypothesis is that this common model reduces integration work and
unnecessary data movement across mixed workloads. The demanding target is
compute → graphics → compute → graphics without intermediate CPU round trips.
The [original image-processing experiment](image-loop.md) completes this loop using
image-to-buffer copies and fragment reads from linear memory. The subsequent
[heap-image experiment](heap-images.md) establishes direct compute image access and
fragment sampling without intermediate copies. It establishes correctness, not
reduced execution time or measured transfer cost.

This project currently implements a host/runtime interface. It does not yet define
a new source language. The host C ABI, shader data/execution contract, executable
format, and implementation language are separate design decisions.

## Current implementation

The [modern-baseline migration](modern-baseline.md) replaces the old execution
backend without keeping compatibility fallbacks. The current public interface is
ABI 3: attachment LOAD/CLEAR broke the draw signature in ABI 2; independent heaps
replaced the coupled table API in ABI 3. Rebuild callers against matching
header/library/shaders; source revision still identifies the experimental checkpoint.

The backend is Rust over Vulkan, with a small C header and directly generated,
pinned Vulkan declarations. It does not depend on ash or Vulkanalia. Only Linux
x86-64 is currently supported. Discovery needs a Vulkan 1.1 loader; execution needs
Vulkan 1.4, descriptor heaps, untyped pointers, address commands and the core
features listed in the baseline decision. The optional graphics path additionally
requires dynamic rendering, unified image layouts and a shared graphics/compute queue.
Pipelines have no layout objects; roots use push data. Barriers, submission and
timestamps use synchronization2. Rendering uses no render-pass/framebuffer objects,
and indirect draws/readback use addresses. Completions now wait on monotonically
increasing values of one device-owned timeline semaphore. The
[retirement experiment](retirement.md) adds completion polling and optional buffer
retention; applications still own scratch-range reuse decisions.

| Part | Implemented contract | Deliberate restriction |
|---|---|---|
| Host boundary | Opaque ownership handles, fixed-width values, explicit errors and lifetime rules | Experimental ABI; some diagnostics/capability fields are Vulkan-specific |
| Linear memory | Owning buffers and separate non-owning GPU addresses | Dedicated host-visible allocations, checked CPU copies, no exposed mapping |
| Executables | Prepared compute kernels and raster programs, caller-defined root bytes | Trusted Vulkan SPIR-V, `main` entry points, limited enabled capabilities |
| Arguments | Inline bytes copied while recording; may contain pointers to larger GPU structures | Layout/padding agreed by caller and shader; no pointer tracing or automatic bounds enforcement |
| Submission | One-shot batches, explicit access barriers, completion wait/poll, optional buffer retention | One queue, externally serialized host calls, no replay/timed waits |
| Timing | Optional whole-batch device timestamps, retrieved after confirmed completion | Approximate interval, counter-wrap limit, no per-region or calibrated clocks |
| Graphics | GPU-produced indirect draws, specialized images, native heap-indexed load/store/sampling, preserved contents and readback | Fixed-state RGBA8, independent heaps with exclusive edits, nearest/linear clamp/repeat sampling |
| Discovery | Device information and supported capability bits | Reporting is not feature negotiation or a complete matrix/type capability description |

The [cooperative reduction](reduction.md) now exercises shared workgroup memory,
shader barriers, and multi-level dispatch using this existing API. It adds workload
evidence, not a new host operation or a reason to freeze the execution model.
The [FP32 matrix experiment](matmul.md) adds numerical references, explicit row
strides, baseline/tiled variants, and separated host timings without matrix-specific
API changes. The [timing follow-up](timing.md) adds optional device batch durations.

The examples establish functional paths and ownership/visibility behavior. They do
not establish competitive performance, portability across hardware vendors, or a
complete graphics/ML programming model. The [experiment ledger](experiments.md)
records what has been tested and what remains unproven.

## Design commitments versus provisional choices

The commitments below are project direction, not promises of stable signatures.

- Separate owning allocations from non-owning device addresses. An address is not
  a CPU pointer, an ownership reference, or a guarantee that an access is safe.
- Let kernels interpret ordinary memory and application-defined argument layouts.
  Keep tensor ownership, operators, and ML graph semantics above the core runtime.
  Hardware-oriented tensor layouts inside kernels are compatible with that boundary.
- Make execution dependencies and completion explicit. CPU return from submission
  is not GPU completion, and queue order alone is not a memory dependency.
- Share the underlying model between graphics and compute while representing
  genuinely specialized facilities, especially images, explicitly.
- Keep a language-neutral C boundary. The runtime's Rust implementation does not
  select the GPU source language or prove asynchronous raw-pointer lifetimes.
- Keep optional hardware facilities discoverable and negotiable; avoid silently
  weakening semantics or pretending all implementations have identical capabilities.

Names such as `Buffer` versus `Allocation`, the exact object model, argument-passing
forms, queue API, executable packaging, and raster-state API remain provisional.
The current fixed root limits, single-dimensional dispatch, dedicated allocations,
and fixed-state rendering are experiment constraints, not the intended final shape.

## Ownership, execution, and safety

Public handles are destroyed once. Children retain their device, and a device
retains its instance. Batches/completions retain explicitly supplied kernels,
raster executables, targets, bound image/sampler heaps and their image entries,
indirect buffers, copy destinations, and buffers explicitly supplied to
`ogpu_batch_retain_buffer`. A raw address inside an argument
block does not create retention. The caller must keep its underlying allocation
alive through explicit retention or its own ownership until all GPU uses finish.

All host operations on a device and its children are externally serialized. GPU
execution can continue between calls. CPU buffer reads/writes require completion
of all submitted uses of that entire allocation because cache maintenance can
touch more than the requested byte range.

Recording validates supported argument shapes and copies inline bytes. Invalid
recording arguments leave the batch unchanged. A submission attempt is one-shot,
including on preparation/submission failure. A successful submission returns a
completion, not a finished result. Destroying a pending completion drains its work;
it does not cancel it. Non-loss wait errors do not permit early resource release.

Shader code, indirect GPU-produced contents, and reachable address accesses remain
trusted inputs. Header checks are not SPIR-V validation or sandboxing. Rust ownership
and host bounds checks cannot establish arbitrary GPU pointer safety. Better
diagnostics are needed, but they must not be confused with guarantees we do not have.

## Open decisions and the evidence needed

This table describes the broader design horizon, not a list of prerequisites for
the first checkpoint. The [candidate decisions D0–D5](plan.md#decisions-blocking-the-candidate)
select the immediate work; other questions remain deferred unless the consumer
brief requires them.

| Decision | Evidence to seek before settling it |
|---|---|
| Inline roots versus GPU-resident argument blocks | GPU-produced structures, layout checks, measured argument/launch costs |
| Allocation/range API and lifetime assistance | Suballocation, completion-based reuse, non-coherent hardware, imported allocations |
| Memory placement and transfer model | Device-local linear data, staging/copies, measured movement costs on discrete and integrated GPUs |
| Queue and batch model | Replayed work, cross-queue dependencies, concurrency, and observable completion/error behavior |
| Executable preparation | Entry points, workgroup variants, specialization, capability requirements, compilation/cache costs |
| Images and graphics state | Native RGBA8 access/sampling, independent heaps and preserved cross-batch images work; general formats/views, concurrent heap edits and broader raster state remain open ([contract](descriptor-heaps.md)) |
| ML profiles | Beyond the tested reduction/FP32 baseline: required storage/arithmetic/conversion/accumulation combinations and accelerated matrix shapes |
| Portability boundary | One real compiler/runtime consumer and a second backend for the common compute subset |
| Tooling | Finer profiling/calibrated clocks, allocation tracking, asynchronous diagnostics, and address-aware capture/replay |

Workgroup/shared-memory operations and subgroup/matrix instructions are primarily
kernel-language/IR capabilities. The host interface needs to select executable
variants and negotiate requirements, not add a host function for each arithmetic
operation. Reusable low-level command sequences do not imply runtime-owned ML graphs.

## First integration checkpoint

The reduction, mixed image loop, FP32 matrix and optional timing experiments have
provided the initial feasibility evidence. They remain regression/diagnostic tools;
their untested variants are not automatically the next development steps.

The [milestone plan](plan.md) records the completed GGML MNIST checkpoint and its
retain/defer API decisions. The integration uses the unchanged public boundary,
and its source revision, reproduction instructions, acceptance and limitations
are recorded in the [consumer brief](consumer-ggml.md). This validates one bounded
compute consumer, not a general GGML backend, graphics consumer, or stable release.

Further experiments must resolve a named decision with a stopping condition.
Benchmark expansion is paused unless that decision needs it. Presentation, broad
rendering features, ray tracing, multi-device communication, and a second backend
are outside this first checkpoint; they are not abandoned project goals.

## Evidence required before stabilization

These gates concern later stabilization and stronger claims, not completion of the
first experimental integration checkpoint. Its narrower exit criteria are in the plan.

- Nontrivial compute, image, and mixed workloads agree with independent references.
- Ownership, layout, visibility, failure, concurrency, and feature-negotiation rules
  are explicit and tested; caller obligations and unsupported cases remain visible.
- Performance measurements separate runtime costs from kernel quality and compare
  against an appropriate lower-level or existing-runtime baseline.
- More hardware vendors and memory configurations are tested. Current local evidence
  from an RX 5700 XT and llvmpipe is not broad hardware-portability evidence.
- At least one real consumer demonstrates an integration benefit. A second backend
  tests whether the common contract depends on accidental Vulkan behavior.
- ABI/version evolution and executable compatibility have a documented policy
  appropriate for consumers, beyond the current experimental version checks.

## Where details live

- [Working plan](plan.md): current phase, first usable scope, decisions and completion gates.
- [Public header](../include/ogpu.h): exact C signatures and caller obligations.
- [Execution baseline](execution.md): linear memory, shader assumptions, blocking example.
- [Batches](batches.md): recording, submission, dependencies, completion, and errors.
- [Offscreen graphics](graphics.md): the implemented optional graphics profile.
- [Descriptor heaps](descriptor-heaps.md): independent image/sampler ownership, mutation and preservation evidence.
- [Reduction](reduction.md): cooperative workgroups and a multi-level compute workload.
- [Image loop](image-loop.md): mixed execution and explicit image/linear conversion.
- [Matrix multiplication](matmul.md): FP32 accuracy, tiled variants, and host timings.
- [Timing](timing.md): optional batch timestamps, ownership, and paired measurements.
- [Experiments](experiments.md): tested evidence, limitations, and next questions.
- [Development](development.md): builds, regeneration, validation, and test commands.

These documents describe the current implementation, not separate competing
proposals. Changes to contracts should update the header and relevant detailed
document together; experiment outcomes should update the ledger and this overview.

## Design history and prior art

Aaltonen's [No Graphics API article](https://www.sebastianaaltonen.com/blog/no-graphics-api)
and [implementation](https://github.com/sebbbi/NoGraphicsAPI) remain design references.
Compare the proposed model, backend implementation choices, and our workload needs
separately. Do not infer what another author considered from a missing feature.

The [archived analysis](archive/design-before-consolidation.md) preserves the original
compute-focused proposal, prior-art discussion, references, and subsequent notes.
It is historical material, not a current comparison of external projects. Recheck
those comparisons when making adoption or differentiation claims. The working
hypothesis must show a concrete benefit over adapting existing interfaces, not
merely resemble them with fewer functions.
