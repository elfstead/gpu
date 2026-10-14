# Open GPU Interface: current design

Updated 2026-10-13. OGPU is an experimental low-level foundation for graphics,
general compute and ML: memory, GPU pointers, prepared executable code, explicit
commands and dependencies. Applications and compilers own data interpretation,
scheduling, allocation policy and numerical choices. The runtime is Rust with a
language-neutral C boundary; Slang is the selected, pinned shader-tooling path,
not a runtime dependency or a new project-owned language.

The implemented Linux interface is [foundation revision 1](../include/ogpu_next.h).
The [whole-API design](whole-api-design.md) describes the broader target; its
[remaining sketch](drafts/ogpu_next.h) is not installed or linkable.
Pin one exact source revision for headers, runtime and artifacts. Version 1 is a
mutable experimental record version, not a cross-commit compatibility guarantee.

## Model and ownership

| Concept | Fundamental responsibility | Policy outside the core |
|---|---|---|
| Device and queue | Exact native topology, requested features and queue counts | Global current device, queue selection or work scheduler |
| Memory and spans | Explicit backing/type/placement, byte ranges and addresses, mapped cache visibility | Tensor shapes, automatic migration, staging allocator or pointer registry |
| Images, views and descriptors | Explicit placement/subresources/state; caller-owned descriptor bytes and heap ranges | Texture loader, slot allocator, whole-heap mutation lock or automatic retention |
| Executable and arguments | Cold preparation, requirements/cache policy; separate inline updates and GPU-root addresses | Runtime reflection, root-pointee copies or command-time JIT |
| Arena, encoder and list | Reserved recording slots; one-shot/serial/simultaneous replay; reset/trim | Hidden command cache, automatic retirement or mandatory per-submit allocation |
| Dependencies and timelines | Stage/access/range scopes, layout/ownership transitions and wait/signal values | Hazard inference, implicit global barriers, resource walks or waits on destruction |

These are common concepts, not a guarantee that every backend exposes every
strategy. Unsupported profiles fail explicitly; missing native support must not
be hidden behind a costly fallback. A small API means little unnecessary policy,
not a requirement to compress every hardware facility into a few functions.

### Execution and arguments

Prepare artifacts and relevant state before recording. Bind executables, update
inline byte ranges or root addresses independently, and issue direct/indirect
dispatches or draws. Inline updates snapshot only the explicit updated bytes.
Device roots snapshot an address; pointed-to data is read at GPU execution.
There is no automatic pointee upload, retention, bounds inference or repacking.

The current Vulkan profile has one shared inline namespace, not independent banks
per shader stage. Root slots and artifact layouts are explicit. Changing inline
values requires recording changes; mutable device-root data and indirect arguments
allow unchanged-list replay. Fixed/count indirect draws retain native draw
identity. General device-generated command topology remains unimplemented.

### Memory, descriptors and synchronization

Linear backing is an addressable buffer allocation with freely partitioned spans.
Optimal images may be placed at explicit offsets in compatible opaque backing,
including discarded-image alias reuse. Mixed buffer/image backing and external/
imported allocations are missing, not approved permanent limitations.

Descriptor writers encode into caller-selected host byte ranges using caller
scratch. Applications choose layout, storage, upload and slot reuse. Unused
nonreserved slots can change while unrelated slots are pending. Native reserved
ranges have stricter command-buffer lifetime requirements; cache-atom isolation
and descriptor/resource hazards remain caller obligations.

Multiple queues, stage-scoped timeline points, global/ranged dependencies and
same-queue split events are explicit. A timeout does not retire work. A narrow
signal is not automatically whole-list completion. Submission uses caller-owned
reusable scratch, not an internally allocated receipt/resource list.

### Host concurrency and safety

Independent objects can be used concurrently. Mutating one queue, arena/encoder,
allocation mapping or externally synchronized cache requires caller serialization
for that object. Device destruction excludes all use. There is no device-wide
recording lock and no implicit wait in destruction.

The C API is a trusted low-level boundary, not a shader sandbox or a safe Rust API.
Pointers/handles must be valid and correctly typed; callers prove GPU-address
bounds, synchronization and asynchronous lifetimes. Local record/size/range/feature
checks catch specified errors but cannot make arbitrary foreign pointers safe.

Recording failure poisons the list and is reported at end. Submission validates
local descriptions before its native call; native OOM leaves the submission
retryable, whereas loss/unknown acceptance is treated conservatively. Never free
borrowed resources merely because an uncertain submit/wait failed. Convenience
wrappers may offer a narrower safe subset, but mandatory ownership tracking,
deferred deletion and wait-on-drop are not restored in the core. No general safe
Rust wrapper or portable device-loss recovery protocol is implemented yet.

## Implemented scope and evidence

Linux foundation implements explicit resource/command ownership, compute,
offscreen raster, descriptor heaps, queries/caches and broader numerical/subgroup
profiles. [Implementation status](foundation-implementation.md) gives exact
profiles and hardware evidence. M1–M5 bounded consumers are complete, including
[the native-matched textured scene](textured-scene-plan.md#clean-m4-offscreen-acceptance)
and [transformer/quantized linear](m5-results.md). Neither consumer adds public
tensor/graph scheduling or texture-allocation policy.

The default Linux SDK exports foundation only; explicit `--legacy` packaging and
plain Cargo defaults preserve ABI20 for remaining callers. Native Metal 4 compute
belongs to that older interface and does not implement foundation. The
[plan](plan.md) schedules Linux presentation before M6's backend migration and
native validation; neither is a claim of current support.

See [support](support.md), [quickstart](foundation-quickstart.md), and
[upgrade policy](release-policy.md). The [ABI20 design history](design-abi20.md)
preserves the earlier candidate, restrictions and decisions.

## Performance-preservation rule

Ask whether a native implementation can preserve a strong native strategy under
equivalent correctness, memory and latency requirements. A forced copy, wait,
allocation, serialization or lost strategy is an API issue even when a GPU-heavy
benchmark hides its cost. Avoidable backend encoding work is a different problem.
A better API alternative is welcome whether or not the current API can be made
to work.

The [consolidated contract review](core-contract-review.md) records retained
strategies, implementation costs and explicit unsupported/unvalidated facilities.
Neither matched-policy tests nor a finite benchmark suite prove universal native
performance parity. Missing capabilities must not become silent foundational
restrictions.

## Evidence required before stabilization

Stabilization requires a specified capability scope, broader physical GPU/memory
evidence, native backend mappings and acceptance, independent integration feedback,
contract/failure tests and performance evidence for genuinely uncertain mappings.
Current evidence is one Radeon plus llvmpipe; software execution is not a second
GPU vendor.

Experimental source packaging can proceed without a Mac or new GPU. It does not
establish Metal parity, clean-machine portability, external adoption or universal
binary support. Publishing/tagging remains separate authorization. The
[working plan](plan.md) owns the next step; the [roadmap](roadmap.md) owns remaining
coverage.
