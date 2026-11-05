# C1 consolidation and C2 cutover

Selected 2026-11-03. **C1's foundation contract is complete; implementation is C2.**
The Vulkan argument, descriptor and generated-layout families are installed as
foundation schema 5, including borrowed heap inheritance and per-entry dependencies.
Unified submission/outcomes and presentation remain to migrate. This document does not declare Windows/Metal acceptance or complete the later M8–M11 hardware coverage.

## Authoritative surface

The [combined C index](drafts/ogpu_minimal.h), the four family contracts
([arguments](minimal-core-arguments.md), [descriptors](minimal-core-descriptors.md),
[presentation](minimal-core-presentation.md), [dependencies](minimal-core-dependencies.md))
and the [operation inventory](drafts/minimal-core-migration.json) select one revision.
The broader [whole-API target](whole-api-design.md) continues to schedule remaining
hardware facilities. It does not override these selected foundation representations.

The index imports the installed header to avoid duplicating unchanged declarations
and shared types in design files. Imported **superseded** declarations are not part
of the target: its operation set is the inventory's retained operations plus draft
destinations. This is a design index, not a drop-in replacement installed header.
At C2 cutover rewrite the real header and Rust types; do not ship both contracts.

The inventory covers all 138 original C operations, including presentation-table
members: 114 signatures remain, 24 are replaced. The 37 draft operations include
the replacements and new explicit storage/layout/query/lifecycle operations.
Each has an exact target C name and direct Rust name. More functions do not imply
more independent runtime models: one argument update, descriptor storage model,
command-storage model, dependency description and submission operation remain.

Retained operations retain their signatures and unaffected obligations, not an
old embedded record that this document replaces. Discovery, allocation/placement,
mapping/cache control, image/view creation, command arenas/replay, work commands,
render state, copies, queries and timeline operations remain. The installed header
baseline is pinned at `40bde34` and its content hash is in the inventory. The audit
reads that baseline from Git and separately checks the current 147-operation surface. No unlisted entry
point is silently deleted or added to the selected core.

## Signature and record migration

All `ogpu_draft_*` names are temporary documentation names. Core C destinations use
`ogpu_next_*`; presentation operations stay in the optional function table. Native
Rust uses direct operations with the same names minus the C prefix, slices for
arrays and explicit borrowed handles. C converts records/errors into those same
backend operations, without Rust calling C or a hidden safe wrapper.

| Existing entry points | Selected destination |
|---|---|
| `set_inline`, `set_root` | `set_arguments(encoder, layout, offset, size, data)` |
| `executable_create` | New shader/executable records plus borrowed prepared argument layout |
| Descriptor scratch/write operations and `bind_heap` | Exact D signatures: shape/heap queries, explicit storage, locations, writes/copies and heap-object bind |
| `generated_layout_create` | Explicit borrowed argument layout in the generated layout description |
| `barrier`, barrier scratch query | Arrays of independently scoped entries; one explicit translation span |
| Split release/acquire and scratch query | Same new dependency records; one span for each complete native operation |
| `queue_submit`, `queue_submit_batch` | One `queue_submit(queue, count, descriptions, outcomes, scratch)` |
| Both submit scratch queries | One device-aware query over batch counts/flags |
| Presentation create/destroy/images/acquire/release/present | Exact P signatures and native effects; `release` becomes `abandon` |

The JSON inventory gives each old symbol's exact destination, including all
unchanged symbols and added operations; this table groups them only for reading.

Delete/replace these records and metadata together:

- `root_slot`, `argument_interface`, `ARGUMENT_SHARED_BYTES` and shader
  `interface_metadata`: replaced by prepared layout fields and a shader record
  without that pointer. There is no second authoritative layout in each shader.
- `execution_limits`: `max_inline_bytes` is an upper bound; `argument_kinds` is a
  bitset indexed by the selected field-kind constants. Creation validates combined
  native budgets, binding coordinates and artifact requirements. A supported kind
  does not imply every combination fits; no spill or hidden pointee upload.
- `heap_binding` and byte-only `descriptor_limits`: replace bindings with heap
  objects; shader-visible capacities use native units. Per-description shape and
  backing requirements come from D's queries. Address/sampler limits remain.
- Secondary `inheritance`: use the exact new record with borrowed resource/sampler
  heap pointers. Rendering/query inheritance records keep their existing meaning.
- Generated `INLINE` token name becomes `GENERATED_ARGUMENT` (same wire kind 2).
  The generated layout borrows the prepared argument layout; wire offsets remain
  explicit. No separate generated-root setter is introduced.
- `dependency`, memory/image barrier and split-pair records: use the new arrays
  and per-entry scope records. No embedded translation storage or implicit global
  entry remains. Preserve old execution-only dependencies explicitly during migration.
- `submit_desc` and `binary_submit_desc`: replace with one flat submission record
  carrying explicit timeline/binary arrays. Delete the binary extension, individual
  scratch fields and distinct binary-submit runtime path.
- Presentation status/frame/outcome/resize records and function table: use P's
  new signatures. Old index-only acquisition and universal replacement assumptions
  cannot remain under the same table version/size.

Use new schema/version identifiers for changed installed ABI records/tables during
cutover and regenerate C/Rust layout checks. Existing binaries/artifacts must not
silently bind to incompatible structures. Rebuild consumers from the pinned revised
SDK; no compatibility shim or stable-ABI claim is part of this experimental revision.
The draft record tags/numeric values are exact within the sketches, not authorization
to reuse an old installed version with a different byte layout.

## Query and platform namespaces

The descriptor and execution query IDs retain their purposes but return the new
records under the revised schema. Dependency limits add query ID 19 with one record
per enabled domain. Queries retain count/capacity and output-failure rules. Old
descriptor byte sizes are not retained as a parallel authority for native slots.

Presentation discovery adds the Win32 bit alongside Wayland. Profile-qualified
contract queries report actual lifecycle capabilities. Existing live Vulkan
surface/mode schemas stay Vulkan; native tokens cannot be relabeled as DXGI.
The selected Windows mapping uses the tagged DXGI creation record, native format/
color-space numbers where queried, and rejects inapplicable Vulkan mode queries.
Any additional Windows live configuration-query schema is a backend-handoff task,
not a prerequisite for Vulkan C2 or a promise of universal mode equivalence.
Exact requested creation always checks native support. P's new occlusion and
still-drawing statuses remain distinct, with explicit acceptance effects.

## Cross-family state rules

**Executables and arguments.** Binding an executable does not restore arguments.
The caller initializes required fields and uses A/B's native domain/base/key and
bind-invalidation rule to reason about reuse. Pipeline binding is separate: an
execution-class change does not implicitly rebind an earlier pipeline. Layouts
outlive executables, generated layouts and all recorded/future/pending uses.

**Secondary commands.** Children initialize their own pipeline/arguments; rendering
inheritance does not import those values. Inherited heaps must match the parent's
actual native bindings at each execution; using the same heap object suffices.
Equivalent borrowed bindings remain a caller proof, not a content-comparison walk.
An inherited kind cannot be rebound by that child. Nonempty secondary execution
invalidates the parent's executable and affected argument domains, viewport/scissor
and vertex/index bindings under the existing native contract. A heap survives only
when every child inherits it. No stored payloads are restored on return.

**Generated commands.** Argument tokens obey the same field bounds and atomic
native-address update rules as CPU updates. Sequence-index writes target one
four-byte INLINE field. An address token is legal only when the native generated
operation can supply that field directly; otherwise layout creation rejects it.
No CPU decoding, per-sequence host loop or hidden shader indirection is introduced.
All selected executables must have matching native argument mappings and artifact
semantics; a shared layout is the straightforward composition. Native requirements
and set-update/preprocess invalidation stay explicit. Executable-selection tokens
invalidate the corresponding pipeline binding. The initial Vulkan mapping keeps
its existing argument invalidation: argument/sequence tokens leave the affected
argument domain undefined, not just the bytes written by a particular sequence.
Narrower preservation requires a native guarantee. Heaps remain subject to their
own binding/inheritance rules. Generated work never gains a shadow restoration path.

**Descriptor indices.** Generated shader metadata describes native location units,
descriptor shapes/array strides and index width. CPU/GPU data producers write the
right native indices; serialized argument bytes are not universally portable.
No mandatory largest-slot layout or shader conversion table is introduced.

**Synchronization and failures.** Heap bytes, argument pointees, preprocessing and
image transitions use the same dependencies. Descriptor writes/binds are not an
implicit dependency graph. Submission acceptance is not rendering completion;
rendering completion is not presentation retirement. No disposal helper can infer
these facts from frame number or an error code alone.

## C2 implementation sequence

This is one coordinated revision with buildable checkpoints, not several retained
production APIs. Commit each coherent checkpoint; use temporary local adapters
only if needed to keep migration reviewable and delete them before C2 completion.

1. **Shared records and arguments.** Implement prepared layout/native mapping,
   schema/metadata updates and one argument updater. Cut compiler generators and
   argument consumers over together. Remove old root-slot/push-interface runtime
   branches, then preserve reuse/partial-update behavior through the new contract.
2. **Descriptor storage and command composition.** Implement D's explicit HOST/
   MEMORY heap backing on Vulkan, locations, writes/copies and cold immutable info.
   Migrate generated heap indices, secondary inheritance and generated argument
   layouts. Delete raw-output/bind-record production paths after callers move.
3. **Dependencies and submission.** Preserve per-entry native scopes, migrate all
   barriers/split descriptions and consolidate submission/scratch. Implement
   acceptance classification and native preflight/failure tests. Delete separate
   single/binary submission paths and linked extension translation.
4. **Presentation and installed consumers.** Implement P's Vulkan frames/outcomes
   and replacement lifecycle. Update the three-slot scene, multi-window probe,
   Rust presentation facade, C table and SDK packaging. Unsupported DXGI-only
   resize/pacing operations remain explicit; do not simulate them on Vulkan.
5. **Contract audit and cutover.** Remove temporary migration helpers, regenerate
   installed layouts/artifacts, check exports and public Rust correspondence, and
   update current documentation/examples. Replace the C1 baseline audit with checks
   of the implemented surface. Mark C2 complete only after the coordinated changes
   and affected checks pass; then run local C3 acceptance.

Existing runtime/backend files remain the implementation locations: shared
`foundation/types.rs`, Vulkan command/executable/descriptor/presentation modules,
`foundation/rust`, C adapters, compiler generators and installed consumers. Do not
create a second core crate or runtime to avoid migrating these users. Keep the
legacy Metal path isolated until its eventual native replacement is accepted.
No DX12 runtime commits are imported into this branch.

## C2 first implementation checkpoint: prepared argument engine

Historical checkpoint (`b20a235`, followed by `c3d1a78`), superseded by the public
cutover below. Implemented 2026-11-03 on Vulkan/Linux, below the installed API:

- Shared field/layout/state records and a prepared device-scoped layout. Cold
  validation checks byte extents, stage classes, address alignment, native budget
  and ambiguous resource mappings. Per-stage Vulkan mappings target push data or
  a push-data address directly; no descriptor allocation or shader rewrite.
- Compute and graphics pipeline creation borrow that layout. Native subgroup
  requirements remain in the shader-stage extension chain. All Vulkan layouts
  report one preserving push-data domain, base zero and a common compatibility
  key; no interning registry is needed for these native facts.
- One internal range updater checks the bound layout and preflights all touched
  native addresses before a single `vkCmdPushDataEXT`. Partial inline updates stay
  direct. Address updates are indivisible; no allocation, stored values, address
  lookup, pointee copy or state restoration occurs during recording. All-inline
  layouts allocate neither address-validation entries nor a boundary table.

**Temporary migration weight:** the installed `set_inline`/`set_root` entries and
per-shader interface records still exist. Executable creation converts those
records into one owned all-inline layout; the common preparation path ignores
per-shader interface metadata. Physical-pointer roots remain inline bits. The
old root-slot validation and owned compatibility layout disappear with compiler/
consumer cutover; they are not a second target API. Generated commands currently
use the adapted executable's byte extent; their explicit borrowed-layout migration
remains in step 2. No new public C operations or revised shader schema are shipped
by this checkpoint.

Validation on Linux x86-64, RADV NAVI10 (RX 5700 XT), Mesa 26.2.5:
70 CPU unit tests; 16 existing foundation native tests; one new prepared-layout
native test; the public C foundation compute/graphics/generated consumer; 1380
C/Rust ABI values (including the actual new runtime records); reproducible native
bindings; and both C1 draft/surface audits pass. The new native test creates a real
pipeline with a borrowed layout, instruments command emission, checks late-address
rejection, exact one-call updates, layout mismatch and poisoned recording behavior.
That first native test does not dispatch mapped buffer declarations. A follow-up
execution fixture now covers that gap: pinned Slang's `foundation-arguments`
shader uses bound inline constants, uniform/read/read-write buffer addresses and
a direct physical pointer in one prepared layout. Two real pipelines share the
layout, with a four-byte inline update and an eight-byte destination-address update
between dispatches. Two executions change every pointee while retaining captured
argument values. Both outputs and all surrounding guard words pass on the same
RADV device, including four-byte storage addresses that do not meet a blanket
storage-descriptor offset alignment. The run also passes with Khronos validation
and synchronization validation enabled, with no validation messages. All checked-in
foundation shaders, including this fixture, reproduce with Slang 2026.14.1 and
validate for Vulkan 1.4.

Existing native/consumer receipts cover the transitional all-inline path. Unsupported
hardware profiles remain explicit exclusions in the consumer, not acceptance.

**Next at that checkpoint (now completed below):** cut the public C/Rust records, layout operations, compiler-generated
metadata and argument consumers over together. Reuse the mapped shader execution/
readback fixture through the final public interface, preserve direct physical-pointer
consumers, and remove the temporary root/inline adapters. Shader
resource/type agreement is an artifact-generation/caller obligation on this unsafe
internal path; no SPIR-V reflection or additional shader traversal was added to
pipeline creation. C2 and local C3 remain incomplete.

## C2 argument public cutover

Implemented 2026-11-03 on Vulkan/Linux. Step 1 is complete:

- C and direct Rust expose explicit argument-layout creation, immutable info and
  destruction, plus one `set_arguments` range update. Executable creation borrows
  the layout. Layout ownership and recorded-use lifetimes belong to the caller.
- Deleted the old root/inline setters, root-slot and per-shader interface records,
  owned compatibility layouts and conversion path. Physical-pointer arguments
  remain ordinary inline bytes; mapped buffer fields retain native address roles.
- Foundation schema 2 rejects schema-1 discovery before loading Vulkan. Shader
  records and execution limits changed; consumers must rebuild against this SDK.
  This remains an experimental ABI, with no compatibility implementation.
- Compiler generators emit explicit direct-inline layouts for existing artifacts.
  The mixed native-mapping fixture uses the public C and Rust APIs for inline,
  uniform/read/read-write addresses, partial updates and replay. Automatic inference
  of native buffer roles from arbitrary shader code is not claimed.
- C/Rust examples, generated headers, scene reuse and the GGML/libplacebo consumers
  explicitly own and share layouts. Per-frame scene slots borrow both shared
  executables and layouts; consumer cleanup destroys executables before layouts.

Local evidence on the existing RADV NAVI10 device: 70 CPU unit tests and 23 native
unit tests pass; the public foundation consumer passes; 1,405 installed C/Rust ABI
values and 376 draft values agree. Strict Clippy and compiler generator checks pass.
All six relocated C consumers pass (compute, graphics, learned-image, indexed
scene, indexed ranges and textured scene), including 36 textured frames. Independent
relocated Rust, GGML and libplacebo handoffs pass; 47 scene-runner host tests pass.
The scene migration checks caught missing shared layout handles in frame slots and
a textured layout incorrectly using the plain scene extent; both are fixed. Native
coverage includes mapped buffer execution, reuse/partial updates, guards and
changed-pointee replay. The public mixed-mapping fixture also passes with Khronos
validation and synchronization validation enabled, without validation messages. These are correctness/lowering checks, not new performance claims or
Windows/Metal acceptance.

**Next at the argument checkpoint:** step 2, descriptor storage and command
composition. Its internal storage engine is now implemented below; the public
cutover and borrowed heap/layout composition remain pending. Generated work
currently reads the executable's prepared argument extent; its explicit layout
contract is still pending. Dependencies/submission and presentation follow before
the full C2 audit and local C3. C2 as a whole is not complete.

## C2 descriptor storage engine

Historical internal checkpoint (`cb6e942`), superseded by the public cutover below.
Implemented 2026-11-04 on Vulkan/Linux, below the installed descriptor API:

- Explicit HOST and MEMORY heaps borrow one backing and expose immutable creation
  facts. Native descriptor size/alignment/typed-array stride remain distinct from
  a portable slot. Exact capacity bounds the native bind even with oversized backing.
  MEMORY creation validates usage, address alignment and the supplied live mapping;
  HOST is never shader-visible. Vulkan NATIVE storage is explicitly unsupported.
- Locations validate device/kind, permissions, alignment, capacity and reservation
  exclusion. Resource shape queries share the actual descriptor validator with the
  existing native writer; sampler shapes likewise validate the supplied description.
- Resource/sampler locations feed the existing single native batch encoder. All
  local validation precedes that write. No slot metadata, resource ownership,
  descriptor recipes or duplicate encoding implementation is introduced.
- CPU copies validate the entire batch, including aliases through distinct heap
  wrappers, before copying any bytes. Caller scratch holds translated ranges and
  sorted host intervals; no allocation, temporary descriptor contents or ordered
  `memmove` semantics. Aliases not visible through these mappings remain caller proof.
- Heap binding consumes prepared native information and validates command-domain
  access, independently of capacity/initialized contents. It neither encodes nor
  copies descriptors. The new command path retains the inherited-kind rebind rule.

Local checks on RADV NAVI10: 71 CPU tests and all 24 native tests pass, as does the
existing public foundation consumer (including mixed image/buffer heaps and
secondary inheritance). The new native fixture covers HOST encoding, real sampler
encoding, host-to-mapped-MEMORY CPU copying, mapping/permission/reservation failures,
late-batch rejection without writes, and alias rejection without partial copying.
A one-buffer heap shader is recorded before descriptors are written; two submissions
then write different entries into the same heap location. Both target outputs and
all guards pass with one native bind and exactly two native descriptor-write calls.
The fixture passes with Khronos and synchronization validation enabled without
messages. Pinned shader reproduction and strict Clippy also pass. No timing claim
or cross-backend acceptance follows from these checks.

**Temporary migration weight:** the installed schema-2 raw-output writers, byte
limits, heap-binding records and secondary inheritance still exist. Old and new
writer destinations share one validation/encoding path. The internal storage module
and new bind method temporarily permit dead code until the public cutover; these
annotations and old entry paths must be deleted in that cutover. This checkpoint
adds no public API or second supported descriptor model.

**Next:** replace descriptor limits/signatures in C and direct Rust, migrate generated
native indices and all heap consumers, and make secondary inheritance borrow heap
objects. Then give generated command layouts their explicit borrowed argument layout.
Remove the raw-output/bind-record paths and validate the new public contract through
those consumers. Device-local GPU upload and pending disjoint mutation still need
coverage through the new storage interface; the legacy consumer receipts alone do
not close those checks. Dependencies/submission, presentation and full C3 follow.

## C2 descriptor public cutover

Implemented 2026-11-05 on Vulkan/Linux. Foundation schema 3 exposes D's selected
heap requirements, per-description shapes, explicit creation/destruction, immutable
info, locations, writes and CPU copies through C and direct Rust. Secondary
inheritance borrows the same resource/sampler heap objects; primary/secondary
binding uses prepared native information. Shader indices continue to use the actual
queried native stride, without a conversion table or forced common slot.

Deleted the raw-host-output writer entry paths, heap-binding records and translation,
and temporary dead-code allowances. The former descriptor byte-limit record is now
private native geometry; public limits expose native capacities and address/sampler
constraints. Old schemas are rejected; consumers must rebuild. No compatibility
implementation or second supported descriptor representation remains.

All affected C and Rust consumers now own heap wrappers explicitly and destroy them
before backing. Compressed sampling prepares storage when the actual view exists.
Libplacebo prepares each consumer-owned bank on first use, before command recording,
then checks later resource shapes against that bank's typed native stride. The runtime
performs no lazy materialization. Future formats with different geometry can require
that consumer to prepare a different bank; they are not padded or converted by core.

For initial reservation geometry, consumers query a supported capacity from public
limits, bounded by host addressability for CPU-accessible storage, then choose their
actual capacity. This query allocates nothing. A native maximum may exceed the host's
addressable range; treating it as an allocatable CPU heap is deliberately rejected.

Local checks: 71 CPU tests, all 24 native tests, 1,452 installed C/Rust ABI values,
376 draft values, the 147-operation audit and strict Clippy pass. The public C
foundation consumer passes device-local descriptor upload, mixed image/buffer
consumption, changed-descriptor replay, pending disjoint sampler mutation and all
secondary heap inheritance masks. Its mixed resource layout now packs buffers at
their own extent/alignment, without padding them to an image-sized slot. Storage-image
compiler checks build 16 variants. The changed-entry replay fixture passes through
public C/Rust heap, info, shape, write and copy operations with synchronization
validation enabled and no validation messages.
All six relocated C consumers pass, including 36 independently checked textured
frames. The isolated native Rust, independent libplacebo rendering/lifecycle and
GGML transformer/quantized/matrix/lifetime handoffs pass. Scene-runner host tests
pass all 47 cases. No Windows/Metal acceptance or new
performance measurement is claimed.

**Next at that checkpoint (completed below):** finish step 2 by giving generated command layouts their explicit borrowed
argument layout, including token bounds and mapping compatibility. Then migrate
per-entry dependencies and unified submission, followed by presentation and the
full C2 audit/local C3. The descriptor/secondary cutover is complete; C2 is not.

## C2 generated argument-layout cutover

Implemented 2026-11-05 on Vulkan/Linux. Foundation schema **4** adds the borrowed
`ArgumentLayout*` to `GeneratedLayoutDesc` and names token kind 2 `GENERATED_ARGUMENT`.
C and direct Rust use the same operations. Rebuild consumers against the matching
header/library; no old generated-layout record shape or token alias is retained.

- Creation checks device/class, argument bounds and indivisible native-address
  updates using the CPU updater's boundary logic. Sequence identity targets one
  declared four-byte INLINE field. Token arrays remain borrowed only for creation.
- Native PUSH_DATA tokens can supply PUSH_ADDRESS mappings directly. GPU-produced
  addresses retain caller-owned alignment, lifetime and access obligations; no
  CPU readback, expansion or shader rewrite is added.
- Set updates check each executable against the original initial executable's
  native mappings before making any native mutation. The mandatory cold requirements
  query checks the generated layout against the fixed executable or initial member.
  Requery each distinct work pairing and after set changes. Recording trusts that
  pairing and native scratch requirements without field scans or a compatibility cache.
- Equal mappings, extent, execution class and address offsets/alignments are accepted
  across separately created layouts. The common layout-pointer case is immediate.
  Native state-preservation keys are not used as mapping identity. Direct INLINE
  field partitioning may differ; artifact meaning remains a caller/compiler contract.
- Existing set/preprocess lifetimes, synchronization and argument-domain invalidation
  remain explicit. Generated layouts borrow their argument layout through every use.

The compute fixture now uses a GPU-produced native buffer address, alongside GPU
executable selection, count, inline seed and sequence identity. It executes both
fixed/selected modes with integrated/explicit preprocessing, 20 submissions total,
and uses separately created equivalent layouts for selected executables. Negative
cases cover split addresses, undeclared/out-of-bounds identity, missing layouts,
incompatible set updates, and incompatible work queries with unchanged output.
CPU tests check all update ranges against address atomicity, sequence-field rules,
and mapping compatibility independently of state-preservation keys.

**Verification:** 73 CPU unit tests and all 24 native tests pass on RX 5700 XT
(RADV/Mesa 26.2.5). The public C fixture passes with synchronization validation
and no validation messages. The 1453 installed ABI layout values, 376 draft
comparisons, 147-operation surface audit, strict Clippy and pinned Slang shader
reproduction checks pass. All six relocated C consumers and the isolated native
Rust consumer pass; the scene-runner suite passes all 47 host cases. The earlier
GGML/libplacebo integrations were not rerun for this generated-layout change;
their schema checks are migrated with the installed SDK.

**Next:** C2 step 3, per-entry dependencies and unified submission; then presentation,
full cutover audit and local C3. No new generated graphics execution, Windows/Metal
acceptance or performance measurement is claimed. C2 as a whole remains incomplete.

## C2 dependency engine

Implemented 2026-11-05 on Vulkan/Linux as a buildable step-3 checkpoint.
`dependencies.rs` implements the selected per-entry records privately; the
installed C/direct Rust records remain schema **4** until coordinated consumer
migration. Unified submission and acceptance outcomes are still pending.

- Global, bounded-buffer and image entries each retain their own stage/access
  pair in one native dependency call. Multiple globals are supported. An empty
  barrier is a no-op; an explicit zero-access global preserves execution ordering.
- The complete translation uses caller scratch. Every entry validates before the
  one native command. An invalid later entry can change scratch but emits no
  native prefix. No allocation, automatic dependency inference or stage-mask
  union is introduced.
- Split release uses the same entry translator. Batched acquire packs native
  event handles, dependency headers and every nested barrier array into one
  scratch span. Requirements inspect counts/flags only, not event/resource handles
  or barrier-array contents. Host scopes reject for every split entry. Reset,
  endpoint matching, queue ordering and pending lifetimes stay caller-owned.
- Internal capability derivation reports exact enabled queue/feature stage masks,
  byte-range buffer scopes and native split/ownership/binary support. The device
  query is not installed yet. Scope availability does not claim measured overlap.
- The installed translator now shares its buffer-range/ownership validation with
  the engine. The new image path uses the existing state/subresource validator via
  a temporary stack record; it does not duplicate image validation.

**Evidence:** 75 CPU unit tests and all 25 native tests pass on RX 5700 XT
(RADV/Mesa 26.2.5). Native interception verifies separate compute→indirect and
color→fragment globals, a bounded copy→copy buffer edge and an image initialization
in one barrier call, without a cross-product union. Invalid later entries,
foreign resources/events, malformed ownership endpoints, bad subresources,
unsupported flags, undersized/misaligned scratch and host split scopes reject
before native effects. Release/acquire captures match exactly across one batched
wait. Two recorded command lists execute two split events with intervening work,
explicit resets, changed host-input replay and independent guard/readback checks.
The full existing C foundation fixture also passes, including its transfer,
ownership and split consumers. Synchronization validation emits no errors.
Strict Clippy passes. No timing campaign or new platform acceptance is claimed.

**Next at that checkpoint (completed below):** expose the selected dependency records, device-domain limits and scratch
signatures in C/direct Rust; migrate all barrier/split consumers. Delete the old
shared-stage translator, old nested scratch paths, temporary image record and
private-module dead-code allowance in that cutover. Then consolidate submission
and classify its outcomes, followed by presentation and the full C2 audit/local C3.
This checkpoint does not complete C2 step 3 or C2 as a whole.

## C2 dependency public cutover

Implemented 2026-11-05 on Vulkan/Linux. Foundation schema **5** exposes the selected
per-entry dependency records through C and direct Rust. Rebuild consumers against
the matching SDK. This completes the dependency half of C2 step 3; submission is
still pending.

- `BarrierScope` supplies independent stages/accesses for every global, bounded
  memory and image entry. `Dependency` has counted arrays and flags, with no record
  header, common stages or embedded scratch. Empty batches are no-ops; execution-only
  dependencies are explicit global entries with zero accesses.
- Barrier/release take a separate caller `HostSpan`. Barrier requirements take a
  device and global/memory/image counts. Split requirements take a device and wait
  descriptions, read only dependency counts/flags, and cover headers plus all nested
  native arrays in one span. Failures preserve requirements outputs. No endpoint
  registry, host allocation, scope union or implicit reset is introduced.
- `QUERY_DEPENDENCY_LIMITS=19` is device-only and reports one immutable record per
  enabled queue domain. The Vulkan mapping reports byte-range memory scope and
  native split/ownership/binary capabilities with exact enabled stage masks.
  Adapter queries reject this device-specific query; capacity failures preserve data.
- Deleted the old shared-stage translator, nested dependency scratch path, obsolete
  record kind, temporary image record conversion and private engine allowance.
  Image state/subresource/ownership checks have one implementation. Existing split
  creation/reset/destruction and explicit lifetime/ordering contracts remain.
- C fixtures, installed C applications, native Rust headless/presentation sources
  and GGML/libplacebo consumers use the new records. Common-stage fixture helpers
  edit caller-owned entry arrays; the runtime receives independent scopes. Small
  fixed global-only batches may use bounded aligned stack scratch, with native
  capacity checks; larger consumer batches query their required storage.

Public C boundary checks cover exact native scopes, unchanged outputs on query
failure, zero scratch requirements, and sticky recording rejection without a native
prefix. The device query checks enabled-domain filtering; host tests cover device-only
queries and atomic capacity/size rejection. The split fixture retains reverse CPU
recording order, independently prepared endpoints, immediate scratch overwrite,
reset/replay and changed data checks through public C.

**Verification:** 75 CPU unit tests, all 25 native GPU tests and the full public C
foundation fixture pass on RX 5700 XT (RADV/Mesa 26.2.5), with no synchronization
validation errors. The final public-boundary test also passes after cleanup.
Checks cover 1,492 C/Rust ABI values, 376 draft layout/constant values, all 147
current surface operations, 47 scene host tests and strict Clippy. All six relocated
C consumers, the isolated native Rust consumer, and the independent GGML and
libplacebo GPU handoffs pass. Native Rust presentation sources compile; interactive
presentation was not rerun for this cutover.

**Next:** unify single/batch/binary submission, move its translation into one caller
scratch span, expose per-batch acceptance outcomes and delete the superseded paths.
Then migrate presentation and run the full C2 audit/local C3. No Windows/Metal,
new generated-graphics coverage or performance measurement is claimed here.

## Verification and open native questions

At this checkpoint, `tools/check-c1-drafts.py` passes C11/C++17 syntax and all
376 C/Rust size/alignment/offset/constant comparisons on Linux x86-64 with GCC
16.2.0 and Rust 1.98.1. `tools/check-c1-surface.py` checks the baseline hash, complete
138-operation disposition, all 37 draft destinations and C/Rust name correspondence.
These are design consistency checks, not runtime or shader execution evidence.

C2 runs focused native-lowering/error checks as each family changes. Local C3 then
uses the existing compute/argument reuse/heap, offscreen graphics, ML, generated-work,
presentation and independent installed C/Rust consumers. Preserve output/guards,
command traffic, caller allocation control and failure outcomes. Measure only
uncertain changed costs; do not reopen the old dependency timing campaign.

Remaining questions are implementation/native acceptance, not competing core designs:
exact native argument budgets/artifact encodings; descriptor geometry and pending
mutation; independently scoped lowering and failure classification; Vulkan lifecycle
failure cases; Windows direct buffer ordering, descriptor-copy limitations, presentation
statuses/resize/teardown and any newer fence-barrier mapping; future Metal drawable,
argument and residency mappings. Available Linux work proceeds without another GPU,
Windows hardware or a Mac. Keep unsupported capabilities honest.

M8 generated graphics/GPU-culling and advanced raster resume after C2/local C3;
M9 rays, M10 memory/interop and M11 compute/media retain their roadmap scope.
C1 completion neither implements those families nor stabilizes this API.
