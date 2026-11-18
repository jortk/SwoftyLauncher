#pragma once

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <string>
#include <string_view>
#include <vector>

// Frame pacing for the core's profiler (perf.cpp): the frame-time histograms, the frame-to-frame changes, uneven frames
// and spikes of a run of game frames or presents, the GPU, CPU, process and display facts of a minute, the [perf] and
// [system] lines that show them and the verdict a minute gets. No Windows calls, so the unit test runs on the host
// (native/test/pacing_logic_test.cpp).

namespace perf::pace {

constexpr int BUCKETS = 581;
constexpr double UNEVEN_SHARE = 0.2, UNEVEN_FLOOR_MS = 1.0;
constexpr double SPIKE_TIMES = 2.0, SPIKE_FLOOR_MS = 4.0;
constexpr double HITCH_MS = 50.0, PAUSE_MS = 250.0;
constexpr int TOP = 8;
constexpr uint32_t DXGI_OCCLUDED = 0x087A0001;

inline int bucket(double ms)
{
    if (ms < 100)
        return std::max(0, int(ms * 4));
    if (ms < 1000)
        return 400 + int((ms - 100) / 10);
    if (ms < 10000)
        return 490 + int((ms - 1000) / 100);
    return BUCKETS - 1;
}

inline double bucket_ms(int b)
{
    if (b < 400)
        return (b + 0.5) / 4;
    if (b < 490)
        return 100 + (b - 400 + 0.5) * 10;
    if (b < BUCKETS - 1)
        return 1000 + (b - 490 + 0.5) * 100;
    return 10000;
}

inline uint64_t total(const uint32_t *h)
{
    uint64_t n = 0;
    for (int b = 0; b < BUCKETS; b++)
        n += h[b];
    return n;
}

inline double percentile(const uint32_t *h, double p)
{
    uint64_t n = total(h);
    if (!n)
        return 0;
    uint64_t want = std::max<uint64_t>(1, uint64_t(std::ceil(p * double(n)))), seen = 0;
    for (int b = 0; b < BUCKETS; b++)
        if ((seen += h[b]) >= want)
            return bucket_ms(b);
    return bucket_ms(BUCKETS - 1);
}

inline uint64_t over(const uint32_t *h, double ms)
{
    uint64_t n = 0;
    for (int b = bucket(ms) + 1; b < BUCKETS; b++)
        n += h[b];
    return n;
}

struct Spike {
    double at = 0, ms = 0, base = 0;
};

struct Pace {
    uint32_t hist[BUCKETS] = {}, change[BUCKETS] = {};
    uint64_t frames = 0, uneven = 0, spikes = 0;
    double max = 0, prev = 0, avg = 0;
    Spike top[TOP] = {};
    int tops = 0;

    void add(double ms, double at)
    {
        hist[bucket(ms)]++;
        frames++;
        if (ms > max)
            max = ms;
        bool pause = ms > PAUSE_MS;
        if (prev > 0 && !pause && prev <= PAUSE_MS) {
            double d = std::fabs(ms - prev);
            change[bucket(d)]++;
            if (d > std::max(prev * UNEVEN_SHARE, UNEVEN_FLOOR_MS))
                uneven++;
        }
        if (avg > 0 && !pause && ms > avg * SPIKE_TIMES && ms > avg + SPIKE_FLOOR_MS)
            keep({at, ms, avg});
        if (!pause)
            avg = avg > 0 ? avg + (ms - avg) / 16 : ms;
        prev = ms;
    }

    void keep(const Spike &s)
    {
        spikes++;
        if (tops < TOP) {
            top[tops++] = s;
            return;
        }
        int low = 0;
        for (int i = 1; i < TOP; i++)
            if (top[i].ms < top[low].ms)
                low = i;
        if (s.ms > top[low].ms)
            top[low] = s;
    }

    void restart()
    {
        double p = prev, a = avg;
        *this = Pace();
        prev = p, avg = a;
    }

    std::vector<Spike> spikes_in_order() const
    {
        std::vector<Spike> v(top, top + tops);
        std::sort(v.begin(), v.end(), [](const Spike &a, const Spike &b) { return a.at < b.at; });
        return v;
    }
};

struct Summary {
    uint64_t frames = 0, uneven = 0, spikes = 0, over_2x = 0, hitches = 0, pauses = 0;
    double p50 = 0, p99 = 0, p999 = 0, max = 0, change_p99 = 0;

    double share(uint64_t n) const
    {
        return frames ? 100.0 * double(n) / double(frames) : 0;
    }
    double rate() const
    {
        return p50 > 0 ? 1000 / p50 : 0;
    }
};

inline Summary summary(const Pace &p)
{
    Summary s;
    s.frames = p.frames;
    s.uneven = p.uneven;
    s.spikes = p.spikes;
    s.max = p.max;
    s.p50 = percentile(p.hist, 0.5);
    s.p99 = percentile(p.hist, 0.99);
    s.p999 = percentile(p.hist, 0.999);
    s.change_p99 = percentile(p.change, 0.99);
    s.over_2x = s.p50 > 0 ? over(p.hist, 2 * s.p50) : 0;
    s.pauses = over(p.hist, PAUSE_MS);
    s.hitches = over(p.hist, HITCH_MS) - s.pauses;
    return s;
}

struct Presents {
    Pace pace;
    uint64_t count = 0, other = 0, tearing = 0, failed = 0, occluded = 0;
    uint64_t sync[5] = {};
    double inside_sum = 0, inside_max = 0;
    uint32_t failed_hr = 0;

    void add(double interval_ms, double inside_ms, unsigned sync_interval, bool tear, uint32_t hr, double at)
    {
        count++;
        if (interval_ms > 0)
            pace.add(interval_ms, at);
        inside_sum += inside_ms;
        if (inside_ms > inside_max)
            inside_max = inside_ms;
        sync[std::min(sync_interval, 4u)]++;
        tearing += tear;
        if (hr == DXGI_OCCLUDED)
            occluded++;
        else if (int32_t(hr) < 0)
            failed++, failed_hr = hr;
    }

    void restart()
    {
        Pace p = pace;
        *this = Presents();
        pace = p;
        pace.restart();
    }

    int sync_mode() const
    {
        if (!count)
            return -1;
        int best = 0;
        for (int i = 1; i < 5; i++)
            if (sync[i] > sync[best])
                best = i;
        return best;
    }

    double inside_avg() const
    {
        return count ? inside_sum / double(count) : 0;
    }
};

inline std::string num(const char *f, double v)
{
    char b[64];
    std::snprintf(b, sizeof b, f, v);
    return b;
}

inline std::string count(uint64_t n)
{
    return std::to_string(n);
}

inline std::string head(double secs)
{
    return "[perf] last " + num("%.0f", secs) + " s: ";
}

inline std::string sync_text(int sync)
{
    switch (sync) {
    case -1:
        return "unknown";
    case 0:
        return "0 (VSync off)";
    case 1:
        return "1 (VSync on)";
    default:
        return std::to_string(sync) + (sync >= 4 ? " or more" : "") + " (VSync on, every " + std::to_string(sync) +
               (sync >= 4 ? "th or later" : sync == 2 ? "nd" : "rd") + " refresh)";
    }
}

struct SpikeRow {
    std::string when, during;
    double ms = 0, base = 0;
};

inline std::string spikes_text(const std::vector<SpikeRow> &rows)
{
    std::string out;
    for (const SpikeRow &r : rows)
        out += (out.empty() ? "" : ", ") + r.when + " " + num("%.1f", r.ms) + " ms (average " + num("%.1f", r.base) +
               ")" + (r.during.empty() ? "" : " during \"" + r.during + "\"");
    return out;
}

inline std::string times_text(const Summary &s)
{
    return "p50 " + num("%.2f", s.p50) + " ms, p99 " + num("%.2f", s.p99) + ", p99.9 " + num("%.2f", s.p999) + ", max " +
           num("%.1f", s.max) + " ms";
}

inline std::string uneven_text(const Summary &s)
{
    return num("%.1f", s.share(s.uneven)) + "% uneven (over 20% and 1 ms off the previous one), change p99 " +
           num("%.2f", s.change_p99) + " ms";
}

inline std::string pacing_text(const Summary &s, const std::vector<SpikeRow> &rows)
{
    std::string out = "pacing: ";
    if (!s.frames)
        return out + "no frames";
    out += std::to_string(s.frames) + " frame(s) " + num("%.0f", s.rate()) + " fps; frame times " + times_text(s) + "; " +
           uneven_text(s) + "; " + count(s.over_2x) + " over 2x the median, " + count(s.hitches) + " over 50 ms, " +
           count(s.pauses) + " over 250 ms; " + count(s.spikes) + " spike(s) over 2x the running average";
    if (!rows.empty())
        out += ": " + spikes_text(rows);
    return out;
}

inline std::string presents_text(double secs, const Presents &p, uint64_t game_frames)
{
    std::string out = "presents: ";
    if (!p.count)
        return out + "none seen (the present hook is off, or the game drew nothing)";
    Summary s = summary(p.pace);
    out += count(p.count) + " (" + num("%.1f", secs > 0 ? double(p.count) / secs : 0) + "/s";
    if (game_frames)
        out += ", " + num("%.2f", double(p.count) / double(game_frames)) + " per game frame";
    out += "); intervals " + times_text(s) + ", " + uneven_text(s) + ", " + count(s.over_2x) + " over 2x the median";
    out += "; inside Present average " + num("%.2f", p.inside_avg()) + " ms, max " + num("%.2f", p.inside_max) + " ms";
    out += "; sync interval " + sync_text(p.sync_mode()) + " in " + num("%.0f", 100.0 * double(p.sync[std::max(0, p.sync_mode())]) / double(p.count)) +
           "%, tearing flag in " + num("%.0f", 100.0 * double(p.tearing) / double(p.count)) + "%";
    if (p.other)
        out += "; " + count(p.other) + " from other swapchains";
    if (p.occluded)
        out += "; " + count(p.occluded) + " occluded";
    if (p.failed) {
        char b[48];
        std::snprintf(b, sizeof b, " (last 0x%08x)", p.failed_hr);
        out += "; " + count(p.failed) + " failed" + b;
    }
    return out;
}

inline std::string pacing_line(double secs, const Summary &s, const std::vector<SpikeRow> &rows)
{
    return head(secs) + pacing_text(s, rows);
}

inline std::string presents_line(double secs, const Presents &p, uint64_t game_frames)
{
    return head(secs) + presents_text(secs, p, game_frames);
}

struct Range {
    double lo = 0, hi = 0, sum = 0;
    uint32_t n = 0;

    void add(double v)
    {
        if (!n || v < lo)
            lo = v;
        if (!n || v > hi)
            hi = v;
        sum += v;
        n++;
    }
    double avg() const
    {
        return n ? sum / n : 0;
    }
};

inline std::string range_text(const Range &r, const char *f, const char *unit)
{
    std::string lo = num(f, r.lo), hi = num(f, r.hi);
    return (lo == hi ? lo : lo + "-" + hi) + unit;
}

struct GpuSample {
    double core = 0, core_max = 0, mem = 0, temp = 0, power = 0, fan = 0;
    bool util = false;
    double busy = 0, game = 0, others = 0;
    std::string other_top;
};

struct GpuMinute {
    Range core, mem, temp, power, fan, busy, game, others;
    double core_max = 0;
    std::string other_top;
    double other_top_pct = 0;

    void add(const GpuSample &s)
    {
        if (s.core > 0)
            core.add(s.core);
        if (s.core_max > core_max)
            core_max = s.core_max;
        if (s.mem > 0)
            mem.add(s.mem);
        if (s.temp > 0)
            temp.add(s.temp);
        if (s.power > 0)
            power.add(s.power);
        if (s.fan > 0)
            fan.add(s.fan);
        if (s.util) {
            busy.add(s.busy);
            game.add(s.game);
            others.add(s.others);
            if (!s.other_top.empty() && s.others >= other_top_pct)
                other_top = s.other_top, other_top_pct = s.others;
        }
    }
};

inline std::string gpu_text(unsigned index, const GpuMinute &g)
{
    std::string out = "gpu " + std::to_string(index) + ": ";
    std::string sensors;
    auto put = [&](const std::string &s) { sensors += (sensors.empty() ? "" : ", ") + s; };
    if (g.core.n)
        put("core " + range_text(g.core, "%.0f", " MHz") + (g.core_max > 0 ? " (max " + num("%.0f", g.core_max) + ")" : ""));
    if (g.mem.n)
        put("memory " + range_text(g.mem, "%.0f", " MHz"));
    if (g.temp.n)
        put(range_text(g.temp, "%.0f", " C"));
    if (g.power.n)
        put("power " + range_text(g.power, "%.0f", "%"));
    if (g.fan.n)
        put("fan " + range_text(g.fan, "%.0f", " RPM"));
    out += sensors.empty() ? "clocks and sensors unavailable" : sensors;
    if (!g.busy.n)
        return out + "; 3D engine use unavailable";
    out += "; 3D engine busy " + range_text(g.busy, "%.0f", "%") + " (average " + num("%.0f", g.busy.avg()) + "%), the game " +
           num("%.0f", g.game.avg()) + "%, other processes " + num("%.0f", g.others.avg()) + "%";
    if (!g.other_top.empty() && g.other_top_pct >= 1)
        out += " (most " + g.other_top + ")";
    return out;
}

inline std::string gpu_line(double secs, unsigned index, const GpuMinute &g)
{
    return head(secs) + gpu_text(index, g);
}

struct CpuSample {
    double load = -1, busiest = -1, mhz = 0, max_mhz = 0, limit_mhz = 0;
};

struct SysCpuMinute {
    Range load, busiest, mhz, limit;
    double max_mhz = 0, game_thread = -1;
    std::string game_thread_what;

    void add(const CpuSample &s)
    {
        if (s.load >= 0)
            load.add(s.load);
        if (s.busiest >= 0)
            busiest.add(s.busiest);
        if (s.mhz > 0)
            mhz.add(s.mhz);
        if (s.limit_mhz > 0)
            limit.add(s.limit_mhz);
        if (s.max_mhz > max_mhz)
            max_mhz = s.max_mhz;
    }
};

inline std::string cpu_text(const SysCpuMinute &c)
{
    std::string out = "cpu: ";
    if (c.load.n)
        out += "system load " + range_text(c.load, "%.0f", "%") + " (average " + num("%.0f", c.load.avg()) + "%), busiest core " +
               range_text(c.busiest, "%.0f", "%") + " (average " + num("%.0f", c.busiest.avg()) + "%)";
    else
        out += "load unavailable";
    if (c.mhz.n)
        out += "; reported clocks " + range_text(c.mhz, "%.0f", " MHz") + " of " + num("%.0f", c.max_mhz) + ", limit " +
               range_text(c.limit, "%.0f", " MHz");
    if (c.game_thread >= 0)
        out += "; busiest game thread " + num("%.2f", c.game_thread) + " cores" +
               (c.game_thread_what.empty() ? "" : " (" + c.game_thread_what + ")");
    return out;
}

inline std::string cpu_line(double secs, const SysCpuMinute &c)
{
    return head(secs) + cpu_text(c);
}

struct Hog {
    std::string name;
    double cores = 0;
};

inline std::string lower(std::string_view s)
{
    std::string out(s);
    for (char &c : out)
        if (c >= 'A' && c <= 'Z')
            c = char(c - 'A' + 'a');
    return out;
}

constexpr std::string_view TOOLS[] = {
    "discord.exe", "obs64.exe", "obs32.exe", "streamlabs obs.exe", "rtss.exe", "msiafterburner.exe",
    "radeonsoftware.exe", "amdrsserv.exe", "amdow.exe", "nvidia overlay.exe", "nvidia share.exe", "nvidia app.exe",
    "gamebar.exe", "gamebarftserver.exe", "xboxgamebarwidgets.exe", "medal.exe", "overwolf.exe", "outplayed.exe",
    "wallpaper32.exe", "wallpaper64.exe", "lively.exe", "icue.exe", "lghub.exe", "lghub_agent.exe",
    "razer synapse service.exe", "armourycrate.service.exe", "lightingservice.exe", "nzxt cam.exe", "signalrgb.exe",
    "hwinfo64.exe", "hwinfo32.exe", "aida64.exe", "hwmonitor.exe", "cpuz.exe", "fpsmonitor.exe", "capframex.exe",
    "presentmon.exe", "simhub.exe", "simhubwpf.exe", "crewchief.exe", "crewchiefv4.exe", "racelab.exe", "steelseriesgg.exe",
    "action.exe", "bdcam.exe", "fraps.exe", "sharex.exe", "geforcenow.exe", "parsecd.exe", "moonlight.exe",
    "sunshine.exe", "steamwebhelper.exe", "msedgewebview2.exe"};

constexpr std::string_view MODULES[] = {
    "rtsshooks64.dll", "discordhook64.dll", "gameoverlayrenderer64.dll", "graphics-hook64.dll", "obs-graphics-hook64.dll",
    "nvspcap64.dll", "ow-graphics-hook64.dll", "owclient.dll", "medal-hook64.dll", "eosovh-win64-shipping.dll",
    "fraps64.dll", "action_x64.dll", "bdcamvk64.dll", "reshade64.dll", "specialk64.dll", "amdxcffx64.dll",
    "gameinput.dll", "mirillis.dll", "xsplit.gamesource64.dll"};

inline bool known(std::string_view lowered, const std::string_view *list, size_t n)
{
    for (size_t i = 0; i < n; i++)
        if (list[i] == lowered)
            return true;
    return false;
}

inline bool known_tool(std::string_view name)
{
    return known(lower(name), TOOLS, std::size(TOOLS));
}

inline bool known_module(std::string_view name)
{
    return known(lower(name), MODULES, std::size(MODULES));
}

inline std::string names_text(const std::vector<std::string> &v)
{
    std::string out;
    for (const std::string &s : v)
        out += (out.empty() ? "" : ", ") + s;
    return out;
}

inline std::string others_line(double secs, double total_cores, std::vector<Hog> top, const std::vector<std::string> &tools,
                               const std::vector<std::string> &modules)
{
    std::string out = head(secs) + "other processes: ";
    if (total_cores < 0)
        return out + "CPU use unavailable";
    out += num("%.2f", total_cores) + " cores in total";
    std::stable_sort(top.begin(), top.end(), [](const Hog &a, const Hog &b) { return a.cores > b.cores; });
    std::string list;
    for (size_t i = 0; i < top.size() && i < 5; i++) {
        if (top[i].cores < 0.05)
            break;
        list += (list.empty() ? "" : ", ") + top[i].name + " " + num("%.2f", top[i].cores);
    }
    if (!list.empty())
        out += "; busiest " + list;
    out += "; overlay, capture and monitoring tools running: " + (tools.empty() ? std::string("none known") : names_text(tools));
    if (!modules.empty())
        out += "; overlay modules in the game: " + names_text(modules);
    return out;
}

struct Engine {
    bool ok = false;
    uint32_t pid = 0, luid_high = 0, luid_low = 0, phys = 0, eng = 0;
    std::string type;
};

inline bool take(std::string_view &s, std::string_view key, uint32_t &out, int base = 10)
{
    if (s.substr(0, key.size()) != key)
        return false;
    s.remove_prefix(key.size());
    if (base == 16 && s.substr(0, 2) == "0x")
        s.remove_prefix(2);
    size_t i = 0;
    uint64_t v = 0;
    for (; i < s.size(); i++) {
        char c = s[i];
        int d = c >= '0' && c <= '9' ? c - '0' : base == 16 && c >= 'a' && c <= 'f' ? c - 'a' + 10
                                             : base == 16 && c >= 'A' && c <= 'F' ? c - 'A' + 10
                                                                                  : -1;
        if (d < 0)
            break;
        v = v * uint64_t(base) + uint64_t(d);
    }
    if (!i || v > 0xffffffffull)
        return false;
    out = uint32_t(v);
    s.remove_prefix(i);
    return true;
}

inline Engine engine(std::string_view s)
{
    Engine e;
    if (!take(s, "pid_", e.pid) || !take(s, "_luid_", e.luid_high, 16) || !take(s, "_", e.luid_low, 16) ||
        !take(s, "_phys_", e.phys) || !take(s, "_eng_", e.eng))
        return e;
    if (s.substr(0, 9) != "_engtype_")
        return e;
    e.type = std::string(s.substr(9));
    e.ok = true;
    return e;
}

struct EngineUse {
    Engine e;
    double pct = 0;
};

struct Busy3d {
    bool known = false;
    double busy = 0, game = 0, others = 0;
    uint32_t top_pid = 0;
    double top_pct = 0;
};

inline Busy3d busy_3d(const std::vector<EngineUse> &v, uint32_t game_pid, bool by_luid, uint32_t luid_high, uint32_t luid_low)
{
    struct Key {
        uint32_t high, low, phys, eng;
        double total = 0, game = 0;
    };
    std::vector<Key> keys;
    auto is_3d = [](const Engine &e) { return lower(e.type) == "3d"; };
    for (const EngineUse &u : v) {
        if (!u.e.ok || !is_3d(u.e) || (by_luid && (u.e.luid_high != luid_high || u.e.luid_low != luid_low)))
            continue;
        auto it = std::find_if(keys.begin(), keys.end(), [&](const Key &k) {
            return k.high == u.e.luid_high && k.low == u.e.luid_low && k.phys == u.e.phys && k.eng == u.e.eng;
        });
        if (it == keys.end()) {
            keys.push_back({u.e.luid_high, u.e.luid_low, u.e.phys, u.e.eng});
            it = keys.end() - 1;
        }
        it->total += u.pct;
        if (u.e.pid == game_pid)
            it->game += u.pct;
    }
    Busy3d out;
    if (keys.empty())
        return out;
    const Key *best = &keys[0];
    for (const Key &k : keys)
        if (k.total > best->total)
            best = &k;
    out.known = true;
    out.busy = std::min(100.0, best->total);
    out.game = std::min(out.busy, best->game);
    out.others = std::max(0.0, out.busy - out.game);
    for (const EngineUse &u : v)
        if (u.e.ok && u.e.pid != game_pid && is_3d(u.e) && u.e.luid_high == best->high && u.e.luid_low == best->low &&
            u.e.phys == best->phys && u.e.eng == best->eng && u.pct > out.top_pct)
            out.top_pct = u.pct, out.top_pid = u.e.pid;
    return out;
}

struct Mode {
    std::string device, monitor;
    uint32_t w = 0, h = 0, hz = 0;
    bool primary = false;
};

inline std::string mode_text(const Mode &m)
{
    return std::to_string(m.w) + "x" + std::to_string(m.h) + " at " + std::to_string(m.hz) + " Hz";
}

inline std::string display_boot_line(unsigned i, const Mode &m)
{
    return "[system] display " + std::to_string(i) + ": " + m.device + (m.monitor.empty() ? "" : " (" + m.monitor + ")") + " " +
           mode_text(m) + (m.primary ? ", primary" : "");
}

struct Chain {
    bool known = false, windowed = true;
    uint32_t w = 0, h = 0, format = 0, buffers = 0, effect = 0, flags = 0;
};

struct Window {
    bool known = false, covers = false, caption = false;
    Mode on;
};

inline std::string effect_text(uint32_t e)
{
    switch (e) {
    case 0:
        return "discard (bitblt)";
    case 1:
        return "sequential (bitblt)";
    case 3:
        return "flip sequential";
    case 4:
        return "flip discard";
    default:
        return "swap effect " + std::to_string(e);
    }
}

inline std::string display_line(const Window &w, const Chain &c)
{
    std::string out = "[perf] display: ";
    if (w.known)
        out += "the game's window is on " + w.on.device + " (" + mode_text(w.on) + "), " +
               (w.covers ? (w.caption ? "covering it with a title bar" : "covering it without a border (borderless or exclusive fullscreen)")
                         : "smaller than the display (windowed)");
    else
        out += "the game's window is unknown";
    if (!c.known)
        return out + "; swapchain unknown";
    out += "; swapchain " + std::to_string(c.w) + "x" + std::to_string(c.h) + " format " + std::to_string(c.format) + ", " +
           std::to_string(c.buffers) + " buffers, " + effect_text(c.effect) + ", " +
           (c.windowed ? "windowed" : "exclusive fullscreen") + ((c.flags & 0x800) ? ", tearing allowed" : "") +
           ((c.flags & 0x40) ? ", frame latency waitable" : "");
    return out;
}

struct Windows {
    std::string plan, overlay, dx_global, layers, priority;
    int game_mode = -1, game_dvr = -1, app_capture = -1, hags_setting = -1;
    uint32_t affinity = 0, logical = 0;
    double timer_ms = 0, timer_coarse = 0, timer_fine = 0;
};

inline std::string plan_name(std::string_view guid)
{
    std::string g = lower(guid);
    if (g == "381b4222-f694-41f0-9685-ff5bb260df2e")
        return "Balanced";
    if (g == "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c")
        return "High performance";
    if (g == "a1841308-3541-4fab-bc81-f71556f20b4a")
        return "Power saver";
    if (g == "e9a42b02-d5df-448d-aa00-03f14749eb61")
        return "Ultimate Performance";
    return g.empty() ? "unknown" : "custom (" + g + ")";
}

inline std::string overlay_name(std::string_view guid)
{
    std::string g = lower(guid);
    if (g == "00000000-0000-0000-0000-000000000000")
        return "Balanced";
    if (g == "ded574b5-45a0-4f42-8737-46345c09c238")
        return "Best performance";
    if (g == "961cc777-2547-4f9d-8174-7d86181b8a7a")
        return "Best power efficiency";
    if (g == "3af9b8d9-7c97-431d-ad78-34a8bfea439f")
        return "Better performance";
    return g.empty() ? "unknown" : g;
}

inline std::string clean(std::string_view s, size_t cap = 160)
{
    std::string out;
    for (char c : s) {
        bool ok = (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_' || c == '=' ||
                  c == ';' || c == '~' || c == ' ' || c == '.' || c == '-';
        if (ok && out.size() < cap)
            out += c;
    }
    while (!out.empty() && out.back() == ' ')
        out.pop_back();
    return out;
}

inline std::string switch_text(int v, const char *on, const char *off)
{
    return v < 0 ? std::string("not set") : v ? on : off;
}

inline std::string windows_line(const Windows &w)
{
    std::string out = "[system] windows: power plan " + w.plan;
    if (!w.overlay.empty())
        out += ", power mode " + w.overlay;
    out += "; game mode " + switch_text(w.game_mode, "on", "off") + "; Game Bar recording " +
           switch_text(w.game_dvr, "allowed", "off") + ", background capture " + switch_text(w.app_capture, "on", "off");
    out += "; hardware-accelerated GPU scheduling setting " +
           (w.hags_setting < 0 ? std::string("not set") : w.hags_setting == 2 ? "on" : w.hags_setting == 1 ? "off" : std::to_string(w.hags_setting));
    std::string dx = w.dx_global;
    while (!dx.empty() && (dx.back() == ';' || dx.back() == ' '))
        dx.pop_back();
    out += "; DirectX graphics settings " + (dx.empty() ? std::string("default") : dx);
    out += "; game exe compatibility flags " + (w.layers.empty() ? std::string("none") : w.layers);
    out += "; process priority " + (w.priority.empty() ? std::string("unknown") : w.priority) + ", affinity " +
           std::to_string(w.affinity) + " of " + std::to_string(w.logical) + " logical processors";
    if (w.timer_ms > 0)
        out += "; timer resolution " + num("%.2f", w.timer_ms) + " ms (" + num("%.2f", w.timer_coarse) + " to " +
               num("%.2f", w.timer_fine) + ")";
    return out;
}

inline std::string wddm_text(uint32_t v)
{
    if (!v)
        return "unknown";
    return std::to_string(v / 1000) + "." + std::to_string((v % 1000) / 100);
}

inline std::string scheduling_line(unsigned i, uint32_t wddm, int hags_caps, int tearing)
{
    std::string out = "[system] gpu " + std::to_string(i) + ": WDDM " + wddm_text(wddm) + ", hardware-accelerated GPU scheduling ";
    if (hags_caps < 0)
        out += "unknown";
    else
        out += std::string((hags_caps & 2) ? "on" : "off") + ((hags_caps & 1) ? " (supported)" : " (not supported)");
    out += ", tearing (variable refresh) presents " + switch_text(tearing, "supported", "not supported");
    return out;
}

inline std::string game_display_settings(const std::string &xml)
{
    static constexpr std::string_view WANT[] = {"resolution", "vsync", "frame", "refresh", "reflex", "latency", "limit", "fps"};
    std::string out;
    size_t at = 0;
    while ((at = xml.find('<', at)) != std::string::npos && out.size() < 900) {
        size_t end = xml.find('>', at);
        if (end == std::string::npos)
            break;
        std::string_view tag(xml.data() + at + 1, end - at - 1);
        at = end;
        if (tag.empty() || tag[0] == '/' || tag[0] == '?' || tag[0] == '!')
            continue;
        size_t name_end = tag.find_first_of(" \t\r\n/");
        std::string name = lower(tag.substr(0, name_end));
        bool wanted = false;
        for (std::string_view w : WANT)
            wanted |= name.find(w) != std::string::npos;
        if (!wanted || name_end == std::string_view::npos)
            continue;
        std::string attrs;
        std::string_view rest = tag.substr(name_end);
        size_t eq;
        while ((eq = rest.find('=')) != std::string_view::npos) {
            std::string_view key = rest.substr(0, eq);
            while (!key.empty() && (key.front() == ' ' || key.front() == '\t' || key.front() == '\r' || key.front() == '\n'))
                key.remove_prefix(1);
            size_t q1 = rest.find('"', eq);
            size_t q2 = q1 == std::string_view::npos ? q1 : rest.find('"', q1 + 1);
            if (q2 == std::string_view::npos)
                break;
            attrs += (attrs.empty() ? "" : " ") + clean(key, 40) + "=" + clean(rest.substr(q1 + 1, q2 - q1 - 1), 40);
            rest.remove_prefix(q2 + 1);
        }
        if (!attrs.empty())
            out += (out.empty() ? "" : " ") + clean(name, 40) + "(" + attrs + ")";
    }
    return out;
}

struct Facts {
    double secs = 0;
    Summary frames;
    bool presents = false;
    Summary present;
    double present_rate = 0;
    int sync = -1;
    double refresh = 0;
    double gpu_busy = -1, game_thread = -1, other_cores = -1, other_gpu = -1;
};

struct Verdict {
    bool smooth = false, stutter = false, judder = false, gpu = false, cpu = false, display = false, background = false;
    std::string text;
};

inline Verdict verdict(const Facts &f)
{
    Verdict v;
    auto add = [&](const std::string &s) { v.text += (v.text.empty() ? "" : "; ") + s; };
    if (f.frames.frames < 30) {
        v.text = "too few frames to judge (" + std::to_string(f.frames.frames) + ")";
        return v;
    }
    double per_min = f.secs > 0 ? 60.0 / f.secs : 1;
    double spikes = double(f.frames.spikes) * per_min;
    double uneven = f.frames.share(f.frames.uneven);
    double present_uneven = f.presents ? f.present.share(f.present.uneven) : 0;
    if (f.frames.hitches || spikes >= 6) {
        v.stutter = true;
        add("stutter (" + count(f.frames.hitches) + " frame(s) over 50 ms, " + count(f.frames.spikes) +
            " spike(s) over 2x the running average)");
    }
    if (uneven >= 10 || present_uneven >= 10) {
        v.judder = true;
        add("judder (" + num("%.0f", uneven) + "% of frames" + (f.presents ? " and " + num("%.0f", present_uneven) + "% of presents" : "") +
            " over 20% off the previous one)");
    }
    if (!v.stutter && !v.judder) {
        v.smooth = true;
        add("smooth in frame times");
    }
    if (f.gpu_busy >= 95) {
        v.gpu = true;
        add("GPU bound (3D engine " + num("%.0f", f.gpu_busy) + "% busy)");
    } else if (f.game_thread >= 0.9) {
        v.cpu = true;
        add("CPU bound (busiest game thread " + num("%.2f", f.game_thread) + " cores)");
    }
    if (f.presents && f.refresh > 0 && f.present_rate > 0 && f.sync >= 0) {
        double target = f.sync >= 1 ? f.refresh / f.sync : f.refresh;
        if (f.present_rate < target * 0.95) {
            v.display = true;
            add(std::string("display mismatch (") + (f.sync >= 1 ? "VSync on" : "VSync off") + " at " + num("%.0f", f.present_rate) +
                " presents/s on a " + num("%.0f", f.refresh) + " Hz display: " +
                (f.sync >= 1 ? "frames stay up for 1 or 2 refreshes" : "tearing and uneven motion") +
                " unless FreeSync or G-Sync is on)");
        }
    }
    if (f.other_cores >= 1.0 || f.other_gpu >= 10) {
        v.background = true;
        std::string what;
        if (f.other_cores >= 1.0)
            what = num("%.2f", f.other_cores) + " cores";
        if (f.other_gpu >= 10)
            what += (what.empty() ? "" : " and ") + num("%.0f", f.other_gpu) + "% of the 3D engine";
        add("background load (" + what + " in other processes)");
    }
    return v;
}

inline std::string verdict_tags(const Verdict &v)
{
    std::string out;
    auto add = [&](bool on, const char *s) {
        if (on)
            out += (out.empty() ? "" : ", ") + std::string(s);
    };
    add(v.smooth, "smooth");
    add(v.stutter, "stutter");
    add(v.judder, "judder");
    add(v.gpu, "GPU bound");
    add(v.cpu, "CPU bound");
    add(v.display, "display mismatch");
    add(v.background, "background load");
    return out.empty() ? "-" : out;
}

struct Tally {
    uint32_t minutes = 0, smooth = 0, stutter = 0, judder = 0, gpu = 0, cpu = 0, display = 0, background = 0;

    void add(const Verdict &v)
    {
        minutes++;
        smooth += v.smooth, stutter += v.stutter, judder += v.judder, gpu += v.gpu, cpu += v.cpu;
        display += v.display, background += v.background;
    }

    std::string text() const
    {
        return std::to_string(minutes) + " minute(s): " + std::to_string(smooth) + " smooth in frame times, " +
               std::to_string(stutter) + " stutter, " + std::to_string(judder) + " judder; " + std::to_string(gpu) +
               " GPU bound, " + std::to_string(cpu) + " CPU bound; " + std::to_string(display) + " display mismatch; " +
               std::to_string(background) + " background load";
    }
};

inline std::string verdict_line(double secs, const Verdict &v)
{
    return head(secs) + "verdict: " + v.text;
}

struct Row {
    std::string when, tags;
    Summary frames;
    bool presents = false;
    Summary present;
    double inside_avg = 0, gpu_core = 0, gpu_temp = 0, gpu_busy = -1, cpu_busiest = -1;
};

inline std::string pacing_header()
{
    char b[256];
    std::snprintf(b, sizeof b, "%-8s %5s %7s %7s %8s %7s %7s %6s %8s %8s %8s %6s %7s %7s  %s", "time", "fps", "p50 ms", "p99 ms",
                  "p99.9 ms", "max ms", "uneven", "spikes", "pres p99", "in pres", "gpu MHz", "gpu C", "3D busy", "cpu max",
                  "verdict");
    return b;
}

inline std::string pacing_row(const Row &r)
{
    auto opt = [](bool on, const char *f, double v) { return on ? num(f, v) : std::string("-"); };
    char b[512];
    std::snprintf(b, sizeof b, "%-8s %5.0f %7.2f %7.2f %8.2f %7.1f %6.1f%% %6llu %8s %8s %8s %6s %7s %7s  %s", r.when.c_str(),
                  r.frames.rate(), r.frames.p50, r.frames.p99, r.frames.p999, r.frames.max, r.frames.share(r.frames.uneven),
                  (unsigned long long)r.frames.spikes, opt(r.presents, "%.2f", r.present.p99).c_str(),
                  opt(r.presents, "%.2f", r.inside_avg).c_str(), opt(r.gpu_core > 0, "%.0f", r.gpu_core).c_str(),
                  opt(r.gpu_temp > 0, "%.0f", r.gpu_temp).c_str(), opt(r.gpu_busy >= 0, "%.0f%%", r.gpu_busy).c_str(),
                  opt(r.cpu_busiest >= 0, "%.0f%%", r.cpu_busiest).c_str(), r.tags.c_str());
    return b;
}

}
