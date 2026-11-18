#include "../runtime.hpp"

#include "../apply/ids.hpp"
#include "../apply/moments.hpp"
#include "../apply/tracks/base.hpp"
#include "../direct_start_tt.hpp"
#include "../era_logic.hpp"
#include "../hooks/core_events.hpp"
#include "../hooks/guard.hpp"
#include "../hooks/hooks.hpp"
#include "../live_car.hpp"
#include "../view_state.hpp"
#include "lap_host.hpp"

#include <swofty/mixin.hpp>
#include <swofty/systems/laps.hpp>

#include <atomic>
#include <deque>
#include <mutex>

// The lap host (swofty_api.h SwoftyLapHost, the core export SwoftyCoreLapHost): what a lap module needs from the
// core beyond the SDK, and the module's callbacks the core calls where it has the moment (flow tick, session
// enter and end, a new lap, hook set offer and settle).
namespace runtime {

std::vector<era::Box> era_boxes();
void player_name_probe_tick();
void player_name_pin_tick();

namespace lap_host {
namespace {

std::atomic<const SwoftyLapClient *> client{nullptr};
SwoftyLapClient kept{};

struct Set {
    std::string name;
    std::vector<SwoftyMixin> mixins;
    std::deque<std::string> targets;
    bool offered = false;
};
std::mutex sets_lock;
std::deque<Set> sets;

const char *keep(std::string s)
{
    thread_local std::string out;
    out = std::move(s);
    return out.c_str();
}

int attach(const SwoftyLapClient *c)
{
    if (!c) {
        client.store(nullptr, std::memory_order_release);
        swofty::log("[lap-host] the lap module detached");
        return 1;
    }
    if (c->size < sizeof(SwoftyLapClient) || c->version < 1) {
        swofty::log("[lap-host] attach refused: client size %u version %u", c->size, c->version);
        return 0;
    }
    client.store(nullptr, std::memory_order_release);
    kept = *c;
    client.store(&kept, std::memory_order_release);
    swofty::log("[lap-host] a lap module attached (client version %u)", c->version);
    return 1;
}

int add_hooks(const char *name, const SwoftyMixin *mixins, size_t count)
{
    if (!name || !mixins || !count || core_mixins_closed())
        return 0;
    std::lock_guard<std::mutex> g(sets_lock);
    for (const Set &s : sets)
        if (s.name == name)
            return 0;
    Set &s = sets.emplace_back();
    s.name = name;
    for (size_t i = 0; i < count; i++) {
        SwoftyMixin m = mixins[i];
        m.target = s.targets.emplace_back(m.target ? m.target : "").c_str();
        s.mixins.push_back(m);
    }
    return 1;
}

int hooks_active(const char *name)
{
    return name && core_mixins_active(name);
}

int host_when_code_ready(const char *what, void (*fn)(void))
{
    if (!fn)
        return 0;
    static std::mutex names_lock;
    static std::deque<std::string> names;
    const char *kept_name;
    {
        std::lock_guard<std::mutex> g(names_lock);
        kept_name = names.emplace_back(what ? what : "lap module").c_str();
    }
    when_code_ready(kept_name, fn);
    return 1;
}

int host_code_ready()
{
    return code_ready();
}

uint32_t build()
{
    return (same_build() ? 1u : 0u) | (verified_build() ? 2u : 0u);
}

const char *build_sha()
{
    return keep(build_exe_sha());
}

int feature(const char *name)
{
    return name && enabled(name, nullptr);
}

const char *host_swofty_root()
{
    return keep(narrow(swofty_root()));
}

const char *host_files_root()
{
    return keep(narrow(files_root()));
}

int host_session_kind()
{
    return session_kind();
}

int mode_kind()
{
    static std::atomic<bool> watching{false};
    if (!watching.exchange(true))
        swofty_core::apply::moments::mode_watch();
    return int(swofty_core::apply::moments::mode_kind());
}

const char *session_mode()
{
    return keep(swofty_core::apply::moments::session_mode_track().first);
}

const char *session_content()
{
    auto facts = swofty_core::hooks::core().current();
    return keep(facts ? facts->content : std::string());
}

const char *content_title(const char *content)
{
    std::string want = content ? content : "";
    for (const auto &box : era_boxes())
        if (!box.title.empty() && (box.owner + "/" + box.entrant_list == want || box.time_trial == want))
            return keep(box.title);
    return keep({});
}

const char *player_label(uint32_t what)
{
    auto facts = swofty_core::hooks::core().current();
    if (!facts || !facts->player)
        return keep({});
    return keep(what == 0 ? facts->player->team : what == 1 ? facts->player->vehicle : what == 2 ? facts->player->driver : std::string());
}

uint32_t view_flags()
{
    return view::current().flags;
}

void *game_manager()
{
    return direct_start_context().game_manager;
}

int host_flow_link_listen(SwoftyLapFlowLinkFn listener)
{
    return listener && flow_link_listen(listener);
}

const char *time_trial_track(int session)
{
    return keep(session ? direct_start::tt::session_track() : direct_start::tt::current_track());
}

const char *time_trial_list()
{
    return keep(direct_start::tt::player_list());
}

int time_trial_team(uint32_t *ref, const char **label)
{
    std::string name;
    uint32_t r = 0;
    bool selected = direct_start::tt::player_team(r, name);
    if (ref)
        *ref = r;
    if (label)
        *label = keep(std::move(name));
    return selected;
}

const char *time_trial_team_of(uint32_t ref)
{
    return keep(direct_start::tt::team_of(ref));
}

int time_trial_track_stat()
{
    return direct_start::tt::track_stat();
}

int time_trial_setup(const char *request, const char **why)
{
    json::Value v;
    std::string said;
    direct_start::Setup s = direct_start::Setup::failed;
    if (!request || !json::parse(request, v))
        said = "the request is not JSON";
    else
        s = direct_start::tt::setup(v, said);
    if (why)
        *why = keep(std::move(said));
    return s == direct_start::Setup::done ? 1 : s == direct_start::Setup::pending ? 0 : -1;
}

void time_trial_setup_reset()
{
    direct_start::tt::reset_for_replay();
}

void press_list(const char *entrant_list)
{
    if (entrant_list)
        era_press(entrant_list);
}

int session_unlisted(const char **list)
{
    std::string name;
    bool unlisted = era_session_unlisted(name);
    if (list)
        *list = keep(std::move(name));
    return unlisted;
}

int track(uint32_t index, SwoftyLapTrack *out)
{
    const auto &base = swofty_core::apply::tracks::base();
    if (!out || index >= base.tracks.size())
        return 0;
    const auto &t = base.tracks[index];
    *out = {t.id.c_str(), t.code.c_str(), t.base.c_str(), t.addon.c_str(), t.name.c_str(), int32_t(t.stat_id), uint32_t(t.kind), t.time_trial ? 1u : 0u};
    return 1;
}

uint32_t content_ids(const char *registry, int64_t *out, uint32_t cap)
{
    if (!registry)
        return 0;
    std::vector<int64_t> ids = swofty_core::apply::ids::store().ids_of(registry);
    for (uint32_t i = 0; out && i < cap && i < ids.size(); i++)
        out[i] = ids[i];
    return uint32_t(ids.size());
}

int car(SwoftyLapCar *out)
{
    if (!out || out->size < sizeof(SwoftyLapCar))
        return 0;
    live_car::LiveCar c;
    bool ok = live_car::read(c);
    SwoftyLapCar o{};
    o.size = sizeof o;
    o.has = (c.has_speed ? SWOFTY_LAP_CAR_SPEED : 0) | (c.has_pedals ? SWOFTY_LAP_CAR_PEDALS : 0) | (c.has_steer ? SWOFTY_LAP_CAR_STEER : 0) |
            (c.has_gear ? SWOFTY_LAP_CAR_GEAR : 0) | (c.has_rpm ? SWOFTY_LAP_CAR_RPM : 0) | (c.has_g ? SWOFTY_LAP_CAR_G : 0) |
            (c.has_surface ? SWOFTY_LAP_CAR_SURFACE : 0) | (c.has_wetness ? SWOFTY_LAP_CAR_WETNESS : 0) |
            (c.has_wheel_wetness ? SWOFTY_LAP_CAR_WHEEL_WETNESS : 0);
    o.wetness = c.wetness;
    o.wheel_wetness = c.wheel_wetness;
    o.speed_kmh = c.speed_kmh;
    o.throttle = c.throttle;
    o.brake = c.brake;
    o.steer = c.steer;
    o.rpm = c.rpm;
    o.max_rpm = c.max_rpm;
    o.g_lateral = c.g_lateral;
    o.g_long = c.g_long;
    o.gear = c.gear;
    o.player = c.player;
    o.ai = c.ai;
    std::memcpy(o.surface, c.surface, sizeof o.surface);
    *out = o;
    return ok;
}

int motion(SwoftyLapMotion *out)
{
    if (!out || out->size < sizeof(SwoftyLapMotion))
        return 0;
    live_car::LiveDynamics d;
    bool ok = live_car::read_dynamics(d);
    SwoftyLapMotion o{};
    o.size = sizeof o;
    o.has = (d.has_pos ? SWOFTY_LAP_MOTION_POS : 0) | (d.has_vel ? SWOFTY_LAP_MOTION_VEL : 0) | (d.has_angvel ? SWOFTY_LAP_MOTION_ANGVEL : 0) |
            (d.has_orient ? SWOFTY_LAP_MOTION_ORIENT : 0) | (d.has_spin ? SWOFTY_LAP_MOTION_SPIN : 0) | (d.has_slip ? SWOFTY_LAP_MOTION_SLIP : 0) |
            (d.has_drs ? SWOFTY_LAP_MOTION_DRS : 0);
    std::memcpy(o.pos, d.pos, sizeof o.pos);
    std::memcpy(o.vel, d.vel, sizeof o.vel);
    std::memcpy(o.angvel, d.angvel, sizeof o.angvel);
    std::memcpy(o.quat, d.quat, sizeof o.quat);
    std::memcpy(o.spin, d.spin, sizeof o.spin);
    std::memcpy(o.slip_angle, d.slip_angle, sizeof o.slip_angle);
    std::memcpy(o.slip_ratio, d.slip_ratio, sizeof o.slip_ratio);
    o.drs = d.drs;
    o.player = d.player;
    o.ai = d.ai;
    *out = o;
    return ok;
}

uint32_t car_state()
{
    return (live_car::available() ? 1u : 0u) | (live_car::kb_fields() ? 2u : 0u);
}

uintptr_t player_car()
{
    return live_car::player_car();
}

int sealing(const char *path, int32_t time_ms, const uint8_t *body, uint32_t body_len, SwoftyLapSectionFn add_section, void *user)
{
    swofty::laps::events::Sealing ev{};
    ev.path_str = path;
    ev.time_ms = time_ms;
    ev.body = body;
    ev.body_len = body_len;
    ev.add_section_fn = add_section;
    ev.user = user;
    return swofty_core::hooks::events::laps_sealing(ev);
}

int recorded(const char *path, int32_t time_ms, int32_t valid)
{
    return swofty_core::hooks::events::laps_recorded(path, time_ms, valid);
}

int copy(void *to, const void *from, size_t n)
{
    return swofty_core::hooks::guard::copy_memory(to, from, n);
}

std::mutex notice_lock;
std::string notice_text;
std::atomic<bool> notice_on{false};

void notice(const char *text)
{
    std::string next = text ? text : "";
    std::lock_guard<std::mutex> g(notice_lock);
    if (next == notice_text)
        return;
    notice_text = std::move(next);
    notice_on.store(!notice_text.empty(), std::memory_order_relaxed);
}

void name_tick(int probe)
{
    if (probe)
        player_name_probe_tick();
    else
        player_name_pin_tick();
}

const SwoftyLapHost table = {sizeof(SwoftyLapHost), SWOFTY_LAP_HOST_VERSION, attach, add_hooks, hooks_active, host_when_code_ready, host_code_ready,
                             build, build_sha, feature, host_swofty_root, host_files_root, host_session_kind, mode_kind, session_mode,
                             session_content, content_title, player_label, view_flags, game_manager, host_flow_link_listen, time_trial_track,
                             time_trial_list, time_trial_team, time_trial_team_of, time_trial_track_stat, time_trial_setup,
                             time_trial_setup_reset, press_list, session_unlisted, track, content_ids, car, motion, car_state, player_car,
                             sealing, recorded, copy, notice, name_tick};

const SwoftyLapClient *live()
{
    return client.load(std::memory_order_acquire);
}

}

void tick()
{
    if (auto c = live(); c && c->tick)
        c->tick();
}

void session_enter()
{
    if (auto c = live(); c && c->session_enter)
        c->session_enter();
}

void session_watch(bool on)
{
    if (auto c = live(); c && c->session_watch)
        c->session_watch(on);
}

void session_end()
{
    if (auto c = live(); c && c->session_end)
        c->session_end();
}

void lap_line(int car_index)
{
    if (auto c = live(); c && c->lap_line)
        c->lap_line(car_index);
}

bool recording()
{
    auto c = live();
    return c && c->recording && c->recording();
}

bool board_session()
{
    auto c = live();
    return c && c->board_session && c->board_session();
}

bool player_id(presence_id::Id &out)
{
    auto c = live();
    presence_id::Id id;
    if (!c || !c->player_id || !c->player_id(&id, uint32_t(sizeof id)))
        return false;
    out = id;
    return true;
}

void screen(const char *name)
{
    if (auto c = live(); c && c->screen && name)
        c->screen(name);
}

bool notice_wanted()
{
    auto c = live();
    return c && c->notice_wanted && c->notice_wanted();
}

bool notice_drawing()
{
    return notice_on.load(std::memory_order_relaxed);
}

void notice_paint(hud::Frame &f, uint64_t)
{
    if (!notice_on.load(std::memory_order_relaxed))
        return;
    std::string text;
    {
        std::lock_guard<std::mutex> g(notice_lock);
        text = notice_text;
    }
    if (text.empty() || f.width <= 0 || f.height <= 0)
        return;
    hud::reset_state(f);
    const float W = f.width, H = f.height, k = H / 1080.0f;
    hud::TextStyle s;
    s.px = 26 * k;
    s.rgba = 0xff3b30ffu;
    s.outline_rgba = 0x000000e0u;
    s.outline_px = 2 * k;
    float w = 0, h = 0;
    hud::measure(s, text, &w, &h);
    const float room = W * 0.92f;
    if (w > room && w > 0) {
        s.px *= room / w;
        s.outline_px *= room / w;
        hud::measure(s, text, &w, &h);
    }
    const float pad_x = 18 * k, pad_y = 10 * k, top = 16 * k;
    const float x = (W - w) * 0.5f;
    hud::rect_rounded(f, x - pad_x, top, w + 2 * pad_x, h + 2 * pad_y, 8 * k, 0x0b0b0fd0u);
    hud::rect_rounded_outline(f, x - pad_x, top, w + 2 * pad_x, h + 2 * pad_y, 8 * k, 2 * k, 0xff3b30c0u);
    hud::text(f, x, top + pad_y, s, text);
}

}

std::vector<CoreMixins> lap_host_mixins()
{
    if (auto c = lap_host::live(); c && c->offer)
        c->offer();
    std::vector<CoreMixins> out;
    std::lock_guard<std::mutex> g(lap_host::sets_lock);
    for (auto &s : lap_host::sets)
        if (!std::exchange(s.offered, true))
            out.push_back({s.name.c_str(), s.mixins});
    return out;
}

void lap_host_commit()
{
    if (auto c = lap_host::live(); c && c->settle)
        c->settle();
}

}

extern "C" __declspec(dllexport) const SwoftyLapHost *SwoftyCoreLapHost(void)
{
    return &runtime::lap_host::table;
}
