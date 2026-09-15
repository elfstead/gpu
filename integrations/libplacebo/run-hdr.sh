#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
test -n "${VK_DRIVER_FILES:-}" || { echo 'Select exactly one ICD with VK_DRIVER_FILES' >&2; exit 1; }
test -f "$VK_DRIVER_FILES"
bash "$repo/integrations/libplacebo/build-hdr.sh" "${1:?path to pinned libplacebo required}"
out="$repo/target/libplacebo-integration"
"$out/heap-lower" --test
reference=$(mktemp -d "$out/hdr-native.XXXXXXXX")
consumer=$(mktemp -d "$out/hdr-ogpu.XXXXXXXX")
"$out/hdr-reference" "$reference" 2>&1 | tee "$reference/run.log"
"$out/hdr-reference" "$consumer" ogpu 2>&1 | tee "$consumer/run.log"
if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$reference/run.log" "$consumer/run.log"; then exit 1; fi
rg -q '^HDR-reference creates=6 compute=9 raster=9 alpha_failures=0 PASS \(native\)$' "$reference/run.log"
rg -q '^HDR-reference creates=6 compute=9 raster=9 alpha_failures=0 PASS \(ogpu\)$' "$consumer/run.log"
rg -q '^HDR-ogpu live=0 receipt_reuses=12 PASS$' "$consumer/run.log"
# Protect the selected color policy: image capability fallback must not silently
# change upstream matrices/scalars. This profile has 56/208-byte parameter blocks.
for run in {0..17}; do
    cmp "$reference/run-$run.push" "$consumer/run-$run.push"
done
"$out/compare-hdr" "$reference" "$consumer" | tee "$consumer/comparison.log"
echo "HDR native: $reference"
echo "HDR OGPU: $consumer"
