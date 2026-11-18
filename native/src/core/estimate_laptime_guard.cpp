#include "runtime.hpp"

#include <windows.h>

#include "estimate_laptime_logic.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <vector>

// Estimate lap time guard (re/db/functions.tsv Sess_VirtualSession_EstimateLapTime).
//
// The virtual session (the game's simulation of the cars that are not driven, used by the red flag unlap
// simulation, the qualifying pace debug and the race finish) estimates a lap time for a vehicle in
// Sess_VirtualSession_EstimateLapTime. After it has filled the compound query for the car's tyre set it takes the
// vehicle's tyre object out of the vector at [[session + 0xa0] + 0x28], indexed by vehicle, and passes it to
// 0x142e80ce0, the compound query, which reads the table at object + 0x10 with no check. A vehicle without a tyre
// object, such as a car that has just been retired from the player's race, hands it a null object and the query
// faults reading 0x10 at F1_25.exe+0x2e80ce0 (rcx=0, rax=compound index, rbx=vehicle).
//
// site_Sess_VirtualSession_EstimateTyreQuery is the lea rdx before that call; both the compound table path and the
// fall through land on it with rcx holding the tyre object. When rcx is 0 the guard continues at
// site_Sess_VirtualSession_EstimateTail, the function's own return for a session that does not estimate: the lap
// time passed in is returned unchanged. A vehicle with a tyre object runs the game's code unchanged.
//
// SWOFTY_ESTIMATE_LAPTIME_GUARD=0 turns it off.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (estimate lap time guard)";
constexpr const char *QUERY = "site_Sess_VirtualSession_EstimateTyreQuery";
constexpr const char *TAIL = "site_Sess_VirtualSession_EstimateTail";

namespace el = runtime::estimate_laptime;

uintptr_t estimate_tail = 0;
bool offered = false, settled = false;
std::atomic<int> skipped{0};
el::Skips skips;

void tyre_query(SwoftyContext *c)
{
    if (!el::tyre_ctx_missing(c->rcx))
        return;
    c->rip = estimate_tail;
    int n = ++skipped;
    if (!skips.logged(n))
        return;
    swofty::log("[session] a virtual session lap estimate has no tyre object for vehicle %u, so it returns the lap time unchanged%s",
                (unsigned)(c->rbx & 0xff), skips.last(n) ? " (further skips are not logged)" : "");
}

}

namespace runtime {

std::vector<CoreMixins> estimate_laptime_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_ESTIMATE_LAPTIME_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[session] estimate lap time guard off (SWOFTY_ESTIMATE_LAPTIME_GUARD=%s)", v);
        return out;
    }
    estimate_tail = swofty::api->resolve(TAIL);
    if (!estimate_tail || !swofty::api->resolve(QUERY)) {
        estimate_tail = 0;
        swofty::log("[session] estimate lap time guard off: needs %s and %s, which this build does not resolve", QUERY, TAIL);
        return out;
    }
    out.push_back({MIXIN, {{QUERY, SWOFTY_MIXIN_SITE, 100, (void *)&tyre_query, nullptr}}});
    return out;
}

void estimate_laptime_commit()
{
    if (!offered || settled || !estimate_tail)
        return;
    settled = true;
    swofty::log("[session] estimate lap time guard %s",
                core_mixins_active(MIXIN) ? "armed: a lap estimate without a tyre object returns the lap time unchanged"
                                          : "did not install (see the [core] lines)");
}

}
