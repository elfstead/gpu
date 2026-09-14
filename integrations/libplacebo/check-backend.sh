#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
out="$repo/target/libplacebo-integration"
log=$(mktemp "$out/backend-checks.XXXXXXXX.log")
ulimit -c 0
for mode in specialization-update partial-upload unsupported-image unsupported-shader \
            frame-reuse frame-capacity frame-partial frame-staging frame-destroy \
            frame-pending frame-poll-error frame-submit-error queued-specialization queued-bank-reuse; do
    "$out/backend-tests" "$mode" 2>&1 | tee -a "$log"
done
for mode in frame-reuse frame-capacity frame-partial frame-staging frame-destroy \
            frame-pending frame-poll-error frame-submit-error queued-specialization queued-bank-reuse; do
    "$out/backend-tests" "$mode" batched 2>&1 | tee -a "$log"
done
status=0
"$out/backend-tests" live-child >> "$log" 2>&1 || status=$?
test "$status" = 134
rg -q '^OGPU libplacebo: live children at device destruction' "$log"
if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then exit 1; fi
test "$(rg -c '^adapter rejection=.* live=0 PASS$' "$log")" = 3
test "$(rg -c '^adapter specialization mode=.* live=0 PASS$' "$log")" = 5
test "$(rg -c '^adapter frame=.* live=0 PASS$' "$log")" = 16
echo "Adapter rejection/cleanup and live-child checks PASS: $log"
