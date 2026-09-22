# Candidate: borrowed host views and explicit range visibility

Selected 2026-09-22 by the [native host-access result](host-access-results.md).
Implemented as an **ABI-16 experiment, not a stable API**. Acceptance comparison
is pending. Vulkan exposes views; Metal returns UNSUPPORTED for the optional calls.

## Smallest proposed shape

Keep `OgpuBuffer` as the owner. For HOST placement, expose a borrowed view with a
stable CPU pointer, logical byte length and independent-access granularity. Add
explicit range publication and invalidation operations; working names are
`ogpu_buffer_host_view`, `ogpu_buffer_host_flush` and `ogpu_buffer_host_invalidate`.
`OgpuHostView` contains `data`, `size_bytes`, `alignment`, `access_granularity` and `coherent`.
Alignment is the backend-guaranteed base-pointer alignment in bytes; granularity
is 1 for coherent storage or the noncoherent cache atom. See the checked C header.
The explicit 0/1 coherence flag permits omission of cache calls on coherent storage;
granularity alone cannot distinguish a hypothetical noncoherent one-byte atom.
Submission/completion and race-free host accesses remain necessary. Requiring
redundant no-op calls would itself exclude a native optimization.
Do not expose Vulkan memory handles or memory-type indices.

The view borrows rather than creating another owner or a dynamically exclusive
lease. Keep the buffer owner alive for all CPU accesses; explicit command-list
retention is not a reason to lose the caller's access handle. A future owning
wrapper can be library convenience, not mandatory per-frame work. Acquiring a
view neither waits for GPU work nor grants permission to touch busy bytes.
No mapping allocation or map/unmap cycle is required per frame.

Return the host pointer only for HOST buffers; DEVICE remains a distinct access
contract even if its native memory happens to be host-visible. The existing copy
helpers remain useful checked conveniences. They should share range arithmetic
with the new operations, not define the fundamental memory-access model.

## Visibility, ownership and hazards

- CPU access needs completion of conflicting GPU accesses to the affected range,
  not every unrelated use of the buffer. Cached noncoherent memory may enlarge
  that range to aligned atoms. Report the granularity instead of hiding whole-
  allocation exclusion. Coherent storage may report 1; a native backend still
  chooses its actual physical memory type.
- Logical API ranges are checked against buffer size; overflow/out-of-bounds
  rejects before cache calls. Backend cache operations cover containing atoms,
  including privately owned final allocation padding. Independently reused slots
  must not share such an atom. No implicit alias or reachable-pointer analysis.
- Before reading device-written data, establish completion and invalidate the
  affected range. Before a partial-atom CPU write that must preserve neighboring
  device-written bytes, invalidate first as well. Publish CPU writes before a
  GPU use; publication does not wait, submit or insert a GPU dependency.
- Do not invalidate unflushed host writes in the same atom: this can lose data.
  Flush must not silently invalidate after the application has written through
  its pointer. Mapping alone does not establish visibility; unmapping/destruction
  must not be treated as an implicit flush.
- Keep existing explicit GPU dependencies and host-call serialization. The API
  cannot enforce correct accesses through a raw CPU or GPU pointer. No automatic
  waiting, global pending-use rejection or per-frame ownership allocation may
  replace the caller's range synchronization.
- A returned pointer conveys no validity after owner release or device loss.
  Buffer destruction remains ownership release, not a hidden GPU wait. Other
  recorded uses keep their explicit owners; they do not make stale views legal.

The atom and visibility requirements follow
[mapped ranges](https://docs.vulkan.org/refpages/latest/refpages/source/VkMappedMemoryRange.html),
[flush](https://docs.vulkan.org/refpages/latest/refpages/source/vkFlushMappedMemoryRanges.html),
and [invalidate](https://docs.vulkan.org/refpages/latest/refpages/source/vkInvalidateMappedMemoryRanges.html).
The portable contract should describe host/device visibility, not mirror Vulkan
object management. On coherent memory these operations may reduce to validated
no-ops, but retain the synchronization obligations.

## Failure contract to implement and test

Required invalid pointers/ranges must not publish or invalidate anything. Clear
new view outputs on failure; a failed query must not expose a usable pointer.
Zero-length ranges are validated no-ops, including offset equal to logical size.
DEVICE placement rejects even empty operations; loss is sticky, including empty
operations and view queries. Invalid/range failures occur before native cache calls.

A failed flush does not roll back application stores; it does not establish the
promised visibility. A failed invalidate grants no read permission. Do not infer
safe completion from either. Non-loss failures may be retried over the complete
affected range, with no dependent access until successful. An unknown error is
still a failure, never visibility evidence; retry requires a successful subsequent
operation. Native cache operations do not destroy/rebind allocation ownership.
Already returned C pointers cannot be revoked mechanically: loss/error obligations
are caller contracts, not a promise of memory-safe sandboxing.

## Alternatives not selected first

Mandatory map/unmap leases would add a per-access state machine and can serialize
independent ranges unnecessarily. They may be convenient wrappers, but the native
control demonstrates a persistent view with caller synchronization. Buffer-per-slot
is still legal and useful; it does not remove forced CPU copies and cannot express
the demonstrated one-allocation schedule. A general suballocator, imported memory,
unified HOST/DEVICE placement or implicit hazard tracker is unnecessary here.

## Implementation acceptance

1. Add the smallest Vulkan implementation and C ABI revision with checked view,
   range and failure behavior; reuse existing host mappings. Do not claim Metal
   support without native evidence. An explicitly unsupported optional Metal path
   is preferable to claiming unvalidated cache/visibility semantics.
2. Unit-test overflow, atom expansion, logical/allocation ends, partial-atom
   preservation and zero-length behavior. Inject native cache failures and loss;
   verify no premature output publication or resource release. Coherent hardware
   requires mocked noncoherent-path tests, not a fictitious hardware claim.
3. Add public mapped/separate and mapped/shared policies to a new fixed frontier
   schema, retaining the accepted native controls. Gate range independence through
   the runtime, prove public-owner/recorded-owner lifetime rules, and retain exact
   outputs/guards, matching allocations and all intervals/tails.
4. Test the view through the relocated installed C consumer. Re-run the runtime
   GPU suite on Radeon/llvmpipe and ordinary ABI/binding checks. Accept only after
   comparing equal strategies and explicitly reporting remaining gaps.

Stop with that evidence and a contract decision. Do not expand into host-parallel
recording, DEVICE mapping or general allocation management to complete this step.

## Implemented protocol

Copy helpers remain whole-buffer operations to preserve their existing cross-backend
contract. They share atom-range arithmetic but do not become safe to mix with pending
disjoint GPU uses. Publish direct writes and complete all GPU uses before calling
copies; caller copy data must not alias backing memory. No implicit flush on destruction.

Schema 2 of `host_access.py --views` preserves the native/copied policies and adds
`ogpu-mapped` and `ogpu-shared`: 20 cases, 60 timing processes, 60,000 measured frames
after 6,000 warmups, and 20 separate timing-mode allocation controls. Correctness
uses 1,000 frames/case on Radeon or 64 in software mode. Old schema 1 remains
available. Match shared and separate requested/allocated bytes and memory types;
public shared creation rejects this fixed-budget protocol if its guarded stride
does not divide the reported granularity, rather than silently reallocating.
This runner restriction is not an API restriction; callers can query and size
atom-aligned storage for other workloads.
Both mapped controls skip cache calls when their backing is explicitly coherent.
The installed example deliberately also checks generic cache calls on coherent
storage, so testing that path does not make it mandatory in optimized applications.

The four standalone native gate cases remain controls. `gpu_host_view_ranges`
adds the corresponding real gated schedule inside the runtime, including live
view queries while pending, narrow CPU rewrite and final object release. The C
frontier and installed consumer exercise the exported view/visibility boundary.
`gpu_host_views` covers actual backing with mocked noncoherent operations, retry,
post-store flush failure, coherent bypass, placement/range rejection and loss.

```sh
python3 examples/performance_frontier/host_access.py --views
python3 examples/performance_frontier/host_access.py --views --software
python3 tools/test-install.py --prefix /new/sdk --recording-storage --replay --host-view
```

Successful view creation is the optional-feature check. No new native Metal support,
noncoherent GPU acceptance, automatic range enforcement or stable compatibility is
claimed. Rebuild matching headers/library/artifacts for ABI 16.
