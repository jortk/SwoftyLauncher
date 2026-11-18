#include "apply/moments.hpp"
#include "gp_weekend_track_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/game.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstddef>
#include <cstdint>
#include <mutex>
#include <set>
#include <string>
#include <utility>
#include <vector>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Weekend track guard (re/db/globals.tsv site_Game_TyreAllocationComponent_PresetTrackRead and
// site_Game_ChampionshipGameMode_SessionTrackRead).
//
// A championship weekend (Grand Prix, Driver Career, My Team) keeps its track as a stat id (Weekend+0x278) that indexes
// the TrackDataSystem item vector +0x28..+0x30 (16 bytes per stat id, TrackDataRecord at +8). The item is past the
// vector or null when the track's addon is off or gone: a Grand Prix remembers the track it last raced, and a career
// calendar keeps a round on an added track (an imported track in a dormant stock slot such as suzuka24, stat id 36,
// which has no track data of its own) after its addon is removed. Two readers of the weekend's track do not survive
// that:
//
// Entering Grand Prix (the activity container's solo_gp, from the F1 World hub or the launcher's Grand Prix card) runs
// Game_Game_SetCurrentGameMode -> Game_GameModeChampionship_Begin -> Game_TyreAllocationComponent_OnStartWeekend ->
// Game_TyreAllocationComponent_SetPresetChoice for the mode's current weekend before any screen shows. SetPresetChoice
// takes the weekend from [component+0x270] vt+0x98 (r9, a Career Weekend), reads its track stat id and indexes the
// career's TrackDataSystem (component+0x278 -> CareerEntity+0xf58) with it. A stat id past the vector or on a null item
// calls Ne_FatalAbort: EA bundles of 2026-11-12, track 47 against 43 slots, and against 84 slots with item 47 null.
//
// The session load runs Game_ChampionshipGameMode_BuildSessionSettings (the championship modes' slot, called from
// Sess_Loader_SetupTrackAssets). It takes the current weekend into rdi, reads its track stat id and the record from the
// mode's TrackDataSystem ([r13+0x1448]) into r12, 0 when the item is past the vector or null, and passes it to
// 0x1407a1310 (KB build), whose loop builds the track's FILENAME from [record+0x78] with no check: ACCESS_VIOLATION
// reading 0x78 at F1_25.exe+0x2e5a48b (Steam 1.0.145) on every load of that weekend's session. Bundles of
// 2026-11-17/18, launcher 1.9.67 to 1.9.73: Driver Career Continue of a round on stat id 36 (suzuka24) after the
// imported track that filled the slot was uninstalled.
//
// The sites are the movzx of the track byte (tyre allocation: r9 = the weekend, r14 = the component; session settings:
// rdi = the weekend, rdx = the TrackDataSystem just loaded). When the item for the weekend's track is missing, the
// guard writes the stat id of the melbourne record (else the first record) into the weekend, the same repair
// apply/moments.cpp makes for a Grand Prix at the session load, so the game's own code reads a track it has. The repair
// is in every championship mode: the game cannot load the weekend's session on a track without a record, and the save
// keeps the replacement. Each weekend and track is logged once.
//
// The sites are found by byte scan (the KB rows are L1: subsystem game is owned by re-career-teams). The tyre
// allocation pattern matches once on the Steam exe (.xtls) and on the EA 1.0.141 dump (.rdata, 0x140bca48f); the
// session settings pattern matches once on Steam 1.0.141 (0x1407b7dd6) and 1.0.145 (0x1407b7e16) and on the EA 1.0.141
// (0x1407b6ee6) and 1.0.145 (0x1407b6f26) dumps.
//
// SWOFTY_GP_WEEKEND_TRACK_GUARD=0 turns both off.

namespace {

namespace mem = swofty::mem;
namespace g = swofty::game;
namespace wt = runtime::gp_weekend_track;

const runtime::Sig S_PRESET_TRACK_READ{"site_Game_TyreAllocationComponent_PresetTrackRead",
                                      "4C 89 4C 24 40 41 0F B6 81 78 02 00 00", 0xbcbb1f, runtime::Sig::addr, 0, 5, 0};
const runtime::Sig S_SESSION_TRACK_READ{
    "site_Game_ChampionshipGameMode_SessionTrackRead",
    "49 8B 95 48 14 00 00 0F B6 87 78 02 00 00 48 8B 4A 28 48 8B 52 30 48 29 CA 48 C1 FA 04 48 39 C2 48 89 7C 24 70",
    0x7b7dd6, runtime::Sig::addr, 0, 7, 0};

constexpr uintptr_t COMPONENT_CAREER = 0x278;
constexpr uintptr_t CAREER_TRACK_DATA = 0xf58;
constexpr uintptr_t TRACK_DATA_ITEMS_BEGIN = 0x28;
constexpr uintptr_t TRACK_DATA_ITEMS_END = 0x30;
constexpr uintptr_t ITEM_STRIDE = 0x10;
constexpr uintptr_t ITEM_RECORD = 0x8;
constexpr uintptr_t RECORD_NAME = 0xd0;

uintptr_t weekend_vtable = 0;
safetyhook::MidHook h_track, h_session;
std::mutex seen_lock;
std::set<std::pair<uintptr_t, int>> seen;

bool item_vector(uintptr_t data, uintptr_t &begin, size_t &count)
{
    uintptr_t end = 0;
    if (!mem::object_like(data) || !mem::read(data + TRACK_DATA_ITEMS_BEGIN, begin) ||
        !mem::read(data + TRACK_DATA_ITEMS_END, end) || end < begin || (end - begin) % ITEM_STRIDE ||
        (end - begin) / ITEM_STRIDE > wt::MAX_ITEMS)
        return false;
    count = (end - begin) / ITEM_STRIDE;
    return true;
}

uintptr_t career_track_data(uintptr_t component)
{
    uintptr_t career = 0, data = 0;
    if (!mem::read(component + COMPONENT_CAREER, career) || !mem::object_like(career) ||
        !mem::read(career + CAREER_TRACK_DATA, data))
        return 0;
    return data;
}

bool has_record(uintptr_t begin, size_t count, size_t i)
{
    uintptr_t record = 0;
    return i < count && mem::read(begin + i * ITEM_STRIDE + ITEM_RECORD, record) && record;
}

std::vector<wt::Item> read_items(uintptr_t begin, size_t count)
{
    std::vector<wt::Item> out(count);
    for (size_t i = 0; i < count; ++i) {
        uintptr_t record = 0, name = 0;
        if (!mem::read(begin + i * ITEM_STRIDE + ITEM_RECORD, record) || !record)
            continue;
        out[i].present = true;
        if (mem::read(record + RECORD_NAME, name) && name)
            mem::c_string(name, out[i].name, 64);
    }
    return out;
}

void guard(uintptr_t weekend, uintptr_t data, wt::Site site)
{
    uintptr_t vt = 0, begin = 0;
    size_t count = 0;
    uint8_t track = 0;
    if (!weekend_vtable || !mem::read(weekend, vt) || vt != weekend_vtable ||
        !mem::read(weekend + offsetof(g::Weekend, track_stat_id), track) || !item_vector(data, begin, count) ||
        has_record(begin, count, track))
        return;
    std::vector<wt::Item> items = read_items(begin, count);
    if (wt::present(track, items))
        return;
    wt::Decision d = wt::decide(track, items);
    if (d.repair)
        *reinterpret_cast<uint8_t *>(weekend + offsetof(g::Weekend, track_stat_id)) = uint8_t(d.to);
    {
        std::lock_guard<std::mutex> lock(seen_lock);
        if (!seen.insert({weekend, track}).second || seen.size() > size_t(wt::LOG_CAP))
            return;
    }
    std::string mode = swofty_core::apply::moments::current_mode(wt::reader(site));
    std::string label = swofty_core::apply::moments::cached_track_label(track);
    swofty::log("[gp] %s",
                wt::line(d, track, items.size(), mode, site, label, d.repair ? items[size_t(d.to)].name : "").c_str());
}

void track_read(safetyhook::Context &c)
{
    guard(c.r9, career_track_data(c.r14), wt::Site::tyre_allocation);
}

void session_track_read(safetyhook::Context &c)
{
    guard(c.rdi, c.rdx, wt::Site::session_settings);
}

bool arm(const runtime::Sig &sig, safetyhook::MidHook &into, safetyhook::MidHookFn fn)
{
    uintptr_t at = runtime::find(sig, "gp", "gp");
    if (!at) {
        swofty::log("[gp] weekend track guard: %s is not found on this build", sig.what);
        return false;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[gp] weekend track guard: %s: the hook failed: %s", sig.what,
                    runtime::hook_error(h.error()).c_str());
        return false;
    }
    into = std::move(*h);
    return true;
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_GP_WEEKEND_TRACK_GUARD", v, sizeof v);
    if (n && n < sizeof v && wt::switch_off(v)) {
        swofty::log("[gp] weekend track guard off (SWOFTY_GP_WEEKEND_TRACK_GUARD=%s)", v);
        return;
    }
    weekend_vtable = swofty::api ? swofty::api->resolve("vt_Career_Weekend") : 0;
    if (!weekend_vtable) {
        swofty::log("[gp] weekend track guard off: vt_Career_Weekend does not resolve on this build");
        return;
    }
    bool tyres = arm(S_PRESET_TRACK_READ, h_track, track_read);
    bool session = arm(S_SESSION_TRACK_READ, h_session, session_track_read);
    if (!tyres && !session) {
        swofty::log("[gp] weekend track guard off: neither site is found on this build");
        return;
    }
    swofty::log("[gp] weekend track guard armed%s: a Grand Prix, Driver Career or My Team weekend whose track has no "
                "track data record races melbourne instead of %s",
                tyres && session ? "" : tyres ? " at the tyre allocation only" : " at the session settings only",
                tyres && session ? "aborting in its tyre allocation or faulting in its session settings"
                : tyres          ? "aborting in its tyre allocation"
                                 : "faulting in its session settings");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("weekend track guard", setup);
}
