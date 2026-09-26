# M3 mapped-range stream checkpoint

Accepted 2026-09-26 against clean `6ef0328`: the mapped dedicated/arena integration
and sustained success-path correctness/allocation portion of the
[M3 brief](resource-reuse-plan.md). **M3 is not complete.** Runtime, shaders,
numerical tolerances and public ABI 17 are unchanged.

## What this establishes

The learned-image stream can partition three parent buffers into consumer-owned
ranges: DEVICE data, HOST upload and HOST readback. The same workload also runs
with dedicated buffers, using the same mapped-host policy, logical payloads,
guards and requested buffer-byte budgets. Images and draw arguments stay dedicated;
weights remain replicated per slot in both strategies. No allocation occurs per
frame and no runtime allocator, implicit hazard tracking or new placement API is
introduced.

Each slot owns recording storage and either resets/re-records or replays a serial
immutable command list. It becomes reusable only after successful GPU completion
and CPU readback consumption. Parent buffers remain alive until executions and
lists retire. HOST layout uses the actual view's access granularity; explicit
flush/invalidate paths remain for noncoherent views. The 4 KiB-rounded HOST capacity
budget is experimental setup policy, not an assumed cache atom. Inadequate capacity
rejects before recording, without hidden growth or fallback.

This follows Aaltonen's caller-owned heap/range and optional-utility separation,
as recorded in the [brief](resource-reuse-plan.md#reuse-established-results).
It demonstrates that the current backing-buffer contract can express this bounded
schedule. It does **not** establish that buffer handles are preferable to his
public address-range API, or settle placement, aliasing, multiple queues or image
pooling. A better public API alternative remains welcome.

## Correctness and accounting

Each configuration runs 1,000 frames: one/two/three slots × dedicated/arena ×
owned reset/re-record/serial replay. Inputs alternate on every reuse of **each
slot**. Global ABAB alone would leave two-slot streams permanently assigned A/B,
so it is insufficient to test stale uploads. CPU tests explicitly check the
per-slot pattern.

- Radeon RX 5700 XT/RADV: 36 configurations, covering 65×47 → 131×95,
  1280×720 → 2560×1440, and 1919×1079 → 2561×1441; 36,000 candidate frames.
- llvmpipe: all 12 small configurations; 12,000 candidate frames.
- Four separate one-slot reference processes add 48 frames. They first pass the
  independent CPU oracle, save A/B outputs, and check repeated serial stability.
  Every candidate frame then passes both byte-exact serial comparison and the
  CPU RGB delta ≤1/exact-alpha gate. No tolerance changed.
- Final input/weight integrity, all intermediate guards, HOST inter-range/trailing
  padding and drained slot state pass. Pending-slot reacquisition rejects through
  the consumer helper; this is not runtime detection of arbitrary pointer writes.
- Vulkan/synchronization validation is enabled. Traces show no allocation growth
  and zero live allocations/bytes after teardown in all 52 processes.

Native allocation count, including dedicated images and draw buffer:

| In-flight slots | Dedicated | Three arenas |
|---|---:|---:|
| 1 | 9 | 5 |
| 2 | 17 | 6 |
| 3 | 25 | 7 |

Three-slot peak allocated bytes are almost unchanged:

| Driver / extent | Dedicated | Arena |
|---|---:|---:|
| Radeon / small | 1,360,240 | 1,360,192 |
| Radeon / 720p input | 390,233,056 | 390,233,024 |
| Radeon / odd video | 544,525,456 | 544,525,360 |
| llvmpipe / small | 1,341,748 | 1,341,748 |

Requested buffer bytes match exactly at each extent/slot count; tiny native byte
differences are allocation rounding. This is a reduction in allocation objects,
**not a meaningful VRAM saving or speedup**. Validation and tracing are enabled,
so recorded setup/wall times are diagnostic only. The trace covers explicit Vulkan
device-memory allocations, not all driver-private command memory or CPU storage.

## Reproduction and retained evidence

Select one driver and enable Vulkan/synchronization validation as in the
[example README](../examples/resource_reuse/README.md), then:

```sh
python3 examples/resource_reuse/run.py --check
python3 examples/resource_reuse/run.py         # small, either driver
python3 examples/resource_reuse/run.py --scale # video extents, Radeon
```

The committed [Radeon report](results/resource-reuse-radv-2026-09-26.json) and
[llvmpipe report](results/resource-reuse-lvp-2026-09-26.json) retain clean revision,
source/binary/runtime/tracer hashes, fixture and serial-output hashes, device and
validation identity, every range, and raw allocation/free records. Full local
stdout/stderr logs are hash-identified under `target/resource-reuse/stream-7bt6f51e`
and `stream-ogic396_`; the runner creates a fresh directory on reproduction.
Source and process-log hashes were rechecked after completion. No cross-driver
byte-identity claim follows from separate within-driver serial references.

An earlier full Radeon control at `67e8340` passed but used the weaker global
ABAB pattern. It is not the accepted sustained evidence; the full matrix was
rerun at `6ef0328` after strengthening the pattern and retaining raw traces.

Optimized C builds and CPU range/state, mapped-slice/padding and parser rejection
tests pass. AddressSanitizer/UBSan and Clang analysis pass; LeakSanitizer is disabled
because of the environment's tracing, not counted as leak evidence. Forty ordinary
Rust tests, strict Clippy and formatting pass. See the
[validation receipt](results/resource-reuse-validation-2026-09-26.txt) for regression
commands and coverage. The full runtime GPU/ABI suites were not rerun because this
slice changes examples only; no new Metal or additional physical-GPU claim follows.

## What remains in M3

1. Integrate injected known-rejected submission, temporary observation failure and
   safely drained terminal failure, with structured frame/slot/generation/operation
   reports. Current CPU quarantine tests are not injected GPU failure evidence.
2. Measure setup and steady-state costs outside validation/tracing, using matched
   direct Vulkan controls where this policy adds a new question. Preserve the
   host-sensitive replay evidence; GPU-heavy parity cannot approve the fundamental
   API. Decide allocator/submission policy from those results, not object count alone.
3. Run the helper/reuse consumer against a relocated installed SDK, without checkout
   headers or private runtime access, and document its failure/lifetime obligations.

Keep the allocator consumer-side for this experiment; do not call that a final
public-interface decision. Real noncoherent hardware, concurrent recording,
cross-queue schedules and all other open performance-audit items remain unaccepted.
