#include "runtime.hpp"
#include "telemetry_feed.hpp"
#include "live/live_telemetry_fill.hpp"
#include "udp_telemetry_logic.hpp"

#include <safetyhook.hpp>
#include <swofty/mixin.hpp>

#include <algorithm>
#include <atomic>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

// Typed read access to live session and car state, and the shared-memory feed external tools
// read. The table stays unavailable until the telemetry feature is on (experimental) and a
// session is live. The per-frame tick and the session start are driven from the events module's
// flow-tick and session-enter hooks (events.cpp), and the results finalise has its own mid hook, so the
// feed's frame counter and session clock advance in a running game. The
// car, participant and session fields fill from the game's memory (live_state.cpp) for every group the
// live state has verified, and from the game's own UDP packets, decoded in the sendto hook (udp_ids.cpp,
// udp_telemetry_logic.hpp), for the rest. A value the memory read has wins over the UDP value. Overtake
// events derive from the classification when the memory source is on; collision and retirement events come from
// the game's own bus messages (r_car_to_car_col, vehicle_status_changed) once the core has subscribed to them
// (live_bus.hpp), and the UDP versions are dropped from then on. Events from the UDP fallback are queued in the
// sendto hook and fired on the flow thread by the frame tick.

namespace {

namespace udp_tel = runtime::udp_tel;

using swofty::tel::feed_begin;
using swofty::tel::feed_copy;
using swofty::tel::feed_end;
using swofty::tel::feed_init;
using swofty::tel::feed_read;

const runtime::Sig S_RESULTS{"Game_SessionStateFinaliseResultsSync_OnResultsFinalised",
                             "56 57 53 48 83 EC 30 4C 89 C7 48 89 D0 48 89 CE 4C 8B 02 48 8D 15 5E C1 67 05", 0x9d2b20};

constexpr const char *FEED_NAME = "Local\\SwoftyTelemetry";

bool g_on;
std::atomic<bool> g_live;
std::mutex write_lock;
SwoftyTelemetryFeed master;

std::mutex map_lock;
std::atomic<bool> feed_on;
HANDLE map_handle;
SwoftyTelemetryFeed *view;

std::vector<udp_tel::Event> pending;
std::atomic<uint64_t> first_seen[3];

safetyhook::MidHook h_results;
LARGE_INTEGER qpc_freq, qpc_last;

double tick_seconds()
{
    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);
    double dt = qpc_freq.QuadPart ? double(now.QuadPart - qpc_last.QuadPart) / double(qpc_freq.QuadPart) : 0.0;
    qpc_last = now;
    return dt < 0 || dt > 1 ? 0.0 : dt;
}

void publish()
{
    if (feed_on.load(std::memory_order_acquire) && view)
        feed_copy(view, &master);
}

void flow_tick()
{
    double dt = tick_seconds();
    std::vector<udp_tel::Event> fire;
    {
        std::lock_guard<std::mutex> g(write_lock);
        feed_begin(&master);
        master.frame++;
        if (g_live.load(std::memory_order_relaxed)) {
            master.session.session_time += float(dt);
            runtime::live_fill_feed(master);
        }
        feed_end(&master);
        publish();
        fire.swap(pending);
        if (g_live.load(std::memory_order_relaxed))
            for (const runtime::LiveEvent &e : runtime::live_take_events())
                fire.push_back({e.name, e.a, e.b});
        else
            runtime::live_take_events();
    }
    for (auto &e : fire)
        runtime::events_fire(e.name, e.a, e.b, nullptr);
}

void set_live(bool live, const char *why)
{
    bool was = g_live.exchange(live);
    {
        std::lock_guard<std::mutex> g(write_lock);
        feed_begin(&master);
        if (!live) {
            uint64_t frame = master.frame;
            feed_init(&master);
            master.frame = frame;
        }
        feed_end(&master);
        publish();
    }
    if (!live) {
        std::lock_guard<std::mutex> g(write_lock);
        pending.clear();
    }
    if (was != live)
        swofty::log("[telemetry] session %s (%s); frame %llu", live ? "started" : "ended", why,
                    (unsigned long long)master.frame);
}

void session_enter()
{
    set_live(true, "Game_GameModule_OnSessionEnter");
}

void on_results(safetyhook::Context &)
{
    set_live(false, "Game_SessionStateFinaliseResultsSync_OnResultsFinalised");
}

int tel_available()
{
    return g_on && g_live.load(std::memory_order_acquire);
}

int tel_session(SwoftySessionState *out)
{
    if (!g_on || !out)
        return g_on ? SWOFTY_ASSET_INVALID : SWOFTY_ASSET_UNAVAILABLE;
    if (!g_live.load(std::memory_order_acquire))
        return SWOFTY_ASSET_UNAVAILABLE;
    SwoftyTelemetryFeed f;
    if (!feed_read(&master, &f))
        return SWOFTY_ASSET_UNAVAILABLE;
    *out = f.session;
    return SWOFTY_ASSET_OK;
}

int tel_car(uint32_t index, SwoftyCarState *out)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!out || index >= swofty::tel::FEED_CARS)
        return SWOFTY_ASSET_INVALID;
    if (!g_live.load(std::memory_order_acquire))
        return SWOFTY_ASSET_UNAVAILABLE;
    SwoftyTelemetryFeed f;
    if (!feed_read(&master, &f))
        return SWOFTY_ASSET_UNAVAILABLE;
    *out = f.cars[index];
    return SWOFTY_ASSET_OK;
}

int tel_participant(uint32_t index, SwoftyParticipant *out)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    if (!out || index >= swofty::tel::FEED_PARTICIPANTS)
        return SWOFTY_ASSET_INVALID;
    if (!g_live.load(std::memory_order_acquire))
        return SWOFTY_ASSET_UNAVAILABLE;
    SwoftyTelemetryFeed f;
    if (!feed_read(&master, &f))
        return SWOFTY_ASSET_UNAVAILABLE;
    *out = f.participants[index];
    return SWOFTY_ASSET_OK;
}

int tel_shared_memory(const char **name)
{
    if (!g_on)
        return SWOFTY_ASSET_UNAVAILABLE;
    std::lock_guard<std::mutex> g(map_lock);
    if (!view) {
        map_handle = CreateFileMappingW(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0,
                                        sizeof(SwoftyTelemetryFeed), L"Local\\SwoftyTelemetry");
        if (map_handle)
            view = static_cast<SwoftyTelemetryFeed *>(
                MapViewOfFile(map_handle, FILE_MAP_WRITE, 0, 0, sizeof(SwoftyTelemetryFeed)));
        if (!view) {
            swofty::log("[telemetry] shared_memory: CreateFileMapping/MapViewOfFile for %s failed (err %lu)", FEED_NAME,
                        GetLastError());
            if (map_handle)
                CloseHandle(map_handle), map_handle = nullptr;
            return SWOFTY_ASSET_UNAVAILABLE;
        }
        feed_init(view);
        {
            std::lock_guard<std::mutex> w(write_lock);
            feed_copy(view, &master);
        }
        swofty::log("[telemetry] shared_memory: %s mapped, %u bytes", FEED_NAME, unsigned(sizeof(SwoftyTelemetryFeed)));
    }
    feed_on.store(true, std::memory_order_release);
    if (name)
        *name = FEED_NAME;
    return SWOFTY_ASSET_OK;
}

std::atomic<uint64_t> checked_packets, mismatched_fields;

bool live_verify_packet(const uint8_t *buf, size_t len)
{
    return runtime::live_verify() && udp_tel::identify(buf, len);
}

void report_mismatches(const std::vector<std::string> &bad)
{
    uint64_t n = ++checked_packets;
    if (bad.empty())
        return;
    uint64_t total = mismatched_fields += bad.size();
    static std::atomic<uint64_t> logged;
    for (const std::string &m : bad)
        if (logged.fetch_add(1) < 200)
            swofty::log("[telemetry] cross-check: %s (packet %llu, %llu mismatched fields so far)", m.c_str(),
                        (unsigned long long)n, (unsigned long long)total);
}

void udp_first(const udp_tel::Spec *sp)
{
    if (sp->id > 15)
        return;
    uint64_t bit = uint64_t(1) << sp->id;
    if (first_seen[sp->format - 2024].fetch_or(bit) & bit)
        return;
    DWORD err = GetLastError();
    swofty::log("[telemetry] first UDP packet %d decoded: format %d, %zu bytes", sp->id, sp->format, sp->size);
    SetLastError(err);
}

const SwoftyTelemetry table = {tel_available, tel_session, tel_car, tel_participant, tel_shared_memory};

void hook(safetyhook::MidHook &slot, const runtime::Sig &sig, safetyhook::MidHookFn fn)
{
    uintptr_t at = runtime::find(sig, "telemetry");
    if (!at)
        return;
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[telemetry] mid hook on %s at base+0x%llx failed: %s", sig.what,
                    (unsigned long long)(at - swofty::api->module_base()), runtime::hook_error(h.error()).c_str());
        return;
    }
    slot = std::move(*h);
    swofty::log("[telemetry] %s hooked at base+0x%llx", sig.what, (unsigned long long)(at - swofty::api->module_base()));
}

void setup()
{
    if (!runtime::enabled("telemetry", nullptr))
        return;
    feed_init(&master);
    QueryPerformanceFrequency(&qpc_freq);
    QueryPerformanceCounter(&qpc_last);
    hook(h_results, S_RESULTS, on_results);
    g_on = true;
    swofty::log("[telemetry] on; frame tick %s, session witnesses %s/%s; car, participant and session fields fill from "
                "the game's memory where the live state has verified it, else from the UDP packets while the game's UDP "
                "telemetry output is on",
                runtime::events_flow_armed() ? "hooked" : "unresolved", runtime::events_session_enter_armed() ? "on" : "off",
                h_results ? "on" : "off");
}

}

namespace runtime {

bool telemetry_wants_udp()
{
    return g_on && (!live_memory_verified() || live_collision_needs_udp());
}

void telemetry_udp(const uint8_t *buf, size_t len)
{
    if (!g_on || !g_live.load(std::memory_order_acquire))
        return;
    if (!udp_tel::identify(buf, len))
        return;
    std::vector<udp_tel::Event> events;
    const udp_tel::Spec *sp;
    {
        std::lock_guard<std::mutex> g(write_lock);
        if (live_verify_packet(buf, len)) {
            std::vector<std::string> bad;
            SwoftyTelemetryFeed scratch;
            feed_init(&scratch);
            const udp_tel::Spec *probe = udp_tel::decode(buf, len, scratch, nullptr);
            if (probe && probe->id < 32)
                swofty_core::live::cross_check(runtime::live_memory_view(), scratch, 1u << probe->id, bad);
            report_mismatches(bad);
        }
        feed_begin(&master);
        sp = udp_tel::decode(buf, len, master, &events);
        runtime::live_fill_feed(master);
        feed_end(&master);
        publish();
        bool from_memory = runtime::live_events_from_memory(), from_bus = runtime::live_bus_armed();
        events.erase(std::remove_if(events.begin(), events.end(),
                                    [&](const udp_tel::Event &e) {
                                        return (from_memory && !std::strcmp(e.name, SWOFTY_EVENT_TELEMETRY_OVERTAKE)) ||
                                               ((from_memory || from_bus) &&
                                                !std::strcmp(e.name, SWOFTY_EVENT_TELEMETRY_RETIREMENT)) ||
                                               (from_bus && !std::strcmp(e.name, SWOFTY_EVENT_TELEMETRY_COLLISION));
                                    }),
                     events.end());
        if (pending.size() < 256)
            pending.insert(pending.end(), events.begin(), events.end());
    }
    if (sp)
        udp_first(sp);
}

const SwoftyTelemetry *telemetry_table()
{
    return &table;
}

void telemetry_setup()
{
    setup();
}

void telemetry_tick()
{
    if (g_on)
        flow_tick();
}

void telemetry_session_enter()
{
    if (g_on)
        session_enter();
}

}
