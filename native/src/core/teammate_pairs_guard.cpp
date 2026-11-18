#include "runtime.hpp"
#include "teammate_pairs_logic.hpp"

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

// Teammate pairs guard (re/db/globals.tsv site_Sess_Strategy_TeammatePairSort).
//
// Sess_Strategy_UpdateTeammatePairs (0x1436abb70) runs as a session is entered and again on every red flag recompute.
// For each team of the session it fetches the team's car records as a vector of pointers (0x1424abd60; r12 = begin,
// rbp = end), and pairs the team's first two cars for strategy through the module at [session + 0x70] vslot +0x28.
// When both of the first two records have the same byte at +0x81 (set for a live car) the function sorts the whole
// vector by the byte at +0x80 (0x1436b36f0, an introsort whose comparator throws through Ne_Std_ThrowException when
// either record's +0x81 byte is 0), or runs the comparator-based sort 0x1436b3e00 when both are 0. A team that fields a
// third car (the garage crew guard sees the same teams as their third garage entry) puts a third record in the vector,
// the record's +0x81 byte is 0, and the sort throws: FatalExceptionTerminate, FAST_FAIL_INVALID_ARG, as the session is
// entered. Only the first two records are read after the sort.
//
// At site_Sess_Strategy_TeammatePairSort (the cmp r13, 0x10 that tells a team with a pair from a team with one car) r13
// is the vector's size in bytes. When a team has more than two cars the guard lowers rbp and r13 to the first two, so
// the sort and the pairing see only them and the extra cars stay unpaired. The vector itself is released through r12.
//
// The site is found by byte scan (the KB row is L1).
//
// SWOFTY_TEAMMATE_PAIRS_GUARD=0 turns it off.

namespace {

namespace mem = swofty::mem;
namespace tp = runtime::teammate_pairs;

const runtime::Sig S_SORT{"site_Sess_Strategy_TeammatePairSort",
                          "49 83 FD 10 0F 82 ?? ?? ?? ?? 49 8B 04 24 49 8B 54 24 08 0F B6 88 81 00 00 00", 0x36abcaa,
                          runtime::Sig::addr, 0, 0, 0};

tp::Seen seen;
std::mutex seen_lock;
safetyhook::MidHook h_sort;

void say(uint64_t session, uint64_t team_ptr, uint64_t bytes)
{
    uint32_t team = 0xffffffffu;
    mem::read(team_ptr + tp::TEAM_ID, team);
    bool first;
    {
        std::lock_guard<std::mutex> hold(seen_lock);
        first = seen.first(session, team);
    }
    if (first)
        swofty::log("[strategy] team %u has %u cars; only its first two are paired for strategy, the others stay unpaired",
                    unsigned(team), unsigned(tp::cars(bytes)));
}

void sort_site(safetyhook::Context &c)
{
    if (!tp::over(c.r13))
        return;
    uint64_t team_ptr = 0;
    if (runtime::readable(reinterpret_cast<const void *>(c.r14), sizeof team_ptr))
        mem::read(c.r14, team_ptr);
    say(c.rsi, team_ptr, c.r13);
    c.rbp = c.r12 + tp::PAIR_BYTES;
    c.r13 = tp::PAIR_BYTES;
}

void setup()
{
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_TEAMMATE_PAIRS_GUARD", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[strategy] teammate pairs guard off (SWOFTY_TEAMMATE_PAIRS_GUARD=%s)", v);
        return;
    }
    uintptr_t at = runtime::find(S_SORT, "strategy", "strategy");
    if (!at)
        return;
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), sort_site);
    if (!h) {
        swofty::log("[strategy] teammate pairs guard: the hook failed: %s", runtime::hook_error(h.error()).c_str());
        return;
    }
    h_sort = std::move(*h);
    swofty::log("[strategy] teammate pairs guard armed: a team with more than two cars pairs its first two for strategy "
                "instead of throwing");
}

}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("teammate pairs guard", setup);
}
