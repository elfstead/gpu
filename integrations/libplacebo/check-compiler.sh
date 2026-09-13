#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
capture=${1:?usage: bash integrations/libplacebo/check-compiler.sh reference-artifact-directory}
test -f "$capture/manifest.txt"
out=$(mktemp -d "$repo/target/libplacebo-integration/compiler.XXXXXXXX")
glslc --version > "$out/compiler-version.txt"
"$repo/target/libplacebo-integration/heap-lower" --test
count=0
for source in "$capture"/*.comp "$capture"/*.frag "$capture"/*.vert; do
    test -f "$source"
    name=$(basename -- "$source")
    "$repo/target/libplacebo-integration/heap-lower" "$source" "$out/$name"
    glslc --target-env=vulkan1.4 "$out/$name" -o "$out/$name.spv"
    spirv-val --target-env vulkan1.4 "$out/$name.spv"
    spirv-dis "$out/$name.spv" -o "$out/$name.spvasm"
    if rg 'OpDecorate .* (DescriptorSet|Binding) ' "$out/$name.spvasm"; then
        echo 'Legacy descriptor decoration survived lowering' >&2
        exit 1
    fi
    if [[ "$name" != *.vert ]]; then
        rg -q 'OpCapability DescriptorHeapEXT' "$out/$name.spvasm"
        rg -q 'BuiltIn ResourceHeapEXT' "$out/$name.spvasm"
        rg -q 'BuiltIn SamplerHeapEXT' "$out/$name.spvasm"
    fi
    count=$((count + 1))
done
test "$count" = 9
echo "Compiler gate: $count upstream shaders lowered and validated; NOT an execution result. Artifacts: $out"
