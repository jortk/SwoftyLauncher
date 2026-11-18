#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
if [ -d include ]; then INC=include; elif [ -d ../../addon/native/include ]; then INC=../../addon/native/include; else INC=../../../packages/sdk/include; fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/build/zig-cache
mkdir -p build
zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -Wno-nullability-completeness -I"$INC" -shared -s \
    -o hud_dial.dll src/addon.cpp
rm -f hud_dial.lib
echo "built $(pwd)/hud_dial.dll"
