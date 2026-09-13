# Image ownership and preservation: design review

Historical review, 2026-09-13, against implementation `cbc6528`. Both proposed
checkpoints are now implemented: [image preservation](image-preservation.md) and
[independent heaps](descriptor-heaps.md). Those documents and the public header
describe the current contract and execution evidence. The original rationale and
acceptance criteria below are retained as design history, not pending approval.

## Recommendation

Replace the coupled image table with independently owned image and sampler heaps.
Applications choose capacity, indices and when to replace entries; the runtime owns
the backing allocations, writes driver descriptors and handles cache maintenance.
Keep image views and sampler descriptions as copied values, not mandatory opaque
objects. Preserve image contents across batches unless an operation explicitly
discards or clears them. These changes improve the model even though the current
example passes; compatibility with its table wrapper is not a veto.

Separate the work into two implementation checkpoints: image preservation first,
then independent heaps. Keep the existing image workload and add targeted variants
and failure tests, not a new unrelated consumer.

## What is actually wrong with the current shape?

| Current coupling | Consequence | Proposed separation |
|---|---|---|
| `ImageTable` owns both heaps and constructs one fixed sampler | Changing image membership recreates sampler storage; samplers cannot be shared independently | Separate image-heap and sampler-heap owners and bind operations |
| Immutable constructor chooses the entire table contents | Replacing one image requires a new table/allocation | Caller-chosen slots, explicit writes and clear operations |
| Every image copy requires a same-batch draw/discard | A later batch cannot read an existing image through the documented API without discarding it | Initialization is an image-use obligation, not a batch-local pattern |
| Every draw clears/discards | Drawing over existing image contents is impossible | Explicit CLEAR / LOAD attachment behavior |

The current backend already has separate resource/sampler heap bindings and GENERAL
images. This is mostly a host-contract change, not a new Vulkan compatibility path.
It does introduce lifecycle/error work for mutation and an attachment-read access mode.

## Ownership proposal

The owning objects are the image allocation, image heap, sampler heap and completion.
A view description chooses how an image is interpreted; it does not independently
own storage. A sampler description is a value written into the sampler heap. Shader
indices remain ordinary uint32 values relative to the bound heap, never ownership tokens.

Suggested operation families (names illustrative, not declarations added to the header):

```text
image_heap_create(capacity)         sampler_heap_create(capacity)
image_heap_write(first, views[])    sampler_heap_write(first, descriptions[])
image_heap_clear(first, count)
batch_bind_image_heap(heap)         batch_bind_sampler_heap(heap)
```

Each image entry retains the explicitly supplied image until replacement/clear or
heap destruction. Bindings retain the heap through completion destruction. This
keeps current early-handle-release behavior without a per-dispatch resource-use list.
It is coarse retention, not shader reachability analysis. Heap entries do not retain
resources reachable through arbitrary GPU pointers. Clearing a slot releases its
image ownership and makes that slot invalid to access; it need not imply a null
descriptor usable by shaders. Reusing an index changes its meaning; stale indices
are a caller error, not something fixed by reference counting.

Start with the current RGBA8 view, separate sampled/storage kinds and explicit
nearest/linear, clamp/repeat sampler choices. Do not invent support for arbitrary
format reinterpretation, mip/layer views or anisotropy in the same patch. Format
filtering support must be checked before advertising a linear sampling combination.
An attachment view may remain cached internally; no public view object is needed
merely because dynamic rendering uses a Vulkan view handle.

## Mutation: important first-implementation restriction

For the first mutable implementation, reject heap edits while ANY recording batch
or completion retains that heap—even if the requested slot appears unused. Rebinding
away from it does not release earlier recorded references. Destroy/discard all such
recordings/completions before editing. Waiting alone does not release those references.
This supplies reusable heap storage without introducing implicit snapshots, slot
versioning, access lists or a central retirement manager.

This is stricter than the intended long-term range-based model, not a claim that
Vulkan universally prohibits descriptor updates after binding. The current allocator
flushes entire allocations, including the heap's implementation reservation. Vulkan
[keeps that reservation bound until command-buffer reset](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBindResourceHeapEXT.html),
and noncoherent [cache operations also access neighboring atom-sized regions](https://docs.vulkan.org/refpages/latest/refpages/source/vkFlushMappedMemoryRanges.html).
An apparently idle descriptor slot is not sufficient evidence that this implementation
can safely write/flush it. A future concurrent-slot path needs bounded cache operations,
reservation isolation and an explicit outstanding-use contract. Do not claim streaming
updates are solved by the coarse first version.

Prevalidate an entire write and generate descriptors into temporary storage before
changing live slots. Validation/descriptor-generation errors leave old entries intact.
If flushing committed bytes fails, reject future binds/edits on that heap until destruction;
do not claim rollback restored GPU visibility. Device loss follows the existing terminal
device contract. Every failed attempt must release temporary resources correctly.

## Image contents across submissions

Keep explicit discard for first initialization or deliberate content loss. Once its
submission is accepted, later ordered submissions may use the image in GENERAL without
another discard. Writes must precede reads, with explicit access dependencies and image
ownership maintained through all uses. A rejected/discarded initialization batch does
not initialize anything; a wait error is not proof of successful initialization.
The caller cannot continue a dependent chain after an uncertain submission outcome as
though the producer succeeded. Existing submission error/drain behavior still applies.

Remove the same-batch draw/discard scan from copy validation. Keep device, size and
alignment checks; initialization and written-texel validity become trusted caller
obligations, just as for heap shader access today. Avoid a host-side `initialized` bit
set during recording: batches can be abandoned or submitted in a different order.
No automatic layout tracker, hidden initialization submission or per-image wait is
needed for this one-queue GENERAL model.

For rendering, add an explicit attachment CLEAR / LOAD choice; preserve STORE at the
end of both. CLEAR can use the existing discard path. LOAD must not call discard, and
must include attachment reads in the access vocabulary/dependencies: Vulkan's
[LOAD operation reads prior attachment contents](https://docs.vulkan.org/refpages/latest/refpages/source/VkAttachmentLoadOp.html).
This does not enable blending, feedback loops, depth or a general render-pass API.
Initialization, preservation, synchronization and allocation lifetime remain separate.

## Comparison with Aaltonen's prototype

The existing local clone is pinned to `8e414bd`, not asserted to be upstream HEAD.
Its [API](https://github.com/sebbbi/NoGraphicsAPI/blob/8e414bd0a8010b9f721d06d470860e27aa69c071/include/NoGraphicsAPI/NoGraphicsAPI.hpp)
already separates texture/sampler heap ranges and writes descriptor bytes from view
and sampler descriptions. Follow that separation. Our proposed checked writes and
owning heap handles retain more runtime assistance than its raw mapped destinations;
that difference is an ownership choice, not an older-descriptor fallback.

Its [implementation](https://github.com/sebbbi/NoGraphicsAPI/blob/8e414bd0a8010b9f721d06d470860e27aa69c071/src/NoGraphicsAPI.cpp)
queues newly created textures for initialization and drains that list in `begin_commands`.
That is a different lifecycle design. Our abandonable one-shot batches and recoverable
errors favor explicit initialization in the producer batch. Do not copy just the pending
list without adopting and verifying its surrounding recording/submission rules.

## Acceptance and stopping conditions

1. Preservation checkpoint: split the current GPU image chain into three submitted
   batches, with no intervening host wait and one final verification. Preserve exact
   pixels across boundaries; exercise producer rejection and abandoned initialization.
   Add a partial-coverage LOAD draw and verify untouched pixels, plus CLEAR behavior.
2. Heap checkpoint: reuse one sampler heap with multiple image heaps; switch samplers
   without recreating image heaps; clear/rewrite slots after releasing retained uses.
   Reject edits during recording, pending execution and completed-but-live retention;
   test wrong-device/range/kind inputs, partial creation, generation and flush failures.
3. Keep existing binding/ABI/mock/execution and GGML gates. Replace the table path and
   its tests/docs; do not retain a second backend solely to preserve the old wrapper.

Complete when those contracts and evidence agree. Concurrent slot streaming, generalized
formats/views and compute-only image deployment remain separately scoped work; physical
GPU and remote execution-CI evidence are still missing. The two implementation
checkpoints now meet the local stopping condition; deployment validation is next.
