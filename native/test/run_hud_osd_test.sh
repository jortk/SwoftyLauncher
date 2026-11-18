#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
CXX=${CXX:-c++}
if ! command -v "$CXX" >/dev/null; then
    exec nix shell nixpkgs#gcc -c "$SELF" "$@"
fi
OUT=${1:-$PWD/../build/hud-osd-test}
mkdir -p "$OUT"
"$CXX" -std=c++23 -O2 -Wall -Wextra -I../../packages/sdk/include -o "$OUT/hud_osd_test" hud_osd_test.cpp
"$OUT/hud_osd_test"
