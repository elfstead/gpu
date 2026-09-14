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
