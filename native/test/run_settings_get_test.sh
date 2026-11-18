#!/usr/bin/env bash
# Builds src/core/settings.cpp with the registry store as native/build.sh builds the core and runs settings_get_test.cpp:
# under Wine on Linux, natively on Windows (Git Bash). No game process starts, so it needs no game lock.
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
OUT=${1:-$PWD/../build/settings-get-test}
SDK=../../packages/sdk/include
mkdir -p "$OUT"
if [[ $NATIVE == 1 ]]; then
    PIN=()
    RUN=()
else
    PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
    RUN=(wine)
    export WINEPREFIX=${WINEPREFIX:-$OUT/prefix} WINEDEBUG=-all
fi
"${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -Wno-nullability-completeness -I$SDK -I../include \
    -o "$OUT/settings_get_test.exe" settings_get_test.cpp ../src/core/settings.cpp ../src/core/registry/store.cpp \
    ../src/core/registry/table.cpp ../src/core/registry/validate.cpp ../src/core/registry/schema.cpp \
    ../src/core/registry/older.cpp || exit 1
[[ $NATIVE == 1 ]] || timeout 300 wineboot -i >/dev/null 2>&1
timeout 300 "${PIN[@]}" "${RUN[@]}" "$OUT/settings_get_test.exe" 2>/dev/null
r=$?
[[ $r -eq 0 ]] || printf 'settings_get test: exit %d (0x%08x)\n' "$r" "$((r & 0xffffffff))"
exit $r
