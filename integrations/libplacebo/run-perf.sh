#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
test -f "${VK_DRIVER_FILES:?select one ICD file}"
bash "$repo/integrations/libplacebo/build-consumer.sh" "${1:?pinned libplacebo checkout required}"
out=$(mktemp -d "$repo/target/libplacebo-integration/perf.XXXXXXXX")
perf="$repo/target/libplacebo-integration/perf"
{
    date -u
    git -C "$repo" rev-parse HEAD
    git -C "$repo" status --short
    uname -a
    "${CC:-cc}" --version
    glslc --version
    printf 'ICD=%s\nOGPU_LOADER=%s\n' "$VK_DRIVER_FILES" "${OGPU_VULKAN_LIBRARY:-default}"
    sha256sum "$perf" "$repo/target/release/libogpu.so" "$repo/target/libplacebo-upstream/src/libplacebo.so"
} > "$out/environment.txt"
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1
unset VK_LOADER_LAYERS_DISABLE
bash "$repo/integrations/libplacebo/check-backend.sh" > "$out/backend-checks.log" 2>&1
for dims in '64 33' '960 540' '1920 1080'; do
    read -r w h <<< "$dims"
    for mode in resident transfers; do
        "$perf" verify "$mode" "$w" "$h" 64 > "$out/verify-$w-$h-$mode.log" 2>&1
        if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$out/verify-$w-$h-$mode.log"; then exit 1; fi
        test "$(rg -c '^VERIFY .* PASS$' "$out/verify-$w-$h-$mode.log")" = 3
        rg '^VERIFY' "$out/verify-$w-$h-$mode.log"
    done
done
if test "${2:-measure}" = verify; then
    echo "Verified benchmark: $out"
    exit 0
fi
test "${2:-measure}" = measure
# Disable explicit and implicit layers for measured runs. Correctness above used
# the requested validation/synchronization layer. Preserve the configuration log.
unset VK_INSTANCE_LAYERS VK_LAYER_VALIDATE_SYNC VK_LOADER_DEBUG
export VK_LOADER_LAYERS_DISABLE='*'
printf 'timing: VK_INSTANCE_LAYERS unset; VK_LAYER_VALIDATE_SYNC unset; VK_LOADER_LAYERS_DISABLE=*\n' >> "$out/environment.txt"
engines=(vulkan operations batched)
for dims in '64 33' '960 540' '1920 1080'; do
    read -r w h <<< "$dims"
    for mode in resident transfers; do
        for run in 0 1 2; do
            for offset in 0 1 2; do
                engine=${engines[$(((run+offset)%3))]}
                log="$out/run-$w-$h-$mode-$run-$engine.log"
                "$perf" "$engine" "$mode" "$w" "$h" 64 > "$log" 2>&1
                if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then exit 1; fi
                test "$(rg -c '^PERF .* PASS$' "$log")" = 1
                rg '^PERF' "$log"
            done
        done
    done
done
awk -f "$repo/integrations/libplacebo/summarize-perf.awk" "$out"/run-*.log > "$out/summary.txt"
echo "Benchmark results: $out"
