#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=${1:?usage: bash integrations/ggml/run-transformer.sh /path/to/pinned/ggml [device-index]}
source_dir=$(cd -- "$source_dir" && pwd)
device_index=${2:-0}
test "$(uname -s)" = Linux || { echo 'Transformer profile currently requires Linux foundation' >&2; exit 1; }
test -z "${CARGO_TARGET_DIR:-}" || { echo 'Leave CARGO_TARGET_DIR unset' >&2; exit 1; }
cd "$repo"
cargo build --locked --release -p ogpu --no-default-features
bash integrations/ggml/check-shaders.sh
build_dir="$repo/target/ggml-integration-foundation"
cmake -S integrations/ggml -B "$build_dir" -G Ninja -DGGML_SOURCE="$source_dir" \
    -DCMAKE_BUILD_TYPE=Release -DOGPU_GGML_FOUNDATION=ON \
    -DOGPU_LIBRARY="$repo/target/release/libogpu.so" -DOGPU_INCLUDE_DIR="$repo/include"
cmake --build "$build_dir" -j 8
log=$(mktemp "$build_dir/transformer.XXXXXXXX")
echo "Transformer evidence: $log"
git rev-parse HEAD | tee "$log"
git status --porcelain | tee -a "$log"
sha256sum "$build_dir/ogpu-ggml-transformer-check" target/release/libogpu.so \
    integrations/ggml/transformer-check.cpp integrations/ggml/backend.cpp integrations/ggml/backend.h \
    integrations/ggml/runtime-foundation.h include/ogpu_next.h integrations/ggml/shaders/*.spv | tee -a "$log"
for placement in host device; do
    "$build_dir/ogpu-ggml-transformer-check" integrations/ggml/shaders "$device_index" "$placement" 2>&1 | tee -a "$log"
done
if rg 'Validation Error:|SYNC-HAZARD|runtime error:|ERROR: AddressSanitizer' "$log"; then
    echo "Validation/sanitizer diagnostic: $log" >&2; exit 1
fi
test "$(rg -c '^transformer .* PASS$' "$log")" = 20
test "$(rg -c '^Transformer profile placement=.* cases=10 PASS;' "$log")" = 2
echo 'Foundation transformer HOST/DEVICE acceptance PASS; no timing claim' | tee -a "$log"
