#include "runtime.hpp"
#include "activity_resume_logic.hpp"

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

// Career activity resume guard (re/db/globals.tsv site_Game_ActivityResumeRaceLengthFirst, ..Second, ..Return).
//
// Game_ActivitiesUIDeeplinkManager_ReportCareerActivityResume (0x143867ff0) asks the .data1 body 0x151948840 (thunk
// 0x14386b1d0, called at 0x1438682e5) for the race length text of the championship ("full", "short" or "medium"). The body
// reads the game mode's track list size, looks up a ui variable group of the championship (virtual +0x10 of the ui
// variable table at championship +0x1030) and reads three u32 fields of the group through 0x140bc9ef0, which returns
// 0 for a group that does not exist. A career whose current track is not in the game mode's track list, such as Miami in a
// 2016 Progressive career, logs `UI_TRACK Track [miami] is not found in game mode tracklist[<invalid/ui-variable>]` and
// gets such a group; the first read, mov r14d,[rax] at 0x1519488a9 (the second is mov ebp,[rax] at 0x1519488b6),
// faults reading 0x0 as Continue loads the save.
//
// The text only feeds the presence report, so a group without fields resumes at the body's epilogue with the default
// text the body already holds in rsi ("full"), the same value it returns when the track list is missing.
//
// The sites are found by byte scan in .data1 (plaintext on the Steam build; a packed build does not resolve them, and
// the guard stays off there).
//
// SWOFTY_ACTIVITY_RESUME_GUARD=0 turns it off.

namespace {

namespace mem = swofty::mem;

const runtime::Sig S_FIRST{"site_Game_ActivityResumeRaceLengthFirst", "E8 ?? ?? ?? ?? 44 8B 30 48 89 F9 B2 01", 0x1519488a9,
                           runtime::Sig::addr, 0, 5, 0, ".data1"};
const runtime::Sig S_SECOND{"site_Game_ActivityResumeRaceLengthSecond", "E8 ?? ?? ?? ?? 8B 28 48 89 F9 31 D2", 0x1519488b6,
                            runtime::Sig::addr, 0, 5, 0, ".data1"};
const runtime::Sig S_RETURN{"site_Game_ActivityResumeRaceLengthReturn", "48 8D 35 ?? ?? ?? ?? 48 89 F0 48 83 C4 20",
                            0x1519488f6, runtime::Sig::addr, 0, 7, 0, ".data1"};


uintptr_t return_at = 0;
bool said = false;
std::mutex said_lock;
safetyhook::MidHook h_first, h_second;

void skip(safetyhook::Context &c)
{
    if (!runtime::activity_resume::skips(c.rax))
        return;
    c.rip = return_at;
    bool first;
    {
        std::lock_guard<std::mutex> hold(said_lock);
        first = runtime::activity_resume::says(c.rax, said);
    }
    if (first)
        swofty::log("[career] activity resume: the championship has no race length fields (its track is not in the "
                    "game mode's track list); the report keeps the default");
}

bool arm(safetyhook::MidHook &slot, const runtime::Sig &s, uintptr_t at)
{
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), skip);
    if (!h) {
        swofty::log("[career] activity resume guard: %s: the hook failed: %s", s.what, runtime::hook_error(h.error()).c_str());
        return false;
    }
    slot = std::move(*h);
    return true;
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_ACTIVITY_RESUME_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[career] activity resume guard off (SWOFTY_ACTIVITY_RESUME_GUARD=%s)", v);
        return;
    }
    uintptr_t first = runtime::find(S_FIRST, "career", "career"), second = runtime::find(S_SECOND, "career", "career");
    return_at = runtime::find(S_RETURN, "career", "career");
    if (!first || !second || !return_at) {
        swofty::log("[career] activity resume guard: not hooked: %s%s%s", first ? "" : "first read ", second ? "" : "second read ",
                    return_at ? "" : "return");
        return;
    }
    if (arm(h_first, S_FIRST, first) && arm(h_second, S_SECOND, second))
        swofty::log("[career] activity resume guard armed: a championship whose track is not in the game mode's track "
                    "list no longer faults the activity report");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("activity resume guard", setup);
}
