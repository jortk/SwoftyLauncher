#include "ai_corner_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <string>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// AI ERS corner guard (re/db/globals.tsv site_Ai_Ers_CornerLineRead).
//
// Ai_Ers_DecidePushReasons checks whether the corner a car is heading for is one of the ERS boost entry's corner_id
// list. It takes the corner from the car's gap evaluation for the next corner (Ai_Overtake_EvaluateGap from the
// current corner end), whose first field is the next corner marker the route accepts between that corner's start
// and end lines. When the next marker is outside that bracket, the evaluation stores 0 there with its valid flag
// still set. A track whose AI line has corner markers that its corner start and end lines do not enclose, such as
// Brands Hatch Grand Prix (mod.brhgp 1.8.0), leaves that 0, and mov r8d,[r8+0x10] at F1_25.exe+0x649819
// (Ai_Ers_DecidePushReasons+0x1039) reads the marker's id without checking it: ACCESS_VIOLATION reading 0x10.
//
// At site_Ai_Ers_CornerLineRead a zero marker continues at site_Ai_Ers_NoCornerMatch, the path the game takes for an
// entry with no corner ids or no id that matches, so the car keeps its ERS decision without a corner bonus for that
// corner. A marker that is there runs the game's code unchanged. The first skip is logged.
//
// The sites are found by byte scan (the KB rows are L1: subsystem ai is owned by ai-re). Each pattern matches once on
// the KB build and on Steam 1.0.145.
//
// SWOFTY_AI_ERS_GUARD=0 turns it off.

namespace {

const runtime::Sig S_READ{"site_Ai_Ers_CornerLineRead", "4F 8B 04 08 45 8B 40 10 48 C1 FA 03", 0x6497d9,
                          runtime::Sig::addr, 0, 4, 0};
const runtime::Sig S_NONE{"site_Ai_Ers_NoCornerMatch", "C4 A1 7A 10 44 C8 04 C4 C1 7A 11 47 5C", 0x649cc6,
                          runtime::Sig::addr, 0, 7, 0};

uintptr_t no_match = 0;
runtime::ai_corner::Tally tally;
safetyhook::MidHook h_read;

void corner_read(safetyhook::Context &c)
{
    if (!runtime::ai_corner::missing(c.r8))
        return;
    c.rip = no_match;
    if (tally.first())
        swofty::log("[ai] ers corner: car record +0x%llx has no corner marker between the next corner's start and end "
                    "lines on this track's AI line; its ERS decision skips that corner's bonus",
                    static_cast<unsigned long long>(c.rbx));
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_AI_ERS_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[ai] ers corner guard off (SWOFTY_AI_ERS_GUARD=%s)", v);
        return;
    }
    uintptr_t at = runtime::find(S_READ, "ers corner guard", "ai");
    no_match = runtime::find(S_NONE, "ers corner guard", "ai");
    if (!at || !no_match) {
        no_match = 0;
        swofty::log("[ai] ers corner guard off: needs %s and %s, which this build does not resolve", S_READ.what,
                    S_NONE.what);
        return;
    }
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), corner_read);
    if (!h) {
        no_match = 0;
        swofty::log("[ai] ers corner guard: the hook failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_read = std::move(*h);
    swofty::log("[ai] ers corner guard armed: a car with no corner marker for its next corner keeps its ERS decision "
                "without that corner's bonus");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("ai ers corner guard", setup);
}
