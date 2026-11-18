#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
CXX=${CXX:-c++}
if ! command -v "$CXX" >/dev/null; then
    exec nix shell nixpkgs#gcc -c "$SELF" "$@"
fi
OUT=${1:-$PWD/../build/garage-placement-test}
mkdir -p "$OUT"
"$CXX" -std=c++23 -O2 -Wall -Wextra -I../src/core/garage_placement -I../../packages/sdk/include \
    -o "$OUT/garage_placement_test" garage_placement_test.cpp
"$OUT/garage_placement_test"
"$CXX" -std=c++23 -O2 -Wall -Wextra -I../src/core/garage_placement -o "$OUT/garage_crew_pose_test" garage_crew_pose_test.cpp
"$OUT/garage_crew_pose_test"
