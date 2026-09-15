#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
source_dir=$(cd -- "${1:?pinned libplacebo checkout required}" && pwd)
bash "$repo/integrations/libplacebo/build.sh" "$source_dir"
build="$repo/target/libplacebo-upstream"
out="$repo/target/libplacebo-integration"
"${CC:-cc}" -std=gnu11 -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
    -I"$source_dir/src" -I"$build/src" -I"$source_dir/src/include" -I"$build/src/include" \
    $(pkg-config --cflags vulkan) "$repo/integrations/libplacebo/hdr.c" \
    -L"$build/src" -Wl,-rpath,"$build/src" -lplacebo -lm -o "$out/hdr-reference"
