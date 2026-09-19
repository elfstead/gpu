# Command storage and retirement: next design check

Selected by the [small-compute frontier](performance-frontier-small.md), not by
assuming that Vulkan object names belong in the public API. This review does not
change the current header or runtime. The standing
[expressibility gate](performance-expressibility.md) remains open.

## Separate the three lifetimes

1. **Storage:** backing memory/allocator capacity used to encode commands.
2. **Recording:** executable commands, inline argument values and retained
   kernels/heaps/images/buffers. Raw addresses still do not confer ownership.
3. **Submission:** an accepted execution, its completion and temporary resources.

Completion must make submitted resources safe to retire; it need not force
storage capacity back to the driver. Reusing executable commands is different:
their referenced resources remain owned/valid as long as reuse is possible.
Do not let an empty-storage cache accidentally become an executable cache that
retains application resources after terminal observation.

The existing public text says to free native command resources at retirement.
The candidate alternative is to **reset/invalidate recorded native references
before releasing user resources**, while allowing explicitly bounded storage to
survive. Vulkan descriptor heaps allow driver-reserved-range access after binding
command buffers are freed or reset; pool destruction is not the only safe route.
See the [normative lifetime rule](https://docs.vulkan.org/spec/latest/chapters/descriptorheaps.html).
This is a proposed contract revision, not a claim that the current contract already
permits every form of pooling.

## Alternatives to compare

| Shape | What the caller can express | Cost/obligation to examine |
|---|---|---|
| Bounded device-owned empty-storage cache | Existing one-shot calls; backend amortizes allocation | Explicit retention bound, release policy, heterogeneous recording sizes; caller cannot directly budget storage |
| Caller-owned recording storage | Reuse/budget backing capacity across one-shot recordings | Additional ownership object; reject reset while any associated execution is pending |
| Resettable batch owning its storage | Re-record after terminal completion using the same owner | Define independence from surviving receipts and early batch-handle destruction; pending reset must reject |
| Reusable executable recording | Repeated execution without repeated per-command host calls | Stable/mutable argument rules, pointer lifetimes, concurrent execution, timing and heap mutation semantics |

Do not select the last alternative merely because the first native result was
faster. Reset/re-record already recovers most of the measured storage-policy gap.
Conversely, that result does not excuse making replay permanently inexpressible.
Keep inline-root copies and shader-visible pointed-to data distinct: changing
pointed-to bytes is not the same as patching an encoded address or dispatch count.

## Concrete next implementation gate

Prototype storage reuse through one of the first three shapes, with an explicit
retention policy. The smallest runtime experiment is a bounded empty-storage cache;
compare its caller control/retention tradeoff before choosing it as the fundamental
surface. A new public object or reset operation needs ABI/header/SDK tests, not just
a native benchmark. Never disguise a contract change as an internal optimization.

Required checks, using the current Radeon and llvmpipe:

- Repeat the small-compute controls against the same native reset/replay baselines.
  Include host work, setup and retained storage; M1's GPU-heavy latency is insufficient.
- Observe terminal completion while keeping both the receipt and old batch handle
  alive. Previously retained buffer/kernel/heap references must be released exactly
  once; reuse must not resurrect a consumed recording.
- Bind real image/sampler heaps, retire, edit/release them, then reuse storage for
  different heaps and kernels. Native command buffers must be reset or destroyed
  **before** references and driver-reserved ranges can be released or modified.
- Deterministically gate a pending submission. Early storage reset/reuse and heap
  edits must reject; a transient poll error must retain everything. A later
  completion must not retire unrelated earlier receipts behind the caller's back.
- Inject reset/preparation/submit failures and device loss. A rejected submission
  is not waited on; accepted work drains before cleanup. Failed reset must never
  put a still-referencing recording into an empty-storage cache.
- Sweep small/large recordings and surplus retired storage. Show the retention
  bound, release on final owner destruction and no growth with frame count. Native
  application-allocation traces alone do not measure driver-private storage bytes.
- Preserve lazy query ownership/results and timing after terminal retirement.
  A reused command storage object must not overwrite a surviving receipt's query.

Metal may keep destroying its native storage under a relaxed common contract.
Native Metal allocator reuse requires its own validation; no Mac round trip is
needed to examine the Linux contract alternative. This review does not select
multi-queue scheduling, concurrent recording or automatic lifetime inference.
