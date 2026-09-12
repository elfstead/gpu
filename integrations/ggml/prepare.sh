#!/usr/bin/env bash
# Fetch test data only. Routine acceptance uses the checked-in trained fixture.
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
data_dir="$repo/target/ggml-data"
mkdir -p "$data_dir"
(cd "$repo/integrations/ggml/fixtures" && sha256sum --check SHA256SUMS)
while read -r digest name; do
    [[ "$name" == t10k-* ]] || continue
    if [[ ! -f "$data_dir/$name" ]]; then
        archive=$(mktemp "$data_dir/download.XXXXXXXX.gz")
        curl --fail --location --retry 2 "https://storage.googleapis.com/cvdf-datasets/mnist/$name.gz" -o "$archive"
        gzip -d "$archive"
        unpacked=${archive%.gz}
        printf '%s  %s\n' "$digest" "$unpacked" | sha256sum --check --status
        mv -- "$unpacked" "$data_dir/$name"
    fi
    printf '%s  %s\n' "$digest" "$data_dir/$name" | sha256sum --check
done < "$repo/integrations/ggml/dataset.sha256"
