#!/usr/bin/env bash
# Explicit network/fixture preparation; ordinary library builds do not do this.
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=${1:?usage: bash integrations/ggml/prepare.sh /path/to/pinned/ggml}
source_dir=$(cd -- "$source_dir" && pwd)
test "$(git -C "$source_dir" rev-parse HEAD)" = 7840aaba1989c6deeefede1d77d5aaf8f52b947e
git -C "$source_dir" diff --exit-code HEAD --
data_dir="$repo/target/ggml-data"
mkdir -p "$data_dir"
while read -r digest name; do
    if [[ ! -f "$data_dir/$name" ]]; then
        # Download to an independent temporary path; never overwrite existing data.
        archive=$(mktemp "$data_dir/download.XXXXXXXX.gz")
        curl --fail --location --retry 2 "https://storage.googleapis.com/cvdf-datasets/mnist/$name.gz" -o "$archive"
        gzip -d "$archive"
        unpacked=${archive%.gz}
        printf '%s  %s\n' "$digest" "$unpacked" | sha256sum --check --status
        mv -- "$unpacked" "$data_dir/$name"
    fi
done < "$repo/integrations/ggml/dataset.sha256"
(cd "$data_dir" && sha256sum --check "$repo/integrations/ggml/dataset.sha256")
cmake -S "$source_dir" -B "$repo/target/ggml-fixture-build" -G Ninja \
    -DGGML_BUILD_TESTS=OFF -DGGML_BUILD_EXAMPLES=ON -DGGML_NATIVE=OFF \
    -DGGML_OPENMP=OFF -DGGML_VULKAN=OFF -DGGML_CUDA=OFF -DGGML_BLAS=OFF
cmake --build "$repo/target/ggml-fixture-build" --target mnist-train -j 8
if [[ ! -f "$data_dir/mnist-fc-f32.gguf" ]]; then
    model_tmp=$(mktemp "$data_dir/model.XXXXXXXX.gguf")
    "$repo/target/ggml-fixture-build/bin/mnist-train" mnist-fc \
        "$model_tmp" "$data_dir/train-images-idx3-ubyte" \
        "$data_dir/train-labels-idx1-ubyte" CPU > "$data_dir/training.log" 2>&1
    mv -- "$model_tmp" "$data_dir/mnist-fc-f32.gguf"
fi
sha256sum "$data_dir/mnist-fc-f32.gguf"
