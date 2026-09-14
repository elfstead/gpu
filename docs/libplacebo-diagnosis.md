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
