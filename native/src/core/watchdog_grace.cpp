#include "load_stall_logic.hpp"
#include "runtime.hpp"
#include "watchdog_grace_logic.hpp"

#include <windows.h>

#include <swofty/mixin.hpp>

#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <vector>

// Boot watchdog grace (re/db/globals.tsv site_Ne_WatchdogKillPath, site_Ne_WatchdogKillSkip).
//
// Ne_Watchdog_ThreadLoop kills the process (a write to address 1) when the frame tick has not fed the watchdog for
// WatchDog.delay ms (30000). On 14-16 GB machines with big modded profiles a front-end flow's Entering holds the
// main thread in game code for longer than that (median 24 s, up to 120 s against a healthy 7 s), the frame thread
// logs "The signal timed out" and the game dies as the load finishes.
//
// At site_Ne_WatchdogKillPath, reached only when the time since the last tick (r14d, ms) exceeds the max delay,
// the hook continues at site_Ne_WatchdogKillSkip, the branch the game takes while the delay has not run out, when
// a flow is in Entering, or entered Updating within the last 5 minutes (career.cpp reports every flow transition),
// and the silence is under 180 s. A stall with no such flow, or one past 180 s, reaches the game's kill unchanged and
// the reason is logged once. A kill with the shutdown flag set (dil, the object's +0x120) is never held. The flow
// table is read under its lock for up to 50 ms, then from a lock-free mirror of the newest Entering and Updating times.
//
// A held kill also asks the load-stall watchdog for a report of every thread's stack (load_stall.cpp), which runs on
// that watchdog's own thread.
//
// SWOFTY_BOOT_WATCHDOG_GRACE=0 turns it off.

namespace {

namespace wg = runtime::watchdog_grace;

constexpr const char *MIXIN = "swofty_core.dll (boot watchdog grace)";

uintptr_t kill_skip = 0;
bool offered = false, settled = false;
std::mutex lock;
wg::Flows flows;
std::atomic<uint64_t> mirror_entering{0}, mirror_updating{0};
std::atomic<uint32_t> last_bucket{~0u};
std::atomic<uint32_t> last_elapsed{0};
std::atomic<uint32_t> declined{0};
std::atomic<uint64_t> held{0};

constexpr int LOCK_TRIES = 50;

void decline(wg::Verdict v, uint32_t elapsed, const wg::Judgement &j, uint8_t rdi)
{
    uint32_t bit = 1u << unsigned(v);
    if (declined.fetch_or(bit) & bit)
        return;
    last_bucket.store(~0u);
    switch (v) {
    case wg::Verdict::Shutdown:
        swofty::log("[core] boot watchdog grace declined: the game's shutdown flag is set (rdi low byte 0x%02x), the "
                    "frame tick was silent %u ms",
                    rdi, elapsed);
        break;
    case wg::Verdict::CapReached:
        swofty::log("[core] boot watchdog grace declined: the frame tick was silent %u ms, past the %u ms cap", elapsed,
                    wg::kCapMs);
        break;
    case wg::Verdict::TooOld:
        swofty::log("[core] boot watchdog grace declined: the frame tick was silent %u ms and the newest flow in %s has "
                    "been there %llu ms (limit %llu ms)",
                    elapsed, j.state == 1 ? "Entering" : "Updating", static_cast<unsigned long long>(j.age),
                    static_cast<unsigned long long>(j.state == 1 ? wg::kEnteringMaxMs : wg::kUpdatingMaxMs));
        break;
    default:
        swofty::log("[core] boot watchdog grace declined: the frame tick was silent %u ms and no flow is in Entering "
                    "(limit %llu ms) or Updating (limit %llu ms)",
                    elapsed, static_cast<unsigned long long>(wg::kEnteringMaxMs),
                    static_cast<unsigned long long>(wg::kUpdatingMaxMs));
        break;
    }
}

void kill_path(SwoftyContext *c)
{
    uint32_t elapsed = static_cast<uint32_t>(c->r14);
    if (elapsed < last_elapsed.exchange(elapsed))
        declined.store(0);
    uint8_t rdi = static_cast<uint8_t>(c->rdi);
    uint64_t now = GetTickCount64();
    if (rdi) {
        decline(wg::Verdict::Shutdown, elapsed, {}, rdi);
        return;
    }
    wg::Judgement j;
    wg::Entering where;
    bool locked = false;
    for (int i = 0; i < LOCK_TRIES && !(locked = lock.try_lock()); i++)
        Sleep(1);
    if (locked) {
        j = flows.judge(elapsed, now);
        if (const wg::Entering *e = flows.newest(now))
            where = *e;
        lock.unlock();
    } else {
        swofty::log("[core] boot watchdog grace: the flow table stayed busy for %d ms, reading the lock-free snapshot",
                    LOCK_TRIES);
        wg::Latest l;
        l.entering = mirror_entering.load();
        l.updating = mirror_updating.load();
        j = wg::judge(elapsed, now, l);
        std::snprintf(where.name, sizeof where.name, "(snapshot)");
    }
    if (j.verdict != wg::Verdict::Hold) {
        decline(j.verdict, elapsed, j, rdi);
        return;
    }
    c->rip = kill_skip;
    held++;
    runtime::load_stall_signal(runtime::load_stall::GameWatchdog, elapsed);
    uint32_t b = wg::bucket(elapsed);
    if (last_bucket.exchange(b) != b)
        swofty::log("[core] boot watchdog grace: the frame tick has been silent %u ms with flow \"%s\" in %s; the "
                    "game's watchdog kill is held (up to %u ms)",
                    elapsed, where.name, j.state == 1 ? "Entering" : "Updating", wg::kCapMs);
}

}

namespace runtime {

void watchdog_flow(uint32_t hash, uint32_t to, const char *name)
{
    if (!offered || !kill_skip)
        return;
    std::lock_guard<std::mutex> g(lock);
    flows.flow(hash, to, GetTickCount64(), name);
    wg::Latest l = flows.latest();
    mirror_entering.store(l.entering);
    mirror_updating.store(l.updating);
}

std::vector<CoreMixins> watchdog_grace_mixins()
{
    std::vector<CoreMixins> out;
    if (offered || !code_ready())
        return out;
    offered = true;
    char v[16] = {};
    DWORD n = GetEnvironmentVariableA("SWOFTY_BOOT_WATCHDOG_GRACE", v, sizeof v);
    if (n && n < sizeof v && (!strcmp(v, "0") || !strcmp(v, "off"))) {
        swofty::log("[core] boot watchdog grace off (SWOFTY_BOOT_WATCHDOG_GRACE=%s)", v);
        return out;
    }
    kill_skip = swofty::api->resolve("site_Ne_WatchdogKillSkip");
    if (!kill_skip || !swofty::api->resolve("site_Ne_WatchdogKillPath")) {
        kill_skip = 0;
        swofty::log("[core] boot watchdog grace off: needs site_Ne_WatchdogKillPath and site_Ne_WatchdogKillSkip, which "
                    "this build does not resolve");
        return out;
    }
    out.push_back({MIXIN, {{"site_Ne_WatchdogKillPath", SWOFTY_MIXIN_SITE, 100, (void *)&kill_path, nullptr}}});
    return out;
}

void watchdog_grace_commit()
{
    if (!offered || settled || !kill_skip)
        return;
    settled = true;
    swofty::log("[core] boot watchdog grace %s",
                core_mixins_active(MIXIN)
                    ? "armed: the game's watchdog kill is held for up to 180 s while a flow is in Entering or recently Updating"
                    : "did not install (see the [core] lines)");
}

}
