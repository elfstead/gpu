#!/usr/bin/env bash
# Diagnostic loader control only; does not change the runtime allocator.
set -euo pipefail
ulimit -c 0
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
test -f "${VK_DRIVER_FILES:?select exactly one ICD}"
bash "$repo/integrations/libplacebo/build-diagnostics.sh" "${1:?pinned libplacebo checkout required}"
bin="$repo/target/libplacebo-integration"
out=$(mktemp -d "$bin/placement.XXXXXXXX")
export OGPU_TRACE_LOADER=${OGPU_VULKAN_LIBRARY:?explicit real Vulkan loader required}
export OGPU_VULKAN_LIBRARY="$bin/trace-allocations.so"
unset OGPU_DIAGNOSTIC_IMAGE_LOCAL OGPU_DIAGNOSTIC_OMIT
{
    date -u
    git -C "$repo" rev-parse HEAD
    git -C "$repo" status --short
    printf 'ICD=%s\nREAL_LOADER=%s\n' "$VK_DRIVER_FILES" "$OGPU_TRACE_LOADER"
    sha256sum "$bin/perf" "$bin/trace-allocations.so" "$repo/target/release/libogpu.so"
} > "$out/environment.txt"
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1
unset VK_LOADER_LAYERS_DISABLE
for dims in '64 33' '960 540' '1920 1080'; do
    read -r w h <<< "$dims"
    for mode in resident transfers; do
        log="$out/verify-$w-$h-$mode.log"
        OGPU_DIAGNOSTIC_IMAGE_LOCAL=1 "$bin/perf" verify "$mode" "$w" "$h" 64 > "$log" 2>&1
        if rg 'Validation Error:|check failed:' "$log"; then exit 1; fi
        test "$(rg -c '^VERIFY .* PASS$' "$log")" = 3
        rg '^VERIFY' "$log"
    done
done
if test "${2:-measure}" = verify; then echo "Placement verification: $out"; exit 0; fi
test "${2:-measure}" = measure
unset VK_INSTANCE_LAYERS VK_LAYER_VALIDATE_SYNC VK_LOADER_DEBUG
export VK_LOADER_LAYERS_DISABLE='*'
printf 'timing: validation disabled; ordinary untimed perf; resident near-4K\n' >> "$out/environment.txt"
variants=(native original local)
for run in 0 1 2; do
    for offset in 0 1 2; do
        variant=${variants[$(((run+offset)%3))]}
        unset OGPU_DIAGNOSTIC_IMAGE_LOCAL
        engine=batched
        if test "$variant" = native; then engine=vulkan; fi
        if test "$variant" = local; then export OGPU_DIAGNOSTIC_IMAGE_LOCAL=1; fi
        log="$out/run-$run-$variant.log"
        "$bin/perf" "$engine" resident 1920 1080 64 > "$log" 2>&1
        if rg 'Validation Error:|check failed:' "$log"; then exit 1; fi
        test "$(rg -c '^PERF .* PASS$' "$log")" = 1
        printf 'VARIANT=%s run=%s\n' "$variant" "$run"
        rg '^PERF' "$log"
    done
done
echo "Placement results: $out"
