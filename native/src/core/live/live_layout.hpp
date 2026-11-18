#pragma once
#include <cstddef>
#include <cstdint>

#include <swofty/game/car.hpp>
#include <swofty/game/live.hpp>
#include <swofty/game/session.hpp>
#include <swofty/game/teams.hpp>

// Layouts of the game's objects the live state reads, build of the KB (F1_25.exe, image base 0x140000000). Every
// value is range-checked where it is read, and a failed read leaves the value's have bit clear. The CarLive fields
// that the generated game headers declare are pinned by static_asserts below; the rest are KB rows that carry their
// own evidence in re/db/structs and re/db/verify.tsv, and the reader takes them only when the group they belong to
// is ready (live_rows.hpp).

namespace swofty_core::live::layout {

namespace car {
constexpr uintptr_t lock = 0x98, lock_cs = 0xa0, systems_vt = 0x380;
constexpr uintptr_t vehicle_id = 0xe8, is_ai = 0x124;
constexpr uintptr_t throttle = 0x188, brake = 0x190, clutch = 0x18c, input_flags = 0x224;
constexpr uintptr_t engine = 0x530, gearbox = 0x538, chassis = 0x558, steering = 0x560, f1_aero = 0x570, ers = 0x5a8;
constexpr uintptr_t lo = 0xe8, hi = 0x5b0;
static_assert(lock + 8 <= hi && systems_vt + 8 <= hi && is_ai + 4 <= hi);
static_assert(offsetof(swofty::game::CarLive, vehicle_id) == vehicle_id);
static_assert(offsetof(swofty::game::CarLive, engine) == engine);
static_assert(offsetof(swofty::game::CarLive, gearbox) == gearbox);
static_assert(offsetof(swofty::game::CarLive, chassis) == chassis);
static_assert(offsetof(swofty::game::CarLive, steering) == steering);
static_assert(offsetof(swofty::game::CarLive, f1_aero) == f1_aero);
static_assert(offsetof(swofty::game::CarLive, ers) == ers);
}

constexpr uintptr_t system_car = 0x8;

namespace engine {
constexpr uintptr_t limiter = 0x2528, rate = 0x2538;
static_assert(offsetof(swofty::game::CarEngine, limiter_rotation_rate) == limiter);
static_assert(offsetof(swofty::game::CarEngine, rotation_rate) == rate);
}

namespace gearbox {
constexpr uintptr_t gear_index = 0x680;
}

namespace chassis {
constexpr uintptr_t g_lat = 0x320, g_long = 0x324, body = 0x340, body_state = 0x18, state_velocity = 0x220,
                    state_position = 0x1c0;
}

namespace steering {
constexpr uintptr_t lock = 0x28, angle = 0xe8;
}

namespace aero {
constexpr uintptr_t drs_request = 0x1988;
}

namespace ers {
constexpr uintptr_t max_deploy = 0x28, capacity = 0x60;
constexpr uintptr_t live_lo = 0x19c8, live_hi = 0x1a6c;
constexpr uintptr_t harvest_lap = 0x19c8, deploy_lap = 0x19d0, energy = 0x19f0, lap_limit = 0x19f4;
constexpr uintptr_t harvest_k = 0x1a00, harvest_h = 0x1a04, deploy = 0x1a14, mode = 0x1a68;
}

namespace progress {
constexpr uintptr_t from_manager = 0x2d0, vec_begin = 0x38, vec_end = 0x40;
constexpr uintptr_t lo = 0x28, hi = 0xa8;
constexpr uintptr_t lap_a = 0x38, lap_b = 0x40, plap_a = 0x48, plap_b = 0x50, plap_delta = 0x58;
constexpr uintptr_t total_a = 0x98, total_b = 0xa0;
}

namespace registry {
constexpr uintptr_t from_manager = 0x30, slots = 0x18, info = 0x20;
constexpr size_t max_slots = 4096;
}

namespace results {
constexpr uintptr_t vec_begin = 0x148, vec_end = 0x150;
constexpr uintptr_t vehicle = 0x0, status = 0x18, penalty_a = 0x38, penalty_b = 0x40, pos_a = 0x48, pos_b = 0x50;
constexpr uintptr_t hi = 0x58;
}

namespace timing {
constexpr uintptr_t vec_begin = 0x30, stride = 0x98;
constexpr uintptr_t leader_ms = 0x58, leader_valid = 0x68, ahead_ms = 0x70, ahead_valid = 0x80, vehicle = 0x90;
}

namespace pit {
constexpr uintptr_t records = 0x68, state = 0x4, stops = 0x14;
constexpr uintptr_t lane_object = 0x48, lane_flags = 0x938, in_lane = 0x103;
constexpr size_t lane_flag_stride = 0x10;
constexpr unsigned max_slots = 0x1b;
}

namespace penalty_log {
constexpr uintptr_t vec_begin = 0x48, stride = 0x30;
constexpr uintptr_t vehicle = 0x08, kind = 0x09, infringement = 0x0a;
constexpr uint8_t warning = 5;
constexpr uint32_t track_limits_mask = 0x3e000380;
}

namespace misconduct {
constexpr uintptr_t drive_through = 0x568, stop_go = 0x578;
constexpr uintptr_t vec_begin = 0x68;
constexpr size_t dt_stride = 0x28, sg_stride = 4;
}

namespace safety_car {
constexpr uintptr_t state = 0x108;
}

namespace marshal {
constexpr uintptr_t vec_begin = 0x88, stride = 0x10, zone = 0x8;
constexpr uintptr_t zone_flag = 0x14, zone_def = 0x0, def_start = 0x10;
constexpr size_t max_zones = 64;
}

namespace red_flag {
constexpr uintptr_t state = 0x40;
}

namespace tyres {
constexpr uintptr_t per_vehicle = 0x110;
constexpr uintptr_t lo = 0x9bc0, hi = 0x9ca0;
constexpr uintptr_t version = 0x9bc0, list = 0x9bd0, count = 0x9c48, built_for = 0x9c50, built = 0x9c58, set_begin = 0x9c90,
                    set_end = 0x9c98;
constexpr size_t desc_stride = 0x18, set_stride = 0x78;
constexpr uintptr_t desc_physical = 0x8, desc_visual = 0x10;
constexpr uintptr_t visual_stat_id = 0x0, visual_compound = 0x50;
constexpr uintptr_t physical_stat_id = 0x0;
}

namespace controllers {
constexpr uintptr_t sets = 0x30, count_vec = 0x90, count_lo = 0x28, count_hi = 0x30, built = 0x365;
constexpr uintptr_t set_human = 0x20, set_vehicle = 0x34;
}

namespace spectate {
constexpr uintptr_t lo = 0xd0, hi = 0xe8, ui = 0xd0, active = 0xd1, vehicle = 0xd6, gap = 0xe4;
}

namespace mode {
constexpr uintptr_t from_context = 0x88, label = 0x278;
}

namespace badge {
constexpr uintptr_t mode = 0x88, game = 0x260, entrants = 0x2e0;
constexpr size_t entrant_stride = 0x88;
constexpr uintptr_t entrant_team = 0x58, entrant_vid = 0x82, tables = 0x2c8, table_vec = 0x8, slot_record = 0x8,
                    record_key = 0x4d8, record_render = 0x58d, entrant_badge = 0x68;
constexpr unsigned max_kind = 2, my_team_kind = 1, badge_none = 0x3fff;
constexpr char catalogue_prefix[] = "#overrides.osd.timing_tower.team_badges_hud.";
constexpr char render_prefix[] = "eaid://ui_package/art/textures/render_badge_";
constexpr char render_default[] = "eaid://ui_package/art/textures/render_badge_default.tga";
constexpr const char *table_names[] = {"ePreLoaded", "eUserDefined", "eNetworkReplicated"};
}

// Driver names, as Sess_Get_CarDriverName (0x142f28c10) resolves them: the car's VehicleHelperSystem participant (found
// by [[p+0x18]+0x58]+0xe8 == vehicle id) with bit 7 of its flags byte set is a human and gets the system's player name;
// any other car gets the name of the driver record its session entrant (badge:: chain) points at through the
// EntrantsSystem driver tables (Ent_Get_Driver_Thunk 0x1423dbcf0).
namespace driver {
constexpr uintptr_t player_name = 0x60, participants = 0x88;
constexpr uintptr_t participant_physics = 0x18, participant_flags = 0x32, physics_car = 0x58;
constexpr uint8_t human_flag = 0x80;
constexpr uintptr_t entrant_ref = 0x50, sets = 0x268, set_stride = 0x10, table_vec = 0x8, slot_record = 0x8,
                    record_name = 0xc8;
constexpr unsigned max_domain = 2;
constexpr size_t max_participants = 64, max_name = 128;
}

namespace track {
constexpr uintptr_t world_to_a = 0xd0, a_to_b = 0xe8, b_to_name = 0xa0;
constexpr uintptr_t sector_end = 0x0, sector_length = 0x58, sector_count = 0x5c;
constexpr size_t sector_block = 0x60;
}

}

// The generated game headers declare a struct field only once its KB row has an independent agree. LIVE_HAS(T, f) is
// true when the header on this build declares T::f, and LIVE_TIE(T, f, off) fails the build when it declares it at a
// different offset than the constant the reader uses. A field the headers do not declare yet ties nothing and keeps its
// group off (live_rows.hpp).
#define LIVE_HAS(T, f)                                                                                                 \
    ([]<class X>() consteval { return requires { offsetof(X, f); }; }.template operator()<swofty::game::T>())
#define LIVE_SIZE(T, n)                                                                                                \
    static_assert(                                                                                                     \
        []<class X>() consteval {                                                                                      \
            if constexpr (requires { sizeof(X); })                                                                     \
                return sizeof(X) <= (n);                                                                               \
            else                                                                                                       \
                return true;                                                                                           \
        }.template operator()<swofty::game::T>(),                                                                      \
        #T " is not the size the live layout steps by")
#define LIVE_TIE(T, f, off)                                                                                            \
    static_assert(                                                                                                     \
        []<class X>() consteval {                                                                                      \
            if constexpr (requires { offsetof(X, f); })                                                                \
                return offsetof(X, f) == (off);                                                                        \
            else                                                                                                       \
                return true;                                                                                           \
        }.template operator()<swofty::game::T>(),                                                                      \
        #T "." #f " is not where the live layout reads it")

namespace swofty_core::live::layout {

LIVE_TIE(CarLiveInputs, throttle, car::throttle);
LIVE_TIE(CarLiveInputs, clutch, car::clutch);
LIVE_TIE(CarLiveInputs, brake, car::brake);
LIVE_TIE(CarLiveInputs, input_flags, car::input_flags);
LIVE_TIE(CarChassisLive, g_lat, chassis::g_lat);
LIVE_TIE(CarChassisLive, g_long, chassis::g_long);
LIVE_TIE(CarChassisLive, body, chassis::body);
LIVE_TIE(CarBodyActor, state, chassis::body_state);
LIVE_TIE(CarBodyState, position, chassis::state_position);
LIVE_TIE(CarBodyState, velocity, chassis::state_velocity);
LIVE_TIE(CarSteeringLive, steering_angle, steering::angle);
LIVE_TIE(CarGearBox, gear_index, gearbox::gear_index);
LIVE_TIE(CarErs, max_deploy_power, ers::max_deploy);
LIVE_TIE(CarErs, energy_store_capacity, ers::capacity);
LIVE_TIE(CarErs, harvest_lap_mguk, ers::harvest_lap);
LIVE_TIE(CarErs, deploy_lap, ers::deploy_lap);
LIVE_TIE(CarErs, energy, ers::energy);
LIVE_TIE(CarErs, lap_harvest_limit, ers::lap_limit);
LIVE_TIE(CarErs, harvest_power_mguk, ers::harvest_k);
LIVE_TIE(CarErs, harvest_power_mguh, ers::harvest_h);
LIVE_TIE(CarErs, deploy_power, ers::deploy);
LIVE_TIE(CarErs, ers_mode, ers::mode);
LIVE_TIE(CarF1Aero, drs_flags, aero::drs_request);
LIVE_TIE(NeRobinHoodMapU64, slots, registry::slots);
LIVE_TIE(NeRobinHoodMapU64, meta, registry::info);
LIVE_TIE(GameManagerLive, systems_map, registry::from_manager);
LIVE_TIE(GameManagerLive, progress_system, progress::from_manager);
LIVE_TIE(GameVehicleProgressSystem, entries_begin, progress::vec_begin);
LIVE_TIE(GameVehicleProgressSystem, entries_end, progress::vec_end);
LIVE_TIE(VehicleProgressEntry, lap_a, progress::lap_a);
LIVE_TIE(VehicleProgressEntry, lap_b, progress::lap_b);
LIVE_TIE(VehicleProgressEntry, progress_lap_a, progress::plap_a);
LIVE_TIE(VehicleProgressEntry, progress_lap_b, progress::plap_b);
LIVE_TIE(VehicleProgressEntry, progress_lap_delta, progress::plap_delta);
LIVE_TIE(VehicleProgressEntry, total_a, progress::total_a);
LIVE_TIE(VehicleProgressEntry, total_b, progress::total_b);
LIVE_TIE(SessionSystem, entries_begin, results::vec_begin);
LIVE_TIE(SessionSystem, entries_end, results::vec_end);
LIVE_TIE(SessionVehicleEntry, vehicle_id, results::vehicle);
LIVE_TIE(SessionVehicleEntry, status, results::status);
LIVE_TIE(SessionVehicleEntry, time_penalty_a, results::penalty_a);
LIVE_TIE(SessionVehicleEntry, time_penalty_b, results::penalty_b);
LIVE_TIE(SessionVehicleEntry, position_a, results::pos_a);
LIVE_TIE(SessionVehicleEntry, position_b, results::pos_b);
LIVE_TIE(RelativeGapHelper, entries_begin, timing::vec_begin);
LIVE_TIE(RelativeGapEntry, leader_gap_ms, timing::leader_ms);
LIVE_TIE(RelativeGapEntry, leader_gap_valid, timing::leader_valid);
LIVE_TIE(RelativeGapEntry, interval_ms, timing::ahead_ms);
LIVE_TIE(RelativeGapEntry, interval_valid, timing::ahead_valid);
LIVE_TIE(RelativeGapEntry, vehicle_id, timing::vehicle);
LIVE_TIE(PitLaneSystem, car_slots, pit::records);
LIVE_TIE(PitLaneCarState, state, pit::state);
LIVE_TIE(PitLaneCarState, pit_stops, pit::stops);
LIVE_TIE(PenaltyLog, records_begin, penalty_log::vec_begin);
LIVE_TIE(PenaltyRecord, vehicle_id, penalty_log::vehicle);
LIVE_TIE(PenaltyRecord, penalty_type, penalty_log::kind);
LIVE_TIE(PenaltyRecord, infringement, penalty_log::infringement);
LIVE_TIE(VehicleHelperSystem, drive_through, misconduct::drive_through);
LIVE_TIE(VehicleHelperSystem, stop_go, misconduct::stop_go);
LIVE_TIE(DriveThroughObject, records_begin, misconduct::vec_begin);
LIVE_TIE(StopGoObject, records_begin, misconduct::vec_begin);
LIVE_TIE(SafetyCarLogicSystem, deployment, safety_car::state);
LIVE_TIE(MarshalZoneMonitor, zones_begin, marshal::vec_begin);
LIVE_TIE(MarshalZoneSlot, zone, marshal::zone);
LIVE_TIE(MarshalZone, definition, marshal::zone_def);
LIVE_TIE(MarshalZone, flag, marshal::zone_flag);
LIVE_TIE(MarshalZoneDef, start_fraction, marshal::def_start);
LIVE_TIE(RedFlagSystem, state, red_flag::state);
LIVE_TIE(ControllerSystem, sets, controllers::sets);
LIVE_TIE(ControllerSystem, vehicle_manager, controllers::count_vec);
LIVE_TIE(ControllerSystem, sets_built, controllers::built);
LIVE_TIE(ControllerSet, human_controller, controllers::set_human);
LIVE_TIE(ControllerSet, vehicle, controllers::set_vehicle);
LIVE_TIE(VehicleTyreComponent, compound_class, tyres::version);
LIVE_TIE(VehicleTyreComponent, slots, tyres::list);
LIVE_TIE(VehicleTyreComponent, slot_count, tyres::count);
LIVE_TIE(VehicleTyreComponent, built_for, tyres::built_for);
LIVE_TIE(VehicleTyreComponent, built, tyres::built);
LIVE_TIE(VehicleTyreComponent, set_begin, tyres::set_begin);
LIVE_TIE(VehicleTyreComponent, set_end, tyres::set_end);
LIVE_TIE(SpectatorDirector, ui_active, spectate::ui);
LIVE_TIE(SpectatorDirector, active, spectate::active);
LIVE_TIE(SpectatorDirector, current_driver, spectate::vehicle);

LIVE_TIE(PitLaneSystem, lane, pit::lane_object);
LIVE_TIE(PitLaneTrack, vehicle_slots, pit::lane_flags);
LIVE_TIE(PitLaneVehicleSlot, lane, 0x0);
LIVE_TIE(PitLaneVehicle, in_lane, pit::in_lane);
LIVE_SIZE(PitLaneVehicleSlot, pit::lane_flag_stride);
LIVE_TIE(SpectatorDirectorGap, gap_mode, spectate::gap);
LIVE_TIE(TyreSystem, vehicles_begin, tyres::per_vehicle);
LIVE_TIE(TyreSystem, vehicles_end, tyres::per_vehicle + 8);
LIVE_TIE(TyreSlot, option, 0x0);
LIVE_TIE(TyreSlot, physical, tyres::desc_physical);
LIVE_TIE(TyreSlot, visual, tyres::desc_visual);
LIVE_SIZE(TyreSlot, tyres::desc_stride);
LIVE_TIE(VisualCompound, stat_id, tyres::visual_stat_id);
LIVE_TIE(VisualCompound, audio_compound, tyres::visual_compound);
LIVE_TIE(PhysicalCompound, stat_id, tyres::physical_stat_id);
LIVE_TIE(GameModeSystem, mode, mode::from_context);
LIVE_TIE(GameModeSystem, mode, badge::mode);
LIVE_TIE(GameMode, name, mode::label);
LIVE_TIE(GameMode, entrants_manager, badge::game);
LIVE_TIE(EntrantsManager, session_begin, badge::entrants);
LIVE_TIE(EntrantsManager, session_end, badge::entrants + 8);
LIVE_TIE(SessionEntrantView, team_stat_id, badge::entrant_team);
LIVE_TIE(SessionEntrantView, vehicle_id, badge::entrant_vid);
LIVE_TIE(SessionEntrantView, badge_stat_id, badge::entrant_badge);
LIVE_SIZE(SessionEntrantView, badge::entrant_stride);
LIVE_TIE(EntrantsSystem, team_sets, badge::tables - 8);
LIVE_TIE(TeamSetRef, slots, 0x8);
LIVE_SIZE(TeamSetRef, 16);
LIVE_TIE(TeamSlots, begin, badge::table_vec);
LIVE_TIE(TeamSlots, end, badge::table_vec + 8);
LIVE_TIE(TeamSlot, record, badge::slot_record);
LIVE_SIZE(TeamSlot, 16);
LIVE_TIE(TeamRecord, timing_tower_texture_path, badge::record_key);
LIVE_TIE(TeamRecord, show_custom_badge_in_ui, badge::record_render);
LIVE_TIE(VehicleHelperSystem, player_name, driver::player_name);
LIVE_TIE(VehicleHelperSystem, participants_begin, driver::participants);
LIVE_TIE(VehicleHelperSystem, participants_end, driver::participants + 8);
LIVE_TIE(VehicleHelperParticipant, physics, driver::participant_physics);
LIVE_TIE(VehicleHelperParticipant, driver_kind, driver::participant_flags);
LIVE_TIE(CarPhysicsRecord, car, driver::physics_car);
LIVE_TIE(SessionEntrantView, driver_ref, driver::entrant_ref);
LIVE_TIE(EntrantsSystem, driver_sets, driver::sets - 8);
LIVE_TIE(DriverSetRef, slots, 0x8);
LIVE_SIZE(DriverSetRef, driver::set_stride);
LIVE_TIE(DriverSlots, begin, driver::table_vec);
LIVE_TIE(DriverSlots, end, driver::table_vec + 8);
LIVE_TIE(DriverSlot, record, driver::slot_record);
LIVE_SIZE(DriverSlot, 16);
LIVE_TIE(DriverRecord, display_name, driver::record_name);
LIVE_TIE(RacingVehicles, owner, track::world_to_a);
LIVE_TIE(RacingOwner, track, track::a_to_b);
LIVE_TIE(RacingTrack, name, track::b_to_name);

}
