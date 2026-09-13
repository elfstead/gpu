#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=${1:?usage: bash integrations/libplacebo/build.sh /path/to/pinned/libplacebo}
source_dir=$(cd -- "$source_dir" && pwd)
test "$(git -C "$source_dir" rev-parse HEAD)" = 3330a515d62139259c26239014f286e233bd3a5c
git -C "$source_dir" diff --exit-code HEAD --
build="$repo/target/libplacebo-upstream"
options=(--buildtype=release --default-library=shared --wrap-mode=nodownload
    -Dvulkan=enabled -Dvk-proc-addr=enabled -Dshaderc=enabled -Dglslang=disabled
    -Dvulkan-registry="$repo/vendor/Vulkan-Headers/registry/vk.xml"
    -Dopengl=disabled -Dd3d11=disabled -Dlcms=disabled -Ddovi=disabled
    -Dlibdovi=disabled -Dunwind=disabled -Dxxhash=disabled
    -Ddemos=false -Dtests=false -Dbench=false)
if test -f "$build/build.ninja"; then
    meson setup --reconfigure "$build" "$source_dir" "${options[@]}"
else
    meson setup "$build" "$source_dir" "${options[@]}"
fi
meson compile -C "$build" -j 8
mkdir -p "$repo/target/libplacebo-integration"
"${CC:-cc}" -std=gnu11 -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
    -I"$source_dir/src" -I"$build/src" -I"$source_dir/src/include" \
    -I"$build/src/include" $(pkg-config --cflags vulkan) \
    "$repo/integrations/libplacebo/reference.c" \
    -L"$build/src" -Wl,-rpath,"$build/src" -lplacebo -lm \
    -o "$repo/target/libplacebo-integration/reference"
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror \
    "$repo/integrations/libplacebo/heap-lower.cpp" \
    -o "$repo/target/libplacebo-integration/heap-lower"
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror \
    "$repo/integrations/libplacebo/compare-reference.cpp" \
    -o "$repo/target/libplacebo-integration/compare-reference"
