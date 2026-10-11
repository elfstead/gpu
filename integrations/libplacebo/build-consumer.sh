#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
if test "${OGPU_LIBPLACEBO_API:-foundation}" != foundation; then
    echo 'Only foundation is supported; see docs/libplacebo-diagnostic-retirement.md for historical controls' >&2
    exit 2
fi
source_dir=$(cd -- "${1:?path to pinned libplacebo required}" && pwd)
bash "$repo/integrations/libplacebo/build.sh" "$source_dir"
cd "$repo"
cargo build --locked --release -p ogpu
build="$repo/target/libplacebo-upstream"
out="$repo/target/libplacebo-integration"
wrappers=(-Wl,--wrap=ogpu_next_timeline_poll -Wl,--wrap=ogpu_next_queue_submit
    -Wl,--wrap=ogpu_next_commands_end -Wl,--wrap=ogpu_next_timeline_wait)
echo 'libplacebo execution API: foundation'
for unit in backend consumer backend-tests stream perf; do
    "${CC:-cc}" -std=gnu11 -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
        -I"$source_dir/src" -I"$build/src" -I"$source_dir/src/include" \
        -I"$build/src/include" -I"$repo/include" $(pkg-config --cflags vulkan) \
        -c "$repo/integrations/libplacebo/$unit.c" -o "$out/$unit.o"
done
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror $(pkg-config --cflags shaderc) \
    -c "$repo/integrations/libplacebo/compiler.cpp" -o "$out/compiler.o"
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror $(pkg-config --cflags SPIRV-Tools) \
    -c "$repo/integrations/libplacebo/metadata.cpp" -o "$out/metadata.o"
read -ra metadata_libs <<< "$(pkg-config --libs SPIRV-Tools)"
for unit in consumer backend-tests stream perf; do
    extra=()
    if test "$unit" = backend-tests; then
        extra=("${wrappers[@]}")
    fi
    "${CXX:-c++}" "$out/backend.o" "$out/$unit.o" "$out/compiler.o" \
        "${extra[@]}" "$out/metadata.o" "${metadata_libs[@]}" \
        -L"$repo/target/release" -Wl,-rpath,"$repo/target/release" -logpu \
        -L"$build/src" -Wl,-rpath,"$build/src" -lplacebo $(pkg-config --libs shaderc) \
        -o "$out/$unit"
done
