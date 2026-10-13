#!/usr/bin/env bash
set -euo pipefail
shaders=$(cd -- "$(dirname -- "$0")/shaders" && pwd)
for shader in "$shaders"/*.spv; do
    spirv-val --target-env vulkan1.2 "$shader"
done
assembly=$(spirv-dis "$shaders/matrix-f16.comp.spv")
capabilities=$(rg 'OpCapability' <<< "$assembly" | awk '{print $2}' | sort)
expected=$(printf '%s\n' PhysicalStorageBufferAddresses Shader StorageBuffer16BitAccess)
test "$capabilities" = "$expected" || {
    echo "Unexpected mixed shader capabilities: $capabilities" >&2; exit 1;
}
rg -q 'OpFConvert %float ' <<< "$assembly"
rg -q 'OpLoad %half .* Aligned 2' <<< "$assembly"
# Capability checks exclude half arithmetic; also exclude reduced-precision FP32.
if rg 'RelaxedPrecision' <<< "$assembly"; then
    echo 'Mixed shader permits reduced-precision arithmetic' >&2; exit 1
fi
echo 'GGML SPIR-V storage-only half contract PASS'
assembly=$(spirv-dis "$shaders/block.comp.spv")
capabilities=$(rg 'OpCapability' <<< "$assembly" | awk '{print $2}' | sort)
expected=$(printf '%s\n' PhysicalStorageBufferAddresses Shader)
test "$capabilities" = "$expected" || {
    echo "Unexpected transformer capabilities: $capabilities" >&2; exit 1;
}
rg -q 'OpExecutionMode %main LocalSize 64 1 1' <<< "$assembly"
offsets=$(rg 'OpMemberDecorate %Root [0-6] Offset' <<< "$assembly" | awk '{print $3 ":" $5}')
expected=$(printf '%s\n' 0:0 1:8 2:16 3:24 4:28 5:32 6:36)
test "$offsets" = "$expected" || { echo 'Transformer root layout mismatch' >&2; exit 1; }
if rg 'RelaxedPrecision|OpGroupNonUniform|OpAtomic' <<< "$assembly"; then
    echo 'Unexpected reduced precision/subgroup/atomic transformer requirement' >&2; exit 1
fi
echo 'GGML transformer FP32 workgroup/root contract PASS'
