# Borrowed HOST views: accepted bounded result — 2026-09-24

Retain the [ABI-16 candidate](host-view-candidate.md): borrowed HOST pointers with
explicit range visibility and caller-owned synchronization. At clean implementation
`9a2bad5c17281a7d0ec29c5798c10d7fa23d3e80`, the public API expresses both direct
application production/consumption and independent reuse within one allocation.
Copy helpers remain conveniences, not the only host-access interface.

This closes the **bounded P3 copy/range experiment**, not the complete
[performance-expressibility audit](performance-expressibility.md). Actual
noncoherent hardware remains unvalidated; Metal returns UNSUPPORTED for these
optional calls. No stable API or universal native-parity claim follows.

## Equal-strategy comparison

Schema 2 extends the [native controls](host-access-results.md) with public mapped
and public shared-range policies. All paths use the same integer transform,
alternating per-slot inputs, guards, application producer and summing consumer.
Production, consumption, submission and completion are inside the timed window.
The native binary does not link OGPU. Both mapped implementations omit cache calls
when their backing explicitly reports coherence; generic no-op cache calls remain
tested separately through the installed consumer.

Wall milliseconds per **1,000 frames**, median of three fresh processes; parentheses
are process min–max, not confidence intervals. Compare within each slot budget.

| Slots / payload | Native copied | OGPU copied | Native mapped | OGPU mapped |
|---|---:|---:|---:|---:|
| 1 / 64 KiB | 74.221 (74.168–75.878) | 74.830 (74.640–75.749) | 68.477 (68.457–75.019) | 69.561 (69.022–70.389) |
| 1 / 4 MiB | 1426.675 (1424.334–1428.120) | 1427.001 (1426.099–1432.148) | 1242.364 (1241.257–1244.672) | 1242.044 (1239.279–1245.943) |
| 2 / 64 KiB | 29.835 (28.679–32.019) | 30.019 (29.154–30.115) | 28.874 (28.518–29.097) | 28.599 (28.257–29.742) |
| 2 / 4 MiB | 957.102 (956.749–957.236) | 957.052 (956.475–957.065) | 956.759 (956.533–956.763) | 956.790 (956.518–956.819) |

At one slot/4 MiB, public mapping takes **13.0% less wall time** than public copying,
and is within 0.03% of native mapping's median. At two slots, mapping has essentially
the same throughput as copying: buffering hides the CPU-copy cost here. Public/
native mapped wall ratios across these four cases are 0.990–1.016. Small-case
variation, different host-code generation and submission machinery prevent
identifying this difference as unavoidable API overhead or statistical equivalence.

CPU access work is production + upload/cache + readback/cache + consumption,
microseconds per frame, median of process medians:

| Slots / payload | Native copied | OGPU copied | Native mapped | OGPU mapped |
|---|---:|---:|---:|---:|
| 1 / 64 KiB | 6.350 | 6.470 | 4.034 | 4.130 |
| 1 / 4 MiB | 437.410 | 435.685 | 252.267 | 249.422 |
| 2 / 64 KiB | 6.210 | 6.335 | 4.230 | 3.950 |
| 2 / 4 MiB | 431.601 | 431.697 | 264.812 | 265.768 |

Public mapping reduces CPU access work about **43%/38%** at one/two slots and
4 MiB. Its median process p95 CPU interval falls from 486.085 to 284.407 µs at
one slot and 467.386 to 291.067 µs at two. Direct mapped reads still cost more
than reading freshly copied staging; that cost is included, not assumed away.
At two slots/4 MiB, public mapped latency has median 1.905 ms and median process
p95 1.995 ms versus native's 1.905/1.988 ms. Similar wall throughput does not prove
tail equivalence. Raw samples and all per-process interval/tail summaries are retained.

## Shared ranges, storage and ownership

Two-slot shared mapped wall times are 28.527 (28.462–28.819) ms native versus
28.554 (28.229–30.144) ms public at 64 KiB, and 957.079 (956.888–957.125) versus
957.067 (956.830–957.172) ms at 4 MiB. Sharing changes two allocation objects to
one, not allocated bytes or an established throughput gain. GPU storage is
65,664/4,194,432 bytes per slot including guards, doubled for two slots whether
separate or shared. Copied policies additionally request that much caller staging;
mapped policies request none. This does not account for malloc metadata, total RSS,
command storage or driver-private memory.

Native/public allocation sizes, types and cleanup match within each policy group.
Both drivers select coherent memory: Radeon type 5, flags 14, and llvmpipe type 0,
flags 15. Public views report granularity 1, coherence 1, and base alignment
4096/64 bytes respectively. The reported physical noncoherent atom is 64 bytes.
The fixed shared benchmark rejects strides incompatible with view granularity
rather than silently increasing its storage budget; other applications can size
atom-isolated ranges themselves. That restriction belongs to the runner, not API.

Four standalone native gates per driver retain the independent-range control.
The runtime's `gpu_host_view_ranges` additionally holds range 1 pending while
range 0 is queried, invalidated, read, rewritten and flushed; pending polls before
and after reject accidental waiting. Releasing the gate and replaying range 0
passes exact output/guard checks. The test also distinguishes public ownership,
recorded retention and a non-owning view, without dereferencing a destroyed view.
This gate exercises runtime internals; the C frontier and relocated consumer
separately exercise the exported boundary. Sharing matters to expressibility under
a stated one-allocation budget, not as a claimed general allocation optimization.

## Validation and retained evidence

- Radeon: 20,000 correctness frames; 60,000 uninstrumented measured frames after
  6,000 drained warmups; 20 separate timing-mode allocation runs add 22,000 frames.
- llvmpipe: 1,280 correctness frames, no speed claim. Each driver also runs four
  native gates with three GPU executions each. All 96 traced application allocations
  across correctness, allocation and gate controls are freed.
- Both reports identify clean `9a2bad5`, identical runtime/benchmark artifact hashes,
  explicit driver selection and complete matrices. Export rechecks source/log hashes,
  memory metadata, outputs/checksums, gate evidence and every sample/statistic.
- 39 ordinary tests, 755 C/Rust ABI values, bindings reproduction, loader mocks,
  Clippy and formatting pass; 27 runtime GPU tests pass on **each** Vulkan driver
  with synchronization validation. Frontier C selftests and Python evidence tests pass.
- New tests cover range overflow/end padding, zero length, HOST/DEVICE rejection,
  output clearing, scoped cache operations, partial writes, non-loss retry, post-store
  flush failure, coherent bypass and sticky loss. Noncoherent cache/error paths use
  mocks over real backing, not a claim of actual noncoherent GPU validation.
- The relocated ABI-16 SDK passes default, explicit-storage, replay and direct-view
  C-consumer paths on both drivers. It checks real production/consumption, partial
  updates, guards, negative ranges/placement, ownership and missing-loader rejection.

[Radeon report](results/host-view-radv-2026-09-24/report.json),
[60,000 samples](results/host-view-radv-2026-09-24/samples.csv),
[software report](results/host-view-lvp-2026-09-24/report.json), and
[validation receipt](results/host-view-validation-2026-09-24.txt) identify artifacts
and scope. Acceptance packages the implementation's September 22 measurements
with September 24 software/SDK validation. Earlier pre-coherence-field diagnostic
runs are not pooled into this accepted matrix.

## Decision and next question

The copy-only application boundary was an API restriction, not just backend memcpy
overhead. The candidate removes that restriction and the whole-buffer exclusion
for explicit views; copy helpers deliberately retain their conservative contract.
Keep explicit coherence metadata so a native optimization does not require a
redundant per-access API call. No per-frame mapping lease, automatic wait, hidden
range tracker or public Vulkan allocation handle was needed.

The bounded case is now demonstrated expressible. Keep noncoherent hardware and
other placements unresolved; do not reopen this experiment for unrelated tuning.
Native replay's serial-use flags still differ from public lists' simultaneous-use
support, as recorded under P2; this run does not settle that independent issue.
No Metal, full consumer/video-scale rerun, exact command-memory budget, host
parallelism, descriptor mutation or broader compiler acceptance is claimed.

Next is the [P4 dependency-scope review](dependency-scope-review.md): investigate
whether a strong legal native schedule exists that the global dependency contract
cannot express. Do not infer independent execution merely from a resource barrier.
