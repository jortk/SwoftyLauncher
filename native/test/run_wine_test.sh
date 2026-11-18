#!/usr/bin/env bash
set -euo pipefail
SELF=$(readlink -f "$0")
cd "$(dirname "$SELF")"
if ! command -v wine >/dev/null || ! command -v xvfb-run >/dev/null || ! command -v zig >/dev/null; then
    exec nix shell nixpkgs#zig nixpkgs#wineWow64Packages.stable nixpkgs#xvfb-run -c "$SELF" "$@"
fi
. ./game_lock.sh
OUT=${1:-$PWD/../build/wine-test}
ITEMS=
grep -qa '\[items\] ownership check hooked' ../dist/swofty/addons/core/swofty_core.dll && ITEMS=1
export ZIG_GLOBAL_CACHE_DIR=$PWD/../build/zig-cache ZIG_LOCAL_CACHE_DIR=$PWD/../build/zig-cache
PIN=(taskset -c 2-7,10,11,14-31 nice -n 10)
# wine cmd /c reports the game process's own exit code; a bare wine launch can exit 0 when a thread the server kills at termination exits last
rm -rf "$OUT/game" "$OUT/stages" "$OUT/packed" "$OUT/filled"
mkdir -p "$OUT/game" "$OUT/stages" "$OUT/packed" "$OUT/filled"
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -o "$OUT/game/F1_25.exe" fake_game.c -lversion -luser32 -lws2_32 &
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -DPACKED_TEST -o "$OUT/packed/F1_25.exe" fake_game.c -lversion -luser32 -lws2_32 &
"${PIN[@]}" zig cc -target x86_64-windows-gnu -O2 -o "$OUT/filled/F1_25.exe" fake_filled_iat.c -luser32 &
cp -r ../dist/. "$OUT/game/"
cp -r ../dist/. "$OUT/filled/"
mkdir -p "$OUT/game/swofty/mods/core" "$OUT/register"
mv "$OUT/game/swofty/addons/core/swofty_gamelog.dll" "$OUT/game/swofty/addons/core/log_channels.tsv" "$OUT/game/swofty/mods/core/"
for m in addons/t_*.cpp; do
    name=$(basename "${m%.cpp}")
    case $name in
    t_chain_b) dest=$OUT/game/swofty/mods ;;
    t_stage_*) dest=$OUT/stages ;;
    t_register) dest=$OUT/register ;;
    t_packed*) dest=$OUT/packed ;;
    t_ntpage | t_int3hook) dest=$OUT ;;
    *) dest=$OUT/game/swofty/addons ;;
    esac
    "${PIN[@]}" zig c++ -target x86_64-windows-gnu -std=c++23 -O2 -Wall -Wextra -I../../packages/sdk/include -I../include -shared -s \
        -o "$dest/$name.dll" "$m" &
done
wait
cp "$OUT/game/swofty/addons/t_drop.dll" "$OUT/game/swofty/mods/t_drop.dll"
python3 - "$OUT/game/swofty/addons/core/bindings.json" bindings.test.json <<'EOF'
import json, sys
doc = json.load(open(sys.argv[1]))
doc['bindings'] += json.load(open(sys.argv[2]))
json.dump(doc, open(sys.argv[1], 'w'), indent=1)
EOF
python3 - "$OUT/game/F1_25.exe" "$OUT/game/swofty/mods/core/log_channels.tsv" <<'EOF'
import struct, sys
b = open(sys.argv[1], 'rb').read()
stamp = struct.unpack_from('<I', b, struct.unpack_from('<I', b, 0x3c)[0] + 8)[0]
open(sys.argv[2], 'w').write(f'# F1_25.exe fake {stamp}\n# hash\tsites\tchannels\n'
                             '0x7c7c88cb\t1\tTEST_WARN\n0x11111111\t1\tTEST_VERBOSE\n0x22222222\t1\tUI_FLOW\n')
EOF
SG=$OUT/stages/game
cp -r "$OUT/game" "$SG"
rm "$SG"/swofty/addons/t_*.dll "$SG"/swofty/mods/t_*.dll
mv "$OUT"/stages/t_stage_*.dll "$SG/swofty/addons/"
python3 - "$SG/F1_25.exe" "$SG/swofty" <<'EOF'
import json, struct, sys
exe, root = sys.argv[1], sys.argv[2]
b = open(exe, 'rb').read()
pe = struct.unpack_from('<I', b, 0x3c)[0]
nsec, stamp, opt = struct.unpack_from('<H', b, pe + 6)[0], struct.unpack_from('<I', b, pe + 8)[0], struct.unpack_from('<H', b, pe + 20)[0]
secs = [struct.unpack_from('<8sIIII', b, pe + 24 + opt + 40 * i) for i in range(nsec)]
_, _, va, size, raw = next(s for s in secs if s[0].rstrip(b'\0') == b'.xtls')
code = b[raw:raw + size]
def kb(tag):
    at = code.find(b'\x48\xb8' + tag.encode())
    assert at >= 0 and code.find(b'\x48\xb8' + tag.encode(), at + 1) < 0, tag
    return hex(va + at)
core = root + '/addons/core/'
doc = json.load(open(core + 'bindings.json'))
for x in doc['bindings']:
    if 'kb_tag' in x:
        x['rva'] = kb(x.pop('kb_tag'))
        if not x.pop('kb_no_match', False):
            x['match'] = kb(x.pop('kb_match_tag')) if 'kb_match_tag' in x else hex(int(x['rva'], 16) - x['anchor'])
    else:
        x.pop('rva', None)
        x.pop('match', None)
        if isinstance(x.get('string_anchor'), dict):
            x['string_anchor'].pop('func_rva', None)
doc['build'] = {'exe': 'F1_25.exe', 'sha256': 'wine-test', 'pe_timestamp': stamp, 'size': len(b)}
json.dump(doc, open(core + 'bindings.wine-test.json', 'w'), indent=1)
json.dump({'schema': 1, 'profile': 'wine-test', 'addons': [], 'early_plugins': ['t_stage_early.dll']},
          open(root + '/settings.json', 'w'))
EOF
PK=$OUT/packed/game PP=$OUT/packed/plain
for d in "$PK" "$PP"; do
    cp -r "$OUT/game" "$d"
    rm "$d"/swofty/addons/t_*.dll "$d"/swofty/mods/t_*.dll "$d/F1_25.exe"
    cp "$OUT"/packed/t_packed*.dll "$d/swofty/addons/"
done
python3 packed_game.py make "$OUT/packed/F1_25.exe" "$PK" "$PP"
export WINEPREFIX="$OUT/prefix" WINEDLLOVERRIDES="version=n,b;mscoree,mshtml=;winemenubuilder.exe,winedbg.exe=d" WINEDEBUG=-all
status=0
LOG=$OUT/game/swofty/loader.log
check() {
    if grep -qE "$2" "$LOG"; then echo "ok  : $1"; else echo "FAIL: $1"; status=1; fi
}
once() {
    local n
    n=$(grep -cE "$2" "$LOG" || true)
    if [[ $n == 1 ]]; then echo "ok  : $1"; else echo "FAIL: $1 (logged $n times)"; status=1; fi
}
count() {
    local n
    n=$(grep -cE "$3" "$LOG" || true)
    if [[ $n == "$2" ]]; then echo "ok  : $1"; else echo "FAIL: $1 (logged $n times, want $2)"; status=1; fi
}
CACHE=$OUT/game/swofty/cache/scan-cache.tsv
for mode in trigger fallback vm settings settings_local session state gamelog_off assets assets_ro nearfull nopool cache_ro udp udp_off; do
    echo "==== $mode"
    if [[ $mode == cache_ro ]]; then
        rm -rf "$OUT/game/swofty/cache" "$WINEPREFIX"/drive_c/users/*/AppData/Local/Swofty/cache/game
        mkdir -p "$OUT/game/swofty/cache"
        chmod a-w "$OUT/game/swofty/cache"
    fi
    stale=
    [[ $mode == vm ]] && stale=$(python3 - "$CACHE" <<'EOF'
import sys
path = sys.argv[1]
lines = open(path).read().split('\n')
i = next(i for i, l in enumerate(lines) if l.endswith('\t48 81 EC 88 02 00 00 C5 F8 29 B4 24 70 02 00 00 4C 89 8C 24') or l.endswith('C5 F8 29 B4 24 70 02 00 00 4C 89 8C 24'))
rva, section, pattern = lines[i].split('\t')
lines[i] = '\t'.join([format(int(rva, 16) + 1, 'x'), section, pattern])
open(path, 'w').write('\n'.join(lines))
print(pattern)
EOF
)
    [[ $mode == settings ]] && sed -i '1s/\t[0-9a-f]*$/\t0123456789abcdef/' "$CACHE"
    resolve=
    [[ $mode == gamelog_off ]] && resolve=scan
    [[ $mode == session ]] && cp session.test.json "$OUT/game/swofty/session.json"
    [[ $mode == udp* ]] && cp udp_ids.test.json "$OUT/game/swofty/udp_ids.json"
    udp_ids=
    [[ $mode == udp_off ]] && udp_ids=0
    req=
    [[ $mode == settings ]] && req=$OUT/state/requests/wine-test.json
    [[ $mode == settings_local ]] && req=$OUT/game/swofty/requests.json
    rm -rf "$OUT/state" "$OUT/game/swofty/requests.json"*
    [[ $req ]] && python3 - settings.test.json "$OUT/game/swofty/settings.json" "$mode" "Z:${req//\//\\}" <<'EOF'
import json, sys
doc = json.load(open(sys.argv[1]))
if sys.argv[3] == 'settings':
    doc['requests'] = sys.argv[4]
else:
    doc['mods'] = doc.pop('addons')
    for item in doc['owned_items']:
        item['mod'] = item.pop('addon')
json.dump(doc, open(sys.argv[2], 'w'), indent=1)
EOF
    [[ $mode == assets* ]] && python3 - "$OUT/game/swofty" "$mode" <<'EOF'
import json, os, shutil, sys
root = sys.argv[1]
addon = os.path.join(root, 'addons', 'example.sample-mod')
files = [('erp/frag.bin', b'frag'), ('lang/extra.lng', b'LNGT'), ('files/sponsor.dds', b'DDS '), ('seeded.txt', b'from the install')]
if sys.argv[2] == 'assets_ro':
    files.append(('save/log.txt', b'zero;'))
for rel, data in files:
    os.makedirs(os.path.dirname(os.path.join(addon, rel)), exist_ok=True)
    open(os.path.join(addon, rel), 'wb').write(data)
os.makedirs(os.path.join(addon, 'native'))
shutil.copy(os.path.join(root, 'addons', 't_assets.dll'), os.path.join(addon, 'native', 't_assets.dll'))
json.dump({'schema': 1, 'profile': 'wine-test',
           'addons': [{'id': 'example.sample-mod', 'dlls': ['t_assets.dll'], 'options': []}],
           'plugins': ['addons/example.sample-mod/native/t_assets.dll']},
          open(os.path.join(root, 'settings.json'), 'w'))
json.dump({'features': ['file', 'telemetry'], 'overlay_off': ['example.sample-mod']},
          open(os.path.join(root, 'assets.json'), 'w'))
EOF
    ro_skip=
    if [[ $mode == assets_ro ]]; then
        LOCAL=$(winepath -u "$(wine cmd /c 'echo %LOCALAPPDATA%' | tr -d '\r')")
        KEPT=$LOCAL/Swofty/addon-storage/example.sample-mod
        rm -rf "$KEPT"
        chmod -R a-w "$OUT/game/swofty/addons/example.sample-mod"
        [[ -w $OUT/game/swofty/addons/example.sample-mod ]] && ro_skip=1
    fi
    if [[ $mode == state ]]; then
        STATE_DIR="$OUT/state-dir-üス"
        rm -rf "$STATE_DIR" "$OUT/game/swofty/registry/ids.json" "$OUT/game/swofty/registry/room.json"
        mkdir -p "$OUT/game/swofty/registry"
        printf '{"version":1,"ids":{"track":{"wine.state/old":{"id":4242}}}}' >"$OUT/game/swofty/registry/ids.json"
        python3 - "$OUT/game/swofty/state.json" "$(winepath -w "$STATE_DIR" | tr -d '\r')" <<'EOF'
import json, sys
json.dump({'version': 1, 'state_dir': sys.argv[2]}, open(sys.argv[1], 'w'))
EOF
    fi
    gl_level=; gl_rate=
    [[ $mode == gamelog_off ]] && gl_level=off
    gl_channels=
    [[ $mode == trigger ]] && gl_rate=0
    [[ $mode == trigger || $mode == vm ]] && gl_channels=default,verbose
    rm -f "$OUT"/game/swofty/stall-*.txt
    SWOFTY_TEST_LOAD_STALL_REPORT=$([[ $mode == state ]] && echo 1) SWOFTY_UDP_STOCK_IDS=$udp_ids SWOFTY_RESOLVE=$resolve SWOFTY_TEST_ASSETS=$([[ $mode == assets* ]] && echo 1) SWOFTY_GAME_LOG=$gl_level SWOFTY_GAME_LOG_RATE=$gl_rate SWOFTY_GAME_LOG_CHANNELS=$gl_channels SWOFTY_TEST_REQUESTS=${req:+Z:${req//\//\\}} SWOFTY_TEST_ITEMS=$ITEMS \
        "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" $mode || status=1
    if [[ $mode == assets_ro ]]; then
        GAME_ADDON=$OUT/game/swofty/addons/example.sample-mod
        ro_files="data log=$(cat "$KEPT/save/log.txt" 2>/dev/null) state=$([[ -e $KEPT/save/state.txt ]] && echo yes || echo no)"
        ro_files+=" game log=$(cat "$GAME_ADDON/save/log.txt" 2>/dev/null) state=$([[ -e $GAME_ADDON/save/state.txt ]] && echo yes || echo no)"
        ro_files+=" seeded=$(cat "$GAME_ADDON/seeded.txt" 2>/dev/null) mark=$(tr -d '\r' <"$KEPT/.swofty-data-folder" 2>/dev/null)"
        chmod -R u+w "$GAME_ADDON"
    fi
    [[ $mode == cache_ro ]] && chmod u+w "$OUT/game/swofty/cache"
    rm -rf "$OUT/game/swofty/udp_ids.json" "$OUT/game/swofty/session.json" "$OUT/game/swofty/settings.json" "$OUT/game/swofty/assets.json" \
        "$OUT/game/swofty/addons/example.sample-mod"
    echo "---- $LOG ($mode)"
    grep -v '  section ' "$LOG"
    if [[ $mode == state ]]; then
        once "stall: the watchdog is armed" '\[stall\] load-stall watchdog armed: a report of every thread.s stack'
        once "stall: the test report is written beside the crash reports" '\[stall\] load-stall report 1 of at most 1: .*thread\(s\) read in [0-9]+ ms; written to swofty\\stall-[0-9]{8}-[0-9]{6}\.txt'
        once "stall: the report lists the threads not in a wait first" '\[stall\] == Threads not in a wait \([0-9]+; [0-9]+ with a frame in F1_25\.exe, listed first'
        once "stall: the report holds the thread table" '\[stall\] == Threads \([0-9]+; [1-9][0-9]* unwound'
        check "stall: a suspended thread's stack is unwound and named" '\[stall\]   #1  [0-9a-f]{16} rsp=[0-9a-f]{16}  [A-Za-z0-9_.]+\+0x[0-9a-f]+ '
        check "stall: a wait is named from the module's exports" '\[stall\] thread [0-9]+.*, waiting in [A-Za-z]+'
        once "stall: the watchdog thread is not suspended" '\[stall\] thread [0-9]+.*\(the load-stall watchdog, not suspended\)'
        once "stall: the report ends" '\[stall\] end of load-stall report 1'
        stall_files=$(ls "$OUT"/game/swofty/stall-*.txt 2>/dev/null | wc -l)
        if [[ $stall_files == 1 ]] && grep -q '^== Loaded modules' "$OUT"/game/swofty/stall-*.txt; then
            echo "ok  : stall: swofty\\stall-<time>.txt holds the report"
        else
            echo "FAIL: stall: swofty\\stall-<time>.txt holds the report ($stall_files file(s))"
            status=1
        fi
        rm -f "$OUT"/game/swofty/stall-*.txt
    fi
    if [[ $mode == trigger ]]; then
        once "stall: the game's deadlock WARN reaches the log" '\[game\] TEST_WARN \(warn\): The signal timed out, this is unexpected and may indicate a deadlock, see trace above for Extra Info[[:space:]]*$'
        once "stall: the WARN asks for a report at once, on the watchdog's thread" '\[stall\] load-stall report 1 of at most 2: the game logged "The signal timed out, this is unexpected and may indicate a deadlock" [0-9]+\.[0-9] s ago; .*thread\(s\) read in [0-9]+ ms; written to swofty\\stall-[0-9]{8}-[0-9]{6}\.txt'
        once "stall: the WARN report says the game's hang signal" '\[stall\] game hang signal: the game.s deadlock warning, [0-9]+\.[0-9] s ago'
        once "stall: the WARN report runs on the load-stall watchdog's own thread" '\[stall\] thread [0-9]+.*\(the load-stall watchdog, not suspended\)'
        once "stall: the WARN report ends" '\[stall\] end of load-stall report 1'
        stall_files=$(ls "$OUT"/game/swofty/stall-*.txt 2>/dev/null | wc -l)
        if [[ $stall_files == 1 ]] && grep -q '^the game logged "The signal timed out' "$OUT"/game/swofty/stall-*.txt; then
            echo "ok  : stall: swofty\\stall-<time>.txt holds the WARN report"
        else
            echo "FAIL: stall: swofty\\stall-<time>.txt holds the WARN report ($stall_files file(s))"
            status=1
        fi
        rm -f "$OUT"/game/swofty/stall-*.txt
    fi
    if [[ $mode == udp ]]; then
        once "udp: the map is read and sendto is hooked" '\[udp\] stock ids on: 1 team id\(s\), 1 driver id\(s\), 0 track\(s\) by Swofty track id, 1 sendto slot\(s\)'
        once "udp: the launcher's mapping lines are logged" '\[udp\] stock ids: team 507 -> 4 \(forceindia\); driver 197 -> 255 \(no stock driver\)'
        once "udp: the first rewritten packet is logged once" '\[udp\] first telemetry packet rewritten: format 2025, packet 4, [a-z0-9]+ model: driver 197 -> 255, team 251 -> 4'
    fi
    if [[ $mode == udp_off ]]; then
        once "udp: SWOFTY_UDP_STOCK_IDS=0 is logged" '\[udp\] stock ids off \(SWOFTY_UDP_STOCK_IDS=0\)'
        count "udp: nothing is hooked or rewritten when off" 0 '\[udp\] (stock ids on|first telemetry)'
    fi
    if [[ $mode == trigger ]]; then
        check "addons: core folder lists swofty_core" 'addons: swofty[\]addons[\]core: 1 dll\(s\)[[:space:]]*$'
        check "addons: legacy core folder still loads" 'addons: swofty[\]mods[\]core: 1 dll\(s\) \(legacy folder\)'
        check "addons: legacy plugin folder still loads" 'addons: swofty[\]mods: 2 dll\(s\) \(legacy folder\)'
        once "addons: a dll in both folders loads from addons only" 'addon t_drop\.dll: also in .*[\]swofty[\]mods, skipped'
        once "mixins: a binding bindings.json does not list is named as missing" 'addon t_want\.dll needs wine-test:Fake_Unlisted, which bindings\.json does not list'
        once "mixins: a listed binding that does not match stays unresolved" 'addon t_want\.dll needs wine-test:Fake_Missing, which does not resolve on this build'
        check "addons: SwoftyAddonInit is called" 'addon t_chain_a\.dll: loaded at .*, calling SwoftyAddonInit \(API v9\)'
        once "apply: the boot plan runs the content appliers with no records" '\[apply\] boot plan: [0-9]+ boot applier\(s\), 0 new database key\(s\) for the value map; nothing for the database to apply'
        check "apply: every boot applier plans no database keys with no content records" '\[apply\] [a-z_.]+ boot: 0 new database key\(s\) planned; no addon records or layers'
        count "apply: no boot applier plans a database key" 0 '\[apply\] [a-z_.]+ boot: [1-9][0-9]* new database key\(s\) planned'
        once "apply: the boot moment runs once with nothing to apply" '\[apply\] boot: [0-9]+ applier\(s\); 0 applied, 0 put back, 0 refused'
        count "apply: no content rows read the database file" 0 '\[apply\] content: read'
        once "apply: the read moment installs at the window stage" '\[apply\] read: [0-9]+ read path\(s\) installed at the window stage'
        once "room: a profile with nothing to size leaves the value map alone" "\\[registry\\] room: nothing to size \\(no older addons' database edits, no 1\\.6 database records\\); the value map is the game's own"
        once "room: room.json is written after boot" '\[registry\] room report written to .*registry.room\.json'
        count "room: no reserve hook without anything to size" 0 'site_Gdb_ReserveTarget hooked'
        python3 - "$OUT/game/swofty/registry/room.json" <<'PY' && echo "ok  : room: room.json says not_needed and names the profile" || { echo "FAIL: room: room.json says not_needed and names the profile"; status=1; }
import json, sys
r = json.load(open(sys.argv[1]))
assert r['version'] == 1 and r['presize'] == 'not_needed' and r['profile'] and r['registry']['asked_keys'] == 0, r
PY
        check "addons: SWOFTY_MOD exports SwoftyAddonInit" 'addon t_conflict_y\.dll: loaded at .*, calling SwoftyAddonInit \(API v9\)'
        if grep -q "core mixin set 'swofty_core.dll (flow states)'" "$LOG"; then
            once "career: the flow-state site is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(flow states\)': 1 mixin"
            once "career: a missing flow-state site drops only its own set" 'addon swofty_core\.dll \(flow states\) dropped: needs site_FeFlow_SetStateStatus'
            once "career: a missing Team HQ site drops only its own set" 'addon swofty_core\.dll \(Team HQ answer\) dropped: needs site_Career_TeamHqIsMyTeamResult'
            check "career: setup reports the library unavailable" '\[career\] setup: the flow-state site is not installed'
        else
            check "career: a core without the FlowState layout registers no career sites" '\[career\] setup: the flow-state site is not installed'
        fi
        check "career: swofty_core itself still initialises" 'addon swofty_core\.dll: SwoftyAddonInit returned 1 \(ok\)'
        check "addons: a plugin exporting only SwoftyModInit loads" 'addon t_v1\.dll: loaded at .*, API v1 plugin, SwoftyModInit runs after the mixin commit'
        check "addons: every dll in both folders loads" 'addons: done, 15 addon\(s\) loaded'
        check "career content: a content registered at init is queued for the commit" '\[career\] content .wine-test. from t_content\.dll registered \(tile 1 after the stock ones, 2026 model, data group f1_career_wine, suffix wt, mode driver_career_wine\): queued for the next mixin commit'
        check "career content: a duplicate data group is refused" '\[career\] content .wine-test-2. from t_content\.dll refused: another content already has data group f1_career_wine'
        check "career content: a stock data group is refused" '\[career\] content .wine-stock. from t_content\.dll refused: data_group f1_career_2026 is a stock career'
        check "career content: the SDK reports ok, invalid, invalid at init" '\[t_content\] at init: available 1, add ok'
        check "career content: bindings the fake exe lacks turn the content off before any hook is offered" '\[career\] content .wine-test. \(t_content\.dll\) is off: needs site_FeTile_CareerContentEnd, .*which this build does not resolve'
        if grep -q "core mixin set 'swofty_core.dll (career contents)'" "$LOG"; then echo "FAIL: career content: no hook set is offered when its bindings are missing"; status=1; else echo "ok  : career content: no hook set is offered when its bindings are missing"; fi
        check "career content: the SDK reads the off state through the core table" '\[t_content\] at load: info ok state 2 \(needs .*\), available 0, data_group unavailable, pressed unavailable, is 0'
        for re in 'scan \.xtls \[.*\] -> 1 hit at' 'registry global base\+0x[0-9a-f]+ \(= [0-9a-f]+ now\)' \
            'hook #1: list\(r14\)=0x[0-9a-f]+ r12=0x2 r15=0x1' 'built mod vtable .* from career vtable .* \(slot 4 populate .* slot 5 getAction' \
            'added button_3 \(items 1 -> 2\)' 'populate #1 .* vm=0*[1-9a-f]' "getAction #1 .* route 'mod_menu'"; do
            grep -qE "\[core\] $re|$re" "$OUT/game/swofty/loader.log" && echo "ok  : T000 event: $re" || { echo "FAIL: T000 event: $re"; status=1; }
        done
    fi
    case $mode in
    state)
        once "state: the core names the folder the launcher gave it" '\[state\] the core.s state files are in .*state-dir-.*\(from swofty/state\.json\)'
        once "state: the old stat-id store moves to it" '\[registry\] stat ids: moved ids\.json from the game folder to .*state-dir-.*registry.ids\.json'
        once "state: the moved stat ids are loaded" '\[registry\] stat ids: ids\.json holds 1 id\(s\)'
        once "state: room.json is written there" '\[registry\] room report written to .*state-dir-.*registry.room\.json'
        count "state: nothing fails to write" 0 '\[state\] .*can.t write to'
        if [[ -s $STATE_DIR/registry/room.json && -s $STATE_DIR/registry/ids.json && ! -e $OUT/game/swofty/registry/room.json && ! -e $OUT/game/swofty/registry/ids.json ]]; then
            echo "ok  : state: room.json and ids.json are in the state folder and not in the game folder"
        else
            echo "FAIL: state: room.json and ids.json are in the state folder and not in the game folder"; status=1
        fi
        rm -rf "$STATE_DIR" "$OUT/game/swofty/state.json"
        ;;
    trigger)
        once "hook pool: the loader reserves it next to the image at attach" 'hook pool: 2048 KB reserved at [0-9A-Fa-f]+ \((above|below) the image\)'
        once "hook pool: the core adopts it" '\[core\] hook pool: 2048 KB at 0x[0-9a-f]+ \(base[-+]0x[0-9a-f]+\), reserved at loader attach'
        ;;
    nearfull)
        once "hook pool: the core adopts it with the space around the image taken" '\[core\] hook pool: 2048 KB at 0x[0-9a-f]+ \(base[-+]0x[0-9a-f]+\), reserved at loader attach'
        check "hook pool: sites install from it" 'site site_FeHome_AfterCareerCall at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\) installed'
        check "hook pool: redirects install from it" '\[core\] redirect [^ ]+ at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\) installed'
        count "hook pool: no install misses memory" 0 'did not allocate'
        ;;
    nopool)
        once "hook pool: the core says when the loader has none" '\[core\] hook pool: the loader reserved none at attach'
        check "hook pool: without it an install names the missing memory and the bytes" '\[core\] (site|wrap|redirect) on [A-Za-z_]+ at 0x[0-9a-f]+: the (trampoline|stub|jump stub) did not allocate: no free memory within 2 GB of the target'
        once "hook pool: the free-region map is logged once" '\[core\]   within 2 GB of 0x[0-9a-f]+: [0-9]+ free region\(s\), 0 whole 64 KB slot\(s\)'
        ;;
    cache_ro)
        once "caches: a game folder that refuses files moves them to the data folder" '\[core\] caches: the game can.t create files in swofty[\]cache \(error [0-9]+\); the scan and exe hash caches are in %LOCALAPPDATA%[\]Swofty[\]cache[\]game[[:space:]]*$'
        check "caches: the scan cache is saved in the data folder" '\[core\] scan cache: [1-9][0-9]* address\(es\) saved to %LOCALAPPDATA%[\]Swofty[\]cache[\]game[\]scan-cache\.tsv'
        count "caches: nothing fails to write" 0 'scan cache: writing .* failed'
        kept=$(echo "$WINEPREFIX"/drive_c/users/*/AppData/Local/Swofty/cache/game)
        if [[ -s $kept/scan-cache.tsv && -s $kept/exe-sha256.tsv ]]; then
            echo "ok  : caches: scan-cache.tsv and exe-sha256.tsv are in the data folder"
        else
            echo "FAIL: caches: scan-cache.tsv and exe-sha256.tsv are in the data folder ($kept)"
            status=1
        fi
        ;;
    esac
    case $mode in
    trigger)
        once "scan cache: the first run starts without one" '\[core\] scan cache: none yet for this exe \(key [0-9a-f]{16}\)'
        check "scan cache: the addresses the scans found are saved" '\[core\] scan cache: [1-9][0-9]* address\(es\) saved to swofty[\]cache[\]scan-cache\.tsv'
        count "resolve: an exe other than the KB build never uses the KB addresses" 0 'at the KB address'
        count "scan cache: nothing is cached yet on the first run" 0 'the address cached for this exe'
        ;;
    fallback)
        once "loader: a dialog before the window is logged with its text" 'dialog shown: Error: Game launch error\. Error code: 0x00000004[[:space:]]*$'
        count "loader: a dialog is not taken as the main window" 0 "main window visible.*class '#32770'"
        once "loader: another module's window is logged and skipped" "fallback watcher: window class 'F125TuneStatus' belongs to .*kernel32\\.dll, not the game"
        count "loader: another module's window is not taken as the main window" 0 "main window visible.*class 'F125TuneStatus'"
        check "scan cache: the second run reads the saved addresses" '\[core\] scan cache: [1-9][0-9]* address\(es\) for this exe \(key [0-9a-f]{16}\), each checked'
        check "scan cache: a cached address is checked against the pattern and used without a scan" 'scan \.xtls \[.*\] -> 1 hit at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), the address cached for this exe'
        count "scan cache: with every address cached, no pattern needs a full scan" 0 'scan \.xtls \[.*\] -> 1 hit at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\)[[:space:]]*$'
        count "scan cache: an unchanged cache is not rewritten" 0 'scan cache: .* saved to'
        ;;
    settings)
        once "scan cache: a cache written for another exe is dropped" '\[core\] scan cache: swofty[\]cache[\]scan-cache\.tsv belongs to another exe; starting over'
        check "scan cache: the rebuilt cache is saved" '\[core\] scan cache: [1-9][0-9]* address\(es\) saved'
        ;;
    gamelog_off)
        once "resolve: SWOFTY_RESOLVE=scan turns the KB addresses and the scan cache off" '\[core\] binding resolution: SWOFTY_RESOLVE=scan, every pattern is scanned and the scan cache is off'
        count "resolve: SWOFTY_RESOLVE=scan uses no cached address" 0 'the address cached for this exe'
        ;;
    esac
    if [[ $stale ]]; then
        python3 - "$LOG" "$stale" <<'EOF' && echo "ok  : scan cache: a stale address fails the pattern check, is scanned again and saved" || { echo "FAIL: scan cache: a stale address fails the pattern check, is scanned again and saved"; status=1; }
import sys
log, pattern = open(sys.argv[1], errors='replace').read(), sys.argv[2]
full = [l for l in log.split('\n') if f'[{pattern}] -> 1 hit at' in l and 'cached' not in l]
sys.exit(not (len(full) == 1 and 'address(es) saved to' in log))
EOF
    fi
    case $mode in
    trigger)
        once "ers: the new-lap guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(ers new lap guard\)': 1 mixin"
        once "ers: the new-lap guard is armed" '\[ers\] new-lap guard armed'
        once "ers mode: the guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(ers mode guard\)': 1 mixin"
        once "ers mode: the guard is armed" '\[ers\] mode guard armed'
        count "ers mode: each car whose ERS mode is out of range is set to Medium and logged once" 2 '\[ers\] car (8: ERS mode -2141928108|9: ERS mode 4) out of range; set to 1 \(Medium\)'
        once "standings: the guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(standings track guard\)': 2 mixin"
        once "standings: the guard is armed" '\[career\] standings guard armed'
        once "standings: a round whose track is off is skipped and logged once" '\[career\] standings: round 7 has no track data \(track 200 is off\); skipped'
        once "standings: a round whose weekend can't be read is skipped and logged once" '\[career\] standings: round 10 has no track data; skipped'
        once "weekends: a round whose track has no record is skipped and logged once" '\[career\] weekends: track 1 has no track data; the round shows no track'
        once "teamhq: the guard is armed" '\[career\] teamhq track guard armed'
        once "teamhq: an event whose track has no track data is skipped and logged once" '\[career\] teamhq: track 6 has no track data; the event shows no track'
        once "teamhq: an event whose track has no record is skipped and logged once" '\[career\] teamhq: track 1 has no track data; the event shows no track'
        once "teamhq: a view model event whose track has no record is skipped and logged once" '\[career\] teamhq: advance view model track 1 has no track data; the event shows no track'
        once "teamhq: a my team view model event whose track has no record is skipped and logged once" '\[career\] teamhq: my team view model track 1 has no track data; the event shows no track'
        once "activity: the guard is armed" '\[career\] activity resume guard armed'
        once "activity: a championship without race length fields is skipped and logged once" '\[career\] activity resume: the championship has no race length fields'
        once "pairs: the guard is armed" '\[strategy\] teammate pairs guard armed'
        once "pairs: a team with a third car is logged once" '\[strategy\] team 6 has 3 cars; only its first two are paired for strategy, the others stay unpaired'
        once "setups: the guard is armed" '\[setups\] car guard armed at 1 of 4 sites'
        once "fe tiles: the guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(front-end tile list guard\)': 3 mixin\(s\)"
        once "fe tiles: the guard is armed" '\[fe\] tiles guard armed'
        once "fe tiles: a list missing from the slot array is skipped and logged once" '\[fe\] tiles: entrant list f2_2026 is not in the game data; its tile is left without a list'
        once "ai corner: the guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(ai brake line guard\)': 1 mixin"
        once "ai corner: the guard is armed" '\[ai\] brake line guard armed'
        once "ai corner: a car with no brake line is skipped and logged once" '\[ai\] brake line: car 3 has no brake line ahead inside the next corner on this track'"'"'s AI line'
        once "replay grid: the guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(replay recorder grid guard\)': 1 mixin"
        once "replay grid: the guard is armed" '\[replay\] recorder grid guard armed'
        once "replay grid: the read guard is armed" '\[replay\] read guard armed'
        once "replay grid: the empty state of a cut recordable is skipped on read and logged once" '\[replay\] a recordable the recorder could not hold has no stored state'
        once "replay grid: a recordable whose state outgrows its holder is skipped and logged once" '\[replay\] session has \? cars, the recorder holds 22; CannedEventsSystem needs 768 bytes of replay state and its holder has 512, so it is not recorded'
        once "garage crew: the guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(garage crew guard\)': 3 mixin"
        once "garage crew: the guard is armed" '\[garage\] crew guard armed'
        count "garage crew: each car dropped from a garage's crew vector is logged once" 4 '\[garage\] garage [5-7]: team 1[2-4] car #4[0-9] \(driver 3[0-9][0-9]\) is the (third|extra) car of its garage'
        once "pit crew: a garage whose car has no crew template skips its setup and is logged once" '\[garage\] garage 10: team 14 car #44 has no pit stops crew template; its pit stops crew is not set up, the car still races'
        once "pit crew: a garage with no cars is logged once" '\[garage\] garage 11 has no cars with a crew template; its pit stops crew is not set up'
        once "garage scene: a missing scene is logged once" '\[garage\] the garage crew scene is missing for this track \(garage crew scene 0x0, vehicle scene 0x1111, equipment scene 0x2222\); no garage crew is shown'
        count "ers: each participant without a vehicle is skipped and logged" 4 '\[ers\] vehicle_newlap for vehicle (9|24): participant [01] of 4 has no vehicle'
        once "leaderboard: the time trial leaderboard guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(time trial leaderboard guard\)': 1 mixin"
        once "leaderboard: the time trial leaderboard guard is armed" '\[leaderboard\] time trial leaderboard guard armed'
        count "leaderboard: each view without a board is skipped and logged" 2 '\[leaderboard\] Time Trial leaderboard view 0x[0-9a-f]+ has no board for this track'
        once "lap host: an addon finds the table and attaches" '\[t_laphost\] lap host version 1, size ok, swofty root set, files root set, build bits [0-3], attach 1'
        once "lap host: the core takes the module's hook set while it collects sets" '\[t_laphost\] offer: add_hooks answered 1'
        once "lap host: the module's hook set is a core mixin set of its own" "core mixin set 't_laphost\.dll \(lap host set\)': 1 mixin"
        once "lap host: the module hears its hook set is in place" '\[t_laphost\] settle: the hook set is active'
        once "acclim: the track acclimatisation guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(track acclimatisation guard\)': 1 mixin"
        once "acclim: the track acclimatisation guard is armed" '\[acclim\] track acclimatisation guard armed'
        once "acclim: the programme without targets is skipped and logged" '\[acclim\] Track Acclimatisation programme 0x[0-9a-f]+ has no targets at this track \(targets 0x0\); refresh skipped'
        once "acclim: the track acclimatisation penalty guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(track acclimatisation penalty guard\)': 1 mixin"
        once "acclim: the track acclimatisation penalty guard is armed" '\[acclim\] track acclimatisation penalty guard armed'
        once "acclim: the penalty flag of a programme without targets is skipped and logged" '\[acclim\] Track Acclimatisation programme 0x[0-9a-f]+ has no targets at this track \(targets 0x0\); penalty flag skipped'
        check "events: flow-tick hook installed" '\[events\] Vm_Update_FlowStateMachine hooked at base\+0x[0-9a-f]+'
        check "events: setup reports the onFrame tick live" '\[events\] setup: live; onFrame armed'
        once "events: first flow tick fires with dt 0" '\[events\] flow tick 1 observed, dt 0\.0'
        once "events: second flow tick fires" '\[events\] flow tick 2 observed'
        once "events: a subscribe in SwoftyAddonInit is queued until the setup" '\[events\] subscribe session\.start by t_events_early\.dll -> ok token [0-9]+, queued until the events setup[[:space:]]*$'
        once "events: an on_frame in SWOFTY_ON_LOAD is queued until the setup" '\[events\] on_frame by t_events_early\.dll -> ok token [0-9]+, queued until the events setup[[:space:]]*$'
        once "events: available() is true and on_frame ok at ON_LOAD, before the setup" '\[t_events_early\] at load: events available 1, on_frame ok \(token [0-9]+\), career available 0'
        once "events: the setup arms the queued registrations and names them" '\[events\] [0-9]+ registration\(s\) queued before setup are armed: .*session\.start token [0-9]+ \(t_events_early\.dll\).*on_frame token [0-9]+ \(t_events_early\.dll\)'
        awk '/\[career\] setup:/ && !a {a = NR} /\[t_events_early\] at load/ && !b {b = NR} /\[events\] setup: live/ && !c {c = NR}
             /queued before setup are armed/ && !d {d = NR} /\[events\] flow tick 1 observed/ && !e {e = NR}
             END {exit !(a && a < b && b < c && c < d && d < e)}' "$LOG" &&
            echo "ok  : events: career setup, then ON_LOAD, then the events setup arms the queue, then the first tick" ||
            { echo "FAIL: events: career setup, then ON_LOAD, then the events setup arms the queue, then the first tick"; status=1; }
        check "gamelog: on with the verbose UI channels from the environment" '\[gamelog\] game log to loader\.log: on \(level warn, from environment\), channels .default,verbose.'
        check "gamelog: overwrite on Ne_Log_Thunk installed" 'wrap Ne_Log_Thunk at .* installed: swofty_gamelog\.dll\(0, overwrite\)'
        check "gamelog: site on Ne_Format_String installed" 'site site_Ne_Format_String_Entry at .* installed: swofty_gamelog\.dll\(0\)'
        check "gamelog: default,verbose opens TEST_WARN and UI_FLOW only" '\[gamelog\] gate g_Ne_LogGate at .*: opened 2 channel hash\(es\): TEST_WARN, UI_FLOW[[:space:]]*$'
        count "gamelog: gated warning logged 100 times, then the repeat cap" 100 '\[game\] TEST_WARN \(warn\): fake gate message from site_a[[:space:]]*$'
        once "gamelog: repeat cap note for the warning" '\[game\] \(format "fake gate message from %s" logged 100 times; further repeats are suppressed\)'
        count "gamelog: info on a UI channel from five threads, intact lines, capped at 100" 100 '\[game\] UI_FLOW: fake ui info from site_c[[:space:]]*$'
        count "gamelog: closed channel never reaches the log" 0 'TEST_VERBOSE|site_b([^a-z_]|$)'
        once "gamelog: route error from Ne_Format_String" '\[game\] UI_ROUTE \(error\): Screen swofty_settings failed to open because of 3[[:space:]]*$'
        once "gamelog: Solution-style error from Ne_Format_String" '\[game\] UI_ROUTE \(error\): Failed to find screen swofty_settings\. Solution: Make sure the the screen is loaded into the runtime[[:space:]]*$'
        once "gamelog: later route error" '\[game\] UI_ROUTE \(error\): No screen called late_screen exists[[:space:]]*$'
        count "gamelog: ordinary formats are not logged" 0 'button_7'
        once "gamelog: a %s into reserved memory logs the raw format" '\[game\] TEST_WARN \(warn\): \(unformattable\) fake replay record %s[[:space:]]*$'
        count "gamelog: rate limit off (SWOFTY_GAME_LOG_RATE=0)" 0 'dropped by the rate limit'
        ;;
    fallback)
        check "gamelog: on with the defaults" '\[gamelog\] game log to loader\.log: on \(level warn, from defaults\)'
        check "gamelog: default channels open TEST_WARN and UI_FLOW (UI channels at warn level)" '\[gamelog\] gate g_Ne_LogGate at .*: opened 2 channel hash\(es\): TEST_WARN, UI_FLOW[[:space:]]*$'
        count "gamelog: info messages on a UI channel are not logged by default" 0 'UI_FLOW: fake ui info'
        ;;
    vm)
        check "gamelog: default rate limit drops the burst and reports it" '\[game\] \([0-9]+ message\(s\) dropped by the rate limit of 100 per second\)'
        ;;
    session)
        check "gamelog: declared in session.json, its overwrite installs" 'wrap Ne_Log_Thunk at .* installed: swofty_gamelog\.dll\(0, overwrite\)'
        ;;
    gamelog_off)
        check "gamelog: SWOFTY_GAME_LOG=off" '\[gamelog\] game log to loader\.log: off \(level off, from environment\)'
        count "gamelog: off writes no game lines" 0 '\[game\] '
        ;;
    esac
    if [[ $mode == assets_ro && $ro_skip ]]; then
        echo "skip: this user writes in read-only folders (root?), so the read-only addon folder cannot be staged"
        continue
    fi
    if [[ $mode == assets* ]]; then
        api='\[assets\] api: example\.sample-mod \(t_assets\.dll\)'
        once "plugins: settings.json lists the addon's plugin" 'addons: settings\.json lists 1 plugin\(s\) in addon folders'
        once "plugins: the plugin loads from its addon folder" 'addon t_assets\.dll: loading .*[\]swofty[\]addons[\]example\.sample-mod[\]native[\]t_assets\.dll[[:space:]]*$'
        once "plugins: the loose copy of a listed plugin is skipped" 'addon t_assets\.dll: settings\.json lists it in an addon folder, the copy in .*[\]swofty[\]addons is skipped'
        once "plugins: the plugin initialises" 'addon t_assets\.dll: SwoftyAddonInit returned 1 \(ok\)'
        check "assets: available follows the features" '\[t_assets\] available: group 1, erp 1, db 1, lang 1, file 1, sound 0'
        once "overlay: on_paint from an addon in overlay_off is refused" '\[overlay\] api: example\.sample-mod on_paint -> unavailable \(turned off for this addon in the launcher\)'
        check "overlay: an addon in overlay_off sees the overlay unavailable" '\[t_assets\] overlay available 0 on_paint unavailable'
        once "assets: register_group during plugin init" "$api register_group asset_groups/t_assets/t\.assetgroup -> ok[[:space:]]*\$"
        once "assets: put_resource during plugin init" "$api put_resource eaid://t_assets/t\.tif\.image -> ok[[:space:]]*\$"
        once "assets: db_set by key is late at the window stage" "$api db_set entrants\.teams\.team\.t_assets = 1 -> late: the core loaded at the window stage"
        once "assets: db_set by hash is late at the window stage" "$api db_set 0xf01d03c7a1b2c3d4 tag 0 = 1 -> late"
        once "assets: lang_set during plugin init" "$api lang_set eng lng_t_assets -> ok[[:space:]]*\$"
        once "assets: lang_table during plugin init" "$api lang_table eng lang/extra\.lng -> ok[[:space:]]*\$"
        once "assets: overlay_file with the file feature on" "$api overlay_file t_assets/sponsor\.dds files/sponsor\.dds -> ok[[:space:]]*\$"
        once "assets: a path outside the addon folder is invalid" "$api lang_table eng \.\./escape\.lng -> invalid"
        once "assets: put_resource after plugin init is late" "$api put_resource eaid://t_assets/late\.tif\.image -> late: its point has passed"
        once "assets: the group joins the declared list" '\[assets\]   asset_groups/t_assets/t\.assetgroup -> 2025_asset_groups/t_assets/t\.erp'
        once "assets: the resource joins the declared list" '\[assets\] resources: 1 declared in swofty/assets\.json'
        once "assets: the string joins a table" "\[assets\] lang: table for 'eng': 1 string\(s\) from assets\.json strings"
        once "assets: the file joins the overlay" '\[assets\] overlay:   t_assets/sponsor\.dds -> /game/swofty/addons/example\.sample-mod/files/sponsor\.dds \(addon example\.sample-mod\)'
        sapi='\[storage\] api: example\.sample-mod \(t_assets\.dll\)'
        check "storage: available with the feature on" '\[t_assets\] storage available 1'
        once "storage: write lands in the addon folder" "$sapi write save/state\.txt \(13 byte\(s\)\) -> ok[[:space:]]*\$"
        once "storage: read returns the file" "$sapi read save/state\.txt \(13 byte\(s\)\) -> ok[[:space:]]*\$"
        once "storage: the round trip returns the bytes written" "\[t_assets\] storage read ok 'round-trip-ok'"
        count "storage: size reports the file length (read-back plus the explicit call)" 2 "$sapi size save/state\.txt -> ok[[:space:]]*\$"
        check "storage: the plugin reads back the size" '\[t_assets\] storage size ok 13'
        once "storage: list enumerates the folder" "$sapi list save -> ok[[:space:]]*\$"
        check "storage: the listing has both files" '\[t_assets\] storage list ok 18 .*log\.txt.*state\.txt|\[t_assets\] storage list ok 18 .*state\.txt.*log\.txt'
        count "storage: append creates then grows a file" 2 "$sapi append save/log\.txt \(4 byte\(s\)\) -> ok"
        check "storage: both appends succeed" '\[t_assets\] storage append ok ok'
        once "storage: a path climbing above the folder is invalid" "$sapi write \.\./escape\.txt \(4 byte\(s\)\) -> invalid: '\.\./escape\.txt' is not a relative path"
        check "storage: climb, absolute, drive and back-climb paths are all rejected" '\[t_assets\] storage escapes invalid invalid invalid invalid'
        if [[ $mode == assets ]]; then
            missing='swofty/addons/example\.sample-mod/missing\.txt is missing'
            gone='swofty/addons/example\.sample-mod/save/state\.txt is missing'
        else
            neither='in neither .*[\]Swofty[\]addon-storage[\]example\.sample-mod[\] nor swofty/addons/example\.sample-mod/'
            missing="missing\\.txt is $neither"
            gone="save/state\\.txt is $neither"
        fi
        once "storage: size of a missing file is invalid" "$sapi size missing\.txt -> invalid: $missing[[:space:]]*\$"
        once "storage: remove deletes the file" "$sapi remove save/state\.txt -> ok[[:space:]]*\$"
        once "storage: remove of a gone file is invalid" "$sapi remove save/state\.txt -> invalid: $gone[[:space:]]*\$"
        check "storage: the plugin sees remove then missing" '\[t_assets\] storage remove-again invalid'
        if [[ $mode == assets ]]; then
            once "storage: the addon folder takes the files" '\[storage\] example\.sample-mod: files in swofty/addons/example\.sample-mod/[[:space:]]*$'
            check "storage: a file the install placed reads and removes" "\[t_assets\] storage seeded ok 'from the install' remove ok"
        else
            once "storage: an addon folder the game can't change moves the files to the data folder" \
                "\[storage\] example\.sample-mod: the game can't create files in swofty/addons/example\.sample-mod/ \(error 5\), so this session its files are in .*[\]Swofty[\]addon-storage[\]example\.sample-mod[\], and a file not there is read from swofty/addons/example\.sample-mod/[[:space:]]*\$"
            check "storage: a file only in the addon folder reads through, and can't be removed" \
                "\[t_assets\] storage seeded ok 'from the install' remove invalid"
            once "storage: remove names the addon folder's copy that stays" \
                "$sapi remove seeded\.txt -> invalid: swofty/addons/example\.sample-mod/seeded\.txt stays: the game can't remove files there \(error 5\)"
            [[ $ro_files == "data log=zero;one;two; state=no game log=zero; state=no seeded=from the install mark=The game can't create files in swofty/addons/example.sample-mod/ (error 5), so the core keeps example.sample-mod's files here this session." ]] &&
                echo "ok  : storage: writes land in the data folder with its mark, an append starts from the addon folder's copy, the addon folder is untouched" ||
                { echo "FAIL: storage: files after the read-only run: $ro_files"; status=1; }
        fi
        once "telemetry: setup wired through ready() when the feature is on" '\[telemetry\] on; frame tick '
        once "telemetry: the frame tick rides the events hook" '\[telemetry\] on; frame tick hooked'
        count "telemetry: no private flow or session-enter scan" 0 '\[assets\] telemetry: (Vm_Update_FlowStateMachine|Game_GameModule_OnSessionEnter) signature matched'
        once "telemetry: reads are gated off outside a session" '\[t_assets\] telemetry: available 0, session unavailable, car0 unavailable, car99 invalid'
        once "telemetry: an addon opens the shared feed" '\[t_assets\] telemetry: shared_memory ok name=Local.SwoftyTelemetry'
        once "telemetry: the shared feed mapping is created" '\[telemetry\] shared_memory: Local.SwoftyTelemetry mapped, 6920 bytes'
        eapi='\[events\]'
        once "events: the plugin's session.start subscription is accepted and queued" "$eapi subscribe session\.start by example\.sample-mod -> ok token [0-9]+, queued until the events setup[[:space:]]*\$"
        once "events: the plugin's on_frame callback is accepted" "$eapi on_frame by example\.sample-mod -> ok token"
        once "events: an unknown event name is rejected" "$eapi subscribe not\.an\.event by example\.sample-mod -> invalid[[:space:]]*\$"
        once "events: unsubscribe of a live token succeeds" "$eapi unsubscribe token [0-9]+ -> ok[[:space:]]*\$"
        once "events: unsubscribe of an already-dropped token is invalid" "$eapi unsubscribe token [0-9]+ -> invalid[[:space:]]*\$"
        check "events: the plugin sees ok/ok/invalid/ok/invalid through the wrapper" '\[t_assets\] events subscribe ok on_frame ok unknown invalid unsubscribe ok unsubscribe-again invalid'
        check "db: read reports unavailable until the database pointer resolves on this build" '\[t_assets\] db available 0 get unavailable'
        continue
    fi
    count "$mode: no SDK calls, no api lines" 0 '\[assets\] api:'
    if [[ $ITEMS && $req ]]; then
        once "items: owned_items summary" '\[items\] 1 mod item\(s\) to report as owned'
        once "items: key without 0x skipped" "\\[items\\] skipping owned_items entry key 'f01d03c7'"
        once "items: ownership check hooked" '\[items\] ownership check hooked at base\+0x[0-9a-f]+'
        once "items: mod key logged once" '\[items\] reporting mod item 0xf01d03c7 \(se_livery 22, rmdesigns\.alphatauri-orlen-livery\) as owned'
    elif [[ $ITEMS && $mode != nopool ]]; then
        check "items: no owned_items, nothing hooked" '\[items\] no "owned_items" in settings\.json'
        count "items: nothing reported" 0 '\[items\] reporting'
    fi
    if [[ -z $req ]]; then
        check "$mode: no settings.json, settings stay at their code defaults" '\[settings\] no settings\.json'
        [[ ! -e $OUT/state && ! -e $OUT/game/swofty/requests.json ]] && echo "ok  : $mode: no requests file written" ||
            { echo "FAIL: $mode: a requests file was written"; status=1; }
        continue
    fi
    [[ $mode == settings ]] && where='Z:.*[\]state[\]requests[\]wine-test\.json' || where='Z:.*[\]game[\]swofty[\]requests\.json'
    check "$mode: settings.json parsed" "\[settings\] Z:.*settings\.json: profile 'wine-test', 4 addon\(s\), 8 option\(s\), 46 command\(s\); requests file $where[[:space:]]*\$"
    check "$mode: core's wrap sits in the FeHome_Handle_Command chain" 'wrap FeHome_Handle_Command at .* installed: t_chain_c\.dll\(10\) > swofty_core\.dll\(0\) > t_chain_a\.dll\(0\) > t_chain_b\.dll\(0\)[[:space:]]*$'
    for re in 'example\.sample-mod\.verbose = ON' 'example\.sample-mod\.greeting_count = 9' 'example\.sample-mod\.offset = 0\.75' \
        'example\.sample-mod\.flavour = Replica' 'example\.sample-mod\.mismatched = 4' 'community\.grid-2026\.team_livery = Tier 1 sponsor' \
        'example\.sample-mod\.verbose = OFF' 'community\.grid-2026: disabled from the next launch' 'restart requested: relaunch = true' \
        'restart: WM_CLOSE posted to 1 main window\(s\)'; do
        check "$mode: logged [settings] $re" "\[settings\] $re[[:space:]]*\$"
    done
    once "$mode: type mismatch logged once by core" '\[settings\] t_settings\.dll \(example\.sample-mod\): mismatched is a slider option, the code reads a toggle; the code default is used'
    once "$mode: undeclared setting logged once by core" '\[settings\] t_settings\.dll \(example\.sample-mod\): no option undeclared in settings\.json; the code default is used'
    once "$mode: another mod's option is not visible to this mod" '\[settings\] t_settings\.dll \(example\.sample-mod\): no option audi_name in settings\.json'
    check "$mode: SDK logs the mismatch at init" 'addon settings: mismatched is not a toggle option of this addon in swofty\.json; the code default is used'
    check "$mode: watcher calls logged by the mod" '\[t_settings\] on_change verbose -> false \(call 2\)'
    grep -q 'writing requests file' "$LOG" && { echo "FAIL: $mode: a requests write failed"; status=1; }
    python3 - "$req" "$mode" <<'EOF' || status=1
import json, os, sys
path, mode = sys.argv[1], sys.argv[2]
mod, grid = 'example.sample-mod', 'community.grid-2026'
steps = [('o', mod, 'verbose', True), ('o', mod, 'greeting_count', 9), ('o', mod, 'offset', 0.75), ('o', mod, 'flavour', 'replica'),
         ('o', mod, 'mismatched', 4), ('o', grid, 'team_livery', 'tier01'), ('o', mod, 'verbose', False), ('e', grid, False),
         None, None, ('r',)]
want = {'schema': 1, 'profile': 'wine-test', 'seq': 0, 'options': {}, 'enabled': {}, 'relaunch': False}
bad = 0
for i, s in enumerate(steps, 1):
    if s and s[0] == 'o':
        want['options'].setdefault(s[1], {})[s[2]] = s[3]
    elif s and s[0] == 'e':
        want['enabled'][s[1]] = s[2]
    elif s:
        want['relaunch'] = True
    want['seq'] += bool(s)
    try:
        got = json.load(open(f'{path}.{i}'))
    except (OSError, ValueError) as e:
        got = repr(e)
    ok = isinstance(got, dict) and json.dumps(got, sort_keys=True) == json.dumps(want, sort_keys=True) and list(got) == list(want)
    bad += not ok
    print(f'{"ok  " if ok else "FAIL"}: {mode}: requests file after command {i} ({"not ours" if not s else s[0]}): seq {want["seq"]}, relaunch {want["relaunch"]}')
    if not ok:
        print(f'      got  {got}\n      want {want}')
text = open(path).read()
exact = ('{\n  "schema": 1,\n  "profile": "wine-test",\n  "seq": 9,\n  "options": { "community.grid-2026": { "team_livery": "tier01" }, '
         '"example.sample-mod": { "verbose": false, "greeting_count": 9, "offset": 0.75, "flavour": "replica", "mismatched": 4 } },\n'
         '  "enabled": { "community.grid-2026": false },\n  "relaunch": true\n}\n')
ok = text == exact and not os.path.exists(path + '.tmp')
bad += not ok
print(f'{"ok  " if ok else "FAIL"}: {mode}: final requests file is byte-exact in the section 5 layout, no .tmp left behind')
if not ok:
    print(text)
sys.exit(bad != 0)
EOF
done
check "crash log armed at attach" '\[crash\] crash log armed'
check "clean exit logged" '\[crash\] ExitProcess\(0\)'
check "crash reports armed with a minidump" '\[crash\] crash reports: .*crash-<time>\.txt .*with a stacks-only minidump'
check "Ne heap error reporting hooked by the pattern" '\[crash\] Ne_ReportMemoryError at base\+0x[0-9a-f]+ hooked'
python3 fake_symbols.py "$OUT/game/F1_25.exe" "$OUT/game/swofty/addons/core/symbols.tsv"
SW=$OUT/game/swofty
# A report parser shared by the crash modes: it checks the newest report parses as the expected
# sections and that the minidump beside it is a real MDMP with streams.
parse_report() {
    python3 - "$SW" "$1" "$2" <<'PY'
import glob, os, struct, sys
root, want_reason, checks = sys.argv[1], sys.argv[2], sys.argv[3].split(',')
reports = sorted(glob.glob(os.path.join(root, 'crash-*.txt')), key=os.path.getmtime)
if not reports:
    print('FAIL: parse: no crash-*.txt was written'); sys.exit(1)
path = reports[-1]
text = open(path, encoding='utf-8', errors='replace').read()
bad = 0
def ok(cond, what):
    global bad
    print(f'{"ok  " if cond else "FAIL"}: {what}')
    bad += not cond
ok(text.startswith('Swofty crash report'), 'parse: report has the header')
ok(want_reason in text, f'parse: reason mentions {want_reason!r}')
sections = {l[3:].strip() for l in text.splitlines() if l.startswith('== ')}
for s in ('Registers', "Stack (unwound with the modules' exception data)", 'Code around rip',
          'Memory at the registers', 'Game state (from the black box)', 'Black box: last events',
          'Swofty probes (hook and callback activity)', 'Loaded modules', 'Threads',
          'loader.log (last 200 lines)', 'Minidump'):
    ok(any(name == s or name.startswith(s + ' (') for name in sections), f'parse: has the {s!r} section')
for c in checks:
    if c:
        ok(c in text, f'parse: contains {c!r}')
dump = path[:-4] + '.dmp'
raw = open(dump, 'rb').read() if os.path.exists(dump) else b''
ok(raw[:4] == b'MDMP', 'parse: a minidump beside it starts with MDMP')
ok(len(raw) > 4096, f'parse: the minidump is non-trivial ({len(raw)} bytes)')
nstreams = struct.unpack_from('<I', raw, 8)[0] if len(raw) >= 12 else 0
ok(0 < nstreams < 64, f'parse: the minidump has {nstreams} stream(s)')
sys.exit(bad != 0)
PY
}
for mode in crash exit heap abort; do
    echo "==== $mode"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    timeout -k 5 $([[ $mode == heap || $mode == abort ]] && echo 30 || echo 120) "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" $mode >/dev/null 2>&1 || code=$?
    grep -F '[crash]' "$LOG"
    if [[ $mode == crash ]]; then
        check "crash: first-chance fault with module, section and KB name" '\[crash\] #1 first-chance .*ACCESS_VIOLATION .* at F1_25\.exe\+0x[0-9a-f]+ \.text Fake_Crash_Here\+0x'
        check "crash: faulting access" '\[crash\]   writing 0x10'
        check "crash: unhandled line" '\[crash\] unhandled ACCESS_VIOLATION .*Fake_Crash_Here'
        check "crash: stack walks past the faulting frame" '\[crash\]   #1 .*F1_25\.exe\+0x'
        check "crash: a report with a minidump is written" '\[crash\] crash report .*crash-.*\.txt: unhandled ACCESS_VIOLATION.*minidump crash-.*\.dmp'
        parse_report "Fake_Crash_Here" "Fake_Crash_Here,writing 0x10" || status=1
        [[ $code != 0 ]] && echo "ok  : crash: process ended with $code" || { echo "FAIL: crash: process exited 0"; status=1; }
    elif [[ $mode == exit ]]; then
        check "exit: ExitProcess(255) with the caller's stack" '\[crash\] ExitProcess\(255\) .*caller stack'
        check "exit: caller frame in the exe" '\[crash\]   #0 .*F1_25\.exe\+0x'
        check "exit: a report is written for the non-zero exit" '\[crash\] crash report .*crash-.*: ExitProcess\(255'
        parse_report "ExitProcess(255" "" || status=1
        [[ $code == 255 ]] && echo "ok  : exit: process ended with 255" || { echo "FAIL: exit: process ended with $code"; status=1; }
    elif [[ $mode == heap ]]; then
        check "heap: the not-fatal heap error is logged with its stats" '\[crash\] Ne heap error 0x50 \(not fatal\).*capacity 65536, used 512, free 65024, largest free 64512'
        check "heap: the fatal heap error 0x16 is logged" '\[crash\] Ne heap error 0x16 \(fatal\).*Failed to find address'
        check "heap: a report is written before the abort" '\[crash\] crash report .*crash-.*: fatal Ne heap error 0x16'
        parse_report "fatal Ne heap error 0x16" "Ne heap,FakeHeap,Failed to find address,capacity 65536,guard 32 bytes,CORRUPT" || status=1
        [[ $code != 0 && $code != 124 && $code != 137 ]] && echo "ok  : heap: the process aborted ($code)" || { echo "FAIL: heap: the process did not abort on its own (exit $code)"; status=1; }
    else
        check "abort: Ne_FatalAbort is recognised" '\[crash\] crash report .*crash-.*: the game aborted itself'
        parse_report "the game aborted itself" "Ne_FatalAbort" || status=1
        [[ $code != 0 && $code != 124 && $code != 137 ]] && echo "ok  : abort: the process aborted ($code)" || { echo "FAIL: abort: the process did not abort on its own (exit $code)"; status=1; }
    fi
done
for guard in on off; do
    echo "==== acclim ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_ACCLIM_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" acclim >"$OUT/acclim.out" 2>&1 || code=$?
    grep -E '\[acclim\]|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "acclim on: the refresh of a programme without targets is skipped" '\[acclim\] Track Acclimatisation programme 0x[0-9a-f]+ has no targets at this track \(targets 0x0\); refresh skipped'
        grep -q 'acclim: the refresh returned' "$OUT/acclim.out" && echo "ok  : acclim on: the refresh returned" ||
            { echo "FAIL: acclim on: the refresh did not return"; status=1; }
        count "acclim on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: acclim on: a crash report was written"; status=1; } ||
            echo "ok  : acclim on: no crash report"
    else
        once "acclim off: the guard is off" '\[acclim\] track acclimatisation guard off \(SWOFTY_ACCLIM_GUARD=0\)'
        check "acclim off: the refresh faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Game_PracticeTrackAcclimatisation_RefreshState\+0x1fb'
        check "acclim off: faulting access" '\[crash\]   reading 0x4'
        parse_report "Game_PracticeTrackAcclimatisation_RefreshState+0x1fb" "reading 0x4" || status=1
        [[ $code != 0 ]] && echo "ok  : acclim off: process ended with $code" || { echo "FAIL: acclim off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== acclim_penalty ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_ACCLIM_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" acclim_penalty >"$OUT/acclim_penalty.out" 2>&1 || code=$?
    grep -E '\[acclim\]|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "acclim_penalty on: the penalty flag of a programme without targets is skipped" '\[acclim\] Track Acclimatisation programme 0x[0-9a-f]+ has no targets at this track \(targets 0x0\); penalty flag skipped'
        grep -q 'acclim: the penalty flag returned' "$OUT/acclim_penalty.out" && echo "ok  : acclim_penalty on: the penalty flag returned" ||
            { echo "FAIL: acclim_penalty on: the penalty flag did not return"; status=1; }
        count "acclim_penalty on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: acclim_penalty on: a crash report was written"; status=1; } ||
            echo "ok  : acclim_penalty on: no crash report"
    else
        once "acclim_penalty off: the guard is off" '\[acclim\] track acclimatisation guard off \(SWOFTY_ACCLIM_GUARD=0\)'
        flag=$(grep -P '\tGame_PracticeTrackAcclimatisation_FlagCurrentTarget\t' "$SW/addons/core/symbols.tsv" | cut -f1)
        fault=$(printf '%x' $((0x${flag:-0} + 0x14)))
        check "acclim_penalty off: the penalty flag faults where the game does" "\[crash\] unhandled ACCESS_VIOLATION \(0xc0000005\) at F1_25\.exe\+0x$fault "
        check "acclim_penalty off: faulting access" '\[crash\]   reading 0xc4'
        parse_report "F1_25.exe+0x$fault" "reading 0xc4" || status=1
        [[ $code != 0 ]] && echo "ok  : acclim_penalty off: process ended with $code" || { echo "FAIL: acclim_penalty off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== ers_mode ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_ERS_MODE_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" ers_mode >"$OUT/ers_mode.out" 2>&1 || code=$?
    grep -E '\[ers\] (mode|car)|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "ers_mode on: the car's ERS mode is set to Medium before the load" '\[ers\] car 7: ERS mode -2141928108 out of range; set to 1 \(Medium\)'
        grep -q 'ers mode: the step returned 0.50' "$OUT/ers_mode.out" && echo "ok  : ers_mode on: the step loaded the Medium gear modifier" ||
            { echo "FAIL: ers_mode on: the step did not load the Medium gear modifier"; status=1; }
        count "ers_mode on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: ers_mode on: a crash report was written"; status=1; } ||
            echo "ok  : ers_mode on: no crash report"
    else
        once "ers_mode off: the guard is off" '\[ers\] mode guard off \(SWOFTY_ERS_MODE_GUARD=0\)'
        check "ers_mode off: the step faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Car_Powertrain_Step\+0x20'
        parse_report "Car_Powertrain_Step+0x20" "" || status=1
        [[ $code != 0 ]] && echo "ok  : ers_mode off: process ended with $code" || { echo "FAIL: ers_mode off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== standings ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_STANDINGS_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" standings >"$OUT/standings.out" 2>&1 || code=$?
    grep -E '\[career\] standings|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "standings on: the round is skipped and logged" '\[career\] standings: round 5 has no track data \(track 200 is off\); skipped'
        grep -q 'standings: the round returned 0\b' "$OUT/standings.out" && echo "ok  : standings on: the round took the game's no-record path" ||
            { echo "FAIL: standings on: the round did not take the game's no-record path"; status=1; }
        count "standings on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: standings on: a crash report was written"; status=1; } ||
            echo "ok  : standings on: no crash report"
    else
        once "standings off: the guard is off" '\[career\] standings guard off \(SWOFTY_STANDINGS_GUARD=0\)'
        check "standings off: the round faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Career_Standings_BuildWeekendNode\+0x28'
        parse_report "Career_Standings_BuildWeekendNode+0x28" "" || status=1
        [[ $code != 0 ]] && echo "ok  : standings off: process ended with $code" || { echo "FAIL: standings off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== weekends ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_STANDINGS_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" weekends >"$OUT/weekends.out" 2>&1 || code=$?
    grep -E '\[career\] weekends|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        grep -q 'weekends: the round returned 0\b' "$OUT/weekends.out" && echo "ok  : weekends on: the round read as empty" ||
            { echo "FAIL: weekends on: the round did not read as empty"; status=1; }
        count "weekends on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: weekends on: a crash report was written"; status=1; } ||
            echo "ok  : weekends on: no crash report"
    else
        check "weekends off: the read faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Career_Weekends_BuildResultNodes\+0x13'
        [[ $code != 0 ]] && echo "ok  : weekends off: process ended with $code" || { echo "FAIL: weekends off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== teamhq ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_TEAMHQ_TRACK_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" teamhq >"$OUT/teamhq.out" 2>&1 || code=$?
    grep -E '\[career\] teamhq|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "teamhq on: the event is skipped and logged" '\[career\] teamhq: track 6 has no track data; the event shows no track'
        grep -q 'teamhq: the event returned e3e7\b' "$OUT/teamhq.out" && echo "ok  : teamhq on: the event took the game's empty node path" ||
            { echo "FAIL: teamhq on: the event did not take the game's empty node path"; status=1; }
        grep -q 'teamhq: the view model event took the empty node 1' "$OUT/teamhq.out" && echo "ok  : teamhq on: the view model event took the game's empty node path" ||
            { echo "FAIL: teamhq on: the view model event did not take the game's empty node path"; status=1; }
        grep -q 'teamhq: the my team view model event took the empty node 1' "$OUT/teamhq.out" && echo "ok  : teamhq on: the my team view model event took the game's empty node path" ||
            { echo "FAIL: teamhq on: the my team view model event did not take the game's empty node path"; status=1; }
        count "teamhq on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: teamhq on: a crash report was written"; status=1; } ||
            echo "ok  : teamhq on: no crash report"
    else
        once "teamhq off: the guard is off" '\[career\] teamhq track guard off \(SWOFTY_TEAMHQ_TRACK_GUARD=0\)'
        check "teamhq off: the read faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Career_TeamHqAdvance_BuildTrackNode\+0x52'
        [[ $code != 0 ]] && echo "ok  : teamhq off: process ended with $code" || { echo "FAIL: teamhq off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== activity ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_ACTIVITY_RESUME_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" activity >"$OUT/activity.out" 2>&1 || code=$?
    grep -E '\[career\] activity resume|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "activity on: the guard is armed" '\[career\] activity resume guard armed'
        once "activity on: the read is skipped and logged once" '\[career\] activity resume: the championship has no race length fields'
        grep -q 'activity: the report returned full\b' "$OUT/activity.out" && echo "ok  : activity on: the report kept the default text" ||
            { echo "FAIL: activity on: the report did not keep the default text"; status=1; }
        count "activity on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: activity on: a crash report was written"; status=1; } ||
            echo "ok  : activity on: no crash report"
    else
        once "activity off: the guard is off" '\[career\] activity resume guard off \(SWOFTY_ACTIVITY_RESUME_GUARD=0\)'
        check "activity off: the read faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*\.data1 fn base\+0x[0-9a-f]+\+0x65 '
        [[ $code != 0 ]] && echo "ok  : activity off: process ended with $code" || { echo "FAIL: activity off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== setup_vm ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_SETUP_VM_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" setup_vm >"$OUT/setup_vm.out" 2>&1 || code=$?
    grep -E '\[setups\]|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "setup_vm on: the refresh is skipped and logged" '\[setups\] the setup screen has no car yet; skipped'
        grep -q 'setups: the refresh returned with 0 range updates' "$OUT/setup_vm.out" && echo "ok  : setup_vm on: the refresh took the game's skip path" ||
            { echo "FAIL: setup_vm on: the refresh did not take the game's skip path"; status=1; }
        count "setup_vm on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: setup_vm on: a crash report was written"; status=1; } ||
            echo "ok  : setup_vm on: no crash report"
    else
        once "setup_vm off: the guard is off" '\[setups\] car guard off \(SWOFTY_SETUP_VM_GUARD=0\)'
        check "setup_vm off: the read faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Game_CarSetupVm_Initialise\+0x18'
        [[ $code != 0 ]] && echo "ok  : setup_vm off: process ended with $code" || { echo "FAIL: setup_vm off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== pairs ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_TEAMMATE_PAIRS_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine "$OUT/game/F1_25.exe" pairs >"$OUT/pairs.out" 2>&1 || code=$?
    grep -E '\[strategy\]|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "pairs on: the team is logged" '\[strategy\] team 6 has 3 cars; only its first two are paired for strategy, the others stay unpaired'
        grep -q 'pairs: the team was paired' "$OUT/pairs.out" && echo "ok  : pairs on: the team with three cars was paired" ||
            { echo "FAIL: pairs on: the team with three cars was not paired"; status=1; }
        count "pairs on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: pairs on: a crash report was written"; status=1; } ||
            echo "ok  : pairs on: no crash report"
    else
        once "pairs off: the guard is off" '\[strategy\] teammate pairs guard off \(SWOFTY_TEAMMATE_PAIRS_GUARD=0\)'
        check "pairs off: the sort faults where the game throws" '\[crash\] unhandled ACCESS_VIOLATION .*Sess_Strategy_SortTeamCars'
        [[ $code != 0 ]] && echo "ok  : pairs off: process ended with $code" || { echo "FAIL: pairs off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== fe_tiles ($guard)"
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    if [[ $guard == on ]]; then
        code=0
        timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" fe_scan >"$OUT/fe_scan.out" 2>&1 || code=$?
        grep -E '\[fe\] tiles|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
        grep -q 'fe tiles: the scan returned 0\b' "$OUT/fe_scan.out" && echo "ok  : fe tiles on: the scan took the game's no-list path" ||
            { echo "FAIL: fe tiles on: the scan did not take the game's no-list path"; status=1; }
        code=0
        timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" fe_row >"$OUT/fe_row.out" 2>&1 || code=$?
        grep -q 'fe tiles: the list returned 0\b' "$OUT/fe_row.out" && echo "ok  : fe tiles on: the list with no row returned 0" ||
            { echo "FAIL: fe tiles on: the list with no row did not return 0"; status=1; }
        count "fe tiles on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: fe tiles on: a crash report was written"; status=1; } ||
            echo "ok  : fe tiles on: no crash report"
    else
        for mode in fe_scan:0x80 fe_row:0xaa; do
            name=${mode%%:*}
            at=${mode##*:}
            fn=$([[ $name == fe_scan ]] && echo FeTile_Get_ListFirstEntrantId || echo FeTile_Check_EntrantListUsable)
            code=0
            rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
            SWOFTY_FE_TILE_GUARD=0 timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" "$name" >"$OUT/$name.out" 2>&1 || code=$?
            grep -E '\[fe\] tiles|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
            once "fe tiles off ($name): the guard is off" '\[fe\] tiles guard off \(SWOFTY_FE_TILE_GUARD=0\)'
            check "fe tiles off ($name): the lookup faults where the game does" "\\[crash\\] unhandled ACCESS_VIOLATION .*$fn\\+$at"
            parse_report "$fn+$at" "" || status=1
            [[ $code != 0 ]] && echo "ok  : fe tiles off ($name): process ended with $code" || { echo "FAIL: fe tiles off ($name): process exited 0"; status=1; }
        done
    fi
done
for guard in on off; do
    echo "==== ai_corner ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_AI_CORNER_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" ai_corner >"$OUT/ai_corner.out" 2>&1 || code=$?
    grep -E '\[ai\] brake line|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "ai corner on: the car is skipped and logged" '\[ai\] brake line: car 3 has no brake line ahead inside the next corner on this track'"'"'s AI line'
        grep -q 'ai corner: the car returned 0x9be\b' "$OUT/ai_corner.out" && echo "ok  : ai corner on: the car took the game's no-corner-line path" ||
            { echo "FAIL: ai corner on: the car did not take the game's no-corner-line path"; status=1; }
        count "ai corner on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: ai corner on: a crash report was written"; status=1; } ||
            echo "ok  : ai corner on: no crash report"
    else
        once "ai corner off: the guard is off" '\[ai\] brake line guard off \(SWOFTY_AI_CORNER_GUARD=0\)'
        check "ai corner off: the read faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Ai_Job_UpdateCarsParallel\+0x10'
        [[ $code != 0 ]] && echo "ok  : ai corner off: process ended with $code" || { echo "FAIL: ai corner off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== replay_grid ($guard)"
    code=0
    SWOFTY_REPLAY_GRID_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" replay_grid >"$OUT/replay_grid.out" 2>&1 || code=$?
    grep -E '\[replay\] (session has|recorder grid guard)' "$LOG" || true
    if [[ $guard == on ]]; then
        once "replay grid on: the oversize state is skipped and logged" '\[replay\] session has \? cars, the recorder holds 22; VehicleSystem needs 12288 bytes of replay state and its holder has 7168, so it is not recorded'
        grep -q 'replay grid: the recorder would copy 0x0 bytes' "$OUT/replay_grid.out" && echo "ok  : replay grid on: the recorder copies nothing into the holder" ||
            { echo "FAIL: replay grid on: the recorder would still copy the state"; status=1; }
    else
        once "replay grid off: the guard is off" '\[replay\] recorder grid guard off \(SWOFTY_REPLAY_GRID_GUARD=0\)'
        grep -q 'replay grid: the recorder would copy 0x3000 bytes' "$OUT/replay_grid.out" && echo "ok  : replay grid off: the recorder copies the whole state, as the game does" ||
            { echo "FAIL: replay grid off: the state was changed"; status=1; }
        count "replay grid off: nothing is logged about a skipped state" 0 '\[replay\] session has'
    fi
done
for guard in on off; do
    echo "==== garage_crew ($guard)"
    code=0
    SWOFTY_GARAGE_CREW_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" garage_crew >"$OUT/garage_crew.out" 2>&1 || code=$?
    grep -E '\[garage\]' "$LOG" || true
    if [[ $guard == on ]]; then
        [[ $code == 0 ]] && echo "ok  : garage_crew on: the test run finished" || { echo "FAIL: garage_crew on: exit $code"; status=1; }
        grep -q 'FAIL' "$OUT/garage_crew.out" && { echo "FAIL: garage_crew on: a check failed"; grep FAIL "$OUT/garage_crew.out"; status=1; } || echo "ok  : garage_crew on: every check passed"
    else
        once "garage_crew off: the guard is off" '\[garage\] crew guard off \(SWOFTY_GARAGE_CREW_GUARD=0\)'
        grep -q 'garage 5 with slots 0,1,2 keeps 3 entries' "$OUT/garage_crew.out" && echo "ok  : garage_crew off: the vector keeps the third car" ||
            { echo "FAIL: garage_crew off: the vector lost the third car"; status=1; }
    fi
done
for guard in on off; do
    echo "==== pit_slots ($guard)"
    code=0
    SWOFTY_PIT_SLOT_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" pit_slots >"$OUT/pit_slots.out" 2>&1 || code=$?
    grep -E '\[pit\]' "$LOG" || true
    if [[ $guard == on ]]; then
        [[ $code == 0 ]] && echo "ok  : pit_slots on: the test run finished" || { echo "FAIL: pit_slots on: exit $code"; status=1; }
        grep -q 'FAIL' "$OUT/pit_slots.out" && { echo "FAIL: pit_slots on: a check failed"; grep FAIL "$OUT/pit_slots.out"; status=1; } || echo "ok  : pit_slots on: every check passed"
        check "pit_slots on: the guard is armed" '\[pit\] slot guard armed'
        check "pit_slots on: a listed car without a slot is logged" '\[pit\] guard: vehicle [0-9]+ has no race slot, a listed car .*the entry is skipped'
        check "pit_slots on: an own car without a slot is logged" '\[pit\] guard: vehicle [0-9]+ has no race slot, its own car .*the check answers no'
    else
        once "pit_slots off: the guard is off" '\[pit\] slot guard off \(SWOFTY_PIT_SLOT_GUARD=0\)'
        grep -q 'FAIL' "$OUT/pit_slots.out" && { echo "FAIL: pit_slots off: a check failed"; grep FAIL "$OUT/pit_slots.out"; status=1; } || echo "ok  : pit_slots off: the unguarded cases pass"
        grep -q 'guard off, the null slot cases are not run' "$OUT/pit_slots.out" && echo "ok  : pit_slots off: the null slot cases are left out" ||
            { echo "FAIL: pit_slots off: the test did not reach its end"; status=1; }
    fi
done
for guard in on off; do
    echo "==== leaderboard ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_LEADERBOARD_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" leaderboard >"$OUT/leaderboard.out" 2>&1 || code=$?
    grep -E '\[leaderboard\]|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    if [[ $guard == on ]]; then
        once "leaderboard on: the view of an empty board list is skipped" '\[leaderboard\] Time Trial leaderboard view 0x[0-9a-f]+ has no board for this track'
        grep -q 'leaderboard: the view returned' "$OUT/leaderboard.out" && echo "ok  : leaderboard on: the view returned" ||
            { echo "FAIL: leaderboard on: the view did not return"; status=1; }
        count "leaderboard on: no unhandled fault" 0 '\[crash\] unhandled '
        compgen -G "$SW/crash-*.txt" >/dev/null && { echo "FAIL: leaderboard on: a crash report was written"; status=1; } ||
            echo "ok  : leaderboard on: no crash report"
    else
        once "leaderboard off: the guard is off" '\[leaderboard\] time trial leaderboard guard off \(SWOFTY_LEADERBOARD_GUARD=0\)'
        check "leaderboard off: the view faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Game_TimeTrialLeaderboard_SelectBoard\+0x81'
        check "leaderboard off: faulting access" '\[crash\]   reading 0x98'
        parse_report "Game_TimeTrialLeaderboard_SelectBoard+0x81" "reading 0x98" || status=1
        [[ $code != 0 ]] && echo "ok  : leaderboard off: process ended with $code" || { echo "FAIL: leaderboard off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== myteam_vehicle ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_MYTEAM_VEHICLE_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" myteam_vehicle >"$OUT/myteam_vehicle.out" 2>&1 || code=$?
    grep -E '\[myteam\]|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    grep -E 'myteam vehicle|FAIL' "$OUT/myteam_vehicle.out" || true
    if [[ $guard == on ]]; then
        once "myteam_vehicle on: the guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(my team vehicle base guard\)': 1 mixin"
        once "myteam_vehicle on: the guard is armed" '\[myteam\] vehicle base guard armed'
        once "myteam_vehicle on: a base the database can't confirm missing is left to the game" "\\[myteam\\] save: My Team vehicle base 1 \\(stat id 0\\) uses vehicle 'myteam_mes_porsche_2026', whose row index reads as missing; it is not a known addon's vehicle or its entrants\\.vehicles row is in the game data, so the game's own load runs for it"
        count "myteam_vehicle on: no base is skipped without the database confirming its row is missing" 0 '\[myteam\] save: .* the base is skipped'
        check "myteam_vehicle on: the load faults where the game does, as with the guard off" '\[crash\] unhandled ACCESS_VIOLATION .*Sess_Load_VehicleAiConfig\+0x'
        [[ $code != 0 ]] && echo "ok  : myteam_vehicle on: process ended with $code" || { echo "FAIL: myteam_vehicle on: process exited 0"; status=1; }
    else
        once "myteam_vehicle off: the guard is off" '\[myteam\] vehicle base guard off \(SWOFTY_MYTEAM_VEHICLE_GUARD=0\)'
        check "myteam_vehicle off: the load faults where the game does" '\[crash\] unhandled ACCESS_VIOLATION .*Sess_Load_VehicleAiConfig\+0x'
        grep -q 'myteam vehicle off: the load returned' "$OUT/myteam_vehicle.out" && { echo "FAIL: myteam_vehicle off: the load returned"; status=1; } ||
            echo "ok  : myteam_vehicle off: the load does not return"
        [[ $code != 0 ]] && echo "ok  : myteam_vehicle off: process ended with $code" || { echo "FAIL: myteam_vehicle off: process exited 0"; status=1; }
    fi
done
for keep in on off; do
    echo "==== myteam_keep ($keep)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_MYTEAM_VEHICLE_KEEP=$([[ $keep == off ]] && echo 0) SWOFTY_TEST_MYTEAM_KEEP_LABEL=myteam_mes_porsche_2026 \
        SWOFTY_TEST_MYTEAM_KEEP_STOCK=myteam_mercedes_2026:0x02160000:2:412 \
        timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" myteam_keep >"$OUT/myteam_keep.out" 2>&1 || code=$?
    grep -E '\[myteam\] (keep|vehicle keep|save:)|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    grep -E 'myteam keep|FAIL' "$OUT/myteam_keep.out" || true
    if [[ $keep == on ]]; then
        once "myteam_keep on: the keep is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(my team vehicle keep\)': 2 mixin"
        once "myteam_keep on: the keep is armed with the test car" '\[myteam\] vehicle keep armed: .*; test car myteam_mes_porsche_2026'
        once "myteam_keep on: the first test car is kept as the stock placeholder" "\[myteam\] keep: My Team car \(custom vehicle 0\) on 'myteam_mes_porsche_2026' \(engine 22, base car 00a40001\) has no vehicle row .* loads as the stock 'myteam_mercedes_2026' \(engine 2, base car 02160000\)"
        once "myteam_keep on: the second test car is kept too" "\[myteam\] keep: My Team car \(custom vehicle 5\) on 'myteam_mes_porsche_2026' \(engine 24, base car 00a40003\)"
        once "myteam_keep on: the save writes the original car" "\[myteam\] keep: the save writes My Team car \(custom vehicle 0\) as its original 'myteam_mes_porsche_2026' \(engine 22, base car 00a40001\)"
        count "myteam_keep on: no base is skipped" 0 '\[myteam\] save: .* the base is skipped'
        grep -q 'FAIL' "$OUT/myteam_keep.out" && { echo "FAIL: myteam_keep on: a check failed"; status=1; } || echo "ok  : myteam_keep on: the fake game's checks pass"
        grep -q "myteam keep on: base 1 live 'myteam_mercedes_2026' engine 2 base 02160000, saved as 'myteam_mes_porsche_2026' engine 22 base 00a40001" "$OUT/myteam_keep.out" &&
            echo "ok  : myteam_keep on: the live car is the placeholder and the save keeps the original" || { echo "FAIL: myteam_keep on: placeholder or save wrong"; status=1; }
        count "myteam_keep on: no unhandled fault" 0 '\[crash\] unhandled '
        [[ $code == 0 ]] && echo "ok  : myteam_keep on: process exited 0" || { echo "FAIL: myteam_keep on: process ended with $code"; status=1; }
    else
        once "myteam_keep off: the keep is off" '\[myteam\] vehicle keep off \(SWOFTY_MYTEAM_VEHICLE_KEEP=0\)'
        count "myteam_keep off: nothing is kept" 0 '\[myteam\] keep: '
        check "myteam_keep off: the test car is left to the game, which faults" '\[crash\] unhandled ACCESS_VIOLATION .*Sess_Load_VehicleAiConfig\+0x'
        [[ $code != 0 ]] && echo "ok  : myteam_keep off: process ended with $code" || { echo "FAIL: myteam_keep off: process exited 0"; status=1; }
    fi
done
for heal in on off; do
    echo "==== myteam_heal ($heal)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_MYTEAM_VEHICLE_HEAL=$([[ $heal == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" myteam_heal >"$OUT/myteam_heal.out" 2>&1 || code=$?
    grep -E '\[myteam\] (heal|vehicle heal)|\[crash\] (unhandled |crash report [^:]*crash-[0-9])' "$LOG" || true
    grep -E 'myteam heal|FAIL' "$OUT/myteam_heal.out" || true
    if [[ $heal == on ]]; then
        once "myteam_heal on: the heal is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(my team vehicle heal\)': 1 mixin"
        once "myteam_heal on: the heal is armed" '\[myteam\] vehicle heal armed'
        once "myteam_heal on: the rebuilt car is logged once, naming the stock car" "\[myteam\] heal: the player's My Team car \(custom vehicle 2\) was missing from the save .*rebuilt it from the stock My Team car 'myteam_mercedes_2026' \(2026 season\)"
        grep -q 'FAIL' "$OUT/myteam_heal.out" && { echo "FAIL: myteam_heal on: a check failed"; status=1; } || echo "ok  : myteam_heal on: the fake game's checks pass"
        grep -q "myteam heal on: the session entered, 4 classes, 4 entrants classified, 1 rebuilt, custom vehicle 2 is 'myteam_mercedes_2026'" "$OUT/myteam_heal.out" &&
            echo "ok  : myteam_heal on: the session entered with the rebuilt car" || { echo "FAIL: myteam_heal on: the session did not enter"; status=1; }
        count "myteam_heal on: no unhandled fault" 0 '\[crash\] unhandled '
        [[ $code == 0 ]] && echo "ok  : myteam_heal on: process exited 0" || { echo "FAIL: myteam_heal on: process ended with $code"; status=1; }
    else
        once "myteam_heal off: the heal is off" '\[myteam\] vehicle heal off \(SWOFTY_MYTEAM_VEHICLE_HEAL=0\)'
        check "myteam_heal off: the session start faults where the game does" '\[crash\]   reading 0xcea'
        grep -q 'myteam heal off: the session entered' "$OUT/myteam_heal.out" && { echo "FAIL: myteam_heal off: the session entered"; status=1; } ||
            echo "ok  : myteam_heal off: the session does not enter"
        [[ $code != 0 ]] && echo "ok  : myteam_heal off: process ended with $code" || { echo "FAIL: myteam_heal off: process exited 0"; status=1; }
    fi
done
for guard in on off; do
    echo "==== track_list ($guard)"
    code=0
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    SWOFTY_TRACK_LIST_GUARD=$([[ $guard == off ]] && echo 0) timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" track_list >"$OUT/track_list.out" 2>&1 || code=$?
    grep -E '\[saves\] (career calendar|custom_track_list)' "$LOG" || true
    grep -E 'track list|FAIL' "$OUT/track_list.out" || true
    grep -q 'FAIL' "$OUT/track_list.out" && { echo "FAIL: track_list $guard: a check failed"; status=1; } || echo "ok  : track_list $guard: the fake game's checks pass"
    grep -q "track list $guard: cycle 3" "$OUT/track_list.out" && echo "ok  : track_list $guard: the save and load cycles ran" ||
        { echo "FAIL: track_list $guard: the test did not reach its end"; status=1; }
    count "track_list $guard: no unhandled fault" 0 '\[crash\] unhandled '
    [[ $code == 0 ]] && echo "ok  : track_list $guard: process exited 0" || { echo "FAIL: track_list $guard: process ended with $code"; status=1; }
    if [[ $guard == on ]]; then
        once "track_list on: the guard is a core mixin set of its own" "core mixin set 'swofty_core\.dll \(career calendar track list guard\)': 1 mixin"
        once "track_list on: the count lines are a core mixin set of their own" "core mixin set 'swofty_core\.dll \(career calendar track list log\)': 2 mixin"
        once "track_list on: the guard is armed" '\[saves\] career calendar track list guard armed'
        count "track_list on: each load of the 5-event calendar loads 5 events" 6 '\[saves\] custom_track_list loaded \(schedule component 0x[0-9a-f]+\): 5 stored entries -> 5 calendar events \(season event list 5\); 45 more matching events the game would also have loaded skipped[[:space:]]*$'
        count "track_list on: a track twice with one event row loads both races" 2 '\[saves\] custom_track_list loaded \(schedule component 0x[0-9a-f]+\): 4 stored entries -> 4 calendar events \(season event list 4\); 18 more matching events the game would also have loaded skipped; 1 entry repeat an event already in the calendar[[:space:]]*$'
        once "track_list on: a blown-up save loads back to the season's 5 events" '\[saves\] custom_track_list loaded \(schedule component 0x[0-9a-f]+\): 65535 stored entries -> 5 calendar events \(season event list 5\); 45 more matching events the game would also have loaded skipped; blown up \(more than 5 entries\): a run of one id reads as one entry, 65530 stored entries dropped[[:space:]]*$'
        count "track_list on: a save count is logged when it changes" 3 '\[saves\] custom_track_list saved \(schedule component 0x[0-9a-f]+\): (5|4|5) calendar events[[:space:]]*$'
    else
        once "track_list off: the guard is off" '\[saves\] career calendar track list guard off \(SWOFTY_TRACK_LIST_GUARD=0\)'
        once "track_list off: the load line shows the game's own growth" '\[saves\] custom_track_list loaded \(schedule component 0x[0-9a-f]+\): 500 stored entries -> 5000 calendar events[[:space:]]*$'
        count "track_list off: each save with a new count is logged" 3 '\[saves\] custom_track_list saved \(schedule component 0x[0-9a-f]+\): (50|500|5000) calendar events[[:space:]]*$'
    fi
done
echo "==== ntpage"
cp "$OUT/t_ntpage.dll" "$SW/mods/core/ntpage_hook.dll"
rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
code=0
timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" exit >"$OUT/ntpage.out" 2>&1 || code=$?
rm -f "$SW/mods/core/ntpage_hook.dll"
grep -F '[ntpage]' "$OUT/ntpage.out" || true
grep -F '[crash]' "$LOG"
if grep -q '^\[ntpage\] ntdll!NtTerminateProcess now exports' "$OUT/ntpage.out"; then
    echo "ok  : ntpage: the export points at a stub on the page of NtProtectVirtualMemory"
else
    echo "FAIL: ntpage: the injected hook did not move the export"
    status=1
fi
check "ntpage: hooking a function on that ntdll page finishes the crash init" '\[crash\] crash log armed: .*ExitProcess hooked, TerminateProcess hooked'
check "ntpage: NtTerminateProcess is hooked there" '\[crash\] crash reports: .*NtTerminateProcess hooked'
check "ntpage: the exit still gets a report" '\[crash\] crash report .*crash-.*: ExitProcess\(255'
[[ $code == 255 ]] && echo "ok  : ntpage: process ended with 255" || { echo "FAIL: ntpage: process ended with $code"; status=1; }
for skip in 0 1; do
    echo "==== int3$([[ $skip == 1 ]] && echo ', unowned on the writer')"
    cp "$OUT/t_int3hook.dll" "$SW/mods/core/int3_hook.dll"
    rm -f "$SW"/crash-*.txt "$SW"/crash-*.dmp
    code=0
    T_INT3_SKIP_WRITER=$skip timeout 120 "${PIN[@]}" xvfb-run -a wine cmd /c "$OUT/game/F1_25.exe" crash >"$OUT/int3.out" 2>&1 || code=$?
    rm -f "$SW/mods/core/int3_hook.dll"
    grep -F '[int3]' "$OUT/int3.out" || true
    grep -F '[crash]' "$LOG"
    if grep -q '^\[int3\] breakpoints planted on 2 GetModuleFileName' "$OUT/int3.out"; then
        echo "ok  : int3: an injected hook serves int3 breakpoints on GetModuleFileNameW and GetModuleFileNameA"
    else
        echo "FAIL: int3: the injected hook planted no breakpoints"
        status=1
    fi
    check "int3: a report with a minidump is written" '\[crash\] crash report .*crash-.*\.txt: unhandled ACCESS_VIOLATION.*minidump crash-.*\.dmp'
    if [[ $skip == 0 ]]; then
        if grep -qE 'crash report writer (faulted|hit)' "$LOG" || grep -q 'cut short' "$SW"/crash-*.txt; then
            echo "FAIL: int3: the writer took the hook's breakpoint for a fault of its own"
            status=1
        else
            echo "ok  : int3: the writer leaves the hook's breakpoints to the hook"
        fi
        parse_report "Fake_Crash_Here" "Fake_Crash_Here,writing 0x10,exe          Z:" || status=1
    else
        check "int3: the writer logs the unowned breakpoint and goes on" '\[crash\] the crash report writer hit BREAKPOINT \(0x80000003\) at .* in the header of swofty.crash-.*; it goes on with the next step'
        parse_report "Fake_Crash_Here" "[the header is cut short: the writer hit BREAKPOINT (0x80000003) at ,Fake_Crash_Here,writing 0x10" || status=1
    fi
    [[ $code != 0 ]] && echo "ok  : int3: process ended with $code" || { echo "FAIL: int3: process exited 0"; status=1; }
done
echo "==== stages"
LOG=$SG/swofty/loader.log
SWOFTY_EARLY_STAGE=1 "${PIN[@]}" xvfb-run -a wine "$SG/F1_25.exe" stages || status=1
echo "---- $LOG (stages)"
grep -v '  section ' "$LOG"
check "stages: early stage on from the environment" 'early stage: on \(SWOFTY_EARLY_STAGE=1\)'
once "stages: the boot moment runs at the early stage" '\[apply\] boot: [0-9]+ applier\(s\); 0 applied, 0 put back, 0 refused'
once "stages: the room line at the early stage" "\\[registry\\] room: nothing to size"
check "stages: early plugin list read from settings.json" 'early stage: plugin t_stage_early\.dll has "early": true'
check "stages: early trigger fired inside the game's CreateMutexW" "trigger: KERNEL32!CreateMutexW from .* \(\.text\) name 'Codemasters F1 25' on thread .*loading addons now \(early stage\)"
once "stages: the window-stage plugin waits for the window" 'addon t_stage_late\.dll: loads at the window stage'
check "stages: core picks the wine-test bindings set by the fake exe's PE timestamp" '\[core\] bindings: [0-9]+ set\(s\) present, selected bindings\.wine-test\.json by PE timestamp [0-9]+'
check "stages: the fake exe is the analysed build of that set" '\[core\] bindings\.wine-test\.json: [0-9]+ binding\(s\) for build wine-test, this exe \(rva checked\)'
check "stages: core's own wrap still installs with the Steam rvas left out of the set" 'wrap FeHome_Handle_Command at .* installed: swofty_core\.dll\(0\)'
once "stages: the early site on the entry of Fake_Stage_Site installs" '\[core\] site wine-test:site_Stage_Entry at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\) installed: t_stage_early\.dll\(0\)[[:space:]]*$'
once "stages: the early wrap on the entry of Fake_Stage_Wrap installs" '\[core\] wrap wine-test:Fake_Stage_Wrap_Early at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\) installed: t_stage_early\.dll\(0\)[[:space:]]*$'
check "stages: window stage after the early load" 'stage: window \(USER32!RegisterClassExW .*\), addons were loaded at the early stage'
once "stages: the early site binds at its KB address" '\[core\] bind wine-test:site_Stage_Entry -> 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), site experimental via pattern at the KB address'
once "stages: the early wrap binds at its KB address" '\[core\] bind wine-test:Fake_Stage_Wrap_Early -> 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), func experimental via pattern at the KB address'
count "stages: the hooked Fake_Stage_Site is matched under its hook without a scan" 0 'scan \.xtls \[48 B8 53 57 46 54 53 49 54 45 B8 07 00 00 00 C3\]'
count "stages: the hooked Fake_Stage_Wrap is matched under its hook without a scan" 0 'scan \.xtls \[48 B8 53 57 46 54 57 52 41 50 B8 09 00 00 00 C3\]'
count "stages: the early patterns that check out at their KB address are never scanned" 0 'scan \.xtls \[48 B8 53 57 46 54 (53 49 54 45|57 52 41 50)\]'
once "stages: Fake_Stage_Plain's KB address holds other bytes" '\[core\] binding wine-test:Fake_Stage_Plain: the pattern does not match at the KB address 0x[0-9a-f]+; scanning \.xtls'
once "stages: Fake_Stage_Plain is then scanned" 'scan \.xtls \[48 B8 53 57 46 54 50 4C 41 4E B8 05 00 00 00 C3\] -> 1 hit at'
once "stages: Fake_Stage_Plain binds by the scan" '\[core\] bind wine-test:Fake_Stage_Plain -> 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), func experimental via pattern[[:space:]]*$'
for id in Fake_Stage_Wrong Fake_Stage_Site_Wrong; do
    once "stages: wine-test:$id does not match at its KB address" "\[core\] binding wine-test:$id: the pattern does not match at the KB address"
done
count "stages: the ambiguous pattern carries no KB address" 0 'binding wine-test:Fake_Stage_Ambiguous: the pattern does not match at the KB address'
for id in Fake_Stage_Site Fake_Stage_Wrap; do
    rva=$(python3 -c 'import json, sys; print(next(b["rva"] for b in json.load(open(sys.argv[1]))["bindings"] if b["id"] == sys.argv[2]))' \
        "$SG/swofty/addons/core/bindings.wine-test.json" "wine-test:$id" || true)
    addr=$(grep -oE "\[core\] bind wine-test:$id -> 0x[0-9a-f]+ \(base\+$rva\)" "$LOG" | grep -oE '0x[0-9a-f]+' | head -n1 || true)
    once "stages: wine-test:$id binds at its KB rva $rva" "\[core\] bind wine-test:$id -> 0x[0-9a-f]+ \(base\+$rva\), func experimental via pattern"
    once "stages: wine-test:$id binds at its KB address ${addr:-none} at attach" \
        "\[core\] bind wine-test:$id -> ${addr:-none} \(base\+$rva\), func experimental via pattern at the KB address"
done
awk '/bind wine-test:Fake_Stage_Site -> / && !a {a = NR} /bind wine-test:Fake_Stage_Wrap -> / && !b {b = NR} /site wine-test:site_Stage_Entry at .* installed/ && !c {c = NR}
     END {exit !(a && b && a < c && b < c)}' "$LOG" && echo "ok  : stages: every binding resolves at attach, before the early hooks install over its bytes" ||
    { echo "FAIL: stages: every binding resolves at attach, before the early hooks install over its bytes"; status=1; }
once "stages: the init report covers every binding of the set" '\[bind\] summary build=unknown pass=init resolved=[0-9]+ deferred=0 unresolved=[0-9]+ total=[0-9]+ pe_timestamp=[0-9]+ size=[0-9]+ sha256=[0-9a-f]{64}[[:space:]]*$'
once "stages: the report line of a binding found by its KB address" '\[bind\] id=wine-test:Fake_Stage_Site build=unknown pass=init result=resolved method=hint matches=1 rva=0x[0-9a-f]+ match=0x[0-9a-f]+ verified=yes reason="" site_bytes=""[[:space:]]*$'
once "stages: the report line of a binding found by a scan" '\[bind\] id=wine-test:Fake_Stage_Plain build=unknown pass=init result=resolved method=scan matches=1 rva=0x[0-9a-f]+ match=0x[0-9a-f]+ verified=yes'
once "stages: the report line of an ambiguous pattern" '\[bind\] id=wine-test:Fake_Stage_Ambiguous build=unknown pass=init result=unresolved method=- matches=([2-9]|[1-9][0-9]+) rva=- match=- verified=- reason="signature did not match exactly once" site=- site_bytes=""[[:space:]]*$'
once "stages: a hint whose bytes differ is reported with 32 bytes at the hint" '\[bind\] id=wine-test:Fake_Stage_Wrong build=unknown pass=init result=unresolved method=- matches=0 rva=- match=0x[0-9a-f]+ verified=no reason="signature did not match exactly once" site=0x[0-9a-f]+ site_bytes="[0-9A-F]{2}( [0-9A-F]{2}){31}"[[:space:]]*$'
count "stages: a build that is not packed has no window resolution pass" 0 '\[bind\] summary build=unknown pass=window' 
once "stages: the window-stage plugin that binds both hooked functions initialises" 'addon t_stage_late\.dll: SwoftyAddonInit returned 1 \(ok\)'
for id in Fake_Stage_Wrong Fake_Stage_Site_Wrong Fake_Stage_Ambiguous; do
    once "stages: wine-test:$id is refused" "\[core\] binding wine-test:$id: signature did not match exactly once"
    once "stages: t_stage_refused needs wine-test:$id" "\[core\] addon t_stage_refused\.dll needs wine-test:$id, which does not resolve on this build"
done
check "stages: the ambiguous pattern hits more than once" 'scan \.xtls \[48 B8 53 57 46 54\] -> ([2-9]|[1-9][0-9]+) hits'
count "stages: no binding needs the under-hook fallback once all resolve at attach" 0 'its bytes carry an installed hook'
once "stages: the plugin with wrong or ambiguous patterns fails its init" 'addon t_stage_refused\.dll: SwoftyAddonInit returned 0 \(FAILED\)'
once "stages: the exe matches the KB build's timestamp and size, so the KB addresses are used" '\[core\] binding resolution: [0-9]+ of [0-9]+ pattern\(s\) carry their KB address'
echo "==== stages, exe size differs from the KB build"
python3 - "$SG/swofty/addons/core/bindings.wine-test.json" <<'EOF'
import json, sys
doc = json.load(open(sys.argv[1]))
doc['build']['size'] += 1
json.dump(doc, open(sys.argv[1], 'w'), indent=1)
EOF
rm -rf "$SG/swofty/cache"
SWOFTY_EARLY_STAGE=1 "${PIN[@]}" xvfb-run -a wine "$SG/F1_25.exe" stages || status=1
once "stages: an exe of another size than the KB build never uses the KB addresses" '\[core\] binding resolution: the exe is [0-9]+ bytes and the KB build [0-9]+; the KB addresses are not used, every pattern is scanned'
count "stages: with another size, nothing binds at a KB address" 0 'via pattern at the KB address|does not match at the KB address'
once "stages: with another size, the early site is found by a scan" '\[core\] bind wine-test:site_Stage_Entry -> 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\), site experimental via pattern[[:space:]]*$'
# The packed runs share one game folder, so packed_never starts with the scan cache that packed saved.
for mode in packed packed_never packed_plain; do
    echo "==== $mode"
    dir=$PK
    [[ $mode == packed_plain ]] && dir=$PP
    LOG=$dir/swofty/loader.log
    SWOFTY_EARLY_STAGE=1 timeout 120 "${PIN[@]}" xvfb-run -a wine "$dir/F1_25.exe" $mode || status=1
    echo "---- $LOG ($mode)"
    grep -E '\[bind\] summary|\[core\] (build|bindings|.* pass:|.* waits:|.*setting it up now|.*fails closed|scan .*refused|addon .*(dropped|runs again|ran again))|\[crash\] Ne_Report|stage: window|trigger: KERNEL32' "$LOG" || true
    python3 packed_game.py check $mode "$LOG" "$dir/F1_25.exe" || status=1
    once "$mode: the set is selected by the exe's PE timestamp" '\[core\] bindings: [0-9]+ set\(s\) present, selected bindings\.packed-test\.json by PE timestamp [0-9]+'
    check "$mode: the core loads at the early trigger" "trigger: KERNEL32!CreateMutexW from .* \(\.text\) name 'Codemasters F1 25' .*loading addons now \(early stage\)"
    case $mode in
    packed | packed_never)
        once "$mode: the set marks the build packed and .xtls reads encrypted at init" '\[core\] build ea: packed \(the set says so\); code section \.xtls entropy 7\.[5-9][0-9], reads encrypted'
        once "$mode: a binding in .xtls is deferred at init" '\[bind\] id=FeHome_Handle_Command build=ea pass=init result=deferred method=- matches=- rva=- match=- verified=- reason="code section \.xtls reads encrypted \(entropy 7\.[5-9][0-9]\)" site_bytes=""[[:space:]]*$'
        once "$mode: a hinted binding in .xtls is deferred at init" '\[bind\] id=wine-test:Fake_Stage_Wrap build=ea pass=init result=deferred method=- matches=- rva=- match=- verified=- reason="code section \.xtls reads encrypted'
        once "$mode: the runtime setup waits for the code to decrypt" '\[core\] runtime setup \(assets, .*\) waits: \.xtls reads encrypted \(entropy 7\.[5-9][0-9]\) on this packed build'
        once "$mode: the crash heap-error hook waits for the code to decrypt" '\[core\] crash: Ne_ReportMemoryError hook waits: \.xtls reads encrypted'
        once "$mode: the popup hook waits for the code to decrypt" '\[core\] popup: message system hook waits: \.xtls reads encrypted'
        check "$mode: an early addon's own scan of encrypted code is refused" '\[core\] scan \.xtls \[48 B8 53 57 46 54 50 4C 41 4E B8 05 00 00 00 C3\] for t_packed_scan\.dll refused: \.xtls reads encrypted \(entropy 7\.[5-9][0-9]\) on this packed build; nothing resolves in encrypted code'
        once "$mode: that addon is called again once the code decrypts" '\[core\] addon t_packed_scan\.dll: its init needs code that still reads encrypted \(a binding or a scan\); SwoftyAddonInit runs again once that code decrypts'
        once "$mode: the window stage runs the core's pass" 'stage: window \(USER32!RegisterClassExW .*\), addons were loaded at the early stage'
        ;;
    esac
    case $mode in
    packed)
        once "packed: the window pass reports every binding" '\[bind\] summary build=ea pass=window resolved=[1-9][0-9]* deferred=0 unresolved=[0-9]+ total=[0-9]+ pe_timestamp=[0-9]+ size=[0-9]+ sha256=[0-9a-f]{64}[[:space:]]*$'
        once "packed: a binding found by a scan at the window pass" '\[bind\] id=FeHome_Handle_Command build=ea pass=window result=resolved method=scan matches=1 rva=0x[0-9a-f]+ match=0x[0-9a-f]+ verified=yes reason="" site_bytes=""[[:space:]]*$'
        once "packed: a binding found by its hint at the window pass" '\[bind\] id=wine-test:Fake_Stage_Wrap build=ea pass=window result=resolved method=hint matches=1 rva=0x[0-9a-f]+ match=0x[0-9a-f]+ verified=yes reason="" site_bytes=""[[:space:]]*$'
        once "packed: t_packed's wrap installs at the window pass" '\[core\] wrap wine-test:Fake_Stage_Wrap at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\) installed: t_packed\.dll\(0\)'
        once "packed: the early addon's scan is refused only while the code reads encrypted" 'refused: \.xtls reads encrypted'
        once "packed: t_packed_scan's second init succeeds" 'addon t_packed_scan\.dll: SwoftyAddonInit ran again now that its code decrypted and returned 1'
        once "packed: the runtime setup runs at the window pass" '\[core\] runtime setup \(assets, .*\): \.xtls reads decrypted \(entropy [0-6]\.[0-9]+\) at the window pass; setting it up now'
        once "packed: the database reader set up at the window pass still gets the window stage" '\[dbread\] window stage: '
        once "packed: the crash heap-error hook installs at the window pass" '\[crash\] Ne_ReportMemoryError at base\+0x[0-9a-f]+ hooked \(found by its prologue\)'
        count "packed: nothing fails closed once the code decrypts" 0 'fails closed'
        awk '/stage: window \(/ && !w {w = NR} /\[lang\] override: / && !l {l = NR} END {exit !(w && l > w)}' "$LOG" &&
            echo "ok  : packed: a lang call at the early stage leaves the string override setup to the decrypted code" ||
            { echo "FAIL: packed: a lang call at the early stage leaves the string override setup to the decrypted code"; status=1; }
        count "packed: no binding waits past the window pass" 0 'pass=stage:'
        ;;
    packed_never)
        once "packed_never: the window pass still defers" '\[bind\] summary build=ea pass=window resolved=[0-9]+ deferred=[1-9][0-9]* unresolved=[0-9]+ total=[0-9]+'
        once "packed_never: the retries start" '\[core\] bindings: [1-9][0-9]* binding\(s\) and [1-9] setup\(s\) still wait for their code to decrypt; retrying every 1000 ms, 8 time\(s\), then they fail closed'
        once "packed_never: the last retry reports nothing deferred" '\[bind\] summary build=ea pass=stage:8 resolved=[0-9]+ deferred=0 unresolved=[0-9]+ total=[0-9]+'
        once "packed_never: a scanned binding fails closed with its reason" '\[bind\] id=FeHome_Handle_Command build=ea pass=stage:8 result=unresolved method=- matches=- rva=- match=- verified=- reason="code section \.xtls reads encrypted \(entropy 7\.[5-9][0-9]\); still so at the last retry, so it fails closed" site=- site_bytes=""[[:space:]]*$'
        once "packed_never: a hinted binding fails closed with 32 bytes at its hint" '\[bind\] id=wine-test:Fake_Stage_Wrap build=ea pass=stage:8 result=unresolved method=- matches=- rva=- match=- verified=- reason="code section \.xtls reads encrypted \(entropy 7\.[5-9][0-9]\); still so at the last retry, so it fails closed" site=0x[0-9a-f]+ site_bytes="[0-9A-F]{2}( [0-9A-F]{2}){31}"'
        once "packed_never: the runtime setup fails closed" '\[core\] runtime setup \(assets, .*\): not set up; \.xtls still reads encrypted \(entropy 7\.[5-9][0-9]\) at the last retry, so it fails closed'
        once "packed_never: the crash heap-error hook fails closed" '\[core\] crash: Ne_ReportMemoryError hook: not set up; \.xtls still reads encrypted'
        once "packed_never: the popup hook fails closed" '\[core\] popup: message system hook: not set up; \.xtls still reads encrypted'
        count "packed_never: the heap-error hook is never installed" 0 '\[crash\] Ne_ReportMemoryError at'
        count "packed_never: the database reader is never set up" 0 '\[dbread\] window stage: '
        once "packed_never: the addon whose wrap never decrypts is dropped" '\[core\] addon t_packed\.dll dropped: needs wine-test:Fake_Stage_Wrap, which does not resolve on this build'
        once "packed_never: the addon whose scan never decrypts is dropped" '\[core\] addon t_packed_scan\.dll dropped: SwoftyAddonInit returned 0 once its code still read encrypted at the last retry'
        count "packed_never: nothing installs" 0 '\[core\] [a-z]+ .* installed: '
        ;;
    packed_plain)
        once "packed_plain: the set says the build is not packed" '\[core\] build steam: not packed \(the set says so\); code section \.xtls entropy [0-6]\.[0-9]+, reads decrypted'
        count "packed_plain: nothing is deferred" 0 'result=deferred'
        count "packed_plain: no setup waits" 0 '\[core\] [a-z]+.* waits: \.xtls'
        count "packed_plain: no scan is refused" 0 'refused: .* on this packed build'
        count "packed_plain: there is no window resolution pass" 0 '\[bind\] summary build=steam pass=window'
        once "packed_plain: the init pass reports every binding" '\[bind\] summary build=steam pass=init resolved=[1-9][0-9]* deferred=0 unresolved=[0-9]+ total=[0-9]+'
        once "packed_plain: a hinted binding resolves at init" '\[bind\] id=wine-test:Fake_Stage_Wrap build=steam pass=init result=resolved method=hint matches=1'
        once "packed_plain: the database reader gets the window stage" '\[dbread\] window stage: '
        once "packed_plain: the crash heap-error hook installs at attach" '\[crash\] Ne_ReportMemoryError at base\+0x[0-9a-f]+ hooked \(found by its prologue\)'
        once "packed_plain: t_packed's wrap installs at the early commit" '\[core\] wrap wine-test:Fake_Stage_Wrap at 0x[0-9a-f]+ \(base\+0x[0-9a-f]+\) installed: t_packed\.dll\(0\)'
        ;;
    esac
done
echo "==== filled"
LOG=$OUT/filled/swofty/loader.log
SWOFTY_EARLY_STAGE=1 "${PIN[@]}" xvfb-run -a wine "$OUT/filled/F1_25.exe" || status=1
echo "---- $LOG (filled)"
grep -v '  section ' "$LOG"
check "filled: the exe's import table lacks the early slot" "trigger: game IAT slot KERNEL32!CreateMutexW not found or not writable; the exe's import table is filled at load"
once "filled: a breakpoint on the entry of KERNEL32!CreateMutexW" 'trigger armed: breakpoint on the entry of KERNEL32!CreateMutexW at [0-9A-Fa-f]+, early stage'
once "filled: a breakpoint on the entry of KERNELBASE!CreateMutexW" 'trigger armed: breakpoint on the entry of KERNELBASE!CreateMutexW at [0-9A-Fa-f]+, early stage'
count "filled: another mutex skips the early trigger" 2 "early trigger skipped: KERNEL32!CreateMutexW from [0-9A-Fa-f]+ \(\.text\) name 'swofty filled-iat test'"
once "filled: the early trigger fires at the breakpoint" "trigger: KERNEL32!CreateMutexW from [0-9A-Fa-f]+ \(\.text\) name 'Codemasters F1 25' on thread .*loading addons now \(early stage\)"
count "filled: both entries are restored" 2 'trigger disarmed: KERNEL(32|BASE)!CreateMutexW entry restored'
once "filled: RegisterClassExW is found in the filled table" 'trigger armed: game IAT slot USER32!RegisterClassExW at [0-9A-Fa-f]+ \(was [0-9A-Fa-f]+\), in the import table the exe filled at load'
once "filled: CreateWindowExW is found in the filled table" 'trigger armed: game IAT slot USER32!CreateWindowExW at [0-9A-Fa-f]+ \(was [0-9A-Fa-f]+\), in the import table the exe filled at load'
once "filled: the window stage runs at RegisterClassExW" 'stage: window \(USER32!RegisterClassExW from [0-9A-Fa-f]+ \(\.text\)\), addons were loaded at the early stage'
count "filled: the fallback watcher leaves the window stage to the armed slots" 0 'waiting for a visible main window for the window stage'
echo "==== gated"
GT=$OUT/gated
rm -rf "$GT"
cp -r "$OUT/filled" "$GT"
rm -f "$GT/swofty/loader.log"
truncate -s 72M "$GT/F1_25.exe"
LOG=$GT/swofty/loader.log
env -u SWOFTY_ALLOW_UNSUPPORTED_BUILD SWOFTY_EARLY_STAGE=1 "${PIN[@]}" xvfb-run -a wine "$GT/F1_25.exe" || :
echo "---- $LOG (gated)"
grep -v '  section ' "$LOG"
once "gated: a large exe the table does not list is refused" 'exe: size [0-9]+, PE timestamp [0-9]+ is not a build this launcher supports; the core and the addons are NOT loaded'
check "gated: the trigger fires and loads nothing" 'addons NOT loaded, this exe is not a supported build'
count "gated: no addon is loaded" 0 'loading addons now'
count "gated: the core never attaches" 0 'provides the mixin API'
rm -f "$LOG"
SWOFTY_ALLOW_UNSUPPORTED_BUILD=1 SWOFTY_EARLY_STAGE=1 "${PIN[@]}" xvfb-run -a wine "$GT/F1_25.exe" || status=1
echo "---- $LOG (gated, override)"
grep -v '  section ' "$LOG"
count "gated: the override leaves the exe unrefused" 0 'is not a build this launcher supports'
once "gated: the override loads the addons" "trigger: KERNEL32!CreateMutexW from [0-9A-Fa-f]+ \\(\\.text\\) name 'Codemasters F1 25' on thread .*loading addons now \\(early stage\\)"
echo "==== register"
SR=$OUT/register/game
rm -rf "$SR"
cp -r "$OUT/game" "$SR"
rm -f "$SR"/swofty/addons/t_*.dll "$SR"/swofty/mods/t_*.dll "$SR/swofty/loader.log"
cp "$OUT/register/t_register.dll" "$SR/swofty/addons/"
python3 - "$SR/swofty" <<'EOF'
import json, os, sys
root = sys.argv[1]
json.dump({'schema': 1, 'profile': 'wine-test', 'addons': [{'id': 't.register', 'dlls': ['t_register.dll'], 'options': []}]},
          open(root + '/settings.json', 'w'))
os.makedirs(root + '/registry', exist_ok=True)
os.makedirs(root + '/addons/wine.data/registry', exist_ok=True)
json.dump({'version': 1, 'profile': 'wine-test', 'order': ['t.register', 'wine.data'],
           'data': {'wine.data': ['addons/wine.data/registry/records.json']}}, open(root + '/registry/index.json', 'w'))
json.dump({'wine.thing': {'extra': {'name': 'extra', 'n': 7}}}, open(root + '/addons/wine.data/registry/records.json', 'w'))
EOF
LOG=$SR/swofty/loader.log
env -u SWOFTY_EARLY_STAGE "${PIN[@]}" xvfb-run -a wine "$SR/F1_25.exe" register || status=1
echo "---- $LOG (register)"
grep -v '  section ' "$LOG"
check "register: a dll that exports SwoftyAddonRegister turns the early stage on" 'early stage: on for 1 addon dll\(s\) that export SwoftyAddonRegister'
check "register: SwoftyAddonRegister is called at the early stage" 'addon t_register\.dll: calling SwoftyAddonRegister \(API v9\) at the early stage'
check "register: index.json's data file waits for its registry and lands on declare" '\[registry\] wine\.thing: declared; applying 1 call'
check "register: the code record and a data-only addon's record read back" '\[t_register\] on_load: t\.register/one n=1, wine\.data/extra n=7'
awk '/addon t_register\.dll: calling SwoftyAddonRegister/ && !a {a = NR} /\[t_register\] register: SWOFTY_REGISTER runs/ && !b {b = NR}
     /fake game: the game database loads now/ && !c {c = NR} /addon t_register\.dll: loaded at .*, calling SwoftyAddonInit/ && !d {d = NR}
     END {exit !(a && b && c && d && a < b && b < c && c < d)}' "$LOG" &&
    echo "ok  : register: the register pass, then the database load, then SwoftyAddonInit" ||
    { echo "FAIL: register: the register pass, then the database load, then SwoftyAddonInit"; status=1; }
exit $status
