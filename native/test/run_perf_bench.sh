#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
OUT=${PERF_OUT:-$PWD/../build/perf-bench}
WINE=${WINE:-wine}
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
SH=../third_party/safetyhook
mkdir -p "$OUT"
[[ $OUT/Zydis.o -nt $SH/Zydis.c ]] || "${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -I$SH -c $SH/Zydis.c -o "$OUT/Zydis.o" &
[[ $OUT/safetyhook.o -nt $SH/safetyhook.cpp ]] ||
    "${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -I$SH -c $SH/safetyhook.cpp -o "$OUT/safetyhook.o" &
wait
"${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wno-nullability-completeness -I../../packages/sdk/include \
    -DSWOFTY_MEM_SHARED -I../include -I$SH -o "$OUT/perf_bench.exe" perf_bench.cpp ../src/core/perf.cpp ../src/core/perf_system.cpp ../src/core/mem_pages.cpp "$OUT/safetyhook.o" "$OUT/Zydis.o" -s
cd "$OUT"
export WINEPREFIX=${WINEPREFIX:-$OUT/prefix} WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml=;winemenubuilder.exe=d"
"${PIN[@]}" $WINE ./perf_bench.exe "$@"
