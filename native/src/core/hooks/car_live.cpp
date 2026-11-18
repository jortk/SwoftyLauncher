#include "../runtime.hpp"

#include <swofty/mem.hpp>
#include <swofty/mixin.hpp>

#include <atomic>
#include <cstring>
#include <memory>
#include <string>
#include <unordered_set>
#include <vector>

#include "../apply/content/race_fuel.hpp"
#include "../perf.hpp"
#include "../registry/table.hpp"
#include "car_live.hpp"
#include "car_profile.hpp"
#include "context.hpp"
#include "core_events.hpp"
#include "hooks.hpp"
#include "player_facts.hpp"
#include "../view_state.hpp"
#include "session_points.hpp"

// The live car operations on the running game (car_live.hpp): g_Sess_VehicleTable and g_Sess_VehicleCount
// resolve once the code is ready, memory reads and writes are checked (swofty::mem), writes run only on the
// flow thread (the one that ticks on_frame), and nothing is handed over on an exe outside car_live::CAR_BUILDS,
// the builds whose field offsets these are. car.built and car.reset are game points: the mid-hook on site_Car_LiveCarBuilt
// and the detour on Car_Systems_Reset_Thunk sit there while an addon subscribes, and the events fire from the next
// frame (car_live.hpp, Ops::take).

extern "C" __declspec(dllexport) const SwoftyLive *SwoftyCoreLive(void);

namespace swofty_core::hooks {
namespace {

namespace mem = swofty::mem;

struct CoreTag {};
using Ops = car_live::Ops<CoreTag>;

std::atomic<uint32_t> g_flow_tid{0};

const car_live::CarBuild *car_build()
{
    static const car_live::CarBuild *build = [] {
        runtime::ImageId id = runtime::image_id();
        return car_live::car_build_for(id.stamp, id.size);
    }();
    return build;
}

bool on_flow_thread()
{
    uint32_t t = g_flow_tid.load();
    return !t || t == GetCurrentThreadId();
}

std::unique_ptr<session_points::MidPoint> g_built, g_setup;
std::unique_ptr<GamePoint> g_reset, g_replay, g_wing, g_hit, g_pit;

// The KB rows the live levers rest on. A row is ready once it has an independent agree verify row (re/PROTOCOL.md
// section 5); the others stay off and log "<field> off: needs <row>". Flip a row here after the lead's verify agent
// has recorded its agree row.
struct Row {
    const char *name;
    bool ready;
};

constexpr Row ROWS[] = {
    {"site_Car_EngineDriveTorque", true},
    {"Car_Upgrades_ApplyToVehicle", true},
    {"site_Car_SetupTuningApplied", true},
    {"site_Ai_TableGripInput", true},
    {"CarF1Aero.lift_scale", true},
    {"CarF1Aero.drag_scale", true},
    {"CarF1Aero.slipstream", true},
    {"CarBrakeEcu.brake_bias", true},
    {"CarBrakeEcu.brake_pressure", true},
    {"CarSuspTyreType.grip_multiplier", false},
    {"CarSuspAxle.setup_camber", true},
    {"CarSuspAxle.setup_toe", true},
    {"CarSuspAxle.setup_anti_roll", true},
    {"CarSuspAxle.ride_height_offset", true},
    {"Car_Tuning_SetChassisZCom", true},
    {"CarChassis.tyre_physics_group", true},
    {"Car_Tyre_SetWearFactor", true},
    {"Car_FrontWing_Init", true},
    {"Car_Register_DetachablePart_Thunk", true},
    {"Car_FrontWing_OnCollision", true},
    {"Game_Pitlane_SetVehicleStatus", true},
    {"Game_Get_ModelNodeId", true},
    {"PitStopInstance.car", false},
    {"Car_FuelEcu_SetMode_Thunk", true},
    {"EngineModeAvail.fuel", true},
    {"site_Ai_BuildSystem_RecordPushed", true},
    {"AiSystem.records_begin", true},
    {"AiCarRecord.vehicle_index", true},
    {"AiCarRecord.driver_config", true},
    {"AiDriverConfig.blocking_fraction_of_follow_speed", true},
    {"AiDriverConfig.blocking_fraction_of_follow_speed_at_race_start", true},
    {"AiDriverConfig.blocking_max_blocking_distance", true},
    {"AiDriverConfig.blocking_max_blocking_distance_at_race_start", true},
    {"AiDriverConfig.alongside_speed_difference_for_limited_zone_yielding", true},
    {"AiDriverConfig.alongside_yield_overtake_rate", true},
    {"AiDriverConfig.alongside_min_presence_within_buffer", false},
    {"AiDriverConfig.divebomb_defense_max_presence_alongside", false},
    {"AiDriverConfig.divebomb_defense_max_time_to_ignore", false},
    {"AiDriverConfig.divebomb_defense_max_track_offset", false},
    {"AiDriverConfig.divebomb_defense_max_track_radius", false},
    {"AiDriverConfig.offset_variation_max_offset", true},
    {"AiDriverConfig.offset_variation_offset_limit_at_corner_start", true},
    {"AiDriverConfig.offset_variation_offset_limit_blend_distance", true},
    {"AiDriverConfig.offset_variation_half_width_scaling_graph", true},
    {"AiDriverConfig.offset_variation_radius_scalar_graph", true},
    {"AiDriverConfig.overtake_attacker_close_enough_car_lengths", true},
    {"AiDriverConfig.overtake_braking_zone_abort_overtake_time", true},
    {"AiDriverConfig.overtake_close_range_car_lengths_to_continue_overtake", true},
    {"AiDriverConfig.overtake_close_range_car_lengths_to_start_overtake", true},
    {"AiDriverConfig.overtake_oppertunist_overtake_time", true},
    {"AiDriverConfig.overtake_oppertunist_overtake_track_speed_difference", false},
    {"AiDriverConfig.overtake_danger_overtake_continue_time_adjustment", true},
    {"AiDriverConfig.overtake_danger_overtake_full_heat_time_adjustment", false},
    {"AiDriverConfig.overtake_danger_overtake_initiate_time_adjustment", true},
    {"AiDriverConfig.overtake_danger_overtake_pull_out_time", true},
    {"AiDriverConfig.overtake_regular_overtake_continue_time_adjustment", true},
    {"AiDriverConfig.overtake_regular_overtake_full_heat_time_adjustment", false},
    {"AiDriverConfig.overtake_regular_overtake_initiate_time_adjustment", true},
    {"AiDriverConfig.overtake_regular_overtake_pull_out_time", true},
    {"AiDriverConfig.overtake_overlap_percentage", true},
    {"AiDriverConfig.overtake_concede_speed_diff", true},
    {"AiDriverConfig.overtake_concede_slowdown_speed_diff", true},
    {"AiDriverConfig.overtake_concede_time_vs_behind", true},
    {"AiDriverConfig.space_seeking_added_buffer", true},
    {"AiDriverConfig.space_seeking_speed_below_follow", true},
    {"AiDriverConfig.space_seeking_first_speed_below_follow", true},
    {"AiDriverConfig.space_seeking_car_in_front_needed", true},
    {"AiDriverConfig.space_seeking_remember_crowed", false},
    {"AiDriverConfig.space_seeking_concede_overtake_blocked_by_crowed_time", true},
    {"AiDriverConfig.space_seeking_total_queue_needed", true},
    {"AiDriverConfig.space_seeking_distance_braking_point", true},
    {"AiDriverConfig.space_seeking_time_to_switch_sides", true},
    {"AiDriverConfig.space_seeking_track_distance_needed", true},
    {"AiDriverConfig.space_seeking_track_abort_distance", true},
    {"AiDriverConfig.space_seeking_cars_moving_away_speed_to_ignore", true},
    {"AiDriverConfig.space_seeking_can_start_track_radius", true},
    {"AiDriverConfig.blocking_start_blocking_intersection_time", false},
    {"AiDriverConfig.blocking_start_blocking_maximum_time_to_corner", true},
    {"AiDriverConfig.blocking_start_blocking_minimum_time_to_corner", true},
    {"AiDriverConfig.blocking_time_to_corner_for_end_merge_rate", true},
    {"AiDriverConfig.blocking_time_to_corner_for_start_merge_rate", true},
    {"AiDriverConfig.blocking_end_blocking_time_to_corner_vs_offset", true},
    {"AiDriverConfig.blocking_blocking_offset_from_car_behind", true},
    {"AiDriverConfig.blocking_blocking_speed_differential", true},
    {"AiDriverConfig.blocking_blocking_threat_time_modifier", true},
    {"AiDriverConfig.blocking_blocking_variable_offset_fraction", true},
    {"AiDriverConfig.blocking_avoidance_decrease_rate", true},
    {"AiDriverConfig.blocking_merge_back_to_racing_line", false},
};

bool row_ready(const char *name)
{
    for (const Row &r : ROWS)
        if (!std::strcmp(r.name, name))
            return r.ready;
    return false;
}

void m_built(safetyhook::Context &ctx) { Ops::built(ctx.rdi); }
void m_setup(safetyhook::Context &ctx) { Ops::setup_applied(ctx.rdi); }

void d_reset(void *loader, uint32_t arg)
{
    g_reset->original<void>(loader, arg);
    Ops::reset(reinterpret_cast<uintptr_t>(loader));
}

uint64_t d_replay(void *loader, void *name, uint32_t flag, float perc)
{
    uint64_t r = g_replay->original<uint64_t>(loader, name, flag, perc);
    Ops::replayed(reinterpret_cast<uintptr_t>(loader), perc);
    return r;
}

constexpr uintptr_t WING_CAR = 0x08, WING_SIDE_SCALE = 0x34, WING_FULL_SCALE = 0x38, WING_LEFT = 0x48, WING_RIGHT = 0x4c;
constexpr uintptr_t PITLANE_STATUS_TABLE = 0x68, PITLANE_STATUS = 0x04, PITLANE_STOPS = 0x14;
constexpr uint32_t MAX_PIT_CARS = 27;

bool detach_predicate(void *wing, float k)
{
    float full = 0, left = 0, right = 0;
    if (!mem::read(reinterpret_cast<uintptr_t>(wing) + WING_FULL_SCALE, full) ||
        !mem::read(reinterpret_cast<uintptr_t>(wing) + WING_LEFT, left) ||
        !mem::read(reinterpret_cast<uintptr_t>(wing) + WING_RIGHT, right))
        return false;
    return k * full <= left + right;
}

int32_t add_part(void *wing, const char *node, float detach_at, bool)
{
    uintptr_t reg = swofty::api->resolve("Car_Register_DetachablePart_Thunk");
    uintptr_t intern = swofty::api->resolve("Game_Get_ModelNodeId");
    if (!reg || !intern || !wing)
        return SWOFTY_ASSET_UNAVAILABLE;
    uint32_t id = reinterpret_cast<uint32_t (*)(const char *)>(intern)(node);
    struct Callback {
        bool (*fn)(void *, float);
        void *ctx;
    } cb{&detach_predicate, wing};
    alignas(16) float params[28] = {};
    int32_t kind = 2;
    std::memcpy(&params[0], &kind, sizeof kind);
    params[1] = 0.4f;
    for (int i = 0; i < 4; i++)
        params[4 + i * 5] = 1.0f;
    params[20] = params[21] = params[22] = 0.05f;
    reinterpret_cast<void (*)(void *, void *, uint32_t *, void *, float)>(reg)(wing, &cb, &id, params, detach_at);
    return SWOFTY_ASSET_OK;
}

void d_wing_init(void *wing, char reset)
{
    g_wing->original<void>(wing, reset);
    uintptr_t car = 0;
    uint8_t id = 0;
    if (reset || !mem::read(reinterpret_cast<uintptr_t>(wing) + WING_CAR, car) || !car ||
        !mem::read(car + 0xe8, id))
        return;
    events::car_parts_register(id, "", wing, [wing](const char *node, float at, bool debris) {
        return add_part(wing, node, at, debris);
    });
}

void d_wing_hit(void *wing, void *contact)
{
    float left = 0, right = 0, left_after = 0, right_after = 0;
    uintptr_t w = reinterpret_cast<uintptr_t>(wing);
    bool ok = mem::read(w + WING_LEFT, left) && mem::read(w + WING_RIGHT, right);
    g_hit->original<void>(wing, contact);
    if (ok && mem::read(w + WING_LEFT, left_after) && mem::read(w + WING_RIGHT, right_after)) {
        Ops::damaged(w, swofty::cars::events::Damage::front_wing_left, left_after - left);
        Ops::damaged(w, swofty::cars::events::Damage::front_wing_right, right_after - right);
    }
}

void d_pit_status(void *lane, uint8_t vehicle, uint32_t status)
{
    uint32_t before = 0, stops_before = 0, after = 0, stops_after = 0;
    uintptr_t entry = 0;
    auto read_entry = [&](uint32_t &state, uint32_t &stops) {
        uintptr_t base = reinterpret_cast<uintptr_t>(lane) + PITLANE_STATUS_TABLE + uintptr_t(vehicle) * 16;
        return mem::read(base, entry) && entry && mem::read(entry + PITLANE_STATUS, state) &&
               mem::read(entry + PITLANE_STOPS, stops);
    };
    bool known = vehicle < MAX_PIT_CARS && read_entry(before, stops_before);
    g_pit->original<void>(lane, vehicle, status);
    if (!known || !read_entry(after, stops_after) || after == before)
        return;
    events::session_pit(vehicle, before, after);
    if (stops_after != stops_before)
        events::session_pit_stop(vehicle, stops_after);
}

std::string session_type()
{
    auto facts = core().current();
    return facts && facts->session ? facts->session->type : std::string();
}

std::vector<car_profile::Entry> profile_entries()
{
    std::vector<car_profile::Entry> out;
    auto &store = swofty_core::registry::store();
    for (const std::string &id : store.list("car.profile", "")) {
        auto rec = store.get("car.profile", id);
        if (rec.status != swofty_core::registry::Status::ok)
            continue;
        car_profile::Entry e;
        e.id = id;
        std::string why;
        if (!car_profile::parse(rec.value, e.profile, &why)) {
            swofty::log("[cars] car.profile %s skipped: %s", id.c_str(), why.c_str());
            continue;
        }
        out.push_back(std::move(e));
    }
    return out;
}

bool within(const car_profile::Entry &e)
{
    if (e.profile.within.is_null())
        return true;
    swofty::json::Value arr = e.profile.within.is_array() ? e.profile.within : swofty::json::Value::array();
    if (!e.profile.within.is_array())
        arr.push_back(e.profile.within);
    Selectors sel;
    std::string why;
    if (!Selectors::compile(swofty::json::dump(arr), sel, &why)) {
        swofty::log("[cars] car.profile %s: within does not compile: %s", e.id.c_str(), why.c_str());
        return false;
    }
    auto facts = core().current();
    return facts && sel.match(*facts);
}

std::unordered_set<std::string> g_logged;

car_profile::Memo g_profiled;

car_profile::Memo::Key profile_key(bool ai)
{
    car_profile::Memo::Key k;
    k.store = swofty_core::registry::store().generation();
    auto facts = core().current();
    k.epoch = facts ? facts->epoch : 0;
    k.tick = GetTickCount64() / 1000;
    k.session = session_type();
    k.ai = ai;
    return k;
}

void apply_profiles(uint32_t index, bool force = false)
{
    static const uint32_t probe = perf::probe("core", "car_live profiles");
    perf::Scope scope(probe);
    uint32_t is_ai = 1;
    Ops::op_car(index, nullptr, &is_ai);
    auto key = profile_key(is_ai != 0);
    if (force)
        g_profiled.forget(index);
    if (g_profiled.fresh(index, key))
        return;
    g_profiled.note(index, key);
    auto entries = profile_entries();
    auto report = car_profile::Applier<Ops>::evaluate(entries, session_type(), within, index, is_ai != 0);
    for (const auto &id : report.needs_identity)
        if (g_logged.insert("identity:" + id).second)
            swofty::log("[cars] car.profile %s skipped: its cars selector needs the car's vehicle, team or driver (T121)",
                        id.c_str());
    for (const auto &why : report.refused)
        if (g_logged.insert("refused:" + why).second)
            swofty::log("[cars] car.profile layer refused: %s (the field is off or unknown)", why.c_str());
    for (const auto &e : entries)
        if (e.profile.fuel_modes && g_logged.insert("fuel:" + e.id).second)
            swofty::log("[cars] car.profile %s: fuel_modes off: the fuel mode call is not bound", e.id.c_str());
    if (report.matched && g_logged.insert("applied:" + std::to_string(index) + session_type()).second)
        swofty::log("[cars] car %u: %zu car.profile layer(s) from %zu profile(s) in a %s session", index, report.layers,
                    report.matched, session_type().c_str());
}

uint64_t g_epoch = 0;
player_facts::Cache g_players;

void sync_players()
{
    auto facts = core().current();
    if (!facts)
        return;
    auto change = player_facts::sync<Ops>(g_players, *facts, session_control_mode);
    if (!change)
        return;
    const bool seen = facts->epoch == g_epoch;
    update_facts([&](Facts &f) {
        f.player = std::move(change->player);
        f.participants = std::move(change->participants);
    });
    if (seen)
        g_epoch = core().current()->epoch;
}

// A box's fuel rules (apply/content/race_fuel.hpp): the era picker hands over the pressed box's rules, and every frame
// the plan reads each car's fuel and pit stop counter (the live snapshot) and writes the fuel it decides.
std::mutex g_fuel_lock;
apply::content::FuelRules g_fuel_rules;
bool g_fuel_new = false;
apply::content::FuelPlan g_fuel_plan;
std::atomic<bool> g_tyres_kept{false};
std::atomic<uint32_t> g_session_type{0};   // the live session type, read each frame while a box rule needs it

void fuel_tick()
{
    apply::content::FuelRules rules;
    {
        std::lock_guard<std::mutex> g(g_fuel_lock);
        if (g_fuel_new) {
            g_fuel_new = false;
            g_fuel_plan.reset();
        }
        if (!g_fuel_rules.any() && !g_fuel_rules.tyres_kept())
            return;
        rules = g_fuel_rules;
    }
    const SwoftyLive *live = SwoftyCoreLive();
    SwoftyLiveSession ses{};
    if (!live || live->session(&ses) != SWOFTY_ASSET_OK || !(ses.have & SWOFTY_LIVE_SHAVE_TYPE)) {
        g_session_type.store(0, std::memory_order_relaxed);
        return;
    }
    g_session_type.store(ses.type, std::memory_order_relaxed);
    if (!rules.any())
        return;
    if (!apply::content::fuel_quali_session(ses.type) && !apply::content::fuel_race_session(ses.type))
        return;
    uint32_t n = 0;
    if (Ops::op_count(&n) != SWOFTY_ASSET_OK || !n)
        return;
    n = std::min<uint32_t>(n, apply::content::FuelPlan::MAX);
    apply::content::FuelCar cars[apply::content::FuelPlan::MAX];
    mem::Trust trust(runtime::HEAP_TRUST_MS);
    for (uint32_t i = 0; i < n; i++) {
        SwoftyLiveCar c{};
        auto &f = cars[i];
        if (live->car(i, &c) != SWOFTY_ASSET_OK || !(c.flags & SWOFTY_LIVE_CAR_RACER) ||
            Ops::fuel_of(i, f.fuel, f.capacity) != SWOFTY_ASSET_OK)
            continue;
        f.present = true;
        f.lap = c.lap;
        f.pit_stops_ok = (c.have & SWOFTY_LIVE_HAVE_PIT_LANE) != 0;
        f.pit_stops = c.pit_stops;
    }
    for (const auto &w : g_fuel_plan.tick(rules, ses.type, ses.generation, cars, n)) {
        int r = Ops::op_set(w.car, nullptr, SWOFTY_CAR_FUEL_MASS, w.kg);
        if (r != SWOFTY_ASSET_OK)
            g_fuel_plan.failed(w.car);
        swofty::log("[fuel] car %u: %s, %.1f -> %.1f kg%s", w.car, w.why, cars[w.car].fuel, w.kg,
                    r == SWOFTY_ASSET_OK ? "" : " (not written yet)");
    }
}

void frame(double, void *)
{
    static const uint32_t unlock_probe = perf::probe("core", "car_live unlock_tick");
    static const uint32_t apply_probe = perf::probe("core", "car_live apply_car");
    static const uint32_t flush_probe = perf::probe("core", "car_live flush");
    g_flow_tid = GetCurrentThreadId();
    sync_players();
    auto facts = core().current();
    bool changed = facts && facts->epoch != g_epoch;
    if (facts)
        g_epoch = facts->epoch;
    if (changed) {
        mem::invalidate();
        Ops::racecraft_forget();
        uint32_t n = 0;
        if (Ops::op_count(&n) == SWOFTY_ASSET_OK)
            for (uint32_t i = 0; i < n; i++)
                apply_profiles(i);
        Ops::flush();
    }
    {
        mem::Trust trust(runtime::HEAP_TRUST_MS);
        perf::Scope scope(unlock_probe);
        Ops::unlock_tick();
        Ops::racecraft_tick();
    }
    const view::State view_now = view::current();
    const bool in_replay = view_now.valid && (view_now.flags & view::REPLAY);
    if (Ops::resume_due(in_replay)) {
        uint32_t n = 0;
        if (Ops::op_count(&n) == SWOFTY_ASSET_OK) {
            mem::Trust trust(runtime::HEAP_TRUST_MS);
            for (uint32_t i = 0; i < n; i++) {
                apply_profiles(i, true);
                Ops::apply_car(i);
            }
        }
    }
    uint32_t profiled = 0, applied = 0;
    for (const auto &p : Ops::take_frame()) {
        swofty::cars::live::Car car(p.index, reinterpret_cast<void *>(p.car));
        uint32_t bit = 1u << p.index;
        const bool hold = Ops::skip_in_replay(p.what, in_replay);
        bool rebuilt = p.what == Ops::What::built || p.what == Ops::What::reset;
        if (!hold && p.what != Ops::What::setup && (rebuilt || !(profiled & bit))) {
            profiled |= bit;
            apply_profiles(p.index, rebuilt);
        }
        if (!hold && !(applied & bit)) {
            applied |= bit;
            mem::Trust trust(runtime::HEAP_TRUST_MS);
            perf::Scope scope(apply_probe);
            Ops::apply_car(p.index);
        }
        switch (p.what) {
        case Ops::What::built: events::car_built(car); break;
        case Ops::What::reset: events::car_reset(car, false); break;
        case Ops::What::replayed: events::car_upgrades_replayed(car, p.progress); break;
        case Ops::What::setup: events::car_setup_applied(car); break;
        case Ops::What::damage: events::car_damage(car, p.part, p.progress); break;
        }
    }
    mem::Trust trust(runtime::HEAP_TRUST_MS);
    perf::Scope scope(flush_probe);
    Ops::flush();
    fuel_tick();
}

void setup()
{
    car_live::Env e;
    e.read = [](uintptr_t a, void *out, size_t n) {
        if (!mem::readable(a, n))
            return false;
        std::memcpy(out, reinterpret_cast<const void *>(a), n);
        return true;
    };
    e.write = [](uintptr_t a, const void *in, size_t n) {
        if (!mem::readable(a, n))
            return false;
        std::memcpy(reinterpret_cast<void *>(a), in, n);
        return true;
    };
    e.flow_thread = on_flow_thread;
    e.enabled = [] { return runtime::enabled("events", nullptr); };
    e.row_ready = row_ready;
    const car_live::CarBuild *build = car_build();
    if (!build) {
        runtime::ImageId id = runtime::image_id();
        swofty::log("[cars] live car operations off: build %u (image 0x%x) has no checked car layouts", id.stamp, id.size);
    } else {
        e.table = swofty::api->resolve("g_Sess_VehicleTable");
        e.count = swofty::api->resolve("g_Sess_VehicleCount");
        if (!e.table || !e.count)
            swofty::log("[cars] live car operations off: g_Sess_VehicleTable or g_Sess_VehicleCount does not resolve");
        else
            swofty::log("[cars] live car operations on (%s): %zu fields", build->name,
                        sizeof car_live::FIELDS / sizeof car_live::FIELDS[0]);
        if (uintptr_t fn = swofty::api->resolve("Car_Engine_ComputeDerived_Thunk"))
            e.engine_derived = [fn](uintptr_t engine) {
                reinterpret_cast<void (*)(void *)>(fn)(reinterpret_cast<void *>(engine));
            };
        else
            swofty::log("[cars] engine_limiter off: Car_Engine_ComputeDerived_Thunk does not resolve");
        if (uintptr_t fn = swofty::api->resolve("Car_Tuning_SetFinalDrive"))
            e.gearbox_reset = [fn](uintptr_t gearbox, uint32_t flags) {
                reinterpret_cast<void (*)(void *, uint32_t)>(fn)(reinterpret_cast<void *>(gearbox), flags);
            };
        else
            swofty::log("[cars] final_drive and gear ratios off: Car_Tuning_SetFinalDrive does not resolve");
        e.chassis_reset = [](uintptr_t chassis, uint32_t flags) {
            uintptr_t vt = 0, fn = 0;
            if (!mem::object_like(chassis) || !mem::read(chassis, vt) || !mem::read(vt + 0x18, fn) || !mem::in_image(fn))
                return;
            reinterpret_cast<void (*)(void *, uint32_t)>(fn)(reinterpret_cast<void *>(chassis), flags);
        };
        if (uintptr_t fn = swofty::api->resolve("Car_Tyre_SetWearFactor"))
            e.tyre_refit = [fn](uintptr_t component) {
                reinterpret_cast<void (*)(void *, bool, uint16_t)>(fn)(reinterpret_cast<void *>(component), true, 0x101);
            };
        for (const car_live::Field &f : car_live::FIELDS)
            if (f.needs && !row_ready(f.needs) && g_logged.insert(std::string("off:") + f.needs).second)
                swofty::log("[cars] %s off: needs %s", f.name, f.needs);
        swofty::log("[session] session.pit_stop stationary time and tyre change off: needs PitStopInstance.car");
        swofty::log("[cars] refit_tyres off: needs Car_Tyre_SetWearFactor, CarChassis.tyre_physics_group and the car to tyre component path");
        if (uintptr_t fn = swofty::api->resolve("Car_FuelEcu_SetMode_Thunk"))
            e.fuel_set_mode = [fn](uintptr_t ecu, int mode) {
                reinterpret_cast<void (*)(void *, int)>(fn)(reinterpret_cast<void *>(ecu), mode);
            };
        else
            swofty::log("[cars] fuel_ecu fields off: Car_FuelEcu_SetMode_Thunk does not resolve");
        e.mode_avail = [rva = build->mode_avail_rva] {
            uintptr_t at = swofty::api->module_base() + rva, ptr = 0;
            return mem::read(at, ptr) && mem::object_like(ptr) ? ptr : uintptr_t(0);
        };
    }
    Ops::bind(std::move(e));
    const SwoftyEvents *ev = runtime::events_table();
    uint32_t token = 0;
    if (ev && ev->on_frame) {
        runtime::frame_probe_name("car_live");
        ev->on_frame(frame, nullptr, &token);
    }
}

}

void race_fuel_rules(const apply::content::FuelRules &rules)
{
    std::lock_guard<std::mutex> g(g_fuel_lock);
    if (rules.line == g_fuel_rules.line && rules.package == g_fuel_rules.package)
        return;
    g_fuel_rules = rules;
    g_fuel_new = true;
    g_tyres_kept.store(rules.tyres_kept(), std::memory_order_relaxed);
    if (rules.any() || !rules.line.empty())
        swofty::log("[fuel] %s", rules.line.empty() ? "the game's own fuel" : rules.line.c_str());
}

// The pit stop tyre change wrap (pit_box.cpp) asks whether to skip the change for this tyre component and call flag.
bool race_tyres_keep(uintptr_t tyre_component, bool pit_stop_call)
{
    bool rule = g_tyres_kept.load(std::memory_order_relaxed);
    uint32_t type = g_session_type.load(std::memory_order_relaxed);
    if (!rule || !pit_stop_call)
        return false;
    uint32_t damage = 0;
    uint64_t count = 0;
    if (swofty::mem::read(tyre_component + apply::content::TYRE_WHEEL_COUNT, count) && count <= 16)
        for (uint64_t i = 0; i < count; i++) {
            uint32_t f = 0;
            if (swofty::mem::read(tyre_component + apply::content::TYRE_WHEELS + i * apply::content::TYRE_WHEEL_STRIDE +
                                      apply::content::TYRE_WHEEL_FLAGS,
                                  f))
                damage |= f;
        }
    uintptr_t entity = 0, car = 0;
    uint8_t vehicle = 0xff;
    if (swofty::mem::read(tyre_component + 0x20, entity) && entity && swofty::mem::read(entity + 0x58, car) && car)
        swofty::mem::read(car + car_live::CAR_VEHICLE_ID, vehicle);
    auto pick = apply::content::race_tyre_change_pick(rule, type, pit_stop_call, damage);
    if (pick == apply::content::TyreChange::game)
        return false;
    swofty::log("[tyres] vehicle %u pit stop in the race: %s (tyre record flags %#x)", unsigned(vehicle),
                pick == apply::content::TyreChange::kept ? "tyres kept (box rule race_tyre_change false)"
                                                         : "tyres changed (a tyre record carries a damage flag)",
                damage);
    return pick == apply::content::TyreChange::kept;
}

void declare_car_points(Table &t)
{
    g_built = std::make_unique<session_points::MidPoint>("car.built", "site_Car_LiveCarBuilt", &m_built, game_ops());
    t.declare_game("car.built", SWOFTY_POINT_NOTIFY, swofty::cars::events::Built::version, g_built.get());
    g_reset = std::make_unique<GamePoint>("car.reset", "Car_Systems_Reset_Thunk", reinterpret_cast<void *>(&d_reset),
                                          game_ops());
    t.declare_game("car.reset", SWOFTY_POINT_NOTIFY, swofty::cars::events::Reset::version, g_reset.get());
    g_replay = std::make_unique<GamePoint>("car.upgrades.replayed", "Car_Upgrades_ApplyToVehicle",
                                           reinterpret_cast<void *>(&d_replay), game_ops());
    t.declare_game("car.upgrades.replayed", SWOFTY_POINT_NOTIFY, swofty::cars::events::UpgradesReplayed::version,
                   g_replay.get());
    g_wing = std::make_unique<GamePoint>("car.parts.register", "Car_FrontWing_Init", reinterpret_cast<void *>(&d_wing_init),
                                         game_ops());
    t.declare_game("car.parts.register", SWOFTY_POINT_NOTIFY, swofty::cars::events::PartsRegister::version, g_wing.get());
    g_hit = std::make_unique<GamePoint>("car.damage", "Car_FrontWing_OnCollision", reinterpret_cast<void *>(&d_wing_hit),
                                        game_ops());
    t.declare_game("car.damage", SWOFTY_POINT_NOTIFY, swofty::cars::events::Damage::version, g_hit.get());
    g_pit = std::make_unique<GamePoint>("session.pit", "Game_Pitlane_SetVehicleStatus",
                                        reinterpret_cast<void *>(&d_pit_status), game_ops());
    t.declare_game("session.pit", SWOFTY_POINT_NOTIFY, swofty::sessions::events::Pit::version, g_pit.get());
    t.declare_game("session.pit_stop", SWOFTY_POINT_NOTIFY, swofty::sessions::events::PitStop::version, g_pit.get());
    g_setup = std::make_unique<session_points::MidPoint>("car.setup.applied", "site_Car_SetupTuningApplied", &m_setup,
                                                         game_ops());
    t.declare_game("car.setup.applied", SWOFTY_POINT_NOTIFY, swofty::cars::events::SetupApplied::version, g_setup.get());
}

void arm_car_points()
{
    if (!car_build())
        return;
    g_built->keep(true);
    g_built->install(false, "the core");
    g_reset->keep(true);
    g_reset->install(false, "the core");
    if (row_ready("Car_Upgrades_ApplyToVehicle")) {
        g_replay->keep(true);
        g_replay->install(false, "the core");
    }
    if (row_ready("Game_Pitlane_SetVehicleStatus")) {
        g_pit->keep(true);
        g_pit->install(false, "the core");
    }
    if (row_ready("site_Car_SetupTuningApplied")) {
        g_setup->keep(true);
        g_setup->install(false, "the core");
    }
}

}

namespace {

constexpr const char *LEVERS = "swofty_core.dll (car levers)";
constexpr const char *RACECRAFT = "swofty_core.dll (ai racecraft)";
bool g_levers_offered = false;

void engine_torque(SwoftyContext *c)
{
    c->xmm0.f32[0] = swofty_core::hooks::Ops::drive_torque_with(c->rbx, c->xmm0.f32[0], c->xmm7.f32[0], [&] {
        float limiter = 0;
        swofty::mem::read(c->rbx + 0x2528, limiter);
        return limiter;
    });
}

void ai_system_built(SwoftyContext *c)
{
    swofty_core::hooks::Ops::racecraft_pushed(c->rbx);
}

void ai_grip(SwoftyContext *c)
{
    c->xmm1.f32[0] = swofty_core::hooks::Ops::ai_grip(uint32_t(c->r12), uint8_t(c->rcx), c->xmm1.f32[0]);
}

}

namespace runtime {

std::vector<CoreMixins> car_levers_mixins()
{
    std::vector<CoreMixins> out;
    if (g_levers_offered || !code_ready() || !swofty_core::hooks::car_build())
        return out;
    g_levers_offered = true;
    std::vector<SwoftyMixin> mixins;
    if (swofty_core::hooks::row_ready("site_Car_EngineDriveTorque") && swofty::api->resolve("site_Car_EngineDriveTorque"))
        mixins.push_back({"site_Car_EngineDriveTorque", SWOFTY_MIXIN_SITE, 100, (void *)&engine_torque, nullptr});
    else
        swofty::log("[cars] engine.power_scale and engine.limiter_offset_rpm off: needs site_Car_EngineDriveTorque");
    if (swofty_core::hooks::row_ready("site_Ai_TableGripInput") && swofty::api->resolve("site_Ai_TableGripInput"))
        mixins.push_back({"site_Ai_TableGripInput", SWOFTY_MIXIN_SITE, 100, (void *)&ai_grip, nullptr});
    else
        swofty::log("[cars] ai.grip_scale off: needs site_Ai_TableGripInput");
    bool racecraft_rows = true;
    for (const char *row : swofty_core::hooks::car_live::RACECRAFT_ROWS)
        racecraft_rows = racecraft_rows && swofty_core::hooks::row_ready(row);
    if (racecraft_rows && swofty::api->resolve("site_Ai_BuildSystem_RecordPushed"))
        out.push_back({RACECRAFT,
                       {{"site_Ai_BuildSystem_RecordPushed", SWOFTY_MIXIN_SITE, 50, (void *)&ai_system_built, nullptr}}});
    else
        swofty::log("[ai] ai.<group>.<key> fields off: need AiSystem.records_begin, AiCarRecord.vehicle_index and "
                    "driver_config and site_Ai_BuildSystem_RecordPushed");
    if (!mixins.empty())
        out.push_back({LEVERS, std::move(mixins)});
    return out;
}

void car_levers_commit()
{
    if (g_levers_offered)
        swofty::log("[cars] car levers %s", core_mixins_active(LEVERS) ? "armed" : "did not install (see the [core] lines)");
}

}

extern "C" __declspec(dllexport) const SwoftyCarOps *SwoftyCoreCarOps(void)
{
    return &swofty_core::hooks::Ops::table;
}

SWOFTY_ON_LOAD
{
    runtime::when_code_ready("live car operations", swofty_core::hooks::setup);
}
