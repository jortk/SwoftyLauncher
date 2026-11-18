#include "ai_corner_logic.hpp"
#include "runtime.hpp"

#include <windows.h>

#include <swofty/mixin.hpp>

#include <cstdint>
#include <cstring>
#include <vector>

// AI brake line guard (re/db/globals.tsv site_Ai_UpdateCars_CornerLineLoad).
//
// Ai_Job_UpdateCarsParallel keeps a track-line query per car (Ai_Overtake_EvaluateGap). Its second pass
// (Ai_TrackLine_FindNextCornerStart; despite the name it reads the lane's brake line table, lane+0x498) stores the
// next brake line ahead of the car at result+8 when that line lies between the corner start and corner end the first
// pass found, or 0 when there is none, with the valid flag result+0x4b still set. A track whose AI line has a corner
// without a brake line inside it, or an incomplete set of corner starts and ends, such as a WIP track addon, leaves
// that 0. One later read, mov rax,[rax+rdi*8+0xb8] at F1_25.exe+0x64069e (Ai_Job_UpdateCarsParallel+0x232e), takes
// the brake line's per-route entry without checking it: ACCESS_VIOLATION reading 0xb8.
//
// At site_Ai_UpdateCars_CornerLineLoad a zero line continues at site_Ai_UpdateCars_NoCornerLine, the path the very
// next test of that load takes for a line without an entry, so the car keeps driving its line without corner data.
// A line that is there runs the game's code unchanged. The first skip is logged.
//
// SWOFTY_AI_CORNER_GUARD=0 turns it off.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (ai brake line guard)";
constexpr const char *LOAD = "site_Ai_UpdateCars_CornerLineLoad";
constexpr const char *NONE = "site_Ai_UpdateCars_NoCornerLine";

uintptr_t no_corner = 0;
bool offered = false, settled = false;
runtime::ai_corner::Tally tally;

void corner_load(SwoftyContext *c)
{
    if (!runtime::ai_corner::missing(c->rax))
        return;
    c->rip = no_corner;
    if (tally.first())
        swofty::log("[ai] brake line: car %d has no brake line ahead inside the next corner on this track's AI line (a "
                    "corner without a brake line, or incomplete corner starts and ends); it drives the line without "
                    "corner data",
                    int(static_cast<int32_t>(c->rdi)));
}

}

namespace runtime {

std::vector<CoreMixins> ai_corner_guard_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_AI_CORNER_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[ai] brake line guard off (SWOFTY_AI_CORNER_GUARD=%s)", v);
        return out;
    }
    no_corner = swofty::api->resolve(NONE);
    if (!no_corner || !swofty::api->resolve(LOAD)) {
        no_corner = 0;
        swofty::log("[ai] brake line guard off: needs %s and %s, which this build does not resolve", LOAD, NONE);
        return out;
    }
    out.push_back({MIXIN, {{LOAD, SWOFTY_MIXIN_SITE, 100, (void *)&corner_load, nullptr}}});
    return out;
}

void ai_corner_guard_commit()
{
    if (!offered || settled || !no_corner)
        return;
    settled = true;
    swofty::log("[ai] brake line guard %s",
                core_mixins_active(MIXIN) ? "armed: a car with no brake line ahead drives its line without corner data"
                                          : "did not install (see the [core] lines)");
}

}
