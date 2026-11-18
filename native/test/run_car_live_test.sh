#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
OUT=${1:-$PWD/../build/car-live-test}
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
mkdir -p "$OUT"
"${PIN[@]}" zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Werror -Wno-nullability-completeness \
    -I../../packages/sdk/include -o "$OUT/car_live_test" hooks/car_live_test.cpp
"$OUT/car_live_test"
"${PIN[@]}" zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Werror -Wno-nullability-completeness \
    -I../../packages/sdk/include -o "$OUT/car_profile_test" hooks/car_profile_test.cpp
"$OUT/car_profile_test"
"${PIN[@]}" zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Werror -Wno-nullability-completeness \
    -I../../packages/sdk/include -o "$OUT/ai_racecraft_test" hooks/ai_racecraft_test.cpp
"$OUT/ai_racecraft_test"
