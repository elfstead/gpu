#!/usr/bin/env bash
# Baseline source/dispatch capture and optional device timing, not a new benchmark baseline.
set -euo pipefail
ulimit -c 0
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
test -f "${VK_DRIVER_FILES:?select exactly one ICD}"
bash "$repo/integrations/libplacebo/build-diagnostics.sh" "${1:?pinned libplacebo checkout required}"
bin="$repo/target/libplacebo-integration"
out=$(mktemp -d "$bin/profile.XXXXXXXX")
{
    date -u
    git -C "$repo" rev-parse HEAD
    git -C "$repo" status --short
    printf 'ICD=%s\n' "$VK_DRIVER_FILES"
    sha256sum "$bin/perf-diagnostic" "$repo/target/release/libogpu.so"
} > "$out/environment.txt"
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1
unset VK_LOADER_LAYERS_DISABLE
export OGPU_DIAGNOSTIC_TIMING=1
for dims in '64 33' '960 540' '1920 1080'; do
    read -r w h <<< "$dims"
    for mode in resident transfers; do
        log="$out/verify-$w-$h-$mode.log"
        "$bin/perf-diagnostic" verify "$mode" "$w" "$h" 64 > "$log" 2>&1
        if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then exit 1; fi
        test "$(rg -c '^VERIFY .* PASS$' "$log")" = 3
        rg '^VERIFY' "$log"
    done
done
if test "${2:-measure}" = verify; then echo "Profile verification: $out"; exit 0; fi
test "${2:-measure}" = measure
unset VK_INSTANCE_LAYERS VK_LAYER_VALIDATE_SYNC VK_LOADER_DEBUG
export VK_LOADER_LAYERS_DISABLE='*'
printf 'timing: validation disabled; timed and untimed diagnostic controls; resident\n' >> "$out/environment.txt"
engines=(vulkan operations batched)
for timing in 0 1; do
    export OGPU_DIAGNOSTIC_TIMING=$timing
    for run in 0 1 2; do
        for offset in 0 1 2; do
            engine=${engines[$(((run+offset)%3))]}
            capture="$out/capture-$timing-$run-$engine"
            mkdir "$capture"
            log="$out/run-$timing-$run-$engine.log"
            OGPU_DIAGNOSTIC_DIR="$capture" "$bin/perf-diagnostic" "$engine" resident 1920 1080 64 > "$log" 2>&1
            if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then exit 1; fi
            test "$(rg -c '^PERF .* PASS$' "$log")" = 1
            if test "$timing" = 1; then
                if test "$engine" = batched; then
                    rg -q '^DEVICE type=other-or-frame samples=64 ' "$log"
                else
                    rg -q '^DEVICE type=compute samples=64 ' "$log"
                    rg -q '^DEVICE type=raster samples=64 ' "$log"
                fi
            fi
            printf 'TIMING=%s run=%s\n' "$timing" "$run"
            rg '^PERF|^DEVICE|^HOST|^DISPATCH|^DIAGNOSTIC' "$log"
        done
    done
done
echo "Profile results: $out"
