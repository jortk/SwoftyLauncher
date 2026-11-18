#pragma once
#include <atomic>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <algorithm>
#include <functional>
#include <mutex>
#include <string>
#include <vector>

#include <swofty_api.h>

#include "ai_racecraft.hpp"

// The core's live car operations (SwoftyCoreCarOps, standard library 1.6): cars::live reads and writes a
// car's handling values while it is on track. A car is a slot of g_Sess_VehicleTable (its CarLive, whose
// vehicle_id is the slot); each value is a field of one of the systems the car caches by section name at
// CarLive+0x530..+0x5e0. Every offset below is a KB field (re/db/structs/{CarLive,CarSteering,CarGearBox,
// CarDifferential,CarTurbo,CarEngine,CarFuelTank,CarDamageFrontWing,CarDamageSuspension,CarObfFloat}.h, each
// exported by bindgen: L2 with an agree verify row, or L3), and "obf" fields are CarObfFloat pairs: the float's
// bits XOR the key 8 bytes on. The offsets are the KB build's; car_live.cpp hands over the table only on a build in
// CAR_BUILDS. Ops<Tag> keeps the state per tag so the host test runs the same code on fake cars.

namespace swofty_core::hooks::car_live {

constexpr uintptr_t CAR_VEHICLE_ID = 0xe8, CAR_IS_AI = 0x124, CAR_LOADER = 0x4a0, SYSTEM_CAR = 0x08;
constexpr uintptr_t FUEL_CAPACITY = 0x98, FUEL_MASS = 0x100;
constexpr uint32_t MAX_CARS = 32;

constexpr uintptr_t ENGINE = 0x530, STEERING = 0x560, GEARBOX = 0x538,
                    DIFFERENTIAL = 0x578, TURBO = 0x5a0, FUEL_TANK = 0x540,
                    WING_DAMAGE = 0x5c8, SUSPENSION_DAMAGE = 0x5d0, FUEL_ECU = 0x548;
constexpr uintptr_t FUEL_ECU_MODE = 0x128;
constexpr uintptr_t MODE_AVAIL_FUEL = 0x28, MODE_AVAIL_STRIDE = 0x20;
constexpr uint32_t MODE_AVAIL_CARS = 26;

// The builds the live car operations run on: the KB build, and the builds re/tools/carlivecmp.py checked against it
// (re/research/car-live-1-0-145.md): every field, system slot, VTF key, called and hooked function and mid-hook site
// above is the same there. mode_avail_rva is g_Game_EngineModeAvail, the one address that moves; size_of_image 0
// takes the KB build's stamp with any image above 0x6000000, as runtime::same_build() does.
struct CarBuild {
    uint32_t pe_timestamp, size_of_image;
    const char *name;
    uintptr_t mode_avail_rva;
};

inline constexpr CarBuild CAR_BUILDS[] = {
    {1789118616u, 0, "Steam KB", 0x606ec28},
    {1794312636u, 0x18434000u, "Steam 26566308", 0x6070c28},
    {1794315435u, 0x1615e000u, "EA 1.0.145", 0x61a7968},
};

constexpr const CarBuild *car_build_for(uint32_t pe_timestamp, uint32_t size_of_image)
{
    for (const CarBuild &b : CAR_BUILDS)
        if (b.pe_timestamp == pe_timestamp &&
            (b.size_of_image ? b.size_of_image == size_of_image : size_of_image > 0x6000000u))
            return &b;
    return nullptr;
}

// cell: a per-car AI driver config float, int or curve behind a game database cell (ai_racecraft.hpp); vslot is its
// index in ai_racecraft::DRIVER.
enum class Kind { plain, obf, virt, cell };

// What set re-runs after the write so the game's derived copy follows: the engine's derived values
// (Car_Engine_ComputeDerived_Thunk), the gearbox's ratio table (Car_Tuning_SetFinalDrive with 0x20|0x2000, which
// leaves the current gear alone; 0x200 would shift to neutral) or the chassis' mass (sqrt_mass, then the system's
// OnReset with 0x208, flag 0x8 being the slot's gate).
enum class Rederive { none, engine, gearbox, chassis, fuel };
// Where the field's storage is: in the system at Field::system, in the suspension's tyre record of an axle (found by
// its name), or in the suspension's axle object (the rear one has the lower local rest position).
enum class Via { system, tyre_front, tyre_rear, axle_front, axle_rear };
constexpr uintptr_t GEARBOX_USE_SETUP_RATIOS = 0x540;
constexpr uint32_t GEARBOX_REBUILD = 0x2020;
constexpr uint32_t CHASSIS_REBUILD = 0x208;
constexpr uintptr_t SUSPENSION = 0x580, SUSP_TYRE_TYPES = 0x38, SUSP_AXLE0 = 0x90, SUSP_AXLE1 = 0x98;
constexpr uintptr_t AXLE_LOCAL_REST_Z = 0x1598, TYRE_TYPE_NAME = 0x00;
constexpr uintptr_t CHASSIS_SQRT_MASS = 0x98;
constexpr uintptr_t CHASSIS_GROUP = 0x118, STRING_CAPACITY = 0x08, STRING_BUFFER = 0x10;
constexpr uint32_t NO_VIRTUAL = 0xff;
constexpr uint32_t VIRTUALS = 3;

struct Field {
    uint32_t id;
    const char *name;
    uintptr_t system, offset;
    Kind kind;
    float lo, hi;
    Rederive rederive = Rederive::none;
    Via via = Via::system;
    const char *needs = nullptr;
    float neutral = 0;
    bool relative = false;
    uint32_t vslot = NO_VIRTUAL;
};

inline constexpr float BIG = 1e9f;
constexpr uintptr_t F1_AERO = 0x570, BRAKE_ECU = 0x550, CHASSIS = 0x558;

#define AI_CELL(ID, GROUP, KEY, LO, HI)                                                                                \
    {ID, "ai." #GROUP "." #KEY, 0, 0, Kind::cell, LO, HI, Rederive::none, Via::system, "AiDriverConfig." #GROUP "_" #KEY, \
     0, false, ai_racecraft::slot(#GROUP "." #KEY)}

inline constexpr Field FIELDS[] = {
    {SWOFTY_CAR_STEERING_LOCK, "steering.max_steering_lock", STEERING, 0x28, Kind::obf, 0.01f, 1.5708f},
    {SWOFTY_CAR_CLUTCH_TORQUE, "gearbox.max_clutch_torque", GEARBOX, 0xe8, Kind::plain, 0, 100},
    {SWOFTY_CAR_DIFF_ON_THROTTLE, "differential.diff_on_throttle", DIFFERENTIAL, 0xf0, Kind::plain, 0, 1},
    {SWOFTY_CAR_DIFF_OFF_THROTTLE, "differential.diff_off_throttle", DIFFERENTIAL, 0xec, Kind::plain, 0, 1},
    {SWOFTY_CAR_TURBO_MAX_BOOST, "turbo.max_boost", TURBO, 0x2d8, Kind::plain, 0, 100},
    {SWOFTY_CAR_FUEL_MASS, "fuel_tank.fuel_mass", FUEL_TANK, 0x100, Kind::plain, 0, BIG},
    {SWOFTY_CAR_WING_SPEED_THRESHOLD, "f1_wing_damage.speed_threshold", WING_DAMAGE, 0x28, Kind::plain, 0, BIG},
    {SWOFTY_CAR_WING_DAMAGE_SCALE, "f1_wing_damage.damage_scale", WING_DAMAGE, 0x2c, Kind::plain, 0, BIG},
    {SWOFTY_CAR_WING_CHASSIS_DAMAGE_MULTIPLE, "f1_wing_damage.chassis_damage_multiple", WING_DAMAGE, 0x30, Kind::plain,
     0, BIG},
    {SWOFTY_CAR_WING_SIDE_DETACH_SCALE, "f1_wing_damage.side_detach_scale", WING_DAMAGE, 0x34, Kind::plain, 0, BIG},
    {SWOFTY_CAR_WING_FULL_DETACH_SCALE, "f1_wing_damage.full_detach_scale", WING_DAMAGE, 0x38, Kind::plain, 0, BIG},
    {SWOFTY_CAR_SUSP_SPEED_THRESHOLD, "f1_suspension_damage.speed_threshold", SUSPENSION_DAMAGE, 0x28, Kind::obf, 0,
     BIG},
    {SWOFTY_CAR_SUSP_FRONT_DAMAGE_SCALE, "f1_suspension_damage.front_damage_scale", SUSPENSION_DAMAGE, 0x38, Kind::obf,
     0, BIG},
    {SWOFTY_CAR_SUSP_REAR_DAMAGE_SCALE, "f1_suspension_damage.rear_damage_scale", SUSPENSION_DAMAGE, 0x48, Kind::obf, 0,
     BIG},
    {SWOFTY_CAR_SUSP_WHEEL_DETACH_SCALE, "f1_suspension_damage.wheel_detach_scale", SUSPENSION_DAMAGE, 0x58, Kind::obf,
     0, BIG},
    {SWOFTY_CAR_SUSP_PARTS_DETACH_SCALE, "f1_suspension_damage.parts_detach_scale", SUSPENSION_DAMAGE, 0x68, Kind::obf,
     0, BIG},
    {SWOFTY_CAR_SUSP_LINEAR_FRACTURE_LIMIT, "f1_suspension_damage.linear_fracture_limit", SUSPENSION_DAMAGE, 0x78,
     Kind::plain, 0, BIG},
    {SWOFTY_CAR_SUSP_ANGULAR_FRACTURE_LIMIT, "f1_suspension_damage.angular_fracture_limit", SUSPENSION_DAMAGE, 0x7c,
     Kind::plain, 0, BIG},
    {SWOFTY_CAR_ENGINE_LIMITER, "engine.vtf_limiter_rotation_rate", ENGINE, 0x8b0, Kind::obf, 100, 2500,
     Rederive::engine},
    {SWOFTY_CAR_FINAL_DRIVE, "gearbox.final_drive_ratio", GEARBOX, 0x28, Kind::obf, 1, 20, Rederive::gearbox},
    // Ratios R, N, 1st..9th are CarObfFloat pairs at +0x38 stride 0x10: 1st is +0x58.
    {SWOFTY_CAR_GEAR_RATIO_1, "gearbox.gear_ratio_1", GEARBOX, 0x58, Kind::obf, 0.1f, 20, Rederive::gearbox},
    {SWOFTY_CAR_GEAR_RATIO_1 + 1, "gearbox.gear_ratio_2", GEARBOX, 0x68, Kind::obf, 0.1f, 20, Rederive::gearbox},
    {SWOFTY_CAR_GEAR_RATIO_1 + 2, "gearbox.gear_ratio_3", GEARBOX, 0x78, Kind::obf, 0.1f, 20, Rederive::gearbox},
    {SWOFTY_CAR_GEAR_RATIO_1 + 3, "gearbox.gear_ratio_4", GEARBOX, 0x88, Kind::obf, 0.1f, 20, Rederive::gearbox},
    {SWOFTY_CAR_GEAR_RATIO_1 + 4, "gearbox.gear_ratio_5", GEARBOX, 0x98, Kind::obf, 0.1f, 20, Rederive::gearbox},
    {SWOFTY_CAR_GEAR_RATIO_1 + 5, "gearbox.gear_ratio_6", GEARBOX, 0xa8, Kind::obf, 0.1f, 20, Rederive::gearbox},
    {SWOFTY_CAR_GEAR_RATIO_1 + 6, "gearbox.gear_ratio_7", GEARBOX, 0xb8, Kind::obf, 0.1f, 20, Rederive::gearbox},
    {SWOFTY_CAR_GEAR_RATIO_1 + 7, "gearbox.gear_ratio_8", GEARBOX, 0xc8, Kind::obf, 0.1f, 20, Rederive::gearbox},
    {SWOFTY_CAR_GEAR_RATIO_1 + 8, "gearbox.gear_ratio_9", GEARBOX, 0xd8, Kind::obf, 0.1f, 20, Rederive::gearbox},
    // Version 2. needs names the KB row that must have an independent agree verify row before the field acts.
    {SWOFTY_CAR_AERO_LIFT_SCALE, "aero.lift_scale", F1_AERO, 0x19a4, Kind::plain, 0, 10, Rederive::none, Via::system,
     "CarF1Aero.lift_scale"},
    {SWOFTY_CAR_AERO_DRAG_SCALE, "aero.drag_scale", F1_AERO, 0x19a0, Kind::plain, 0, 10, Rederive::none, Via::system,
     "CarF1Aero.drag_scale"},
    {SWOFTY_CAR_ENGINE_POWER_SCALE, "engine.power_scale", 0, 0, Kind::virt, 0, 4, Rederive::none, Via::system,
     "site_Car_EngineDriveTorque", 1, false, 0},
    {SWOFTY_CAR_ENGINE_LIMITER_OFFSET_RPM, "engine.limiter_offset_rpm", 0, 0, Kind::virt, -5000, 5000, Rederive::none,
     Via::system, "site_Car_EngineDriveTorque", 0, false, 1},
    {SWOFTY_CAR_AI_GRIP_SCALE, "ai.grip_scale", 0, 0, Kind::virt, 0, 4, Rederive::none, Via::system,
     "site_Ai_TableGripInput", 1, false, 2},
    {SWOFTY_CAR_TYRES_FRONT_GRIP_SCALE, "tyres.front.grip_scale", 0, 0x2c, Kind::plain, 0, 10, Rederive::none,
     Via::tyre_front, "CarSuspTyreType.grip_multiplier"},
    {SWOFTY_CAR_TYRES_REAR_GRIP_SCALE, "tyres.rear.grip_scale", 0, 0x2c, Kind::plain, 0, 10, Rederive::none,
     Via::tyre_rear, "CarSuspTyreType.grip_multiplier"},
    {SWOFTY_CAR_CHASSIS_BALLAST_KG, "chassis.ballast_kg", CHASSIS, 0x28, Kind::obf, -300, 600, Rederive::chassis,
     Via::system, "Car_Tuning_SetChassisZCom", 0, true},
    {SWOFTY_CAR_BRAKES_BIAS, "brakes.bias", BRAKE_ECU, 0x21d8, Kind::plain, 0, 1, Rederive::none, Via::system,
     "CarBrakeEcu.brake_bias"},
    {SWOFTY_CAR_BRAKES_PRESSURE, "brakes.pressure", BRAKE_ECU, 0x21dc, Kind::plain, 0, 4, Rederive::none, Via::system,
     "CarBrakeEcu.brake_pressure"},
    {SWOFTY_CAR_SUSP_FRONT_CAMBER, "suspension.front.camber", 0, 0x1650, Kind::plain, -1, 1, Rederive::none,
     Via::axle_front, "CarSuspAxle.setup_camber"},
    {SWOFTY_CAR_SUSP_FRONT_TOE, "suspension.front.toe", 0, 0x1654, Kind::plain, -1, 1, Rederive::none, Via::axle_front,
     "CarSuspAxle.setup_toe"},
    {SWOFTY_CAR_SUSP_FRONT_ANTI_ROLL, "suspension.front.anti_roll", 0, 0x1664, Kind::plain, 0, BIG, Rederive::none,
     Via::axle_front, "CarSuspAxle.setup_anti_roll"},
    {SWOFTY_CAR_SUSP_FRONT_RIDE_HEIGHT_OFFSET, "suspension.front.ride_height_offset", 0, 0x15e4, Kind::plain, -1, 1,
     Rederive::none, Via::axle_front, "CarSuspAxle.ride_height_offset"},
    {SWOFTY_CAR_SUSP_REAR_CAMBER, "suspension.rear.camber", 0, 0x1650, Kind::plain, -1, 1, Rederive::none,
     Via::axle_rear, "CarSuspAxle.setup_camber"},
    {SWOFTY_CAR_SUSP_REAR_TOE, "suspension.rear.toe", 0, 0x1654, Kind::plain, -1, 1, Rederive::none, Via::axle_rear,
     "CarSuspAxle.setup_toe"},
    {SWOFTY_CAR_SUSP_REAR_ANTI_ROLL, "suspension.rear.anti_roll", 0, 0x1664, Kind::plain, 0, BIG, Rederive::none,
     Via::axle_rear, "CarSuspAxle.setup_anti_roll"},
    {SWOFTY_CAR_SUSP_REAR_RIDE_HEIGHT_OFFSET, "suspension.rear.ride_height_offset", 0, 0x15e4, Kind::plain, -1, 1,
     Rederive::none, Via::axle_rear, "CarSuspAxle.ride_height_offset"},
    // Version 3: the FuelECU mode tables, burn, power, revs and temperature scale of lean, standard, rich, max.
    {SWOFTY_CAR_FUEL_BASE + 0, "fuel_ecu.fuel_burn_scale.lean", FUEL_ECU, 0x28, Kind::obf, 0.0f, 10.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 1, "fuel_ecu.fuel_burn_scale.standard", FUEL_ECU, 0x38, Kind::obf, 0.0f, 10.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 2, "fuel_ecu.fuel_burn_scale.rich", FUEL_ECU, 0x48, Kind::obf, 0.0f, 10.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 3, "fuel_ecu.fuel_burn_scale.max", FUEL_ECU, 0x58, Kind::obf, 0.0f, 10.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 4, "fuel_ecu.fuel_power_scale.lean", FUEL_ECU, 0x68, Kind::obf, 0.0f, 4.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 5, "fuel_ecu.fuel_power_scale.standard", FUEL_ECU, 0x78, Kind::obf, 0.0f, 4.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 6, "fuel_ecu.fuel_power_scale.rich", FUEL_ECU, 0x88, Kind::obf, 0.0f, 4.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 7, "fuel_ecu.fuel_power_scale.max", FUEL_ECU, 0x98, Kind::obf, 0.0f, 4.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 8, "fuel_ecu.fuel_revs_scale.lean", FUEL_ECU, 0xa8, Kind::obf, 0.5f, 2.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 9, "fuel_ecu.fuel_revs_scale.standard", FUEL_ECU, 0xb8, Kind::obf, 0.5f, 2.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 10, "fuel_ecu.fuel_revs_scale.rich", FUEL_ECU, 0xc8, Kind::obf, 0.5f, 2.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 11, "fuel_ecu.fuel_revs_scale.max", FUEL_ECU, 0xd8, Kind::obf, 0.5f, 2.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 12, "fuel_ecu.temperature_scale.lean", FUEL_ECU, 0xe8, Kind::obf, 0.0f, 4.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 13, "fuel_ecu.temperature_scale.standard", FUEL_ECU, 0xf8, Kind::obf, 0.0f, 4.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 14, "fuel_ecu.temperature_scale.rich", FUEL_ECU, 0x108, Kind::obf, 0.0f, 4.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    {SWOFTY_CAR_FUEL_BASE + 15, "fuel_ecu.temperature_scale.max", FUEL_ECU, 0x118, Kind::obf, 0.0f, 4.0f, Rederive::fuel,
     Via::system, "Car_FuelEcu_SetMode_Thunk"},
    // Version 4: the F1Aero slipstream scales. A value below 1 makes the tow stronger; the car width widens the wake.
    {SWOFTY_CAR_AERO_SLIPSTREAM_DRAG, "aero.slipstream_drag", F1_AERO, 0x118, Kind::obf, 0, 1, Rederive::none,
     Via::system, "CarF1Aero.slipstream"},
    {SWOFTY_CAR_AERO_SLIPSTREAM_FRONT_WING, "aero.slipstream_front_wing", F1_AERO, 0xe8, Kind::obf, 0, 1, Rederive::none,
     Via::system, "CarF1Aero.slipstream"},
    {SWOFTY_CAR_AERO_SLIPSTREAM_REAR_WING, "aero.slipstream_rear_wing", F1_AERO, 0xf8, Kind::obf, 0, 1, Rederive::none,
     Via::system, "CarF1Aero.slipstream"},
    {SWOFTY_CAR_AERO_SLIPSTREAM_LIFT, "aero.slipstream_lift", F1_AERO, 0x108, Kind::obf, 0, 1, Rederive::none,
     Via::system, "CarF1Aero.slipstream"},
    {SWOFTY_CAR_AERO_SLIPSTREAM_CAR_WIDTH, "aero.slipstream_car_width", F1_AERO, 0x128, Kind::obf, 0.1f, 10, Rederive::none,
     Via::system, "CarF1Aero.slipstream"},
    // Version 5: the blocking evaluator's per-car config floats (ai_racecraft.hpp), read afresh on every AI tick.
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_FRACTION, blocking, fraction_of_follow_speed, 0.0f, 4.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_FRACTION_RACE_START, blocking, fraction_of_follow_speed_at_race_start, 0.0f, 4.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE, blocking, max_blocking_distance, 0.0f, 1000.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE_RACE_START,
            blocking, max_blocking_distance_at_race_start, 0.0f, 1000.0f),
    // Version 6: the rest of the driver config's float, int and curve slots (ai_racecraft.hpp DRIVER). A curve field
    // is a flat curve at the composed value; get returns the curve at x = 0.
    AI_CELL(SWOFTY_CAR_AI_ALONGSIDE_SPEED_DIFFERENCE_FOR_LIMITED_ZONE_YIELDING,
            alongside, speed_difference_for_limited_zone_yielding, 0.0f, 500.0f),
    AI_CELL(SWOFTY_CAR_AI_ALONGSIDE_YIELD_OVERTAKE_RATE, alongside, yield_overtake_rate, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_ALONGSIDE_MIN_PRESENCE_WITHIN_BUFFER, alongside, min_presence_within_buffer, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_PRESENCE_ALONGSIDE,
            divebomb_defense, max_presence_alongside, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TIME_TO_IGNORE, divebomb_defense, max_time_to_ignore, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TRACK_OFFSET, divebomb_defense, max_track_offset, 0.0f, 50.0f),
    AI_CELL(SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TRACK_RADIUS, divebomb_defense, max_track_radius, 0.0f, 10000.0f),
    AI_CELL(SWOFTY_CAR_AI_OFFSET_VARIATION_MAX_OFFSET, offset_variation, max_offset, 0.0f, 50.0f),
    AI_CELL(SWOFTY_CAR_AI_OFFSET_VARIATION_OFFSET_LIMIT_AT_CORNER_START,
            offset_variation, offset_limit_at_corner_start, 0.0f, 50.0f),
    AI_CELL(SWOFTY_CAR_AI_OFFSET_VARIATION_OFFSET_LIMIT_BLEND_DISTANCE,
            offset_variation, offset_limit_blend_distance, 0.0f, 1000.0f),
    AI_CELL(SWOFTY_CAR_AI_OFFSET_VARIATION_HALF_WIDTH_SCALING_GRAPH,
            offset_variation, half_width_scaling_graph, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OFFSET_VARIATION_RADIUS_SCALAR_GRAPH, offset_variation, radius_scalar_graph, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_ATTACKER_CLOSE_ENOUGH_CAR_LENGTHS,
            overtake, attacker_close_enough_car_lengths, 0.0f, 50.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_BRAKING_ZONE_ABORT_OVERTAKE_TIME,
            overtake, braking_zone_abort_overtake_time, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_CLOSE_RANGE_CAR_LENGTHS_TO_CONTINUE_OVERTAKE,
            overtake, close_range_car_lengths_to_continue_overtake, 0.0f, 50.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_CLOSE_RANGE_CAR_LENGTHS_TO_START_OVERTAKE,
            overtake, close_range_car_lengths_to_start_overtake, 0.0f, 50.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_OPPERTUNIST_OVERTAKE_TIME, overtake, oppertunist_overtake_time, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_OPPERTUNIST_OVERTAKE_TRACK_SPEED_DIFFERENCE,
            overtake, oppertunist_overtake_track_speed_difference, 0.0f, 500.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_CONTINUE_TIME_ADJUSTMENT,
            overtake, danger_overtake_continue_time_adjustment, -100.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_FULL_HEAT_TIME_ADJUSTMENT,
            overtake, danger_overtake_full_heat_time_adjustment, -100.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_INITIATE_TIME_ADJUSTMENT,
            overtake, danger_overtake_initiate_time_adjustment, -100.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_PULL_OUT_TIME, overtake, danger_overtake_pull_out_time, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_CONTINUE_TIME_ADJUSTMENT,
            overtake, regular_overtake_continue_time_adjustment, -100.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_FULL_HEAT_TIME_ADJUSTMENT,
            overtake, regular_overtake_full_heat_time_adjustment, -100.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_INITIATE_TIME_ADJUSTMENT,
            overtake, regular_overtake_initiate_time_adjustment, -100.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_PULL_OUT_TIME,
            overtake, regular_overtake_pull_out_time, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_OVERLAP_PERCENTAGE, overtake, overlap_percentage, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_CONCEDE_SPEED_DIFF, overtake, concede_speed_diff, -500.0f, 500.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_CONCEDE_SLOWDOWN_SPEED_DIFF, overtake, concede_slowdown_speed_diff, 0.0f, 500.0f),
    AI_CELL(SWOFTY_CAR_AI_OVERTAKE_CONCEDE_TIME_VS_BEHIND, overtake, concede_time_vs_behind, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_ADDED_BUFFER, space_seeking, added_buffer, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_SPEED_BELOW_FOLLOW, space_seeking, speed_below_follow, 0.0f, 500.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_FIRST_SPEED_BELOW_FOLLOW,
            space_seeking, first_speed_below_follow, 0.0f, 500.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_CAR_IN_FRONT_NEEDED, space_seeking, car_in_front_needed, 0.0f, 32.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_REMEMBER_CROWED, space_seeking, remember_crowed, 0.0f, 600.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_CONCEDE_OVERTAKE_BLOCKED_BY_CROWED_TIME,
            space_seeking, concede_overtake_blocked_by_crowed_time, 0.0f, 600.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_TOTAL_QUEUE_NEEDED, space_seeking, total_queue_needed, 0.0f, 32.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_DISTANCE_BRAKING_POINT, space_seeking, distance_braking_point, 0.0f, 1000.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_TIME_TO_SWITCH_SIDES, space_seeking, time_to_switch_sides, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_TRACK_DISTANCE_NEEDED, space_seeking, track_distance_needed, 0.0f, 1000.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_TRACK_ABORT_DISTANCE, space_seeking, track_abort_distance, 0.0f, 1000.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_CARS_MOVING_AWAY_SPEED_TO_IGNORE,
            space_seeking, cars_moving_away_speed_to_ignore, 0.0f, 500.0f),
    AI_CELL(SWOFTY_CAR_AI_SPACE_SEEKING_CAN_START_TRACK_RADIUS, space_seeking, can_start_track_radius, 0.0f, 10000.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_INTERSECTION_TIME, blocking, start_blocking_intersection_time, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_MAXIMUM_TIME_TO_CORNER, blocking, start_blocking_maximum_time_to_corner, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_MINIMUM_TIME_TO_CORNER, blocking, start_blocking_minimum_time_to_corner, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_TIME_TO_CORNER_FOR_END_MERGE_RATE, blocking, time_to_corner_for_end_merge_rate, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_TIME_TO_CORNER_FOR_START_MERGE_RATE, blocking, time_to_corner_for_start_merge_rate, 0.0f, 60.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_END_BLOCKING_TIME_TO_CORNER_VS_OFFSET, blocking, end_blocking_time_to_corner_vs_offset, 0.0f, 50.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_BLOCKING_OFFSET_FROM_CAR_BEHIND, blocking, blocking_offset_from_car_behind, 0.0f, 50.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_BLOCKING_SPEED_DIFFERENTIAL, blocking, blocking_speed_differential, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_BLOCKING_THREAT_TIME_MODIFIER, blocking, blocking_threat_time_modifier, -10.0f, 10.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_BLOCKING_VARIABLE_OFFSET_FRACTION, blocking, blocking_variable_offset_fraction, 0.0f, 1.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_AVOIDANCE_DECREASE_RATE, blocking, avoidance_decrease_rate, 0.0f, 100.0f),
    AI_CELL(SWOFTY_CAR_AI_BLOCKING_MERGE_BACK_TO_RACING_LINE, blocking, merge_back_to_racing_line, 0.0f, 10.0f),
};


#undef AI_CELL

inline constexpr const char *RACECRAFT_ROWS[] = {"AiSystem.records_begin", "AiCarRecord.vehicle_index",
                                                 "AiCarRecord.driver_config", "site_Ai_BuildSystem_RecordPushed"};

inline constexpr size_t FIELD_COUNT = sizeof FIELDS / sizeof FIELDS[0];

inline int field_index(uint32_t id)
{
    for (size_t i = 0; i < FIELD_COUNT; i++)
        if (FIELDS[i].id == id)
            return int(i);
    return -1;
}

inline const Field *find_field_path(const char *path)
{
    if (!path)
        return nullptr;
    for (auto &f : FIELDS)
        if (!std::strcmp(f.name, path))
            return &f;
    return nullptr;
}

inline const Field *field(uint32_t id)
{
    int i = field_index(id);
    return i < 0 ? nullptr : &FIELDS[i];
}

struct Env {
    std::function<bool(uintptr_t addr, void *out, size_t n)> read;
    std::function<bool(uintptr_t addr, const void *in, size_t n)> write;
    std::function<bool()> flow_thread;
    std::function<bool()> enabled;
    std::function<void(uintptr_t engine)> engine_derived;
    std::function<void(uintptr_t gearbox, uint32_t flags)> gearbox_reset;
    std::function<void(uintptr_t chassis, uint32_t flags)> chassis_reset;
    std::function<void(uintptr_t fuel_ecu, int mode)> fuel_set_mode;
    // The address of the object g_Game_EngineModeAvail points at (CarBuild::mode_avail_rva), or 0.
    std::function<uintptr_t()> mode_avail;
    std::function<uintptr_t(uint32_t index, uintptr_t car)> tyre_component;
    std::function<void(uintptr_t tyre_component)> tyre_refit;
    // Whether the KB row a field needs has its independent agree verify row; null means every row is ready.
    std::function<bool(const char *row)> row_ready;
    uintptr_t table = 0, count = 0;
};

constexpr float RPM_TO_RAD_S = 0.10471976f;
constexpr float LIMITER_RAMP_RPM = 60.0f;

// The engine drive torque per full throttle after the car's power factor and limiter offset (the cut starts
// LIMITER_RAMP_RPM under the lowered limiter and reaches zero torque at it). A positive offset changes nothing.
inline float shaped_torque(float torque, float rad_s, float limiter_rad_s, float power_scale, float limiter_offset_rpm)
{
    float k = power_scale;
    if (limiter_offset_rpm < 0 && limiter_rad_s > 0) {
        float cut = limiter_rad_s + limiter_offset_rpm * RPM_TO_RAD_S;
        float ramp = LIMITER_RAMP_RPM * RPM_TO_RAD_S;
        if (cut > 0 && rad_s > cut - ramp) {
            float f = (cut - rad_s) / ramp;
            k *= f > 0.f ? f : 0.f;
        }
    }
    return torque * k;
}

template <class Tag> struct Ops {
    static inline Env env;
    static inline std::atomic<bool> bound{false};

    struct LayerRec {
        uint32_t token;
        std::string owner;
        uint32_t index, field;
        uint32_t op;
        float value;
        int32_t priority;
        uint64_t order;
    };
    struct Slot {
        bool valid = false;
        float base = 0, written = 0;
    };
    static inline std::mutex lay_lock;
    static inline std::vector<LayerRec> layers;
    static inline uint32_t next_token = 1;
    static inline uint64_t next_order = 0;
    static inline Slot slots[MAX_CARS][FIELD_COUNT];
    static inline ai_racecraft::Fields racecraft;
    static inline std::atomic<float> virt[MAX_CARS][VIRTUALS];
    static inline std::atomic<bool> dirty{false}, power_active{false}, ai_active{false};
    static inline std::mutex slot_lock;

    static void reset_virtuals()
    {
        for (uint32_t c = 0; c < MAX_CARS; c++)
            for (auto &f : FIELDS)
                if (f.kind == Kind::virt)
                    virt[c][f.vslot] = f.neutral;
        power_active = false;
        ai_active = false;
    }

    static void bind(Env e)
    {
        bound = false;
        env = std::move(e);
        {
            std::lock_guard<std::mutex> g(lay_lock);
            layers.clear();
            next_token = 1;
            next_order = 0;
        }
        {
            std::lock_guard<std::mutex> g(slot_lock);
            for (auto &row : slots)
                for (auto &s : row)
                    s = Slot{};
        }
        reset_virtuals();
        racecraft.forget();
        dirty = false;
        std::lock_guard<std::mutex> g(pending_lock);
        pending.clear();
        bound = true;
    }

    template <class T> static bool rd(uintptr_t a, T &v) { return a && env.read && env.read(a, &v, sizeof v); }

    static int available() { return bound.load() && env.table && env.count && (!env.enabled || env.enabled()) ? 1 : 0; }

    static bool ready(const Field &f)
    {
        if (f.needs && env.row_ready && !env.row_ready(f.needs))
            return false;
        if (f.kind == Kind::cell && env.row_ready)
            for (const char *row : RACECRAFT_ROWS)
                if (!env.row_ready(row))
                    return false;
        return true;
    }

    static ai_racecraft::Mem cell_mem() { return {env.read, env.write}; }

    static int cars(uint32_t &n)
    {
        uint32_t c = 0;
        if (!available() || !rd(env.count, c))
            return SWOFTY_ASSET_UNAVAILABLE;
        n = c < MAX_CARS ? c : MAX_CARS;
        return SWOFTY_ASSET_OK;
    }

    static int car_at(uint32_t index, const void *handle, uintptr_t &car)
    {
        uint32_t n = 0;
        if (int r = cars(n); r != SWOFTY_ASSET_OK)
            return r;
        if (index >= MAX_CARS)
            return SWOFTY_ASSET_INVALID;
        uint8_t id = 0;
        if (index >= n || !rd(env.table + index * 8, car) || !car || !rd(car + CAR_VEHICLE_ID, id) || id != index)
            return SWOFTY_ASSET_LATE;
        if (handle && car != reinterpret_cast<uintptr_t>(handle))
            return SWOFTY_ASSET_LATE;
        return SWOFTY_ASSET_OK;
    }

    static int system_at(uintptr_t car, uintptr_t offset, uintptr_t &sys)
    {
        uintptr_t owner = 0;
        if (!rd(car + offset, sys))
            return SWOFTY_ASSET_LATE;
        if (!sys)
            return SWOFTY_ASSET_UNAVAILABLE;
        return rd(sys + SYSTEM_CAR, owner) && owner == car ? SWOFTY_ASSET_OK : SWOFTY_ASSET_LATE;
    }

    // The address of a field's storage on one car; sys is the system the field belongs to (for a rederive).
    static int locate(uintptr_t car, const Field &f, uintptr_t &sys, uintptr_t &at)
    {
        if (f.kind == Kind::virt || f.kind == Kind::cell)
            return SWOFTY_ASSET_UNAVAILABLE;
        if (f.via == Via::system) {
            if (int r = system_at(car, f.system, sys); r != SWOFTY_ASSET_OK)
                return r;
            at = sys + f.offset;
            return SWOFTY_ASSET_OK;
        }
        if (int r = system_at(car, SUSPENSION, sys); r != SWOFTY_ASSET_OK)
            return r;
        if (f.via == Via::tyre_front || f.via == Via::tyre_rear) {
            const char *want = f.via == Via::tyre_front ? "Front" : "Rear";
            for (uintptr_t i = 0; i < 2; i++) {
                uintptr_t rec = 0;
                char name[16] = {};
                if (!rd(sys + SUSP_TYRE_TYPES + i * 8, rec) || !rec || !rd(rec + TYRE_TYPE_NAME, name))
                    continue;
                name[15] = 0;
                if (!std::strcmp(name, want)) {
                    at = rec + f.offset;
                    return SWOFTY_ASSET_OK;
                }
            }
            return SWOFTY_ASSET_UNAVAILABLE;
        }
        uintptr_t a0 = 0, a1 = 0;
        float z0 = 0, z1 = 0;
        if (!rd(sys + SUSP_AXLE0, a0) || !rd(sys + SUSP_AXLE1, a1) || !a0 || !a1 || !rd(a0 + AXLE_LOCAL_REST_Z, z0) ||
            !rd(a1 + AXLE_LOCAL_REST_Z, z1) || (z0 <= 0) == (z1 <= 0))
            return SWOFTY_ASSET_UNAVAILABLE;
        uintptr_t rear = z0 <= 0 ? a0 : a1, front = z0 <= 0 ? a1 : a0;
        at = (f.via == Via::axle_rear ? rear : front) + f.offset;
        return SWOFTY_ASSET_OK;
    }

    static bool load(uintptr_t at, Kind kind, float &v)
    {
        uint32_t a = 0, key = 0;
        if (!rd(at, a) || (kind == Kind::obf && !rd(at + 8, key)))
            return false;
        a ^= key;
        std::memcpy(&v, &a, 4);
        return true;
    }

    static bool store(uintptr_t at, Kind kind, float value)
    {
        uint32_t bits = 0, key = 0;
        std::memcpy(&bits, &value, 4);
        if (kind == Kind::obf && !rd(at + 8, key))
            return false;
        bits ^= key;
        return env.write && env.write(at, &bits, 4);
    }

    static int op_available() { return available(); }

    static int op_count(uint32_t *n)
    {
        uint32_t c = 0;
        int r = cars(c);
        if (r == SWOFTY_ASSET_OK && n)
            *n = c;
        return r;
    }

    static int op_car(uint32_t index, void **handle, uint32_t *is_ai)
    {
        uintptr_t car = 0;
        if (int r = car_at(index, nullptr, car); r != SWOFTY_ASSET_OK)
            return r;
        uint32_t ai = 0;
        if (is_ai && !rd(car + CAR_IS_AI, ai))
            return SWOFTY_ASSET_LATE;
        if (handle)
            *handle = reinterpret_cast<void *>(car);
        if (is_ai)
            *is_ai = ai;
        return SWOFTY_ASSET_OK;
    }

    static std::vector<LayerRec> layers_of(uint32_t index)
    {
        std::vector<LayerRec> mine;
        std::lock_guard<std::mutex> g(lay_lock);
        for (auto &l : layers)
            if (l.index == SWOFTY_CAR_ALL || l.index == index)
                mine.push_back(l);
        return mine;
    }

    static float composed(uint32_t index, const Field &f, float start)
    {
        return composed_of(layers_of(index), f, start);
    }

    static float composed_of(const std::vector<LayerRec> &all, const Field &f, float start)
    {
        std::vector<LayerRec> mine;
        for (auto &l : all)
            if (l.field == f.id)
                mine.push_back(l);
        std::stable_sort(mine.begin(), mine.end(), [](const LayerRec &a, const LayerRec &b) {
            return a.priority != b.priority ? a.priority < b.priority : a.order < b.order;
        });
        float v = start;
        for (auto &l : mine)
            v = l.op == SWOFTY_CAR_OP_SET ? l.value : l.op == SWOFTY_CAR_OP_SCALE ? v * l.value : v + l.value;
        return v;
    }

    static bool has_layer(uint32_t index, uint32_t field)
    {
        std::lock_guard<std::mutex> g(lay_lock);
        for (auto &l : layers)
            if (l.field == field && (l.index == SWOFTY_CAR_ALL || l.index == index))
                return true;
        return false;
    }

    static float clamp(const Field &f, float v) { return std::min(std::max(v, f.lo), f.hi); }

    static void refresh_virtuals()
    {
        bool power = false, ai = false;
        for (auto &f : FIELDS) {
            if (f.kind != Kind::virt)
                continue;
            for (uint32_t c = 0; c < MAX_CARS; c++) {
                float v = clamp(f, composed(c, f, f.neutral));
                virt[c][f.vslot] = v;
                if (v != f.neutral) {
                    if (f.vslot == 2)
                        ai = true;
                    else
                        power = true;
                }
            }
        }
        power_active = power;
        ai_active = ai;
    }

    static int op_get(uint32_t index, const void *handle, uint32_t id, float *value)
    {
        const Field *f = field(id);
        if (!f || !value)
            return SWOFTY_ASSET_INVALID;
        if (!ready(*f))
            return SWOFTY_ASSET_UNAVAILABLE;
        if (f->kind == Kind::virt) {
            if (index >= MAX_CARS)
                return SWOFTY_ASSET_INVALID;
            *value = virt[index][f->vslot].load();
            return SWOFTY_ASSET_OK;
        }
        if (f->kind == Kind::cell) {
            uintptr_t car = 0;
            if (int r = car_at(index, handle, car); r != SWOFTY_ASSET_OK)
                return r;
            return racecraft.read(cell_mem(), index, f->vslot, *value) ? SWOFTY_ASSET_OK : SWOFTY_ASSET_UNAVAILABLE;
        }
        uintptr_t car = 0, sys = 0, at = 0;
        if (int r = car_at(index, handle, car); r != SWOFTY_ASSET_OK)
            return r;
        if (int r = locate(car, *f, sys, at); r != SWOFTY_ASSET_OK)
            return r;
        float v = 0;
        if (!load(at, f->kind, v))
            return SWOFTY_ASSET_LATE;
        if (f->relative) {
            std::lock_guard<std::mutex> g(slot_lock);
            Slot &s = slots[index][field_index(id)];
            v -= s.valid ? s.base : v;
        }
        *value = v;
        return SWOFTY_ASSET_OK;
    }

    // Writes one value into a located field and re-runs what the game derives from it.
    static bool rerun_fuel(uintptr_t sys)
    {
        int32_t mode = 1;
        if (!rd(sys + FUEL_ECU_MODE, mode) || mode < 0 || mode > 3)
            return false;
        env.fuel_set_mode(sys, mode == 0 ? 1 : 0);
        env.fuel_set_mode(sys, mode);
        return true;
    }

    static int write_value(uintptr_t car, uint32_t index, const Field &f, uintptr_t sys, uintptr_t at, float value,
                           bool rerun = true)
    {
        if ((f.rederive == Rederive::engine && !env.engine_derived) ||
            (f.rederive == Rederive::gearbox && !env.gearbox_reset) ||
            (f.rederive == Rederive::chassis && !env.chassis_reset) || (f.rederive == Rederive::fuel && !env.fuel_set_mode))
            return SWOFTY_ASSET_UNAVAILABLE;
        uint8_t setup_ratios = 0;
        if (f.rederive == Rederive::gearbox && (!rd(sys + GEARBOX_USE_SETUP_RATIOS, setup_ratios) || setup_ratios))
            return SWOFTY_ASSET_UNAVAILABLE;
        if (f.id == SWOFTY_CAR_FUEL_MASS) {
            float capacity = 0;
            if (load(sys + FUEL_CAPACITY, Kind::obf, capacity) && capacity > 0 && value > capacity)
                return SWOFTY_ASSET_INVALID;
        }
        if (!store(at, f.kind, value))
            return SWOFTY_ASSET_LATE;
        if (f.rederive == Rederive::engine)
            env.engine_derived(sys);
        else if (f.rederive == Rederive::gearbox)
            env.gearbox_reset(sys, GEARBOX_REBUILD);
        else if (f.rederive == Rederive::fuel) {
            if (rerun && !rerun_fuel(sys))
                return SWOFTY_ASSET_LATE;
        } else if (f.rederive == Rederive::chassis) {
            if (!store(sys + CHASSIS_SQRT_MASS, Kind::obf, std::sqrt(std::max(value, 0.f))))
                return SWOFTY_ASSET_LATE;
            env.chassis_reset(sys, CHASSIS_REBUILD);
        }
        (void)car;
        (void)index;
        return SWOFTY_ASSET_OK;
    }

    // One car's fuel tank: the kg in it (+0x100) and its capacity (+0x98), for the box fuel rules (race_fuel.hpp).
    static int fuel_of(uint32_t index, float &mass, float &capacity)
    {
        uintptr_t car = 0, sys = 0;
        if (int r = car_at(index, nullptr, car); r != SWOFTY_ASSET_OK)
            return r;
        if (int r = system_at(car, FUEL_TANK, sys); r != SWOFTY_ASSET_OK)
            return r;
        if (!load(sys + FUEL_MASS, Kind::plain, mass) || !load(sys + FUEL_CAPACITY, Kind::obf, capacity) ||
            !std::isfinite(mass) || !std::isfinite(capacity))
            return SWOFTY_ASSET_LATE;
        return SWOFTY_ASSET_OK;
    }

    static int op_set(uint32_t index, const void *handle, uint32_t id, float value)
    {
        const Field *f = field(id);
        if (!f || !std::isfinite(value) || value < f->lo || value > f->hi)
            return SWOFTY_ASSET_INVALID;
        if (!ready(*f) || f->kind == Kind::cell)
            return SWOFTY_ASSET_UNAVAILABLE;
        if (f->kind == Kind::virt) {
            if (index >= MAX_CARS)
                return SWOFTY_ASSET_INVALID;
            virt[index][f->vslot] = value;
            if (value != f->neutral)
                (f->vslot == 2 ? ai_active : power_active).store(true);
            return SWOFTY_ASSET_OK;
        }
        if (env.flow_thread && !env.flow_thread())
            return SWOFTY_ASSET_LATE;
        uintptr_t car = 0, sys = 0, at = 0;
        if (int r = car_at(index, handle, car); r != SWOFTY_ASSET_OK)
            return r;
        if (int r = locate(car, *f, sys, at); r != SWOFTY_ASSET_OK)
            return r;
        float memory = value;
        if (f->relative) {
            float current = 0;
            if (!load(at, f->kind, current))
                return SWOFTY_ASSET_LATE;
            std::lock_guard<std::mutex> g(slot_lock);
            Slot &s = slots[index][field_index(id)];
            float base = s.valid && bitwise_equal(current, s.written) ? s.base : current;
            memory = base + value;
            if (!(memory > 0))
                return SWOFTY_ASSET_INVALID;
            if (int r = write_value(car, index, *f, sys, at, memory); r != SWOFTY_ASSET_OK)
                return r;
            float written = memory;
            load(at, f->kind, written);
            s = Slot{true, base, written};
            return SWOFTY_ASSET_OK;
        }
        return write_value(car, index, *f, sys, at, memory);
    }

    static bool bitwise_equal(float a, float b)
    {
        uint32_t x, y;
        std::memcpy(&x, &a, 4);
        std::memcpy(&y, &b, 4);
        return x == y;
    }

    // A cell field changes only on the flow thread, with the AI system settled (ai_racecraft.hpp); anything else waits
    // for the next apply.
    static void apply_cell(uint32_t index, const Field &f, bool want, const std::vector<LayerRec> &mine)
    {
        if (!want && !racecraft.swapped(index, f.vslot))
            return;
        if (!ready(f))
            return;
        if (env.flow_thread && !env.flow_thread()) {
            dirty = true;
            return;
        }
        racecraft.apply(cell_mem(), index, f.vslot, want,
                        [&](float base) { return clamp(f, composed_of(mine, f, base)); });
    }

    // The core's handlers: the AI system built (site_Ai_BuildSystem_RecordPushed), a frame, a new session.
    static void racecraft_pushed(uintptr_t system) { racecraft.pushed(cell_mem(), system); }
    static void racecraft_forget() { racecraft.forget(); }
    static void racecraft_tick()
    {
        if (racecraft.tick(cell_mem()))
            dirty = true;
    }

    // Applies every layer that matches one car now. A memory field's base is the value the game last wrote: the
    // value read is the game's when it differs from what this wrote, so a reset (which restores the game's value)
    // is picked up as the new base, and a layer applied twice does not compound.
    static void apply_car(uint32_t index)
    {
        uintptr_t car = 0;
        if (car_at(index, nullptr, car) != SWOFTY_ASSET_OK)
            return;
        uintptr_t fuel_sys = 0;
        const std::vector<LayerRec> mine = layers_of(index);
        std::unique_lock<std::mutex> g(slot_lock);
        for (size_t i = 0; i < FIELD_COUNT; i++) {
            const Field &f = FIELDS[i];
            if (f.kind == Kind::virt)
                continue;
            bool want = std::any_of(mine.begin(), mine.end(), [&](const LayerRec &l) { return l.field == f.id; });
            if (f.kind == Kind::cell) {
                apply_cell(index, f, want, mine);
                continue;
            }
            Slot &s = slots[index][i];
            if (!want && !s.valid)
                continue;
            if (!ready(f))
                continue;
            uintptr_t sys = 0, at = 0;
            float current = 0;
            if (locate(car, f, sys, at) != SWOFTY_ASSET_OK || !load(at, f.kind, current)) {
                s = Slot{};
                continue;
            }
            float base = s.valid && bitwise_equal(current, s.written) ? s.base : current;
            bool deferred = f.rederive == Rederive::fuel;
            if (!want) {
                if (s.valid && bitwise_equal(current, s.written) && !bitwise_equal(base, current) &&
                    write_value(car, index, f, sys, at, base, !deferred) == SWOFTY_ASSET_OK && deferred)
                    fuel_sys = sys;
                s = Slot{};
                continue;
            }
            float target = f.relative ? base + clamp(f, composed_of(mine, f, 0)) : clamp(f, composed_of(mine, f, base));
            if (s.valid && bitwise_equal(current, s.written) && bitwise_equal(target, current)) {
                s.base = base;
                continue;
            }
            if (!std::isfinite(target) || write_value(car, index, f, sys, at, target, !deferred) != SWOFTY_ASSET_OK) {
                s = Slot{};
                continue;
            }
            if (deferred)
                fuel_sys = sys;
            float written = target;
            load(at, f.kind, written);
            s = Slot{true, base, written};
        }
        g.unlock();
        if (fuel_sys)
            rerun_fuel(fuel_sys);
    }

    static void apply_all()
    {
        uint32_t n = 0;
        if (cars(n) != SWOFTY_ASSET_OK)
            return;
        for (uint32_t i = 0; i < n; i++)
            apply_car(i);
    }

    static int op_field(const char *path, uint32_t *id)
    {
        const Field *f = find_field_path(path);
        if (!f || !id)
            return SWOFTY_ASSET_INVALID;
        *id = f->id;
        return SWOFTY_ASSET_OK;
    }

    static int add_layer(const std::string &owner, uint32_t index, uint32_t id, uint32_t op, float value, int32_t priority,
                         uint32_t *token)
    {
        const Field *f = field(id);
        if (!f || op > SWOFTY_CAR_OP_ADD || !std::isfinite(value) || (index != SWOFTY_CAR_ALL && index >= MAX_CARS))
            return SWOFTY_ASSET_INVALID;
        if (!ready(*f))
            return SWOFTY_ASSET_UNAVAILABLE;
        uint32_t t = 0;
        {
            std::lock_guard<std::mutex> g(lay_lock);
            t = next_token++;
            layers.push_back({t, owner, index, id, op, value, priority, next_order++});
        }
        if (token)
            *token = t;
        refresh_virtuals();
        dirty = true;
        return SWOFTY_ASSET_OK;
    }

    static int op_layer(const char *owner, uint32_t index, uint32_t id, uint32_t op, float value, int32_t priority,
                        uint32_t *token)
    {
        int r = add_layer(owner ? owner : "", index, id, op, value, priority, token);
        if (r == SWOFTY_ASSET_OK && (!env.flow_thread || env.flow_thread()))
            flush();
        return r;
    }

    static int op_unlayer(uint32_t token)
    {
        {
            std::lock_guard<std::mutex> g(lay_lock);
            auto it = std::find_if(layers.begin(), layers.end(), [&](const LayerRec &l) { return l.token == token; });
            if (it == layers.end())
                return SWOFTY_ASSET_INVALID;
            layers.erase(it);
        }
        refresh_virtuals();
        dirty = true;
        if (!env.flow_thread || env.flow_thread())
            flush();
        return SWOFTY_ASSET_OK;
    }

    // Removes every layer of one owner for one car (or all, SWOFTY_CAR_ALL): a car.profile re-evaluation.
    static void drop_owner(const std::string &owner, uint32_t index)
    {
        {
            std::lock_guard<std::mutex> g(lay_lock);
            std::erase_if(layers, [&](const LayerRec &l) {
                return l.owner == owner && (index == SWOFTY_CAR_ALL || l.index == index);
            });
        }
        refresh_virtuals();
        dirty = true;
    }

    static void flush()
    {
        if (!dirty.exchange(false))
            return;
        apply_all();
    }

    static int op_refit_tyres(uint32_t index, const char *group)
    {
        if (!group || !*group || std::strlen(group) > 31)
            return SWOFTY_ASSET_INVALID;
        for (const char *c = group; *c; c++)
            if (!((*c >= 'A' && *c <= 'Z') || (*c >= 'a' && *c <= 'z') || (*c >= '0' && *c <= '9') || *c == '_'))
                return SWOFTY_ASSET_INVALID;
        if (!env.tyre_component || !env.tyre_refit || (env.row_ready && !env.row_ready("CarChassis.tyre_physics_group")))
            return SWOFTY_ASSET_UNAVAILABLE;
        if (env.flow_thread && !env.flow_thread())
            return SWOFTY_ASSET_LATE;
        uintptr_t car = 0, chassis = 0;
        if (int r = car_at(index, nullptr, car); r != SWOFTY_ASSET_OK)
            return r;
        if (int r = system_at(car, CHASSIS, chassis); r != SWOFTY_ASSET_OK)
            return r;
        uintptr_t component = env.tyre_component(index, car);
        if (!component)
            return SWOFTY_ASSET_UNAVAILABLE;
        size_t capacity = 0;
        uintptr_t buffer = 0;
        size_t len = std::strlen(group);
        if (!rd(chassis + CHASSIS_GROUP + STRING_CAPACITY, capacity) || !rd(chassis + CHASSIS_GROUP + STRING_BUFFER, buffer) ||
            !buffer || capacity <= len || capacity > 0x1000)
            return SWOFTY_ASSET_UNAVAILABLE;
        if (!env.write || !env.write(buffer, group, len + 1))
            return SWOFTY_ASSET_LATE;
        env.tyre_refit(component);
        return SWOFTY_ASSET_OK;
    }

    // The car of an engine, AI-state or other system pointer (system+8 is its car), for the core's site handlers.
    static bool car_index_of_system(uintptr_t sys, uint32_t &index)
    {
        uintptr_t car = 0;
        uint8_t id = 0;
        if (!rd(sys + SYSTEM_CAR, car) || !car || !rd(car + CAR_VEHICLE_ID, id) || id >= MAX_CARS)
            return false;
        index = id;
        return true;
    }

    static float drive_torque(uintptr_t engine, float torque, float rad_s, float limiter_rad_s)
    {
        return drive_torque_with(engine, torque, rad_s, [&] { return limiter_rad_s; });
    }

    // The site runs for every car on the physics threads; with no power layer it reads no memory, so the engine's
    // limiter is read only once a layer needs it.
    template <class Limiter> static float drive_torque_with(uintptr_t engine, float torque, float rad_s, Limiter limiter)
    {
        uint32_t index = 0;
        if (!power_active.load(std::memory_order_relaxed) || !car_index_of_system(engine, index))
            return torque;
        return shaped_torque(torque, rad_s, limiter(), virt[index][0].load(std::memory_order_relaxed),
                             virt[index][1].load(std::memory_order_relaxed));
    }

    static float ai_grip(uint32_t index, uint8_t weather, float grip)
    {
        if (!ai_active.load(std::memory_order_relaxed) || index >= MAX_CARS || weather != 0)
            return grip;
        return grip * virt[index][2].load(std::memory_order_relaxed);
    }

    // car.built, car.reset, car.upgrades.replayed and car.setup.applied: the game builds each car on a loader thread
    // (T119) and re-reads its parts through Car_Systems_Reset_Thunk (T120), so the hooks only queue the car and the
    // next frame takes the queue on the flow thread, where layers apply and set() may write, keeping the cars still
    // in their slot.
    enum class What : uint8_t { built, reset, replayed, setup, damage };
    struct Pending {
        uint32_t index;
        uintptr_t car;
        What what;
        float progress;
        uint32_t part;
    };
    static inline std::mutex pending_lock;
    static inline std::vector<Pending> pending;

    static void queue(uintptr_t car, What what, float progress = 0, uint32_t part = 0)
    {
        uint8_t id = 0;
        if (!bound.load() || !car || !rd(car + CAR_VEHICLE_ID, id) || id >= MAX_CARS)
            return;
        std::lock_guard<std::mutex> g(pending_lock);
        if (pending.size() < MAX_CARS * 8)
            pending.push_back({id, car, what, progress, part});
    }

    static void built(uintptr_t car) { queue(car, What::built); }
    static void reset(uintptr_t loader) { queue(loader >= CAR_LOADER ? loader - CAR_LOADER : 0, What::reset); }
    static void replayed(uintptr_t loader, float progress)
    {
        queue(loader >= CAR_LOADER ? loader - CAR_LOADER : 0, What::replayed, progress);
    }
    static void damaged(uintptr_t wing, uint32_t part, float amount)
    {
        uintptr_t car = 0;
        if (amount > 0 && rd(wing + SYSTEM_CAR, car))
            queue(car, What::damage, amount, part);
    }
    static void setup_applied(uintptr_t f1_tuning)
    {
        uintptr_t car = 0;
        if (rd(f1_tuning + SYSTEM_CAR, car))
            queue(car, What::setup);
    }

    static std::vector<Pending> take()
    {
        std::vector<Pending> out;
        {
            std::lock_guard<std::mutex> g(pending_lock);
            out.swap(pending);
        }
        std::erase_if(out, [](const Pending &p) {
            uintptr_t car = 0;
            return car_at(p.index, reinterpret_cast<const void *>(p.car), car) != SWOFTY_ASSET_OK;
        });
        return out;
    }

    static std::vector<Pending> coalesce(std::vector<Pending> in)
    {
        std::vector<Pending> out;
        for (const Pending &p : in) {
            auto it = std::find_if(out.begin(), out.end(), [&](const Pending &o) {
                return o.index == p.index && o.what == p.what && o.part == p.part;
            });
            if (it == out.end())
                out.push_back(p);
            else if (p.what == What::damage)
                it->progress += p.progress;
            else {
                it->car = p.car;
                it->progress = p.progress;
            }
        }
        return out;
    }

    static std::vector<Pending> take_frame() { return coalesce(take()); }

    static inline bool replay_deferred = false;

    static bool skip_in_replay(What what, bool in_replay)
    {
        if (!in_replay || what == What::built)
            return false;
        replay_deferred = true;
        return true;
    }

    static bool resume_due(bool in_replay)
    {
        if (in_replay || !replay_deferred)
            return false;
        replay_deferred = false;
        return true;
    }

    static inline std::mutex unlock_lock;
    static inline std::vector<std::pair<std::string, uint32_t>> unlocks;

    static int op_unlock_fuel_mixes(const char *owner, uint32_t mask)
    {
        if (!owner || !*owner || (mask & ~0xfu))
            return SWOFTY_ASSET_INVALID;
        if (!env.mode_avail || !env.write || (env.row_ready && !env.row_ready("EngineModeAvail.fuel")))
            return SWOFTY_ASSET_UNAVAILABLE;
        std::lock_guard<std::mutex> g(unlock_lock);
        for (auto &u : unlocks)
            if (u.first == owner) {
                u.second = mask;
                return SWOFTY_ASSET_OK;
            }
        unlocks.emplace_back(owner, mask);
        return SWOFTY_ASSET_OK;
    }

    static uint32_t unlocked_mask()
    {
        std::lock_guard<std::mutex> g(unlock_lock);
        uint32_t m = 0;
        for (auto &u : unlocks)
            m |= u.second;
        return m;
    }

    // Marks each requested mix available (the entry 0) for every car on track. The MFD's mask and the controller
    // read the entry (Game_Mfd_FuelMix_Refresh, Sess_ControllerSystem_ApplyRecord), so the game's own locks, such as
    // the practice fuel programme's, are reasserted by the game and cleared again on the next frame.
    static void unlock_tick()
    {
        uint32_t mask = unlocked_mask();
        if (!mask || !env.mode_avail || !env.write)
            return;
        uint32_t n = 0;
        if (cars(n) != SWOFTY_ASSET_OK)
            return;
        n = std::min(n, MODE_AVAIL_CARS);
        uintptr_t base = env.mode_avail();
        if (!base)
            return;
        if (!env.read || !n)
            return;
        uint8_t block[MODE_AVAIL_CARS * MODE_AVAIL_STRIDE];
        const size_t span = (n - 1) * MODE_AVAIL_STRIDE + 16;
        if (!env.read(base + MODE_AVAIL_FUEL, block, span))
            return;
        uint32_t zero = 0;
        for (uint32_t c = 0; c < n; c++)
            for (uint32_t m = 0; m < 4; m++) {
                if (!(mask & (1u << m)))
                    continue;
                const size_t off = c * MODE_AVAIL_STRIDE + m * 4;
                uint32_t now;
                std::memcpy(&now, block + off, 4);
                if (now != 0)
                    env.write(base + MODE_AVAIL_FUEL + off, &zero, 4);
            }
    }

    static int op_fuel_mix(uint32_t index, uint32_t *mode)
    {
        uintptr_t car = 0, ecu = 0;
        int32_t m = 0;
        if (!mode)
            return SWOFTY_ASSET_INVALID;
        if (int r = car_at(index, nullptr, car); r != SWOFTY_ASSET_OK)
            return r;
        if (int r = system_at(car, FUEL_ECU, ecu); r != SWOFTY_ASSET_OK)
            return r;
        if (!rd(ecu + FUEL_ECU_MODE, m) || m < 0 || m > 3)
            return SWOFTY_ASSET_LATE;
        *mode = uint32_t(m);
        return SWOFTY_ASSET_OK;
    }

    static int op_set_fuel_mix(uint32_t index, uint32_t mode)
    {
        if (mode > 3)
            return SWOFTY_ASSET_INVALID;
        if (!env.fuel_set_mode)
            return SWOFTY_ASSET_UNAVAILABLE;
        if (env.flow_thread && !env.flow_thread())
            return SWOFTY_ASSET_LATE;
        uintptr_t car = 0, ecu = 0;
        if (int r = car_at(index, nullptr, car); r != SWOFTY_ASSET_OK)
            return r;
        if (int r = system_at(car, FUEL_ECU, ecu); r != SWOFTY_ASSET_OK)
            return r;
        int32_t current = 0;
        if (!rd(ecu + FUEL_ECU_MODE, current) || current < 0 || current > 3)
            return SWOFTY_ASSET_LATE;
        if (env.mode_avail && index < MODE_AVAIL_CARS) {
            uintptr_t base = env.mode_avail();
            uint32_t entry = 0;
            if (!base || !rd(base + MODE_AVAIL_FUEL + index * MODE_AVAIL_STRIDE + mode * 4, entry))
                return SWOFTY_ASSET_LATE;
            if (entry != 0)
                return SWOFTY_ASSET_INVALID;
        }
        if (int32_t(mode) != current)
            env.fuel_set_mode(ecu, int(mode));
        return SWOFTY_ASSET_OK;
    }

    static inline const SwoftyCarOps table = {
        SWOFTY_CAR_OPS_VERSION, op_available, op_count, op_car, op_get, op_set, op_field, op_layer, op_unlayer,
        op_refit_tyres, op_unlock_fuel_mixes, op_fuel_mix, op_set_fuel_mix,
    };
};

}
