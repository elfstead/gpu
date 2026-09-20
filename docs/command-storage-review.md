# Command storage and retirement: next design check

Selected by the [small-compute frontier](performance-frontier-small.md), not by
assuming that Vulkan object names belong in the public API. The ABI-13 experiment
below implements the first alternative; it does not select the final surface. The standing
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

The ABI-12 public text said to free native command resources at retirement.
The implemented alternative is to **reset/invalidate recorded native references
before releasing user resources**, while allowing explicitly bounded storage to
survive. Vulkan descriptor heaps allow driver-reserved-range access after binding
command buffers are freed or reset; pool destruction is not the only safe route.
See the [normative lifetime rule](https://docs.vulkan.org/spec/latest/chapters/descriptorheaps.html).
This is an explicit contract revision, not a claim that ABI 12 already permitted
every form of pooling.

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

## ABI-13 implementation experiment — 2026-09-20

The first alternative is implemented. The [repeated comparison](command-storage-results.md)
accepts local safety and warmed small-workload performance, not the final surface.
The Vulkan device owns at most three empty pools with one command buffer each.
Only recordings of at most 256 steps and 64 KiB aggregate inline roots may borrow
or return that storage. Oversized recordings use fresh pools and cannot inflate
the retained cache. Full/failed/lost recordings are destroyed; a successful
terminal observation resets before releasing any owning step or retained buffer.
Reset allocation failures fall back to destruction, while reset-reported loss is
sticky. The cache has no ownership cycle and dies with the final device owner.
This implements the [Vulkan reset operation](https://docs.vulkan.org/refpages/latest/refpages/source/vkResetCommandPool.html)
without the release-resources flag; actual driver-private byte retention is not
measured or bounded by an exact byte count.

No public function or layout changes. ABI 13 makes the semantic relaxation
explicit: callers using the older version are rejected. Batches remain consumed;
this is neither executable replay nor a public caller-owned storage object.
Metal retains its existing destruction policy; no native Mac validation is claimed.

Both Radeon and llvmpipe pass all 22 runtime GPU tests with synchronization
validation. Added coverage includes heterogeneous 0/1/256/257-step recordings,
100 storage-reuse iterations, surplus retirement, unread timing from an old
receipt, final-device cleanup, warm-cache preparation/submit failure, reset OOM
fallback and simulated reset loss after a real wait. Admission root-byte/overflow
boundaries are host tests. Existing deterministic gates now also check unavailable
pending/unobserved storage and unchanged cache state after a transient poll error.
Real image/sampler heaps are edited/released and replaced across cached storage
while consumed handles/receipts survive. Tests do not quantify driver-private
memory or simulate genuine hardware loss.

The caller-control tradeoff is unresolved: this policy gives no explicit trim or
capacity budget, treats large workloads differently and retains storage until the
device's last owner dies. Compare those costs with caller-owned/resettable storage
before calling the final API selected. The repeated comparison closes most of the
original small-workload gap; explicit storage control and replay remain the next
design checks, rather than tuning cache constants as a substitute.
