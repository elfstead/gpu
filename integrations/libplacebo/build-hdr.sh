#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=$(cd -- "${1:?pinned libplacebo checkout required}" && pwd)
bash "$repo/integrations/libplacebo/build-consumer.sh" "$source_dir"
build="$repo/target/libplacebo-upstream"
out="$repo/target/libplacebo-integration"
"${CC:-cc}" -std=gnu11 -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
    -I"$source_dir/src" -I"$build/src" -I"$source_dir/src/include" -I"$build/src/include" \
    $(pkg-config --cflags vulkan) -c "$repo/integrations/libplacebo/hdr.c" -o "$out/hdr.o"
"${CXX:-c++}" "$out/hdr.o" "$out/backend.o" "$out/compiler.o" \
    -L"$repo/target/release" -Wl,-rpath,"$repo/target/release" -logpu \
    -L"$build/src" -Wl,-rpath,"$build/src" -lplacebo $(pkg-config --libs shaderc) -lm -o "$out/hdr-reference"
"${CC:-cc}" -std=gnu11 -O2 -Wall -Wextra -Werror \
    "$repo/integrations/libplacebo/compare-hdr.c" -lm -o "$out/compare-hdr"
