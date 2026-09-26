# Consumer-side resource reuse

Implementation of the first two slices of the [M3 brief](../../docs/resource-reuse-plan.md).
No runtime changes or allocator performance claim. The mapped-stream control below
now connects these helpers to the existing workload; see the M3 status for accepted
run coverage rather than treating a successful build as GPU evidence.

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
runtime/GPU failure evidence. The separate [mapped-stream checkpoint](../../docs/resource-reuse-results.md)
accepts sustained success-path integration and allocation accounting at `6ef0328`;
neither follows from CPU helper tests alone.

## Mapped-stream integration

```sh
python3 examples/resource_reuse/run.py --check
# Select VK_DRIVER_FILES and enable Vulkan/sync validation first:
python3 examples/resource_reuse/run.py --preflight
python3 examples/resource_reuse/run.py         # 1,000 small frames/configuration
python3 examples/resource_reuse/run.py --scale # also 720p and odd-video cases
```

The opt-in `FRONTIER_REUSE` build of the existing stream compares dedicated buffers
with three parent buffers: DEVICE data, HOST upload and HOST readback. Every mode
uses the same mapped-host copying/cache policy and unchanged shader artifacts.
Each slot has caller-owned recording storage; `ogpu` resets/re-records and `compiled`
replays a serial immutable list. Raw-pointer backing remains live until all lists
and executions retire. The old public/native controls remain separately built.

A one-slot reference first passes the independent CPU pixel oracle and saves two
guarded images. All later frames must pass BOTH that CPU gate and byte-exact serial
comparison. Input/weights, intermediate guards and HOST inter-range/trailing padding
are checked after drain. The trace verifies allocation/free/peak counts, not private
driver command-memory size. Runtime shaders, arithmetic and tolerances are unchanged.
Inputs alternate on each reuse of each slot; merely alternating global frame input
would leave two-slot streams permanently assigned A/B and miss stale uploads.

To keep requested bytes equal between strategies, HOST capacity budgets round each
logical range to 4 KiB in both modes. This is a declared test budget, not a claim
that a cache atom is 4 KiB: actual views determine placement and flush/invalidation.
Insufficient capacity rejects before recording; no hidden fallback, growing ring or
per-frame backing allocation. DEVICE slices need only scalar alignment here and
have no inter-range gaps. Real noncoherent hardware is not inferred from coherent
driver success; CPU range tests exercise larger/odd granularity independently.

The runner records revision/dirty state, hashes, individual process logs, logical
range ownership, requested bytes and traced peak bytes under a fresh
`target/resource-reuse/stream-*` directory. These runs keep validation/tracing on,
so their elapsed times are diagnostic, not performance evidence. Runtime fault
injection, matched native timing and independent installed handoff remain later
M3 gates. There is no implied GPU race detection from the bookkeeping helper.

At `c4009c0`, optimized checks, AddressSanitizer/UBSan and Clang analysis pass.
LeakSanitizer cannot run under the test environment's tracing; the sanitizer run
uses `ASAN_OPTIONS=detect_leaks=0`. This is not leak-detection evidence.
