#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
bash "$repo/integrations/libplacebo/build-consumer.sh" "${1:?pinned libplacebo checkout required}"
out="$repo/target/libplacebo-integration"
build="$repo/target/libplacebo-upstream"
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror $(pkg-config --cflags shaderc) \
    -DOGPU_DIAGNOSTIC_OPTIMIZE \
    -c "$repo/integrations/libplacebo/compiler.cpp" -o "$out/compiler-optimized.o"
for unit in perf backend-tests; do
    extra=()
    if test "$unit" = backend-tests; then
        extra=(-Wl,--wrap=ogpu_completion_poll -Wl,--wrap=ogpu_batch_submit)
    fi
    "${CXX:-c++}" "$out/backend.o" "$out/$unit.o" "$out/compiler-optimized.o" \
        "${extra[@]}" \
        -L"$repo/target/release" -Wl,-rpath,"$repo/target/release" -logpu \
        -L"$build/src" -Wl,-rpath,"$build/src" -lplacebo $(pkg-config --libs shaderc) \
        -o "$out/$unit-optimized"
done
source_dir=$(cd -- "$1" && pwd)
for unit in perf diagnostics; do
    "${CC:-cc}" -std=gnu11 -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
        -DOGPU_DIAGNOSTICS -I"$source_dir/src" -I"$build/src" \
        -I"$source_dir/src/include" -I"$build/src/include" -I"$repo/include" \
        $(pkg-config --cflags vulkan) \
        -c "$repo/integrations/libplacebo/$unit.c" -o "$out/$unit-diagnostic.o"
done
"${CXX:-c++}" "$out/backend.o" "$out/perf-diagnostic.o" "$out/diagnostics-diagnostic.o" "$out/compiler.o" \
    -Wl,--wrap=ogpu_batch_create -Wl,--wrap=ogpu_batch_submit \
    -Wl,--wrap=ogpu_completion_poll -Wl,--wrap=ogpu_completion_wait -Wl,--wrap=ogpu_completion_destroy \
    -Wl,--wrap=ogpu_batch_dispatch -Wl,--wrap=ogpu_batch_draw_indirect \
    -L"$repo/target/release" -Wl,-rpath,"$repo/target/release" -logpu \
    -L"$build/src" -Wl,-rpath,"$build/src" -lplacebo $(pkg-config --libs shaderc) \
    -o "$out/perf-diagnostic"
