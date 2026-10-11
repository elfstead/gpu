#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=${1:?usage: bash integrations/ggml/run.sh /path/to/pinned/ggml [device-index] [host|device] [f32|f16] [foundation|installed]}
source_dir=$(cd -- "$source_dir" && pwd)
device_index=${2:-0}
memory=${3:-device}
precision=${4:-f32}
default_api=foundation
library_suffix=so
if [[ $(uname -s) == Darwin ]]; then default_api=installed; library_suffix=dylib; fi
api=${5:-$default_api}
case "$api" in foundation) foundation=ON;; installed) foundation=OFF;; *) echo 'API must be foundation or installed' >&2; exit 2;; esac
build_dir="$repo/target/ggml-integration-$api"
case "$memory" in host|device) ;; *) echo 'memory must be host or device' >&2; exit 2;; esac
case "$precision" in f32|f16) ;; *) echo 'precision must be f32 or f16' >&2; exit 2;; esac
cd "$repo"
if [[ -n ${CARGO_TARGET_DIR:-} ]]; then
    echo 'leave CARGO_TARGET_DIR unset so the consumer links the library built by this run' >&2
    exit 2
fi
if [[ $(uname -s) != Darwin ]] && command -v sha256sum >/dev/null; then
    sha256=(sha256sum)
else
    sha256=(shasum -a 256)
fi
(cd target/ggml-data && rg '  t10k-' "$repo/integrations/ggml/dataset.sha256" | "${sha256[@]}" -c)
(cd integrations/ggml/fixtures && "${sha256[@]}" -c SHA256SUMS)
cargo build --locked --release -p ogpu
bash integrations/ggml/check-shaders.sh
cmake -S integrations/ggml -B "$build_dir" -G Ninja \
    -DGGML_SOURCE="$source_dir" -DCMAKE_BUILD_TYPE=Release -DOGPU_GGML_FOUNDATION="$foundation" \
    -DOGPU_LIBRARY="$repo/target/release/libogpu.$library_suffix" -DOGPU_INCLUDE_DIR="$repo/include"
cmake --build "$build_dir" -j 8
bash integrations/ggml/check-lifecycle.sh "$memory" "$build_dir"
log=$(mktemp "$build_dir/acceptance.XXXXXXXX")
echo "GGML API=$api memory=$memory precision=$precision" | tee "$log"
if test "$api" = foundation; then
    "$build_dir/ogpu-ggml-foundation-check" 2>&1 | tee -a "$log"
fi
"$build_dir/ogpu-ggml-matrix-check" integrations/ggml/shaders "$device_index" "$memory" 2>&1 | tee -a "$log"
extra=()
if test "$precision" = f16; then
    derivatives=$(mktemp -d "$build_dir/weights.XXXXXXXX")
    extra=("$derivatives/half.gguf" "$derivatives/widened.gguf")
    "$build_dir/ogpu-ggml-convert" integrations/ggml/fixtures/mnist-fc-f32.gguf "${extra[@]}" 2>&1 | tee -a "$log"
    "${sha256[@]}" "${extra[@]}" | tee -a "$log"
fi
"$build_dir/ogpu-mnist" integrations/ggml/fixtures/mnist-fc-f32.gguf \
    target/ggml-data/t10k-images-idx3-ubyte target/ggml-data/t10k-labels-idx1-ubyte \
    integrations/ggml/shaders "$device_index" "$memory" "${extra[@]}" 2>&1 | tee -a "$log"
if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then
    echo "Validation/sanitizer diagnostic: $log" >&2
    exit 1
fi
test "$(rg -c '^batch=.* PASS$' "$log")" = 6
test "$(rg -c '^mixed_matrix.* PASS$' "$log")" = 2
echo "Acceptance log: $log"
