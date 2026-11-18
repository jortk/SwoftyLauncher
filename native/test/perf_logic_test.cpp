// Host-native unit test for the minute summary lines of the core's profiler (native/src/core/perf_logic.hpp): the
// VirtualQuery line, the process CPU line and the busiest probes with their call rates. Built and run by
// native/test/run_perf_logic_test.sh.

#include "perf_logic.hpp"

#include <cstdio>
#include <string>

namespace lg = perf::logic;

static int g_fail;

#define CHECK(c)                                                                                                       \
    do {                                                                                                               \
        if (!(c)) {                                                                                                    \
            std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #c);                                                    \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

#define CHECK_EQ(a, b)                                                                                                 \
    do {                                                                                                               \
        std::string x_ = (a), y_ = (b);                                                                                \
        if (x_ != y_) {                                                                                                \
            std::printf("FAIL %s:%d\n  got:  %s\n  want: %s\n", __FILE__, __LINE__, x_.c_str(), y_.c_str());          \
            g_fail++;                                                                                                  \
        }                                                                                                              \
    } while (0)

static void grouped()
{
    CHECK_EQ(lg::grouped(0), "0");
    CHECK_EQ(lg::grouped(999), "999");
    CHECK_EQ(lg::grouped(1000), "1,000");
    CHECK_EQ(lg::grouped(18402), "18,402");
    CHECK_EQ(lg::grouped(1234567890), "1,234,567,890");
}

static void deltas()
{
    CHECK(lg::delta(10, 4) == 6);
    CHECK(lg::delta(4, 10) == 0);
    CHECK(lg::cores(60ull * 10000000, 60) == 1.0);
    CHECK(lg::cores(5, 0) == 0);
    CHECK_EQ(lg::rva_label("core", 0x1a2b3c), "core+0x1a2b3c");
}

static void vq_line()
{
    lg::VqMinute m;
    m.secs = 60;
    m.calls = 18402;
    m.ms = 912.44;
    m.worst_ms = 12.345;
    m.game_calls = 12000;
    m.game_ms = 640.06;
    m.modules = {{"progressive.dll", 3402, 112.2}, {"core", 15000, 800.24}, {"other", 0, 0}};
    m.sites = {{"core+0x2200", 4000, 100.0},
               {"core+0x1a2b3c", 9100, 500.3},
               {"core+0x10", 1, 0.01},
               {"core+0x20", 2, 0.02},
               {"core+0x30", 3, 0.03},
               {"core+0x40", 4, 0.04}};
    m.exe_hooked = true;
    m.exe_calls = 120334;
    m.exe_ms = 30.05;
    m.page_resident = 52301;
    m.page_region = 3;
    CHECK_EQ(lg::vq_line(m),
             "[perf] last 60 s: VirtualQuery from swofty 18,402 calls 912.4 ms, worst 12.35 ms (game threads 12,000 "
             "calls 640.1 ms): core 15,000 calls 800.2 ms, progressive.dll 3,402 calls 112.2 ms; top sites "
             "core+0x1a2b3c 9,100 calls 500.3 ms, core+0x2200 4,000 calls 100.0 ms, core+0x40 4 calls 0.0 ms, "
             "core+0x30 3 calls 0.0 ms, core+0x20 2 calls 0.0 ms; game exe 120,334 calls 30.1 ms; memory checks: 52,301 "
             "page(s) from the working set, 3 from VirtualQuery");

    lg::VqMinute idle;
    idle.secs = 60;
    idle.modules = {{"core", 0, 0}};
    CHECK_EQ(lg::vq_line(idle),
             "[perf] last 60 s: VirtualQuery from swofty 0 calls 0.0 ms (game threads 0 calls 0.0 ms); memory "
             "checks: 0 page(s) from the working set, 0 from VirtualQuery");
}

static void ranked()
{
    std::vector<lg::Count> v = {{"a", 5, 1.0}, {"b", 9, 1.0}, {"c", 0, 9.0}, {"d", 1, 2.0}};
    auto r = lg::ranked(v, 2);
    CHECK(r.size() == 2);
    CHECK(r[0].what == "d");
    CHECK(r[1].what == "b");
}

static void memory_line()
{
    lg::MemoryMinute m;
    m.secs = 60;
    m.system_known = m.process_known = true;
    m.ram_total = 32768ull << 20;
    m.ram_free = 14000ull << 20;
    m.commit_limit = 40000ull << 20;
    m.commit_free = 18000ull << 20;
    m.working_set = 6500ull << 20;
    m.private_bytes = 7200ull << 20;
    lg::GpuMinute g;
    g.known = true;
    g.local_used = 5100ull << 20;
    g.local_budget = 11000ull << 20;
    g.non_local_used = 120ull << 20;
    g.non_local_budget = 7000ull << 20;
    m.gpus = {g, lg::GpuMinute{}};
    CHECK_EQ(lg::memory_line(m), "[perf] last 60 s: memory: RAM 18768/32768 MiB used, commit 22000/40000 MiB used; game working set "
                                 "6500 MiB, private 7200 MiB; VRAM gpu 0 5100/11000 MiB local used/budget, 120/7000 MiB shared; "
                                 "VRAM gpu 1 unavailable");
    lg::MemoryMinute none;
    none.secs = 60;
    CHECK_EQ(lg::memory_line(none), "[perf] last 60 s: memory: system RAM unavailable; VRAM unavailable");
    lg::MemoryMinute odd;
    odd.secs = 60;
    odd.system_known = true;
    odd.ram_total = 1ull << 30;
    odd.ram_free = 2ull << 30;
    CHECK(lg::memory_line(odd).find("RAM 0/1024 MiB used") != std::string::npos);
}

static void boot_lines()
{
    CHECK_EQ(lg::driver_text((31ull << 48) | (0ull << 32) | (15ull << 16) | 4601), "31.0.15.4601");
    CHECK_EQ(lg::gpu_boot_line(0, "NVIDIA GeForce RTX 3060", 0x10de, 0x2504, "31.0.15.4601", 12ull << 30, 0, 16ull << 30, false),
             "[system] gpu 0: NVIDIA GeForce RTX 3060 (vendor 0x10de device 0x2504), driver 31.0.15.4601, dedicated VRAM 12288 MiB, "
             "dedicated system 0 MiB, shared system 16384 MiB");
    CHECK(lg::gpu_boot_line(1, "Basic Render", 0x1414, 0x8c, "", 0, 0, 0, true).find("driver unknown") != std::string::npos);
    CHECK_EQ(lg::ram_boot_line(32768ull << 20, 40000ull << 20),
             "[system] ram: 32768 MiB physical, 40000 MiB commit limit (RAM plus page file)");
}

static void cpu_line()
{
    lg::CpuMinute c;
    c.secs = 60;
    c.user = 1.4249;
    c.kernel = 0.61;
    c.ours_user = 0.061;
    c.ours_kernel = 0.0204;
    c.threads = 7;
    c.top = {{"core+0x5150 (tid 812)", 0.012},
             {"core+0x9a20 (tid 900, via _beginthreadex)", 0.05},
             {"progressive.dll+0x400 (tid 77)", 0.0001},
             {"core+0x6000 (tid 4)", 0.02},
             {"core+0x7000 (tid 5)", 0.001}};
    CHECK_EQ(lg::cpu_line(c), "[perf] last 60 s: process CPU user 1.42 kernel 0.61 cores; swofty threads 0.081 cores "
                              "(user 0.061 kernel 0.020) in 7 thread(s): core+0x9a20 (tid 900, via _beginthreadex) "
                              "0.050, core+0x6000 (tid 4) 0.020, core+0x5150 (tid 812) 0.012");

    lg::CpuMinute quiet;
    quiet.secs = 60;
    quiet.user = 0.5;
    quiet.top = {{"core+0x1 (tid 1)", 0.0001}};
    quiet.threads = 1;
    CHECK_EQ(lg::cpu_line(quiet), "[perf] last 60 s: process CPU user 0.50 kernel 0.00 cores; swofty threads 0.000 "
                                  "cores (user 0.000 kernel 0.000) in 1 thread(s)");
}

static void busiest()
{
    std::vector<lg::Busy> v = {{"core game log (Ne_Log_Thunk)", 23.59, 50484},
                               {"core flow tick (events, career poll)", 60.69, 3600},
                               {"swofty.progressive api storage read", 1.2, 60},
                               {"core hud present", 0.5, 10000}};
    CHECK_EQ(lg::busiest(v, 60, 3), "core flow tick (events, career poll) 1.012 ms/s 60.0 calls/s, core game log "
                                    "(Ne_Log_Thunk) 0.393 ms/s 841.4 calls/s, swofty.progressive api storage read "
                                    "0.020 ms/s 1.0 calls/s");
    CHECK_EQ(lg::busiest({}, 60, 3), "");
    CHECK_EQ(lg::busiest({{"x", 6, 6}}, 0, 3), "x 0.000 ms/s 0.0 calls/s");
}

static void slots()
{
    for (uintptr_t a : {uintptr_t(0x180001234), uintptr_t(0x7ff612345678), uintptr_t(1)})
        CHECK(lg::site_slot(a, 10) < 1024);
    CHECK(lg::site_slot(0x180001234, 10) != lg::site_slot(0x180001235, 10));
}

int main()
{
    grouped();
    deltas();
    vq_line();
    ranked();
    cpu_line();
    memory_line();
    boot_lines();
    busiest();
    slots();
    if (g_fail) {
        std::printf("%d check(s) failed\n", g_fail);
        return 1;
    }
    std::printf("perf logic test: all checks passed\n");
    return 0;
}
