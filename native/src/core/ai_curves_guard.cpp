#include "ai_curves_logic.hpp"
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

// AI team performance curves guard (re/db/functions.tsv Ai_Build_TeamPerformanceCurves, 0x14065d170).
//
// Sess_Build_AiSystem builds the AI's performance curves from the team entry whose name the player's vehicle names:
// Sess_Find_Vehicle takes the vehicle ref of the player's entrant record (+0x5c, a kind byte and a stat id) in the entrants
// system's vehicle table, and movzx eax,[rax+0xce8] at F1_25.exe+0x65d213 reads the vehicle's team index. The lookup
// returns 0 for a ref the table has no vehicle for: ACCESS_VIOLATION reading 0xce8 as the first session loads. A Grand Prix
// saved before an addon update that moved its vehicle ids continues with refs that name nothing (Classic Seasons 0.7.0
// pinned the ids of its vehicles; Continue of a saved Grand Prix crashed in the 2018 and 2020 boxes).
//
// At the read a zero vehicle continues at the loop's own exit (F1_25.exe+0x65d25e), the path the loop takes when no entry
// carries the vehicle's team name: the first curve entry. Any vehicle runs the game's code unchanged. The first skip is
// logged.
//
// The sites are found by byte scan in .xtls, so the guard needs no exported binding.
//
// SWOFTY_AI_CURVES_GUARD=0 turns it off.

namespace {

const runtime::Sig S_READ{"site_Ai_TeamCurves_VehicleTeamRead", "0F B6 80 E8 0C 00 00 48 8B 89 E0 04 00 00 48 C1 E0 04", 0x65d213,
                          runtime::Sig::addr, 0, 0};
const runtime::Sig S_EXIT{"site_Ai_TeamCurves_NoTeamEntry", "41 8B 46 60 89 46 68 41 8B 46 64 89 46 6C", 0x65d25e,
                          runtime::Sig::addr, 0, 0};

uintptr_t exit_at = 0;
bool said = false;
std::mutex said_lock;
safetyhook::MidHook h_read;

void skip(safetyhook::Context &c)
{
    if (!runtime::ai_curves::skips(c.rax))
        return;
    c.rip = exit_at;
    bool first;
    {
        std::lock_guard<std::mutex> hold(said_lock);
        first = runtime::ai_curves::says(c.rax, said);
    }
    if (first)
        swofty::log("[ai] team curves: the player's vehicle is not in the entrants system's vehicle table (a saved Grand "
                    "Prix from before its addon moved the vehicle ids); the AI builds with the first team's curves");
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_AI_CURVES_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[ai] team curves guard off (SWOFTY_AI_CURVES_GUARD=%s)", v);
        return;
    }
    uintptr_t read = runtime::find(S_READ, "ai", "ai");
    exit_at = runtime::find(S_EXIT, "ai", "ai");
    if (!read || !exit_at) {
        swofty::log("[ai] team curves guard: not hooked: %s%s", read ? "" : "vehicle read ", exit_at ? "" : "loop exit");
        return;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(read), skip);
    if (!h) {
        swofty::log("[ai] team curves guard: the hook failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_read = std::move(*h);
    swofty::log("[ai] team curves guard armed: a player whose vehicle is not in the vehicle table no longer faults the AI "
                "build");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("ai team curves guard", setup);
}
