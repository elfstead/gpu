#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=$(cd -- "${1:?path to pinned libplacebo required}" && pwd)
bash "$repo/integrations/libplacebo/build.sh" "$source_dir"
cd "$repo"
cargo build --locked --release -p ogpu
build="$repo/target/libplacebo-upstream"
out="$repo/target/libplacebo-integration"
api=${OGPU_LIBPLACEBO_API:-foundation}
defines=()
case "$api" in
    foundation) backend=backend; wrappers=(-Wl,--wrap=ogpu_next_timeline_poll -Wl,--wrap=ogpu_next_queue_submit
        -Wl,--wrap=ogpu_next_commands_end -Wl,--wrap=ogpu_next_timeline_wait) ;;
    installed) backend=backend-installed; defines=(-DOGPU_LIBPLACEBO_INSTALLED); wrappers=(-Wl,--wrap=ogpu_completion_poll -Wl,--wrap=ogpu_batch_submit) ;;
    *) echo 'OGPU_LIBPLACEBO_API must be foundation or installed' >&2; exit 2 ;;
esac
echo "libplacebo execution API: $api"
for unit in backend consumer backend-tests stream perf; do
    source_unit=$unit
    if test "$unit" = backend; then source_unit=$backend; fi
    "${CC:-cc}" -std=gnu11 -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
        "${defines[@]}" \
        -I"$source_dir/src" -I"$build/src" -I"$source_dir/src/include" \
        -I"$build/src/include" -I"$repo/include" $(pkg-config --cflags vulkan) \
        -c "$repo/integrations/libplacebo/$source_unit.c" -o "$out/$unit.o"
done
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror $(pkg-config --cflags shaderc) \
    -c "$repo/integrations/libplacebo/compiler.cpp" -o "$out/compiler.o"
metadata=()
if test "$api" = foundation; then
    "${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror $(pkg-config --cflags SPIRV-Tools) \
        -c "$repo/integrations/libplacebo/metadata.cpp" -o "$out/metadata.o"
    metadata=("$out/metadata.o")
    read -ra metadata_libs <<< "$(pkg-config --libs SPIRV-Tools)"
    metadata+=("${metadata_libs[@]}")
fi
for unit in consumer backend-tests stream perf; do
    extra=()
    if test "$unit" = backend-tests; then
        extra=("${wrappers[@]}")
    fi
    "${CXX:-c++}" "$out/backend.o" "$out/$unit.o" "$out/compiler.o" \
        "${extra[@]}" "${metadata[@]}" \
        -L"$repo/target/release" -Wl,-rpath,"$repo/target/release" -logpu \
        -L"$build/src" -Wl,-rpath,"$build/src" -lplacebo $(pkg-config --libs shaderc) \
        -o "$out/$unit"
done
