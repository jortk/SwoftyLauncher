#!/usr/bin/env bash
# Builds src/core/mem_pages.cpp with SWOFTY_MEM_SHARED as native/build.sh builds the core and runs mem_pages_test.cpp,
# then builds mem_addon_test.cpp as an addon builds swofty/mem.hpp (no SWOFTY_MEM_SHARED) with two swofty_core.dll
# stand-ins from mem_addon_core.cpp and runs it once per mode: under Wine on Linux, natively on Windows (Git Bash).
set -uo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
case "$(uname -s)" in
MINGW* | MSYS* | CYGWIN*) NATIVE=1 ;;
*) NATIVE=0 ;;
esac
if [[ $NATIVE == 0 ]] && { ! command -v wine >/dev/null || ! command -v zig >/dev/null; }; then
    exec nix shell nixpkgs#zig nixpkgs#wineWow64Packages.stable -c "$SELF" "$@"
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
OUT=${1:-$PWD/../build/mem-pages-test}
SDK=../../packages/sdk/include
mkdir -p "$OUT/core" "$OUT/fake"
if [[ $NATIVE == 1 ]]; then
    PIN=()
    RUN=()
else
    PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
    RUN=(wine)
    export WINEPREFIX=${WINEPREFIX:-$OUT/prefix} WINEDEBUG=-all
fi
CXX=(zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I$SDK)
"${PIN[@]}" "${CXX[@]}" -DSWOFTY_MEM_SHARED -o "$OUT/mem_pages_test.exe" mem_pages_test.cpp ../src/core/mem_pages.cpp || exit 1
"${PIN[@]}" "${CXX[@]}" -DSWOFTY_MEM_SHARED -shared -o "$OUT/core/swofty_core.dll" mem_addon_core.cpp ../src/core/mem_pages.cpp || exit 1
"${PIN[@]}" "${CXX[@]}" -DMEM_ADDON_FAKE_CORE -shared -o "$OUT/fake/swofty_core.dll" mem_addon_core.cpp || exit 1
"${PIN[@]}" "${CXX[@]}" -Werror -o "$OUT/mem_addon_test.exe" mem_addon_test.cpp || exit 1
[[ $NATIVE == 1 ]] || timeout 300 wineboot -i >/dev/null 2>&1
timeout 300 "${PIN[@]}" "${RUN[@]}" "$OUT/mem_pages_test.exe" 2>/dev/null
r=$?
[[ $r -eq 0 ]] || printf 'mem pages test: exit %d (0x%08x)\n' "$r" "$((r & 0xffffffff))"
for mode in core absent v0 nullfn; do
    timeout 120 "${PIN[@]}" "${RUN[@]}" "$OUT/mem_addon_test.exe" "$mode" 2>/dev/null
    a=$?
    [[ $a -eq 0 ]] || printf 'mem addon test (%s): exit %d (0x%08x)\n' "$mode" "$a" "$((a & 0xffffffff))"
    [[ $r -ne 0 ]] || r=$a
done
exit $r
