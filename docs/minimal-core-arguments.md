# C1 A/B: one argument update and explicit native reuse

Selected design, 2026-11-03. **Not implemented.** Resolves the argument-update
and reuse decisions A/B in the [C1 draft](minimal-core-contract.md). The exact
[C record/signature sketch](drafts/ogpu_arguments.h) is separate from the installed
header. [Descriptor decision D](minimal-core-descriptors.md) is now also selected;
presentation P and the complete C1 migration remain open. No native Windows or
Metal execution is claimed.

## Selected shape

One prepared argument layout describes one logical record. One operation updates
any supported byte range of that record independently of drawing/dispatching.
Native buffer-address fields are atomic eight-byte updates; inline fields admit
four-byte subranges. Layout metadata chooses the interpretation before recording.
There is no root object, root-slot setter, argument-buffer allocator, current-value
cache or per-operation argument resupply in the core.

A layout containing one native constant-buffer address supports a single-root
program. An inline record supports small scalar/pointer arguments without a GPU
argument allocation. More logical groups can be application structures reached
through that root. When direct native delivery of several buffer references matters,
those references are fields of the same record, not another argument API.

Select byte ranges over field-indexed updates because they already express one
root, a scalar patch and a whole record with one operation. Forcing field-at-a-time
calls would split a contiguous Vulkan update; forcing whole-record calls would
resupply unchanged data. A cold offset lookup and declared atomic fields make
range validation/lowering proportional to the touched region, not the full layout.

## Records and preparation

The header defines these schemas; the [Rust mirror](drafts/arguments.rs) uses
`#[repr(C)]` records with `u32`, `u64` and borrowed pointers. A Rust-native operation may
take references/slices after the same record validation; it does not call C.

| Record | Contents and meaning |
|---|---|
| `argument_field` | Logical byte offset/size, kind, address alignment, shader visibility and compiler resource space/binding |
| `argument_layout_desc` | Compute or graphics execution class, logical record size and field array |
| `argument_state_info` | Native storage domain, binding-compatibility key, logical-to-storage base offset, size and bind invalidation rule |

Layout creation is cold and may allocate a direct lookup table and native interface
objects. The device is borrowed. Layout destruction requires all referring
executables and command uses to have ended; it neither waits nor destroys them.
Executables explicitly borrow a layout, so several pipelines can share one native
interface without a device-wide interning cache. The artifact must agree with its
layout; preparation rejects mismatched resource roles, sizes, stages and bindings.
The former embedded argument-layout record must not remain a competing authority.

Preparation rules:

- `byte_size` and inline field extents are multiples of four; nonempty fields are
  ordered by increasing offset, nonoverlapping and inside the record. Padding is
  allowed. Empty layouts have zero fields and zero size. Arithmetic is checked.
- An address field is eight bytes at an eight-aligned logical offset. Its requested
  pointee alignment is a power of two that satisfies the selected native binding.
  An unsupported requested interface rejects at preparation; it is not spilled.
- Field stages belong to the execution class and agree with the linked artifact.
  Visibility does not promise physically independent state for different stages.
- `INLINE` describes captured bits, including scalar values and physical-pointer
  bits. Shader pointer dereference is a separate artifact capability. Arbitrary
  inline bits do not imply a native buffer binding or automatic pointer validation.
- `UNIFORM_ADDRESS`, `READ_ADDRESS` and `READ_WRITE_ADDRESS` describe native buffer
  references with those access roles. Their `space/binding` identify exactly one
  matching artifact resource. These are not arbitrary nested shader pointers.
- For `INLINE`, a resource binding identifies a constant-data declaration; the
  `DIRECT` sentinel instead identifies a direct push-data declaration. Artifacts
  can require different representations across compiler targets. Unsupported
  target/artifact mappings reject; no runtime shader rewrite is implied.
- A field's logical offset does not define pointee layout. Generated metadata
  separately checks root-data layout, constant-buffer padding, array strides and
  address space. Host structure size alone is insufficient.

Vulkan can map buffer declarations to push data or an address in push data.
D3D12 can map constants and buffer references through its native root signature.
The mapping belongs to preparation; the update operation does not rediscover the
shader interface. [Vulkan mapping][vk-mapping], [D3D12 constants][dx-constants]

## Updates, observation and failure

`set_arguments(encoder, layout, offset, size, data)` targets the executable currently
bound for that layout's execution class. It validates the entire requested range
before emitting any native update. The caller supplies a valid host input extent;
it may be unaligned and is borrowed only during this call.

An update may cover several fields and padding. Inline subranges must be four-byte
aligned. Any address field touched must be supplied in full and satisfy its declared
address alignment. Splitting a native buffer address into two updates is invalid;
OGPU does not retain half of it to merge later. An application manipulating physical
pointer bits as data may declare them inline instead, with the shader-side validity
obligations that entails. A zero-sized update is a no-op after encoder/layout checks.

Only the supplied bits/address values are captured. Pointees remain application
storage read during execution; every pending/replayed use must satisfy lifetime,
visibility, bounds and aliasing obligations. Null address bits can be recorded, but
are not a promise of valid null-buffer reads on a target without that guarantee.
Changing or destroying host input immediately after recording is legal.

Local errors emit no update and poison the recording as today. End reports the
sticky error; cancel/reset follows existing storage rules. No whole-value rollback,
record-time allocation or command-time reflection is introduced. Successful
recording says nothing about completion or validity of arbitrary shader addresses.

The implementation precomputes range-to-native-location information. On Vulkan,
a contiguous range lowers to one push-data command, even across several fields.
On D3D12 it can lower to several root updates for the affected native parameters.
That is native encoding work for the selected interface, not heap reconstruction.
No unchanged field is restored or inspected for deduplication.

## Reuse: expose storage facts instead of emulating a universal bank

The state-info query is cold immutable metadata, not a state-tracking service.
`domain` identifies argument storage that may overlap, within one live device.
`offset_base + logical_offset` supplies byte coordinates within that domain.
Distinct domains do not overwrite each other's argument values. Updates in the
same domain may replace the intersecting bytes; portable code treats padding in
the updated range as overwritten as well. Unaffected coordinates remain unchanged.
Preparation checks that all coordinate arithmetic is representable. Domain/key
numbers must not be reused while any layout or recorded reference to them lives.

`binding_key` identifies a compatible native interface within that domain. Sharing
one layout shares the key. Backends may report the same key for other genuinely
compatible layouts, but are not required to intern otherwise separate objects.
Keys/domains are equality tokens, not native handles or persistent artifact IDs.

The bind rule is one of two native facts:

- `BIND_PRESERVES`: binding an executable does not invalidate that domain's
  argument bytes, even when its key differs. Changed interpretation is the caller's
  responsibility; no automatic initialization is implied.
- `BIND_INVALIDATES_ON_CHANGE`: binding with a different key invalidates the
  domain's prior argument values. Rebinding the same key preserves them. All layouts
  in a domain agree on the rule. Other domains are unaffected.

Beginning a list leaves argument state undefined. Calling work requires every value
the artifact reads to be initialized and valid. This is a caller obligation, not
a mandatory per-byte validity bitmap in OGPU. Binding never restores remembered
values. An optional external validation/cache layer can track these rules using
the same public metadata; ordinary callers can simply write the required arguments.

For the current Vulkan descriptor-heap path, compute/graphics use the same push-data
domain, with `BIND_PRESERVES` and base zero. D3D12 has distinct compute/graphics
domains and invalidates a domain on root-signature change. This model preserves
both without making either backend simulate the other. [Vulkan push data][vk-push],
[D3D12 binding rules][dx-state]

Storage facts must include native effects. A backend cannot advertise separate
domains backed by overlapping storage without native isolation, or promise
preservation that requires copying cached values. An encoder split, native interop,
secondary execution or generated argument update is a separate state event; its
declared footprints/invalidation must compose with these same coordinates. Existing
conservative secondary invalidation remains in force until that mapping is revised.
Descriptor setup must not occupy unreported user-argument bytes. Decisions D and
the generated-work migration must account for any reserved/native binding state.

## Concrete lowering and reuse cases

Use a graphics layout with an eight-byte uniform address at offset 0 and an inline
constant block at offset 8, size 56. Source bindings are generated with matching
artifact declarations. Logical size is 64 bytes; this does not imply a 64-byte
constant-buffer allocation.

| Request | Vulkan descriptor-heap path | D3D12 mapping constraint |
|---|---|---|
| Update address `[0,8)` | One eight-byte push; prepared address mapping | One graphics root CBV update |
| Patch scalar `[60,64)` | One four-byte push | One root-constant update at the field-relative word offset |
| Update `[0,64)` | One 64-byte push after full preflight | CBV plus the inline block's root constants; no pointee copy |
| Draw 100 times unchanged | No further argument update | No further argument update |
| Update `[4,8)` of the address | Reject before a native call | Same rejection; no half-address cache |
| Bind another executable with the same layout | Preserve values | Same native root signature; preserve values |
| Bind incompatible graphics layout, then return | Push bytes survive binds; intervening writes may overlap | Different signature invalidates graphics values; caller reinitializes |

For mixed compute/graphics, suppose compute overwrites logical `[0,8)` and graphics
previously initialized `[0,64)`. On current Vulkan they share the domain: graphics
needs its first eight bytes restored if their value changed, not the other 56. On
D3D12 the domains differ: compute does not invalidate graphics arguments. An
application can derive exactly these schedules from metadata; no extra rebinding
is imposed by the common interface. Native call/command traffic, not a promise of
equal elapsed time, is the claim to verify during C2.

These examples explicitly bind the relevant executable before changing execution
class. Argument preservation does not imply that the native pipeline itself remains
bound. The full C1 executable contract must keep pipeline binding and argument
storage separate; A/B does not authorize an implicit PSO restore during draw/dispatch.

## Migration and verification boundary

| Current surface | Replacement |
|---|---|
| `argument_interface.byte_size/roots` embedded in shaders | Explicit prepared layout, borrowed by executable preparation; generated shader records declare matching field roles/bindings |
| `set_inline(stages, offset, size, data)` | `set_arguments(layout, offset, size, data)`; layout determines execution class and artifact visibility |
| `set_root(stages, slot, address)` | Generated helper calls the same range update at the declared field offset; no exported root-slot operation |
| Roots used as ordinary SPIR-V physical-pointer bits | Inline fields plus the artifact's pointer requirements; preserve shader behavior rather than silently converting to CBVs |
| Universal preservation at equal byte offsets | Query domain/base/bind rule; reuse where native, explicitly update after actual clobber/invalidation |
| `Executable.roots` and root-slot dispatch | Prepared field/range mapping used by one update path; no second root shadow or retained current values |
| Per-draw payload or hidden resupply helpers | User/tooling helpers only; draw/dispatch consume previously recorded state |

Rust uses the same records/semantics: layout creation and executable preparation
return `Result`, immutable state info is returned by value, and unsafe argument
recording accepts a byte slice after checking the same layout/range constraints.
No Rust object has implicit wait-on-drop or transitively owns GPU data. The C sketch
imports current opaque types only to make signatures concrete; its `ogpu_draft_*`
names are intentionally uninstalled and not an additional supported API version.

C2 must validate the range mapper and atomic-field rejection, emitted offsets and
call counts, same-layout reuse, incompatible layouts and overlapping/independent
domains. Reuse the existing snapshot, partial-patch and argument-reuse consumers;
extend their generated interfaces rather than invent a new performance workload.
Include zero-length, overflow, wrong-device/layout, bad alignment and poisoned-list
cases. Compile the C/Rust record correspondence and test actual artifacts' bindings.

The checked-in sketch selects A/B's contract; syntax/layout checks alone do not
prove a backend mapping. Runtime migration, shader generation, native validation
and any disputed timing remain C2/C3. Metal must report supported roles and native
state effects; a missing inline fast path is unsupported rather than an invisible
upload. No additional hardware is needed to begin the Vulkan implementation.

## Record check, 2026-11-03

`python3 tools/check-c1-drafts.py` compiles the headers as C11 and C++17 and
compares the C/Rust sizes, alignments, member offsets and constants. Set `CC` and
`RUSTC` to explicit compiler executables if they are not on PATH. The script uses
temporary files and runs no OGPU runtime or GPU work.

The initial argument-only check on Linux x86-64 with GCC 16.2.0 and Rust 1.98.1
passed all 33 argument comparisons; the checker now includes descriptor records.
Field/layout/state records are 32/24/32 bytes on this target. This checks record consistency,
not portability to every host ABI, shader layout or native argument correctness.

[vk-mapping]: https://docs.vulkan.org/refpages/latest/refpages/source/VkDescriptorSetAndBindingMappingEXT.html
[vk-push]: https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdPushDataEXT.html
[dx-constants]: https://learn.microsoft.com/en-us/windows/win32/direct3d12/using-constants-directly-in-the-root-signature
[dx-state]: https://learn.microsoft.com/en-us/windows/win32/direct3d12/using-a-root-signature
