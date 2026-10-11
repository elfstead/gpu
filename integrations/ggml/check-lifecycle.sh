#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo"
ulimit -c 0
memory=${1:-device}
default_api=foundation
if [[ $(uname -s) == Darwin ]]; then default_api=installed; fi
build_dir=${2:-$repo/target/ggml-integration-$default_api}
log=$(mktemp "$build_dir/lifecycle.XXXXXXXX")
"$build_dir/ogpu-ggml-lifecycle" integrations/ggml/shaders normal "$memory" 2>&1 | tee "$log"
if rg 'Validation Error:' "$log"; then exit 1; fi
for mode in live-backend live-buffer; do
    status=0
    "$build_dir/ogpu-ggml-lifecycle" integrations/ggml/shaders "$mode" "$memory" > "$log" 2>&1 || status=$?
    test "$status" = 134
    rg -q '^OGPU session destroyed with live backends/buffers$' "$log"
done
echo 'live-child lifetime violations rejected PASS'
