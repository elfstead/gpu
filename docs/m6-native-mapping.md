# M6: native Metal 4 mapping and proposed profiles

Design checkpoint: 2026-10-17. **Mapping proposal, not an implemented or natively
validated foundation backend.** This completes the first design item of the
[M6 handoff](m6-entry.md), with unresolved native guarantees explicitly below.
No Mac is available here. The existing [legacy Metal backend](metal.md) stays
intact until a replacement has native acceptance; it is not a fallback strategy.

## Direction

Keep explicit allocation, lifetimes, command storage, synchronization and prepared
executables. Introduce native argument, command and residency profiles where the
current foundation vocabulary cannot express the efficient native operation.
Do not turn the Vulkan implementation into an obligatory intermediate language.
An alternative that exposes a better API is worth adopting even if emulation is
technically possible. Conversely, do not remove Vulkan inline arguments or replay
merely to obtain a lowest-common-denominator interface.

Apple's [Metal 4 core overview](https://developer.apple.com/documentation/metal/understanding-the-metal-4-core-api)
describes explicit allocators, argument tables, untracked hazards and command
buffers that do not retain resources. These align with the foundation's explicit
ownership model. Reusing a command-buffer object for another recording is not,
by itself, a guarantee of replaying its existing recording.

The current header implements only `SHADER_SPIRV` and `ARGUMENT_SHARED_BYTES`.
Foundation is Linux-gated and its adapter, snapshots and native object types
depend on Vulkan. The legacy Metal path is a different API: it uploads argument
bytes, creates argument tables per dispatch, retains referenced resources and
uses a device-default residency set. None of those policies should automatically
become foundation behavior.

## Mapping matrix

All dispositions below describe proposed implementation, not current support.
“Candidate” means an apparent native match with remaining contract/SDK checks;
“new profile” means the public vocabulary needs an explicit alternative.

| Area | Native mapping / disposition | Remaining proof or boundary |
|---|---|---|
| Device and queues | Candidate: Metal 4 device/queue, explicit caller-selected queues | Query device support and queue capabilities; do not invent Vulkan queue-family ownership or silently select a queue |
| Allocation and placement | Candidate: buffers for linear addressable backing; placement heaps for explicitly placed resources | Exact size/alignment/storage/cache requirements, CPU mapping and alias lifetime; mixed buffer/image backing needs the planned M10 subset |
| Addresses | Candidate: native buffer GPU addresses and caller-owned pointees | Offsets, alignment, supported address spaces and shader ABI; no pointer scanning or backing registry for shader addresses |
| Command storage | Candidate: arena-owned native allocator plus reserved command-buffer slots | One active recording per allocator; caller proves completion before reset; no reset-time wait |
| Recorded replay | Unresolved: begin/end/object reuse does not establish unchanged-list replay | First implementation advertises one-shot only unless a documented native replay guarantee is established; reject other modes before recording |
| Secondary/generated commands | Separate native profiles, not assumed Vulkan-secondary equivalence | Native indirect-command capabilities and inheritance require their own mapping; render-pass suspend/resume is not a generic secondary-list mechanism |
| Arguments | New profile: native address/resource binding slots and generated interface metadata | No claim of `SHARED_BYTES`; no hidden per-dispatch upload or host byte-bank emulation |
| Descriptors | Candidate native resource IDs/argument-buffer layout, separate from recording-time argument tables | Exact artifact ABI, mutation granularity, view lifetime and execution-time indexing; no claim that current Vulkan descriptor bytes are portable |
| Residency | New explicit set operations, pulling forward the necessary M10 subset | Update/commit/attach/detach boundaries and allocation lifetime; no implicit all-device set or per-submit transitive resource walk |
| Dependencies | Candidate explicit Metal 4 barriers and queue events | Native scopes/stages and cross-encoder ordering must be mapped exactly; queue-wide event waits cannot silently implement narrower waits |
| Completion/errors | Candidate native commit feedback plus explicit events for GPU ordering | Accepted versus terminal/error states, callback storage lifetime and device loss; event values alone do not prove success |
| Executables | New native MSL/metallib artifact profile; prepare outside recording | Generated binding layouts, function constants, compiler identity and cache compatibility; no runtime SPIR-V translation |
| Images/raster/copy/query | Candidate native operations with per-format/per-operation limits | Usage/aspect/sample/stride/alignment constraints and render-pass state; timestamps and other optional features independently queried |
| Presentation | New drawable/layer profile under the optional presentation boundary | Acquisition/backpressure, render versus presentation retirement, resizing and borrowed image lifetime; no fabricated Vulkan swapchain semantics |

## Arguments: the first API decision

The existing shared-byte profile permits partial inline writes, eight-byte root
updates into that same namespace, and preservation at identical offsets across
executable binds and stages. Those are real semantics, not implementation hints.

The documented [Metal 4 argument table](https://developer.apple.com/documentation/metal/mtl4argumenttable)
binds addresses and resource IDs. Compute
[`setArgumentTable`](https://developer.apple.com/documentation/metal/mtl4computecommandencoder/setargumenttable(_:))
documents that dispatch/execute captures the table's bindings during encoding.
It is therefore not a live descriptor heap whose later host edits change already
recorded dispatch bindings. The pointed-to application data is a separate matter:
its execution-time reads still obey ordinary lifetime and dependency rules.

Proposed contract, with final record names deferred to implementation:

- An executable declares its argument profile and native binding metadata.
  Native-address slots name stage/binding indices and pointee alignment, not
  offsets into an imaginary shared host byte bank.
- Root updates lower to native address binding. Start with no inline-byte support
  in this profile unless a native mechanism with matching semantics is established.
  Unsupported inline calls fail through the existing recording-error path.
- Application-owned GPU argument structures remain available, including
  GPU-produced arguments. The application chooses allocation, writes, visibility
  and retirement. They are not mandatory for every native binding operation.
- Reserve reusable table storage with command storage, outside dispatch. Table
  capacity/layout comes from explicit limits or reservation, not lazy growth.
  Independent encoders must not race on one mutable table. Exact graphics-stage
  capture semantics and table lifetime still need native SDK verification.
- Generated native interfaces describe the chosen ABI. Host tooling must not
  substitute SPIR-V push offsets or promise that two artifact formats have the
  same struct layout without checking them.

This preserves a simple model—bind prepared code, supply explicit bindings, issue
work—without inserting an OGPU copy, allocation or resource walk per dispatch.
Metal's native binding capture remains native driver work; “thin” does not mean
that the driver itself performs no recording or allocation.

## Storage, replay and residency

[`beginCommandBuffer`](https://developer.apple.com/documentation/metal/mtl4commandbuffer/begincommandbuffer(allocator:))
allows only one active recording per allocator and clears command-buffer residency
attachments on each begin. Reapply explicitly selected attachments, not a hidden
global set. Native
[`reset`](https://developer.apple.com/documentation/metal/mtl4commandallocator/reset())
requires completion of the allocator's GPU uses. Map that to the caller's existing
arena-reset obligation, not a wait or an internal pending-resource graph.

Keep command support queryable independently: primary/secondary levels, one-shot,
serial replay and simultaneous replay. A reused allocator or native command-buffer
object is not enough to advertise the last two. Do not implement replay by caching
an OGPU command stream and encoding it again at submit. A native replay experiment
can check behavior, but a passing test alone cannot establish an undocumented
lifetime/recommit guarantee. The initial one-shot restriction is a conservative
implementation scope, not a claim that Metal fundamentally lacks replay.

Native [placement buffers](https://developer.apple.com/documentation/metal/mtlheap/makebuffer(length:options:offset:))
have heap-specific size/alignment and matching storage/cache rules. Expose actual
requirements; do not model all resources as subranges of a manufactured giant
buffer. Plan mixed-resource placement alongside explicit residency where it
exposes the better allocation API, without claiming the whole M10 milestone.

[`MTLResidencySet`](https://developer.apple.com/documentation/metal/mtlresidencyset)
provides staged additions/removals and explicit commit. Its changes are not hazard
synchronization, and mutation is not thread-safe. Heap-backed resource residency
can cover the entire heap, not just the named suballocation; report that granularity.

Pull these proposed operations forward from the whole-API draft:

- Create/destroy a set and explicitly reserve its host-side capacity where needed.
- Add/remove native allocation identities and commit changes.
- Attach/detach sets on a queue, or attach caller-selected sets while recording.
- Document exactly when membership is observed and when removal/destruction is
  legal. Caller-controlled explicit membership is distinct from retaining every
  resource mentioned by a command or recursively following GPU pointers.

No hidden residency updates in root binding, dispatch or submission. The native
set's own ownership rules must be reflected, not misrepresented as zero native
retention. Sparse resources need a separate contract; this subset is not VM support.

## Synchronization, presentation and legacy code reuse

The [Metal 4 queue](https://developer.apple.com/documentation/metal/mtl4commandqueue)
exposes event waits before subsequent work and signals after preceding work.
That gives a candidate whole-queue dependency profile, not automatic parity with
every stage-scoped dependency currently accepted by Vulkan. Map in-encoder barriers
separately. Reject unsupported scopes or expose the native scope explicitly; do
not silently over-synchronize all requests to claim support.

Submission also needs a precise partial-acceptance contract: a sequence of queue
waits, commits and signals must not promise atomic rejection if native calls have
already enqueued work. Validate host records before native side effects, then
document native failure/terminal handling. Completion storage should be caller-
reserved or explicitly prepared, not allocated anew per submit by default.

Metal's [`signalDrawable`](https://developer.apple.com/documentation/metal/mtl4commandqueue/signaldrawable(_:))
belongs after commits targeting the drawable and before its presentation call;
it does not itself present. Design acquisition, queue availability waits, render
completion and display retirement around the native lifecycle. No extra bridge
submission merely to present the appearance of a Vulkan binary semaphore.

Reusable legacy ingredients include native selector/loading/error handling and
the distinction between terminal feedback and event progress. Review each before
sharing it. Do not import legacy `Batch::upload`, per-dispatch table allocation,
transient/resource retention, the default residency set or its implicit unaligned
copy kernel. Expose native copy constraints; a caller-selected utility kernel is
separate from the fundamental copy operation.

## Implementation and verification handoff

1. Add the profile/query vocabulary and precise records for arguments, command
   modes and residency, with C/Rust layout checks and unsupported-path tests.
   Leave Vulkan's current profile semantics intact. Profile selection must happen
   before choosing artifacts/recording strategy, not after a failed submission.
2. Extract backend-independent records and validation only where they really are
   independent. Introduce native module selection below the shared Rust operations
   and C adapters. Do not just ungate the Vulkan-specific `foundation` module or
   alias legacy Metal types into it; do not introduce an obligatory public backend
   trait before these differences are represented.
3. Implement placement, explicit residency, one-shot command storage, native
   address binding and queue/feedback operations as one coherent resource/compute
   tranche. Add prepared native artifacts. Then extend images/raster and the
   mixed learned-image/heap consumer, followed by native presentation.
4. On a Mac, check exact SDK contracts and run the same CPU oracles with native
   artifacts. Exercise root/table A/B/A capture, changing pointees between distinct
   submissions, independent encoders, allocator retirement/reset, residency updates
   with unrelated work pending, rejection/retry, device errors and destruction.
   Label re-recorded A/B/A as such; it is not recorded-list replay evidence.
5. Run mixed offscreen and interactive acceptance with explicit profiles. Compare
   native call structure and inspect hidden allocation/copy/wait paths first.
   Measure only unresolved costs using a native Metal reference with the same
   artifacts and workload; do not turn every obvious native mapping into a new
   benchmark gate. Linux mocks prove host lowering, not Metal correctness.

Assumptions: Metal 4 baseline, no classic-Metal fallback; explicit unsupported
profiles are acceptable; native Rust stays below C with no new runtime policy;
ABI revision 1 remains mutable and consumers pin matching sources/artifacts.
Device feature/format support still requires queries, not assumptions based on OS
version. M6 remains open until native acceptance; unavailable Mac testing does not
block independent Linux M8 work after a coherent handoff.
