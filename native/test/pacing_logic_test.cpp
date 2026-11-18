// Host-native unit test for the frame pacing of the core's profiler (native/src/core/pacing_logic.hpp): the histograms,
// uneven frames and spikes, the present, GPU, CPU, process and display lines, the verdict and the perf.log pacing rows,
// and the cost of one frame on the hot path. Built and run by native/test/run_pacing_logic_test.sh.

#include "pacing_logic.hpp"

#include <chrono>
#include <cstdio>
#include <string>

namespace pc = perf::pace;

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

static bool contains(const std::string &s, const std::string &part)
{
    if (s.find(part) != std::string::npos)
        return true;
    std::printf("  missing \"%s\" in: %s\n", part.c_str(), s.c_str());
    return false;
}

static void buckets()
{
    CHECK(pc::bucket(0) == 0);
    CHECK(pc::bucket(8.3) == 33);
    CHECK(pc::bucket(99.99) == 399);
    CHECK(pc::bucket(100) == 400);
    CHECK(pc::bucket(1000) == 490);
    CHECK(pc::bucket(20000) == pc::BUCKETS - 1);
    CHECK(pc::bucket_ms(33) == 8.375);
    uint32_t h[pc::BUCKETS] = {};
    CHECK(pc::percentile(h, 0.5) == 0);
    h[pc::bucket(8.3)] = 998;
    h[pc::bucket(20)] = 1;
    h[pc::bucket(60)] = 1;
    CHECK(pc::percentile(h, 0.5) == 8.375);
    CHECK(pc::percentile(h, 0.999) == 20.125);
    CHECK(pc::percentile(h, 1.0) == 60.125);
    CHECK(pc::over(h, 16.75) == 2);
    CHECK(pc::over(h, 50) == 1);
    CHECK(pc::total(h) == 1000);
}

static void even_frames()
{
    pc::Pace p;
    for (int i = 0; i < 1000; i++)
        p.add(8.3 + (i % 2 ? 0.2 : -0.2), i * 0.0083);
    pc::Summary s = pc::summary(p);
    CHECK(s.frames == 1000);
    CHECK(s.uneven == 0);
    CHECK(s.spikes == 0);
    CHECK(s.hitches == 0 && s.pauses == 0 && s.over_2x == 0);
    CHECK(s.p50 > 7.9 && s.p50 < 8.7);
    CHECK(s.change_p99 > 0.3 && s.change_p99 < 0.5);
    CHECK(s.rate() > 115 && s.rate() < 127);
}

static void uneven_and_spikes()
{
    pc::Pace p;
    for (int i = 0; i < 600; i++)
        p.add(i % 3 == 2 ? 16.6 : 8.3, i * 0.01);
    CHECK(p.uneven == 399);
    CHECK(p.spikes == 0);
    pc::Pace q;
    for (int i = 0; i < 200; i++)
        q.add(8.0, i * 0.008);
    q.add(30.0, 1.6);
    q.add(8.0, 1.63);
    q.add(13.0, 1.64);
    q.add(70.0, 1.7);
    q.add(600.0, 2.3);
    q.add(8.0, 2.31);
    CHECK(q.spikes == 2);
    std::vector<pc::Spike> order = q.spikes_in_order();
    CHECK(order.size() == 2 && order[0].ms == 30.0 && order[1].ms == 70.0);
    CHECK(order[0].base == 8.0);
    pc::Summary s = pc::summary(q);
    CHECK(s.hitches == 1);
    CHECK(s.pauses == 1);
    CHECK(s.max == 600.0);
    CHECK(q.avg < 20);
}

static void top_spikes()
{
    pc::Pace p;
    p.add(8, 0);
    for (int i = 0; i < 20; i++) {
        p.add(20.0 + i, i);
        for (int k = 0; k < 40; k++)
            p.add(8, i + k * 0.01);
    }
    CHECK(p.spikes == 20);
    CHECK(p.tops == pc::TOP);
    std::vector<pc::Spike> v = p.spikes_in_order();
    CHECK(v.front().ms == 32.0 && v.back().ms == 39.0);
    p.restart();
    CHECK(p.frames == 0 && p.spikes == 0 && p.tops == 0 && p.prev == 8 && p.avg > 7.9);
}

static void pacing_line()
{
    pc::Pace p;
    for (int i = 0; i < 7000; i++)
        p.add(8.3, i * 0.0083);
    p.add(25.0, 30.0);
    pc::Summary s = pc::summary(p);
    std::string l = pc::pacing_line(60, s, {{"14:02:33", "screen garage", 25.0, 8.3}, {"14:02:51", "", 18.04, 8.21}});
    CHECK_EQ(l, "[perf] last 60 s: pacing: 7001 frame(s) 119 fps; frame times p50 8.38 ms, p99 8.38, p99.9 8.38, max 25.0 ms; "
                "0.0% uneven (over 20% and 1 ms off the previous one), change p99 0.12 ms; 1 over 2x the median, 0 over 50 "
                "ms, 0 over 250 ms; 1 spike(s) over 2x the running average: 14:02:33 25.0 ms (average 8.3) during \"screen "
                "garage\", 14:02:51 18.0 ms (average 8.2)");
    CHECK_EQ(pc::pacing_line(60, pc::Summary{}, {}), "[perf] last 60 s: pacing: no frames");
}

static void presents_line()
{
    pc::Presents p;
    CHECK(p.sync_mode() == -1);
    CHECK_EQ(pc::presents_line(60, p, 100), "[perf] last 60 s: presents: none seen (the present hook is off, or the game drew nothing)");
    for (int i = 0; i < 6000; i++)
        p.add(i ? 10.0 : 0, 0.2, 0, true, 0, i * 0.01);
    p.add(10.0, 3.0, 1, false, pc::DXGI_OCCLUDED, 60);
    p.add(10.0, 0.2, 0, true, 0x887A0005, 60);
    p.other = 12;
    CHECK(p.sync_mode() == 0);
    CHECK(p.count == 6002 && p.pace.frames == 6001);
    std::string l = pc::presents_line(60, p, 6002);
    CHECK_EQ(l, "[perf] last 60 s: presents: 6002 (100.0/s, 1.00 per game frame); intervals p50 10.12 ms, p99 10.12, p99.9 "
                "10.12, max 10.0 ms, 0.0% uneven (over 20% and 1 ms off the previous one), change p99 0.12 ms, 0 over 2x the "
                "median; inside Present average 0.20 ms, max 3.00 ms; sync interval 0 (VSync off) in 100%, tearing flag in "
                "100%; 12 from other swapchains; 1 occluded; 1 failed (last 0x887a0005)");
    pc::Presents q;
    for (int i = 0; i < 10; i++)
        q.add(16.7, 4, 1, false, 0, i);
    CHECK(contains(pc::presents_line(10, q, 5), "2.00 per game frame"));
    CHECK(contains(pc::presents_line(10, q, 5), "sync interval 1 (VSync on) in 100%, tearing flag in 0%"));
    q.restart();
    CHECK(q.count == 0 && q.pace.frames == 0 && q.pace.prev == 16.7);
    CHECK_EQ(pc::sync_text(2), "2 (VSync on, every 2nd refresh)");
    CHECK_EQ(pc::sync_text(4), "4 or more (VSync on, every 4th or later refresh)");
}

static void gpu_line()
{
    pc::GpuMinute g;
    CHECK_EQ(pc::gpu_line(60, 0, g), "[perf] last 60 s: gpu 0: clocks and sensors unavailable; 3D engine use unavailable");
    for (double c : {2430.0, 2510.0, 2480.0}) {
        pc::GpuSample s;
        s.core = c, s.core_max = 2600, s.mem = 2250, s.temp = c > 2500 ? 71 : 63, s.power = 90, s.fan = 1450;
        s.util = true, s.busy = c > 2500 ? 99 : 96, s.game = 94, s.others = 3, s.other_top = "wallpaper64.exe 3%";
        g.add(s);
    }
    CHECK_EQ(pc::gpu_line(60, 0, g), "[perf] last 60 s: gpu 0: core 2430-2510 MHz (max 2600), memory 2250 MHz, 63-71 C, power 90%, "
                                     "fan 1450 RPM; 3D engine busy 96-99% (average 97%), the game 94%, other processes 3% (most "
                                     "wallpaper64.exe 3%)");
}

static void cpu_line()
{
    pc::SysCpuMinute c;
    CHECK_EQ(pc::cpu_line(60, c), "[perf] last 60 s: cpu: load unavailable");
    c.add({31, 88, 4200, 4200, 4200});
    c.add({44, 100, 4200, 4200, 3600});
    c.game_thread = 0.97;
    c.game_thread_what = "tid 812";
    CHECK_EQ(pc::cpu_line(60, c), "[perf] last 60 s: cpu: system load 31-44% (average 38%), busiest core 88-100% (average 94%); "
                                  "reported clocks 4200 MHz of 4200, limit 3600-4200 MHz; busiest game thread 0.97 cores (tid 812)");
}

static void others_line()
{
    CHECK(pc::known_tool("Discord.exe") && pc::known_tool("obs64.exe") && !pc::known_tool("notepad.exe"));
    CHECK(pc::known_module("RTSSHooks64.dll") && !pc::known_module("kernel32.dll"));
    CHECK_EQ(pc::others_line(60, 1.351, {{"simhub.exe", 0.22}, {"chrome.exe", 0.41}, {"idle.exe", 0.01}}, {"Discord.exe", "obs64.exe"},
                             {"DiscordHook64.dll"}),
             "[perf] last 60 s: other processes: 1.35 cores in total; busiest chrome.exe 0.41, simhub.exe 0.22; overlay, capture "
             "and monitoring tools running: Discord.exe, obs64.exe; overlay modules in the game: DiscordHook64.dll");
    CHECK_EQ(pc::others_line(60, -1, {}, {}, {}), "[perf] last 60 s: other processes: CPU use unavailable");
    CHECK_EQ(pc::others_line(60, 0.2, {}, {}, {}),
             "[perf] last 60 s: other processes: 0.20 cores in total; overlay, capture and monitoring tools running: none known");
}

static void engines()
{
    pc::Engine e = pc::engine("pid_1234_luid_0x00000000_0x0000C3A1_phys_0_eng_3_engtype_3D");
    CHECK(e.ok && e.pid == 1234 && e.luid_high == 0 && e.luid_low == 0xc3a1 && e.phys == 0 && e.eng == 3 && e.type == "3D");
    CHECK(pc::engine("pid_9_luid_0x00000001_0x00001234_phys_1_eng_12_engtype_VideoDecode").type == "VideoDecode");
    CHECK(!pc::engine("pid_x_luid").ok);
    CHECK(!pc::engine("_Total").ok);
    CHECK(!pc::engine("pid_1_luid_0x0_0x1_phys_0_eng_0").ok);
    auto use = [](const char *name, double pct) { return pc::EngineUse{pc::engine(name), pct}; };
    std::vector<pc::EngineUse> v = {
        use("pid_100_luid_0x00000000_0x0000C3A1_phys_0_eng_0_engtype_3D", 90),
        use("pid_200_luid_0x00000000_0x0000C3A1_phys_0_eng_0_engtype_3D", 6),
        use("pid_300_luid_0x00000000_0x0000C3A1_phys_0_eng_0_engtype_3D", 1),
        use("pid_100_luid_0x00000000_0x0000C3A1_phys_0_eng_1_engtype_3D", 30),
        use("pid_100_luid_0x00000000_0x0000C3A1_phys_0_eng_5_engtype_Copy", 99),
        use("pid_400_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D", 99),
    };
    pc::Busy3d b = pc::busy_3d(v, 100, true, 0, 0xc3a1);
    CHECK(b.known && b.busy == 97 && b.game == 90 && b.others == 7 && b.top_pid == 200 && b.top_pct == 6);
    pc::Busy3d any = pc::busy_3d(v, 100, false, 0, 0);
    CHECK(any.busy == 99 && any.game == 0 && any.top_pid == 400);
    CHECK(!pc::busy_3d({}, 1, false, 0, 0).known);
    std::vector<pc::EngineUse> over = {use("pid_1_luid_0x00000000_0x00000001_phys_0_eng_0_engtype_3D", 70),
                                       use("pid_2_luid_0x00000000_0x00000001_phys_0_eng_0_engtype_3D", 50)};
    CHECK(pc::busy_3d(over, 1, false, 0, 0).busy == 100);
    CHECK(pc::busy_3d(over, 1, false, 0, 0).others == 30);
}

static void display_lines()
{
    pc::Mode m{"\\\\.\\DISPLAY1", "Generic PnP Monitor", 2560, 1440, 144, true};
    CHECK_EQ(pc::display_boot_line(0, m), "[system] display 0: \\\\.\\DISPLAY1 (Generic PnP Monitor) 2560x1440 at 144 Hz, primary");
    pc::Window w;
    pc::Chain c;
    CHECK_EQ(pc::display_line(w, c), "[perf] display: the game's window is unknown; swapchain unknown");
    w.known = true, w.covers = true, w.on = m;
    c.known = true, c.w = 2560, c.h = 1440, c.format = 24, c.buffers = 3, c.effect = 4, c.flags = 0x840, c.windowed = true;
    CHECK_EQ(pc::display_line(w, c), "[perf] display: the game's window is on \\\\.\\DISPLAY1 (2560x1440 at 144 Hz), covering it "
                                     "without a border (borderless or exclusive fullscreen); swapchain 2560x1440 format 24, 3 "
                                     "buffers, flip discard, windowed, tearing allowed, frame latency waitable");
    w.covers = false;
    c.windowed = false, c.flags = 0, c.effect = 0;
    CHECK(contains(pc::display_line(w, c), "smaller than the display (windowed); swapchain 2560x1440 format 24, 3 buffers, discard "
                                           "(bitblt), exclusive fullscreen"));
}

static void system_lines()
{
    CHECK_EQ(pc::plan_name("381B4222-F694-41F0-9685-FF5BB260DF2E"), "Balanced");
    CHECK_EQ(pc::plan_name("8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"), "High performance");
    CHECK_EQ(pc::plan_name("12345678-0000-0000-0000-000000000000"), "custom (12345678-0000-0000-0000-000000000000)");
    CHECK_EQ(pc::overlay_name("ded574b5-45a0-4f42-8737-46345c09c238"), "Best performance");
    CHECK_EQ(pc::clean("~ RUNASADMIN C:\\Users\\Bob\\x <y>"), "~ RUNASADMIN CUsersBobx y");
    pc::Windows w;
    w.plan = "Balanced", w.overlay = "Best performance", w.game_mode = 1, w.game_dvr = 0, w.hags_setting = 2;
    w.dx_global = "SwapEffectUpgradeEnable=1;", w.priority = "high", w.affinity = 16, w.logical = 16;
    w.timer_ms = 1.0, w.timer_coarse = 15.625, w.timer_fine = 0.5;
    CHECK_EQ(pc::windows_line(w), "[system] windows: power plan Balanced, power mode Best performance; game mode on; Game Bar "
                                  "recording off, background capture not set; hardware-accelerated GPU scheduling setting on; "
                                  "DirectX graphics settings SwapEffectUpgradeEnable=1; game exe compatibility flags none; "
                                  "process priority high, affinity 16 of 16 logical processors; timer resolution 1.00 ms (15.62 "
                                  "to 0.50)");
    CHECK_EQ(pc::wddm_text(3100), "3.1");
    CHECK_EQ(pc::wddm_text(2700), "2.7");
    CHECK_EQ(pc::scheduling_line(0, 3100, 3, 1), "[system] gpu 0: WDDM 3.1, hardware-accelerated GPU scheduling on (supported), "
                                                 "tearing (variable refresh) presents supported");
    CHECK_EQ(pc::scheduling_line(1, 0, -1, -1), "[system] gpu 1: WDDM unknown, hardware-accelerated GPU scheduling unknown, "
                                                "tearing (variable refresh) presents not set");
    CHECK(contains(pc::scheduling_line(0, 2700, 1, 0), "scheduling off (supported), tearing (variable refresh) presents not supported"));
}

static void settings_xml()
{
    std::string xml = "<?xml version=\"1.0\"?>\n<hardware_settings_config version=\"53\">\n\t<resolution width=\"2560\" "
                      "height=\"1440\" fullscreen=\"true\" vsync=\"0\" refreshRate=\"144\">\n\t\t<hdr mode=\"0\" />\n\t"
                      "</resolution>\n\t<frame_gen mode=\"1\" />\n\t<multi_frame_gen value=\"0\" />\n\t<shadows quality=\"3\" />"
                      "\n\t<framerateLimit enabled=\"true\" value=\"120\"/>\n\t<reflex mode=\"2\"/>\n</hardware_settings_config>";
    CHECK_EQ(pc::game_display_settings(xml), "resolution(width=2560 height=1440 fullscreen=true vsync=0 refreshRate=144) "
                                             "frame_gen(mode=1) multi_frame_gen(value=0) frameratelimit(enabled=true value=120) "
                                             "reflex(mode=2)");
    CHECK_EQ(pc::game_display_settings(""), "");
    CHECK_EQ(pc::game_display_settings("<resolution width=\"1"), "");
}

static pc::Summary frames(uint64_t n, uint64_t uneven, uint64_t spikes, uint64_t hitches)
{
    pc::Summary s;
    s.frames = n, s.uneven = uneven, s.spikes = spikes, s.hitches = hitches, s.p50 = 8.3;
    return s;
}

static void verdicts()
{
    pc::Facts f;
    f.secs = 60;
    f.frames = frames(10, 0, 0, 0);
    CHECK_EQ(pc::verdict(f).text, "too few frames to judge (10)");
    f.frames = frames(7000, 50, 1, 0);
    f.presents = true;
    f.present = frames(7000, 40, 0, 0);
    f.present_rate = 117, f.sync = 0, f.refresh = 144, f.gpu_busy = 98;
    pc::Verdict v = pc::verdict(f);
    CHECK(v.smooth && v.gpu && v.display && !v.judder && !v.stutter && !v.cpu && !v.background);
    CHECK_EQ(v.text, "smooth in frame times; GPU bound (3D engine 98% busy); display mismatch (VSync off at 117 presents/s on a 144 "
                     "Hz display: tearing and uneven motion unless FreeSync or G-Sync is on)");
    CHECK_EQ(pc::verdict_tags(v), "smooth, GPU bound, display mismatch");
    f.refresh = 120, f.present_rate = 118;
    CHECK(!pc::verdict(f).display);
    f.sync = 1, f.refresh = 144, f.present_rate = 100;
    CHECK(contains(pc::verdict(f).text, "VSync on at 100 presents/s on a 144 Hz display: frames stay up for 1 or 2 refreshes"));
    f.sync = 2, f.refresh = 144, f.present_rate = 71;
    CHECK(!pc::verdict(f).display);
    f.frames = frames(7000, 900, 9, 2);
    f.gpu_busy = 60, f.game_thread = 0.95, f.other_cores = 1.5, f.other_gpu = 12;
    v = pc::verdict(f);
    CHECK(v.stutter && v.judder && !v.smooth && v.cpu && !v.gpu && v.background);
    CHECK(contains(v.text, "stutter (2 frame(s) over 50 ms, 9 spike(s) over 2x the running average)"));
    CHECK(contains(v.text, "judder (13% of frames and 1% of presents over 20% off the previous one)"));
    CHECK(contains(v.text, "CPU bound (busiest game thread 0.95 cores)"));
    CHECK(contains(v.text, "background load (1.50 cores and 12% of the 3D engine in other processes)"));
    f.frames = frames(3500, 0, 3, 0);
    f.secs = 30, f.game_thread = 0.5, f.other_cores = 0.1, f.other_gpu = 0;
    CHECK(pc::verdict(f).stutter);
    pc::Tally t;
    t.add(v);
    t.add(pc::verdict(f));
    CHECK_EQ(t.text(), "2 minute(s): 0 smooth in frame times, 2 stutter, 1 judder; 0 GPU bound, 1 CPU bound; 0 display mismatch; 1 "
                       "background load");
    CHECK_EQ(pc::verdict_line(60, v).substr(0, 35), "[perf] last 60 s: verdict: stutter ");
}

static void rows()
{
    pc::Row r;
    r.when = "14:02:00", r.tags = "smooth, GPU bound";
    r.frames = frames(7000, 70, 2, 0);
    r.frames.p99 = 10.25, r.frames.p999 = 14.75, r.frames.max = 21.4;
    r.presents = true;
    r.present.p99 = 10.75;
    r.inside_avg = 0.21, r.gpu_core = 2480, r.gpu_temp = 71, r.gpu_busy = 97, r.cpu_busiest = 92;
    std::string h = pc::pacing_header(), row = pc::pacing_row(r);
    CHECK_EQ(h, "time       fps  p50 ms  p99 ms p99.9 ms  max ms  uneven spikes pres p99  in pres  gpu MHz  gpu C 3D busy cpu max  "
                "verdict");
    CHECK_EQ(row, "14:02:00   120    8.30   10.25    14.75    21.4    1.0%      2    10.75     0.21     2480     71     97%     92%  "
                  "smooth, GPU bound");
    pc::Row none;
    none.when = "14:03:00", none.tags = "-";
    CHECK(contains(pc::pacing_row(none), "       -        -        -      -       -       -  -"));
}

static void cost()
{
    static pc::Pace p;
    static pc::Presents q;
    const int N = 2000000;
    auto t0 = std::chrono::steady_clock::now();
    for (int i = 0; i < N; i++) {
        double ms = 8.0 + double(i % 7) * 0.3 + (i % 997 == 0 ? 20 : 0);
        p.add(ms, i * 0.008);
        q.add(ms, 0.2, 0, true, 0, i * 0.008);
    }
    double ns = std::chrono::duration<double, std::nano>(std::chrono::steady_clock::now() - t0).count() / N;
    std::printf("pacing cost: %.1f ns per game frame plus present (%llu spikes kept of %llu)\n", ns,
                (unsigned long long)p.tops, (unsigned long long)p.spikes);
    CHECK(ns < 2000);
}

int main()
{
    buckets();
    even_frames();
    uneven_and_spikes();
    top_spikes();
    pacing_line();
    presents_line();
    gpu_line();
    cpu_line();
    others_line();
    engines();
    display_lines();
    system_lines();
    settings_xml();
    verdicts();
    rows();
    cost();
    if (g_fail) {
        std::printf("%d check(s) failed\n", g_fail);
        return 1;
    }
    std::printf("pacing logic test: all checks passed\n");
    return 0;
}
