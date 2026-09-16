#!/usr/bin/env bash
set -euo pipefail
shaders=$(cd -- "$(dirname -- "$0")/shaders" && pwd)
for name in matmul-paired matmul-half; do
    spirv-val --target-env vulkan1.4 "$shaders/$name.comp.spv"
done
control=$(spirv-dis "$shaders/matmul-paired.comp.spv")
half=$(spirv-dis "$shaders/matmul-half.comp.spv")
test "$(rg 'OpCapability' <<< "$control" | awk '{print $2}' | sort)" = \
    "$(printf '%s\n' PhysicalStorageBufferAddresses Shader)"
test "$(rg 'OpCapability' <<< "$half" | awk '{print $2}' | sort)" = \
    "$(printf '%s\n' Float16 PhysicalStorageBufferAddresses Shader)"
rg -q 'OpFMul %v2half ' <<< "$half"
rg -q 'OpFConvert %v2float ' <<< "$half"
rg -q 'OpFAdd %v2float ' <<< "$half"
rg -q 'OpFMul %v2float ' <<< "$control"
# Check the actual arithmetic results, not merely some unrelated decoration.
for assembly in "$control" "$half"; do
    if rg 'RelaxedPrecision|OpExtInst .* Fma ' <<< "$assembly"; then exit 1; fi
    while read -r result; do
        rg -q "OpDecorate $result NoContraction" <<< "$assembly"
    done < <(awk '/OpFMul|OpFAdd/ {print $1}' <<< "$assembly")
done
echo 'Paired matrix executable capability/arithmetic contract PASS'
