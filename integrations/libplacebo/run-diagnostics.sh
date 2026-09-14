#!/usr/bin/env bash
# Isolated compiler-policy control; ordinary benchmark binaries stay unchanged.
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
test -f "${VK_DRIVER_FILES:?select exactly one ICD}"
bash "$repo/integrations/libplacebo/build-diagnostics.sh" "${1:?pinned libplacebo checkout required}"
bin="$repo/target/libplacebo-integration"
out=$(mktemp -d "$bin/diagnosis.XXXXXXXX")
{
    date -u
    git -C "$repo" rev-parse HEAD
    git -C "$repo" status --short
    printf 'ICD=%s\n' "$VK_DRIVER_FILES"
    sha256sum "$bin/perf" "$bin/perf-optimized" "$repo/target/release/libogpu.so"
} > "$out/environment.txt"
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation VK_LAYER_VALIDATE_SYNC=1
unset VK_LOADER_LAYERS_DISABLE
OGPU_BACKEND_TESTS="$bin/backend-tests-optimized" bash "$repo/integrations/libplacebo/check-backend.sh" > "$out/backend-checks.log" 2>&1
for dims in '64 33' '960 540' '1920 1080'; do
    read -r w h <<< "$dims"
    for mode in resident transfers; do
        log="$out/verify-$w-$h-$mode.log"
        "$bin/perf-optimized" verify "$mode" "$w" "$h" 64 > "$log" 2>&1
        if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then exit 1; fi
        test "$(rg -c '^VERIFY .* PASS$' "$log")" = 3
        rg '^VERIFY' "$log"
    done
done
if test "${2:-measure}" = verify; then echo "Diagnostic verification: $out"; exit 0; fi
test "${2:-measure}" = measure
unset VK_INSTANCE_LAYERS VK_LAYER_VALIDATE_SYNC VK_LOADER_DEBUG
export VK_LOADER_LAYERS_DISABLE='*'
printf 'timing: validation disabled; variants=native,baseline,optimized; mode=resident\n' >> "$out/environment.txt"
variants=(native baseline optimized)
for dims in '960 540' '1920 1080'; do
    read -r w h <<< "$dims"
    for run in 0 1 2; do
        for offset in 0 1 2; do
            variant=${variants[$(((run+offset)%3))]}
            perf="$bin/perf"; engine=batched
            if test "$variant" = native; then engine=vulkan; fi
            if test "$variant" = optimized; then perf="$bin/perf-optimized"; fi
            log="$out/run-$w-$h-$run-$variant.log"
            "$perf" "$engine" resident "$w" "$h" 64 > "$log" 2>&1
            if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then exit 1; fi
            test "$(rg -c '^PERF .* PASS$' "$log")" = 1
            printf 'VARIANT=%s run=%s ' "$variant" "$run"
            rg '^PERF' "$log"
        done
    done
done
echo "Diagnostic results: $out"
