// Host-native unit test for the core's boot watchdog grace (native/src/core/watchdog_grace_logic.hpp): which stalls
// the grace holds the game's watchdog kill for and which it lets through, against the stall shapes seen in the
// 1.6.11-1.7.2 hang reports. Built and run by native/test/run_watchdog_grace_test.sh.

#include "watchdog_grace_logic.hpp"

#include <cstdio>
#include <cstring>

namespace wg = runtime::watchdog_grace;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

int main()
{
    {
        wg::Flows f;
        CHECK(!f.allows(31000, 100000));
        CHECK(f.open(100000) == 0);
    }
    {
        wg::Flows f;
        f.flow(0xaa, 1, 50000, "StateFrontend");
        const wg::Entering *e = f.allows(31000, 90000);
        CHECK(e && e->hash == 0xaa && !std::strcmp(e->name, "StateFrontend"));
        CHECK(f.allows(wg::kCapMs, 90000));
        CHECK(!f.allows(wg::kCapMs + 1, 90000));
        f.flow(0xaa, 3, 120000, "StateFrontend");
        CHECK(!f.allows(31000, 121000));
        CHECK(f.open(121000) == 0);
    }
    {
        wg::Flows f;
        f.flow(1, 1, 1000, "A");
        f.flow(2, 1, 2000, "B");
        CHECK(f.open(3000) == 2);
        CHECK(!std::strcmp(f.allows(40000, 3000)->name, "B"));
        f.flow(2, 3, 4000, "B");
        CHECK(!std::strcmp(f.allows(40000, 5000)->name, "A"));
        f.flow(1, 3, 6000, "A");
        CHECK(!f.allows(40000, 7000));
    }
    {
        wg::Flows f;
        f.flow(7, 1, 1000, "Stale");
        CHECK(f.allows(40000, 1000 + wg::kEnteringMaxMs));
        CHECK(!f.allows(40000, 1000 + wg::kEnteringMaxMs + 1));
        CHECK(f.open(1000 + wg::kEnteringMaxMs + 1) == 0);
    }
    {
        wg::Flows f;
        f.flow(9, 1, 5000, "Again");
        f.flow(9, 1, 8000, "Again");
        CHECK(f.open(9000) == 1);
        CHECK(f.newest(9000)->since == 8000);
    }
    {
        wg::Flows f;
        for (uint32_t i = 0; i < wg::kSlots + 5; i++)
            f.flow(100 + i, 1, 1000 + i, "Many");
        CHECK(f.open(2000) == wg::kSlots);
        CHECK(f.newest(2000)->hash == 100 + wg::kSlots + 4);
        char longname[200];
        std::memset(longname, 'x', sizeof longname);
        longname[sizeof longname - 1] = 0;
        f.flow(1, 1, 5000, longname);
        CHECK(std::strlen(f.newest(5000)->name) == wg::kName - 1);
    }
    {
        wg::Flows f;
        f.flow(3, 1, 10000, "Back");
        CHECK(!f.newest(9000));
    }
    {
        wg::Flows f;
        wg::Judgement j = f.judge(31000, 100000);
        CHECK(j.verdict == wg::Verdict::NoFlow);
        f.flow(5, 1, 50000, "S");
        CHECK(f.judge(31000, 100000).verdict == wg::Verdict::Hold);
        CHECK(f.judge(31000, 100000).state == 1);
        j = f.judge(wg::kCapMs + 1, 100000);
        CHECK(j.verdict == wg::Verdict::CapReached);
        j = f.judge(31000, 50000 + wg::kEnteringMaxMs + 7);
        CHECK(j.verdict == wg::Verdict::TooOld && j.age == wg::kEnteringMaxMs + 7 && j.state == 1);
    }
    {
        wg::Flows f;
        f.flow(6, 1, 10000, "StateSession");
        f.flow(6, 2, 20000, "StateSession");
        CHECK(f.open(21000) == 1);
        wg::Judgement j = f.judge(35000, 55000);
        CHECK(j.verdict == wg::Verdict::Hold && j.state == 2 && j.age == 35000);
        CHECK(!std::strcmp(f.allows(35000, 55000)->name, "StateSession"));
        j = f.judge(35000, 20000 + wg::kUpdatingMaxMs + 1);
        CHECK(j.verdict == wg::Verdict::TooOld && j.state == 2);
        CHECK(!f.allows(35000, 20000 + wg::kUpdatingMaxMs + 1));
        f.flow(6, 3, 60000, "StateSession");
        CHECK(f.judge(35000, 61000).verdict == wg::Verdict::NoFlow);
    }
    {
        wg::Flows f;
        f.flow(1, 1, 1000, "Old");
        f.flow(2, 2, 900000, "Live");
        wg::Judgement j = f.judge(40000, 1000 + wg::kEnteringMaxMs + 5000);
        CHECK(j.verdict == wg::Verdict::Hold && j.state == 2);
        wg::Latest l = f.latest();
        CHECK(l.entering == 1000 && l.updating == 900000);
        CHECK(wg::judge(40000, 901000, l).verdict == wg::Verdict::Hold);
        CHECK(wg::judge(40000, 901000, wg::Latest{}).verdict == wg::Verdict::NoFlow);
        CHECK(!std::strcmp(wg::verdict_name(wg::Verdict::LockBusy), "flow table busy"));
    }
    CHECK(wg::bucket(0) == 0 && wg::bucket(29999) == 0 && wg::bucket(30000) == 1 && wg::bucket(179999) == 5);
    if (g_fail) {
        std::printf("watchdog grace test: %d failure(s)\n", g_fail);
        return 1;
    }
    std::printf("watchdog grace test OK\n");
    return 0;
}
