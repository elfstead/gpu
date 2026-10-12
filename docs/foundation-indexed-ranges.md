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

This ports fresh recording of range/count/identity scenes. The sustained one/two-slot
range replay and per-draw-scope/argument controls still require migration. Generated
interfaces and independent SDK packaging for this range caller remain separate
work; the packaged foundation scene currently covers the simpler two-record
reset/replay workload. Historical performance results, native Metal parity, general
graphics completeness and default SDK cutover are not established by this step.
