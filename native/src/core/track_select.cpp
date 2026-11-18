#include "runtime.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstring>
#include <mutex>
#include <string>

#include "apply/tracks/box.hpp"

// The track select guard (work/std16/TRACKS-PLAN.md R1; re/db/subsystems/tracks.md#select, verify V1625-V1637).
// The Grand Prix track select (HomeSoloTrackSelectViewModel::onCommandRaised 0x141c06dc0, advance_pressed
// 0xfc8f026c) and the Time Trial one (TrackSelectViewModel::onCommandRaised 0x141f28410, select_pressed 0x2b0357a)
// are wrapped at their function entries (func_start signatures), which also covers the Grand Prix adjustor thunk.
// On a confirm the guard asks the core's decision (apply/tracks: the active box, the track base, the facts) for the
// track the game last looked up in its track data (site_Trk_TrackData_SetMatched, rdx = the TrackDataRecord: stat
// id +0x01, name +0xd0) after the screen's last selected_track_changed, which the screen's contract does for the
// selected track (Game_BaseGameModeSetupDataContract_UpdateSelectedTrackContextual 0x143852b40). A track that
// doesn't fit the box shows the popup of docs/std/systems/tracks.md and the confirm returns without calling the
// game, which leaves no state (the handlers write nothing before their confirm tests). No lookup since the last
// selection change, or no active box: the confirm runs and one line says why. SWOFTY_TRACK_GUARD=0 or off turns
// the guard off.

namespace {

namespace trk = swofty_core::apply::tracks;
namespace mem = swofty::mem;

constexpr const char *MIXIN = "swofty_core.dll (track select guard)";
constexpr const char *GP = "Trk_GrandPrixTrackSelectVm_OnCommandRaised";
constexpr const char *TT = "Trk_TimeTrialTrackSelectVm_OnCommandRaised";
constexpr const char *SITE = "site_Trk_TrackData_SetMatched";
constexpr uint32_t ADVANCE_PRESSED = 0xfc8f026cu, SELECT_PRESSED = 0x2b0357au, SELECTED_CHANGED = 0xdcd54bcbu;
constexpr uintptr_t RECORD_STAT_ID = 0x01, RECORD_NAME = 0xd0;
constexpr uint32_t LOG_FIRST = 8;

using Handler = void *(void *vm, uint32_t **cmd);

Handler *gp_orig, *tt_orig;
std::atomic<uintptr_t> last_record{0};
std::atomic<uint64_t> counter{0}, record_seq{0}, select_seq{0};
std::atomic<bool> offered, wanted, settled, live;
std::atomic<uint32_t> logged{0};
std::mutex confirmed_lock;
std::string confirmed_time_trial;

void site_matched(SwoftyContext *c)
{
    last_record.store(c->rdx, std::memory_order_relaxed);
    record_seq.store(++counter, std::memory_order_relaxed);
}

void note(const std::string &line)
{
    if (logged++ < LOG_FIRST * 4)
        swofty::log("%s", line.c_str());
}

bool refused(bool time_trial)
{
    uintptr_t rec = last_record.load(std::memory_order_relaxed);
    std::string name;
    uint8_t stat = 0xff;
    if (!rec || record_seq.load() <= select_seq.load() || !mem::read(rec + RECORD_STAT_ID, stat)) {
        note(std::string("[tracks] guard: ") + (time_trial ? "Time Trial" : "Grand Prix") +
             " confirm with no track looked up since the last selection; allowed");
        return false;
    }
    uintptr_t at = 0;
    if (!mem::read(rec + RECORD_NAME, at) || !mem::c_string(at, name, 64))
        name.clear();
    if (time_trial) {
        std::lock_guard<std::mutex> lk(confirmed_lock);
        confirmed_time_trial = name;
    }
    trk::Active a = trk::active();
    if (!a.set) {
        note("[tracks] guard: " + name + " confirmed with no box picked; allowed");
        return false;
    }
    const trk::Base &b = trk::base();
    const trk::BaseTrack *track = b.ready ? b.by_code(name) : nullptr;
    if (!track && b.ready)
        track = b.by_stat_id(stat);
    trk::Decision d = trk::guard_package(a.package, track, time_trial ? trk::Mode::time_trial : trk::Mode::grand_prix,
                                         track ? std::string_view(track->name) : std::string_view(name));
    note(d.line);
    if (d.ok)
        return false;
    runtime::popup_show(d.title, d.body, "swofty.core");
    return true;
}

void *handle(Handler *orig, void *vm, uint32_t **cmd, uint32_t confirm, bool time_trial)
{
    uint32_t hash = 0;
    uintptr_t p = 0;
    if (cmd && mem::read(reinterpret_cast<uintptr_t>(cmd), p) && p && mem::read(p, hash)) {
        if (hash == SELECTED_CHANGED)
            select_seq.store(++counter);
        else if (hash == confirm && live && refused(time_trial))
            return nullptr;
    }
    return orig(vm, cmd);
}

void *gp_entry(void *vm, uint32_t **cmd)
{
    return handle(gp_orig, vm, cmd, ADVANCE_PRESSED, false);
}

void *tt_entry(void *vm, uint32_t **cmd)
{
    return handle(tt_orig, vm, cmd, SELECT_PRESSED, true);
}

}

namespace runtime {

std::vector<CoreMixins> track_select_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_TRACK_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[tracks] track select guard off (SWOFTY_TRACK_GUARD=%s)", v);
        return out;
    }
    for (const char *id : {GP, TT, SITE})
        if (!swofty::api->resolve(id)) {
            swofty::log("[tracks] track select guard off: needs %s, which this build does not resolve", id);
            return out;
        }
    wanted = true;
    out.push_back({MIXIN,
                   {{GP, SWOFTY_MIXIN_WRAP, 100, (void *)&gp_entry, (void **)&gp_orig},
                    {TT, SWOFTY_MIXIN_WRAP, 100, (void *)&tt_entry, (void **)&tt_orig},
                    {SITE, SWOFTY_MIXIN_SITE, 100, (void *)&site_matched, nullptr}}});
    return out;
}

void track_select_commit()
{
    if (!wanted || settled)
        return;
    settled = true;
    live = core_mixins_active(MIXIN);
    swofty::log("[tracks] track select guard %s",
                live ? "armed: a Grand Prix or Time Trial confirm of a track the box can't run shows why and stays on the "
                       "screen"
                     : "did not install (see the [core] lines)");
}

std::string track_select_time_trial()
{
    std::lock_guard<std::mutex> lk(confirmed_lock);
    return confirmed_time_trial;
}

}
