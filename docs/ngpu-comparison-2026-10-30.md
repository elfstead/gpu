# Cross-backend API shape versus latest NGPU

Inspected 2026-10-30 after a fresh HTTPS clone of `sebbbi/NoGraphicsAPI` default
branch `main`. Exact upstream commit:
[`ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89`](https://github.com/sebbbi/NoGraphicsAPI/commit/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89),
dated 2026-10-19, "Improve Metal 4 presentation and texture resource handling".
Comparison target: OGPU DX12 candidate `34930b0` and the proposed
[cross-backend contract revision](cross-backend-api-shape-2026-10-30.md).

This is an implementation/source comparison, not an NGPU benchmark or independent
native test run. The earlier comparison in `whole-api-design.md` is pinned to
2026-10-08 and must not be read as a description of today's upstream.

Decision amendment, 2026-10-30: source observations remain as recorded, but the
[minimal composable core rule](design.md#minimal-composable-core) supersedes blanket
recommendations to retain every alternative. The [revised roadmap](roadmap.md#minimal-core-revision)
requires a user-composition recipe or a concrete counterexample before selecting
the core. This comparison is input to that decision, not a mandate for parallel
opaque/byte heaps or multiple root-binding profiles.

## Overall conclusion

NGPU supplies concrete Vulkan/Metal evidence for two of our proposed corrections:
opaque indexed descriptor heaps, and a native root-address ABI rather than an
emulated universal inline byte bank. We should adopt those lessons.

It does **not** show that its selected interface preserves every needed capability
and efficient execution strategy. Some restrictions have concrete counterexamples;
others may be good simplifications for OGPU too. Separate user compositions from
missing primitives before adding alternatives, and distinguish implementation
costs from restrictions inherent in the public contract.

NGPU has native Vulkan and Metal implementations. Windows selects Vulkan, not
D3D12; the backend source selection is explicit in [CMake][build]. Thus it does
not solve the DXIL nested-pointer boundary exposed by our Windows branch.

## At a glance

| Question | Latest NGPU implementation | OGPU disposition |
| --- | --- | --- |
| Descriptor representation | Opaque indexed texture/sampler heaps; native updates separate from binding | Strong candidate for one storage model; assess GPU mutation/placement before deciding whether bytes need exposure |
| Arguments | One GPU root pointer supplied per draw/dispatch; native constant-buffer root ABI | Start from one logical interface; distinguish root count, inline/reference semantics and binding reuse before adding mechanisms |
| State invalidation | Explicit rebind at pass boundaries; no Vulkan PSO restore; Metal restores compute binding across internal encoder splits | Specify compatibility/invalidation instead of universal restoration; inspect backend-imposed splits separately |
| Shader portability | Shared Slang sources/structures with target-specific helpers and artifacts | Strong tooling example; verify actual layouts rather than assuming common source means common ABI |
| Barriers/queues | Global barriers, all-stage timeline waits, concurrent Vulkan sharing across selected families | Stage-scoped waits have a concrete overlap case; ranged barriers, exclusive sharing and explicit layouts require separate justification |
| Presentation | Device-associated window/layer, acquisition and combined submit/present with internal synchronization | Useful convenience shape; not the only fundamental lifecycle or submission strategy |
| Indirect work/replay | One-shot buffers; CPU draw count; Metal indirect multi-draw loops | Do not mistake an indirect call for identical native execution granularity; retain richer generated/replay profiles |
| Address-based copies | Backing-address lookup on Metal; optional Vulkan fallback when address commands are absent | Keep native backing/span and raw-address alternatives explicit; do not force a registry into all command paths |
| Residency/storage | Device residency set, internal submission growth and completion tracking | Inspect whether policy removes needed budget/scheduling control; implementation cost alone does not establish an API defect |
| Failure guarantees | Runtime operations largely void/assert-based | Not evidence for OGPU's stronger status, preflight and uncertain-acceptance contracts |

## 1. Descriptor storage: a directly useful solution

The public header provides `TextureDescriptorHeap` and `SamplerDescriptorHeap`
with create/destroy, indexed writes, range copies and independent binding.
It does not expose arbitrary descriptor bytes. [Public declarations][header]

On Metal, texture heaps are `MTLTextureViewPool` objects. Writes call
`setTextureView:descriptor:atIndex:`; copies call `copyResourceViewsFromPool`.
A small buffer supplies the pool's base resource ID to shaders. Binding updates
that buffer address in the argument table, rather than recreating all texture
descriptors. Samplers use an array of resource IDs with native sampler ownership.
[Metal heap implementation][metal-heaps]

On Vulkan, heaps own mapped native descriptor storage. Writers encode native
descriptors; range copies update the selected slots. The implementation also
keeps CPU descriptor storage and copies encoded bytes into the mapped heap,
avoiding reads back from mapped GPU storage. Binding issues native heap-bind
commands, not a recipe-materialization pass. [Vulkan heap writes][vk-heaps],
[Vulkan binding][vk-bind]

**Lesson:** opaque native storage is compatible with caller-controlled indices
and updates. Our DX12 branch's recipe representation is not necessary to retain
the high-level idea. However, NGPU's API hides raw descriptor-byte placement and
mutation even on Vulkan. The revised question is whether one storage model can
expose needed GPU mutation and placement without also exposing raw bytes. Stable
descriptor tables plus GPU-generated indices may already suffice for many users.

## 2. Roots: native ABI, with a deliberate reduction in expressibility

Every NGPU draw/dispatch accepts one application-owned GPU root pointer. Vulkan
pushes only its eight-byte address using `vkCmdPushDataEXT`. Pipeline binding-zero
mapping uses `VK_DESCRIPTOR_MAPPING_SOURCE_PUSH_ADDRESS_EXT`, and shared shaders
declare their root as a `ConstantBuffer`. Metal writes the root address into
argument-table slot 0; slots 1 and 2 serve the texture-pool base and sampler heap.
Neither path uploads the root structure during the draw/dispatch.
[Vulkan root mapping][vk-root], [Vulkan root update][vk-bind],
[Metal root update and work][metal-work], [shader header][shader]

This is a useful refinement of our proposal: distinguish **native argument-buffer
or constant-buffer roots** from generic physical-pointer data access. Binding a
root through the native constant-data path can preserve compiler/driver choices
without pretending it is an inline byte bank. It does not establish that this is
faster on every device; no such timing claim is made here.

The trade-off is visible in the public API: no general partial inline updates,
multiple independent roots or separate root-bind-then-many-launch operations.
Non-null roots are resupplied through the draw/dispatch path. Null is documented
for rootless shaders, not a portable public bind-once strategy. The application
must place scalar/root data in GPU-accessible storage. This is a strong and simple
profile. These differences require composition analysis; they do not individually
justify multiple roots or separate core operations. [Work declarations][header], [root contract][slang]

NGPU also explicitly requires rebinding graphics within each render pass and
compute after a render pass. Vulkan does not restore a cached PSO. Metal does
retain/reapply compute PSO state when its own internal encoder splitting requires
it. This is more specific than claiming "no restoration anywhere".
[Vulkan state contract][vk-doc], [Metal compute encoder][metal-encoder]

**Lesson:** one logical root can contain arbitrary user argument groups. Its count
is not the key limitation. Compare pointer-only storage with inline observation/
lifetime semantics, and examine update reuse separately. Select a simple native
interface with explicit invalidation; add alternatives only for non-derivable
capability or efficient-execution requirements.

## 3. Shared shaders: real work, not a slogan

NGPU has one shared shader header with target-specific implementations: a Vulkan
push-address mapping, Metal constant-address-space roots, Metal resource-ID
texture lookup and sampler wrappers. These are build-time ABI choices; the
runtime consumes precompiled SPIR-V or metallib. [Shader implementation][shader]

Root structures use a constrained shared constant-buffer layout; pointed-to
structures retain C POD layout. `tools/check_root_layouts.py` checks C++ field
layouts, SPIR-V reflection and native Metal compiler assertions. This is a useful
conformance pattern for OGPU's generated Rust/C interfaces. Do not infer compatible
field offsets merely from matching total size. [Root-layout documentation][slang],
[layout checker][layout-tool]

The repository reports 36/37 Metal tests passing in its October 18 run, with a
render-timestamp failure and instrumentation exceptions. That is upstream's
reported evidence for its tested hardware/toolchains, not a test performed here
or proof of the entire advertised hardware baseline. [Validation receipt][validation]

**Lesson:** sharing source and selected data structures across Vulkan/Metal is
demonstrably worth pursuing; a universal runtime argument representation is not
required. This evidence does not make the DXIL pointer problem disappear.

## 4. Synchronization: simple by restricting strategies

NGPU's `barrier` takes producer/consumer stage and access masks without resource
lists. Vulkan emits a global memory barrier and keeps ordinary textures in
`GENERAL`; unified image layouts are optional. Metal maps stage dependencies and
write visibility to producer barriers. These paths support its chosen model.
[Vulkan synchronization][vk-doc], [Metal barriers][metal-barriers]

Submission waits cover all stages and require one application completion point.
Vulkan resources use concurrent sharing across the selected families. That removes
ownership transfers from the public model. It is a simplification, not proof that
exclusive sharing, narrower waits, ranged barriers or explicit layouts are never
useful native strategies. [Submit contract][header], [native submit][vk-submit]

**Lesson:** global dependencies are a strong simple starting point. Their resource
scope is distinct from submission wait stages: a fragment-only cross-queue wait
can permit a draw's independent vertex work while an all-stage wait cannot.
That is a concrete scheduling distinction. It does not prove ranged barriers,
exclusive sharing or explicit image layouts necessary under our modern baseline.
Do not use NGPU as evidence for the broken DX12 barrier lowering.

## 5. Presentation: a smaller product contract

NGPU associates a native window/layer with device creation. `acquire(commands)`
returns an attachment view, then `submit_and_present` uses queue zero. Vulkan owns
the WSI binary semaphores and presentation transitions internally. Metal acquires
a drawable and orders it through native queue drawable waits/signals. A separately
selected Steam compatibility path adds a classic-Metal presentation submission.
[Device/presentation surface][header], [Vulkan present][vk-present],
[Metal submit/present][metal-submit]

This avoids pretending every native API exposes the same explicit acquisition
objects. It also chooses window association, presentation queue and synchronization
policy for the caller. OGPU wants independent surfaces/swapchains, explicit queue
composition and separately observable retirement.

**Lesson:** a small convenience layer can present one pleasant frame workflow.
The fundamental layer still needs honest native lifecycle profiles. Do not copy
NGPU's policy wholesale, nor carry HWND through a Wayland record to avoid designing
that boundary.

## 6. Hidden work and omitted strategies matter to our comparison

The native Metal source contains choices outside our intended core contract:

- A device-wide residency set is updated/committed on resource creation/removal
  and attached to queues. Ordinary submission does not scan every resource, but
  the residency policy is still selected by the library. [Residency code][metal-residency]
- A buffer-address map resolves copy ranges to native buffers, with a documented
  limit of 64 live GPU heaps. Vulkan uses that fallback only without native address
  commands. This is command-side lookup, **not shader pointer emulation**.
  [Address map][address-map], [backend explanation][metal-doc]
- Submission storage grows with `realloc` when needed, and Metal signals an
  internal completion event as well as the application completion. These are
  explicit implementation costs, even though later submissions reuse storage.
  [Metal submission][metal-submit], [Vulkan submission][vk-submit]
- Metal multi-draw indirect wrappers loop over the CPU draw count and emit one
  native draw per record. The public API has no GPU count-buffer or arbitrary
  executable-selection/preprocessing surface. This does not prove NGPU is slower
  than the corresponding native strategy, but it disproves an assumption that the
  same public indirect operation implies one equivalent native command everywhere.
  [Metal indirect operations][metal-work], [public work surface][header]
- Commands are one-shot. Reusing a command pool is not unchanged-list replay.
  NGPU does not establish the replay/secondary/generated contracts OGPU exposes
  or plans. [Vulkan command contract][vk-doc]

NGPU's ordinary API calls generally return no recoverable status and rely on
native-result assertions/diagnostics. That narrower failure contract cannot
validate OGPU's retryability, loss propagation or partial-acceptance guarantees.
[Runtime result handling][vk-errors]

These are scope and engineering choices, not allegations that NGPU fails its own
goals. Our target is broader and must not inherit them unnoticed. Similarly,
NGPU's texture-copy interface demonstrates explicit row/slice pitches, but there
is no D3D12 implementation here to prove our footprint query/mapping decisions.

## What this comparison changes in the proposal

1. **Descriptor redesign is strongly supported.** Start with one explicit storage
   and update model; derive allocation/caching in user code. Decide byte exposure
   from concrete GPU-mutation/placement needs. Keep binding free of rebuilds.
2. **Use one logical argument interface as the starting point.** Native address
   roots are a strong candidate. Compare inline/reference semantics and costs;
   multiple roots and binding styles are not automatically separate primitives.
3. **Specify interface compatibility/invalidation before restoration behavior.**
   Source/compiler checks should demonstrate when reuse is valid. Avoid a host
   shadow whose purpose is to manufacture compatibility.
4. **Use compiled-layout tests across targets.** Adopt the testing principle,
   including offsets and strides, without assuming C++ tooling is our Rust/C ABI.
5. **Retain capabilities through the smallest sufficient mechanism.** Replay,
   GPU-driven counts, independent surfaces and stage-scoped waits have concrete
   use cases that the narrower contract cannot reproduce equivalently. Other
   alternatives still need justification; existing code is not that justification.

The final answer is not "copy NGPU" or "invent a profile for every backend".
Its implementation supports a small shared conceptual model with native
representations. Some restrictions lose useful capabilities; others may be exactly
the simplifications OGPU should adopt. C1 must separate them using composition
and concrete counterexamples, not a blanket preference for optional profiles.

[build]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/CMakeLists.txt#L33-L46
[header]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/include/NoGraphicsAPI/NoGraphicsAPI.hpp
[shader]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/include/NoGraphicsAPI/shader.slang
[slang]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/docs/slang.md
[layout-tool]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/tools/check_root_layouts.py
[validation]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/docs/metal-validation.md
[vk-doc]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/docs/vulkan-support.md
[metal-doc]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/docs/metal-support.md
[vk-errors]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPI.cpp#L86-L122
[vk-heaps]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPI.cpp#L2848-L3010
[vk-root]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPI.cpp#L3020-L3026
[vk-submit]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPI.cpp#L3425-L3529
[vk-present]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPI.cpp#L3544-L3600
[vk-bind]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPI.cpp#L3736-L3773
[metal-residency]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPIMetal.mm#L300-L337
[metal-encoder]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPIMetal.mm#L492-L502
[metal-heaps]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPIMetal.mm#L902-L1050
[metal-submit]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPIMetal.mm#L1297-L1400
[metal-barriers]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPIMetal.mm#L1483-L1498
[metal-work]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/NoGraphicsAPIMetal.mm#L1745-L1850
[address-map]: https://github.com/sebbbi/NoGraphicsAPI/blob/ffd9f2ab6b7442dcfbe0cb0051d84d0edb078d89/src/BufferAddressMap.hpp
