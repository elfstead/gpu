#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
bash "$repo/integrations/libplacebo/build.sh" "${1:?path to pinned libplacebo required}"
run=$(mktemp -d "$repo/target/libplacebo-integration/reference.XXXXXXXX")
"$repo/target/libplacebo-integration/reference" "$run" 2>&1 | tee "$run/run.log"
if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$run/run.log"; then
    exit 1
fi
test "$(rg -c '^reference=.* PASS$' "$run/run.log")" = 9
rg -q '^upstream-reference .*compute=9 raster=9 PASS' "$run/run.log"
echo "Reference artifacts: $run"
