#include "runtime.hpp"

#include <windows.h>

#include "track_list_guard_logic.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <mutex>
#include <string>
#include <vector>

// Career calendar track list guard (re/db/functions.tsv Career_ScheduleComponent_Load, Career_ScheduleComponent_Save).
//
// A career's schedule component (vtable 0x1456cdc80, the ScheduleComponent derived from vt_Game_TrackListComponent)
// keeps its calendar at +0x2a8: a vector of 0x28-byte event entries (begin +0x2b0, end +0x2b8) whose u8 event stat id
// sits at +8. Career_ScheduleComponent_Save writes it to the save as the array custom_track_list, one child per entry
// holding only that byte (track_event_id). Career_ScheduleComponent_Load clears the calendar and, for every stored
// byte, walks the career's event list ([[career+0xf58]+0x138], 0x28-byte rows) and pushes every row whose id equals the
// byte. The stock game has one event per id, so the calendar comes back as it was saved. Addon seasons and tracks add
// events that share ids (one per season at the same track), so each load pushes every event with the id once per
// stored entry, the next save writes all of them, and the calendar multiplies on every save and load. The save then
// asks SAVE_PROPERTY_BAG_TEMP for a child per entry until the pool is full (Ne_TempPool_ReportError <
// Ne_TempPool_Alloc < Ne_SaveObject_ReserveChildSlot from F1_25.exe+0x1272188), and the u16 child count wraps.
//
// site_Career_ScheduleComponent_TrackListPush is the push (rsi = the component, rbx = the matching event row, rbp =
// the event list's end, r13b = the stored id, r15 = the stored entry, rdi = the stored array). At an entry's first
// match the guard picks the one row that entry loads: the first row with its id that this load has not pushed yet, or
// the first match again when every such row is in the calendar (a track twice in one season with one event row). It
// sets rbx to that row and lets the push run; every other match of the entry continues the walk at
// site_Career_ScheduleComponent_TrackListNextRow, as a row with another id does. So each stored entry loads exactly
// one event and the calendar keeps its length across saves and loads.
//
// A save the game already blew up holds far more entries than the season has events. The bound is the larger of two
// counts. One is the event list length the component keeps at +0x2c8 (written by vt_Game_TrackListComponent slot 16
// from the default track list), which every schedule component of a loaded career holds as the full default list's
// length (24 in a 2026 career at the custom_track_list load, also for 16- and 10-race calendars; 2026-11-13 in game).
// The other is the race weekends the career already runs on: the RaceWeekend activities (ActivityDef type 0x0a) on the
// busiest driver calendar of the component's current season record (+0x428 into the 0x80-byte records at +0x410),
// which Career_ScheduleComponent_Load reads from the same save before custom_track_list, trusted when it is 1..64. The
// career clock stops at those activities and races Season.weekends[current_weekend] at each, and the daily tick
// (0x14126a1a0) refills them from this calendar, so a calendar cut below them leaves the season's last rounds without
// a race weekend and the season never completes (a 32-round season cut to 24 at a 2026-11-13 load). With neither
// count the bound is 64. Past the bound, each run of one id (one original entry, multiplied) reads as one entry and
// entries past the bound are dropped, so the calendar comes back to the season's length.
// site_Career_ScheduleComponent_TrackListEntryDone (after each stored entry) logs the load once its last entry is
// done, and site_Career_ScheduleComponent_TrackListSave logs the count a save writes when it changed for that component.
//
// SWOFTY_TRACK_LIST_GUARD=0 turns the guard off; the count lines stay.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (career calendar track list guard)";
constexpr const char *LOG_MIXIN = "swofty_core.dll (career calendar track list log)";
constexpr const char *PUSH = "site_Career_ScheduleComponent_TrackListPush";
constexpr const char *NEXT_ROW = "site_Career_ScheduleComponent_TrackListNextRow";
constexpr const char *ENTRY_DONE = "site_Career_ScheduleComponent_TrackListEntryDone";
constexpr const char *SAVE = "site_Career_ScheduleComponent_TrackListSave";

namespace tl = runtime::track_list_guard;

uintptr_t next_row = 0;
bool offered = false, settled = false, guarded = false, logging = false;
std::mutex lock;
tl::Load load;
std::vector<uintptr_t> matches;
tl::SaveLog saves;

size_t calendar_size(uintptr_t component)
{
    uint64_t begin = 0, end = 0;
    if (!swofty::mem::read(component + tl::CALENDAR_BEGIN, begin) || !swofty::mem::read(component + tl::CALENDAR_END, end))
        return 0;
    return tl::entries(begin, end);
}

size_t race_weekends(uintptr_t component)
{
    return tl::race_weekends(component, [](uintptr_t at, auto &out) { return swofty::mem::read(at, out); });
}

void reset(tl::Load &into, uintptr_t component, size_t stored)
{
    uint32_t season = 0;
    swofty::mem::read(component + tl::SEASON_EVENTS, season);
    tl::reset(into, component, stored, tl::season_events(season), race_weekends(component));
}

void track_list_push(SwoftyContext *c)
{
    size_t calendar = calendar_size(c->rsi);
    uint16_t stored = 0;
    swofty::mem::read(c->rdi + 0x10, stored);
    uint8_t id = uint8_t(c->r13);
    std::lock_guard<std::mutex> hold(lock);
    if (tl::fresh(load, c->rsi, calendar))
        reset(load, c->rsi, stored);
    if (!tl::enter(load, c->r15, id)) {
        c->rip = next_row;
        return;
    }
    matches.clear();
    for (uintptr_t row = c->rbx; row < c->rbp; row += tl::ENTRY) {
        uint8_t row_id = 0;
        if (!swofty::mem::read(row + 8, row_id))
            break;
        if (row_id == id)
            matches.push_back(row);
    }
    uintptr_t pick = tl::choose(load, matches);
    if (!pick)
        c->rip = next_row;
    else
        c->rbx = pick;
}

void track_list_entry_done(SwoftyContext *c)
{
    uint16_t stored = 0;
    if (!swofty::mem::read(c->rdi + 0x10, stored) || !tl::last_entry(c->r15, stored))
        return;
    size_t loaded = calendar_size(c->rsi);
    tl::Load done;
    {
        std::lock_guard<std::mutex> hold(lock);
        done = tl::finish(load, c->rsi);
    }
    if (guarded && !done.component)
        reset(done, c->rsi, stored);
    swofty::log("%s", tl::load_line(c->rsi, stored, loaded, guarded ? &done : nullptr).c_str());
}

void track_list_save(SwoftyContext *c)
{
    size_t count = calendar_size(c->r14);
    {
        std::lock_guard<std::mutex> hold(lock);
        if (!saves.changed(c->r14, count))
            return;
    }
    swofty::log("%s", tl::save_line(c->r14, count).c_str());
}

bool env_off()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_TRACK_LIST_GUARD", v, sizeof v);
    if (!n || n >= sizeof v || !tl::switch_off(v))
        return false;
    swofty::log("[saves] career calendar track list guard off (SWOFTY_TRACK_LIST_GUARD=%s); the custom_track_list count lines stay", v);
    return true;
}

}

namespace runtime {

std::vector<CoreMixins> track_list_guard_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    if (!env_off()) {
        next_row = swofty::api->resolve(NEXT_ROW);
        if (!next_row || !swofty::api->resolve(PUSH)) {
            next_row = 0;
            swofty::log("[saves] career calendar track list guard off: needs %s and %s, which this build does not resolve", PUSH, NEXT_ROW);
        } else {
            guarded = true;
            out.push_back({MIXIN, {{PUSH, SWOFTY_MIXIN_SITE, 100, (void *)&track_list_push, nullptr}}});
        }
    }
    std::vector<SwoftyMixin> sites;
    if (swofty::api->resolve(ENTRY_DONE))
        sites.push_back({ENTRY_DONE, SWOFTY_MIXIN_SITE, 100, (void *)&track_list_entry_done, nullptr});
    if (swofty::api->resolve(SAVE))
        sites.push_back({SAVE, SWOFTY_MIXIN_SITE, 100, (void *)&track_list_save, nullptr});
    if (sites.empty()) {
        swofty::log("[saves] custom_track_list count lines off: needs %s or %s, which this build does not resolve", ENTRY_DONE, SAVE);
    } else {
        logging = true;
        out.push_back({LOG_MIXIN, std::move(sites)});
    }
    return out;
}

void track_list_guard_commit()
{
    if (!offered || settled)
        return;
    settled = true;
    if (guarded) {
        guarded = core_mixins_active(MIXIN);
        swofty::log("[saves] career calendar track list guard %s",
                    guarded ? "armed: each stored custom_track_list entry loads one event instead of every event with its id"
                            : "did not install (see the [core] lines)");
    }
    if (logging && !core_mixins_active(LOG_MIXIN))
        swofty::log("[saves] custom_track_list count lines did not install (see the [core] lines)");
}

}
