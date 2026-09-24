# Dependency scope: accepted native controls — 2026-09-24

At clean `79ce5c33fdf173064080c3c0db0f37d8540af6b1`, the
[declared P4 controls](dependency-scope-review.md) validate a native split dependency
for A→C with independent same-stage B. The current public global barrier cannot
express that graph without adding A→B or B→C, even after enumerating every legal
command order and barrier position. A buffer-range barrier narrows visibility,
not that execution ordering. This is a **structural contract restriction with a
working native mapping**, not a demonstrated throughput penalty on this GPU.

Select [optional recording-local dependency endpoints](split-dependencies.md) for
public implementation, not a replacement for ordinary barriers. P4 public acceptance
remains open. Do not require a measured speedup before preserving native scheduling
freedom, or call a slower native split control proof that the restriction is harmless.

## Measurements

Same transform and two HOST buffers throughout: X=64 KiB transformed by A/C,
Y=64 KiB or 4 MiB transformed by B. One queue and one execution in flight; fixed
compiled commands, 100 drained warmups and 1,000 measured executions per process.
Wall milliseconds per 1,000 executions, median of three fresh processes, with
process min–max in parentheses. These ranges are not confidence intervals.

| Y payload / order | Native global | Native X-range barrier | Public global |
|---|---:|---:|---:|
| 64 KiB / A\|BC | 95.709 (95.603–99.483) | 95.764 (95.714–95.974) | 95.805 (95.661–95.994) |
| 64 KiB / AB\|C | 95.625 (95.362–95.976) | 95.715 (95.459–95.748) | 95.800 (95.696–99.358) |
| 64 KiB / BA\|C | 96.103 (95.537–96.163) | 95.964 (95.602–96.242) | 96.069 (95.945–96.458) |
| 64 KiB / A\|CB | 95.524 (95.400–95.609) | 95.583 (95.496–95.749) | 96.229 (95.472–96.496) |
| 4 MiB / A\|BC | 1012.870 (1012.483–1013.915) | 1013.020 (1011.873–1015.967) | 1013.207 (1012.876–1013.431) |
| 4 MiB / AB\|C | 1014.002 (1013.612–1014.407) | 1013.547 (1010.998–1013.956) | 1014.331 (1011.184–1015.745) |
| 4 MiB / BA\|C | 1013.385 (1012.940–1015.322) | 1013.051 (1012.975–1013.174) | 1013.818 (1013.147–1013.841) |
| 4 MiB / A\|CB | 1014.517 (1011.048–1015.565) | 1013.480 (1010.716–1013.655) | 1013.689 (1011.776–1016.687) |

Native split `A; set; B; wait; C` takes **97.806 (97.713–98.013)** ms at 64 KiB
and **1015.165 (1012.337–1015.833)** ms at 4 MiB. Versus the lowest-median native
global order at each size, that is 2.4%/0.23% slower; the larger process ranges
overlap. No achieved overlap or split speedup is claimed. Native split's median
host-submit interval is 10.740/13.160 µs versus roughly 10.3/12.6–12.9 µs for global
orders. Its median process p95 latency is 102.809/1024.679 µs; all samples and
per-process tails are retained. No per-region GPU timestamps were collected, so
these measurements do not identify the driver scheduling or cache mechanism.

The stronger schedule is permitted, not guaranteed to run faster. Reset/event
commands add work; a memory-heavy transform need not expose spare execution
capacity. No shader tuning or additional sweep was used to search for a win.
Public/global and native/global medians are close but not an isolated API tax:
public lists use SIMULTANEOUS_USE while these native recordings use serial flags.
The selected public candidate will make that execution choice explicit.

## Correctness, memory and failures

All 26 cases pass 256 full-output/guard checks on Radeon and 32 on llvmpipe:
6,656 and 832 executions. Both X/Y outputs must match the independent uint32
reference after each correctness execution; timing checks the full final buffers
outside its wall window. Primary timing supplies 78,000 samples after 7,800
warmups. Separate traced timing-mode controls add 28,600 executions. Successful
execution never uses queue idle. Failure handling drains before releasing commands,
events or shader-reachable buffers; host injections cover pending rejection,
submit/wait errors, loss and command-before-event destruction.

Every case allocates exactly two application buffers, each payload plus 128 guard
bytes. Total requested and allocated bytes match: 131,328 at small Y and 4,260,096
at large Y. Memory types match native/public policies: Radeon type 5 (coherent,
cached HOST) and llvmpipe type 0. All 156 traced allocations across both drivers'
correctness/allocation controls are freed. Split adds one event object and a
recorded reset/barrier prefix; driver-private command/event memory and total RSS
are not measured. No cross-policy equal total-memory-budget claim follows.

The first run (`3eaa639`, `dependencies-myc82_4w`) is rejected: host event reset
triggered a validation error despite timeline completion. The [brief records the
failure and correction](dependency-scope-review.md#reset-protocol-correction-before-acceptance).
Accepted split replay records GPU reset and explicit reset→set ordering before A;
it does not suppress validation or add queue-idle pacing. First-run samples are
not pooled with this result. The installed layer's exact internal cause remains
unverified; current upstream tracking code suggests a retirement race.

## Reproduction and evidence

```sh
python3 examples/performance_frontier/dependencies.py --check
python3 examples/performance_frontier/dependencies.py
python3 examples/performance_frontier/dependencies.py --software
```

Use the documented single-ICD/synchronization-validation environment. Host C builds
use warnings as errors; both C oracle tests, failure injection, seven graph/evidence
tests and eight existing runner tests pass. Static analysis of the initial controls
reported no defects; no full runtime/SDK/Metal reacceptance was needed for this
benchmark-only change. The runtime remains ABI 16.

[Radeon report](results/dependencies-radv-2026-09-24/report.json),
[78,000 samples](results/dependencies-radv-2026-09-24/samples.csv),
[software report](results/dependencies-lvp-2026-09-24/report.json).
Exports recheck all matrices, graph enumeration, source/log hashes, sample statistics,
device identity and allocation signatures. Both reports identify clean `79ce5c3`
and runtime hash `cb3c5f6a48e2e19bb776b5d797a877506d4c44647e71b99d29e22538297d8fc3`.
Raw folders are `dependencies-sohusei6` and `dependencies-ophzmsfj` under
`target/performance-frontier`. Host: Ryzen 9 5900X, Linux 6.18.47; Radeon RX 5700 XT,
RADV Mesa 26.2.1; llvmpipe LLVM 21.1.8; both Vulkan 1.4.354. Rust 1.97.1/clang 21.
Clocks/affinity are not pinned and the host is not exclusive.

Retained local log SHA-256:

- `/tmp/ogpu-dependencies-build-gpu-reset.log`: `24c8ad92b79b7ab63df94f77254893e7206851be46b6bbe0910e449ba50d4657`
- `/tmp/ogpu-dependencies-radv-final.log`: `b73da0c7a4e3ea91d9973f33f9a9bc02650e1549871b5599cf007a267ea34930`
- `/tmp/ogpu-dependencies-lvp-final.log`: `a1c064ef36ebf9fe614bcbadd385ee97a9f8e778946b68b501a23cb869e1fcff`
