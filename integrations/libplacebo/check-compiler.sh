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
    args=()
    if [[ "$name" == *.vert ]]; then
        pass=${name#pass-}; pass=${pass%.vert}
        # Confirm offsets/types from the pinned pass metadata, not attribute names alone.
        header=$(awk -v id="$pass" '$1 == "create=" id { print }' "$capture/manifest.txt")
        [[ "$header" =~ ^create=$pass\ type=raster\ descriptors=[0-9]+\ constants=[0-9]+\ push_bytes=0\ vertex_stride=16\ vertex_attributes=2\ topology=1\ target=rgba8\ blend=0$ ]]
        mapfile -t attrs < <(awk -v id="$pass" '
            /^create=/ { active=($1 == "create=" id) }
            active && /^  vertex / { print }
        ' "$capture/manifest.txt")
        test "${#attrs[@]}" = 2
        [[ "${attrs[0]}" =~ ^[[:space:]]*vertex\ location=0\ offset=0\ format=rg32f\ name=([A-Za-z_][A-Za-z_0-9]*)$ ]]
        name0=${BASH_REMATCH[1]}
        [[ "${attrs[1]}" =~ ^[[:space:]]*vertex\ location=1\ offset=8\ format=rg32f\ name=([A-Za-z_][A-Za-z_0-9]*)$ ]]
        args=(--vertex "$name0" "${BASH_REMATCH[1]}")
    fi
    "$repo/target/libplacebo-integration/heap-lower" "$source" "$out/$name" "${args[@]}"
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
    else
        rg -q 'OpCapability PhysicalStorageBufferAddresses' "$out/$name.spvasm"
        rg -q 'BuiltIn VertexIndex' "$out/$name.spvasm"
        if rg 'OpVariable .* Input' "$out/$name.spvasm" | rg -v 'gl_VertexIndex|gl_InstanceIndex'; then
            echo 'Unexpected vertex input survived lowering' >&2
            exit 1
        fi
    fi
    count=$((count + 1))
done
test "$count" = 9
echo "Compiler gate: $count upstream shaders lowered and validated; NOT an execution result. Artifacts: $out"
