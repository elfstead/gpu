# Consumer-side resource reuse

First implementation slice of the [M3 brief](../../docs/resource-reuse-plan.md).
No runtime changes, GPU execution or allocator performance claim yet.

```sh
mkdir -p target/resource-reuse
cc -std=c11 -O2 -DNDEBUG -Wall -Wextra -Werror \
  examples/resource_reuse/test_reuse.c -o target/resource-reuse/test-reuse
target/resource-reuse/test-reuse
```

`ranges.h` reserves guarded, aligned payloads inside a caller-provided capacity.
It uses the least common multiple of alignment and cache granularity, with checked
arithmetic; alignment need not be a power of two. Whole spans own complete cache
atoms. No allocation, individual free, compaction, growth or implicit cache work.
Query the **actual** HOST view before choosing ranges. Check backing base alignment
separately and initialize/check all padding and gaps in the GPU consumer.

`slots.h` tracks one mutable range's execution/CPU-consumption cycle. Each acquire
issues a monotonically increasing, slot-local ticket; generation wrap rejects.
Successful submission moves recorded → pending; confirmed completion moves pending
→ ready; CPU consumption precedes release. Known unsubmitted work may abort.
Unknown submit outcomes and terminal failures quarantine the slot; explicit proven
drain allows teardown but never reuse. Incomplete polls and transient errors leave
it pending. Calls assert facts supplied by the caller; they cannot establish that a
real GPU completed, stop direct pointer writes, or discover transitive ownership.

Tickets must stay associated with their slot; they are not globally unique handles.
Externally serialize all helper access. Keep actual memory, list, recording and
completion owners elsewhere. A compiled list may retain backing after an execution
is drained, so the helper's drained predicate alone never authorizes freeing it.

CPU tests cover 23,808 mixed-alignment ranges, boundaries/overflow and unchanged
outputs on rejection, 1,000 reuse generations, stale tickets, pending/ready reuse
rejection, known-rejected submission, unknown-outcome/terminal quarantine, explicit
drain and generation exhaustion. This is bookkeeping validation, **not** injected
runtime/GPU failure evidence. Sustained stream integration and physical allocation
accounting are the next slice; neither is accepted by these tests.

At `c4009c0`, optimized checks, AddressSanitizer/UBSan and Clang analysis pass.
LeakSanitizer cannot run under the test environment's tracing; the sanitizer run
uses `ASAN_OPTIONS=detect_leaks=0`. This is not leak-detection evidence.
