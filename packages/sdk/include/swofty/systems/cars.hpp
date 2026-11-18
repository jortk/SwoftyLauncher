// SPDX-License-Identifier: MIT
#pragma once
#include <array>
#include <atomic>
#include <optional>
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/control.hpp>
#include <swofty/systems/forward.hpp>

// Cars: vehicle rows and their packages (boot), each car's handling file (applied as the car is built at
// every session load). Handling records written in data files are applied by the launcher at build (experimental,
// 1.7.0): see Handling. Handling parts are named after the handling
// file's sections and their fields after its keys, in snake_case (FrontCoeffLiftMultiplier ->
// front_coeff_lift_multiplier); a key this header doesn't name is kept in the part's `extra`. cars::live
// is the cars on track, read and written through the core's car operations (SwoftyCoreCarOps): cars()
// lists them, get/set/scale read and write one live::Field, decoded and re-encoded by the core. Without
// those operations (an older core, or an exe that isn't the build the field offsets were read from)
// live::available() is false and live::cars() is empty. A live::Car is a trivially copyable handle (the
// car's index and the game's car), so event payloads carry it as is. control() reads the car's controller
// mode and flags fresh from the core (nothing while the game rebuilds its controller sets, or on a core
// without session operations). cars::live is experimental: nothing in it has run in game yet, and the final
// drive and gear ratio fields depend on the gearbox's use_setup_ratios byte (+0x540), not yet read at runtime.
// Version 2 of the car operations adds field paths (Car::get/set/scale with a string), live::Layer (a layer the
// core keeps and applies again after every build, reset, upgrade replay and setup apply) and the car.profile
// registry; a field whose KB row has no independent verify row answers Result::unavailable.

namespace swofty::cars {

using Result = swofty::Result;

enum class Class { f1, f1_26, f2, classic, my_team, safety_car };
SWOFTY_ENUM(Class, f1, f1_26, f2, classic, my_team, safety_car)
enum class Department : int32_t { chassis, aero, powertrain, durability, tyres };
SWOFTY_ENUM(Department, chassis, aero, powertrain, durability, tyres)

struct Package {
    static constexpr Version version{1, 0};
    Ref<Package> donor;
    std::string resource_path;
    std::string ai_package_folder;
    std::string driver_animation_set;
    bool halo = true;
    bool aero_2026 = false;
    Ref<Package> setups_from;
    StatId stat_id = StatId::automatic;
    json::Value extra;
};
SWOFTY_RECORD(Package, donor, resource_path, ai_package_folder, driver_animation_set, halo, aero_2026, setups_from,
              stat_id)

struct AiSlot {
    Ref<ai::TrackPack> track_pack;
    std::string asset_filename = "auto";
};
SWOFTY_RECORD(AiSlot, track_pack, asset_filename)

struct Vehicle {
    static constexpr Version version{1, 1};
    Ref<Package> package;
    std::string label;
    Ref<Vehicle> donor;
    Path model;
    Ref<sounds::EngineSound> engine_sound;
    Ref<liveries::Livery> livery;
    Ref<Handling> handling;
    Ref<power::Unit> power_unit;
    AiSlot ai;
    Class performance_class = Class::f1;
    int performance_tier = 0;
    float tyre_wear_scale = 1.0f;
    StatId stat_id = StatId::automatic;
    std::optional<bool> drs_capable;
    Ref<power::Supplier> engine;
    std::string tyre_group;
    json::Value extra;
};
SWOFTY_RECORD(Vehicle, package, label, donor, model, engine_sound, livery, handling, power_unit, ai, performance_class,
              performance_tier, tyre_wear_scale, stat_id, drs_capable, engine, tyre_group)

struct WingDamage {
    float speed_threshold = 3, damage_scale = 0.0001f, chassis_damage_multiple = 0.3f, side_detach_scale = 0.5f,
          full_detach_scale = 1;
    json::Value extra;
};
SWOFTY_RECORD(WingDamage, speed_threshold, damage_scale, chassis_damage_multiple, side_detach_scale, full_detach_scale)

struct SuspensionDamage {
    float speed_threshold = 7, front_damage_scale = 0.001f, rear_damage_scale = 0.0001f, wheel_detach_scale = 1,
          parts_detach_scale = 1.5f, linear_fracture_limit = 50, angular_fracture_limit = 30;
    json::Value extra;
};
SWOFTY_RECORD(SuspensionDamage, speed_threshold, front_damage_scale, rear_damage_scale, wheel_detach_scale,
              parts_detach_scale, linear_fracture_limit, angular_fracture_limit)

struct Chassis {
    float mass = 0, moi_x = 0, moi_y = 0, moi_z = 0, com_y = 0, com_z = 0;
    float ffb_scale = 1;
    std::string tyre_physics_group;
    json::Value extra;
};
SWOFTY_RECORD(Chassis, mass, moi_x, moi_y, moi_z, com_y, com_z, ffb_scale, tyre_physics_group)

struct Slipstream {
    float drag = 0.8f, front_wing = 0.76f, rear_wing = 0.82f, lift = 0.91f, car_width = 1.5f;
};
SWOFTY_RECORD(Slipstream, drag, front_wing, rear_wing, lift, car_width)

struct Drs {
    bool hardware = true;
    float angle = 0, open_close_speed = 0;
};
SWOFTY_RECORD(Drs, hardware, angle, open_close_speed)

struct F1Aero {
    float front_coeff_lift_multiplier = 1, rear_coeff_lift_multiplier = 1, body_drag_coeff_min = 0,
          body_drag_coeff_max = 0;
    Curve front_coeff_lift_vs_ride_height, rear_coeff_lift_vs_ride_height;
    Slipstream slipstream;
    Drs drs;
    json::Value extra;
};
SWOFTY_RECORD(F1Aero, front_coeff_lift_multiplier, rear_coeff_lift_multiplier, body_drag_coeff_min, body_drag_coeff_max,
              front_coeff_lift_vs_ride_height, rear_coeff_lift_vs_ride_height, slipstream, drs)

struct CoreAero {
    bool slip_streaming = true, slip_stream_while_braking = false;
    json::Value extra;
};
SWOFTY_RECORD(CoreAero, slip_streaming, slip_stream_while_braking)

struct Engine {
    Curve revs_power_spline;
    float idle_rotation_rate = 0, limiter_rotation_rate = 0;
    float mass = 0, inertia = 0;
    float wear_ice_rate = 0, wear_tc_rate = 0, wear_mguh_rate = 0, wear_ce_rate = 0;
    json::Value extra;
};
SWOFTY_RECORD(Engine, revs_power_spline, idle_rotation_rate, limiter_rotation_rate, mass, inertia, wear_ice_rate,
              wear_tc_rate, wear_mguh_rate, wear_ce_rate)

struct FuelTank {
    float fuel_mass = 0, fuel_bsfc_min = 0;
    json::Value extra;
};
SWOFTY_RECORD(FuelTank, fuel_mass, fuel_bsfc_min)

struct FuelEcu {
    std::array<float, 4> fuel_burn_scale{}, fuel_power_scale{}, fuel_revs_scale{}, temperature_scale{};
    json::Value extra;
};
SWOFTY_RECORD(FuelEcu, fuel_burn_scale, fuel_power_scale, fuel_revs_scale, temperature_scale)

struct ErsMode {
    std::vector<float> deploy;
    json::Value extra;
};
SWOFTY_RECORD(ErsMode, deploy)

struct Ers {
    float max_deploy_power = 0, energy_store_capacity = 0;
    float exhaust_power_coefficient = 0, overtake_power_efficiency = 0, max_brake_torque_mguk = 0;
    std::array<ErsMode, 4> modes;
    json::Value extra;
};
SWOFTY_RECORD(Ers, max_deploy_power, energy_store_capacity, exhaust_power_coefficient, overtake_power_efficiency,
              max_brake_torque_mguk, modes)

struct Range {
    float min = 0, max = 0;
};
SWOFTY_RECORD(Range, min, max)

struct Tuning {
    float differential_on_throttle_multiple_min = 0, brake_bias_max = 0;
    Range chassis_com_z, engine_braking, differential_on_throttle_multiple, differential_off_throttle_multiple,
        differential_mid_corner_multiple, differential_high_speed_kph, differential_preload, front_suspension_camber,
        rear_suspension_camber, front_suspension_toe, rear_suspension_toe, front_suspension_rest_length,
        rear_suspension_rest_length, front_ride_height, rear_ride_height, front_suspension_spring_constant,
        rear_suspension_spring_constant, front_suspension_third_spring, rear_suspension_third_spring,
        front_suspension_anti_roll_bar, rear_suspension_anti_roll_bar, brake_pressure, brake_migration,
        front_tyre_pressure, rear_tyre_pressure;
    json::Value extra;
};
SWOFTY_RECORD(Tuning, differential_on_throttle_multiple_min, brake_bias_max, chassis_com_z, engine_braking,
              differential_on_throttle_multiple, differential_off_throttle_multiple, differential_mid_corner_multiple,
              differential_high_speed_kph, differential_preload, front_suspension_camber, rear_suspension_camber,
              front_suspension_toe, rear_suspension_toe, front_suspension_rest_length, rear_suspension_rest_length,
              front_ride_height, rear_ride_height, front_suspension_spring_constant, rear_suspension_spring_constant,
              front_suspension_third_spring, rear_suspension_third_spring, front_suspension_anti_roll_bar,
              rear_suspension_anti_roll_bar, brake_pressure, brake_migration, front_tyre_pressure, rear_tyre_pressure)

struct Plank {
    float offset_x = 0, offset_y = 0, offset_z = 0, extent_x = 0, extent_y = 0, extent_z = 0;
    json::Value extra;
};
SWOFTY_RECORD(Plank, offset_x, offset_y, offset_z, extent_x, extent_y, extent_z)

struct Section {
    json::Value extra;
};
SWOFTY_RECORD(Section)

struct GearBox {
    float final_drive_ratio = 0, gear_ratio_reverse = 0;
    std::vector<float> ratios;
    std::vector<float> up_speed_multiple, down_speed_multiple;
    float upshift_clutch_time = 0, upshift_gear_delay = 0, time_off_throttle_gear_up = 0;
    float downshift_clutch_time = 0, downshift_gear_delay = 0, time_on_throttle_gear_down = 0;
    float max_clutch_torque = 0;
    bool allow_gear_ratio_tuning = false, allow_h_pattern = false;
    json::Value extra;
};
SWOFTY_RECORD(GearBox, final_drive_ratio, gear_ratio_reverse, ratios, up_speed_multiple, down_speed_multiple,
              upshift_clutch_time, upshift_gear_delay, time_off_throttle_gear_up, downshift_clutch_time,
              downshift_gear_delay, time_on_throttle_gear_down, max_clutch_torque, allow_gear_ratio_tuning,
              allow_h_pattern)

enum class TurboMode { off, lag_spoof, full };
SWOFTY_ENUM(TurboMode, off, lag_spoof, full)

struct Turbo {
    TurboMode mode = TurboMode::full;
    float max_boost = 0;
    json::Value extra;
};
SWOFTY_RECORD(Turbo, mode, max_boost)

struct Axle {
    float rest_x = 0, rest_z = 0, rest_length = 0, min_height = 0, max_height = 0, bump_stop_height = 0,
          ride_height_offset = 0;
    float spring_constant = 0, third_spring_constant = 0, anti_roll_constant = 0, slow_bump_damping = 0,
          fast_bump_damping = 0, slow_rebound_damping = 0, fast_rebound_damping = 0;
    float bump_stop_rising_spring = 0, bump_stop_rising_damping = 0;
    float grip_multiplier = 1, tyre_radius = 0;
    json::Value extra;
};
SWOFTY_RECORD(Axle, rest_x, rest_z, rest_length, min_height, max_height, bump_stop_height, ride_height_offset,
              spring_constant, third_spring_constant, anti_roll_constant, slow_bump_damping, fast_bump_damping,
              slow_rebound_damping, fast_rebound_damping, bump_stop_rising_spring, bump_stop_rising_damping,
              grip_multiplier, tyre_radius)

struct Suspension {
    Axle front, rear;
    bool use_model_positions = false;
    json::Value extra;
};
SWOFTY_RECORD(Suspension, front, rear, use_model_positions)

struct BrakeEcu {
    float brake_front = 0, brake_rear = 0, brake_front_backwards = 0, brake_rear_backwards = 0,
          brake_front_forwards = 0, brake_rear_forwards = 0;
    float brake_migration_torque = 0, brake_migration_torque_ai = 0, brake_pressure = 0;
    json::Value extra;
};
SWOFTY_RECORD(BrakeEcu, brake_front, brake_rear, brake_front_backwards, brake_rear_backwards, brake_front_forwards,
              brake_rear_forwards, brake_migration_torque, brake_migration_torque_ai, brake_pressure)

enum class DriveType { rwd, fwd, awd };
SWOFTY_ENUM(DriveType, rwd, fwd, awd)

struct Differential {
    DriveType drive_type = DriveType::rwd;
    float rear_power_proportion = 100, diff_rear_max_locking_torque = 0;
    json::Value extra;
};
SWOFTY_RECORD(Differential, drive_type, rear_power_proportion, diff_rear_max_locking_torque)

enum class EditOp { set, scale, add };
SWOFTY_ENUM(EditOp, set, scale, add)

struct Edit {
    std::string path;
    EditOp op = EditOp::set;
    float value = 0;
};
SWOFTY_RECORD(Edit, path, op, value)

struct UpgradeChange {
    std::string section, key;
    char op = '=';
    float value = 0;
};
SWOFTY_RECORD(UpgradeChange, section, key, op, value)
[[maybe_unused]] inline std::string_view swofty_field_schema(const UpgradeChange *, std::string_view field)
{
    return field == "op" ? R"({"type":"string","enum":["=","+","-","*"]})" : std::string_view{};
}

struct RadUpgrade {
    std::string name;
    std::vector<UpgradeChange> changes;
    RadUpgrade &set(std::string_view section, std::string_view key, char op, float value)
    {
        for (auto &c : changes)
            if (c.section == section && c.key == key) {
                c.op = op;
                c.value = value;
                return *this;
            }
        changes.push_back({std::string(section), std::string(key), op, value});
        return *this;
    }
};
SWOFTY_RECORD(RadUpgrade, name, changes)

struct Handling {
    static constexpr Version version{1, 1};
    Ref<Handling> from;
    Path vtf;
    Class cls = Class::f1;
    float damage_scale = 1.0f;
    Chassis chassis;
    F1Aero f1_aero;
    CoreAero core_aero;
    WingDamage wing_damage;
    SuspensionDamage suspension_damage;
    Section tyre_damage;
    Section body_damage;
    Section tyres;
    Engine engine;
    GearBox gearbox;
    FuelTank fuel_tank;
    FuelEcu fuel_ecu;
    BrakeEcu brake_ecu;
    Section steering;
    Differential differential;
    Suspension suspension;
    Tuning tuning;
    Section assists;
    Turbo turbo;
    std::optional<Ers> ers = Ers{};
    Plank plank;
    std::vector<RadUpgrade> rad_upgrades;
    std::vector<Edit> edits;
    std::vector<std::string> remove;
    json::Value extra;

    RadUpgrade &upgrade(std::string_view name)
    {
        for (auto &u : rad_upgrades)
            if (u.name == name)
                return u;
        rad_upgrades.push_back({std::string(name), {}});
        return rad_upgrades.back();
    }
};
SWOFTY_RECORD(Handling, from, vtf, cls, damage_scale, chassis, f1_aero, core_aero, wing_damage, suspension_damage,
              tyre_damage, body_damage, tyres, engine, gearbox, fuel_tank, fuel_ecu, brake_ecu, steering, differential,
              suspension, tuning, assists, turbo, ers, plank, rad_upgrades, edits, remove)

struct CarSelect {
    std::vector<uint32_t> indices;
    std::vector<std::string> vehicle_package, vehicle, team;
};
SWOFTY_RECORD(CarSelect, indices, vehicle_package, vehicle, team)

struct Profile {
    static constexpr Version version{1, 0};
    json::Value within;
    std::vector<events::SessionType> when;
    CarSelect cars;
    bool ai_only = false, player_only = false;
    int priority = 0;
    float power_scale = 1, limiter_offset_rpm = 0, lift_scale = 1, drag_scale = 1;
    float grip_scale = 1, ai_grip_scale = 1;
    std::optional<uint32_t> fuel_modes;
    json::Value extra;
};
SWOFTY_RECORD(Profile, within, when, cars, ai_only, player_only, priority, power_scale, limiter_offset_rpm, lift_scale,
              drag_scale, grip_scale, ai_grip_scale, fuel_modes)

struct Part {
    std::string node;
    float detach_at = 0;
    bool debris = true;
};
SWOFTY_RECORD(Part, node, detach_at, debris)

inline float front_weight(const Handling &h)
{
    float wheelbase = h.suspension.front.rest_z - h.suspension.rear.rest_z;
    return wheelbase > 0 ? (-h.chassis.com_z - h.suspension.rear.rest_z) / wheelbase : 0;
}

inline Registry<Vehicle> vehicles{"vehicle", {.applies = Applies::boot}};
inline Registry<Package> packages{"vehicle.package", {.applies = Applies::boot}};
inline Registry<Handling> handling{"vehicle.handling", {.applies = Applies::session}};
inline Registry<Profile> profiles{"car.profile", {.applies = Applies::read, .snapshot = true}};

inline Result reset_upgrades(int) { return Result::unavailable; }

namespace live {

// The fields set() can write, by part. Units and ranges are the handling file's; set() refuses a value
// outside the range with Result::invalid.
enum class Field : uint32_t {
    steering_lock = SWOFTY_CAR_STEERING_LOCK,                  // Steering/MaxSteeringLock, rad, 0.01 .. 1.5708
    clutch_torque = SWOFTY_CAR_CLUTCH_TORQUE,                  // GearBox/MaxClutchTorque, multiple of engine torque
    diff_on_throttle = SWOFTY_CAR_DIFF_ON_THROTTLE,            // setup differential on throttle, 0 .. 1
    diff_off_throttle = SWOFTY_CAR_DIFF_OFF_THROTTLE,          // setup differential off throttle, 0 .. 1
    turbo_max_boost = SWOFTY_CAR_TURBO_MAX_BOOST,              // Turbo/MaxBoost, bar
    fuel_mass = SWOFTY_CAR_FUEL_MASS,                          // kg in the tank, up to its capacity
    wing_speed_threshold = SWOFTY_CAR_WING_SPEED_THRESHOLD,    // F1WingDamage, read on every hit
    wing_damage_scale = SWOFTY_CAR_WING_DAMAGE_SCALE,
    wing_chassis_damage_multiple = SWOFTY_CAR_WING_CHASSIS_DAMAGE_MULTIPLE,
    wing_side_detach_scale = SWOFTY_CAR_WING_SIDE_DETACH_SCALE,
    wing_full_detach_scale = SWOFTY_CAR_WING_FULL_DETACH_SCALE,
    susp_speed_threshold = SWOFTY_CAR_SUSP_SPEED_THRESHOLD,    // F1SuspensionDamage, read on every hit
    susp_front_damage_scale = SWOFTY_CAR_SUSP_FRONT_DAMAGE_SCALE,
    susp_rear_damage_scale = SWOFTY_CAR_SUSP_REAR_DAMAGE_SCALE,
    susp_wheel_detach_scale = SWOFTY_CAR_SUSP_WHEEL_DETACH_SCALE,
    susp_parts_detach_scale = SWOFTY_CAR_SUSP_PARTS_DETACH_SCALE,
    susp_linear_fracture_limit = SWOFTY_CAR_SUSP_LINEAR_FRACTURE_LIMIT,
    susp_angular_fracture_limit = SWOFTY_CAR_SUSP_ANGULAR_FRACTURE_LIMIT,
    engine_limiter = SWOFTY_CAR_ENGINE_LIMITER,                // Engine/LimiterRotationRate (rad/s); the core re-runs the derived limiter
    final_drive = SWOFTY_CAR_FINAL_DRIVE,                      // GearBox/FinalDriveRatio; the core rebuilds the ratio table
    gear_ratio_1 = SWOFTY_CAR_GEAR_RATIO_1,                    // GearBox ratio of 1st .. 9th; the same rebuild
    gear_ratio_2 = SWOFTY_CAR_GEAR_RATIO_1 + 1,
    gear_ratio_3 = SWOFTY_CAR_GEAR_RATIO_1 + 2,
    gear_ratio_4 = SWOFTY_CAR_GEAR_RATIO_1 + 3,
    gear_ratio_5 = SWOFTY_CAR_GEAR_RATIO_1 + 4,
    gear_ratio_6 = SWOFTY_CAR_GEAR_RATIO_1 + 5,
    gear_ratio_7 = SWOFTY_CAR_GEAR_RATIO_1 + 6,
    gear_ratio_8 = SWOFTY_CAR_GEAR_RATIO_1 + 7,
    gear_ratio_9 = SWOFTY_CAR_GEAR_RATIO_1 + 8,
    aero_lift_scale = SWOFTY_CAR_AERO_LIFT_SCALE,              // F1Aero/lift_scale, multiplies the body lift; an R&D replay resets it to 1
    aero_drag_scale = SWOFTY_CAR_AERO_DRAG_SCALE,              // F1Aero/drag_scale, multiplies the body drag
    engine_power_scale = SWOFTY_CAR_ENGINE_POWER_SCALE,        // the core's handler on the engine drive torque; writes no game data
    engine_limiter_offset_rpm = SWOFTY_CAR_ENGINE_LIMITER_OFFSET_RPM, // negative: the torque is cut that many rpm under the limiter
    ai_grip_scale = SWOFTY_CAR_AI_GRIP_SCALE,                  // the AI's table grip input, dry weather only
    tyres_front_grip_scale = SWOFTY_CAR_TYRES_FRONT_GRIP_SCALE, // Suspension Tyre Front/GripMultiplier
    tyres_rear_grip_scale = SWOFTY_CAR_TYRES_REAR_GRIP_SCALE,
    chassis_ballast_kg = SWOFTY_CAR_CHASSIS_BALLAST_KG,        // kilograms added to the VTF mass, then the chassis reset
    brakes_bias = SWOFTY_CAR_BRAKES_BIAS,                      // BrakeECU bias, 0 .. 1
    brakes_pressure = SWOFTY_CAR_BRAKES_PRESSURE,
    susp_front_camber = SWOFTY_CAR_SUSP_FRONT_CAMBER,          // the axle's setup camber, toe, anti-roll bar and ride height offset
    susp_front_toe = SWOFTY_CAR_SUSP_FRONT_TOE,
    susp_front_anti_roll = SWOFTY_CAR_SUSP_FRONT_ANTI_ROLL,
    susp_front_ride_height_offset = SWOFTY_CAR_SUSP_FRONT_RIDE_HEIGHT_OFFSET,
    susp_rear_camber = SWOFTY_CAR_SUSP_REAR_CAMBER,
    susp_rear_toe = SWOFTY_CAR_SUSP_REAR_TOE,
    susp_rear_anti_roll = SWOFTY_CAR_SUSP_REAR_ANTI_ROLL,
    susp_rear_ride_height_offset = SWOFTY_CAR_SUSP_REAR_RIDE_HEIGHT_OFFSET,
    aero_slipstream_drag = SWOFTY_CAR_AERO_SLIPSTREAM_DRAG,
    aero_slipstream_front_wing = SWOFTY_CAR_AERO_SLIPSTREAM_FRONT_WING,
    aero_slipstream_rear_wing = SWOFTY_CAR_AERO_SLIPSTREAM_REAR_WING,
    aero_slipstream_lift = SWOFTY_CAR_AERO_SLIPSTREAM_LIFT,
    aero_slipstream_car_width = SWOFTY_CAR_AERO_SLIPSTREAM_CAR_WIDTH,
    ai_blocking_fraction_of_follow_speed = SWOFTY_CAR_AI_BLOCKING_FRACTION,   // layers only; per car, read on every AI tick
    ai_blocking_fraction_of_follow_speed_at_race_start = SWOFTY_CAR_AI_BLOCKING_FRACTION_RACE_START,
    ai_blocking_max_blocking_distance = SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE,
    ai_blocking_max_blocking_distance_at_race_start = SWOFTY_CAR_AI_BLOCKING_MAX_DISTANCE_RACE_START,
    ai_alongside_speed_difference_for_limited_zone_yielding = SWOFTY_CAR_AI_ALONGSIDE_SPEED_DIFFERENCE_FOR_LIMITED_ZONE_YIELDING,
    ai_alongside_yield_overtake_rate = SWOFTY_CAR_AI_ALONGSIDE_YIELD_OVERTAKE_RATE,
    ai_alongside_min_presence_within_buffer = SWOFTY_CAR_AI_ALONGSIDE_MIN_PRESENCE_WITHIN_BUFFER,
    ai_divebomb_defense_max_presence_alongside = SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_PRESENCE_ALONGSIDE,
    ai_divebomb_defense_max_time_to_ignore = SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TIME_TO_IGNORE,
    ai_divebomb_defense_max_track_offset = SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TRACK_OFFSET,
    ai_divebomb_defense_max_track_radius = SWOFTY_CAR_AI_DIVEBOMB_DEFENSE_MAX_TRACK_RADIUS,
    ai_offset_variation_max_offset = SWOFTY_CAR_AI_OFFSET_VARIATION_MAX_OFFSET,
    ai_offset_variation_offset_limit_at_corner_start = SWOFTY_CAR_AI_OFFSET_VARIATION_OFFSET_LIMIT_AT_CORNER_START,
    ai_offset_variation_offset_limit_blend_distance = SWOFTY_CAR_AI_OFFSET_VARIATION_OFFSET_LIMIT_BLEND_DISTANCE,
    ai_offset_variation_half_width_scaling_graph = SWOFTY_CAR_AI_OFFSET_VARIATION_HALF_WIDTH_SCALING_GRAPH,
    ai_offset_variation_radius_scalar_graph = SWOFTY_CAR_AI_OFFSET_VARIATION_RADIUS_SCALAR_GRAPH,
    ai_overtake_attacker_close_enough_car_lengths = SWOFTY_CAR_AI_OVERTAKE_ATTACKER_CLOSE_ENOUGH_CAR_LENGTHS,
    ai_overtake_braking_zone_abort_overtake_time = SWOFTY_CAR_AI_OVERTAKE_BRAKING_ZONE_ABORT_OVERTAKE_TIME,
    ai_overtake_close_range_car_lengths_to_continue_overtake = SWOFTY_CAR_AI_OVERTAKE_CLOSE_RANGE_CAR_LENGTHS_TO_CONTINUE_OVERTAKE,
    ai_overtake_close_range_car_lengths_to_start_overtake = SWOFTY_CAR_AI_OVERTAKE_CLOSE_RANGE_CAR_LENGTHS_TO_START_OVERTAKE,
    ai_overtake_oppertunist_overtake_time = SWOFTY_CAR_AI_OVERTAKE_OPPERTUNIST_OVERTAKE_TIME,
    ai_overtake_oppertunist_overtake_track_speed_difference = SWOFTY_CAR_AI_OVERTAKE_OPPERTUNIST_OVERTAKE_TRACK_SPEED_DIFFERENCE,
    ai_overtake_danger_overtake_continue_time_adjustment = SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_CONTINUE_TIME_ADJUSTMENT,
    ai_overtake_danger_overtake_full_heat_time_adjustment = SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_FULL_HEAT_TIME_ADJUSTMENT,
    ai_overtake_danger_overtake_initiate_time_adjustment = SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_INITIATE_TIME_ADJUSTMENT,
    ai_overtake_danger_overtake_pull_out_time = SWOFTY_CAR_AI_OVERTAKE_DANGER_OVERTAKE_PULL_OUT_TIME,
    ai_overtake_regular_overtake_continue_time_adjustment = SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_CONTINUE_TIME_ADJUSTMENT,
    ai_overtake_regular_overtake_full_heat_time_adjustment = SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_FULL_HEAT_TIME_ADJUSTMENT,
    ai_overtake_regular_overtake_initiate_time_adjustment = SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_INITIATE_TIME_ADJUSTMENT,
    ai_overtake_regular_overtake_pull_out_time = SWOFTY_CAR_AI_OVERTAKE_REGULAR_OVERTAKE_PULL_OUT_TIME,
    ai_overtake_overlap_percentage = SWOFTY_CAR_AI_OVERTAKE_OVERLAP_PERCENTAGE,
    ai_overtake_concede_speed_diff = SWOFTY_CAR_AI_OVERTAKE_CONCEDE_SPEED_DIFF,
    ai_overtake_concede_slowdown_speed_diff = SWOFTY_CAR_AI_OVERTAKE_CONCEDE_SLOWDOWN_SPEED_DIFF,
    ai_overtake_concede_time_vs_behind = SWOFTY_CAR_AI_OVERTAKE_CONCEDE_TIME_VS_BEHIND,
    ai_space_seeking_added_buffer = SWOFTY_CAR_AI_SPACE_SEEKING_ADDED_BUFFER,
    ai_space_seeking_speed_below_follow = SWOFTY_CAR_AI_SPACE_SEEKING_SPEED_BELOW_FOLLOW,
    ai_space_seeking_first_speed_below_follow = SWOFTY_CAR_AI_SPACE_SEEKING_FIRST_SPEED_BELOW_FOLLOW,
    ai_space_seeking_car_in_front_needed = SWOFTY_CAR_AI_SPACE_SEEKING_CAR_IN_FRONT_NEEDED,
    ai_space_seeking_remember_crowed = SWOFTY_CAR_AI_SPACE_SEEKING_REMEMBER_CROWED,
    ai_space_seeking_concede_overtake_blocked_by_crowed_time = SWOFTY_CAR_AI_SPACE_SEEKING_CONCEDE_OVERTAKE_BLOCKED_BY_CROWED_TIME,
    ai_space_seeking_total_queue_needed = SWOFTY_CAR_AI_SPACE_SEEKING_TOTAL_QUEUE_NEEDED,
    ai_space_seeking_distance_braking_point = SWOFTY_CAR_AI_SPACE_SEEKING_DISTANCE_BRAKING_POINT,
    ai_space_seeking_time_to_switch_sides = SWOFTY_CAR_AI_SPACE_SEEKING_TIME_TO_SWITCH_SIDES,
    ai_space_seeking_track_distance_needed = SWOFTY_CAR_AI_SPACE_SEEKING_TRACK_DISTANCE_NEEDED,
    ai_space_seeking_track_abort_distance = SWOFTY_CAR_AI_SPACE_SEEKING_TRACK_ABORT_DISTANCE,
    ai_space_seeking_cars_moving_away_speed_to_ignore = SWOFTY_CAR_AI_SPACE_SEEKING_CARS_MOVING_AWAY_SPEED_TO_IGNORE,
    ai_space_seeking_can_start_track_radius = SWOFTY_CAR_AI_SPACE_SEEKING_CAN_START_TRACK_RADIUS,
    ai_blocking_start_blocking_intersection_time = SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_INTERSECTION_TIME,
    ai_blocking_start_blocking_maximum_time_to_corner = SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_MAXIMUM_TIME_TO_CORNER,
    ai_blocking_start_blocking_minimum_time_to_corner = SWOFTY_CAR_AI_BLOCKING_START_BLOCKING_MINIMUM_TIME_TO_CORNER,
    ai_blocking_time_to_corner_for_end_merge_rate = SWOFTY_CAR_AI_BLOCKING_TIME_TO_CORNER_FOR_END_MERGE_RATE,
    ai_blocking_time_to_corner_for_start_merge_rate = SWOFTY_CAR_AI_BLOCKING_TIME_TO_CORNER_FOR_START_MERGE_RATE,
    ai_blocking_end_blocking_time_to_corner_vs_offset = SWOFTY_CAR_AI_BLOCKING_END_BLOCKING_TIME_TO_CORNER_VS_OFFSET,
    ai_blocking_blocking_offset_from_car_behind = SWOFTY_CAR_AI_BLOCKING_BLOCKING_OFFSET_FROM_CAR_BEHIND,
    ai_blocking_blocking_speed_differential = SWOFTY_CAR_AI_BLOCKING_BLOCKING_SPEED_DIFFERENTIAL,
    ai_blocking_blocking_threat_time_modifier = SWOFTY_CAR_AI_BLOCKING_BLOCKING_THREAT_TIME_MODIFIER,
    ai_blocking_blocking_variable_offset_fraction = SWOFTY_CAR_AI_BLOCKING_BLOCKING_VARIABLE_OFFSET_FRACTION,
    ai_blocking_avoidance_decrease_rate = SWOFTY_CAR_AI_BLOCKING_AVOIDANCE_DECREASE_RATE,
    ai_blocking_merge_back_to_racing_line = SWOFTY_CAR_AI_BLOCKING_MERGE_BACK_TO_RACING_LINE,
};

}

}

namespace swofty::detail {

inline std::atomic<const SwoftyCarOps *> &car_ops_override()
{
    static std::atomic<const SwoftyCarOps *> t{nullptr};
    return t;
}

inline void set_car_ops_table(const SwoftyCarOps *t) { car_ops_override() = t; }

inline const SwoftyCarOps *car_ops_table()
{
    if (auto t = car_ops_override().load())
        return t->version >= 1 ? t : nullptr;
#if defined(_WIN32)
    static std::atomic<const SwoftyCarOps *> found{nullptr};
    if (auto t = found.load())
        return t;
    HMODULE core = GetModuleHandleW(L"swofty_core.dll");
    auto get = core ? reinterpret_cast<SwoftyCoreCarOpsFn>(GetProcAddress(core, "SwoftyCoreCarOps")) : nullptr;
    const SwoftyCarOps *t = get ? get() : nullptr;
    if (t && t->version < 1)
        t = nullptr;
    found = t;
    return t;
#else
    return nullptr;
#endif
}

inline const SwoftyCarOps *live_car_ops()
{
    auto t = car_ops_table();
    return t && t->available && t->available() ? t : nullptr;
}

inline const SwoftyCarOps *live_car_ops_v2()
{
    auto t = live_car_ops();
    return t && t->version >= 2 && t->field && t->layer && t->unlayer ? t : nullptr;
}

}

namespace swofty::cars::live {

class Car;

enum class FuelMix : uint32_t { lean = SWOFTY_CAR_FUEL_MIX_LEAN, standard = SWOFTY_CAR_FUEL_MIX_STANDARD, rich = SWOFTY_CAR_FUEL_MIX_RICH, max = SWOFTY_CAR_FUEL_MIX_MAX };

// Offers the mixes in mask (FuelMix bits) on the MFD of every car while the mask stays set; 0 withdraws the owner's
// request. The mode tables themselves are fields: "fuel_ecu.fuel_burn_scale.rich", "fuel_ecu.fuel_power_scale.max",
// "fuel_ecu.fuel_revs_scale.lean", "fuel_ecu.temperature_scale.standard" (burn, power, revs, temperature x lean,
// standard, rich, max).
inline Result unlock_fuel_mixes(std::string_view owner, uint32_t mask)
{
    auto t = swofty::detail::live_car_ops();
    if (!t || t->version < 3 || !t->unlock_fuel_mixes)
        return Result::unavailable;
    return swofty::detail::result(t->unlock_fuel_mixes(std::string(owner).c_str(), mask));
}

inline std::optional<FuelMix> fuel_mix(uint32_t car_index)
{
    auto t = swofty::detail::live_car_ops();
    uint32_t mode = 0;
    if (!t || t->version < 3 || !t->fuel_mix || t->fuel_mix(car_index, &mode) != SWOFTY_ASSET_OK || mode > 3)
        return std::nullopt;
    return static_cast<FuelMix>(1u << mode);
}

inline bool set_fuel_mix_available()
{
    auto t = swofty::detail::live_car_ops();
    return t && t->version >= 4 && t->set_fuel_mix;
}

// Switches the car to a mix the MFD offers it, through the game's own mode switch. Result::invalid for a mix the car
// is not offered (see unlock_fuel_mixes), Result::late off the game's flow thread, Result::unavailable on a core
// older than car ops version 4.
inline Result set_fuel_mix(uint32_t car_index, FuelMix mix)
{
    auto t = swofty::detail::live_car_ops();
    if (!t || t->version < 4 || !t->set_fuel_mix)
        return Result::unavailable;
    const uint32_t bits = static_cast<uint32_t>(mix);
    if (bits == 0 || (bits & (bits - 1)) || bits > SWOFTY_CAR_FUEL_MIX_MAX)
        return Result::invalid;
    uint32_t mode = 0;
    while ((1u << mode) != bits)
        mode++;
    return swofty::detail::result(t->set_fuel_mix(car_index, mode));
}

inline std::optional<Field> field(std::string_view path)
{
    auto t = swofty::detail::live_car_ops_v2();
    uint32_t id = 0;
    if (!t || t->field(std::string(path).c_str(), &id) != SWOFTY_ASSET_OK)
        return std::nullopt;
    return static_cast<Field>(id);
}

class Car {
public:
    Car() = default;
    explicit Car(uint32_t index, void *handle = nullptr) : index_(index), handle_(handle) {}
    uint32_t index() const { return index_; }
    void *handle() const { return handle_; }
    Ref<Vehicle> vehicle() const { return {}; }
    Ref<teams::Identity> team() const { return {}; }
    Ref<drivers::Identity> driver() const { return {}; }
    // Whether the AI drives the car right now (the player's car too, while the AI has it); true when the
    // core can't tell.
    bool is_ai() const
    {
        auto t = swofty::detail::live_car_ops();
        void *h = nullptr;
        uint32_t ai = 1;
        if (!t || !t->car || t->car(index_, &h, &ai) != SWOFTY_ASSET_OK || (handle_ && h != handle_))
            return true;
        return ai == 1;
    }
    std::optional<sessions::ControlState> control() const { return swofty::detail::read_control(index_); }

    std::optional<float> get(Field f) const
    {
        float v = 0;
        return read(f, v) == Result::ok ? std::optional<float>(v) : std::nullopt;
    }
    Result set(Field f, float value) const
    {
        auto t = swofty::detail::live_car_ops();
        if (!t || !t->set)
            return Result::unavailable;
        return swofty::detail::result(t->set(index_, handle_, static_cast<uint32_t>(f), value));
    }
    Result scale(Field f, float factor) const
    {
        float v = 0;
        Result r = read(f, v);
        return r == Result::ok ? set(f, v * factor) : r;
    }

    std::optional<float> get(std::string_view path) const
    {
        auto f = field(path);
        return f ? get(*f) : std::nullopt;
    }
    Result set(std::string_view path, float value) const
    {
        auto f = field(path);
        return f ? set(*f, value) : Result::invalid;
    }
    Result scale(std::string_view path, float factor) const
    {
        auto f = field(path);
        return f ? scale(*f, factor) : Result::invalid;
    }

private:
    Result read(Field f, float &v) const
    {
        auto t = swofty::detail::live_car_ops();
        if (!t || !t->get)
            return Result::unavailable;
        return swofty::detail::result(t->get(index_, handle_, static_cast<uint32_t>(f), &v));
    }
    uint32_t index_ = 0;
    void *handle_ = nullptr;
};

inline bool available() { return swofty::detail::live_car_ops() != nullptr; }

inline std::vector<Car> cars()
{
    std::vector<Car> out;
    auto t = swofty::detail::live_car_ops();
    uint32_t n = 0;
    if (!t || !t->count || !t->car || t->count(&n) != SWOFTY_ASSET_OK)
        return out;
    for (uint32_t i = 0; i < n; i++) {
        void *h = nullptr;
        if (t->car(i, &h, nullptr) == SWOFTY_ASSET_OK)
            out.emplace_back(i, h);
    }
    return out;
}


enum class Op : uint32_t { set = SWOFTY_CAR_OP_SET, scale = SWOFTY_CAR_OP_SCALE, add = SWOFTY_CAR_OP_ADD };

// Which cars a layer reaches: every car (also the ones built later), or the cars that are on track when the
// selector is made.
class Cars {
public:
    static Cars all() { return Cars(true, {}); }
    static Cars car(const Car &c) { return Cars(false, {c.index()}); }
    static Cars ai();
    static Cars player();
    bool every() const { return all_; }
    const std::vector<uint32_t> &indices() const { return indices_; }

private:
    Cars(bool all, std::vector<uint32_t> indices) : all_(all), indices_(std::move(indices)) {}
    bool all_ = false;
    std::vector<uint32_t> indices_;
};

// A layer the core keeps and applies again after every car.built, car.reset, R&D upgrade replay and setup apply.
// It is removed when the Layer is destroyed. Layers of one field compose by priority (lower first), then by the
// order they were made. Experimental.
class Layer {
public:
    Layer() = default;
    Layer(const Layer &) = delete;
    Layer &operator=(const Layer &) = delete;
    Layer(Layer &&o) noexcept : tokens_(std::move(o.tokens_)), result_(o.result_)
    {
        o.tokens_.clear();
        o.result_ = Result::unavailable;
    }
    Layer &operator=(Layer &&o) noexcept
    {
        if (this != &o) {
            release();
            tokens_ = std::move(o.tokens_);
            result_ = o.result_;
            o.tokens_.clear();
            o.result_ = Result::unavailable;
        }
        return *this;
    }
    ~Layer() { release(); }

    static Layer set(const Cars &cars, std::string_view path, float value, int priority = 0)
    {
        return make(cars, path, Op::set, value, priority);
    }
    static Layer scale(const Cars &cars, std::string_view path, float factor, int priority = 0)
    {
        return make(cars, path, Op::scale, factor, priority);
    }
    static Layer add(const Cars &cars, std::string_view path, float delta, int priority = 0)
    {
        return make(cars, path, Op::add, delta, priority);
    }

    Result result() const { return result_; }
    bool ok() const { return result_ == Result::ok && !tokens_.empty(); }

    void release()
    {
        auto t = swofty::detail::live_car_ops_v2();
        if (t)
            for (uint32_t token : tokens_)
                t->unlayer(token);
        tokens_.clear();
    }

private:
    static Layer make(const Cars &cars, std::string_view path, Op op, float value, int priority)
    {
        Layer l;
        auto t = swofty::detail::live_car_ops_v2();
        auto f = field(path);
        if (!t || !f) {
            l.result_ = t ? Result::invalid : Result::unavailable;
            return l;
        }
        std::vector<uint32_t> targets = cars.every() ? std::vector<uint32_t>{SWOFTY_CAR_ALL} : cars.indices();
        for (uint32_t index : targets) {
            uint32_t token = 0;
            Result r = swofty::detail::result(t->layer("addon", index, static_cast<uint32_t>(*f),
                                                       static_cast<uint32_t>(op), value, priority, &token));
            if (r != Result::ok) {
                l.result_ = r;
                l.release();
                return l;
            }
            l.tokens_.push_back(token);
        }
        l.result_ = Result::ok;
        return l;
    }
    std::vector<uint32_t> tokens_;
    Result result_ = Result::unavailable;
};

inline Cars Cars::ai()
{
    std::vector<uint32_t> out;
    for (const Car &c : cars())
        if (c.is_ai())
            out.push_back(c.index());
    return Cars(false, std::move(out));
}

inline Cars Cars::player()
{
    std::vector<uint32_t> out;
    for (const Car &c : cars())
        if (!c.is_ai())
            out.push_back(c.index());
    return Cars(false, std::move(out));
}

}

namespace swofty::cars::events {

struct UpgradesFit {
    static constexpr const char *point = "car.upgrades.fit";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int32_t car;
    const char *vehicle_id;
    Department department;
    float driver_stat;
    float rating;

    Ref<Vehicle> vehicle() const { return swofty::detail::payload_ref<Vehicle>(vehicle_id); }
};

struct VtfRead {
    static constexpr const char *point = "car.vtf.read";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    live::Car car;
    const char *section_str;
    const char *field_str;
    const char *upgrade_str;
    float value;

    std::string_view section() const { return swofty::detail::payload_str(section_str); }
    std::string_view field() const { return swofty::detail::payload_str(field_str); }
    std::string_view upgrade() const { return swofty::detail::payload_str(upgrade_str); }
};

struct Built {
    static constexpr const char *point = "car.built";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    live::Car car;
};

struct Reset {
    static constexpr const char *point = "car.reset";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    live::Car car;
    bool upgrade;
};

struct UpgradesReplayed {
    static constexpr const char *point = "car.upgrades.replayed";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    live::Car car;
    float progress;
};

struct SetupApplied {
    static constexpr const char *point = "car.setup.applied";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    live::Car car;
};

struct Damage {
    static constexpr const char *point = "car.damage";
    static constexpr uint32_t version = 1;
    enum Part : uint32_t { front_wing_left = 0, front_wing_right = 1 };
    uint32_t size, payload_version;
    live::Car car;
    uint32_t part;
    float amount;
};

struct ControllersRebuilt {
    static constexpr const char *point = "car.controllers.rebuilt";
    static constexpr uint32_t version = 1;
    enum Reason : uint32_t { rebuild = 0, resettle = 1 };
    uint32_t size, payload_version;
    live::Car car;
    uint32_t reason;
};

struct PartsRegister {
    static constexpr const char *point = "car.parts.register";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int32_t car;
    const char *vehicle_id;
    void *front_wing;
    int32_t (*add_fn)(void *user, const char *node, float detach_at, bool debris);
    void *user;

    Ref<Vehicle> vehicle() const { return swofty::detail::payload_ref<Vehicle>(vehicle_id); }
    Result add(const Part &part) const
    {
        return add_fn ? swofty::detail::result(add_fn(user, part.node.c_str(), part.detach_at, part.debris))
                      : Result::unavailable;
    }
};

}
