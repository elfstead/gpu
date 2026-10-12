# Foundation indexed-scene reset/replay

`examples/graphics_scene/reuse.py --foundation` runs the existing native and
public one/two-slot scene through explicit foundation ownership. The default
installed control remains separately selectable; no runtime fallback exists.

Each slot owns attachments, dedicated backing, geometry/index/indirect allocations,
mapped control/readback storage, scratch, an arena and a timeline. Shared shader
executables/device outlive every slot. Reset records anew only after observing
completion; replay records once per slot, then changes the mapped phase/empty
control values between executions. All queued uses are serial per slot. The caller
can have two unretired slots; this is not proof of physical GPU overlap.

The native shaders and A/B/empty/A inputs are unchanged. Both index widths, four
render-scope modes (0/6/7/8), one/two slots and reset/replay retain exact reference
images, geometry and guard checks. Compiler inspection validates the control-pointer
root and shader requirements before execution. The reference reports must be clean,
complete native scene acceptances; their artifacts are hash-checked and independently
rechecked by the analytic oracle. API labels and imported symbols distinguish
foundation from ABI20.

## Checks

```sh
python3 examples/graphics_scene/reuse.py --foundation \
    --reference32 /path/to/clean-uint32/report.json \
    --reference16 /path/to/clean-uint16/report.json
```

Select one ICD and enable Vulkan/synchronization validation. Default is 1,000 frames
per consumer/configuration at 257x193; `--software` selects 64. `--scale` adds
eight-frame per-slot checks at 1280x720; `--preflight` is development-only eight-frame
coverage. `--check` builds and runs host checks without a GPU. There are no timing runs.

`--generated` selects foundation ownership plus compiler-generated typed
control-pointer roots and shader requirements. It checks the generated artifacts
for staleness and exact shader-byte equality with native before running the same
matrix. `foundation_reuse_failures.py` preserves this choice from its input receipt.

Native command tracing proves replay does not record or reset pools in the frame
loop. Foundation creates all pools during setup; reset allocates no per-frame pools
and resets once per collected frame. Each slot has nine live GPU allocations, all
released at teardown. Logical requested-byte budgets match native. Foundation's
explicit alignment/backing may change native allocation sizes/types; these are
recorded and checked for stability **within** each backend, not falsely asserted
identical across backends. Historical installed-control budget checks are unchanged.

`foundation_reuse_failures.py --reuse-report /path/to/foundation/report.json`
adds ASan/UBSan and test-only flush/record/submit/wait failures. Known accepted work
in both slots drains before releasing either slot or shared executables. A rejected
second-slot operation while the first is pending still cleans up. A transient wait
failure permits a real cleanup wait; no completion is fabricated. Unknown submit
acceptance or persistent failed completion fail-stops without operand teardown.
CPU/native allocation counters and pool counts check cleanup. This is injected
failure coverage, not real device loss or LeakSanitizer evidence.

## Local checkpoint — 2026-10-12

Radeon/RADV 26.2.4 passes 64,000 matched 257x193 frames plus 512 at 1280x720;
llvmpipe 26.2.1 passes 4,096 matched frames. Counts include native and foundation.
All 18 sanitized failure cases pass on each driver. The shared fresh-recording
helpers also reproduce the full accepted serial image/geometry matrices for both
widths and tested extents. Strict GCC/Clang builds and host negative tests pass.
Reports retain source hashes and development status; no throughput comparison,
native Metal or second physical GPU result is claimed.

The generated variant additionally passes 4,096 matched frames on each driver,
including native command traces, plus all 18 sanitized failure cases. This is
bounded interface/caller regression coverage, not a repeat of the 1,000-frame run.

## Independent handoff

The opt-in `foundation-indexed-scene` SDK example now builds both a fresh-recording
`consumer` and a `reuse` executable. Both use generated artifacts and only the
public foundation header/library. C11, Python and pkg-config suffice; neither
Slang nor Rust is needed in the copied consumer directory.

`python3 run.py` first independently accepts 30 fresh frames per index width,
then compares replay/reset outputs against those accepted files. Both widths,
four modes, one/two slots and both strategies give 32 cases / 256 reuse frames
at 257x193. `--scale` repeats the complete set at 1280x720. Complete/incomplete
receipts retain source, executable, runtime and output/log hashes. Host negative
tests reject changed profiles, frame order, slot/generation, recording budgets
and missing drain gates. Relocation checks verify both executables' public-only
dependencies and actual loaded runtime; all four copied preview examples pass
on both local drivers. The package has no native trace or timing dependency.

The [fresh-recording range/count/identity caller](foundation-indexed-ranges.md)
now also uses foundation, including slot replay and per-draw render scopes.
Argument bind-once/resupply also matches native through explicit inline updates.
Next migrate snapshot/partial-word controls, retiring historical callers at their last dependency. These remain
separate ABI20 workflows; this does not complete default SDK cutover or broader
graphics coverage.
