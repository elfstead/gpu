# Retired indexed-scene diagnostics

The foundation scene now covers fresh recording, one/two-slot reset/replay,
range/count/draw identity, persistent per-draw scope bindings, explicit argument
resupply and partial updates, with native correctness traces and independent SDK
handoffs. The old ABI20 timing investigations are no longer an implementation gate.

Current source removes the grouped/scope range timing driver, argument timing,
native draw-order timing and runtime-variant comparison runners, their two parser
test files, and their recording/submission timestamp hooks. It also removes the
ABI20 HOST-write failure interposer and runner. Foundation's separate sticky-error,
unknown-acceptance, wait-failure and cleanup tests remain, including their shared
C-allocation tracker and ASan/UBSan controls.

This retires nine files and unused instrumentation, **not** recorded evidence or
all ABI20 support. Native range/identity/order correctness, argument snapshot and
partial-update controls, count→fixed regression probes, analytic oracles, generated
interfaces and SDK examples remain. No timing is silently redirected to foundation;
its current native command mapping is not a throughput-parity claim.

## Reproduce historical measurements

The last complete checkpoint containing the removed tools is `eaacda8`
("Export independent foundation range, count and replay acceptance"). To inspect
or rerun that tooling without mixing generations:

```sh
git worktree add --detach ../ogpu-scene-controls eaacda8
cd ../ogpu-scene-controls
git submodule update --init --recursive
```

Use that revision's scene README and a separate runtime/build directory. Full
timing runs require matching clean correctness receipts, source/runtime/shader
hashes and the appropriate single-ICD environment; preflight is not timing evidence.
For an exact published result, use **its recorded revision**, not automatically the
last tooling checkpoint. In particular, the fixed payload shader acquired an
explicit `ForceUnroll` in `5a43b6e`; earlier looped-shader timings describe different
artifacts. Never relabel ABI19/20 results as foundation results.

The [range timing](graphics-range-timing-results.md),
[scope/scale results](graphics-scope-scale-results.md),
[argument audit](argument-reuse-plan.md) and their original raw receipts remain
unchanged. Historical plans describe their then-current workflow; the current
[foundation roadmap](plan.md) supersedes it.

## Remaining cutover

Follow-up cutover (2026-10-13): `reuse.py`, `ranges.py`, `range_reuse.py`,
`argument_snapshot.py`, `argument_reuse.py` and `argument_patch.py` now execute
foundation unconditionally. `--foundation` remains a harmless compatibility
spelling. `frontier_public.c`, `reuse_public.c`, the installed snapshot/partial
branches and their unused hooks in `public.c` are removed. Use checkpoint
`c4c16f9` to recover these correctness callers (or `eaacda8` for the earlier
timing suite). Native controls and historical receipt parsing remain unchanged;
the build helper explicitly rejects retired `public`/`public-current` targets.

`public.c` and its generated headers still support the installed basic indexed
scene and `run.py --public`. The ABI20 SDK/runtime has other consumers too. Do
not remove shared old runtime code until those consumers are migrated or retired.

Cutover verification: both local drivers pass all six default runners without
`--foundation`, including bounded reset/replay and per-draw scope matrices, fresh
count/identity output, 8/64/256-byte snapshots, partial-word updates and argument
resupply. All scene host gates pass; the retained installed scene still compiles
warning-clean. This is correctness coverage, not a renewed timing campaign.

Post-cleanup checks (2026-10-12): all current scene host tests pass. Both local
drivers pass the bounded 16-frame native/foundation argument matrix (1,152 frames
per driver) and all 18 generated range cleanup/fail-stop cases with ASan/UBSan.
Generated failure controls additionally require embedded shader bytes to match
their accepted reference. LeakSanitizer remains disabled; this is not exhaustive
allocation-failure or real device-loss evidence.
