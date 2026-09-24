# Open GPU Interface: current design

Updated 2026-09-16. This is an experimental programming model with a working
implementation, not a stable API or standard. Workload evidence should drive API
changes. A small function count alone is not a measure of success.

The [performance-expressibility gate](performance-expressibility.md) asks whether
a native implementation could preserve Vulkan's attainable performance, not just
whether this backend is close to a native control constrained to the same policy.
An API-imposed disadvantage challenges the fundamental contract. Matched-policy
measurements alone cannot approve that contract; unresolved restrictions remain
explicit in the audit.

Initial feasibility and the first bounded consumer checkpoint are complete:
[GGML's FP32 MNIST forward graph](consumer-ggml.md) works through the public API.
The second bounded consumer, [libplacebo image processing](consumer-libplacebo.md),
also executes upstream compute and raster passes through OGPU and matches its
Vulkan reference on both tested drivers. Neither integration is a general backend.
Libplacebo's [two-frame follow-up](libplacebo-inflight.md) retains consumer-owned
scheduling/reuse policy and uses existing completion/ownership rules to remove
per-operation waits. Subsequent [frame batching and native comparisons](libplacebo-performance.md)
measure the implementation; the [image-allocation correction](libplacebo-diagnosis.md)
recovers most of the large resident-image gap on the tested Radeon. No runtime
scheduler was introduced. The [two-consumer checkpoint](checkpoint.md) collects
the current scope, reproduction path and evidence limits.
The [milestone plan](plan.md) owns working status, scope, blocking decisions and exit
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
ABI 17, with recording-local split dependencies and explicit serial/simultaneous
replay modes. On Vulkan, ordinary device creation enables compute/images/heaps; rasterization is
explicitly opt-in. [Enabled capabilities and exact image-support queries](execution-capabilities.md)
describe the created device, not just physical support. Terminal completion
observation retires submission resources independently of the surviving result
receipt. [16-bit buffer storage](ggml-mixed-precision.md) is baseline; the later
[numerical experiment](ml-executable-requirements.md) enables FP16 arithmetic when
supported, with caller-owned variant selection. Image backing prefers eligible device-only local memory while accepting
visible local/UMA memory; buffer placement remains explicit.
The [bounded HDR consumer](libplacebo-hdr.md) adds RGBA16F image/render targets,
sampled/transfer RGBA16 UNORM, and explicit raster target-format matching.
Raster parameter packing remains consumer-side; no new public uniform-buffer or
color-management object is introduced.
Rebuild callers against matching header/library/shaders. Source revision identifies
the experimental checkpoint; the [ledger](experiments.md) and
[historical plan](plan-history.md) preserve the sequence of ABI changes.

The backends are Rust over Vulkan on Linux x86-64 and native Metal compute on
macOS arm64. Common contract/state code is shared; native encoding is not forced
through a Vulkan-shaped abstraction. The [Metal backend](metal.md) records its
native Metal 4 implementation and acceptance. Shader
artifacts are explicitly tagged: Vulkan uses SPIR-V; Metal also accepts native MSL
and metallib, with SPIR-V translation an optional input adapter rather than the
definition of the programming model. Native inputs initially require pre-specialization.

The [compiler-facing experiment](compiler-workflow.md) adds a bounded offline
Slang-to-C adapter: reflection and checked SPIR-V generate argument layout,
entry/local-size metadata and capability requirements together with the artifact.
The [learned-image application](learned-image-compiler.md) extends it across
FP32-pointer compute and bounded vertex/fragment executables; changing root order
and workgroup size preserves the host source and checked output on both Vulkan
drivers. This is an evidenced compiler boundary, not a general shader package.
The runtime does not depend on Slang or reflection. Application code still owns
memory, dispatch extent, synchronization and numerical permission; broader shader
types and a general compiler/runtime package contract remain open.

Vulkan uses directly generated, pinned declarations, not ash or Vulkanalia.
Vulkan discovery needs a Vulkan 1.1 loader; execution needs
Vulkan 1.4, descriptor heaps, untyped pointers, address commands and the core
features listed in the baseline decision. The optional graphics path additionally
requires dynamic rendering and a shared graphics/compute queue. Unified image layouts
are enabled when supported for their layout-efficiency guarantee, not required.
Pipelines have no layout objects; roots use push data. Barriers, submission and
timestamps use synchronization2. Rendering uses no render-pass/framebuffer objects,
and indirect draws/readback use addresses. Completions now wait on monotonically
increasing values of one device-owned timeline semaphore. The
[retirement experiment](retirement.md) adds completion polling and optional buffer
retention; applications still own scratch-range reuse decisions.

| Part | Implemented contract | Deliberate restriction |
|---|---|---|
| Host boundary | Opaque ownership handles, fixed-width values, explicit errors and lifetime rules | Experimental ABI; some diagnostics/capability fields are Vulkan-specific |
| Linear memory | Owning buffers, non-owning GPU addresses, checked CPU copies and optional borrowed HOST views with explicit range visibility | Dedicated HOST/DEVICE placement; view access needs caller-managed range/atom synchronization; [HOST-view acceptance](host-view-results.md) |
| Executables | Prepared compute kernels and raster programs, caller-defined root bytes, explicit artifact formats | Vulkan SPIR-V and optional Metal translation use `main` and scalar specialization; native Metal inputs are pre-specialized, compute-only; trusted code throughout |
| Arguments | Inline bytes copied while recording; may contain pointers to larger GPU structures | Layout/padding agreed by caller and shader; no pointer tracing or automatic bounds enforcement |
| Submission | One-shot batches, optional explicit recording storage and immutable replay, access barriers, completion wait/poll, optional buffer retention | One queue, externally serialized host calls, fixed replay commands/roots, no timeout waits; storage/replay currently Vulkan-only |
| Timing | Optional whole-batch device timestamps, retrieved after confirmed completion | Approximate interval, counter-wrap limit, no per-region or calibrated clocks |
| Graphics | GPU-produced indirect draws, specialized images, native heap-indexed load/store/sampling, preserved contents, upload/readback | 1D/2D RGBA8/R32F/RGBA16F images with explicit usages; fixed-state RGBA8/RGBA16F rendering, independent heaps with exclusive edits, nearest/linear clamp/repeat sampling |
| Discovery | Physical support, cached enabled capabilities/limits, exact image-description checks | Reporting is not feature negotiation, shader reflection or a complete matrix/type capability description |

The [completion-resource follow-up](completion-resource-review.md#implementation-abi-9)
implements this separation. Pending/transient-error polls retain resources; terminal
observation releases per-execution ownership. One-shot retirement invalidates native
commands before dropping recorded objects; bounded empty command capacity may survive
under implicit or caller-owned storage policy. Immutable lists retain commands and
objects between uses until the last list owner is gone. No collector or automatic
range allocator is added. Polling can incur host destruction cost. See
[storage](recording-storage-results.md) and [replay](command-list-results.md) acceptance.

The [cooperative reduction](reduction.md) now exercises shared workgroup memory,
shader barriers, and multi-level dispatch using this existing API. It adds workload
evidence, not a new host operation or a reason to freeze the execution model.
The [FP32 matrix experiment](matmul.md) adds numerical references, explicit row
strides, baseline/tiled variants, and separated host timings without matrix-specific
API changes. The [timing follow-up](timing.md) adds optional device batch durations.

The examples establish functional paths and ownership/visibility behavior. The
bounded consumer measurements do not establish general performance parity,
portability across hardware vendors, or a
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
The current fixed root limits, dedicated allocations,
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
| Memory placement and transfer model | Explicit HOST/DEVICE and staged GGML work on RADV/llvmpipe; physical UMA/BAR evidence, mixed access/locality contracts and allocation strategy remain open ([checkpoint](memory-transfers.md)) |
| Queue and batch model | Replayed work, cross-queue dependencies, concurrency, and observable completion/error behavior |
| Executable preparation | Entry points, workgroup variants, specialization, capability requirements, compilation/cache costs |
| Images and graphics state | 1D/2D RGBA8/R32F/RGBA16F uploads, native access/sampling, independent heaps and preserved cross-batch images work; additional formats/views, concurrent heap edits and broader raster state remain open ([contract](descriptor-heaps.md)) |
| ML profiles | Beyond reduction/FP32 and the bounded [FP16-weight checkpoint](ggml-mixed-precision.md): other storage/arithmetic/conversion/accumulation combinations and accelerated matrix shapes |
| Portability boundary | One real compiler/runtime consumer and a second backend for the common compute subset |
| Tooling | Finer profiling/calibrated clocks, allocation tracking, asynchronous diagnostics, and address-aware capture/replay |

Workgroup/shared-memory operations and subgroup/matrix instructions are primarily
kernel-language/IR capabilities. The host interface needs to select executable
variants and negotiate requirements, not add a host function for each arithmetic
operation. Reusable low-level command sequences do not imply runtime-owned ML graphs.

## Completed integration checkpoints

The reduction, mixed image loop, FP32 matrix and optional timing experiments have
provided the initial feasibility evidence. They remain regression/diagnostic tools;
their untested variants are not automatically the next development steps.

The [milestone plan](plan.md) records the completed GGML MNIST checkpoint and its
retain/defer API decisions. The integration uses the unchanged public boundary,
and its source revision, reproduction instructions, acceptance and limitations
are recorded in the [consumer brief](consumer-ggml.md). This validates one bounded
compute consumer, not a general GGML backend or stable release. The subsequent
libplacebo checkpoint supplies bounded mixed compute/raster evidence, including
two-slot execution, frame batching and a native performance comparison. Neither
integration constitutes a general consumer backend; current reproduction is
collected in the [two-consumer checkpoint](checkpoint.md).

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
