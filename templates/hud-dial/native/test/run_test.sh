#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
CXX=${CXX:-c++}
if ! command -v "$CXX" >/dev/null; then
    exec nix shell nixpkgs#gcc -c "$SELF" "$@"
fi
if [ -d ../include ]; then INC=../include; elif [ -d ../../../addon/native/include ]; then INC=../../../addon/native/include; else INC=../../../../packages/sdk/include; fi
OUT=${1:-$PWD/../build/test}
mkdir -p "$OUT"
"$CXX" -std=c++23 -O1 -Wall -Wextra -I"$INC" -o "$OUT/dial_test" dial_test.cpp
"$OUT/dial_test"
