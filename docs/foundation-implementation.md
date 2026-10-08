# Foundation revision: implementation status

Started 2026-10-08, following the [whole-API design](whole-api-design.md).
The source-only [next header](../include/ogpu_next.h) contains implemented Linux
Vulkan declarations; the [combined sketch](drafts/ogpu_next.h) adds the remaining
unimplemented surface. Neither is installed by the SDK yet. ABI 20 still owns the
working examples/consumers. This is a migration boundary, not two permanent runtimes.

## Implemented setup

- Immutable discovery snapshot with borrowed adapter handles. Created devices
  retain the native connection, not discovery or a global current context.
- Concrete versioned records for identity, queue domains/counts/granularity,
  memory types/heaps, and available versus requested/enabled optional features.
  Heap size is not presented as free memory/budget. Unknown records/features fail.
- Explicit device queue requests, exact counts and finite priorities. No queue
  clamping, family substitution, aliasing or automatically selected queue zero.
  Device queries report created counts; adapter queries report physical counts.
- Separate timeline objects and non-owning value points; poll, timed wait and host
  signal. No heap-allocated completion receipt, implicit retention or wait-on-drop.
- Object-local host concurrency. Independent native creations/queries/waits share
  immutable device data; terminal loss is atomic, not a device-wide mutex.

The baseline remains modern Vulkan with descriptor heaps, untyped pointers and
address commands. Raster feature enabling, FP16 and unified image layouts are
explicit optional bits, not silently enabled merely because hardware supports them.
These bits describe native enabling, **not implemented new draw/dispatch commands**.
Sparse/protected queue flags and memory properties describe physical facilities;
their execution features are not implicitly enabled.

Version 1 accepts exact record sizes and rejects record chains/flags it does not
understand. Query buffers belong to the caller: count-only queries allocate nothing;
insufficient capacity reports the required count without a partial array write.
Descriptions are validated before reading the larger record. Every handle creation
clears its output on failure. Native failure outputs are not adopted as handles.

Queue domain IDs map directly to Vulkan families; requests lower to separate native
queue-create records. This follows Vulkan's exact family/count/priority rules,
without inventing a portable scheduler. See the
[native queue-create contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkDeviceQueueCreateInfo.html).
Stage-scoped submission preserves actual wait/signal scopes rather
than treating every signaled value as whole-list completion; see
[native semaphore submit scopes](https://docs.vulkan.org/refpages/latest/refpages/source/VkSemaphoreSubmitInfo.html).

## Implemented backing and ranges

The next header also implements explicit linear/opaque allocations, compatible
memory-type/size/alignment requirements, non-owning 64-bit spans, GPU addresses,
persistent mappings, and range flush/invalidate. Callers choose the exact memory
type and explicit exclusive/concurrent queue-family sharing; there is no hidden
HOST/DEVICE preference, staging allocator, resource retention or wait.

Linear backing is one native addressable buffer/allocation which the application
partitions freely. It does not create a native buffer per span. Ordinary alignment
uses Vulkan's native address guarantee with no added padding. Larger requested GPU
alignment uses bounded leading padding, included in the requirements query. Opaque
backing allocates raw memory without a buffer; image placement and mixed-resource
aliasing are not implemented yet. Device byte sizes remain 64-bit; host mapping
alone checks whether an allocation fits the host pointer range.

The native requirements query creates neither temporary buffers nor GPU memory.
Vulkan maintenance4 is explicitly enabled for this path. See the
[requirements query](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetDeviceBufferMemoryRequirements.html)
and [address-alignment guarantee](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetBufferDeviceAddress.html).
Noncoherent views expose both atom size and the first-byte offset within an atom;
cache operations round outward without overflowing or exceeding the allocation.
The application owns synchronization of every affected byte, including neighbors.
Mapping/unmapping one allocation is externally serialized; distinct views share
one mapping, and unmap invalidates them all.

## Implemented command storage and submission

Explicit per-domain arenas reserve primary list slots, not fictitious native-byte
budgets. Begin consumes a reserved slot without growing OGPU storage. Reserve can
grow without moving existing list handles; reset retains capacity, and trim resets,
frees excess slots and asks the driver to release pool storage. Neither destruction
nor cancellation waits. Failed begin/end consume their slot until reset; any reset
attempt invalidates handles, and failed reset prevents recording until a successful
reset. Independent arenas can record on different host threads.

One-shot, serial replay and simultaneous replay map to native command-buffer modes.
The caller owns pending-use tracking, synchronization and resource lifetimes; an
arena retains no memory, timeline or resource list. One-shot work becomes consumed
only on successful submission. Serial completion and simultaneous-use hazards are
caller preconditions, not an internal completion registry. See the native
[submission](https://docs.vulkan.org/refpages/latest/refpages/source/vkQueueSubmit2.html)
and [pool reset](https://docs.vulkan.org/refpages/latest/refpages/source/vkResetCommandPool.html)
contracts.

Submission borrows explicitly sized/aligned caller scratch for native array
translation. Batched ranged memory dependencies use the same approach and lower
to one native barrier call. Query once and reuse stack/heap storage; no OGPU heap
allocation occurs in begin/end, command encoding or submission. Driver-internal
allocations remain possible. This choice does **not** require per-submit allocation
or make a native command-memory byte-budget promise. Duplicate-list/point checks
and signal ordering are trusted preconditions: no quadratic scans or hidden sets.

Copy uses device-address commands and preserves the allocation's known usage flags;
fill writes a selected backing range. Invalid range/device/usage/scope or encoding
errors poison end, which returns no executable list. Global/ranged barriers separate
stage and access masks. Exclusive queue-family ownership transfers require explicit
release/acquire records plus timeline edges; concurrent sharing remains an explicit
allocation choice. There is no implicit current owner or queue serialization.

Wait/signal stages are preserved, with no automatic completion signal. A narrow
signal is not a blanket completion receipt. Submit OOM is retryable with one-shot
work intact; loss is sticky. Unexpected native submission errors conservatively
poison the device instead of pretending a retry or resource reuse is safe.

This is still an unstabilized subset. Image barriers return UNSUPPORTED, secondary
recording/inheritance and split dependencies are not implemented, and the current
stage vocabulary includes broad transfer/vertex/depth groups. Finer graphics scopes
must be added with the executable surface. Multiple lists and timeline edges are
batched in one native submit record; an array of distinct submit records in one
native call remains to implement. None is ruled out by the design. No performance
equivalence claim follows merely from the absence of OGPU hot-path allocation.

## Verification so far

- 57 ordinary Rust tests pass, including eight foundation contract, status and
  cache-boundary/usage tests, plus the expanded C/Rust layout expectations.
- `gpu_foundation_setup` and `gpu_foundation_failures` pass on Radeon RX 5700 XT
  and llvmpipe with validation enabled. They cover exact multi-domain/multi-queue
  creation where available, optional features off/on, four concurrent host workers,
  timeline timeout/signal/poll, failed creation and sticky synthetic device loss.
- `cargo xtask foundation` builds/runs the public C example on both drivers. It
  includes C/Rust layout expectations and device survival after discovery teardown.
- The existing C compute example still passes on Radeon; ABI-20 checks pass
  (847 layout values). Clippy and C11/C++17 combined-header syntax checks pass.
- The public C example now also checks compatible host-visible placement, aligned
  addresses, subrange offsets, shared mapped views, cache calls, and remapping.
  `gpu_foundation_memory` adds invalid descriptions, requirements-capacity atomicity,
  raw opaque backing, concurrent sharing where available, and allocation-failure
  cleanup. Noncoherent native call parameters are also checked with injected calls
  on real backing; this is not evidence from a new noncoherent physical GPU.
  All four foundation GPU tests pass on each available driver. Pinned Vulkan
  bindings reproduce exactly after adding the requirements/property records.
- No-GPU C example/header checks are added to CI configuration; no hosted CI run
  is claimed.
- The C command path passes copy/upload, explicit local working backing, fill,
  global/ranged dependencies, readback and three changed-input replays on both
  drivers. It also covers stable handles across reserve, capacity failure, sticky
  encoding failure, cancellation, one-shot rejection, two host-gated simultaneous
  pending executions, reset and trim. Radeon additionally passes exclusive
  ownership transfer from family 0 to family 1 with a timeline edge and readback;
  llvmpipe reports that it has no second transfer-capable family.
- `gpu_foundation_commands` injects allocation/begin/end/reset/submit failures on
  real devices, checks failed-output adoption, OOM retry without one-shot
  consumption, terminal-loss/unknown-error poisoning and no native retry after
  loss. Four host workers independently record/reset arenas. These tests do not
  claim a real GPU-loss event or measured concurrent GPU execution.

No timing campaign, GPU queue-overlap claim, real device-loss event, Metal support,
or new SDK support follows. Hardware execution requires sandbox-external GPU access;
the initial sandboxed Radeon discovery failed, then passed with that access.

## Next implementation work

Placed images/views and descriptor storage, then both argument paths,
expanded graphics/compute and consumers as the coordinated tranche proceeds.
The existing setup policy in ABI 20 is temporary migration weight and should be
removed at consumer cutover, not maintained as a fallback backend. Native connection
loading and the modern-baseline predicate are shared; no old `Rc` device, internal
timeline, command cache or resource-retention policy is reused by the new handles.
