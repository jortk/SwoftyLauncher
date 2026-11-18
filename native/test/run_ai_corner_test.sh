#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
CXX=${CXX:-c++}
if ! command -v "$CXX" >/dev/null; then
    exec nix shell nixpkgs#gcc -c "$SELF" "$@"
fi
OUT=${1:-$PWD/../build/ai-corner-test}
mkdir -p "$OUT"
"$CXX" -std=c++23 -O2 -Wall -Wextra -I../src/core -pthread -o "$OUT/ai_corner_test" ai_corner_test.cpp
"$OUT/ai_corner_test"
