# Small-compute execution frontier — 2026-09-19

Accepted at clean implementation `c437fb9`, following the predeclared
[expressibility brief](performance-expressibility.md). This tests repeated small
compute, not application performance or a globally optimal Vulkan implementation.

## Result

M1's matched-policy result concealed a substantial opportunity: retaining native
command storage is much faster than fresh pool allocation/retirement in this
host-sensitive workload. Native reset/re-record captures most of that improvement;
replay reduces recording work further but does not uniformly improve throughput.
Do not retain the existing command-resource lifecycle as the fundamental design
on the strength of M1. Separate storage reuse from executable replay.

RX 5700 XT / RADV NAVI10, Mesa 26.2.1, Vulkan 1.4.354; Ryzen 9 5900X, Linux
6.18.47. Each frame executes 1 or 64 dependent in-place transforms of 65 uint32s,
using identical generated SPIR-V. One or three independently owned HOST buffers,
one graphics/compute queue. Three fresh processes per configuration, 100 drained
warmups then 1,000 measured frames. No pinning or exclusive-machine control.

Wall milliseconds per 1,000 frames, median of three process totals. Parentheses
are the minimum–maximum of those totals, not confidence intervals:

| Slots / dispatches | Native fresh | Native reset/re-record | Native replay | OGPU |
|---|---:|---:|---:|---:|
| 1 / 1 | 247.115 (219.029–262.713) | 52.887 (52.690–52.965) | 51.194 (51.126–51.477) | 218.378 (217.363–218.950) |
| 1 / 64 | 278.674 (276.802–291.338) | 172.679 (170.451–189.627) | 161.716 (161.368–162.836) | 279.825 (276.718–286.497) |
| 3 / 1 | 132.334 (131.780–189.033) | 14.250 (13.078–14.766) | 13.611 (13.203–14.551) | 116.140 (115.097–189.473) |
| 3 / 64 | 224.348 (222.759–225.656) | 120.367 (119.572–120.671) | 121.649 (121.639–122.590) | 221.064 (220.600–222.790) |

At equal slots/work, OGPU takes about 1.62–8.15 times the native reset/re-record
wall time. That is a measured implementation/lifecycle gap, not an unavoidable
cost of every possible OGPU implementation. Different slot counts change both
latency and command/storage occupancy; do not report their ratios as API overhead.

For 64 dispatches, native reset/re-record host record+submit medians are about
20–21 microseconds versus replay's 11–12 microseconds. One-slot replay improves
wall time here; three-slot replay does not beat reset/re-record. The per-frame
latency/host clocks, all outliers and all 48,000 samples remain in the evidence.
Pool-policy changes affect both host and wait intervals; this experiment does not
assign the full wall difference to a measured allocator function or driver cause.
There are no device timestamps in this host-sensitive comparison.

## Correctness and memory

All 16 Radeon validation configurations pass 1,000 frames each, checking the full
buffer and prefix/suffix guards on every retired frame. All 16 llvmpipe controls
pass 64 frames each, without performance claims. Timed runs check final buffers
after the wall clock stops against an independent affine/modulo-32-bit CPU oracle.
No CPU buffer access precedes slot completion. All success paths avoid queue idle;
cleanup drains all slots before releasing shader-reachable allocations.

Independent traces cover the validation runs and 16 additional Radeon timing-mode
processes (100 warmups + 1,000 frames). Application allocations match all strategies:
400 native bytes for one slot, 1,200 for three (388/1,164 requested). All 96 tracked
allocations across these Radeon/software controls are freed. This excludes native
driver-private command-pool storage: retained storage is a real tradeoff, not zero
memory. The runtime/library/shader are unchanged. No broad runtime GPU suite or
Metal reacceptance is claimed for this experiment-only implementation.

Host acceptance: both C builds use warnings as errors; native undefined-symbol
inspection finds no OGPU dependency. Parsing/oracle checks, six Python evidence
tests and injected pending-slot, reset/submit/wait failure, drain, device-loss and
exactly-once retirement checks pass. Clang static analysis reports no defects
(the Nix wrapper emits unused linker-option warnings during analysis).

## Contract consequence

The current header says completion frees command resources; batch/heap documents
specifically promise native pool destruction before object release. This is a
stronger statement than merely releasing the submission's references. Pool reuse
cannot silently be described as a contract-preserving backend fix without resolving
that promise. The descriptor-heap constraint does not itself demand destruction:
the Vulkan specification permits access to driver-reserved ranges after all binding
command buffers are freed **or reset**. See [descriptor-heap lifetime](https://docs.vulkan.org/spec/latest/chapters/descriptorheaps.html).

The better candidate contract separates:

- **Submission retirement:** establish terminal state, invalidate/reset its recorded
  native references before releasing retained user resources, preserve the receipt.
- **Reusable command storage:** may survive retirement under an explicit bounded
  ownership/reuse policy; no recorded heap/kernel/buffer references survive in it.
- **Reusable executable recording:** retains its referenced objects while reusable;
  caller-owned pointees remain valid, argument mutation and concurrent executions
  need explicit rules. It is not an empty storage cache.

Two storage alternatives remain to test: a bounded device-owned cache, or explicit
caller-owned recording storage. The former preserves call signatures but needs a
defined retention bound; the latter exposes reuse/budget directly but adds an object.
Neither requires a hidden graph scheduler. Reset must precede heap-reference release;
reset failure/device loss must destroy safely rather than cache a live recording.
Metal's current destruction path may continue to satisfy this weaker lifetime rule;
native Metal reuse is not inferred from Linux evidence.

Replay is a separate expressibility concern: the public one-shot API cannot request
reuse of an existing recording and currently obliges repeated command calls. Native
reset/re-record is a concrete legal lower-level strategy for those calls, not proof
that an ideal native OGPU implementation must pay every Vulkan encoding cost.
Do not label the measured reset/replay gap an unavoidable quantitative API tax;
retain the missing reusable-recording expression as an open design issue.

P1: three in-flight independent slots are expressible without a new queue API.
The Radeon exposes another compute/transfer family with four queues, but this run
does not execute it or establish physical overlap. Host concurrency, cross-queue
scheduling, direct mapped/range access, streaming descriptor mutation and compiler
restrictions remain open. Next: the streaming application and a storage-retirement
alternative with heap/lifetime regression coverage, not an API freeze.

## Evidence and reproduction

Use the [runner instructions](../examples/performance_frontier/README.md).
The exporter rechecked complete matrices, retained log hashes, samples/statistics,
allocation traces and source hashes. It does not rerun the GPU oracle.

- [Radeon report and traces](results/performance-frontier-small-radv-2026-09-19/report.json)
- [All 48,000 raw samples](results/performance-frontier-small-radv-2026-09-19/samples.csv)
- [llvmpipe correctness report](results/performance-frontier-small-lvp-2026-09-19/report.json)

Local artifacts: `target/performance-frontier/small-sdhzlejp` (Radeon) and
`small-68liezcl` (llvmpipe). Logs: `/tmp/ogpu-frontier-final-radv.log` and
`/tmp/ogpu-frontier-final-lvp.log`. Earlier preflight runs remain retained. One
generated Python cache was accidentally included in the implementation commit;
the following cleanup commit removes it and adds ignore/prevention rules. No user
data was removed; that generated file remains recoverable from Git history.
