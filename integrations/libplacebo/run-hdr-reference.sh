#!/usr/bin/env bash
# Native-only gate using the accepted binary16 alpha tolerance.
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
test -n "${VK_DRIVER_FILES:-}" || { echo 'Select exactly one ICD with VK_DRIVER_FILES' >&2; exit 1; }
test -f "$VK_DRIVER_FILES"
bash "$repo/integrations/libplacebo/build-hdr.sh" "${1:?path to pinned libplacebo required}"
out="$repo/target/libplacebo-integration"
capture=$(mktemp -d "$out/hdr-native.XXXXXXXX")
status=0
"$out/hdr-reference" "$capture" 2>&1 | tee "$capture/run.log" || status=$?
echo "Native HDR capture: $capture"
if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$capture/run.log"; then exit 1; fi
# Exit 2 denotes the alpha-tolerance gate, not an execution pass.
if test "$status" = 2; then
    rg -q '^HDR-reference creates=6 compute=9 raster=9 .* GATE FAILED \(native\)$' "$capture/run.log" || exit 1
    echo 'Native capture complete; acceptance gate failed. See docs/libplacebo-hdr.md.' >&2
elif test "$status" = 0; then
    rg -q '^HDR-reference creates=6 compute=9 raster=9 .* PASS \(native\)$' "$capture/run.log" || exit 1
fi
exit "$status"
