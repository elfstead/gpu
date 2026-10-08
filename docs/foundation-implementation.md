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
Future stage-scoped submission must preserve actual wait/signal scopes rather
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

## Verification so far

- 56 ordinary Rust tests pass, including seven foundation contract, status and
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
  All three foundation GPU tests pass on each available driver. Pinned Vulkan
  bindings reproduce exactly after adding the requirements/property records.
- No-GPU C example/header checks are added to CI configuration; no hosted CI run
  is claimed. GPU consumption of the new allocations awaits the command path.

No timing campaign, GPU queue-overlap claim, real device-loss event, Metal support,
or new SDK support follows. Hardware execution requires sandbox-external GPU access;
the initial sandboxed Radeon discovery failed, then passed with that access.

## Next implementation work

Caller-owned command arenas/lists with submission and placed images/views.
Integrate both argument paths,
expanded graphics/compute and consumers as the coordinated tranche proceeds.
The existing setup policy in ABI 20 is temporary migration weight and should be
removed at consumer cutover, not maintained as a fallback backend. Native connection
loading and the modern-baseline predicate are shared; no old `Rc` device, internal
timeline, command cache or resource-retention policy is reused by the new handles.
