#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=$(cd -- "${1:?path to pinned libplacebo required}" && pwd)
bash "$repo/integrations/libplacebo/build.sh" "$source_dir"
cd "$repo"
cargo build --locked --release -p ogpu
build="$repo/target/libplacebo-upstream"
out="$repo/target/libplacebo-integration"
for unit in backend consumer backend-tests stream; do
    "${CC:-cc}" -std=gnu11 -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
        -I"$source_dir/src" -I"$build/src" -I"$source_dir/src/include" \
        -I"$build/src/include" -I"$repo/include" \
        -c "$repo/integrations/libplacebo/$unit.c" -o "$out/$unit.o"
done
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror $(pkg-config --cflags shaderc) \
    -c "$repo/integrations/libplacebo/compiler.cpp" -o "$out/compiler.o"
for unit in consumer backend-tests stream; do
    "${CXX:-c++}" "$out/backend.o" "$out/$unit.o" "$out/compiler.o" \
        -L"$repo/target/release" -Wl,-rpath,"$repo/target/release" -logpu \
        -L"$build/src" -Wl,-rpath,"$build/src" -lplacebo $(pkg-config --libs shaderc) \
        -o "$out/$unit"
done
