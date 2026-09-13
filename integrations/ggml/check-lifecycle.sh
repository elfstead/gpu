#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo"
ulimit -c 0
memory=${1:-device}
log=$(mktemp "$repo/target/ggml-integration/lifecycle.XXXXXXXX.log")
target/ggml-integration/ogpu-ggml-lifecycle integrations/ggml/shaders normal "$memory" 2>&1 | tee "$log"
if rg 'Validation Error:' "$log"; then exit 1; fi
for mode in live-backend live-buffer; do
    status=0
    target/ggml-integration/ogpu-ggml-lifecycle integrations/ggml/shaders "$mode" "$memory" > "$log" 2>&1 || status=$?
    test "$status" = 134
    rg -q '^OGPU session destroyed with live backends/buffers$' "$log"
done
echo 'live-child lifetime violations rejected PASS'
