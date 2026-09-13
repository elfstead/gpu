#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
test -n "${VK_DRIVER_FILES:-}" || { echo 'Select exactly one ICD with VK_DRIVER_FILES' >&2; exit 2; }
test -f "$VK_DRIVER_FILES" # rejects a driver list as well as a missing ICD
bash "$repo/integrations/libplacebo/build-consumer.sh" "${1:?path to pinned libplacebo required}"
out="$repo/target/libplacebo-integration"
bash "$repo/integrations/libplacebo/check-backend.sh"
reference=$(mktemp -d "$out/reference.XXXXXXXX")
consumer=$(mktemp -d "$out/ogpu.XXXXXXXX")
"$out/reference" "$reference" 2>&1 | tee "$reference/run.log"
"$out/consumer" "$consumer" 2>&1 | tee "$consumer/run.log"
if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$reference/run.log" "$consumer/run.log"; then exit 1; fi
test "$(rg -c '^reference=.* PASS$' "$reference/run.log")" = 9
test "$(rg -c '^ogpu=.* PASS$' "$consumer/run.log")" = 9
rg -q '^upstream-reference creates=6 compute=9 raster=9 PASS' "$reference/run.log"
rg -q '^ogpu-consumer creates=6 compute=9 raster=9 uploads=12 downloads=18 live=0 PASS' "$consumer/run.log"
"$out/compare-reference" "$reference" "$consumer" --consumer | tee "$consumer/comparison.log"
echo "Reference: $reference"
echo "OGPU: $consumer"
