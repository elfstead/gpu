# C1 consolidation and C2 cutover

Selected 2026-11-03. **C1's foundation contract is complete; implementation is C2.**
The installed SDK/runtime still has the old signatures. This document does not
declare Windows/Metal acceptance or complete the later M8–M11 hardware coverage.

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

The inventory covers all 138 current C operations, including presentation-table
members: 114 signatures remain, 24 are replaced. The 37 draft operations include
the replacements and new explicit storage/layout/query/lifecycle operations.
Each has an exact target C name and direct Rust name. More functions do not imply
more independent runtime models: one argument update, descriptor storage model,
command-storage model, dependency description and submission operation remain.

Retained operations retain their signatures and unaffected obligations, not an
old embedded record that this document replaces. Discovery, allocation/placement,
mapping/cache control, image/view creation, command arenas/replay, work commands,
render state, copies, queries and timeline operations remain. The installed header
is pinned at `40bde34` and its content hash is in the inventory. No unlisted entry
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

Implemented 2026-11-03 on Vulkan/Linux, below the installed API:

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
It does not dispatch a shader consuming the newly mapped buffer declarations.
Existing native/consumer receipts cover the transitional all-inline path. Unsupported
hardware profiles remain explicit exclusions in the consumer, not acceptance.

**Next:** cut the public C/Rust records, layout operations, compiler-generated
metadata and argument consumers over together. Add shader execution/readback for
bound inline constants and uniform/read/read-write address fields, preserve direct
physical-pointer consumers, and remove the temporary root/inline adapters. Shader
resource/type agreement is an artifact-generation/caller obligation on this unsafe
internal path; no SPIR-V reflection or additional shader traversal was added to
pipeline creation. C2 and local C3 remain incomplete.

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
