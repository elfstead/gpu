# C1: dependency and submission decisions

Selected 2026-11-03. **Design, not implemented revision.** The
[C records](drafts/ogpu_dependencies.h) and [Rust mirror](drafts/dependencies.rs)
replace the affected parts of the installed header during C2. They complete the
dependency dispositions in the [minimal-core contract](minimal-core-contract.md).

## Keep one dependency description, with independent scopes

A dependency contains arrays of global, memory-range and image barriers. Each
entry specifies its own execution stages and memory accesses. One `barrier` call
records the complete batch using caller scratch. An empty description is a no-op;
an execution-only dependency is an explicit global entry with zero access masks.
There is no implicit all-stage dependency and no state/hazard inference.

This corrects an existing limitation: `Dependency.before/after` currently supplies
the same stage pair to every native barrier. For example, compute→indirect and
color-output→fragment dependencies at the same recording point need not become
the cross-product of both stage sets. Separate calls can express those edges, but
cannot preserve native one-call batching. Independent per-entry scopes retain
both properties without a second operation. [Native per-dependency scopes][vk-barrier]

| Mechanism | Disposition and concrete reason |
|---|---|
| Global stage/access entry | Core. Pointer-reachable resources and aliases need visibility without an allocation registry or a resource list. |
| Resource/range entry | Capability-dependent core in the same description. Identifies bounded visibility/cache scope and exclusive ownership endpoints. Global visibility cannot communicate that identity. |
| Image state/subresource entry | Core where native state changes require it. A global barrier cannot transition a presentable image or initialize a new image interpretation. |
| Split endpoints | Capability-dependent core. Capture a producer scope before independent work and consume it afterward; a single barrier position cannot express that schedule. |
| Exclusive/concurrent sharing | Keep explicit native resource policy. Forcing concurrent sharing cannot transfer an existing exclusive resource or express an external ownership boundary. |
| Stage-scoped queue wait/signal | Capability-dependent core. A fragment-only dependency must not silently block independent vertex work; narrower signal scope is not whole-list completion. |
| Barrier graph, automatic layouts, alias registry, retirement queue | User composition. Applications know their resource graph; the low-level API does not reconstruct it. |

Range specificity constrains the declared memory scope, not the physical cache
implementation or which same-stage commands the GPU can overlap. A native driver
may flush a larger cache unit. OGPU must nevertheless preserve resource identity
and range where supported, rather than throw away information a native driver
could use. On a machine with independently maintainable resource caches, a global
flush may touch unrelated storage that a resource barrier need not touch. This is
a structural performance argument, not a measured speedup on the available GPU.
Ranges also have a direct semantic use in exclusive ownership transfer.
[Native memory and ownership scopes][vk-buffer]

## Split dependencies are already justified

The accepted [scope review](dependency-scope-results.md) enumerated the legal
global-barrier orderings for A→C plus independent same-stage B. Every such ordering
adds A→B or B→C. The native sequence `A; release; B; acquire; C` avoids both extra
edges. The [public experiment](split-dependency-results.md) established correctness,
not a local throughput gain. Repeating that benchmark would not change the
structural decision.

Keep the current foundation's caller-owned split objects, explicit reset and
matching release/acquire descriptions. Do not restore the earlier arena-local token
implementation. [Current foundation evidence](foundation-implementation.md#caller-owned-split-dependencies)
covers independent arenas and batched waits. Objects are device-local, not queue
timelines. Matching execution stays on one native queue; CPU recording order may
differ. Descriptions, including array ordering, must match at both endpoints.
The caller orders reset after prior uses and before the next release. Replaying
a command list does not reset its event or make overlapping cycles independent.
[Native event scopes][vk-set]

Keep create/destroy/reset signatures. Release consumes the new dependency record
and explicit scratch. Batched acquire uses one scratch span sized for all native
headers and entries. Remove the nested per-dependency scratch fields; requirements
queries inspect counts, not resource contents. No endpoint registry, allocation,
host event reset, implicit ALL barrier or automatic reset appears in ordinary use.

## Native capability boundaries

`QUERY_DEPENDENCY_LIMITS=19` adds one immutable device-query record per enabled
queue domain. It reports split/ownership/binary support, memory scope granularity,
and supported wait/signal/split stages. Stage masks describe selectable interface
scopes; they are not overlap measurements. ALL-only submission support rejects
narrower stage requests. Ordinary stage/access/resource validity still applies;
these flags do not guarantee every possible combination is legal.

Memory scope NONE rejects resource-memory entries, OBJECT requires the entire
native buffer represented by the backing object, and BYTES accepts native bounded
ranges. No rounding or substitute global barrier is hidden. Caller suballocations
do not become independent native objects merely because they have different spans.
Both domains IGNORED means no ownership transfer; native family transfer requires
its capability, matching exclusive-resource endpoints and explicit queue ordering.
Concurrent sharing needs no invented ownership operations.

The reviewed DX12 baseline supports whole-buffer enhanced barriers, not byte
subranges. Classic enhanced split barriers are resource-associated, so they do
not implement the general global split-event contract. Its queue waits do not
establish narrower stage scopes. Do not advertise those capabilities through
conservative emulation. The current upstream specification also describes newer
command-list fence barriers; their existence is a candidate mapping, not proof
that this branch, SDK and hardware implement them. [Enhanced-barrier specification][dx-barriers]

DX12 global lowering must preserve grouped/free access combinations on both sides,
including storage→indirect and copy→shader. An omitted barrier or substituted
NO_ACCESS is not a simplification. Image state changes still require image identity.
These are the existing [R1 corrections](dx12-review-handoff-2026-10-30.md#r1--p1-global-dependencies-can-emit-no-barrier),
not new Windows acceptance. Per-entry scopes make exact lowering possible; they do
not automatically make all access combinations representable.

Keep explicit image states, discard and ownership fields. GENERAL remains the
ordinary modern path where supported. State transition and synchronization are
separate: discard does not remove prior-use ordering, and PRESENT is not GENERAL.
Native image states remain available when they express a needed transition or
native strategy; no automatic layout planner or legacy fallback is selected.
Discarded alias reuse composes old-access visibility, new-image discard and
initialization. Preserving reinterpretations and foreign resource ownership remain
the scheduled M10 work, not implied by that composition.

## One submission operation, explicit outcomes

Select `queue_submit(queue, count, descriptions, outcomes, scratch)`. A single
batch uses count one; count zero is a no-op on a live device. There is one native
Rust operation and one C adapter, not separate single/batch/binary implementations.
A consumer may wrap count one without a runtime object. Each description retains
its own ordered list/wait/signal boundary; do not flatten all descriptions.

Descriptions carry timeline and optional binary arrays directly. Remove the
linked binary-submit extension and per-description scratch. One reusable caller
span holds translation for the entire call. The single requirements query reads
counts/flags only, so it can run before handles are populated. It checks summed
sizes and alignment for overflow, returns alignment one for zero storage, and
leaves output unchanged on failure. Flags are zero in this revision.

Preflight every batch before native side effects. Validate object/device/domain,
enabled scope, counts and scratch; trust asynchronous lifetime, duplicates and
ordering obligations as in the existing core. No quadratic scan or per-list
retained resource walk. Native Vulkan keeps one `vkQueueSubmit2` with the original
batch boundaries. A backend whose native submission requires wait/execute/signal
calls may issue that sequence, but cannot claim Vulkan's failure atomicity.

Every outcome has a status and REJECTED, ACCEPTED or UNKNOWN acceptance:

| Condition | Contract |
|---|---|
| Local preflight failure | All batches REJECTED; no one-shot consumption or sync effects. Every outcome is written with the rejection status. |
| Native success | ACCEPTED batches have their requested work/waits/signals accepted; this is not completion. |
| Proven native rejection without effects | REJECTED; retry possible if the error/device state allows it. |
| Partial or unclassified native failure | UNKNOWN for the affected batch; preserve proven earlier acceptance and later non-attempted rejection. No blind retry. |

The aggregate return preserves failure. Outcomes are mandatory caller storage and
borrowed only for the call; no allocated receipt is retained. Not-attempted batches
carry the stopping error, not OK. A failing native call may make every batch's
acceptance unknown if its guarantee gives no finer proof. Accepted one-shot lists
are consumed. UNKNOWN lists are not retryable and all potentially referenced
objects stay protected until conservative recovery; uncertain acceptance poisons
the device under the existing policy. A REJECTED batch does not make shared objects
safe if another accepted/unknown batch uses them.

Vulkan native OOM has an unchanged-state guarantee; it is retryable under that
guarantee. It is not a universal rule for an OOM after another backend has already
issued waits or executed work. Loss is failure, never a completed timeline sentinel.
Timeout does not prove retirement. Only the appropriate completed signal scope
proves the associated work retired. [Native submission guarantees][vk-submit]

## Exact migration and checks

Existing dependency stage pairs are copied into each new entry; existing global
accesses become a global entry when present. With no resource entries, preserve
the old execution-only dependency by emitting a global entry even when accesses
are zero. Do not migrate an old nonzero execution scope to an empty new record.
Multiple independent global entries are allowed. Resource ranges, state, discard
and ownership values retain their meaning. Split endpoint records use these same
entries, with scratch separate from semantic matching.

Old `queue_submit` and `queue_submit_batch` become the same operation with one or
many descriptions. Copy binary extension arrays into explicit fields, request
one scratch span, and inspect outcomes before retry/retirement. Preserve native
Rust's direct call path; remove `queue_submit_with_binary` as a distinct core path.

C2 checks: independent per-entry native stage/access pairs; no cross-product union;
global-only/execution-only/empty records; whole-call rejection; bounded range and
ownership checks; split reset/matching/replay; one native Vulkan barrier/submit
batch; no lost global dependencies; native OOM, unknown failure and device loss;
zero-count behavior and scratch overflow/alias preconditions. Reuse accepted
compute-written indirect, explicit transfer and ownership consumers. No fresh
timing campaign is needed to choose these semantics. Newer DX12 fence barriers
and native Metal ordering remain later mapping checks, not Linux prerequisites.

Record check: GCC 16.2.0 and Rust 1.98.1 on Linux x86-64 pass C11/C++17 syntax and
all 307 C/Rust comparisons in `tools/check-c1-drafts.py` after adding these records.
This does not validate native execution of the revised signatures.

[vk-barrier]: https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdPipelineBarrier2.html
[vk-buffer]: https://docs.vulkan.org/refpages/latest/refpages/source/VkBufferMemoryBarrier2.html
[vk-set]: https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdSetEvent2.html
[vk-submit]: https://docs.vulkan.org/refpages/latest/refpages/source/vkQueueSubmit2.html
[dx-barriers]: https://microsoft.github.io/DirectX-Specs/d3d/D3D12EnhancedBarriers.html
