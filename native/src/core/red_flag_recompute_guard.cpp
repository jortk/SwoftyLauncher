#include "runtime.hpp"

#include <windows.h>

#include "red_flag_recompute_logic.hpp"

#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <vector>

// Red flag recompute guard (re/db/functions.tsv Sess_RaceStrategy_RedFlagRecompute).
//
// When a red flag is deployed, Sess_Strategy_RedFlagRecomputeAll walks the session's participants and, for each one
// with a vehicle, hands the vehicle's race session record [[strategy_sys + 0x38] + vehicle * 8] to slot 13 (+0x68)
// of the race strategy processor, Sess_RaceStrategy_RedFlagRecompute. It does so from three places (its two inline
// loops and the job body 0x1436ba330 it queues on the strategy queue), and none of them tests the record. The
// strategy system's other per-vehicle walks skip a null record; these do not, and the function's first instruction
// after its prologue reads [record + 0x10]: ACCESS_VIOLATION reading 0x10 at F1_25.exe+0x369c7ec with rdx = 0.
//
// The guard wraps the function and returns when the record is null. The function is void and its callers discard
// rax; it writes only to the record and through it, and its own exit for a red flag that is not deployed returns
// before any write, so a skipped vehicle is left as that exit leaves it. A vehicle with a record runs the game's
// code unchanged. The wrap runs on the session thread and the strategy job threads at once.
//
// SWOFTY_RED_FLAG_RECOMPUTE_GUARD=0 turns it off.

namespace {

constexpr const char *MIXIN = "swofty_core.dll (red flag recompute guard)";
constexpr const char *TARGET = "Sess_RaceStrategy_RedFlagRecompute";

namespace rf = runtime::red_flag_recompute;

using Recompute = void(void *, void *);
Recompute *orig_recompute = nullptr;
bool offered = false, settled = false, wanted = false;
std::atomic<int> skipped{0};
rf::Skips skips;

void recompute(void *processor, void *race_session_data)
{
    if (!rf::record_missing((uint64_t)(uintptr_t)race_session_data)) {
        orig_recompute(processor, race_session_data);
        return;
    }
    int n = ++skipped;
    if (!skips.logged(n))
        return;
    swofty::log("[session] a red flag strategy recompute has no race session record for a vehicle, so that vehicle is skipped%s",
                skips.last(n) ? " (further skips are not logged)" : "");
}

}

namespace runtime {

std::vector<CoreMixins> red_flag_recompute_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_RED_FLAG_RECOMPUTE_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[session] red flag recompute guard off (SWOFTY_RED_FLAG_RECOMPUTE_GUARD=%s)", v);
        return out;
    }
    if (!swofty::api->resolve(TARGET)) {
        swofty::log("[session] red flag recompute guard off: needs %s, which this build does not resolve", TARGET);
        return out;
    }
    wanted = true;
    out.push_back({MIXIN, {{TARGET, SWOFTY_MIXIN_WRAP, 100, (void *)&recompute, (void **)&orig_recompute}}});
    return out;
}

void red_flag_recompute_commit()
{
    if (!offered || settled || !wanted)
        return;
    settled = true;
    swofty::log("[session] red flag recompute guard %s",
                core_mixins_active(MIXIN) ? "armed: a red flag strategy recompute without a race session record returns instead of faulting"
                                          : "did not install (see the [core] lines)");
}

}
