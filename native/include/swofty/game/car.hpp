#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct CarBrakeEcu {
    uint8_t _pad_0x0[0x21d8];
    float brake_bias; // L3 stable
    float brake_pressure; // L3 stable
};
static_assert(offsetof(CarBrakeEcu, brake_bias) == 0x21d8);
static_assert(offsetof(CarBrakeEcu, brake_pressure) == 0x21dc);

struct CarChassis {
    uint8_t _pad_0x0[0x118];
    uint8_t tyre_physics_group[40]; // L2 experimental
    uint8_t _pad_0x140[0x10];
    float mass; // L3 stable
};
static_assert(offsetof(CarChassis, tyre_physics_group) == 0x118);
static_assert(offsetof(CarChassis, mass) == 0x150);

struct CarDamageFrontWing {
    uint8_t _pad_0x0[0x28];
    float speed_threshold; // L2 experimental
    float damage_scale; // L2 experimental
    uint8_t _pad_0x30[0x4];
    float side_detach_scale; // L2 experimental
    float full_detach_scale; // L2 experimental
    uint8_t _pad_0x3c[0x6c];
    void *parts_allocator; // L2 experimental
    void **parts_begin; // L2 experimental
    void **parts_end; // L2 experimental
    uint64_t parts_capacity; // L2 experimental
};
static_assert(offsetof(CarDamageFrontWing, speed_threshold) == 0x28);
static_assert(offsetof(CarDamageFrontWing, damage_scale) == 0x2c);
static_assert(offsetof(CarDamageFrontWing, side_detach_scale) == 0x34);
static_assert(offsetof(CarDamageFrontWing, full_detach_scale) == 0x38);
static_assert(offsetof(CarDamageFrontWing, parts_allocator) == 0xa8);
static_assert(offsetof(CarDamageFrontWing, parts_begin) == 0xb0);
static_assert(offsetof(CarDamageFrontWing, parts_end) == 0xb8);
static_assert(offsetof(CarDamageFrontWing, parts_capacity) == 0xc0);

struct CarDamageSuspension {
    uint8_t _pad_0x0[0x28];
    uint32_t speed_threshold_obf; // L2 experimental
    uint8_t _pad_0x2c[0xc];
    uint32_t front_damage_scale_obf; // L2 experimental
    uint8_t _pad_0x3c[0xc];
    uint32_t rear_damage_scale_obf; // L2 experimental
    uint8_t _pad_0x4c[0xc];
    uint32_t wheel_detach_scale_obf; // L2 experimental
    uint8_t _pad_0x5c[0xc];
    uint32_t parts_detach_scale_obf; // L2 experimental
    uint8_t _pad_0x6c[0xc];
    float linear_fracture_limit; // L2 experimental
    float angular_fracture_limit; // L2 experimental
};
static_assert(offsetof(CarDamageSuspension, speed_threshold_obf) == 0x28);
static_assert(offsetof(CarDamageSuspension, front_damage_scale_obf) == 0x38);
static_assert(offsetof(CarDamageSuspension, rear_damage_scale_obf) == 0x48);
static_assert(offsetof(CarDamageSuspension, wheel_detach_scale_obf) == 0x58);
static_assert(offsetof(CarDamageSuspension, parts_detach_scale_obf) == 0x68);
static_assert(offsetof(CarDamageSuspension, linear_fracture_limit) == 0x78);
static_assert(offsetof(CarDamageSuspension, angular_fracture_limit) == 0x7c);

struct CarDifferential {
    uint8_t _pad_0x0[0x8];
    CarLive *car; // L2 experimental
    uint8_t _pad_0x10[0xdc];
    float diff_off_throttle; // L2 experimental
    float diff_on_throttle; // L2 experimental
};
static_assert(offsetof(CarDifferential, car) == 0x8);
static_assert(offsetof(CarDifferential, diff_off_throttle) == 0xec);
static_assert(offsetof(CarDifferential, diff_on_throttle) == 0xf0);

struct CarObfFloat {
    uint32_t a; // L2 experimental
    uint8_t _pad_0x4[0x4];
    uint32_t b; // L2 experimental
};
static_assert(offsetof(CarObfFloat, a) == 0x0);
static_assert(offsetof(CarObfFloat, b) == 0x8);

struct CarEngine {
    uint8_t _pad_0x0[0x8];
    CarLive *car; // L2 experimental
    uint8_t _pad_0x10[0x8a0];
    CarObfFloat vtf_limiter_rotation_rate; // L2 experimental
    uint8_t _pad_0x8bc[0x1c5c];
    CarObfFloat max_torque; // L3 stable
    uint8_t _pad_0x2524[0x4];
    float limiter_rotation_rate; // L3 stable
    uint8_t _pad_0x252c[0xc];
    float rotation_rate; // L3 stable
    uint8_t _pad_0x253c[0x38];
    float throttle; // L3 stable
};
static_assert(offsetof(CarEngine, car) == 0x8);
static_assert(offsetof(CarEngine, vtf_limiter_rotation_rate) == 0x8b0);
static_assert(offsetof(CarEngine, max_torque) == 0x2518);
static_assert(offsetof(CarEngine, limiter_rotation_rate) == 0x2528);
static_assert(offsetof(CarEngine, rotation_rate) == 0x2538);
static_assert(offsetof(CarEngine, throttle) == 0x2574);

struct CarErsMode {
};

struct CarSpline {
};

struct CarErs {
    void *vtable; // L2 experimental
    CarLive *car; // L2 experimental
    void *allocator; // L2 experimental
    void *data_vtable; // L2 experimental
    uint8_t _pad_0x20[0x8];
    float max_deploy_power; // L2 experimental
    float max_derate_power; // L2 experimental
    float boost_mode_power_add_limit; // L2 experimental
    float min_initial_deploy_power; // L2 experimental
    float power_reduction_rate_limit; // L2 experimental
    float power_reduction_speed_min; // L2 experimental
    float full_throttle_time; // L2 experimental
    float off_throttle_time; // L2 experimental
    float off_throttle_timer_torque; // L2 experimental
    float harvest_torque_factor; // L2 experimental
    float harvest_limit_mguk; // L2 experimental
    float harvest_override_bonus_mguk; // L2 experimental
    float harvest_limit_qualifying_mguk; // L2 experimental
    float deploy_limit_mguk; // L2 experimental
    float energy_store_capacity; // L2 experimental
    float energy_deploy_efficiency_mguk; // L2 experimental
    float energy_harvest_efficiency_mguk; // L2 experimental
    float energy_harvest_efficiency_mguh; // L2 experimental
    float max_brake_torque_mguk; // L2 experimental
    float exhaust_power_coefficient; // L2 experimental
    float turbo_power_coefficient; // L2 experimental
    float overtake_power_efficiency; // L2 experimental
    uint8_t use_derate_lights; // L2 experimental
    uint8_t override_mode_available; // L2 experimental
    uint8_t allow_full_throttle_power_increase; // L2 experimental
    uint8_t _pad_0x83[0xaa5];
    float base_wear_rate_es; // L2 experimental
    uint8_t _pad_0xb2c[0x7fc];
    float base_wear_rate_mguk; // L2 experimental
    uint8_t _pad_0x132c[0x654];
    float medium_deploy_factor; // L2 experimental
    float hotlap_deploy_factor; // L2 experimental
    float harvest_factor_mguh; // L2 experimental
    float medium_derate_factor; // L2 experimental
    float hotlap_derate_factor; // L2 experimental
    float override_detection_lap_progress; // L2 experimental
    float override_activation_lap_progress; // L2 experimental
    uint8_t _pad_0x199c[0x4];
    uint8_t track_settings_found; // L2 experimental
    uint8_t _pad_0x19a1[0x27];
    float harvest_lap_mguk; // L2 experimental
    float harvest_lap_mguh; // L2 experimental
    float deploy_lap; // L2 experimental
    uint8_t _pad_0x19d4[0x1c];
    float energy; // L3 experimental
    float lap_harvest_limit; // L2 experimental
    uint8_t _pad_0x19f8[0x8];
    float harvest_power_mguk; // L2 experimental
    float harvest_power_mguh; // L2 experimental
    uint8_t _pad_0x1a08[0xc];
    float deploy_power; // L2 experimental
    uint8_t _pad_0x1a18[0x50];
    int32_t ers_mode; // L3 experimental
};
static_assert(offsetof(CarErs, vtable) == 0x0);
static_assert(offsetof(CarErs, car) == 0x8);
static_assert(offsetof(CarErs, allocator) == 0x10);
static_assert(offsetof(CarErs, data_vtable) == 0x18);
static_assert(offsetof(CarErs, max_deploy_power) == 0x28);
static_assert(offsetof(CarErs, max_derate_power) == 0x2c);
static_assert(offsetof(CarErs, boost_mode_power_add_limit) == 0x30);
static_assert(offsetof(CarErs, min_initial_deploy_power) == 0x34);
static_assert(offsetof(CarErs, power_reduction_rate_limit) == 0x38);
static_assert(offsetof(CarErs, power_reduction_speed_min) == 0x3c);
static_assert(offsetof(CarErs, full_throttle_time) == 0x40);
static_assert(offsetof(CarErs, off_throttle_time) == 0x44);
static_assert(offsetof(CarErs, off_throttle_timer_torque) == 0x48);
static_assert(offsetof(CarErs, harvest_torque_factor) == 0x4c);
static_assert(offsetof(CarErs, harvest_limit_mguk) == 0x50);
static_assert(offsetof(CarErs, harvest_override_bonus_mguk) == 0x54);
static_assert(offsetof(CarErs, harvest_limit_qualifying_mguk) == 0x58);
static_assert(offsetof(CarErs, deploy_limit_mguk) == 0x5c);
static_assert(offsetof(CarErs, energy_store_capacity) == 0x60);
static_assert(offsetof(CarErs, energy_deploy_efficiency_mguk) == 0x64);
static_assert(offsetof(CarErs, energy_harvest_efficiency_mguk) == 0x68);
static_assert(offsetof(CarErs, energy_harvest_efficiency_mguh) == 0x6c);
static_assert(offsetof(CarErs, max_brake_torque_mguk) == 0x70);
static_assert(offsetof(CarErs, exhaust_power_coefficient) == 0x74);
static_assert(offsetof(CarErs, turbo_power_coefficient) == 0x78);
static_assert(offsetof(CarErs, overtake_power_efficiency) == 0x7c);
static_assert(offsetof(CarErs, use_derate_lights) == 0x80);
static_assert(offsetof(CarErs, override_mode_available) == 0x81);
static_assert(offsetof(CarErs, allow_full_throttle_power_increase) == 0x82);
static_assert(offsetof(CarErs, base_wear_rate_es) == 0xb28);
static_assert(offsetof(CarErs, base_wear_rate_mguk) == 0x1328);
static_assert(offsetof(CarErs, medium_deploy_factor) == 0x1980);
static_assert(offsetof(CarErs, hotlap_deploy_factor) == 0x1984);
static_assert(offsetof(CarErs, harvest_factor_mguh) == 0x1988);
static_assert(offsetof(CarErs, medium_derate_factor) == 0x198c);
static_assert(offsetof(CarErs, hotlap_derate_factor) == 0x1990);
static_assert(offsetof(CarErs, override_detection_lap_progress) == 0x1994);
static_assert(offsetof(CarErs, override_activation_lap_progress) == 0x1998);
static_assert(offsetof(CarErs, track_settings_found) == 0x19a0);
static_assert(offsetof(CarErs, harvest_lap_mguk) == 0x19c8);
static_assert(offsetof(CarErs, harvest_lap_mguh) == 0x19cc);
static_assert(offsetof(CarErs, deploy_lap) == 0x19d0);
static_assert(offsetof(CarErs, energy) == 0x19f0);
static_assert(offsetof(CarErs, lap_harvest_limit) == 0x19f4);
static_assert(offsetof(CarErs, harvest_power_mguk) == 0x1a00);
static_assert(offsetof(CarErs, harvest_power_mguh) == 0x1a04);
static_assert(offsetof(CarErs, deploy_power) == 0x1a14);
static_assert(offsetof(CarErs, ers_mode) == 0x1a68);

struct CarF1Aero {
    uint8_t _pad_0x0[0x1988];
    uint8_t drs_flags; // L2 experimental
    uint8_t _pad_0x1989[0x17];
    float drag_scale; // L3 experimental
    float lift_scale; // L3 experimental
};
static_assert(offsetof(CarF1Aero, drs_flags) == 0x1988);
static_assert(offsetof(CarF1Aero, drag_scale) == 0x19a0);
static_assert(offsetof(CarF1Aero, lift_scale) == 0x19a4);

struct CarFuelEcu {
    uint8_t _pad_0x0[0x28];
    CarObfFloat fuel_burn_scale_lean; // L2 experimental
    uint8_t _pad_0x34[0x4];
    CarObfFloat fuel_burn_scale_standard; // L2 experimental
    uint8_t _pad_0x44[0x4];
    CarObfFloat fuel_burn_scale_rich; // L2 experimental
    uint8_t _pad_0x54[0x4];
    CarObfFloat fuel_burn_scale_max; // L2 experimental
    uint8_t _pad_0x64[0x4];
    CarObfFloat fuel_power_scale_lean; // L2 experimental
    uint8_t _pad_0x74[0x4];
    CarObfFloat fuel_power_scale_standard; // L2 experimental
    uint8_t _pad_0x84[0x4];
    CarObfFloat fuel_power_scale_rich; // L2 experimental
    uint8_t _pad_0x94[0x4];
    CarObfFloat fuel_power_scale_max; // L2 experimental
    uint8_t _pad_0xa4[0x4];
    CarObfFloat fuel_revs_scale_lean; // L2 experimental
    uint8_t _pad_0xb4[0x4];
    CarObfFloat fuel_revs_scale_standard; // L2 experimental
    uint8_t _pad_0xc4[0x4];
    CarObfFloat fuel_revs_scale_rich; // L2 experimental
    uint8_t _pad_0xd4[0x4];
    CarObfFloat fuel_revs_scale_max; // L2 experimental
    uint8_t _pad_0xe4[0x4];
    CarObfFloat temperature_scale_lean; // L2 experimental
    uint8_t _pad_0xf4[0x4];
    CarObfFloat temperature_scale_standard; // L2 experimental
    uint8_t _pad_0x104[0x4];
    CarObfFloat temperature_scale_rich; // L2 experimental
    uint8_t _pad_0x114[0x4];
    CarObfFloat temperature_scale_max; // L2 experimental
    uint8_t _pad_0x124[0x4];
    int32_t fuel_mode; // L3 stable
};
static_assert(offsetof(CarFuelEcu, fuel_burn_scale_lean) == 0x28);
static_assert(offsetof(CarFuelEcu, fuel_burn_scale_standard) == 0x38);
static_assert(offsetof(CarFuelEcu, fuel_burn_scale_rich) == 0x48);
static_assert(offsetof(CarFuelEcu, fuel_burn_scale_max) == 0x58);
static_assert(offsetof(CarFuelEcu, fuel_power_scale_lean) == 0x68);
static_assert(offsetof(CarFuelEcu, fuel_power_scale_standard) == 0x78);
static_assert(offsetof(CarFuelEcu, fuel_power_scale_rich) == 0x88);
static_assert(offsetof(CarFuelEcu, fuel_power_scale_max) == 0x98);
static_assert(offsetof(CarFuelEcu, fuel_revs_scale_lean) == 0xa8);
static_assert(offsetof(CarFuelEcu, fuel_revs_scale_standard) == 0xb8);
static_assert(offsetof(CarFuelEcu, fuel_revs_scale_rich) == 0xc8);
static_assert(offsetof(CarFuelEcu, fuel_revs_scale_max) == 0xd8);
static_assert(offsetof(CarFuelEcu, temperature_scale_lean) == 0xe8);
static_assert(offsetof(CarFuelEcu, temperature_scale_standard) == 0xf8);
static_assert(offsetof(CarFuelEcu, temperature_scale_rich) == 0x108);
static_assert(offsetof(CarFuelEcu, temperature_scale_max) == 0x118);
static_assert(offsetof(CarFuelEcu, fuel_mode) == 0x128);

struct CarFuelTank {
    uint8_t _pad_0x0[0x8];
    CarLive *car; // L2 experimental
    uint8_t _pad_0x10[0x88];
    CarObfFloat fuel_capacity; // L3 stable
    uint8_t _pad_0xa4[0x5c];
    float fuel_mass; // L3 stable
    uint8_t _pad_0x104[0x4];
    float fuel_flow; // L3 stable
};
static_assert(offsetof(CarFuelTank, car) == 0x8);
static_assert(offsetof(CarFuelTank, fuel_capacity) == 0x98);
static_assert(offsetof(CarFuelTank, fuel_mass) == 0x100);
static_assert(offsetof(CarFuelTank, fuel_flow) == 0x108);

struct CarGearBox {
    uint8_t _pad_0x0[0x8];
    CarLive *car; // L2 experimental
    uint8_t _pad_0x10[0x18];
    CarObfFloat final_drive_ratio; // L2 experimental
    uint8_t _pad_0x34[0x4];
    CarObfFloat gear_ratio_reverse; // L2 experimental
    uint8_t _pad_0x44[0x14];
    CarObfFloat gear_ratio1st; // L2 experimental
    uint8_t _pad_0x64[0x4];
    CarObfFloat gear_ratio2nd; // L2 experimental
    uint8_t _pad_0x74[0x4];
    CarObfFloat gear_ratio3rd; // L2 experimental
    uint8_t _pad_0x84[0x4];
    CarObfFloat gear_ratio4th; // L2 experimental
    uint8_t _pad_0x94[0x4];
    CarObfFloat gear_ratio5th; // L2 experimental
    uint8_t _pad_0xa4[0x4];
    CarObfFloat gear_ratio6th; // L2 experimental
    uint8_t _pad_0xb4[0x4];
    CarObfFloat gear_ratio7th; // L2 experimental
    uint8_t _pad_0xc4[0x4];
    CarObfFloat gear_ratio8th; // L2 experimental
    uint8_t _pad_0xd4[0x4];
    CarObfFloat gear_ratio9th; // L2 experimental
    uint8_t _pad_0xe4[0x4];
    float max_clutch_torque; // L2 experimental
    uint8_t _pad_0xec[0x454];
    uint8_t use_setup_ratios; // L2 experimental
    uint8_t _pad_0x541[0xb7];
    int32_t top_gear_index; // L2 experimental
    float inv_total_ratio[11]; // L2 experimental
    uint8_t _pad_0x628[0x58];
    uint32_t gear_index; // L3 stable
    uint32_t target_gear_index; // L2 experimental
};
static_assert(offsetof(CarGearBox, car) == 0x8);
static_assert(offsetof(CarGearBox, final_drive_ratio) == 0x28);
static_assert(offsetof(CarGearBox, gear_ratio_reverse) == 0x38);
static_assert(offsetof(CarGearBox, gear_ratio1st) == 0x58);
static_assert(offsetof(CarGearBox, gear_ratio2nd) == 0x68);
static_assert(offsetof(CarGearBox, gear_ratio3rd) == 0x78);
static_assert(offsetof(CarGearBox, gear_ratio4th) == 0x88);
static_assert(offsetof(CarGearBox, gear_ratio5th) == 0x98);
static_assert(offsetof(CarGearBox, gear_ratio6th) == 0xa8);
static_assert(offsetof(CarGearBox, gear_ratio7th) == 0xb8);
static_assert(offsetof(CarGearBox, gear_ratio8th) == 0xc8);
static_assert(offsetof(CarGearBox, gear_ratio9th) == 0xd8);
static_assert(offsetof(CarGearBox, max_clutch_torque) == 0xe8);
static_assert(offsetof(CarGearBox, use_setup_ratios) == 0x540);
static_assert(offsetof(CarGearBox, top_gear_index) == 0x5f8);
static_assert(offsetof(CarGearBox, inv_total_ratio) == 0x5fc);
static_assert(offsetof(CarGearBox, gear_index) == 0x680);
static_assert(offsetof(CarGearBox, target_gear_index) == 0x684);

struct CarLive {
    char *teleport_tag; // L2 experimental
    int32_t teleport_priority; // L2 experimental
    uint8_t _pad_0xc[0x4];
    float teleport_prev_mtx[16]; // L2 experimental
    float teleport_new_mtx[16]; // L2 experimental
    float teleport_dt; // L2 experimental
    uint32_t teleport_flags; // L2 experimental
    uint8_t _pad_0x98[0x50];
    uint8_t vehicle_id; // L3 stable
    uint8_t _pad_0xe9[0x2af];
    uint32_t system_count; // L3 stable
    uint8_t _pad_0x39c[0x104];
    void *upgrades_allocator; // L2 experimental
    uint8_t _pad_0x4a8[0x88];
    void *engine; // L3 stable
    void *gearbox; // L3 stable
    void *fuel_tank; // L3 stable
    void *fuel_ecu; // L3 stable
    void *brake_ecu; // L3 stable
    void *chassis; // L3 stable
    void *steering; // L3 stable
    void *core_aero; // L3 stable
    void *f1_aero; // L3 stable
    void *differential; // L3 stable
    void *suspension; // L3 stable
    uint8_t _pad_0x588[0x8];
    void *f1_tuning; // L3 stable
    void *f1_assists; // L3 stable
    void *turbo; // L3 stable
    void *ers; // L3 stable
    uint8_t _pad_0x5b0[0x18];
    void *f1_wing_damage; // L2 experimental
    void *f1_suspension_damage; // L2 experimental
    uint8_t _pad_0x5d8[0x9968];
    uint32_t handling_version; // L3 stable
};
static_assert(offsetof(CarLive, teleport_tag) == 0x0);
static_assert(offsetof(CarLive, teleport_priority) == 0x8);
static_assert(offsetof(CarLive, teleport_prev_mtx) == 0x10);
static_assert(offsetof(CarLive, teleport_new_mtx) == 0x50);
static_assert(offsetof(CarLive, teleport_dt) == 0x90);
static_assert(offsetof(CarLive, teleport_flags) == 0x94);
static_assert(offsetof(CarLive, vehicle_id) == 0xe8);
static_assert(offsetof(CarLive, system_count) == 0x398);
static_assert(offsetof(CarLive, upgrades_allocator) == 0x4a0);
static_assert(offsetof(CarLive, engine) == 0x530);
static_assert(offsetof(CarLive, gearbox) == 0x538);
static_assert(offsetof(CarLive, fuel_tank) == 0x540);
static_assert(offsetof(CarLive, fuel_ecu) == 0x548);
static_assert(offsetof(CarLive, brake_ecu) == 0x550);
static_assert(offsetof(CarLive, chassis) == 0x558);
static_assert(offsetof(CarLive, steering) == 0x560);
static_assert(offsetof(CarLive, core_aero) == 0x568);
static_assert(offsetof(CarLive, f1_aero) == 0x570);
static_assert(offsetof(CarLive, differential) == 0x578);
static_assert(offsetof(CarLive, suspension) == 0x580);
static_assert(offsetof(CarLive, f1_tuning) == 0x590);
static_assert(offsetof(CarLive, f1_assists) == 0x598);
static_assert(offsetof(CarLive, turbo) == 0x5a0);
static_assert(offsetof(CarLive, ers) == 0x5a8);
static_assert(offsetof(CarLive, f1_wing_damage) == 0x5c8);
static_assert(offsetof(CarLive, f1_suspension_damage) == 0x5d0);
static_assert(offsetof(CarLive, handling_version) == 0x9f40);

struct CarSteering {
    uint8_t _pad_0x0[0x8];
    CarLive *car; // L2 experimental
    uint8_t _pad_0x10[0x18];
    CarObfFloat max_steering_lock; // L2 experimental
};
static_assert(offsetof(CarSteering, car) == 0x8);
static_assert(offsetof(CarSteering, max_steering_lock) == 0x28);

struct CarSuspAxle {
    uint8_t _pad_0x0[0x15e4];
    float ride_height_offset; // L2 experimental
    uint8_t _pad_0x15e8[0x68];
    float setup_camber; // L2 experimental
    float setup_toe; // L2 experimental
    uint8_t _pad_0x1658[0xc];
    float setup_anti_roll; // L2 experimental
};
static_assert(offsetof(CarSuspAxle, ride_height_offset) == 0x15e4);
static_assert(offsetof(CarSuspAxle, setup_camber) == 0x1650);
static_assert(offsetof(CarSuspAxle, setup_toe) == 0x1654);
static_assert(offsetof(CarSuspAxle, setup_anti_roll) == 0x1664);

struct CarTurbo {
    uint8_t _pad_0x0[0x8];
    CarLive *car; // L2 experimental
    uint8_t _pad_0x10[0x2c8];
    float max_boost; // L2 experimental
};
static_assert(offsetof(CarTurbo, car) == 0x8);
static_assert(offsetof(CarTurbo, max_boost) == 0x2d8);

inline constexpr address<"Car_Assets_CreateSystemForPerformanceMode"> Car_Assets_CreateSystemForPerformanceMode{}; // L3 stable
inline constexpr address<"Car_Build_VtfComponents"> Car_Build_VtfComponents{}; // L2 experimental
inline constexpr address<"Car_Chassis_Build"> Car_Chassis_Build{}; // L3 stable
using Car_Create_DetachablePart_t = void *(void *part_manager, void **cb, uint32_t *name_id, int32_t *params, float k);
inline constexpr fn<"Car_Create_DetachablePart", Car_Create_DetachablePart_t> Car_Create_DetachablePart{}; // L2 experimental
inline constexpr address<"Car_Deformation_AcquireParticles"> Car_Deformation_AcquireParticles{}; // L2 experimental
inline constexpr address<"Car_Deformation_ClearPool"> Car_Deformation_ClearPool{}; // L2 experimental
inline constexpr address<"Car_Deformation_CreateBody"> Car_Deformation_CreateBody{}; // L2 experimental
inline constexpr address<"Car_Deformation_DestroyBody"> Car_Deformation_DestroyBody{}; // L2 experimental
inline constexpr address<"Car_Deformation_ReleaseParticles"> Car_Deformation_ReleaseParticles{}; // L2 experimental
inline constexpr address<"Car_Deformation_ReservePool"> Car_Deformation_ReservePool{}; // L2 experimental
using Car_DetachablePart_Update_t = bool(void *part, float dt);
inline constexpr fn<"Car_DetachablePart_Update", Car_DetachablePart_Update_t> Car_DetachablePart_Update{}; // L2 experimental
inline constexpr address<"Car_Engine_ComputeDerived_Thunk"> Car_Engine_ComputeDerived_Thunk{}; // L2 experimental
using Car_FrontWing_Init_t = void(void *front_wing, char reset);
inline constexpr fn<"Car_FrontWing_Init", Car_FrontWing_Init_t> Car_FrontWing_Init{}; // L2 experimental, game::FrontWing
inline constexpr address<"Car_FrontWing_OnCollision"> Car_FrontWing_OnCollision{}; // L2 experimental
inline constexpr address<"Car_FuelEcu_SetMode_Thunk"> Car_FuelEcu_SetMode_Thunk{}; // L2 experimental
inline constexpr address<"Car_Load_Vtf"> Car_Load_Vtf{}; // L2 experimental
inline constexpr address<"Car_Model_BindCockpitNodes"> Car_Model_BindCockpitNodes{}; // L2 experimental
inline constexpr address<"Car_Physics_PostVehicleUpdate"> Car_Physics_PostVehicleUpdate{}; // L2 experimental
using Car_PitStop_ApplyTyreAndWingVisuals_t = void(void *car_visual, float a2, bool a3);
inline constexpr fn<"Car_PitStop_ApplyTyreAndWingVisuals", Car_PitStop_ApplyTyreAndWingVisuals_t> Car_PitStop_ApplyTyreAndWingVisuals{}; // L2 experimental
inline constexpr address<"Car_Powertrain_Step"> Car_Powertrain_Step{}; // L2 experimental
inline constexpr address<"Car_Read_Engine"> Car_Read_Engine{}; // L2 experimental
inline constexpr address<"Car_Read_F1Aero"> Car_Read_F1Aero{}; // L2 experimental
inline constexpr address<"Car_Read_F1Assists"> Car_Read_F1Assists{}; // L2 experimental
inline constexpr address<"Car_Read_F1Tuning"> Car_Read_F1Tuning{}; // L2 experimental
inline constexpr address<"Car_Read_GearBox"> Car_Read_GearBox{}; // L2 experimental
using Car_Register_DetachablePart_Thunk_t = void(void *front_wing, void **cb, uint32_t *name_id, void *params, float k);
inline constexpr fn<"Car_Register_DetachablePart_Thunk", Car_Register_DetachablePart_Thunk_t> Car_Register_DetachablePart_Thunk{}; // L2 experimental
using Car_Reset_LastFittedUpgrade_t = void(void *inst);
inline constexpr fn<"Car_Reset_LastFittedUpgrade", Car_Reset_LastFittedUpgrade_t> Car_Reset_LastFittedUpgrade{}; // L2 experimental
inline constexpr address<"Car_Setup_ApplyTuning"> Car_Setup_ApplyTuning{}; // L3 stable
inline constexpr address<"Car_SteeringWheelLeds_Update"> Car_SteeringWheelLeds_Update{}; // L2 experimental
using Car_SuspensionDamage_OnCollision_t = void(void *susp_damage, void *contact);
inline constexpr fn<"Car_SuspensionDamage_OnCollision", Car_SuspensionDamage_OnCollision_t> Car_SuspensionDamage_OnCollision{}; // L2 experimental
inline constexpr address<"Car_Systems_FindOrCreate"> Car_Systems_FindOrCreate{}; // L2 experimental
inline constexpr address<"Car_Systems_Reset_Thunk"> Car_Systems_Reset_Thunk{}; // L3 stable
inline constexpr address<"Car_Tuning_SetChassisZCom"> Car_Tuning_SetChassisZCom{}; // L2 experimental
inline constexpr address<"Car_Tuning_SetEngineBrakingAndRadiator"> Car_Tuning_SetEngineBrakingAndRadiator{}; // L2 experimental
inline constexpr address<"Car_Tuning_SetFinalDrive"> Car_Tuning_SetFinalDrive{}; // L2 experimental
inline constexpr address<"Car_TyreSystem_PitstopComplete"> Car_TyreSystem_PitstopComplete{}; // L2 experimental
using Car_Tyre_CheckCompoundClassRule_t = bool(const void *set);
inline constexpr fn<"Car_Tyre_CheckCompoundClassRule", Car_Tyre_CheckCompoundClassRule_t> Car_Tyre_CheckCompoundClassRule{}; // L2 experimental
inline constexpr address<"Car_Tyre_GripScale"> Car_Tyre_GripScale{}; // L2 experimental
using Car_Tyre_IsCompoundAllowed_t = bool(const void *set);
inline constexpr fn<"Car_Tyre_IsCompoundAllowed", Car_Tyre_IsCompoundAllowed_t> Car_Tyre_IsCompoundAllowed{}; // L2 experimental
inline constexpr address<"Car_Tyre_MagicFormula"> Car_Tyre_MagicFormula{}; // L2 experimental
using Car_Tyre_SetWearFactor_t = void(void *tyre_component, bool apply_set, uint16_t force);
inline constexpr fn<"Car_Tyre_SetWearFactor", Car_Tyre_SetWearFactor_t> Car_Tyre_SetWearFactor{}; // L2 experimental
inline constexpr address<"Car_Tyre_UpdateWear"> Car_Tyre_UpdateWear{}; // L2 experimental
inline constexpr address<"Car_TyresPitstopComplete_ForCarPair"> Car_TyresPitstopComplete_ForCarPair{}; // L2 experimental
inline constexpr address<"Car_Tyres_ApplyStateToCarPair"> Car_Tyres_ApplyStateToCarPair{}; // L2 experimental
inline constexpr address<"Car_Tyres_CompletePitstopForCarList"> Car_Tyres_CompletePitstopForCarList{}; // L2 experimental
inline constexpr address<"Car_Upgrades_ApplyToVehicle"> Car_Upgrades_ApplyToVehicle{}; // L3 experimental
inline constexpr address<"Car_Upgrades_CalcNonLinearMass"> Car_Upgrades_CalcNonLinearMass{}; // L2 experimental
inline constexpr address<"Car_Upgrades_CalcTypeMass"> Car_Upgrades_CalcTypeMass{}; // L2 experimental
inline constexpr address<"Car_Write_Differential"> Car_Write_Differential{}; // L2 experimental
inline constexpr address<"Car_Write_EnergyRecovery"> Car_Write_EnergyRecovery{}; // L2 experimental
inline constexpr address<"Car_Write_Engine"> Car_Write_Engine{}; // L2 experimental
inline constexpr address<"Car_Write_FuelTank"> Car_Write_FuelTank{}; // L2 experimental
inline constexpr address<"Car_Write_GearBox"> Car_Write_GearBox{}; // L2 experimental
inline constexpr address<"Car_Write_Turbo"> Car_Write_Turbo{}; // L2 experimental
inline constexpr site<"site_Car_AeroStoreRearDownforce"> site_Car_AeroStoreRearDownforce{}; // L2 experimental
inline constexpr site<"site_Car_AeroStoreTotalDrag"> site_Car_AeroStoreTotalDrag{}; // L2 experimental
inline constexpr site<"site_Car_AeroStoreTotalLift"> site_Car_AeroStoreTotalLift{}; // L2 experimental
inline constexpr site<"site_Car_ChassisRegistryCount"> site_Car_ChassisRegistryCount{}; // L2 experimental
inline constexpr site<"site_Car_EngineDriveTorque"> site_Car_EngineDriveTorque{}; // L2 experimental
inline constexpr site<"site_Car_ErsGearModifierLoad"> site_Car_ErsGearModifierLoad{}; // L2 experimental
inline constexpr site<"site_Car_FuelFlowStore"> site_Car_FuelFlowStore{}; // L2 experimental
inline constexpr site<"site_Car_FuelMassStore"> site_Car_FuelMassStore{}; // L2 experimental
inline constexpr site<"site_Car_LiveCarBuilt"> site_Car_LiveCarBuilt{}; // L3 stable
inline constexpr site<"site_Car_RegisterVehicleId"> site_Car_RegisterVehicleId{}; // L3 experimental
inline constexpr site<"site_Car_SetupTuningApplied"> site_Car_SetupTuningApplied{}; // L3 stable
inline constexpr site<"site_Car_UpgradeSectionApplyCall"> site_Car_UpgradeSectionApplyCall{}; // L2 experimental
inline constexpr site<"site_Car_VehicleTableRemove"> site_Car_VehicleTableRemove{}; // L3 stable
inline constexpr vtable<"vt_Car_SystemsContainer", 1> vt_Car_SystemsContainer{}; // L2 experimental
inline constexpr vtable<"vt_Car_VehicleEnvironment", 7> vt_Car_VehicleEnvironment{}; // L2 experimental

}
