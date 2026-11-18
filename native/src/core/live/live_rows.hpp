#pragma once
#include "../game_manager_layout.hpp"
#include "live_layout.hpp"
#include "live_mem.hpp"

namespace swofty_core::live {

// A group reads on its own when every site it needs resolves (bindgen exports a site only after its KB row has an
// independent agree), every field has a generated declaration or an explicit proof for this exact build, and its
// reads are all backed by KB rows. `mapped` is false for a group that still reads a chain the KB has no row for; that
// group stays off until SWOFTY_LIVE_FORCE=1 or until the row lands and the entry is flipped.
struct Row {
    Group group;
    bool fields;
    bool mapped;
    const char *needs;
};

inline constexpr bool F_CAR_SYSTEMS = LIVE_HAS(CarLiveInputs, throttle) && LIVE_HAS(CarLiveInputs, clutch) &&
                                      LIVE_HAS(CarLiveInputs, brake) && LIVE_HAS(CarLiveInputs, input_flags) &&
                                      LIVE_HAS(CarChassisLive, g_lat) && LIVE_HAS(CarChassisLive, g_long) &&
                                      LIVE_HAS(CarChassisLive, body) && LIVE_HAS(CarBodyActor, state) &&
                                      LIVE_HAS(CarBodyState, velocity) && LIVE_HAS(CarSteeringLive, steering_angle) &&
                                      LIVE_HAS(CarGearBox, gear_index) && LIVE_HAS(CarEngine, rotation_rate) &&
                                      LIVE_HAS(CarEngine, limiter_rotation_rate);
inline constexpr bool F_ERS = LIVE_HAS(CarErs, max_deploy_power) && LIVE_HAS(CarErs, energy_store_capacity) &&
                              LIVE_HAS(CarErs, harvest_lap_mguk) && LIVE_HAS(CarErs, deploy_lap) &&
                              LIVE_HAS(CarErs, energy) && LIVE_HAS(CarErs, lap_harvest_limit) &&
                              LIVE_HAS(CarErs, harvest_power_mguk) && LIVE_HAS(CarErs, harvest_power_mguh) &&
                              LIVE_HAS(CarErs, deploy_power) && LIVE_HAS(CarErs, ers_mode);
inline constexpr bool F_WORLD = LIVE_HAS(CarBodyState, position);
inline constexpr bool F_AERO = LIVE_HAS(CarF1Aero, drs_flags);
inline constexpr bool F_PROGRESS = LIVE_HAS(GameManagerLive, progress_system) &&
                                   LIVE_HAS(GameVehicleProgressSystem, entries_begin) &&
                                   LIVE_HAS(GameVehicleProgressSystem, entries_end) && LIVE_HAS(VehicleProgressEntry, lap_a) &&
                                   LIVE_HAS(VehicleProgressEntry, lap_b) && LIVE_HAS(VehicleProgressEntry, progress_lap_a) &&
                                   LIVE_HAS(VehicleProgressEntry, progress_lap_b) &&
                                   LIVE_HAS(VehicleProgressEntry, progress_lap_delta) &&
                                   LIVE_HAS(VehicleProgressEntry, total_a) && LIVE_HAS(VehicleProgressEntry, total_b);
inline constexpr bool F_REGISTRY = LIVE_HAS(GameManagerLive, systems_map) && LIVE_HAS(NeRobinHoodMapU64, slots) &&
                                   LIVE_HAS(NeRobinHoodMapU64, meta);
inline constexpr bool F_CLASS = LIVE_HAS(SessionSystem, entries_begin) && LIVE_HAS(SessionSystem, entries_end) &&
                                LIVE_HAS(SessionVehicleEntry, status) && LIVE_HAS(SessionVehicleEntry, time_penalty_a) &&
                                LIVE_HAS(SessionVehicleEntry, time_penalty_b) && LIVE_HAS(SessionVehicleEntry, position_a) &&
                                LIVE_HAS(SessionVehicleEntry, position_b);
inline constexpr bool F_TIMING = LIVE_HAS(RelativeGapHelper, entries_begin) && LIVE_HAS(RelativeGapEntry, leader_gap_ms) &&
                                 LIVE_HAS(RelativeGapEntry, leader_gap_valid) && LIVE_HAS(RelativeGapEntry, interval_ms) &&
                                 LIVE_HAS(RelativeGapEntry, interval_valid) && LIVE_HAS(RelativeGapEntry, vehicle_id) &&
                                 LIVE_HAS(SafetyCarLogicSystem, deployment);
inline constexpr bool F_PIT = LIVE_HAS(PitLaneSystem, car_slots) && LIVE_HAS(PitLaneCarState, state);
inline constexpr bool F_PIT_LANE = LIVE_HAS(PitLaneCarState, pit_stops) && LIVE_HAS(PitLaneSystem, lane) &&
                                   LIVE_HAS(PitLaneTrack, vehicle_slots) && LIVE_HAS(PitLaneVehicleSlot, lane) &&
                                   LIVE_HAS(PitLaneVehicle, in_lane);
inline constexpr bool F_PENALTIES = LIVE_HAS(PenaltyLog, records_begin) && LIVE_HAS(PenaltyRecord, vehicle_id) &&
                                    LIVE_HAS(PenaltyRecord, penalty_type) && LIVE_HAS(PenaltyRecord, infringement) &&
                                    LIVE_HAS(VehicleHelperSystem, drive_through) && LIVE_HAS(VehicleHelperSystem, stop_go) &&
                                    LIVE_HAS(DriveThroughObject, records_begin) && LIVE_HAS(StopGoObject, records_begin);
inline constexpr bool F_TYRES = LIVE_HAS(VehicleTyreComponent, compound_class) && LIVE_HAS(VehicleTyreComponent, slots) &&
                                LIVE_HAS(VehicleTyreComponent, slot_count) && LIVE_HAS(VehicleTyreComponent, built_for) &&
                                LIVE_HAS(VehicleTyreComponent, built) && LIVE_HAS(VehicleTyreComponent, set_begin) &&
                                LIVE_HAS(VehicleTyreComponent, set_end) && LIVE_HAS(TyreSystem, vehicles_begin) &&
                                LIVE_HAS(TyreSystem, vehicles_end) && LIVE_HAS(TyreSlot, option) &&
                                LIVE_HAS(TyreSlot, physical) && LIVE_HAS(TyreSlot, visual) &&
                                LIVE_HAS(VisualCompound, stat_id) && LIVE_HAS(VisualCompound, audio_compound) &&
                                LIVE_HAS(PhysicalCompound, stat_id);
inline constexpr bool F_FLAGS = LIVE_HAS(MarshalZoneMonitor, zones_begin) && LIVE_HAS(MarshalZoneSlot, zone) &&
                                LIVE_HAS(MarshalZone, definition) && LIVE_HAS(MarshalZone, flag) &&
                                LIVE_HAS(MarshalZoneDef, start_fraction) && LIVE_HAS(RedFlagSystem, state);
inline constexpr bool F_CONTROL = LIVE_HAS(ControllerSystem, sets) && LIVE_HAS(ControllerSystem, vehicle_manager) &&
                                  LIVE_HAS(ControllerSystem, sets_built) && LIVE_HAS(ControllerSet, human_controller) &&
                                  LIVE_HAS(ControllerSet, vehicle);
inline constexpr bool F_SPECTATE = LIVE_HAS(SpectatorDirector, ui_active) && LIVE_HAS(SpectatorDirector, active) &&
                                   LIVE_HAS(SpectatorDirector, current_driver);
inline constexpr bool F_SPECTATE_GAP = LIVE_HAS(SpectatorDirectorGap, gap_mode);
inline constexpr bool F_MODE = LIVE_HAS(GameModeSystem, mode) && LIVE_HAS(GameMode, name);
inline constexpr bool F_BADGES = LIVE_HAS(GameModeSystem, mode) && LIVE_HAS(GameMode, entrants_manager) &&
                                 LIVE_HAS(EntrantsManager, session_begin) && LIVE_HAS(EntrantsManager, session_end) &&
                                 LIVE_HAS(SessionEntrantView, team_stat_id) && LIVE_HAS(SessionEntrantView, vehicle_id) &&
                                 LIVE_HAS(EntrantsSystem, team_sets) && LIVE_HAS(TeamSetRef, slots) &&
                                 LIVE_HAS(TeamSlots, begin) && LIVE_HAS(TeamSlots, end) && LIVE_HAS(TeamSlot, record) &&
                                 LIVE_HAS(TeamRecord, timing_tower_texture_path) &&
                                 LIVE_HAS(TeamRecord, show_custom_badge_in_ui);
inline constexpr bool F_DRIVER_NAMES =
    LIVE_HAS(GameModeSystem, mode) && LIVE_HAS(GameMode, entrants_manager) && LIVE_HAS(EntrantsManager, session_begin) &&
    LIVE_HAS(EntrantsManager, session_end) && LIVE_HAS(SessionEntrantView, vehicle_id) &&
    LIVE_HAS(SessionEntrantView, driver_ref) && LIVE_HAS(EntrantsSystem, driver_sets) && LIVE_HAS(DriverSetRef, slots) &&
    LIVE_HAS(DriverSlots, begin) && LIVE_HAS(DriverSlots, end) && LIVE_HAS(DriverSlot, record) &&
    LIVE_HAS(DriverRecord, display_name) && LIVE_HAS(VehicleHelperSystem, player_name) &&
    LIVE_HAS(VehicleHelperSystem, participants_begin) && LIVE_HAS(VehicleHelperSystem, participants_end) &&
    LIVE_HAS(VehicleHelperParticipant, physics) && LIVE_HAS(VehicleHelperParticipant, driver_kind) &&
    LIVE_HAS(CarPhysicsRecord, car);
inline constexpr bool F_BADGE_RENDER = F_BADGES && LIVE_HAS(SessionEntrantView, badge_stat_id);
inline constexpr bool F_TRACK_NAME =
    LIVE_HAS(RacingVehicles, owner) && LIVE_HAS(RacingOwner, track) && LIVE_HAS(RacingTrack, name);

inline constexpr Row ROWS[G_COUNT] = {
    {G_CAR_SYSTEMS, F_CAR_SYSTEMS, true,
     "CarLiveInputs, CarChassisLive, CarBodyActor/State, CarSteeringLive, CarEngine and CarGearBox fields"},
    {G_ERS, F_ERS, true, "CarErs live fields"},
    {G_WORLD, F_WORLD, true, "CarBodyState.position"},
    {G_AERO, F_AERO, true, "CarF1Aero.drs_flags"},
    {G_PROGRESS, F_PROGRESS && F_REGISTRY, true, "GameVehicleProgressSystem and VehicleProgressEntry fields"},
    {G_CLASS, F_CLASS && F_REGISTRY, true, "SessionSystem and SessionVehicleEntry fields"},
    {G_TIMING, F_TIMING && F_REGISTRY, true, "RelativeGapHelper, RelativeGapEntry, SafetyCarLogicSystem fields"},
    {G_PIT, F_PIT && F_REGISTRY, true, "PitLaneSystem, PitLaneCarSlot, PitLaneCarState.state"},
    {G_PENALTIES, F_PENALTIES && F_REGISTRY, true, "PenaltyLog, VehicleHelperSystem and the misconduct objects"},
    {G_TYRES, F_TYRES && F_REGISTRY, true, "VehicleTyreComponent, TyreSystem, TyreSlot, VisualCompound and PhysicalCompound fields"},
    {G_FLAGS, F_FLAGS && F_REGISTRY, true, "MarshalZoneMonitor, RedFlagSystem, the sector globals"},
    {G_CONTROL, F_CONTROL, true, "ControllerSystem and ControllerSet fields"},
    {G_SPECTATE, F_SPECTATE && F_REGISTRY, true, "SpectatorDirector fields"},
    {G_MODE, F_MODE && F_REGISTRY, true, "GameModeSystem.mode and GameMode.name"},
    {G_BADGES, F_BADGES && F_REGISTRY, true,
     "the game mode chain, EntrantsManager entrant vector, SessionEntrantView, EntrantsSystem team tables and TeamRecord badge fields"},
    {G_TRACK, true, true, "g_Game_TrackLength, g_Sess_TyreRulesSessionType"},
    {G_PIT_LANE, F_PIT_LANE, true, "PitLaneSystem.lane, PitLaneTrack.vehicle_slots, PitLaneVehicle.in_lane, PitLaneCarState.pit_stops"},
    {G_TRACK_NAME, F_TRACK_NAME, true, "RacingVehicles.owner, RacingOwner.track and RacingTrack.name"},
    {G_SPECTATE_GAP, F_SPECTATE_GAP && F_REGISTRY, true, "SpectatorDirectorGap.gap_mode"},
    {G_BUS_EVENTS, true, true, "g_Game_MessageBus and Ne_Subscribe_BusMessage; the message layouts are the agreed Live_Subscribe_* rows (no signature, not resolved)"},
    {G_DRIVER_NAMES, F_DRIVER_NAMES && F_REGISTRY, true,
     "the game mode entrant chain, SessionEntrantView.driver_ref, EntrantsSystem driver tables, DriverRecord.display_name, "
     "VehicleHelperSystem participants and player name, VehicleHelperParticipant and CarPhysicsRecord.car"},
    {G_BADGE_RENDER, F_BADGE_RENDER && F_REGISTRY, true, "the badges rows and SessionEntrantView.badge_stat_id"},
};

struct Needs {
    Group group;
    uintptr_t Sites::*sites[6];
};

inline constexpr Needs NEEDS[] = {
    {G_CAR_SYSTEMS, {&Sites::vehicle_table, &Sites::vehicle_count}},
    {G_ERS, {&Sites::vehicle_table, &Sites::vehicle_count}},
    {G_WORLD, {&Sites::vehicle_table, &Sites::vehicle_count}},
    {G_AERO, {&Sites::vehicle_table, &Sites::vehicle_count}},
    {G_PROGRESS, {&Sites::game_manager, &Sites::track_length}},
    {G_CLASS, {&Sites::game_manager, &Sites::key_results}},
    {G_TIMING, {&Sites::game_manager, &Sites::key_timing, &Sites::key_safety_car}},
    {G_PIT, {&Sites::game_manager, &Sites::key_pit}},
    {G_PIT_LANE, {&Sites::game_manager, &Sites::key_pit}},
    {G_PENALTIES, {&Sites::game_manager, &Sites::key_penalty_log, &Sites::key_session_component}},
    {G_TYRES, {&Sites::game_manager, &Sites::key_tyres}},
    {G_FLAGS,
     {&Sites::game_manager, &Sites::key_marshal, &Sites::key_red_flag, &Sites::sector_block, &Sites::sector_count,
      &Sites::track_length}},
    {G_CONTROL, {&Sites::controllers, &Sites::vt_controllers, &Sites::vt_human}},
    {G_SPECTATE, {&Sites::game_manager, &Sites::key_spectator}},
    {G_SPECTATE_GAP, {&Sites::game_manager, &Sites::key_spectator}},
    {G_MODE, {&Sites::game_manager, &Sites::key_game_mode}},
    {G_BADGES, {&Sites::game_manager, &Sites::key_game_mode, &Sites::key_team_tables}},
    {G_TRACK, {&Sites::track_length, &Sites::session_type}},
    {G_TRACK_NAME, {&Sites::track_world}},
    {G_BUS_EVENTS, {&Sites::bus_slot, &Sites::bus_subscribe}},
    {G_DRIVER_NAMES, {&Sites::game_manager, &Sites::key_game_mode, &Sites::key_team_tables, &Sites::key_session_component}},
    {G_BADGE_RENDER, {&Sites::game_manager, &Sites::key_game_mode, &Sites::key_team_tables}},
};

constexpr uint32_t bit(Group g)
{
    return 1u << g;
}

// A build other than the KB build reads a group only when the group is listed here for that build's PE timestamp: the
// layouts the group reads were compared against the KB build's (re/tools/layoutcmp.py, research/ea-layouts.md) and the
// sites it needs resolve in that build's bindings set. Displacements that differ are selected per build below.
struct BuildProof {
    uint32_t pe_timestamp;
    const char *name;
    uint32_t groups;
    uintptr_t progress_from_manager;
    uintptr_t ers_mode = layout::ers::mode;
    // Field layouts independently checked for this build when the baseline KB headers omit a field.
    uint32_t field_proofs = 0;
};

inline constexpr uint32_t ALL_GROUPS = (1u << G_COUNT) - 1;

inline constexpr BuildProof KB_BUILD = {1789118616u, "Steam", ALL_GROUPS, layout::progress::from_manager};

// Driver names read the EntrantsSystem driver tables and VehicleHelperSystem participants, which have not been compared
// against the EA build's layout yet; the rendered badge path reads a SessionEntrantView field that has not been either.
inline constexpr uint32_t EA_GROUPS = ALL_GROUPS & ~(bit(G_DRIVER_NAMES) | bit(G_BADGE_RENDER));

inline constexpr BuildProof EA_BUILD = {1789128892u, "EA", EA_GROUPS,
                                         runtime::game_manager::tail_field(layout::progress::from_manager, "ea")};

// Steam 26566308 (1.0.145.2472): existing Live State layouts checked against this executable.
// See re/research/live-steam-26566308.md for the build identity and instruction evidence.
// Resolving a binding alone does not prove the object layout behind it; the current ERS mode moved by four bytes.
inline constexpr uint32_t STEAM_26566308_GROUPS = ALL_GROUPS;
inline constexpr BuildProof STEAM_26566308_BUILD = {
    1794312636u, "Steam 26566308", STEAM_26566308_GROUPS, layout::progress::from_manager, 0x1a6c, bit(G_DRIVER_NAMES)};

inline constexpr const BuildProof *PROVEN_BUILDS[] = {&KB_BUILD, &EA_BUILD, &STEAM_26566308_BUILD};

inline const BuildProof *proof_for(uint32_t pe_timestamp)
{
    for (const BuildProof *b : PROVEN_BUILDS)
        if (b->pe_timestamp == pe_timestamp)
            return b;
    return nullptr;
}

enum class Why : uint8_t { on, site, row, build };

struct Verdict {
    Why why = Why::row;
    uintptr_t Sites::*missing = nullptr;
};

struct Decision {
    Gates gates;
    Verdict verdict[G_COUNT];
};

// A group is on when every site it needs resolved in this build's set, its rows are all KB-backed (live_rows above),
// and the build has the group's layout proven. `force` reads a group whose rows or build proof are missing.
inline Decision decide(const Sites &s, const BuildProof &b, bool force, const Row *rows_table = ROWS)
{
    Decision d;
    for (const Needs &n : NEEDS) {
        const Row &row = rows_table[n.group];
        Verdict &v = d.verdict[n.group];
        for (auto slot : n.sites)
            if (slot && !(s.*slot) && !v.missing)
                v.missing = slot;
        bool rows = (row.fields || (b.field_proofs & bit(n.group))) && row.mapped;
        bool proven = (b.groups & bit(n.group)) != 0;
        v.why = v.missing ? Why::site : !rows ? Why::row : !proven ? Why::build : Why::on;
        d.gates.on[n.group] = !v.missing && ((rows && proven) || force);
        if (d.gates.on[n.group])
            v.why = Why::on;
    }
    return d;
}

}
