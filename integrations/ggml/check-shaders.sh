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
