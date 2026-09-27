# M3 matched range/submission comparison

Protocol selected 2026-09-27, before timing acceptance. Builds on the
[sustained stream](resource-reuse-results.md), [failure checkpoint](resource-reuse-failures.md)
and existing host-sensitive [replay evidence](command-list-results.md).

Question: can the public contract express the same native strategy under the same
resource/correctness requirements, and what allocation/submission policy should
this consumer use? Matched timing diagnoses this implementation; it cannot approve
the entire fundamental API or exclude untested native strategies.

## Controls and gates

The `FRONTIER_NATIVE` + `FRONTIER_REUSE` build uses direct Vulkan with no OGPU
linkage. Share fixtures, range layout, guarded payloads, cache-atom policy, host
copies, per-slot input alternation, retirement state and wall-clock sampling.
Keep command encoding, resource creation and completion operations independent.
Dedicated images/draw arguments and per-slot weights are unchanged. Native range
cache operations target the actual parent memory range; whole-buffer cache work
must not touch other in-flight slots. CPU fakes test noncoherent range arguments,
but do not claim noncoherent hardware acceptance.

Compare all eight combinations: dedicated/arena × public/native × owned reset/
serial replay. Native reset pairs with public `ogpu`; native replay pairs with
public `compiled`. Require exact native/public allocation sizes/types, equal
requested byte budgets between allocation strategies, identical range layouts and
byte-exact public-serial outputs plus the independent CPU oracle. Runtime, shaders
and tolerances remain unchanged.

- Both drivers: small odd-extent correctness for all one/two/three-slot controls.
  The extended ordinary runner can run 1,000 frames/control with `--native`.
- Performance suite correctness: 64 fully checked frames/control, plus 12-frame
  serial reference per extent. Radeon covers 65×47 → 131×95 and
  1919×1079 → 2561×1441. llvmpipe covers small correctness only.
- Radeon timing: four fresh processes/control/extent/slot count, each with 100
  warmups and 500 recorded frames: 192 processes, 96,000 samples. Rotate the eight
  controls by `2*round + extent_index + slots - 1`, reverse odd rounds. Validation
  and allocation tracing are disabled. No clock locking or device query timing.
- Separate memory controls repeat the measured lifecycle under validation/tracing:
  100 warmups + 64 frames. Check final output from every slot, input/weights,
  intermediate guards and HOST padding, as in the timed path. Only the separate
  correctness runs verify every frame; never present final-only gates as full
  per-frame timing validation.

Retain all timing samples and raw allocation records. Report per-process wall
time/frame, host record+submit, upload/readback costs, median latency and p95 tails;
summarize across process repetitions, not by pretending frames are independent
trials. Setup is an inclusive process setup interval (files, initialization,
pipelines and recordings), not an isolated allocator cost or a cold-cache result.
Suppress native-only per-buffer printing for these controls so it does not bias
setup comparison. Fixed range/identity reporting remains in both paths.

Record requested/allocated/peak bytes and allocation counts, logical guards/cache
padding, and known application stack/sample/fixture storage. Runtime/driver command
storage is explicitly unknown, not zero. Do not infer a meaningful VRAM saving or
speedup from allocation count alone. Explain material mismatches/noise before
choosing a policy; retain prior small-dispatch evidence even if this GPU-heavy
workload finds parity. A better public API alternative remains an option.

## Commands

```sh
python3 examples/resource_reuse/performance.py --check
python3 examples/resource_reuse/run.py --native     # 1,000 small frames/control
python3 examples/resource_reuse/performance.py --software # selected llvmpipe
python3 examples/resource_reuse/performance.py            # selected Radeon
python3 examples/resource_reuse/export_performance.py /run/report.json /new/evidence-directory
```

Select the ICD and enable Vulkan/synchronization validation in the invoking
environment; the runner creates separate validation/tracing and uninstrumented
timing environments. Run GPU suites sequentially and keep binaries/sources stable.
Export rechecks logs, samples, matrix/order, statistics and source/fixture hashes.
The installed-SDK handoff remains a separate final M3 gate after the decision.
