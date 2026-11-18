#include <atomic>
#include "runtime.hpp"
#include "badge_images.hpp"
#include "main_menu.hpp"
#include "services/lap_host.hpp"

#include <safetyhook.hpp>
#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include "apply/moments.hpp"
#include "events_dispatch.hpp"
#include "perf.hpp"
#include "session_lifecycle_logic.hpp"

#include <cstring>
#include <mutex>
#include <string>
#include <utility>
#include <vector>

namespace runtime {
std::string hook_error(const safetyhook::MidHook::Error &e);
}

std::string settings_addon(const void *from, std::string &dll);

namespace swofty_core::hooks {
void bridge_legacy(const char *event, int64_t a, int64_t b, const void *data);
}

namespace {

const runtime::Sig S_FLOW{"Vm_Update_FlowStateMachine",
                          "41 57 41 56 41 55 41 54 56 57 55 53 48 81 EC 58 02 00 00 C5 F8 29 B4 24 40 02 00 00 C5 F9 28 F2",
                          0x4423ce0};
const runtime::Sig S_SESSION_ENTER{"Game_GameModule_OnSessionEnter",
                                   "41 57 41 56 41 54 56 57 53 48 81 EC 38 02 00 00 48 89 CE 80 3D ?? ?? ?? ?? 01",
                                   0x80c300};
const runtime::Sig S_RACE_STARTED{"Game_ChampionshipFormationLapComponent_OnRaceStarted",
                                  "41 57 41 56 41 55 41 54 56 57 55 53 48 83 EC 28 48 89 CE 80 3D ?? ?? ?? ?? 01 75 53",
                                  0xb808a0};

runtime::ev::Dispatcher<perf::Scope> disp;
runtime::session_life::Lifecycle lifecycle;
std::mutex lifecycle_lock;
safetyhook::MidHook h_flow, h_session_enter, h_race_started;
std::mutex queued_lock;
std::vector<std::pair<uint32_t, std::string>> queued_by;

double now_seconds()
{
    static LARGE_INTEGER freq = [] {
        LARGE_INTEGER f;
        QueryPerformanceFrequency(&f);
        return f;
    }();
    LARGE_INTEGER t;
    QueryPerformanceCounter(&t);
    return freq.QuadPart ? double(t.QuadPart) / double(freq.QuadPart) : 0.0;
}

uint64_t caller_module(const void *from)
{
    HMODULE m = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                       reinterpret_cast<LPCWSTR>(from), &m);
    return reinterpret_cast<uint64_t>(m);
}

std::string who(const void *from)
{
    std::string dll;
    std::string id = settings_addon(from, dll);
    return id.empty() ? dll : id;
}

struct Step {
    const char *name;
    void (*fn)();
};

const Step STEPS[] = {
    {"career_tick", [] { runtime::career_tick(); }},
    {"live_tick", [] { runtime::live_tick(now_seconds()); }},
    {"telemetry_tick", [] { runtime::telemetry_tick(); }},
    {"toast::tick", [] { runtime::toast::tick(); }},
    {"lap_host::tick", [] { runtime::lap_host::tick(); }},
    {"popup_tick", [] { runtime::popup_tick(); }},
    {"main_menu::tick", [] { runtime::main_menu::tick(); }},
    {"mfd_tick", [] { runtime::mfd_tick(); }},
    {"career_content_tick", [] { runtime::career_content_tick(); }},
    {"entitlements_tick", [] { runtime::entitlements_tick(); }},
    {"hub_tick", [] { runtime::hub_tick(); }},
    {"tyre_models_tick", [] { runtime::tyre_models_tick(); }},
    {"testagent_tick", [] { runtime::testagent_tick(); }},
    {"direct_start_tick", [] { runtime::direct_start_tick(); }},
};
constexpr size_t STEP_COUNT = sizeof STEPS / sizeof STEPS[0];

// Only the thread holding the frame claim runs the steps, so their totals need no lock.
double step_ms[STEP_COUNT];
double steps_since;
uint64_t slow_steps;

void report_steps(double now)
{
    double span = now - steps_since;
    if (span < 60.0)
        return;
    std::string line;
    for (size_t i = 0; i < STEP_COUNT; i++)
        if (step_ms[i] / span >= 1.0) {
            char b[96];
            snprintf(b, sizeof b, "%s%s %.1f", line.empty() ? "" : ", ", STEPS[i].name, step_ms[i] / span);
            line += b;
        }
    if (!line.empty())
        swofty::log("[perf] flow tick steps over 1 ms/s in the last %.0f s (ms per second): %s", span, line.c_str());
    for (double &ms : step_ms)
        ms = 0;
    steps_since = now;
}

// Once per frame: a step that holds the flow thread for over 100 ms is named in the log (first hits, then every 50th).
void run_steps()
{
    double t0 = now_seconds();
    if (!steps_since)
        steps_since = t0;
    for (size_t i = 0; i < STEP_COUNT; i++) {
        STEPS[i].fn();
        double t1 = now_seconds(), ms = (t1 - t0) * 1000.0;
        t0 = t1;
        step_ms[i] += ms;
        if (ms > 100.0) {
            uint64_t n = ++slow_steps;
            if (n <= 10 || n % 50 == 0)
                swofty::log("[perf] flow tick step %s took %.0f ms (slow step %llu)", STEPS[i].name, ms,
                            static_cast<unsigned long long>(n));
        }
    }
    report_steps(t0);
}

// The game runs the flow state machine several times a frame, on several threads; the steps and the on_frame
// callbacks run on the one tick a frame that takes the frame claim.
void flow_mid(safetyhook::Context &)
{
    static const uint32_t probe = perf::probe("core", "flow tick (events, career poll)");
    perf::flow_tick();
    if (!perf::frame_claim())
        return;
    double dt;
    {
        perf::Scope p(probe);
        run_steps();
        dt = disp.tick(now_seconds());
    }
    perf::frame_release();
    static int logged;
    if (logged < 2)
        swofty::log("[events] flow tick %d observed, dt %f; onFrame is live", ++logged, dt);
}

std::atomic<int> live_kind{runtime::session_life::NONE};
std::atomic<uint64_t> live_serial{0};

void set_kind(int kind)
{
    if (live_kind.exchange(kind, std::memory_order_relaxed) != kind)
        live_serial.fetch_add(1, std::memory_order_relaxed);
}

void note_kind()
{
    set_kind(runtime::session_life::kind_of(swofty_core::apply::moments::session_mode_track().first));
    runtime::player_name_session();
}

void session_enter_mid(safetyhook::Context &)
{
    static const uint32_t probe = perf::probe("core", "session enter");
    perf::mark("session", "enter");
    perf::Scope p(probe);
    swofty::mem::invalidate();
    swofty_core::apply::moments::session_load("Game_GameModule_OnSessionEnter");
    runtime::telemetry_session_enter();
    runtime::lap_host::session_enter();
    runtime::main_menu::session_enter();
    runtime::hud_rebuilt();
    bool restart;
    {
        std::lock_guard<std::mutex> g(lifecycle_lock);
        restart = lifecycle.on_start();
    }
    if (restart)
        runtime::session_emit({true, false, runtime::session_life::END_RESTART});
    note_kind();
    runtime::lap_host::session_watch(true);
    disp.fire(SWOFTY_EVENT_SESSION_START, 0, 0, nullptr, now_seconds());
    swofty_core::hooks::bridge_legacy(SWOFTY_EVENT_SESSION_START, 0, 0, nullptr);
}

void race_started_mid(safetyhook::Context &)
{
    static const uint32_t probe = perf::probe("core", "race start");
    perf::mark("race", "start");
    perf::Scope p(probe);
    disp.fire(SWOFTY_EVENT_RACE_START, 0, 0, nullptr, now_seconds());
    swofty_core::hooks::bridge_legacy(SWOFTY_EVENT_RACE_START, 0, 0, nullptr);
}

int ev_available()
{
    return runtime::enabled("events", nullptr) && disp.phase() != runtime::ev::Phase::failed;
}

std::string outcome(int r, uint32_t token, bool queued)
{
    if (r == SWOFTY_ASSET_UNAVAILABLE)
        return "unavailable: the flow tick did not resolve on this build, so onFrame never fires";
    if (r != SWOFTY_ASSET_OK)
        return "invalid";
    return "ok token " + std::to_string(token) + (queued ? ", queued until the events setup" : "");
}

void remember(uint32_t token, const std::string &by)
{
    std::lock_guard<std::mutex> g(queued_lock);
    queued_by.push_back({token, by});
}

int ev_subscribe(const char *event, SwoftyEventFn fn, void *user, uint32_t *token)
{
    const void *from = __builtin_return_address(0);
    if (!runtime::enabled("events", nullptr))
        return SWOFTY_ASSET_UNAVAILABLE;
    uint32_t t = 0;
    bool queued = false;
    int r = disp.subscribe(caller_module(from), event, fn, user, &t, &queued);
    if (r == SWOFTY_ASSET_OK && token)
        *token = t;
    std::string by = who(from);
    if (r == SWOFTY_ASSET_OK)
        disp.set_probe(t, perf::probe(perf::owner_of(reinterpret_cast<const void *>(fn)), std::string("event ") + event));
    if (r == SWOFTY_ASSET_OK && queued)
        remember(t, by);
    swofty::log("[events] subscribe %s by %s -> %s", event ? event : "(null)", by.c_str(), outcome(r, t, queued).c_str());
    return r;
}

thread_local const char *next_frame_probe = nullptr;

int ev_on_frame(SwoftyFrameFn fn, void *user, uint32_t *token)
{
    const void *from = __builtin_return_address(0);
    const char *probe = next_frame_probe;
    next_frame_probe = nullptr;
    if (!runtime::enabled("events", nullptr))
        return SWOFTY_ASSET_UNAVAILABLE;
    uint32_t t = 0;
    bool queued = false;
    int r = disp.on_frame(caller_module(from), fn, user, &t, &queued);
    if (r == SWOFTY_ASSET_OK && token)
        *token = t;
    std::string by = who(from);
    std::string what = probe ? std::string("on_frame ") + probe : "on_frame";
    if (r == SWOFTY_ASSET_OK)
        disp.set_probe(t, perf::probe(perf::owner_of(reinterpret_cast<const void *>(fn)), what));
    if (r == SWOFTY_ASSET_OK && queued)
        remember(t, by);
    swofty::log("[events] on_frame by %s -> %s", by.c_str(), outcome(r, t, queued).c_str());
    return r;
}

int ev_unsubscribe(uint32_t token)
{
    if (!runtime::enabled("events", nullptr))
        return SWOFTY_ASSET_UNAVAILABLE;
    int r = disp.unsubscribe(token);
    swofty::log("[events] unsubscribe token %u -> %s", token, r == SWOFTY_ASSET_OK ? "ok" : "invalid");
    return r;
}

const SwoftyEvents table = {ev_available, ev_subscribe, ev_on_frame, ev_unsubscribe};

uintptr_t resolve(const runtime::Sig &s)
{
    uintptr_t at = 0, base = swofty::api->module_base();
    int hits = swofty::api->scan(".xtls", s.pattern, &at);
    if (hits != 1 || !at) {
        swofty::log("[events] %s signature matched %d time(s); hook not installed", s.what, hits);
        return 0;
    }
    if (at - base != s.rva)
        swofty::log("[events] %s resolved to base+0x%llx, KB build has base+0x%llx", s.what,
                    (unsigned long long)(at - base), (unsigned long long)s.rva);
    return at;
}

safetyhook::MidHook mid(const runtime::Sig &s, void (*fn)(safetyhook::Context &))
{
    uintptr_t at = resolve(s);
    if (!at)
        return {};
    auto h = safetyhook::MidHook::create(reinterpret_cast<void *>(at), fn);
    if (!h) {
        swofty::log("[events] %s at base+0x%llx: the hook failed: %s", s.what,
                    (unsigned long long)(at - swofty::api->module_base()),
                    runtime::hook_error(h.error()).c_str());
        return {};
    }
    swofty::log("[events] %s hooked at base+0x%llx", s.what, (unsigned long long)(at - swofty::api->module_base()));
    return std::move(*h);
}

void report(const std::vector<runtime::ev::Queued> &held, bool live)
{
    std::vector<std::pair<uint32_t, std::string>> by;
    {
        std::lock_guard<std::mutex> g(queued_lock);
        by.swap(queued_by);
    }
    auto owner = [&](uint32_t token) {
        for (auto &[t, name] : by)
            if (t == token)
                return name;
        return std::string("an addon");
    };
    if (held.empty())
        return;
    if (live) {
        std::string list;
        for (auto &q : held)
            list += (list.empty() ? "" : ", ") + std::string(q.name ? q.name : "on_frame") + " token " +
                    std::to_string(q.token) + " (" + owner(q.token) + ")";
        swofty::log("[events] %zu registration(s) queued before setup are armed: %s", held.size(), list.c_str());
        return;
    }
    size_t subscriptions = 0;
    for (auto &q : held) {
        if (q.name) {
            subscriptions++;
            continue;
        }
        swofty::log("[events] on_frame token %u by %s was queued before setup and will not fire: the flow tick did "
                    "not resolve on this build",
                    q.token, owner(q.token).c_str());
    }
    if (subscriptions)
        swofty::log("[events] %zu event subscription(s) queued before setup stay registered; each fires only when its "
                    "source hook is armed (session.start %s, race.start %s; the career and flow events follow the "
                    "[career] setup line)",
                    subscriptions, h_session_enter ? "armed" : "off", h_race_started ? "armed" : "off");
}

void setup()
{
    if (!runtime::enabled("events", "events")) {
        swofty::log("[events] feature off; subscribe/on_frame will report unavailable");
        return;
    }
    h_flow = mid(S_FLOW, flow_mid);
    h_session_enter = mid(S_SESSION_ENTER, session_enter_mid);
    h_race_started = mid(S_RACE_STARTED, race_started_mid);
    swofty::log("[events] setup: %s; onFrame %s, session.start %s, race.start %s",
                h_flow ? "live" : "flow tick did not resolve (onFrame will not fire on this build)",
                h_flow ? "armed" : "off", h_session_enter ? "armed" : "off", h_race_started ? "armed" : "off");
    report(disp.arm(bool(h_flow)), bool(h_flow));
}

}

namespace runtime {

const SwoftyEvents *events_table()
{
    return &table;
}

void frame_probe_name(const char *what)
{
    next_frame_probe = what;
}

bool events_flow_armed()
{
    return bool(h_flow);
}

bool events_session_enter_armed()
{
    return bool(h_session_enter);
}

void events_setup()
{
    setup();
}

int session_kind()
{
    return live_kind.load(std::memory_order_relaxed);
}

uint64_t session_serial()
{
    return live_serial.load(std::memory_order_relaxed);
}

void session_emit(const session_life::Emit &e)
{
    if (e.end || e.unload) {
        garage_placement_reset();
        set_kind(session_life::NONE);
        tyre_models_drop();
    }
    if (e.end) {
        lap_host::session_end();
        swofty::log("[events] session.end (%s)", e.reason == session_life::END_RESTART  ? "restart"
                                                 : e.reason == session_life::END_UNLOAD ? "ended as the unload began"
                                                                                       : "StateSession exited");
        events_fire(SWOFTY_EVENT_SESSION_END, e.reason, 0, nullptr);
    }
    if (e.unload) {
        badge_images::session_unloaded();
        swofty::log("[events] session.unload (StateUnloadSession entering)");
        events_fire(SWOFTY_EVENT_SESSION_UNLOAD, 0, 0, nullptr);
        runtime::hud_rebuilt();
    }
}

void session_flow(const char *known, int to)
{
    if (!known || (to != 1 && to != 3))
        return;
    if (to == 1 && !std::strcmp(known, "StateLoadSession"))
        note_kind();
    session_life::Emit e;
    bool exiting = to == 3 && !std::strcmp(known, "StateSession");
    bool unloading = to == 1 && !std::strcmp(known, "StateUnloadSession");
    {
        std::lock_guard<std::mutex> g(lifecycle_lock);
        if (exiting)
            e = lifecycle.on_session_exiting();
        else if (unloading)
            e = lifecycle.on_unload_entering();
    }
    if (exiting || unloading)
        swofty::mem::invalidate();
    session_emit(e);
}

bool garage_placement_listening() { return disp.garage_listened(); }

size_t events_fire(const char *event, int64_t a, int64_t b, const void *data)
{
    if (!runtime::enabled("events", nullptr))
        return 0;
    size_t n = disp.fire(event, a, b, data, now_seconds());
    swofty_core::hooks::bridge_legacy(event, a, b, data);
    return n;
}

void events_drop_module(void *module)
{
    size_t n = disp.drop_owner(reinterpret_cast<uint64_t>(module));
    if (n)
        swofty::log("[events] dropped %zu subscription(s) from a module that unloaded or failed to init", n);
}

}
