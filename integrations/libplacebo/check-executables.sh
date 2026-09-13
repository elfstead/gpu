#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/../.." && pwd)
capture=${1:?usage: check-executables.sh capture-directory compiler-directory}
compiled=${2:?compiler-directory required}
test -f "$capture/manifest.txt"
cd "$repo"
cargo build --locked --release -p ogpu
"${CXX:-c++}" -std=c++17 -O2 -Wall -Wextra -Werror -I"$repo/include" \
    "$repo/integrations/libplacebo/check-executables.cpp" \
    -L"$repo/target/release" -Wl,-rpath,"$repo/target/release" -logpu \
    -o "$repo/target/libplacebo-integration/check-executables"
log=$(mktemp "$repo/target/libplacebo-integration/executables.XXXXXXXX.log")
"$repo/target/libplacebo-integration/check-executables" "$capture/manifest.txt" "$compiled" 2>&1 | tee "$log"
if rg 'Validation Error:|runtime error:|ERROR: AddressSanitizer' "$log"; then exit 1; fi
test "$(rg -c '^prepared pass=.* PASS$' "$log")" = 6
echo "Executable-creation log: $log"
