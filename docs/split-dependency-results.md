# Split dependencies: public acceptance — 2026-09-24

At clean `2397baf22c74d77a763b90be7da934b39e613ee0` (ABI 17), retain
[recording-local split endpoints and explicit replay modes](split-dependencies.md).
The public API now expresses A→C with independent same-stage B between the
endpoints. Ordinary barriers remain available; this closes the bounded P4
recording-local scheduling experiment, not the complete performance audit.

## Matched comparison

The schema-2 matrix adds public split replay to the original 26 policies/extent
cases, and explicitly selects serial mode for public global replay too. All paths
use the same shader, two HOST allocations, one queue/slot and one compiled command
buffer/list. Both split paths own one event per executable and order GPU reset
before reuse. No host reset, implicit scheduler or per-submit event allocation.

Wall milliseconds per 1,000 executions: median of three fresh processes and
min–max in parentheses, not confidence intervals. Each has 100 drained warmups.

| Y payload | Native split | Public split | Public/native median ratio |
|---|---:|---:|---:|
| 64 KiB | 98.174 (97.178–98.213) | 98.138 (98.102–98.149) | 0.9996 |
| 4 MiB | 1015.151 (1012.027–1018.611) | 1015.635 (1012.479–1016.018) | 1.0005 |

The lowest-median native global orders take 95.443 ms (BA|C, small) and
1011.924 ms (AB|C, large). Public split is about 2.8%/0.37% slower than those
orders. The small penalty is consistent with the native event strategy; the
large ranges overlap. No local split speedup or achieved GPU overlap is claimed.
The structural scheduling freedom remains valuable regardless of these timings.

Median host-submit intervals for native/public split are 10.740/10.900 µs at
64 KiB and 14.310/13.440 µs at 4 MiB. Median process p95 latencies are
104.189/102.749 µs and 1020.680/1020.199 µs. All 84,000 samples and every global/
range order remain in the report, including two late large-global runs around
1029 ms; no outlier removal or selection of a favorable rerun. These measurements
do not isolate a universal API tax or establish tail parity on other workloads.

## Correctness and ownership

All 28 cases pass 256 full X/Y output and guard checks on Radeon and 32 on
llvmpipe: 7,168 and 896 executions. Timing correctness is checked after each
window, outside the clocks. Separate timing-mode allocation controls cover all
28 cases. Each case uses exactly two backing allocations: 131,328 bytes total
at small Y or 4,260,096 at large Y. Native/public sizes and memory types match;
all 168 traced allocations across the two correctness matrices and Radeon memory
matrix are freed. Driver-private event/command memory and RSS are not measured.

Both driver suites pass all 28 GPU tests. New coverage includes malformed/foreign/
ended tokens, correctable unmatched scopes, nested/crossed intervals, one-shot
execution, repeated serial execution with changing data and guards, partial event
creation/encoding failure, unsupported simultaneous split compilation, retry and
poison paths. A real GPU semaphore gate keeps serial work pending, rejects reuse
without waiting and retains recorded ownership after public list destruction.
Transient poll errors retain the reservation; retirement releases it while old
receipts survive. Existing simultaneous replay still passes its three-use gate.
Native command references are reset/destroyed before event release. Loss injection
is synthetic and never substitutes for successful completion of real pending work.

Forty ordinary tests, strict Clippy, generated bindings, 760 C/Rust ABI values,
loader/cleanup mocks and the C/Python frontier checks pass. The independently
relocated SDK executes ordinary/storage/replay/HOST-view/split paths on both
drivers. Its split consumer checks output clearing, unmatched-point correction,
unsupported simultaneous mode, serial pending rejection and early parent release.
No native Metal validation or graphics-split performance claim follows.

## Evidence and reproduction

- [Radeon report](results/split-dependencies-radv-2026-09-24/report.json) and
  [84,000 samples](results/split-dependencies-radv-2026-09-24/samples.csv).
- [llvmpipe correctness report](results/split-dependencies-lvp-2026-09-24/report.json).
- Raw matrices: `target/performance-frontier/dependencies-c2mkccfg` and
  `dependencies-qkhryfn5`, both recorded clean at the revision above. Exporters
  rechecked the complete schema-2 matrices, source/artifact/log hashes, graph,
  samples/statistics, identities and allocation signatures before export.
- Runtime SHA-256: `97fdc2b5fde2cd1d2c288cb2f42e8302e282bb8d3c435cfcef2eda775853aaa1`.
  SDK `/tmp/ogpu-sdk-split-abi17`; relocated consumers
  `/tmp/ogpu-external-s03tebkr` (Radeon), `/tmp/ogpu-external-yalj_u81` (llvmpipe).

Use the established single-ICD synchronization-validation environment:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask bindings --check
cargo xtask abi
cargo xtask mock
cargo xtask gpu-tests
python3 examples/performance_frontier/dependencies.py
# Select llvmpipe, then:
python3 examples/performance_frontier/dependencies.py --software
python3 tools/test-install.py --prefix /new/sdk --recording-storage --replay --host-view --split
```

Retained local receipt hashes (`/tmp/ogpu-split-` prefix, `.log` suffix):

```text
host          e1e5dab45ad1edec12bf9bd3f71ebd9970562ade41853b84cb7cd4198f89a09c
clippy        d181ca5f2dfa85e67000c7eed42da387c32f514c1cb51bb9875ff72d82b5598c
bindings      ab03d56b9a3bd43b029f6473324170ff7843552c8055161a86624a35ec7d6991
abi           9b2330cc6cefca63a527c25e022d43b482839ce930b10bc190ef052bb21c56a7
mock          38e5fc30975f66f966c9dfacec36572343e60f744ae23c3acfedc18f829363f4
radv-tests    21bf34d2427998f491b2aea07c47227335b0a3acd80bc4aabd9dcf6f33119c91
lvp-tests     a59504a5f68882b423cadace02b1c60aa82479827f178814424a0553752d252d
radv-matrix   88e243a219cfc541cf6f496ed7a473823b9be20c7d885811918ed556d3862c65
lvp-matrix    8860657193c4183be8c1b6b7f32b6982bae16995310714f162af2b72d884b7d7
install       a272395cba9010e5cc4aff062473fd87081b4ab0aea9e18d551d84427c423522
radv-sdk      349d70b2449bfa081450ac96756021b3022753635705014aed67a807e6a9ca3e
lvp-sdk       565234c6c561ffa7e8a6b3f27507b47eef4ae95627c843f0ecfc96a4d35bd946
```

Keep simultaneous split replay, cross-recording endpoints, resource-scoped cache
opportunities, total command-memory budgets and other performance-audit items
open. The reset prefix's broad cross-execution ordering is a backend cost, not an
added portable promise. Next is M2's compiler/programming contract and P7 audit;
this acceptance does not stabilize the API or close the language question.
