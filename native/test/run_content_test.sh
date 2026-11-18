#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig -c "$SELF" "$@"
fi
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
OUT=${1:-$PWD/../build/content-test}
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
mkdir -p "$OUT"
SRC=(../src/core/registry/store.cpp ../src/core/registry/table.cpp ../src/core/registry/validate.cpp ../src/core/registry/schema.cpp ../src/core/registry/engine.cpp)
CXX=(zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -I../../packages/sdk/include)
pids=()
"${PIN[@]}" "${CXX[@]}" -o "$OUT/lower_tool" content/lower_tool.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" "${CXX[@]}" -o "$OUT/rows_tool" content/rows_tool.cpp ../src/core/apply/ids.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" "${CXX[@]}" -o "$OUT/rows_test" content/rows_test.cpp ../src/core/apply/ids.cpp ../src/core/apply/scheduler.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" "${CXX[@]}" -o "$OUT/vehicle_tower_test" content/vehicle_tower_test.cpp ../src/core/apply/ids.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" "${CXX[@]}" -o "$OUT/package_id_test" content/package_id_test.cpp ../src/core/apply/ids.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" "${CXX[@]}" -o "$OUT/id_pool_test" content/id_pool_test.cpp ../src/core/apply/ids.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" "${CXX[@]}" -o "$OUT/wear_test" content/wear_test.cpp ../src/core/apply/ids.cpp "${SRC[@]}" & pids+=($!)
"${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I../../packages/sdk/include -I../include -I../third_party/zstd \
    -c ../src/core/apply/content/appliers.cpp -o "$OUT/appliers.windows.o" & pids+=($!)
"${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I../../packages/sdk/include -I../include -I../third_party/zstd \
    -c ../src/core/apply/content/logos.cpp -o "$OUT/logos.windows.o" & pids+=($!)
ZS=../third_party/zstd
mkdir -p "$OUT/zstd"
zobjs=()
for c in $ZS/common/*.c $ZS/compress/*.c $ZS/decompress/*.c; do
    o=$OUT/zstd/$(basename "${c%.c}").o
    zobjs+=("$o")
    [[ $o -nt $c ]] || "${PIN[@]}" zig cc -target x86_64-linux-musl -O2 -DXXH_NAMESPACE=ZSTD_ -DZSTD_LEGACY_SUPPORT=0 \
        -DZSTD_DISABLE_ASM -DDEBUGLEVEL=0 -I$ZS -c "$c" -o "$o" &
done
for p in $(jobs -p); do wait "$p"; done
"${PIN[@]}" zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -ffp-contract=off \
    -o "$OUT/bc7_test" content/bc7_test.cpp & pids+=($!)
"${PIN[@]}" zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness \
    -o "$OUT/logo_cache_test" content/logo_cache_test.cpp & pids+=($!)
"${PIN[@]}" zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -ffp-contract=off -I$ZS \
    -o "$OUT/logo_build_test" content/logo_build_test.cpp "${zobjs[@]}" & pids+=($!)
"${PIN[@]}" zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -ffp-contract=off -I$ZS \
    -o "$OUT/garage_test" content/garage_test.cpp "${zobjs[@]}" & pids+=($!)
"${PIN[@]}" zig c++ -target x86_64-linux-musl -std=c++23 -O1 -Wall -Wextra -Wno-nullability-completeness -ffp-contract=off -I$ZS \
    -I../../packages/sdk/include -o "$OUT/logo_deliver_test" content/logo_deliver_test.cpp "${zobjs[@]}" & pids+=($!)
"${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -Wno-nullability-completeness -ffp-contract=off \
    -c content/bc7_test.cpp -o "$OUT/bc7_test.windows.o" & pids+=($!)
for p in "${pids[@]}"; do wait "$p"; done
"$OUT/bc7_test" content/bc7
"$OUT/logo_cache_test"
"$OUT/logo_build_test" content/bc7 "$OUT/logo_out"
"$OUT/logo_deliver_test" content/bc7
"$OUT/garage_test"
python3 content/check_logo_build.py content/bc7 "$OUT/logo_out"
"$OUT/rows_test" ../../packages/sdk/std/registries
"$OUT/vehicle_tower_test"
"$OUT/package_id_test"
"$OUT/id_pool_test"
"$OUT/wear_test" ../../packages/sdk/std/registries
python3 content/check_lowering.py "$OUT/lower_tool"
python3 content/check_wide_default.py "$OUT/lower_tool"
GAMEDB="/mnt/gaming/SteamLibrary/steamapps/common/F1 25/game_package/gamedatabases/gamedatabases.2025.bin"
if [[ -f $GAMEDB ]]; then
    "$OUT/rows_tool" --registries ../../packages/sdk/std/registries --db "$GAMEDB" --out "$OUT/f2012_edits.json" --summary "$OUT/f2012_summary.json" \
        swofty.f2012 content/wear/f2012.json
    "$OUT/rows_tool" --registries ../../packages/sdk/std/registries --db "$GAMEDB" --out "$OUT/drs_edits.json" --summary "$OUT/drs_summary.json" \
        test.drs content/drs/drs.json
    python3 - "$OUT/drs_summary.json" <<'EOF'
import json, sys
s = json.load(open(sys.argv[1]))
lines = s["lines"]["rules.drs"]
assert s["records"].get("rules.drs") == 2 and s["boot_fresh"].get("rules.drs") == 16, s["records"]
assert any("test.drs/never: group swofty_drs_never_" in l and "race start lap 200" in l for l in lines), lines
print("rules.drs on the game database: 2 groups, 16 new keys")
EOF
fi
