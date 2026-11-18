#include "live/live_bus.hpp"
#include "live/live_rows.hpp"
#include "live/live_snapshot.hpp"
#include "live/live_state.hpp"
#include "live/live_telemetry_fill.hpp"
#include "live/live_turns.hpp"
#include "runtime.hpp"
#include "badge_images.hpp"
#include "telemetry_feed.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <memory>
#include <mutex>
#include <string>

namespace {

namespace lv = swofty_core::live;

struct Named {
    const char *name;
    uintptr_t lv::Sites::*slot;
};

const Named SITE_NAMES[] = {
    {"g_Sess_VehicleTable", &lv::Sites::vehicle_table},
    {"g_Sess_VehicleCount", &lv::Sites::vehicle_count},
    {"g_Game_Manager", &lv::Sites::game_manager},
    {"g_Game_TrackLength", &lv::Sites::track_length},
    {"g_Sess_TyreRulesSessionType", &lv::Sites::session_type},
    {"vt_Car_SystemsContainer", &lv::Sites::vt_car_systems},
    {"k_Live_SessionSystemKey", &lv::Sites::key_results},
    {"k_Live_SpectatorDirectorKey", &lv::Sites::key_spectator},
    {"k_Live_RelativeGapHelperKey", &lv::Sites::key_timing},
    {"k_Live_PitLaneSystemKey", &lv::Sites::key_pit},
    {"k_Live_SafetyCarLogicSystemKey", &lv::Sites::key_safety_car},
    {"k_Live_PenaltyLogKey", &lv::Sites::key_penalty_log},
    {"k_Live_VehicleHelperSystemKey", &lv::Sites::key_session_component},
    {"k_Live_TyreSystemKey", &lv::Sites::key_tyres},
    {"k_Live_MarshalZoneMonitorKey", &lv::Sites::key_marshal},
    {"k_Live_RedFlagSystemKey", &lv::Sites::key_red_flag},
    {"k_Live_GameModeSystemKey", &lv::Sites::key_game_mode},
    {"k_Live_EntrantsSystemKey", &lv::Sites::key_team_tables},
    {"g_Live_ControllerSystem", &lv::Sites::controllers},
    {"vt_Sess_ControllerSystem", &lv::Sites::vt_controllers},
    {"vt_Sess_HumanController", &lv::Sites::vt_human},
    {"g_Live_SectorEnds", &lv::Sites::sector_block},
    {"g_Live_SectorCount", &lv::Sites::sector_count},
    {"g_Live_RacingVehicles", &lv::Sites::track_world},
    {"g_Game_MessageBus", &lv::Sites::bus_slot},
    {"Ne_Subscribe_BusMessage", &lv::Sites::bus_subscribe},
};

bool kernel_copy(void *, uintptr_t addr, void *dst, size_t n)
{
    SIZE_T got = 0;
    return ReadProcessMemory(GetCurrentProcess(), reinterpret_cast<LPCVOID>(addr), dst, n, &got) && got == n;
}

bool env_on(const char *name)
{
    char v[8] = {};
    return GetEnvironmentVariableA(name, v, sizeof v) && v[0] && v[0] != '0';
}

constexpr lv::Group TELEMETRY_GROUPS[] = {lv::G_CAR_SYSTEMS, lv::G_ERS, lv::G_PROGRESS, lv::G_CLASS, lv::G_TIMING,
                                          lv::G_PIT,        lv::G_PIT_LANE,  lv::G_PENALTIES, lv::G_TYRES, lv::G_FLAGS, lv::G_CONTROL,
                                          lv::G_TRACK};

bool g_on;
bool g_forced;
bool g_verify;
lv::Sites g_sites;
lv::Gates g_gates;
std::unique_ptr<lv::Live> g_live;
lv::Snapshot g_snap;
std::mutex g_lock;
std::atomic<bool> g_valid;
uint64_t g_frame;
std::vector<lv::Event> g_events;
SwoftyTelemetryFeed g_mem_view;
float g_prev_track_len = -1;
char g_prev_track_name[48];
int g_official = -1;
lv::turns::Table g_turns;
lv::bus::Queue g_bus_queue;
lv::bus::Subscriber g_bus_sub;
std::atomic<bool> g_bus_armed;
double g_bus_tried = -1e9;

double qpc_ms()
{
    static LARGE_INTEGER freq;
    LARGE_INTEGER now;
    if (!freq.QuadPart)
        QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&now);
    return freq.QuadPart ? double(now.QuadPart) * 1000.0 / double(freq.QuadPart) : 0.0;
}

uint64_t bus_on_collision(void *, const uint8_t *msg)
{
    if (msg)
        g_bus_queue.push_collision(msg, qpc_ms());
    return 0;
}

uint64_t bus_on_status(void *, const uint8_t *msg)
{
    if (msg)
        g_bus_queue.push_retirement(msg);
    return 0;
}

uintptr_t bus_alloc(uintptr_t allocator, uintptr_t fn, uintptr_t ctx)
{
    uintptr_t vt = 0, call = 0;
    if (!kernel_copy(nullptr, allocator, &vt, sizeof vt) || !vt ||
        !kernel_copy(nullptr, vt + lv::bus::allocator_alloc, &call, sizeof call) || !call ||
        (g_sites.image_hi && (call < g_sites.image_lo || call >= g_sites.image_hi)))
        return 0;
    void *p = reinterpret_cast<void *(*)(uintptr_t, size_t, size_t)>(call)(allocator, lv::bus::delegate_size,
                                                                           lv::bus::delegate_align);
    if (!p)
        return 0;
    auto *d = static_cast<uintptr_t *>(p);
    d[0] = fn;
    d[1] = ctx;
    return uintptr_t(p);
}

void bus_subscribe(uintptr_t bus, const char *name, uintptr_t delegate)
{
    reinterpret_cast<void (*)(uintptr_t, const char *, uintptr_t)>(g_sites.bus_subscribe)(bus, name, delegate);
}

void bus_arm(double now)
{
    if (!g_gates[lv::G_BUS_EVENTS] || (now - g_bus_tried < 1.0 && now >= g_bus_tried))
        return;
    g_bus_tried = now;
    lv::bus::Ops o;
    o.mem.fn = kernel_copy;
    o.bus_slot = g_sites.bus_slot;
    o.alloc = bus_alloc;
    o.subscribe = bus_subscribe;
    o.collision_fn = uintptr_t(&bus_on_collision);
    o.status_fn = uintptr_t(&bus_on_status);
    bool was = g_bus_sub.armed();
    if (!g_bus_sub.arm(o) || was)
        return;
    g_live->set_retirements_from_bus(true);
    g_bus_armed.store(true, std::memory_order_release);
    swofty::log("[live] subscribed to the game's message bus 0x%llx: %s, %s", (unsigned long long)g_bus_sub.bus(),
                lv::bus::COLLISION_MESSAGE, lv::bus::STATUS_MESSAGE);
}

void resolve_sites()
{
    for (const Named &n : SITE_NAMES)
        g_sites.*n.slot = swofty::api && swofty::api->resolve ? swofty::api->resolve(n.name) : 0;
    if (auto *base = reinterpret_cast<const uint8_t *>(GetModuleHandleW(nullptr))) {
        auto *dos = reinterpret_cast<const IMAGE_DOS_HEADER *>(base);
        auto *nt = reinterpret_cast<const IMAGE_NT_HEADERS64 *>(base + dos->e_lfanew);
        g_sites.image_lo = uintptr_t(base);
        g_sites.image_hi = uintptr_t(base) + nt->OptionalHeader.SizeOfImage;
    }
}

const char *unresolved_name(uintptr_t lv::Sites::*slot)
{
    for (const Named &n : SITE_NAMES)
        if (n.slot == slot)
            return n.name;
    return "a scanned site";
}

uint32_t module_stamp()
{
    auto *base = reinterpret_cast<const uint8_t *>(GetModuleHandleW(nullptr));
    if (!base)
        return 0;
    auto *dos = reinterpret_cast<const IMAGE_DOS_HEADER *>(base);
    return reinterpret_cast<const IMAGE_NT_HEADERS64 *>(base + dos->e_lfanew)->FileHeader.TimeDateStamp;
}

std::string group_list(const lv::Gates &g)
{
    std::string out;
    for (unsigned i = 0; i < lv::G_COUNT; i++)
        if (g.on[i])
            out += (out.empty() ? "" : ", ") + std::string(lv::group_name(lv::Group(i)));
    return out;
}

void decide_gates(const lv::BuildProof &build)
{
    g_forced = env_on("SWOFTY_LIVE_FORCE");
    g_verify = env_on("SWOFTY_TELEMETRY_VERIFY");
    g_sites.progress_from_manager = build.progress_from_manager;
    g_sites.ers_mode = build.ers_mode;
    lv::Decision d = lv::decide(g_sites, build, g_forced);
    g_gates = d.gates;
    for (unsigned i = 0; i < lv::G_COUNT; i++) {
        const lv::Verdict &v = d.verdict[i];
        const lv::Group g = lv::Group(i);
        if (v.why == lv::Why::on)
            continue;
        if (v.why == lv::Why::site)
            swofty::log("[live] %s off on the %s build: %s does not resolve in this build's bindings", lv::group_name(g),
                        build.name, unresolved_name(v.missing));
        else if (v.why == lv::Why::build)
            swofty::log("[live] %s off on the %s build: its layout is not proven there (SWOFTY_LIVE_FORCE=1 reads it anyway)",
                        lv::group_name(g), build.name);
        else
            swofty::log("[live] %s off: %s (SWOFTY_LIVE_FORCE=1 reads it unverified)", lv::group_name(g), lv::ROWS[g].needs);
    }
}

void publish()
{
    lv::snap_begin(&g_snap);
    g_snap.frame = ++g_frame;
    g_snap.valid = g_live->active();
    g_snap.ses = g_live->ses;
    std::memcpy(g_snap.cars, g_live->cars, sizeof g_snap.cars);
    std::memcpy(g_snap.present, g_live->present, sizeof g_snap.present);
    g_snap.official = g_official;
    g_snap.turn_length = g_turns.length;
    lv::snap_end(&g_snap);
    g_valid.store(g_snap.valid, std::memory_order_release);
}

void refresh_turns()
{
    const SwoftyLiveSession &s = g_live->ses;
    float len = (s.have & SWOFTY_LIVE_SHAVE_TRACK_LENGTH) ? s.track_length_m : 0;
    if (len == g_prev_track_len && !std::strcmp(g_prev_track_name, s.track_name))
        return;
    g_prev_track_len = len;
    std::memcpy(g_prev_track_name, s.track_name, sizeof g_prev_track_name);
    if (len > 0)
        g_turns.reset(s.track_name, len);
    else
        g_turns = lv::turns::Table{};
    g_official = g_turns.official;
}

int live_available()
{
    return g_on && g_valid.load(std::memory_order_acquire) ? 1 : 0;
}

bool read_snapshot(lv::Snapshot &out)
{
    return g_on && lv::snap_read(&g_snap, &out) && out.valid;
}

int live_session(SwoftyLiveSession *out)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!out)
        return SWOFTY_ASSET_INVALID;
    static thread_local lv::Snapshot s;
    if (!read_snapshot(s))
        return SWOFTY_ASSET_UNAVAILABLE;
    *out = s.ses;
    return SWOFTY_ASSET_OK;
}

int car_from(const lv::Snapshot &s, uint32_t vehicle, SwoftyLiveCar *out)
{
    if (vehicle >= SWOFTY_LIVE_MAX_CARS || !s.present[vehicle])
        return SWOFTY_ASSET_UNAVAILABLE;
    *out = s.cars[vehicle];
    return SWOFTY_ASSET_OK;
}

int live_car(uint32_t vehicle, SwoftyLiveCar *out)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!out || vehicle >= SWOFTY_LIVE_MAX_CARS)
        return SWOFTY_ASSET_INVALID;
    static thread_local lv::Snapshot s;
    if (!read_snapshot(s))
        return SWOFTY_ASSET_UNAVAILABLE;
    return car_from(s, vehicle, out);
}

int live_subject(SwoftyLiveCar *out)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!out)
        return SWOFTY_ASSET_INVALID;
    static thread_local lv::Snapshot s;
    if (!read_snapshot(s) || s.ses.subject < 0)
        return SWOFTY_ASSET_UNAVAILABLE;
    return car_from(s, uint32_t(s.ses.subject), out);
}

int live_turn(uint32_t vehicle, SwoftyLiveTurn *out)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!out || vehicle >= SWOFTY_LIVE_MAX_CARS)
        return SWOFTY_ASSET_INVALID;
    static thread_local lv::Snapshot s;
    if (!read_snapshot(s) || !s.present[vehicle] || !(s.cars[vehicle].have & SWOFTY_LIVE_HAVE_PROGRESS) || s.official < 0)
        return SWOFTY_ASSET_UNAVAILABLE;
    lv::turns::Table t;
    t.official = s.official;
    t.length = s.turn_length;
    return t.at(s.cars[vehicle].lap_m, *out) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_UNAVAILABLE;
}

int live_track_turns(uint32_t *count, SwoftyLiveTurn *out, uint32_t cap)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!count)
        return SWOFTY_ASSET_INVALID;
    static thread_local lv::Snapshot s;
    if (!read_snapshot(s) || s.official < 0)
        return SWOFTY_ASSET_UNAVAILABLE;
    lv::turns::Table t;
    t.official = s.official;
    t.length = s.turn_length;
    *count = uint32_t(t.count());
    for (uint32_t i = 0; out && i < cap && i < *count; i++)
        t.fill(int(i), out[i], 0.f);
    return SWOFTY_ASSET_OK;
}

int live_team_badge(uint32_t vehicle, char *key, uint32_t cap)
{
    if (!g_on || !g_live)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!key || cap < 2 || vehicle >= SWOFTY_LIVE_MAX_CARS)
        return SWOFTY_ASSET_INVALID;
    std::lock_guard<std::mutex> g(g_lock);
    return g_live->read_team_badge(vehicle, key, cap) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_UNAVAILABLE;
}

int live_driver_name(uint32_t vehicle, char *out, uint32_t cap)
{
    if (!g_on || !g_live)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!out || cap < 2 || vehicle >= SWOFTY_LIVE_MAX_CARS)
        return SWOFTY_ASSET_INVALID;
    std::lock_guard<std::mutex> g(g_lock);
    return g_live->read_driver_name(vehicle, out, cap) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_UNAVAILABLE;
}

int live_team_badge_texture(uint32_t vehicle, char *path, uint32_t cap)
{
    if (!g_on || !g_live)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!path || cap < 2 || vehicle >= SWOFTY_LIVE_MAX_CARS)
        return SWOFTY_ASSET_INVALID;
    std::lock_guard<std::mutex> g(g_lock);
    return g_live->read_team_badge_texture(vehicle, path, cap) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_UNAVAILABLE;
}

int live_seq(uint64_t *frame)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!frame)
        return SWOFTY_ASSET_INVALID;
    std::atomic_ref<uint64_t> f(g_snap.frame);
    *frame = f.load(std::memory_order_acquire);
    return SWOFTY_ASSET_OK;
}

const SwoftyLive table = {SWOFTY_LIVE_VERSION, live_available,   live_session,    live_car, live_subject,
                          live_turn,           live_track_turns, live_team_badge, live_seq, live_driver_name,
                          live_team_badge_texture, badge_images::api};

}

extern "C" __declspec(dllexport) const SwoftyLive *SwoftyCoreLive(void)
{
    return &table;
}

namespace runtime {

void live_setup()
{
    if (!enabled("telemetry", nullptr))
        return;
    uint32_t stamp = module_stamp();
    const lv::BuildProof *build = same_build() ? &lv::KB_BUILD : lv::proof_for(stamp);
    if (!build) {
        swofty::log("[live] off: build %u has no proven live layouts", stamp);
        return;
    }
    resolve_sites();
    decide_gates(*build);
    bool any = false;
    for (bool b : g_gates.on)
        any = any || b;
    if (!any) {
        swofty::log("[live] off on the %s build: no group is verified and resolved", build->name);
        return;
    }
    lv::Mem mem;
    mem.fn = kernel_copy;
    g_sites.my_team_saved_badge = stamp == 1794312636u;
    g_live = std::make_unique<lv::Live>(mem, g_sites, g_gates);
    g_on = true;
    swofty::log("[live] on (%s build): groups %s%s%s", build->name, group_list(g_gates).c_str(),
                g_forced ? ", reading unverified groups (SWOFTY_LIVE_FORCE)" : "",
                g_verify ? ", cross-checking against UDP (SWOFTY_TELEMETRY_VERIFY)" : "");
}

bool live_on()
{
    return g_on;
}

void live_tick(double now)
{
    if (!g_on)
        return;
    {
        std::lock_guard<std::mutex> g(g_lock);
        g_live->update(now);
        bus_arm(now);
        for (const lv::Event &e : g_bus_queue.drain())
            g_live->add_event(e);
        refresh_turns();
        for (const lv::Event &e : g_live->take_events())
            g_events.push_back(e);
        if (g_events.size() > 256)
            g_events.erase(g_events.begin(), g_events.end() - 256);
        publish();
    }
}

bool live_memory_verified()
{
    if (!g_on)
        return false;
    for (lv::Group g : TELEMETRY_GROUPS)
        if (!g_gates[g])
            return false;
    return true;
}

bool live_collision_needs_udp()
{
    return !(g_on && g_gates[lv::G_BUS_EVENTS]);
}

bool live_events_from_memory()
{
    return g_on && g_gates[lv::G_CLASS];
}

bool live_bus_armed()
{
    return g_bus_armed.load(std::memory_order_acquire);
}

uint32_t live_fill_feed(SwoftyTelemetryFeed &f)
{
    if (!g_on)
        return 0;
    lv::Snapshot s;
    if (!lv::snap_read(&g_snap, &s) || !s.valid)
        return 0;
    uint32_t groups = lv::fill_feed(f, s.ses, s.cars, s.present);
    if (g_verify) {
        swofty::tel::feed_init(&g_mem_view);
        lv::fill_feed(g_mem_view, s.ses, s.cars, s.present);
    }
    return groups;
}

bool live_verify()
{
    return g_on && g_verify;
}

const SwoftyTelemetryFeed &live_memory_view()
{
    return g_mem_view;
}

std::vector<LiveEvent> live_take_events()
{
    std::vector<LiveEvent> out;
    for (const lv::Event &e : g_events)
        out.push_back({e.kind == lv::Event::retirement  ? SWOFTY_EVENT_TELEMETRY_RETIREMENT
                       : e.kind == lv::Event::collision ? SWOFTY_EVENT_TELEMETRY_COLLISION
                                                        : SWOFTY_EVENT_TELEMETRY_OVERTAKE,
                       e.a, e.b});
    g_events.clear();
    return out;
}

}
