#include "perf_system.hpp"

#include "runtime.hpp"

#include <swofty/mixin.hpp>

#include <d3d12.h>
#include <dxgi1_6.h>
#include <pdh.h>
#include <pdhmsg.h>
#include <psapi.h>
#include <tlhelp32.h>

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <mutex>
#include <unordered_map>

namespace perf::sys {

namespace {


bool under_wine()
{
    HMODULE nt = GetModuleHandleW(L"ntdll.dll");
    return nt && GetProcAddress(nt, "wine_get_version") != nullptr;
}

std::string narrow(const wchar_t *w, size_t cap = 128)
{
    std::string out;
    for (; w && *w && out.size() < cap; w++)
        out += *w < 0x80 ? char(*w) : '?';
    return out;
}

template <class F> F proc(const wchar_t *dll, const char *name)
{
    HMODULE m = GetModuleHandleW(dll);
    if (!m)
        m = LoadLibraryW(dll);
    return m ? reinterpret_cast<F>(reinterpret_cast<void *>(GetProcAddress(m, name))) : nullptr;
}

std::string guid_text(const GUID &g)
{
    char b[48];
    std::snprintf(b, sizeof b, "%08lx-%04x-%04x-%02x%02x-%02x%02x%02x%02x%02x%02x", (unsigned long)g.Data1, g.Data2, g.Data3,
                  g.Data4[0], g.Data4[1], g.Data4[2], g.Data4[3], g.Data4[4], g.Data4[5], g.Data4[6], g.Data4[7]);
    return b;
}

namespace kmt {

struct OpenFromLuid {
    LUID luid;
    UINT adapter;
};

struct Close {
    UINT adapter;
};

struct Query {
    UINT adapter;
    UINT type;
    void *data;
    UINT size;
};

struct AdapterPerf {
    UINT32 physical;
    UINT64 mem_hz, mem_max, mem_max_oc, mem_bandwidth, pcie_bandwidth;
    ULONG fan_rpm, power, temperature;
    UCHAR override_;
};

struct NodePerf {
    UINT32 node, physical;
    UINT64 hz, max_hz, max_oc_hz;
    ULONG voltage, voltage_max, voltage_max_oc;
    UINT64 latency;
};

static_assert(sizeof(AdapterPerf) == 64 && sizeof(NodePerf) == 56);

constexpr UINT Q_DRIVER = 13, Q_NODE_PERF = 61, Q_ADAPTER_PERF = 62, Q_WDDM_2_7 = 70;

using OpenFn = LONG(APIENTRY *)(OpenFromLuid *);
using QueryFn = LONG(APIENTRY *)(const Query *);
OpenFn open_fn;
QueryFn query_fn;

bool load()
{
    static int state;
    if (!state) {
        state = -1;
        if (!under_wine()) {
            open_fn = proc<OpenFn>(L"gdi32.dll", "D3DKMTOpenAdapterFromLuid");
            query_fn = proc<QueryFn>(L"gdi32.dll", "D3DKMTQueryAdapterInfo");
            if (open_fn && query_fn)
                state = 1;
        }
    }
    return state > 0;
}

UINT open(LUID luid)
{
    if (!load())
        return 0;
    OpenFromLuid o{luid, 0};
    return open_fn(&o) >= 0 ? o.adapter : 0;
}

template <class T> LONG ask(UINT adapter, UINT type, T &out)
{
    Query q{adapter, type, &out, sizeof out};
    return adapter && query_fn ? query_fn(&q) : LONG(0xC0000001);
}

}

struct Gpu {
    LUID luid{};
    UINT kmt = 0;
    unsigned index = 0;
    bool perf = true, node = true;
};

std::mutex lock;
std::vector<Gpu> gpus;
pace::GpuMinute minute_gpu, session_gpu;
pace::SysCpuMinute minute_cpu, session_cpu;

struct ChainSeen {
    pace::Chain chain;
    HWND hwnd = nullptr;
    LUID luid{};
    bool luid_known = false;
};

std::mutex chain_lock;
ChainSeen chain_seen;
std::string display_now, display_logged;

bool same(const LUID &a, const LUID &b)
{
    return a.LowPart == b.LowPart && a.HighPart == b.HighPart;
}

Gpu *game_gpu()
{
    LUID luid{};
    bool known;
    {
        std::lock_guard<std::mutex> g(chain_lock);
        luid = chain_seen.luid;
        known = chain_seen.luid_known;
    }
    for (Gpu &g : gpus)
        if (known && same(g.luid, luid))
            return &g;
    return gpus.empty() ? nullptr : &gpus[0];
}

namespace engines {

using OpenQ = PDH_STATUS(WINAPI *)(LPCWSTR, DWORD_PTR, PDH_HQUERY *);
using AddEnglish = PDH_STATUS(WINAPI *)(PDH_HQUERY, LPCWSTR, DWORD_PTR, PDH_HCOUNTER *);
using Collect = PDH_STATUS(WINAPI *)(PDH_HQUERY);
using Array = PDH_STATUS(WINAPI *)(PDH_HCOUNTER, DWORD, LPDWORD, LPDWORD, PPDH_FMT_COUNTERVALUE_ITEM_W);

Collect collect_fn;
Array array_fn;
PDH_HQUERY query;
PDH_HCOUNTER counter;
bool off;
unsigned every = 1, ticks;
std::vector<BYTE> buf;

bool start()
{
    if (off)
        return false;
    if (query)
        return true;
    off = true;
    if (under_wine())
        return false;
    auto open_fn = proc<OpenQ>(L"pdh.dll", "PdhOpenQueryW");
    auto add_fn = proc<AddEnglish>(L"pdh.dll", "PdhAddEnglishCounterW");
    collect_fn = proc<Collect>(L"pdh.dll", "PdhCollectQueryData");
    array_fn = proc<Array>(L"pdh.dll", "PdhGetFormattedCounterArrayW");
    if (!open_fn || !add_fn || !collect_fn || !array_fn) {
        swofty::log("[perf] gpu engine use: pdh.dll is unavailable; 3D engine use is not sampled");
        return false;
    }
    PDH_STATUS s = open_fn(nullptr, 0, &query);
    if (s == ERROR_SUCCESS)
        s = add_fn(query, L"\\GPU Engine(*)\\Utilization Percentage", 0, &counter);
    if (s != ERROR_SUCCESS) {
        swofty::log("[perf] gpu engine use: the GPU Engine counters are unavailable (0x%08lx); 3D engine use is not sampled",
                    (unsigned long)s);
        query = nullptr;
        return false;
    }
    auto t0 = std::chrono::steady_clock::now();
    collect_fn(query);
    double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();
    swofty::log("[perf] gpu engine use: sampled from the GPU Engine counters every 5 s (first read %.0f ms)", ms);
    off = false;
    return false;
}

bool read(std::vector<pace::EngineUse> &out)
{
    if (!start())
        return false;
    if (++ticks % every)
        return false;
    auto t0 = std::chrono::steady_clock::now();
    PDH_STATUS s = collect_fn(query);
    double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();
    if (ms > 2000) {
        off = true;
        swofty::log("[perf] gpu engine use: one read took %.0f ms; 3D engine use is not sampled from now on", ms);
        return false;
    }
    if (ms > 250 && every < 6) {
        every = 6;
        swofty::log("[perf] gpu engine use: one read took %.0f ms; sampled every 30 s from now on", ms);
    }
    if (s != ERROR_SUCCESS)
        return false;
    DWORD size = 0, n = 0;
    s = array_fn(counter, PDH_FMT_DOUBLE, &size, &n, nullptr);
    if (s != PDH_STATUS(PDH_MORE_DATA) || !size)
        return false;
    buf.resize(size);
    auto *items = reinterpret_cast<PPDH_FMT_COUNTERVALUE_ITEM_W>(buf.data());
    if (array_fn(counter, PDH_FMT_DOUBLE, &size, &n, items) != ERROR_SUCCESS)
        return false;
    out.reserve(n);
    for (DWORD i = 0; i < n; i++) {
        DWORD st = items[i].FmtValue.CStatus;
        if (st != PDH_CSTATUS_VALID_DATA && st != PDH_CSTATUS_NEW_DATA)
            continue;
        pace::Engine e = pace::engine(narrow(items[i].szName, 160));
        if (e.ok)
            out.push_back({e, items[i].FmtValue.doubleValue});
    }
    return true;
}

}

std::string exe_name(DWORD pid)
{
    HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!h)
        return "pid " + std::to_string(pid);
    wchar_t path[MAX_PATH * 2] = {};
    DWORD n = MAX_PATH * 2;
    std::string out = "pid " + std::to_string(pid);
    if (QueryFullProcessImageNameW(h, 0, path, &n)) {
        const wchar_t *base = path;
        for (const wchar_t *c = path; *c; c++)
            if (*c == L'\\' || *c == L'/')
                base = c + 1;
        out = pace::lower(narrow(base, 64));
    }
    CloseHandle(h);
    return out;
}

pace::GpuSample gpu_sample()
{
    pace::GpuSample s;
    Gpu *g = game_gpu();
    if (!g)
        return s;
    Gpu &gpu = *g;
    if (gpu.perf) {
        kmt::AdapterPerf a{};
        LONG st = kmt::ask(gpu.kmt, kmt::Q_ADAPTER_PERF, a);
        if (st < 0) {
            gpu.perf = false;
            swofty::log("[perf] gpu %u: the driver gives no adapter performance data (0x%08lx); no memory clock, temperature, "
                        "power or fan",
                        gpu.index, (unsigned long)st);
        } else {
            if (a.mem_hz > 0 && a.mem_hz < 100000000000ull)
                s.mem = double(a.mem_hz) / 1e6;
            if (a.temperature > 0 && a.temperature < 1500)
                s.temp = a.temperature / 10.0;
            if (a.power > 0 && a.power <= 2000)
                s.power = a.power / 10.0;
            if (a.fan_rpm > 0 && a.fan_rpm < 30000)
                s.fan = a.fan_rpm;
        }
    }
    if (gpu.node) {
        kmt::NodePerf n{};
        LONG st = kmt::ask(gpu.kmt, kmt::Q_NODE_PERF, n);
        if (st < 0) {
            gpu.node = false;
            swofty::log("[perf] gpu %u: the driver gives no engine performance data (0x%08lx); no core clock", gpu.index,
                        (unsigned long)st);
        } else if (n.hz > 0 && n.hz < 10000000000ull) {
            s.core = double(n.hz) / 1e6;
            if (n.max_hz > 0 && n.max_hz < 10000000000ull)
                s.core_max = double(n.max_hz) / 1e6;
        }
    }
    std::vector<pace::EngineUse> use;
    if (engines::read(use)) {
        pace::Busy3d b = pace::busy_3d(use, GetCurrentProcessId(), true, uint32_t(gpu.luid.HighPart), gpu.luid.LowPart);
        if (!b.known)
            b = pace::busy_3d(use, GetCurrentProcessId(), false, 0, 0);
        if (b.known) {
            s.util = true;
            s.busy = b.busy, s.game = b.game, s.others = b.others;
            if (b.top_pid && b.top_pct >= 1)
                s.other_top = exe_name(b.top_pid) + " " + pace::num("%.0f", b.top_pct) + "%";
        }
    }
    return s;
}

namespace cpu {

struct Perf {
    LARGE_INTEGER idle, kernel, user, dpc, interrupt;
    ULONG interrupts;
};

struct Power {
    ULONG number, max_mhz, current_mhz, limit_mhz, max_idle, current_idle;
};

static_assert(sizeof(Perf) == 48 && sizeof(Power) == 24);

using NtQsi = LONG(NTAPI *)(ULONG, PVOID, ULONG, PULONG);
using PowerInfo = LONG(WINAPI *)(int, PVOID, ULONG, PVOID, ULONG);

std::vector<Perf> last;

pace::CpuSample sample()
{
    pace::CpuSample s;
    static NtQsi qsi = proc<NtQsi>(L"ntdll.dll", "NtQuerySystemInformation");
    static PowerInfo power = under_wine() ? nullptr : proc<PowerInfo>(L"powrprof.dll", "CallNtPowerInformation");
    static const DWORD count = std::max<DWORD>(1, GetActiveProcessorCount(ALL_PROCESSOR_GROUPS));
    if (qsi) {
        std::vector<Perf> now(std::min<DWORD>(count, 64));
        ULONG got = 0;
        if (qsi(8, now.data(), ULONG(now.size() * sizeof(Perf)), &got) >= 0 && got >= sizeof(Perf)) {
            now.resize(got / sizeof(Perf));
            if (last.size() == now.size()) {
                double busy_all = 0, total_all = 0, busiest = 0;
                for (size_t i = 0; i < now.size(); i++) {
                    double idle = double(now[i].idle.QuadPart - last[i].idle.QuadPart);
                    double total = double(now[i].kernel.QuadPart - last[i].kernel.QuadPart) +
                                   double(now[i].user.QuadPart - last[i].user.QuadPart);
                    if (total <= 0)
                        continue;
                    double busy = std::clamp(total - idle, 0.0, total);
                    busy_all += busy, total_all += total;
                    busiest = std::max(busiest, 100 * busy / total);
                }
                if (total_all > 0)
                    s.load = 100 * busy_all / total_all, s.busiest = busiest;
            }
            last = std::move(now);
        }
    }
    if (power) {
        std::vector<Power> p(count);
        if (power(11, nullptr, 0, p.data(), ULONG(p.size() * sizeof(Power))) >= 0) {
            double sum = 0, limit = 0, max = 0;
            for (const Power &c : p) {
                sum += c.current_mhz;
                max = std::max(max, double(c.max_mhz));
                limit = limit ? std::min(limit, double(c.limit_mhz)) : double(c.limit_mhz);
            }
            s.mhz = sum / double(p.size()), s.max_mhz = max, s.limit_mhz = limit;
        }
    }
    return s;
}

}

namespace procs {

struct Seen {
    uint64_t created = 0, cpu = 0;
    std::string name;
    bool alive = false;
};

std::unordered_map<DWORD, Seen> seen;
bool primed;
std::vector<std::string> modules_seen;

uint64_t ft(FILETIME f)
{
    return (uint64_t(f.dwHighDateTime) << 32) | f.dwLowDateTime;
}

void scan(double secs, Minute &m)
{
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE)
        return;
    for (auto &[pid, s] : seen)
        s.alive = false;
    DWORD self = GetCurrentProcessId();
    uint64_t others = 0;
    std::vector<pace::Hog> hogs;
    PROCESSENTRY32W pe{};
    pe.dwSize = sizeof pe;
    for (BOOL more = Process32FirstW(snap, &pe); more; more = Process32NextW(snap, &pe)) {
        DWORD pid = pe.th32ProcessID;
        if (pid == self || pid == 0)
            continue;
        std::string name = narrow(pe.szExeFile, 64);
        if (pace::known_tool(name) &&
            std::find(m.tools.begin(), m.tools.end(), pace::lower(name)) == m.tools.end())
            m.tools.push_back(pace::lower(name));
        HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if (!h)
            continue;
        FILETIME c, e, k, u;
        if (GetProcessTimes(h, &c, &e, &k, &u)) {
            auto [it, fresh] = seen.try_emplace(pid);
            Seen &s = it->second;
            uint64_t cpu = ft(k) + ft(u);
            if (fresh || s.created != ft(c)) {
                s = Seen();
                s.created = ft(c);
                s.cpu = primed ? 0 : cpu;
                s.name = pace::lower(name);
            }
            s.alive = true;
            uint64_t d = cpu > s.cpu ? cpu - s.cpu : 0;
            s.cpu = cpu;
            others += d;
            if (d)
                hogs.push_back({s.name, double(d) / 1e7 / secs});
        }
        CloseHandle(h);
    }
    CloseHandle(snap);
    std::erase_if(seen, [](const auto &kv) { return !kv.second.alive; });
    if (primed && secs > 0) {
        m.other_cores = double(others) / 1e7 / secs;
        std::sort(hogs.begin(), hogs.end(), [](const pace::Hog &a, const pace::Hog &b) { return a.cores > b.cores; });
        if (hogs.size() > 5)
            hogs.resize(5);
        m.hogs = std::move(hogs);
    }
    primed = true;
    std::sort(m.tools.begin(), m.tools.end());
}

void modules(Minute &m)
{
    HMODULE hs[1024];
    DWORD need = 0;
    if (!K32EnumProcessModules(GetCurrentProcess(), hs, sizeof hs, &need))
        return;
    for (DWORD i = 0; i < std::min<DWORD>(need / sizeof(HMODULE), 1024); i++) {
        wchar_t name[MAX_PATH] = {};
        if (!K32GetModuleBaseNameW(GetCurrentProcess(), hs[i], name, MAX_PATH))
            continue;
        std::string n = narrow(name, 64);
        if (!pace::known_module(n))
            continue;
        m.modules.push_back(n);
        if (std::find(modules_seen.begin(), modules_seen.end(), pace::lower(n)) == modules_seen.end()) {
            modules_seen.push_back(pace::lower(n));
            swofty::log("[perf] overlay module in the game: %s", n.c_str());
        }
    }
}

}

pace::Mode mode_of(const wchar_t *device)
{
    pace::Mode m;
    m.device = narrow(device, 32);
    DEVMODEW dm{};
    dm.dmSize = sizeof dm;
    if (EnumDisplaySettingsW(device, ENUM_CURRENT_SETTINGS, &dm))
        m.w = dm.dmPelsWidth, m.h = dm.dmPelsHeight, m.hz = dm.dmDisplayFrequency;
    return m;
}

pace::Window window_of(HWND hwnd)
{
    pace::Window w;
    if (!hwnd || !IsWindow(hwnd))
        return w;
    HMONITOR mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    MONITORINFOEXW mi{};
    mi.cbSize = sizeof mi;
    RECT r{};
    if (!mon || !GetMonitorInfoW(mon, &mi) || !GetWindowRect(hwnd, &r))
        return w;
    w.known = true;
    w.on = mode_of(mi.szDevice);
    w.on.primary = (mi.dwFlags & MONITORINFOF_PRIMARY) != 0;
    w.covers = r.left <= mi.rcMonitor.left && r.top <= mi.rcMonitor.top && r.right >= mi.rcMonitor.right &&
               r.bottom >= mi.rcMonitor.bottom;
    w.caption = (GetWindowLongW(hwnd, GWL_STYLE) & WS_CAPTION) == WS_CAPTION;
    return w;
}

double primary_hz()
{
    DEVMODEW dm{};
    dm.dmSize = sizeof dm;
    return EnumDisplaySettingsW(nullptr, ENUM_CURRENT_SETTINGS, &dm) ? double(dm.dmDisplayFrequency) : 0;
}

void boot_displays()
{
    DISPLAY_DEVICEW dd{};
    dd.cb = sizeof dd;
    unsigned shown = 0;
    for (DWORD i = 0; shown < 8 && EnumDisplayDevicesW(nullptr, i, &dd, 0); i++, dd = DISPLAY_DEVICEW{}, dd.cb = sizeof dd) {
        if (!(dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP))
            continue;
        pace::Mode m = mode_of(dd.DeviceName);
        m.primary = (dd.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE) != 0;
        DISPLAY_DEVICEW mon{};
        mon.cb = sizeof mon;
        if (EnumDisplayDevicesW(dd.DeviceName, 0, &mon, 0))
            m.monitor = pace::clean(narrow(mon.DeviceString, 64), 64);
        swofty::log("%s", pace::display_boot_line(shown++, m).c_str());
    }
    if (!shown)
        swofty::log("[system] display: none listed");
}

int tearing_supported()
{
    auto create = proc<HRESULT(WINAPI *)(REFIID, void **)>(L"dxgi.dll", "CreateDXGIFactory1");
    IDXGIFactory5 *f = nullptr;
    if (!create || FAILED(create(__uuidof(IDXGIFactory5), reinterpret_cast<void **>(&f))) || !f)
        return -1;
    BOOL allow = FALSE;
    HRESULT hr = f->CheckFeatureSupport(DXGI_FEATURE_PRESENT_ALLOW_TEARING, &allow, sizeof allow);
    f->Release();
    return SUCCEEDED(hr) ? int(allow != FALSE) : -1;
}

void boot_gpus()
{
    if (under_wine())
        return;
    auto create = proc<HRESULT(WINAPI *)(REFIID, void **)>(L"dxgi.dll", "CreateDXGIFactory1");
    IDXGIFactory1 *f = nullptr;
    if (!create || FAILED(create(__uuidof(IDXGIFactory1), reinterpret_cast<void **>(&f))) || !f)
        return;
    int tearing = tearing_supported();
    IDXGIAdapter1 *a = nullptr;
    for (UINT i = 0; i < 8 && f->EnumAdapters1(i, &a) != DXGI_ERROR_NOT_FOUND; i++) {
        DXGI_ADAPTER_DESC1 d{};
        bool hw = a && SUCCEEDED(a->GetDesc1(&d)) && !(d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE);
        if (a)
            a->Release();
        a = nullptr;
        if (!hw)
            continue;
        Gpu g;
        g.luid = d.AdapterLuid;
        g.index = i;
        g.kmt = kmt::open(d.AdapterLuid);
        UINT wddm = 0, caps = 0;
        bool have_wddm = g.kmt && kmt::ask(g.kmt, kmt::Q_DRIVER, wddm) >= 0;
        bool have_caps = g.kmt && kmt::ask(g.kmt, kmt::Q_WDDM_2_7, caps) >= 0;
        swofty::log("%s", pace::scheduling_line(i, have_wddm ? wddm : 0, have_caps ? int(caps & 7) : -1, tearing).c_str());
        if (!g.kmt)
            g.perf = g.node = false;
        std::lock_guard<std::mutex> l(lock);
        gpus.push_back(g);
    }
    f->Release();
}

using RegGetValueFn = LSTATUS(WINAPI *)(HKEY, LPCWSTR, LPCWSTR, DWORD, LPDWORD, PVOID, LPDWORD);

RegGetValueFn reg()
{
    static RegGetValueFn fn = proc<RegGetValueFn>(L"advapi32.dll", "RegGetValueW");
    return fn;
}

int reg_dword(HKEY root, const wchar_t *key, const wchar_t *value)
{
    DWORD v = 0, size = sizeof v;
    return reg() && reg()(root, key, value, RRF_RT_REG_DWORD, nullptr, &v, &size) == ERROR_SUCCESS ? int(v) : -1;
}

std::string reg_text(HKEY root, const wchar_t *key, const wchar_t *value)
{
    wchar_t buf[512] = {};
    DWORD size = sizeof buf - sizeof(wchar_t);
    if (!reg() || reg()(root, key, value, RRF_RT_REG_SZ, nullptr, buf, &size) != ERROR_SUCCESS)
        return "";
    return pace::clean(narrow(buf, 400));
}

const char *priority_text(DWORD c)
{
    switch (c) {
    case IDLE_PRIORITY_CLASS:
        return "idle";
    case BELOW_NORMAL_PRIORITY_CLASS:
        return "below normal";
    case NORMAL_PRIORITY_CLASS:
        return "normal";
    case ABOVE_NORMAL_PRIORITY_CLASS:
        return "above normal";
    case HIGH_PRIORITY_CLASS:
        return "high";
    case REALTIME_PRIORITY_CLASS:
        return "realtime";
    default:
        return "unknown";
    }
}

void boot_windows()
{
    pace::Windows w;
    using ActiveFn = DWORD(WINAPI *)(HKEY, GUID **);
    using OverlayFn = DWORD(WINAPI *)(GUID *);
    if (!under_wine()) {
        if (auto active = proc<ActiveFn>(L"powrprof.dll", "PowerGetActiveScheme")) {
            GUID *g = nullptr;
            if (active(nullptr, &g) == ERROR_SUCCESS && g) {
                w.plan = pace::plan_name(guid_text(*g));
                LocalFree(g);
            }
        }
        if (auto overlay = proc<OverlayFn>(L"powrprof.dll", "PowerGetEffectiveOverlayScheme")) {
            GUID g{};
            if (overlay(&g) == ERROR_SUCCESS)
                w.overlay = pace::overlay_name(guid_text(g));
        }
    }
    if (w.plan.empty())
        w.plan = "unknown";
    w.game_mode = reg_dword(HKEY_CURRENT_USER, L"Software\\Microsoft\\GameBar", L"AutoGameModeEnabled");
    w.game_dvr = reg_dword(HKEY_CURRENT_USER, L"System\\GameConfigStore", L"GameDVR_Enabled");
    w.app_capture = reg_dword(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\GameDVR", L"AppCaptureEnabled");
    w.hags_setting = reg_dword(HKEY_LOCAL_MACHINE, L"SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers", L"HwSchMode");
    w.dx_global = reg_text(HKEY_CURRENT_USER, L"Software\\Microsoft\\DirectX\\UserGpuPreferences", L"DirectXUserGlobalSettings");
    wchar_t exe[MAX_PATH * 2] = {};
    if (GetModuleFileNameW(nullptr, exe, MAX_PATH * 2)) {
        const wchar_t *layers = L"Software\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Layers";
        std::string user = reg_text(HKEY_CURRENT_USER, layers, exe), all = reg_text(HKEY_LOCAL_MACHINE, layers, exe);
        w.layers = user + (!user.empty() && !all.empty() ? " " : "") + all;
        std::string gpu = reg_text(HKEY_CURRENT_USER, L"Software\\Microsoft\\DirectX\\UserGpuPreferences", exe);
        if (!gpu.empty())
            w.dx_global += (w.dx_global.empty() ? "" : " ") + std::string("game exe: ") + gpu;
    }
    w.priority = priority_text(GetPriorityClass(GetCurrentProcess()));
    DWORD_PTR pm = 0, sm = 0;
    if (GetProcessAffinityMask(GetCurrentProcess(), &pm, &sm))
        w.affinity = unsigned(__builtin_popcountll(uint64_t(pm)));
    w.logical = GetActiveProcessorCount(ALL_PROCESSOR_GROUPS);
    using TimerFn = LONG(NTAPI *)(PULONG, PULONG, PULONG);
    if (auto timer = proc<TimerFn>(L"ntdll.dll", "NtQueryTimerResolution")) {
        ULONG coarse = 0, fine = 0, cur = 0;
        if (timer(&coarse, &fine, &cur) >= 0 && cur)
            w.timer_ms = cur / 10000.0, w.timer_coarse = coarse / 10000.0, w.timer_fine = fine / 10000.0;
    }
    swofty::log("%s", pace::windows_line(w).c_str());
}

void boot_game_settings()
{
    std::wstring path = runtime::hardware_settings_file();
    std::string text = path.empty() ? std::string() : pace::game_display_settings(runtime::read_file(path, 1 << 20));
    swofty::log("[system] game display settings: %s", text.empty() ? "not found" : text.c_str());
}

}

void boot()
{
    boot_displays();
    boot_gpus();
    boot_windows();
    boot_game_settings();
    Minute primer;
    procs::scan(1, primer);
    cpu::sample();
}

void learn_chain(void *swapchain)
{
    auto *sc = static_cast<IDXGISwapChain1 *>(swapchain);
    ChainSeen c;
    DXGI_SWAP_CHAIN_DESC1 d{};
    if (SUCCEEDED(sc->GetDesc1(&d))) {
        c.chain.known = true;
        c.chain.w = d.Width, c.chain.h = d.Height, c.chain.format = d.Format, c.chain.buffers = d.BufferCount;
        c.chain.effect = d.SwapEffect, c.chain.flags = d.Flags;
        DXGI_SWAP_CHAIN_FULLSCREEN_DESC fd{};
        if (SUCCEEDED(sc->GetFullscreenDesc(&fd)))
            c.chain.windowed = fd.Windowed != FALSE;
    }
    sc->GetHwnd(&c.hwnd);
    ID3D12CommandQueue *q = nullptr;
    if (SUCCEEDED(sc->GetDevice(IID_PPV_ARGS(&q))) && q) {
        ID3D12Device *dev = nullptr;
        if (SUCCEEDED(q->GetDevice(IID_PPV_ARGS(&dev))) && dev) {
            c.luid = dev->GetAdapterLuid();
            c.luid_known = true;
            dev->Release();
        }
        q->Release();
    }
    std::lock_guard<std::mutex> g(chain_lock);
    chain_seen = c;
}

void sample()
{
    pace::GpuSample g = gpu_sample();
    pace::CpuSample c = cpu::sample();
    std::lock_guard<std::mutex> l(lock);
    minute_gpu.add(g);
    session_gpu.add(g);
    minute_cpu.add(c);
    session_cpu.add(c);
}

Minute minute(double secs)
{
    Minute m;
    ChainSeen c;
    {
        std::lock_guard<std::mutex> g(chain_lock);
        c = chain_seen;
    }
    pace::Window w = window_of(c.hwnd);
    m.refresh = w.known && w.on.hz > 1 ? double(w.on.hz) : primary_hz();
    std::string text = pace::display_line(w, c.chain);
    if (text != display_logged)
        m.display = display_logged = text;
    procs::scan(secs, m);
    procs::modules(m);
    Gpu *gpu = game_gpu();
    std::lock_guard<std::mutex> l(lock);
    display_now = text;
    m.gpu = minute_gpu;
    m.gpu_index = gpu ? gpu->index : 0;
    m.cpu = minute_cpu;
    minute_gpu = pace::GpuMinute();
    minute_cpu = pace::SysCpuMinute();
    return m;
}

bool session(pace::GpuMinute &gpu, unsigned &gpu_index, pace::SysCpuMinute &cpu, std::string &display)
{
    std::unique_lock<std::mutex> c(chain_lock, std::try_to_lock);
    if (!c.owns_lock())
        return false;
    c.unlock();
    Gpu *g = game_gpu();
    std::unique_lock<std::mutex> l(lock, std::try_to_lock);
    if (!l.owns_lock())
        return false;
    gpu = session_gpu;
    cpu = session_cpu;
    display = display_now;
    gpu_index = g ? g->index : 0;
    return true;
}

}
