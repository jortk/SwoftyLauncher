#include "calendar_track_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <mutex>
#include <string>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Career calendar track guard (re/db/globals.tsv site_Career_ScheduleComponent_CalendarFilled).
//
// The ScheduleComponent's season builder (vtable slot 0x1456cdee8, 0x14126a1a0 on the KB build, 0x14126a7e0 on Steam
// 1.0.145) fills the calendar (+0x2a8 vector, begin +0x2b0, end +0x2b8, 0x28-byte entries: vtable, u8 event id +8,
// event +0x10, day +0x18) for the current season, adds a race weekend activity per entry to every driver and team
// calendar, and then calls the championship's vt+0x60 (0x1493efb90 on the KB build, 0x149161110 on Steam 1.0.145),
// which builds one Weekend per entry. For each entry that function takes the event's track (byte [event+0xb0]) and reads
// the track's TrackDataRecord from the career's TrackDataSystem (CareerEntity +0xf58, 16-byte items at +0x28..+0x30
// indexed by track stat id) with no bound or null check: movzx r12d,byte [rax+1] on the item's record pointer.
//
// The record vector only holds the tracks the game edition's content set lists (content_sets.content_set 2026
// available_tracks, plus the tracks the launcher adds): tracks.Track rows outside it, such as styria (25),
// imola_prototype (33), melbourne24 (34), suzuka24, imola24, miami24 and the dormant rows without assets, leave null
// items. A calendar addon that writes such a track into an event (Custom Calendar 0.2.15/0.2.16 assigns the picked tracks
// to the date slot events 110 and up) makes the confirm of a new career fault reading 0x1 at F1_25.exe+0x9161246
// (Steam 1.0.145) or F1_25.exe+0x93efcd6 (1.0.141), at the first round on such a track.
//
// The guard runs right after the calendar fill, before the activities and the weekends are made from it. It drops every
// entry whose track has no record from the calendar, the way the game's own vector moves entries (the four data words
// copied, the vacated slots destroyed through their vtable[0] with 0), so the activities, the weekends and the saved
// custom_track_list all hold the same rounds. A calendar in which no round has track data is left as it is.
//
// SWOFTY_CALENDAR_TRACK_GUARD=0 turns it off.

namespace {

namespace mem = swofty::mem;
namespace ct = runtime::calendar_track;

const runtime::Sig S_CALENDAR_FILLED{"site_Career_ScheduleComponent_CalendarFilled",
                                     "49 8D 9D A8 02 00 00 45 8B 85 28 04 00 00 4C 89 E9 48 89 DA E8 ?? ?? ?? ?? 49 8B BD 10 04 00 00",
                                     0x126a23d, runtime::Sig::addr, 0, 0x19, 0};

constexpr size_t MAX_LINES = 24;

safetyhook::MidHook h_filled;
std::mutex lock;
bool unread_said = false, none_said = false;

void destroy(uintptr_t entry)
{
    uintptr_t vt = 0, dtor = 0;
    if (!mem::read(entry, vt) || !mem::in_image(vt) || !mem::read(vt, dtor) || !mem::in_image(dtor))
        return;
    reinterpret_cast<void (*)(void *, int)>(dtor)(reinterpret_cast<void *>(entry), 0);
}

void filled(safetyhook::Context &c)
{
    uintptr_t component = c.r13;
    auto read = [](uintptr_t at, auto &out) { return mem::read(at, out); };
    std::lock_guard<std::mutex> hold(lock);
    ct::Calendar cal = ct::scan(component, read);
    if (!cal.read) {
        if (!unread_said) {
            unread_said = true;
            swofty::log("[career] calendar track guard: the calendar of schedule component 0x%llx could not be read; "
                        "left as it is",
                        (unsigned long long)component);
        }
        return;
    }
    size_t missing = 0;
    for (const ct::Round &r : cal.rounds)
        missing += !r.data;
    if (!missing)
        return;
    std::vector<size_t> keep = ct::kept(cal.rounds);
    if (keep.empty()) {
        if (!none_said) {
            none_said = true;
            swofty::log("[career] calendar track guard: none of the %zu round(s) has track data in this game; the "
                        "calendar is left as it is",
                        cal.rounds.size());
        }
        return;
    }
    size_t said = 0;
    for (size_t i = 0; i < cal.rounds.size() && said < MAX_LINES; ++i) {
        const ct::Round &r = cal.rounds[i];
        if (r.data)
            continue;
        ++said;
        swofty::log("[career] calendar: round %zu (event %u) is on track %u, which this game loaded no track data for "
                    "(it is not in the game's content); the round is left out of the season",
                    i + 1, unsigned(r.event), unsigned(r.track));
    }
    uintptr_t end = ct::compact(cal.begin, keep, read,
                                [](uintptr_t at, uint64_t v) { *reinterpret_cast<uint64_t *>(at) = v; });
    if (!end) {
        swofty::log("[career] calendar track guard: the kept rounds could not be read; the calendar is left as it is");
        return;
    }
    for (uintptr_t slot = end; slot < cal.end; slot += ct::ENTRY)
        destroy(slot);
    *reinterpret_cast<uint64_t *>(component + ct::CALENDAR_END) = end;
    swofty::log("[career] calendar: %zu of %zu round(s) kept; the season runs without the %zu round(s) on tracks the "
                "game has no track data for",
                keep.size(), cal.rounds.size(), cal.rounds.size() - keep.size());
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_CALENDAR_TRACK_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[career] calendar track guard off (SWOFTY_CALENDAR_TRACK_GUARD=%s)", v);
        return;
    }
    uintptr_t at = runtime::find(S_CALENDAR_FILLED, "career", "career");
    if (!at) {
        swofty::log("[career] calendar track guard: not hooked");
        return;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), filled);
    if (!h) {
        swofty::log("[career] calendar track guard: the hook failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_filled = std::move(*h);
    swofty::log("[career] calendar track guard armed: a season round on a track the game has no track data for is left "
                "out of the calendar instead of faulting");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("calendar track guard", setup);
}
