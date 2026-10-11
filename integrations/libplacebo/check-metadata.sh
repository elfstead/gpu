#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
out="$repo/target/libplacebo-integration"
mkdir -p "$out"
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror \
    $(pkg-config --cflags shaderc SPIRV-Tools) \
    "$repo/integrations/libplacebo/check-metadata.cpp" \
    "$repo/integrations/libplacebo/compiler.cpp" \
    $(pkg-config --libs shaderc SPIRV-Tools) -o "$out/check-metadata"
"$out/check-metadata"
