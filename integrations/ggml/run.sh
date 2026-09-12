#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=${1:?usage: bash integrations/ggml/run.sh /path/to/pinned/ggml [device-index]}
source_dir=$(cd -- "$source_dir" && pwd)
device_index=${2:-0}
cd "$repo"
(cd target/ggml-data && rg '  t10k-' "$repo/integrations/ggml/dataset.sha256" | sha256sum --check)
(cd integrations/ggml/fixtures && sha256sum --check SHA256SUMS)
cargo build --locked --release -p ogpu
for shader in integrations/ggml/shaders/*.spv; do
    spirv-val --target-env vulkan1.2 "$shader"
done
cmake -S integrations/ggml -B target/ggml-integration -G Ninja \
    -DGGML_SOURCE="$source_dir" -DCMAKE_BUILD_TYPE=Release
cmake --build target/ggml-integration -j 8
bash integrations/ggml/check-lifecycle.sh
log=$(mktemp "$repo/target/ggml-integration/acceptance.XXXXXXXX.log")
target/ggml-integration/ogpu-mnist integrations/ggml/fixtures/mnist-fc-f32.gguf \
    target/ggml-data/t10k-images-idx3-ubyte target/ggml-data/t10k-labels-idx1-ubyte \
    integrations/ggml/shaders "$device_index" 2>&1 | tee "$log"
if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then
    echo "Validation/sanitizer diagnostic: $log" >&2
    exit 1
fi
test "$(rg -c '^batch=.* PASS$' "$log")" = 6
echo "Acceptance log: $log"
