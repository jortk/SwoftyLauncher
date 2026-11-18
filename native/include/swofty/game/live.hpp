#pragma once
#include <swofty/game/base.hpp>
#include <swofty/game/car.hpp>

namespace swofty::game {

struct CarBodyActor {
    uint8_t _pad_0x0[0x18];
    CarBodyState *state; // L2 experimental
};
static_assert(offsetof(CarBodyActor, state) == 0x18);

struct CarBodyState {
    uint8_t _pad_0x0[0x1c0];
    float position[3]; // L2 experimental
    uint8_t _pad_0x1cc[0x54];
    float velocity[3]; // L2 experimental
};
static_assert(offsetof(CarBodyState, position) == 0x1c0);
static_assert(offsetof(CarBodyState, velocity) == 0x220);

struct CarChassisLive {
    uint8_t _pad_0x0[0x320];
    float g_lat; // L2 experimental
    float g_long; // L2 experimental
    float g_vert; // L2 experimental
    uint8_t _pad_0x32c[0x14];
    CarBodyActor *body; // L2 experimental
};
static_assert(offsetof(CarChassisLive, g_lat) == 0x320);
static_assert(offsetof(CarChassisLive, g_long) == 0x324);
static_assert(offsetof(CarChassisLive, g_vert) == 0x328);
static_assert(offsetof(CarChassisLive, body) == 0x340);

struct CarLiveInputs {
    uint8_t _pad_0x0[0x188];
    float throttle; // L2 experimental
    float clutch; // L2 experimental
    float brake; // L2 experimental
    uint8_t _pad_0x194[0x90];
    uint8_t input_flags; // L2 experimental
};
static_assert(offsetof(CarLiveInputs, throttle) == 0x188);
static_assert(offsetof(CarLiveInputs, clutch) == 0x18c);
static_assert(offsetof(CarLiveInputs, brake) == 0x190);
static_assert(offsetof(CarLiveInputs, input_flags) == 0x224);

struct CarSteeringLive {
    uint8_t _pad_0x0[0xe8];
    CarObfFloat steering_angle; // L2 experimental
};
static_assert(offsetof(CarSteeringLive, steering_angle) == 0xe8);

struct ControllerSystem {
    uint8_t _pad_0x0[0x30];
    ControllerSet **sets; // L2 experimental
    uint8_t _pad_0x38[0x58];
    void *vehicle_manager; // L2 experimental
    uint8_t _pad_0x98[0x2cc];
    uint8_t active; // L2 experimental
    uint8_t sets_built; // L2 experimental
};
static_assert(offsetof(ControllerSystem, sets) == 0x30);
static_assert(offsetof(ControllerSystem, vehicle_manager) == 0x90);
static_assert(offsetof(ControllerSystem, active) == 0x364);
static_assert(offsetof(ControllerSystem, sets_built) == 0x365);

struct DriveThroughObject {
    uint8_t _pad_0x0[0x68];
    uint8_t *records_begin; // L2 experimental
    uint8_t *records_end; // L2 experimental
};
static_assert(offsetof(DriveThroughObject, records_begin) == 0x68);
static_assert(offsetof(DriveThroughObject, records_end) == 0x70);

struct DriverSetRef {
    uint8_t _pad_0x0[0x8];
    DriverSlots *slots; // L2 experimental
};
static_assert(offsetof(DriverSetRef, slots) == 0x8);

struct DriverSlot {
    uint8_t _pad_0x0[0x8];
    DriverRecord *record; // L2 experimental
};
static_assert(offsetof(DriverSlot, record) == 0x8);

struct DriverSlots {
    uint8_t _pad_0x0[0x8];
    DriverSlot *begin; // L2 experimental
    DriverSlot *end; // L2 experimental
};
static_assert(offsetof(DriverSlots, begin) == 0x8);
static_assert(offsetof(DriverSlots, end) == 0x10);

struct TeamSetRef {
    uint8_t _pad_0x0[0x8];
    TeamSlots *slots; // L2 experimental
};
static_assert(offsetof(TeamSetRef, slots) == 0x8);

struct EntrantsSystem {
    uint8_t _pad_0x0[0x260];
    DriverSetRef driver_sets[3]; // L2 experimental
    uint8_t _pad_0x290[0x30];
    TeamSetRef team_sets[3]; // L2 experimental
};
static_assert(offsetof(EntrantsSystem, driver_sets) == 0x260);
static_assert(offsetof(EntrantsSystem, team_sets) == 0x2c0);

struct GameManagerLive {
    uint8_t _pad_0x0[0x30];
    NeRobinHoodMapU64 *systems_map; // L2 experimental
    uint8_t _pad_0x38[0x298];
    GameVehicleProgressSystem *progress_system; // L2 experimental
};
static_assert(offsetof(GameManagerLive, systems_map) == 0x30);
static_assert(offsetof(GameManagerLive, progress_system) == 0x2d0);

struct GameMode {
    uint8_t _pad_0x0[0x260];
    EntrantsManager *entrants_manager; // L2 experimental
    uint8_t _pad_0x268[0x10];
    char *name; // L2 experimental
    uint8_t _pad_0x280[0x20];
};
static_assert(offsetof(GameMode, entrants_manager) == 0x260);
static_assert(offsetof(GameMode, name) == 0x278);
static_assert(sizeof(GameMode) == 0x2a0);

struct GameModeSystem {
    uint8_t _pad_0x0[0x88];
    GameMode *mode; // L2 experimental
};
static_assert(offsetof(GameModeSystem, mode) == 0x88);

struct GameVehicleProgressSystem {
    uint8_t _pad_0x0[0x38];
    void **entries_begin; // L2 experimental
    void **entries_end; // L2 experimental
};
static_assert(offsetof(GameVehicleProgressSystem, entries_begin) == 0x38);
static_assert(offsetof(GameVehicleProgressSystem, entries_end) == 0x40);

struct MarshalZone {
    MarshalZoneDef *definition; // L2 experimental
    uint8_t _pad_0x8[0xc];
    uint8_t flag; // L2 experimental
};
static_assert(offsetof(MarshalZone, definition) == 0x0);
static_assert(offsetof(MarshalZone, flag) == 0x14);

struct MarshalZoneDef {
    uint8_t _pad_0x0[0x10];
    float start_fraction; // L2 experimental
};
static_assert(offsetof(MarshalZoneDef, start_fraction) == 0x10);

struct MarshalZoneMonitor {
    uint8_t _pad_0x0[0x88];
    MarshalZoneSlot *zones_begin; // L2 experimental
    MarshalZoneSlot *zones_end; // L2 experimental
};
static_assert(offsetof(MarshalZoneMonitor, zones_begin) == 0x88);
static_assert(offsetof(MarshalZoneMonitor, zones_end) == 0x90);

struct MarshalZoneSlot {
    uint8_t _pad_0x0[0x8];
    MarshalZone *zone; // L2 experimental
};
static_assert(offsetof(MarshalZoneSlot, zone) == 0x8);

struct NeRobinHoodMapU64 {
    uint8_t _pad_0x0[0x10];
    uint64_t hash_seed; // L2 experimental
    uint8_t *slots; // L2 experimental
    uint8_t *meta; // L2 experimental
    uint64_t count; // L2 experimental
    uint64_t mask; // L2 experimental
    uint64_t max_count; // L2 experimental
    uint32_t step; // L2 experimental
    uint32_t shift; // L2 experimental
};
static_assert(offsetof(NeRobinHoodMapU64, hash_seed) == 0x10);
static_assert(offsetof(NeRobinHoodMapU64, slots) == 0x18);
static_assert(offsetof(NeRobinHoodMapU64, meta) == 0x20);
static_assert(offsetof(NeRobinHoodMapU64, count) == 0x28);
static_assert(offsetof(NeRobinHoodMapU64, mask) == 0x30);
static_assert(offsetof(NeRobinHoodMapU64, max_count) == 0x38);
static_assert(offsetof(NeRobinHoodMapU64, step) == 0x40);
static_assert(offsetof(NeRobinHoodMapU64, shift) == 0x44);

struct PenaltyLog {
    uint8_t _pad_0x0[0x48];
    PenaltyRecord *records_begin; // L2 experimental
    PenaltyRecord *records_end; // L2 experimental
};
static_assert(offsetof(PenaltyLog, records_begin) == 0x48);
static_assert(offsetof(PenaltyLog, records_end) == 0x50);

struct PenaltyRecord {
    uint8_t _pad_0x0[0x8];
    uint8_t vehicle_id; // L2 experimental
    uint8_t penalty_type; // L2 experimental
    uint8_t infringement; // L2 experimental
};
static_assert(offsetof(PenaltyRecord, vehicle_id) == 0x8);
static_assert(offsetof(PenaltyRecord, penalty_type) == 0x9);
static_assert(offsetof(PenaltyRecord, infringement) == 0xa);

struct PhysicalCompound {
    uint8_t stat_id; // L2 experimental
};
static_assert(offsetof(PhysicalCompound, stat_id) == 0x0);

struct PitLaneCarSlot {
    PitLaneCarState *state; // L2 experimental
};
static_assert(offsetof(PitLaneCarSlot, state) == 0x0);

struct PitLaneCarState {
    uint8_t _pad_0x0[0x4];
    uint32_t state; // L2 experimental
    uint8_t _pad_0x8[0xc];
    uint32_t pit_stops; // L2 experimental
};
static_assert(offsetof(PitLaneCarState, state) == 0x4);
static_assert(offsetof(PitLaneCarState, pit_stops) == 0x14);

struct PitLaneSystem {
    uint8_t _pad_0x0[0x48];
    PitLaneTrack *lane; // L2 experimental
    uint8_t _pad_0x50[0x18];
    PitLaneCarSlot car_slots[27]; // L2 experimental
};
static_assert(offsetof(PitLaneSystem, lane) == 0x48);
static_assert(offsetof(PitLaneSystem, car_slots) == 0x68);

struct PitLaneVehicleSlot {
    PitLaneVehicle *lane; // L2 experimental
};
static_assert(offsetof(PitLaneVehicleSlot, lane) == 0x0);

struct PitLaneTrack {
    uint8_t _pad_0x0[0x938];
    PitLaneVehicleSlot vehicle_slots[27]; // L2 experimental
};
static_assert(offsetof(PitLaneTrack, vehicle_slots) == 0x938);

struct PitLaneVehicle {
    uint8_t _pad_0x0[0x103];
    uint8_t in_lane; // L2 experimental
};
static_assert(offsetof(PitLaneVehicle, in_lane) == 0x103);

struct RacingOwner {
    uint8_t _pad_0x0[0xe8];
    RacingTrack *track; // L2 experimental
};
static_assert(offsetof(RacingOwner, track) == 0xe8);

struct RacingTrack {
    uint8_t _pad_0x0[0xa0];
    char *name; // L2 experimental
};
static_assert(offsetof(RacingTrack, name) == 0xa0);

struct RacingVehicles {
    uint8_t _pad_0x0[0xd0];
    RacingOwner *owner; // L2 experimental
};
static_assert(offsetof(RacingVehicles, owner) == 0xd0);

struct RelativeGapEntry {
    uint8_t _pad_0x0[0x58];
    int64_t leader_gap_ms; // L2 experimental
    uint8_t _pad_0x60[0x8];
    uint8_t leader_gap_valid; // L2 experimental
    uint8_t _pad_0x69[0x7];
    int64_t interval_ms; // L2 experimental
    uint8_t _pad_0x78[0x8];
    uint8_t interval_valid; // L2 experimental
    uint8_t _pad_0x81[0xf];
    uint8_t vehicle_id; // L2 experimental
};
static_assert(offsetof(RelativeGapEntry, leader_gap_ms) == 0x58);
static_assert(offsetof(RelativeGapEntry, leader_gap_valid) == 0x68);
static_assert(offsetof(RelativeGapEntry, interval_ms) == 0x70);
static_assert(offsetof(RelativeGapEntry, interval_valid) == 0x80);
static_assert(offsetof(RelativeGapEntry, vehicle_id) == 0x90);

struct RelativeGapHelper {
    uint8_t _pad_0x0[0x30];
    RelativeGapEntry *entries_begin; // L2 experimental
    RelativeGapEntry *entries_end; // L2 experimental
    uint8_t _pad_0x40[0x90];
    PitLaneSystem *pit_lane; // L2 experimental
};
static_assert(offsetof(RelativeGapHelper, entries_begin) == 0x30);
static_assert(offsetof(RelativeGapHelper, entries_end) == 0x38);
static_assert(offsetof(RelativeGapHelper, pit_lane) == 0xd0);

struct SafetyCarLogicSystem {
    uint8_t _pad_0x0[0x108];
    uint8_t deployment; // L2 experimental
};
static_assert(offsetof(SafetyCarLogicSystem, deployment) == 0x108);

struct SessionEntrantView {
    uint8_t _pad_0x0[0x58];
    uint32_t team_stat_id; // L2 experimental
    uint8_t _pad_0x5c[0xc];
    uint32_t badge_stat_id; // L2 experimental
    uint8_t _pad_0x6c[0x16];
    uint8_t vehicle_id; // L2 experimental
};
static_assert(offsetof(SessionEntrantView, team_stat_id) == 0x58);
static_assert(offsetof(SessionEntrantView, badge_stat_id) == 0x68);
static_assert(offsetof(SessionEntrantView, vehicle_id) == 0x82);

struct SessionSystem {
    uint8_t _pad_0x0[0x148];
    SessionVehicleEntry **entries_begin; // L2 experimental
    SessionVehicleEntry **entries_end; // L2 experimental
};
static_assert(offsetof(SessionSystem, entries_begin) == 0x148);
static_assert(offsetof(SessionSystem, entries_end) == 0x150);

struct SessionVehicleEntry {
    uint8_t vehicle_id; // L2 experimental
    uint8_t _pad_0x1[0x17];
    uint8_t status; // L2 experimental
    uint8_t _pad_0x19[0x1f];
    uint64_t time_penalty_a; // L2 experimental
    uint64_t time_penalty_b; // L2 experimental
    uint32_t position_a; // L2 experimental
    uint8_t _pad_0x4c[0x4];
    uint32_t position_b; // L2 experimental
    uint8_t _pad_0x54[0x3c];
    uint32_t grid_position_a; // L2 experimental
    uint8_t _pad_0x94[0x4];
    uint32_t grid_position_b; // L2 experimental
};
static_assert(offsetof(SessionVehicleEntry, vehicle_id) == 0x0);
static_assert(offsetof(SessionVehicleEntry, status) == 0x18);
static_assert(offsetof(SessionVehicleEntry, time_penalty_a) == 0x38);
static_assert(offsetof(SessionVehicleEntry, time_penalty_b) == 0x40);
static_assert(offsetof(SessionVehicleEntry, position_a) == 0x48);
static_assert(offsetof(SessionVehicleEntry, position_b) == 0x50);
static_assert(offsetof(SessionVehicleEntry, grid_position_a) == 0x90);
static_assert(offsetof(SessionVehicleEntry, grid_position_b) == 0x98);

struct SpectatorDirectorGap {
    uint8_t _pad_0x0[0xe4];
    uint32_t gap_mode; // L2 experimental
};
static_assert(offsetof(SpectatorDirectorGap, gap_mode) == 0xe4);

struct StopGoObject {
    uint8_t _pad_0x0[0x68];
    uint8_t *records_begin; // L2 experimental
    uint8_t *records_end; // L2 experimental
};
static_assert(offsetof(StopGoObject, records_begin) == 0x68);
static_assert(offsetof(StopGoObject, records_end) == 0x70);

struct TeamSlot {
    uint8_t _pad_0x0[0x8];
    TeamRecord *record; // L2 experimental
};
static_assert(offsetof(TeamSlot, record) == 0x8);

struct TeamSlots {
    uint8_t _pad_0x0[0x8];
    TeamSlot *begin; // L2 experimental
    TeamSlot *end; // L2 experimental
};
static_assert(offsetof(TeamSlots, begin) == 0x8);
static_assert(offsetof(TeamSlots, end) == 0x10);

struct TyreSlot {
    uint32_t option; // L2 experimental
    uint8_t _pad_0x4[0x4];
    PhysicalCompound *physical; // L2 experimental
    VisualCompound *visual; // L2 experimental
};
static_assert(offsetof(TyreSlot, option) == 0x0);
static_assert(offsetof(TyreSlot, physical) == 0x8);
static_assert(offsetof(TyreSlot, visual) == 0x10);

struct TyreSystem {
    uint8_t _pad_0x0[0x110];
    VehicleTyreComponent **vehicles_begin; // L2 experimental
    VehicleTyreComponent **vehicles_end; // L2 experimental
};
static_assert(offsetof(TyreSystem, vehicles_begin) == 0x110);
static_assert(offsetof(TyreSystem, vehicles_end) == 0x118);

struct VehicleHelperSystem {
    uint8_t _pad_0x0[0x60];
    char *player_name; // L2 experimental
    uint8_t _pad_0x68[0x20];
    VehicleHelperParticipant **participants_begin; // L2 experimental
    VehicleHelperParticipant **participants_end; // L2 experimental
    uint8_t _pad_0x98[0x4d0];
    DriveThroughObject *drive_through; // L2 experimental
    uint8_t _pad_0x570[0x8];
    StopGoObject *stop_go; // L2 experimental
};
static_assert(offsetof(VehicleHelperSystem, player_name) == 0x60);
static_assert(offsetof(VehicleHelperSystem, participants_begin) == 0x88);
static_assert(offsetof(VehicleHelperSystem, participants_end) == 0x90);
static_assert(offsetof(VehicleHelperSystem, drive_through) == 0x568);
static_assert(offsetof(VehicleHelperSystem, stop_go) == 0x578);

struct VehicleProgressEntry {
    uint8_t _pad_0x0[0x38];
    uint16_t lap_a; // L2 experimental
    uint8_t _pad_0x3a[0x6];
    uint16_t lap_b; // L2 experimental
    uint8_t _pad_0x42[0x6];
    uint16_t progress_lap_a; // L2 experimental
    uint8_t _pad_0x4a[0x6];
    uint16_t progress_lap_b; // L2 experimental
    uint8_t _pad_0x52[0x6];
    int16_t progress_lap_delta; // L2 experimental
    uint8_t _pad_0x5a[0x3e];
    uint32_t total_a; // L2 experimental
    uint8_t _pad_0x9c[0x4];
    uint32_t total_b; // L2 experimental
};
static_assert(offsetof(VehicleProgressEntry, lap_a) == 0x38);
static_assert(offsetof(VehicleProgressEntry, lap_b) == 0x40);
static_assert(offsetof(VehicleProgressEntry, progress_lap_a) == 0x48);
static_assert(offsetof(VehicleProgressEntry, progress_lap_b) == 0x50);
static_assert(offsetof(VehicleProgressEntry, progress_lap_delta) == 0x58);
static_assert(offsetof(VehicleProgressEntry, total_a) == 0x98);
static_assert(offsetof(VehicleProgressEntry, total_b) == 0xa0);

struct VehicleTyreComponent {
    uint8_t _pad_0x0[0x9bc0];
    void *compound_class; // L2 experimental
    uint8_t _pad_0x9bc8[0x8];
    TyreSlot slots[5]; // L2 experimental
    uint64_t slot_count; // L2 experimental
    void *built_for; // L2 experimental
    void *built; // L2 experimental
    uint8_t _pad_0x9c60[0x30];
    uint8_t *set_begin; // L2 experimental
    uint8_t *set_end; // L2 experimental
};
static_assert(offsetof(VehicleTyreComponent, compound_class) == 0x9bc0);
static_assert(offsetof(VehicleTyreComponent, slots) == 0x9bd0);
static_assert(offsetof(VehicleTyreComponent, slot_count) == 0x9c48);
static_assert(offsetof(VehicleTyreComponent, built_for) == 0x9c50);
static_assert(offsetof(VehicleTyreComponent, built) == 0x9c58);
static_assert(offsetof(VehicleTyreComponent, set_begin) == 0x9c90);
static_assert(offsetof(VehicleTyreComponent, set_end) == 0x9c98);

struct VisualCompound {
    uint8_t stat_id; // L2 experimental
    uint8_t _pad_0x1[0x4f];
    int32_t audio_compound; // L2 experimental
};
static_assert(offsetof(VisualCompound, stat_id) == 0x0);
static_assert(offsetof(VisualCompound, audio_compound) == 0x50);

inline constexpr data<"g_Live_ControllerSystem", ControllerSystem *> g_Live_ControllerSystem{}; // L2 experimental
inline constexpr data<"g_Live_RacingVehicles", RacingVehicles *> g_Live_RacingVehicles{}; // L2 experimental
inline constexpr data<"g_Live_SectorCount", uint32_t> g_Live_SectorCount{}; // L2 experimental
inline constexpr data<"g_Live_SectorEnds", float[22]> g_Live_SectorEnds{}; // L2 experimental
inline constexpr data<"k_Live_EntrantsSystemKey", uint8_t> k_Live_EntrantsSystemKey{}; // L2 experimental
inline constexpr data<"k_Live_GameModeSystemKey", uint8_t> k_Live_GameModeSystemKey{}; // L2 experimental
inline constexpr data<"k_Live_GameVehicleProgressSystemKey", uint8_t> k_Live_GameVehicleProgressSystemKey{}; // L2 experimental
inline constexpr data<"k_Live_MarshalZoneMonitorKey", uint8_t> k_Live_MarshalZoneMonitorKey{}; // L2 experimental
inline constexpr data<"k_Live_PenaltyLogKey", uint8_t> k_Live_PenaltyLogKey{}; // L2 experimental
inline constexpr data<"k_Live_PitLaneSystemKey", uint8_t> k_Live_PitLaneSystemKey{}; // L2 experimental
inline constexpr data<"k_Live_RedFlagSystemKey", uint8_t> k_Live_RedFlagSystemKey{}; // L2 experimental
inline constexpr data<"k_Live_RelativeGapHelperKey", uint8_t> k_Live_RelativeGapHelperKey{}; // L2 experimental
inline constexpr data<"k_Live_SafetyCarLogicSystemKey", uint8_t> k_Live_SafetyCarLogicSystemKey{}; // L2 experimental
inline constexpr data<"k_Live_SessionSystemKey", uint8_t> k_Live_SessionSystemKey{}; // L2 experimental
inline constexpr data<"k_Live_SpectatorDirectorKey", uint8_t> k_Live_SpectatorDirectorKey{}; // L2 experimental
inline constexpr data<"k_Live_TyreSystemKey", uint8_t> k_Live_TyreSystemKey{}; // L2 experimental
inline constexpr data<"k_Live_VehicleHelperSystemKey", uint8_t> k_Live_VehicleHelperSystemKey{}; // L2 experimental

}
