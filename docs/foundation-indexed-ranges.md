# Foundation indexed ranges and draw identity

`examples/graphics_scene/ranges.py --foundation` migrates the fixed/count/identity
caller to explicit foundation ownership. It uses the existing API: indirect
argument spans, an optional GPU count span, index binding, inline root bytes,
explicit barriers, caller-owned memory/arena and a timeline. No runtime or public
API expansion is needed. The default ABI20 control remains explicitly selectable.

The consumer generates UINT32 geometry, signed vertex offsets, 1/64/512 indexed
records and a count word on the GPU. Eight frames include empty, partial, full
and above-capacity counts. Counted mode keeps every record nonempty: correct
clamping and empty output must come from the count, not record masking. The caller
checks that even its above-capacity count stays within the device's native limit.

The identity shader observes native `DrawIndex`. Separate calls restart the ID;
multi-draw and counted calls preserve within-call identity. No scan, patch, fusion,
CPU count readback or synthetic identity translation is introduced. This is the
same shader/output workload as the previously accepted native frontier.

## Ownership and acceptance

Each case creates one arena during setup. After the initial upload, each frame
records, submits, waits for its actual timeline point and resets that arena.
There are no per-frame pool creations, device-cache admission decisions, queue-idle
waits or hidden completion objects. Nine GPU allocations are released after
completion. Shader code/executables and root pointees outlive their recorded uses.

```sh
python3 examples/graphics_scene/ranges.py --foundation \
    --native /path/to/clean-native-identity/report.json
```

Select one ICD and enable Vulkan/synchronization validation. `--scale` additionally
checks 1280x720 and requires the corresponding complete native matrix. `--check`
builds public/trace/failure controls and runs host gates without GPU execution.

The runner requires a clean, complete native identity report, verifies artifact
hashes, and rechecks its serial references with the independent analytic oracle.
It compiles the current shaders, checks exact native binary equality, root layout,
workgroup size, required features and stage linkage. This metadata remains
compiler-side; the foundation runtime does not reflect shader code.

Every output byte is compared with accepted native images and geometry, including
depth, counts, padding and guards. An independent identity/geometry oracle is also
applied. Native command traces verify draw strategy and explicit arena policy.
The logical requested-byte budget matches native; actual allocation sizes/types
may differ due to explicit backing/alignment. They must remain stable across the
three strategies within foundation, not falsely identical across backends.

Four test-only failures reject recording/submission or leave submission/completion
unknown. Known rejections destroy the arena before all nine memory allocations and
the device. Unknown completion fail-stops without operand teardown. No successful
wait is fabricated. This is not real device-loss or sanitizer evidence.

## Local checkpoint — 2026-10-12

Radeon/RADV 26.2.4 passes 18 cases / 144 frames at 257x193 and 1280x720;
llvmpipe 26.2.1 passes nine cases / 72 frames at 257x193. All four failure controls
pass on each driver. Reference data comes from clean native reports; these are
correctness comparisons, not fresh native timing measurements. Host negative
tests cover draw identity, profiles, pools/resets and failure receipts. GCC and
Clang compile the caller warning-clean.

## Slot replay and per-draw scopes

`ranges.py --generated` and `range_reuse.py --generated` select typed foundation
interfaces generated from the range compute and DrawIndex raster shaders. Their
embedded SPIR-V must match the accepted native binaries exactly. Generated
requirements drive device/executable checks, and generated root structures supply
the inline bytes; no runtime reflection or policy is introduced. Both local
drivers pass the nine fresh cases and the full bounded 16-frame reset/replay
matrix, including per-draw scopes (1,536 native/foundation frames per driver).
This is not a new endurance or timing run.

`range_reuse.py --foundation --native REPORT` now uses the same foundation
slot scheduler as the simpler scene. Each slot owns its backing, control/readback
memory, arena and timeline; executables are shared and borrowed. Reset records
after completion; replay records once per slot and changes the mapped control
between serial uses. `--scopes` selects separate draws with CLEAR then LOAD
render scopes. Pipeline, index, viewport/scissor and inline bindings survive these
scope boundaries: foundation binds each once per recording, like the native control.
No argument-bank cache, hidden rebind or CPU count readback is introduced.

Default is 1,000 frames per case; `--software` selects 64 and `--preflight` 16.
`--scale` adds 16-frame 1280x720 cases. These are correctness/ownership checks,
not timing. Traces check both compiled and frame-loop command counts, no hot pool
creation, per-retired-frame reset and zero replay recording/reset. Actual allocation
shapes stay stable within each backend; logical requested budgets match native.

`foundation_reuse_failures.py --range-report REPORT` adds the 18 sanitized
failure cases to either scope policy. All pending slots drain before any slot or
shared executable is released; unknown acceptance/completion fails closed.

Local bounded acceptance: both drivers pass 4,608 ordinary-range and 1,536
per-draw-scope frames at 257x193 (counts include native and foundation), plus all
36 sanitized failure cases per driver across the two policies. This does not
claim a new 1,000-frame endurance or 720p run. Existing fresh range acceptance
above remains separate.

## Explicit argument reuse and resupply

`argument_reuse.py --foundation --native REPORT --snapshots SNAPSHOTS` now
compares native-once/native-resupply with foundation-once/foundation-resupply.
All paths change the forward/reverse draw order between recordings, check full
images/geometry/guards, and use the same 8/64/256-byte observable payloads. The
pointer and every scalar word affect output. One/two slots, 1/64/512 separate
draws and 64 frames per case give 4,608 frames per driver including native.
Both local drivers pass that complete matrix.

Foundation exposes the choice directly: bind-once calls `set_inline` once per
recording; resupply calls it before every draw. Native traces verify exact push
counts and byte totals, including forward/reverse draw order. The runtime does
not compare argument bytes or deduplicate caller requests. Both policies keep
pipeline/index/viewport binding counts unchanged. Logical budgets match native;
actual allocation shapes match between policies within each backend. No command
storage is created in the frame loop. This demonstrates mapping and semantics,
not throughput parity or a preference for resupplying unchanged data.

The small fixed payload
shader now uses Slang's [ForceUnroll](https://shader-slang.org/stdlib-reference/attributes/forceunroll-05.html)
so the compiler adapter can prove constant push-array indices before emitting
metadata. The adapter still rejects unproven loop-carried uniformity; this is not
a restriction on native shaders accepted by the foundation API. Fresh snapshot
references must be generated for the changed fixture; historical looped-shader
receipts and timings are not silently reclassified as this artifact. New clean
native/ABI20 snapshot references pass at `5a43b6e` on both drivers. Foundation
recompiles and inspects the exact same binaries, including full root layout,
required features, stage linkage and device inline-size limits.

## Call-time snapshots and partial words

`argument_snapshot.py --foundation` now compares the foundation snapshot caller
with direct Vulkan at 8/64/256 bytes. Four outputs preserve equal/equal/changed/equal
values in one recording. The host argument array is poisoned immediately after
each `set_inline`, before even recording its draw, then zeroed before submission.
Changed pointers and every payload word remain observable. Both drivers pass
full native byte equality and the independent color/depth/guard oracle.

Foundation errors are sticky recording errors, not ABI20's recoverable draw
rejections. This fixture deliberately makes eight misaligned inline updates in
eight separate recordings, requires `commands_end` to reject each with no list,
resets the arena, then records valid work. It does not retry within a poisoned
encoder or pretend the two error contracts are identical.

`argument_patch.py --foundation --snapshots CLEAN_REPORT` compares native and
foundation full updates with explicit four-byte updates at 64/256 bytes. The changed
word affects only color; pointer/geometry/depth remain unchanged. Full foundation
updates emit eight full raster pushes, while partial mode emits four full roots
and one four-byte update. Native traces check exact offsets and byte counts, eight
draws, four scopes, ten balanced allocations, one arena and only two submissions
(initial upload and valid work). Rejected recordings never reach a native end or
submission. Both drivers pass all four strategies for both sizes; this is not timing.
The five foundation C variants (three snapshot sizes and two partial-update sizes)
also execute under ASan/UBSan on both drivers, with no validation/synchronization
errors. LeakSanitizer is disabled; this is not a runtime-wide sanitizer claim.

Clean foundation snapshot reports can now seed the argument-reuse controls without
requiring a new ABI20 execution. Historical installed controls remain explicitly
selectable while their remaining consumers/diagnostics are audited for retirement.

## Independent SDK handoff

The opt-in preview exports `share/ogpu/examples/foundation-indexed-ranges`.
Copy that directory outside the checkout, select `ogpu-foundation.pc`, then run
`python3 build.py` and `python3 run.py`. It builds four public-only executables:
an analytic scene baseline, fresh ranges, slot replay and per-draw scopes. No
Slang, Rust, Vulkan headers or preexisting native reports are needed by the consumer.
Shader sources and generated headers are included for inspection/regeneration.

The runner checks 30 baseline frames, 72 fresh range/identity/count frames and
48 one/two-slot reset/replay/scope cases (768 frames). Sixteen frames per reuse
case cover all eight control variants in both slots. It checks every byte against
fresh independently accepted results, including geometry, counts, depth and guards.
Receipts bind sources/executables/runtime, device and outputs, and remain incomplete
on failure. `--scale` adds the same matrix at 1280x720.

Both local drivers pass the default independent matrix with synchronization
validation. The external-install test relocates the SDK and source directories,
disables Rust/Slang, checks compiler dependency closure and the actually loaded
runtime for all four executables, and verifies missing-loader rejection with an
incomplete receipt. Host mutation tests reject corrupt profiles, count/slot
sequences, budgets, image/depth/geometry/count bytes and guards. Native trace and
fault-injection checks remain separate; this package does not claim to measure them.

The maintainer install check's optional `--shader-check` also regenerates the
installed range compute and identity stage-pair interfaces, deliberately reverses
the four compute-root fields, rejects the stale header, then regenerates/rebuilds
without changing any host C. The complete fresh/replay/scope matrix must still
pass, and every fresh image/geometry hash must match the original layout. Runtime
identity must stay fixed while the generated header hash changes. Both local
drivers pass this external regeneration check; it does not relax pointer lifetime
or synchronization responsibilities.

The old timing/failure diagnostics are now [retired with replay instructions](graphics-scene-diagnostic-retirement.md);
remaining installed correctness selectors still await cutover. Historical performance results, native Metal
parity, general graphics completeness and default SDK cutover are not established
by this handoff.
