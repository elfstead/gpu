# Resident libplacebo performance diagnosis

Selected 2026-09-15 after the [controlled comparison](libplacebo-performance.md).
The near-4K resident workload ran at 553 fps native versus 99 fps grouped OGPU;
host recording CPU was similar. Determine a reproducible implementation cause,
not an assumed API deficiency. Runtime ABI 10, workload, staging and upstream
revision remain unchanged. No shader mathematics, new formats or API expansion.

First source finding: pinned upstream `src/glsl/spirv_shaderc.c` requests
`shaderc_optimization_level_performance`; the adapter's `compiler.cpp` does not.
Test an explicitly labeled performance-optimized compiler variant against the
unchanged default. Use the same perf harness, native control, 8 warmup / 64
measured frames, three rotated fresh-process runs. Gate timing on all three
extents/modes passing same-driver references with synchronization validation on
RADV and llvmpipe. Preserve the original baseline and all observations.

If this does not explain the gap, add diagnostic-only device timestamps and
generated-shader/dispatch capture to separate compute/raster execution from host
collection. Use existing timing contracts, label instrumentation perturbation,
and compare against untimed controls. Inspect image/barrier paths only as the
evidence warrants; do not change several factors at once.

Stop when the dominant gap has an evidenced cause and a retain/change decision,
or report precisely what attribution remains unresolved. A validated compiler
policy correction is within this implementation investigation; a public API
alternative or larger optimization program needs a separate decision.

## Compiler-policy gate

The isolated `OGPU_DIAGNOSTIC_OPTIMIZE` build fails the first llvmpipe backend
specialization test: validation reports an undefined forward-referenced SPIR-V
ID, and compute pipeline creation fails. No optimized timing is accepted.
Reproducer: build with `build-diagnostics.sh`, then run
`backend-tests-optimized specialization-update` under validation. Full local
failure: `target/libplacebo-integration/diagnosis.xUcPLUQX/backend-checks.log`.
The ordinary compiler passes; this does not establish optimization's performance
impact or justify accepting invalid shaders. Upstream sources remain unchanged.

## Baseline profiling instrumentation

`run-profile.sh` builds a separate `perf-diagnostic` executable. It captures the
upstream GLSL, constants, device limits and dispatch grids before measurement.
A pinned backend-table hook (as in the existing reference capture) substitutes
separate native libplacebo compute/raster timers. Link wrappers opt OGPU batches
into the existing public timing API and retrieve durations after terminal
observation, before receipt destruction. Fixed bookkeeping holds at most 32
receipts; no runtime or normal adapter change is needed.

Run all extent/mode correctness gates on both drivers, then three rotated
near-4K resident runs each for native/per-operation/grouped execution with
diagnostic timing off and on. Require all 64 measured timer samples per relevant
pass/frame. Native already uses an internal timer in ordinary dispatch; "off"
means no extra diagnostic timing, not removal of that upstream behavior.
The wrapper clocks and query retrieval can perturb timing. Whole-batch and native
pass durations include their respective dependencies and need not be additive
or exactly equivalent boundaries. Host poll/wait/destroy/query totals include
measured-phase cleanup; they are not a disjoint CPU/GPU decomposition.
