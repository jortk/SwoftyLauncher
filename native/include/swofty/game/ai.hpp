#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct AiDriverConfig {
    void **alongside_off_target_angle_interpolation; // L2 experimental
    void **alongside_steering_understeer_offset; // L2 experimental
    void **alongside_steering_understeer_slip_max_throttle; // L2 experimental
    void **alongside_trailbrake_speed_reduction_scalar; // L2 experimental
    void **alongside_speed_difference_for_limited_zone_yielding; // L2 experimental
    void **alongside_yield_overtake_rate; // L2 experimental
    void **alongside_min_presence_within_buffer; // L2 experimental
    void **alongside_offset_limiter_to_vehicle_blend_graph; // L2 experimental
    void **blocking_start_blocking_intersection_time; // L2 experimental
    void **blocking_start_blocking_maximum_time_to_corner; // L2 experimental
    void **blocking_start_blocking_minimum_time_to_corner; // L2 experimental
    void **blocking_time_to_corner_for_end_merge_rate; // L2 experimental
    void **blocking_time_to_corner_for_start_merge_rate; // L2 experimental
    void **blocking_end_blocking_time_to_corner_vs_offset; // L2 experimental
    void **blocking_blocking_offset_from_car_behind; // L2 experimental
    void **blocking_blocking_speed_differential; // L2 experimental
    void **blocking_blocking_threat_time_modifier; // L2 experimental
    void **blocking_blocking_variable_offset_fraction; // L2 experimental
    void **blocking_fraction_of_follow_speed; // L2 experimental
    void **blocking_fraction_of_follow_speed_at_race_start; // L2 experimental
    void **blocking_max_blocking_distance; // L2 experimental
    void **blocking_max_blocking_distance_at_race_start; // L2 experimental
    void **blocking_avoidance_decrease_rate; // L2 experimental
    void **blocking_merge_back_to_racing_line; // L2 experimental
    void **cede_position_ceding_radius; // L2 experimental
    void **cede_position_ignore_ceding_over_speed; // L2 experimental
    void **cede_position_max_cede_time_ratio; // L2 experimental
    void **cede_position_min_cede_time_ratio; // L2 experimental
    void **cede_position_flying_lap_yield_to_human_track_speed_diff; // L2 experimental
    void **cede_position_yellow_flag_ramp_up_time; // L2 experimental
    void **divebomb_defense_max_presence_alongside; // L2 experimental
    void **divebomb_defense_max_time_to_ignore; // L2 experimental
    void **divebomb_defense_max_track_offset; // L2 experimental
    void **divebomb_defense_max_track_radius; // L2 experimental
    void **exit_fallibility_throttle_error_practice; // L2 experimental
    void **exit_fallibility_throttle_error_qualifying_race; // L2 experimental
    void **offset_variation_max_offset; // L2 experimental
    void **offset_variation_offset_limit_at_corner_start; // L2 experimental
    void **offset_variation_offset_limit_blend_distance; // L2 experimental
    void **offset_variation_blend_rate_inside_of_curve; // L2 experimental
    void **offset_variation_blend_rate_outside_of_curve; // L2 experimental
    void **offset_variation_half_width_scaling_graph; // L2 experimental
    void **offset_variation_radius_scalar_graph; // L2 experimental
    void **overtake_attacker_close_enough_car_lengths; // L2 experimental
    void **overtake_braking_zone_abort_overtake_time; // L2 experimental
    void **overtake_close_range_car_lengths_to_continue_overtake; // L2 experimental
    void **overtake_close_range_car_lengths_to_start_overtake; // L2 experimental
    void **overtake_oppertunist_overtake_time; // L2 experimental
    void **overtake_oppertunist_overtake_track_speed_difference; // L2 experimental
    void **overtake_danger_overtake_continue_time_adjustment; // L2 experimental
    void **overtake_danger_overtake_full_heat_time_adjustment; // L2 experimental
    void **overtake_danger_overtake_initiate_time_adjustment; // L2 experimental
    void **overtake_danger_overtake_pull_out_time; // L2 experimental
    void **overtake_danger_overtake_buffer; // L2 experimental
    void **overtake_regular_overtake_continue_time_adjustment; // L2 experimental
    void **overtake_regular_overtake_full_heat_time_adjustment; // L2 experimental
    void **overtake_regular_overtake_initiate_time_adjustment; // L2 experimental
    void **overtake_regular_overtake_pull_out_time; // L2 experimental
    void **overtake_overlap_percentage; // L2 experimental
    void **overtake_concede_speed_diff; // L2 experimental
    void **overtake_concede_slowdown_speed_diff; // L2 experimental
    void **overtake_concede_time_vs_behind; // L2 experimental
    void **race_start_max_reaction_time_lower_bound; // L2 experimental
    void **race_start_min_reaction_time_lower_bound; // L2 experimental
    void **race_start_max_reaction_time_upper_bound; // L2 experimental
    void **race_start_min_reaction_time_upper_bound; // L2 experimental
    void **race_start_max_bad_start_lower_bound; // L2 experimental
    void **race_start_min_bad_start_lower_bound; // L2 experimental
    void **race_start_max_bad_start_upper_bound; // L2 experimental
    void **race_start_min_bad_start_upper_bound; // L2 experimental
    void **race_start_stationary_vehicle_space; // L2 experimental
    void **space_seeking_added_buffer; // L2 experimental
    void **space_seeking_speed_below_follow; // L2 experimental
    void **space_seeking_first_speed_below_follow; // L2 experimental
    void **space_seeking_car_in_front_needed; // L2 experimental
    void **space_seeking_remember_crowed; // L2 experimental
    void **space_seeking_concede_overtake_blocked_by_crowed_time; // L2 experimental
    void **space_seeking_total_queue_needed; // L2 experimental
    void **space_seeking_distance_braking_point; // L2 experimental
    void **space_seeking_time_to_switch_sides; // L2 experimental
    void **space_seeking_track_distance_needed; // L2 experimental
    void **space_seeking_track_abort_distance; // L2 experimental
    void **space_seeking_cars_moving_away_speed_to_ignore; // L2 experimental
    void **space_seeking_can_start_track_radius; // L2 experimental
};
static_assert(offsetof(AiDriverConfig, alongside_off_target_angle_interpolation) == 0x0);
static_assert(offsetof(AiDriverConfig, alongside_steering_understeer_offset) == 0x8);
static_assert(offsetof(AiDriverConfig, alongside_steering_understeer_slip_max_throttle) == 0x10);
static_assert(offsetof(AiDriverConfig, alongside_trailbrake_speed_reduction_scalar) == 0x18);
static_assert(offsetof(AiDriverConfig, alongside_speed_difference_for_limited_zone_yielding) == 0x20);
static_assert(offsetof(AiDriverConfig, alongside_yield_overtake_rate) == 0x28);
static_assert(offsetof(AiDriverConfig, alongside_min_presence_within_buffer) == 0x30);
static_assert(offsetof(AiDriverConfig, alongside_offset_limiter_to_vehicle_blend_graph) == 0x38);
static_assert(offsetof(AiDriverConfig, blocking_start_blocking_intersection_time) == 0x40);
static_assert(offsetof(AiDriverConfig, blocking_start_blocking_maximum_time_to_corner) == 0x48);
static_assert(offsetof(AiDriverConfig, blocking_start_blocking_minimum_time_to_corner) == 0x50);
static_assert(offsetof(AiDriverConfig, blocking_time_to_corner_for_end_merge_rate) == 0x58);
static_assert(offsetof(AiDriverConfig, blocking_time_to_corner_for_start_merge_rate) == 0x60);
static_assert(offsetof(AiDriverConfig, blocking_end_blocking_time_to_corner_vs_offset) == 0x68);
static_assert(offsetof(AiDriverConfig, blocking_blocking_offset_from_car_behind) == 0x70);
static_assert(offsetof(AiDriverConfig, blocking_blocking_speed_differential) == 0x78);
static_assert(offsetof(AiDriverConfig, blocking_blocking_threat_time_modifier) == 0x80);
static_assert(offsetof(AiDriverConfig, blocking_blocking_variable_offset_fraction) == 0x88);
static_assert(offsetof(AiDriverConfig, blocking_fraction_of_follow_speed) == 0x90);
static_assert(offsetof(AiDriverConfig, blocking_fraction_of_follow_speed_at_race_start) == 0x98);
static_assert(offsetof(AiDriverConfig, blocking_max_blocking_distance) == 0xa0);
static_assert(offsetof(AiDriverConfig, blocking_max_blocking_distance_at_race_start) == 0xa8);
static_assert(offsetof(AiDriverConfig, blocking_avoidance_decrease_rate) == 0xb0);
static_assert(offsetof(AiDriverConfig, blocking_merge_back_to_racing_line) == 0xb8);
static_assert(offsetof(AiDriverConfig, cede_position_ceding_radius) == 0xc0);
static_assert(offsetof(AiDriverConfig, cede_position_ignore_ceding_over_speed) == 0xc8);
static_assert(offsetof(AiDriverConfig, cede_position_max_cede_time_ratio) == 0xd0);
static_assert(offsetof(AiDriverConfig, cede_position_min_cede_time_ratio) == 0xd8);
static_assert(offsetof(AiDriverConfig, cede_position_flying_lap_yield_to_human_track_speed_diff) == 0xe0);
static_assert(offsetof(AiDriverConfig, cede_position_yellow_flag_ramp_up_time) == 0xe8);
static_assert(offsetof(AiDriverConfig, divebomb_defense_max_presence_alongside) == 0xf0);
static_assert(offsetof(AiDriverConfig, divebomb_defense_max_time_to_ignore) == 0xf8);
static_assert(offsetof(AiDriverConfig, divebomb_defense_max_track_offset) == 0x100);
static_assert(offsetof(AiDriverConfig, divebomb_defense_max_track_radius) == 0x108);
static_assert(offsetof(AiDriverConfig, exit_fallibility_throttle_error_practice) == 0x110);
static_assert(offsetof(AiDriverConfig, exit_fallibility_throttle_error_qualifying_race) == 0x118);
static_assert(offsetof(AiDriverConfig, offset_variation_max_offset) == 0x120);
static_assert(offsetof(AiDriverConfig, offset_variation_offset_limit_at_corner_start) == 0x128);
static_assert(offsetof(AiDriverConfig, offset_variation_offset_limit_blend_distance) == 0x130);
static_assert(offsetof(AiDriverConfig, offset_variation_blend_rate_inside_of_curve) == 0x138);
static_assert(offsetof(AiDriverConfig, offset_variation_blend_rate_outside_of_curve) == 0x140);
static_assert(offsetof(AiDriverConfig, offset_variation_half_width_scaling_graph) == 0x148);
static_assert(offsetof(AiDriverConfig, offset_variation_radius_scalar_graph) == 0x150);
static_assert(offsetof(AiDriverConfig, overtake_attacker_close_enough_car_lengths) == 0x158);
static_assert(offsetof(AiDriverConfig, overtake_braking_zone_abort_overtake_time) == 0x160);
static_assert(offsetof(AiDriverConfig, overtake_close_range_car_lengths_to_continue_overtake) == 0x168);
static_assert(offsetof(AiDriverConfig, overtake_close_range_car_lengths_to_start_overtake) == 0x170);
static_assert(offsetof(AiDriverConfig, overtake_oppertunist_overtake_time) == 0x178);
static_assert(offsetof(AiDriverConfig, overtake_oppertunist_overtake_track_speed_difference) == 0x180);
static_assert(offsetof(AiDriverConfig, overtake_danger_overtake_continue_time_adjustment) == 0x188);
static_assert(offsetof(AiDriverConfig, overtake_danger_overtake_full_heat_time_adjustment) == 0x190);
static_assert(offsetof(AiDriverConfig, overtake_danger_overtake_initiate_time_adjustment) == 0x198);
static_assert(offsetof(AiDriverConfig, overtake_danger_overtake_pull_out_time) == 0x1a0);
static_assert(offsetof(AiDriverConfig, overtake_danger_overtake_buffer) == 0x1a8);
static_assert(offsetof(AiDriverConfig, overtake_regular_overtake_continue_time_adjustment) == 0x1b0);
static_assert(offsetof(AiDriverConfig, overtake_regular_overtake_full_heat_time_adjustment) == 0x1b8);
static_assert(offsetof(AiDriverConfig, overtake_regular_overtake_initiate_time_adjustment) == 0x1c0);
static_assert(offsetof(AiDriverConfig, overtake_regular_overtake_pull_out_time) == 0x1c8);
static_assert(offsetof(AiDriverConfig, overtake_overlap_percentage) == 0x1d0);
static_assert(offsetof(AiDriverConfig, overtake_concede_speed_diff) == 0x1d8);
static_assert(offsetof(AiDriverConfig, overtake_concede_slowdown_speed_diff) == 0x1e0);
static_assert(offsetof(AiDriverConfig, overtake_concede_time_vs_behind) == 0x1e8);
static_assert(offsetof(AiDriverConfig, race_start_max_reaction_time_lower_bound) == 0x1f0);
static_assert(offsetof(AiDriverConfig, race_start_min_reaction_time_lower_bound) == 0x1f8);
static_assert(offsetof(AiDriverConfig, race_start_max_reaction_time_upper_bound) == 0x200);
static_assert(offsetof(AiDriverConfig, race_start_min_reaction_time_upper_bound) == 0x208);
static_assert(offsetof(AiDriverConfig, race_start_max_bad_start_lower_bound) == 0x210);
static_assert(offsetof(AiDriverConfig, race_start_min_bad_start_lower_bound) == 0x218);
static_assert(offsetof(AiDriverConfig, race_start_max_bad_start_upper_bound) == 0x220);
static_assert(offsetof(AiDriverConfig, race_start_min_bad_start_upper_bound) == 0x228);
static_assert(offsetof(AiDriverConfig, race_start_stationary_vehicle_space) == 0x230);
static_assert(offsetof(AiDriverConfig, space_seeking_added_buffer) == 0x238);
static_assert(offsetof(AiDriverConfig, space_seeking_speed_below_follow) == 0x240);
static_assert(offsetof(AiDriverConfig, space_seeking_first_speed_below_follow) == 0x248);
static_assert(offsetof(AiDriverConfig, space_seeking_car_in_front_needed) == 0x250);
static_assert(offsetof(AiDriverConfig, space_seeking_remember_crowed) == 0x258);
static_assert(offsetof(AiDriverConfig, space_seeking_concede_overtake_blocked_by_crowed_time) == 0x260);
static_assert(offsetof(AiDriverConfig, space_seeking_total_queue_needed) == 0x268);
static_assert(offsetof(AiDriverConfig, space_seeking_distance_braking_point) == 0x270);
static_assert(offsetof(AiDriverConfig, space_seeking_time_to_switch_sides) == 0x278);
static_assert(offsetof(AiDriverConfig, space_seeking_track_distance_needed) == 0x280);
static_assert(offsetof(AiDriverConfig, space_seeking_track_abort_distance) == 0x288);
static_assert(offsetof(AiDriverConfig, space_seeking_cars_moving_away_speed_to_ignore) == 0x290);
static_assert(offsetof(AiDriverConfig, space_seeking_can_start_track_radius) == 0x298);

struct AiCarRecord {
    uint8_t vehicle_index; // L2 experimental
    uint8_t _pad_0x1[0x1f];
    void *driver_profile; // L2 experimental
    uint8_t _pad_0x28[0x30];
    void *speed_table_58; // L2 experimental
    void *speed_table_60; // L2 experimental
    void *radius_speed_table; // L2 experimental
    void *table_70; // L2 experimental
    uint8_t _pad_0x78[0x10];
    uint8_t table58_cursor[112]; // L2 experimental
    uint8_t table60_cursor[96]; // L2 experimental
    uint8_t radius_table_cursor[56]; // L2 experimental
    uint8_t _pad_0x190[0xa0];
    void *handling_config; // L2 experimental
    AiDriverConfig driver_config; // L2 experimental
};
static_assert(offsetof(AiCarRecord, vehicle_index) == 0x0);
static_assert(offsetof(AiCarRecord, driver_profile) == 0x20);
static_assert(offsetof(AiCarRecord, speed_table_58) == 0x58);
static_assert(offsetof(AiCarRecord, speed_table_60) == 0x60);
static_assert(offsetof(AiCarRecord, radius_speed_table) == 0x68);
static_assert(offsetof(AiCarRecord, table_70) == 0x70);
static_assert(offsetof(AiCarRecord, table58_cursor) == 0x88);
static_assert(offsetof(AiCarRecord, table60_cursor) == 0xf8);
static_assert(offsetof(AiCarRecord, radius_table_cursor) == 0x158);
static_assert(offsetof(AiCarRecord, handling_config) == 0x230);
static_assert(offsetof(AiCarRecord, driver_config) == 0x238);

struct AiHandlingConfig {
    void *allow_to_pass_begin; // L2 experimental
    uint8_t _pad_0x8[0x78];
    void *blocking_begin; // L2 experimental
    uint8_t _pad_0x88[0x20];
    void *buffer_space_begin; // L2 experimental
    uint8_t _pad_0xb0[0x10];
    void *buffer_space_track_edge_buffer; // L2 experimental
    void *buffer_space_vehicle_buffer; // L2 experimental
    void *celebration_begin; // L2 experimental
    uint8_t _pad_0xd8[0x10];
    void *doughnut_begin; // L2 experimental
    uint8_t _pad_0xf0[0x48];
    void *damage_settings_begin; // L2 experimental
    uint8_t _pad_0x140[0xe0];
    void *ers_use_begin; // L2 experimental
    uint8_t _pad_0x228[0xf0];
    float exit_fallibility_max_yaw_angle_rad; // L2 experimental
    uint8_t _pad_0x31c[0x9c];
    void *merge_steering_heatmap_begin; // L2 experimental
    uint8_t _pad_0x3c0[0x38];
    void *merge_vehicle_heatmap_begin; // L2 experimental
    uint8_t _pad_0x400[0x2b8];
    void *under_safety_car_delta_pace_high_speed_point; // L2 experimental
    void *under_safety_car_delta_pace_low_speed_point; // L2 experimental
    void *under_safety_car_delta_pace_time_adjustment; // L2 experimental
    void *under_safety_car_large_follow_gap; // L2 experimental
    void *under_safety_car_small_follow_gap; // L2 experimental
    uint8_t _pad_0x6e0[0x28];
    void *last_slot; // L2 experimental
};
static_assert(offsetof(AiHandlingConfig, allow_to_pass_begin) == 0x0);
static_assert(offsetof(AiHandlingConfig, blocking_begin) == 0x80);
static_assert(offsetof(AiHandlingConfig, buffer_space_begin) == 0xa8);
static_assert(offsetof(AiHandlingConfig, buffer_space_track_edge_buffer) == 0xc0);
static_assert(offsetof(AiHandlingConfig, buffer_space_vehicle_buffer) == 0xc8);
static_assert(offsetof(AiHandlingConfig, celebration_begin) == 0xd0);
static_assert(offsetof(AiHandlingConfig, doughnut_begin) == 0xe8);
static_assert(offsetof(AiHandlingConfig, damage_settings_begin) == 0x138);
static_assert(offsetof(AiHandlingConfig, ers_use_begin) == 0x220);
static_assert(offsetof(AiHandlingConfig, exit_fallibility_max_yaw_angle_rad) == 0x318);
static_assert(offsetof(AiHandlingConfig, merge_steering_heatmap_begin) == 0x3b8);
static_assert(offsetof(AiHandlingConfig, merge_vehicle_heatmap_begin) == 0x3f8);
static_assert(offsetof(AiHandlingConfig, under_safety_car_delta_pace_high_speed_point) == 0x6b8);
static_assert(offsetof(AiHandlingConfig, under_safety_car_delta_pace_low_speed_point) == 0x6c0);
static_assert(offsetof(AiHandlingConfig, under_safety_car_delta_pace_time_adjustment) == 0x6c8);
static_assert(offsetof(AiHandlingConfig, under_safety_car_large_follow_gap) == 0x6d0);
static_assert(offsetof(AiHandlingConfig, under_safety_car_small_follow_gap) == 0x6d8);
static_assert(offsetof(AiHandlingConfig, last_slot) == 0x708);
static_assert(sizeof(AiHandlingConfig) == 0x710);

struct AiSystem {
    uint8_t _pad_0x0[0xa8];
    AiCarRecord *records_begin; // L2 experimental
};
static_assert(offsetof(AiSystem, records_begin) == 0xa8);

using Ai_Apply_TetherSpeedCap_t = void(void *self, void *ai_car_record, void *ai_ctx);
inline constexpr fn<"Ai_Apply_TetherSpeedCap", Ai_Apply_TetherSpeedCap_t> Ai_Apply_TetherSpeedCap{}; // L2 experimental
inline constexpr address<"Ai_BehaviourRecover_Evaluate"> Ai_BehaviourRecover_Evaluate{}; // L2 experimental
inline constexpr address<"Ai_BehaviourTestRetire_Update"> Ai_BehaviourTestRetire_Update{}; // L2 experimental
using Ai_BehaviourTestSpaceSeeking_Ctor_t = void *(void *obj, void *alloc, bool flag, void *owner);
inline constexpr fn<"Ai_BehaviourTestSpaceSeeking_Ctor", Ai_BehaviourTestSpaceSeeking_Ctor_t> Ai_BehaviourTestSpaceSeeking_Ctor{}; // L2 experimental
using Ai_Build_BehaviourSystem_t = void(void *ai_ctl);
inline constexpr fn<"Ai_Build_BehaviourSystem", Ai_Build_BehaviourSystem_t> Ai_Build_BehaviourSystem{}; // L2 experimental
using Ai_Build_DriverProfiles_t = void(void *builder);
inline constexpr fn<"Ai_Build_DriverProfiles", Ai_Build_DriverProfiles_t> Ai_Build_DriverProfiles{}; // L2 experimental
using Ai_Build_SpeedProfileAhead_t = void(void *alloc, void *ai_car_record, float lookahead, int vehicle_index, bool flag, void *out_vec_float);
inline constexpr fn<"Ai_Build_SpeedProfileAhead", Ai_Build_SpeedProfileAhead_t> Ai_Build_SpeedProfileAhead{}; // L2 experimental
using Ai_Calc_GripLimitBlend_t = float(float x, float y, uint64_t vehicle_key, uint8_t flags, void *zone_a, float t_a, void *zone_b, float t_b);
inline constexpr fn<"Ai_Calc_GripLimitBlend", Ai_Calc_GripLimitBlend_t> Ai_Calc_GripLimitBlend{}; // L2 experimental
inline constexpr address<"Ai_Calc_GripLimitScaleA"> Ai_Calc_GripLimitScaleA{}; // L2 experimental
inline constexpr address<"Ai_Calc_GripLimitScaleB"> Ai_Calc_GripLimitScaleB{}; // L2 experimental
using Ai_Calc_SpeedForRadius_t = float(void *table, float radius, void *cursor);
inline constexpr fn<"Ai_Calc_SpeedForRadius", Ai_Calc_SpeedForRadius_t> Ai_Calc_SpeedForRadius{}; // L2 experimental
using Ai_CannedAccident_ApplyFailure_t = void(void *event);
inline constexpr fn<"Ai_CannedAccident_ApplyFailure", Ai_CannedAccident_ApplyFailure_t> Ai_CannedAccident_ApplyFailure{}; // L2 experimental
using Ai_CannedEvents_LoadPerRaceDnfs_t = void(void *cfg);
inline constexpr fn<"Ai_CannedEvents_LoadPerRaceDnfs", Ai_CannedEvents_LoadPerRaceDnfs_t> Ai_CannedEvents_LoadPerRaceDnfs{}; // L2 experimental
using Ai_CannedEvents_PressureLockUp_t = void(void *sys, void *evState, void *cfg, float dt);
inline constexpr fn<"Ai_CannedEvents_PressureLockUp", Ai_CannedEvents_PressureLockUp_t> Ai_CannedEvents_PressureLockUp{}; // L2 experimental
using Ai_CannedEvents_PushQueueMessage_t = uint32_t(void *queue, const void *msg);
inline constexpr fn<"Ai_CannedEvents_PushQueueMessage", Ai_CannedEvents_PushQueueMessage_t> Ai_CannedEvents_PushQueueMessage{}; // L2 experimental
using Ai_CannedEvents_RollFailureChance_t = void(void *sys, void *evState, void *cfg, float dt);
inline constexpr fn<"Ai_CannedEvents_RollFailureChance", Ai_CannedEvents_RollFailureChance_t> Ai_CannedEvents_RollFailureChance{}; // L2 experimental
using Ai_CannedEvents_SetObfLockupValue_t = void(void *evState, bool firstSlot, float value);
inline constexpr fn<"Ai_CannedEvents_SetObfLockupValue", Ai_CannedEvents_SetObfLockupValue_t> Ai_CannedEvents_SetObfLockupValue{}; // L2 experimental
using Ai_CannedEvents_Update_t = void(void *self, const uint64_t *time);
inline constexpr fn<"Ai_CannedEvents_Update", Ai_CannedEvents_Update_t> Ai_CannedEvents_Update{}; // L2 experimental
using Ai_CarProfile_Dtor_t = void(void *profile);
inline constexpr fn<"Ai_CarProfile_Dtor", Ai_CarProfile_Dtor_t> Ai_CarProfile_Dtor{}; // L2 experimental
using Ai_CarProfile_ResolveNames_t = void(void *profile, void *vehicle, void *vehicle_data);
inline constexpr fn<"Ai_CarProfile_ResolveNames", Ai_CarProfile_ResolveNames_t> Ai_CarProfile_ResolveNames{}; // L2 experimental
using Ai_Car_CaptureVehicleState_t = uint64_t(void *ai_input, void *car_entry, void *handling_config);
inline constexpr fn<"Ai_Car_CaptureVehicleState", Ai_Car_CaptureVehicleState_t> Ai_Car_CaptureVehicleState{}; // L2 experimental
using Ai_Car_ComputeLateralLimitMargins_t = void(void *lat_state, void *ai_car_record);
inline constexpr fn<"Ai_Car_ComputeLateralLimitMargins", Ai_Car_ComputeLateralLimitMargins_t> Ai_Car_ComputeLateralLimitMargins{}; // L2 experimental
using Ai_Car_UpdateAttitudeState_t = void(uint8_t *ctl, float dt, AiCarRecord *rec);
inline constexpr fn<"Ai_Car_UpdateAttitudeState", Ai_Car_UpdateAttitudeState_t> Ai_Car_UpdateAttitudeState{}; // L2 experimental
using Ai_Car_UpdateLateralLimitMargins_t = void(uint8_t *margins, float dt, void *ai_car_record);
inline constexpr fn<"Ai_Car_UpdateLateralLimitMargins", Ai_Car_UpdateLateralLimitMargins_t> Ai_Car_UpdateLateralLimitMargins{}; // L2 experimental
using Ai_Controller_UpdateStateAndDrive_t = void(void *ctl, float dt, void *ai_system);
inline constexpr fn<"Ai_Controller_UpdateStateAndDrive", Ai_Controller_UpdateStateAndDrive_t> Ai_Controller_UpdateStateAndDrive{}; // L2 experimental
using Ai_Create_VehicleController_t = void(void *ai_manager, uint32_t car_idx);
inline constexpr fn<"Ai_Create_VehicleController", Ai_Create_VehicleController_t> Ai_Create_VehicleController{}; // L2 experimental
using Ai_Drafting_Evaluate_t = bool(void *ai_sys, void **gap_rec, uint8_t *ai_car, uint8_t *car_ahead, bool use_end_time, int *out_reason);
inline constexpr fn<"Ai_Drafting_Evaluate", Ai_Drafting_Evaluate_t> Ai_Drafting_Evaluate{}; // L2 experimental
using Ai_DriverConfig_InitDefault_t = AiDriverConfig *(AiDriverConfig *cfg);
inline constexpr fn<"Ai_DriverConfig_InitDefault", Ai_DriverConfig_InitDefault_t> Ai_DriverConfig_InitDefault{}; // L2 experimental
using Ai_DriverProfiles_ComputeTraitNormalisers_t = void(void *builder);
inline constexpr fn<"Ai_DriverProfiles_ComputeTraitNormalisers", Ai_DriverProfiles_ComputeTraitNormalisers_t> Ai_DriverProfiles_ComputeTraitNormalisers{}; // L2 experimental
inline constexpr address<"Ai_DriverVariationInRaceSystem_Setup"> Ai_DriverVariationInRaceSystem_Setup{}; // L2 experimental
using Ai_DriverVariationInRace_Update_t = void(void *self, float progress);
inline constexpr fn<"Ai_DriverVariationInRace_Update", Ai_DriverVariationInRace_Update_t> Ai_DriverVariationInRace_Update{}; // L2 experimental
using Ai_DriverVariation_IsGraphAvailable_t = bool(void *sys, size_t graph_index, bool preferred_tracks);
inline constexpr fn<"Ai_DriverVariation_IsGraphAvailable", Ai_DriverVariation_IsGraphAvailable_t> Ai_DriverVariation_IsGraphAvailable{}; // L2 experimental
using Ai_ErsTable_SampleDelta_t = float(void *table, uint32_t group, uint32_t mode, float key, float f0, float f1);
inline constexpr fn<"Ai_ErsTable_SampleDelta", Ai_ErsTable_SampleDelta_t> Ai_ErsTable_SampleDelta{}; // L2 experimental
using Ai_ErsTable_SampleKeyBlended_t = float(void *ers_tables, uint32_t mode, uint32_t slot, float key, float t);
inline constexpr fn<"Ai_ErsTable_SampleKeyBlended", Ai_ErsTable_SampleKeyBlended_t> Ai_ErsTable_SampleKeyBlended{}; // L2 experimental
inline constexpr address<"Ai_Ers_DecidePushReasons"> Ai_Ers_DecidePushReasons{}; // L2 experimental
using Ai_Ers_GetCurveDelta_t = float(void *ers_tables, uint32_t mode, uint32_t slot, float key, float from, float to);
inline constexpr fn<"Ai_Ers_GetCurveDelta", Ai_Ers_GetCurveDelta_t> Ai_Ers_GetCurveDelta{}; // L2 experimental
inline constexpr address<"Ai_Eval_ClampedTestSpeed"> Ai_Eval_ClampedTestSpeed{}; // L2 experimental
inline constexpr address<"Ai_Eval_ClampedTestSpeed_2"> Ai_Eval_ClampedTestSpeed_2{}; // L2 experimental
using Ai_Eval_SlowStateFlag_t = void(void **ctl_sub, AiCarRecord *rec);
inline constexpr fn<"Ai_Eval_SlowStateFlag", Ai_Eval_SlowStateFlag_t> Ai_Eval_SlowStateFlag{}; // L2 experimental
using Ai_Gap_ComputeVehicleSpanLower_t = float(void *ctrl, int opp);
inline constexpr fn<"Ai_Gap_ComputeVehicleSpanLower", Ai_Gap_ComputeVehicleSpanLower_t> Ai_Gap_ComputeVehicleSpanLower{}; // L2 experimental
using Ai_Gap_ComputeVehicleSpanUpper_t = float(void *ai_car_record, int vehicle);
inline constexpr fn<"Ai_Gap_ComputeVehicleSpanUpper", Ai_Gap_ComputeVehicleSpanUpper_t> Ai_Gap_ComputeVehicleSpanUpper{}; // L2 experimental
using Ai_Get_GeneralSettings_t = void **();
inline constexpr fn<"Ai_Get_GeneralSettings", Ai_Get_GeneralSettings_t> Ai_Get_GeneralSettings{}; // L2 experimental
using Ai_Get_SegmentTurnRadius_t = float(void *ai, float dist, uint32_t node_idx, float lateral_offset, int route_idx);
inline constexpr fn<"Ai_Get_SegmentTurnRadius", Ai_Get_SegmentTurnRadius_t> Ai_Get_SegmentTurnRadius{}; // L2 experimental
using Ai_Get_TurnRadiusAtDistance_t = float(void *ai, float dist, float lateral_offset, int route_idx);
inline constexpr fn<"Ai_Get_TurnRadiusAtDistance", Ai_Get_TurnRadiusAtDistance_t> Ai_Get_TurnRadiusAtDistance{}; // L2 experimental
using Ai_Handle_PitStateChanged_Thunk_t = uint64_t(void *ai, void *msg);
inline constexpr fn<"Ai_Handle_PitStateChanged_Thunk", Ai_Handle_PitStateChanged_Thunk_t> Ai_Handle_PitStateChanged_Thunk{}; // L2 experimental
using Ai_HashMap_FindIndexU64_t = uint64_t(void *map, const uint64_t *key);
inline constexpr fn<"Ai_HashMap_FindIndexU64", Ai_HashMap_FindIndexU64_t> Ai_HashMap_FindIndexU64{}; // L2 experimental
using Ai_Init_TestSpeedLimits_t = void *(void *entry);
inline constexpr fn<"Ai_Init_TestSpeedLimits", Ai_Init_TestSpeedLimits_t> Ai_Init_TestSpeedLimits{}; // L2 experimental
using Ai_IntervalProfile_AddRamps_t = void(void *prof, void *other, float a, float b, float c, int id);
inline constexpr fn<"Ai_IntervalProfile_AddRamps", Ai_IntervalProfile_AddRamps_t> Ai_IntervalProfile_AddRamps{}; // L2 experimental
using Ai_Interval_AddFullSpanWeighted_t = void(void *set, float value, float limit, uint32_t tag);
inline constexpr fn<"Ai_Interval_AddFullSpanWeighted", Ai_Interval_AddFullSpanWeighted_t> Ai_Interval_AddFullSpanWeighted{}; // L2 experimental
using Ai_Interval_AddWeightedRange_t = void(void *set, float x1, float x2, float v, float vmax, uint32_t tag);
inline constexpr fn<"Ai_Interval_AddWeightedRange", Ai_Interval_AddWeightedRange_t> Ai_Interval_AddWeightedRange{}; // L2 experimental
using Ai_Interval_MergeInsert_t = void(const float *ival, void *vec);
inline constexpr fn<"Ai_Interval_MergeInsert", Ai_Interval_MergeInsert_t> Ai_Interval_MergeInsert{}; // L2 experimental
inline constexpr address<"Ai_Job_UpdateCarsParallel"> Ai_Job_UpdateCarsParallel{}; // L2 experimental
inline constexpr address<"Ai_LineCache_UpdateIfChanged"> Ai_LineCache_UpdateIfChanged{}; // L2 experimental
using Ai_Lookahead_AdvancePhase_t = void(void *unused, void *ctrl, int step, const float *in4, float *out4);
inline constexpr fn<"Ai_Lookahead_AdvancePhase", Ai_Lookahead_AdvancePhase_t> Ai_Lookahead_AdvancePhase{}; // L2 experimental
using Ai_Overtake_EvaluateGap_t = void(void *out, void *a2, void *a3, void *a4, int car);
inline constexpr fn<"Ai_Overtake_EvaluateGap", Ai_Overtake_EvaluateGap_t> Ai_Overtake_EvaluateGap{}; // L2 experimental
using Ai_Parse_BlockingConfig_t = void(void **slots, void *db_row);
inline constexpr fn<"Ai_Parse_BlockingConfig", Ai_Parse_BlockingConfig_t> Ai_Parse_BlockingConfig{}; // L2 experimental
inline constexpr address<"Ai_Parse_BrakeLineSettings"> Ai_Parse_BrakeLineSettings{}; // L2 experimental
using Ai_Parse_DriverConfig_t = void(AiDriverConfig *cfg, void *row);
inline constexpr fn<"Ai_Parse_DriverConfig", Ai_Parse_DriverConfig_t> Ai_Parse_DriverConfig{}; // L2 experimental
using Ai_Parse_OvertakeConfig_t = void(void **slots, void *driver_row);
inline constexpr fn<"Ai_Parse_OvertakeConfig", Ai_Parse_OvertakeConfig_t> Ai_Parse_OvertakeConfig{}; // L2 experimental
using Ai_Parse_RaceStartConfig_t = void(void **slots, void *driver_row);
inline constexpr fn<"Ai_Parse_RaceStartConfig", Ai_Parse_RaceStartConfig_t> Ai_Parse_RaceStartConfig{}; // L2 experimental
inline constexpr address<"Ai_RaceStart_ReactionTime"> Ai_RaceStart_ReactionTime{}; // L2 experimental
using Ai_Read_AlongsideConfig_t = void(AiDriverConfig *cfg, void *row_key);
inline constexpr fn<"Ai_Read_AlongsideConfig", Ai_Read_AlongsideConfig_t> Ai_Read_AlongsideConfig{}; // L2 experimental
using Ai_Resolve_HashedSettingBlocks_t = void(void **out16);
inline constexpr fn<"Ai_Resolve_HashedSettingBlocks", Ai_Resolve_HashedSettingBlocks_t> Ai_Resolve_HashedSettingBlocks{}; // L2 experimental
inline constexpr address<"Ai_Select_ErsDeployMode"> Ai_Select_ErsDeployMode{}; // L2 experimental
using Ai_SpeedPlan_EvalAtDistance_t = void(void *plan, float *out8, float dist, AiSystem *sys);
inline constexpr fn<"Ai_SpeedPlan_EvalAtDistance", Ai_SpeedPlan_EvalAtDistance_t> Ai_SpeedPlan_EvalAtDistance{}; // L2 experimental
inline constexpr address<"Ai_SplineCursor_Ctor"> Ai_SplineCursor_Ctor{}; // L2 experimental
using Ai_StateMachine_SetStateById_t = void(void *fsm, uint32_t state_id);
inline constexpr fn<"Ai_StateMachine_SetStateById", Ai_StateMachine_SetStateById_t> Ai_StateMachine_SetStateById{}; // L2 experimental
using Ai_Steering_UpdateLateralPid_t = void(uint8_t *ctl, float dt, void *ai_car);
inline constexpr fn<"Ai_Steering_UpdateLateralPid", Ai_Steering_UpdateLateralPid_t> Ai_Steering_UpdateLateralPid{}; // L2 experimental
using Ai_Steering_UpdateYawRateCorrection_t = void(uint8_t *ctl, void *ai_car);
inline constexpr fn<"Ai_Steering_UpdateYawRateCorrection", Ai_Steering_UpdateYawRateCorrection_t> Ai_Steering_UpdateYawRateCorrection{}; // L2 experimental
using Ai_SuggestedGear_ComputeForZone_t = void(void *sys, void *ctx, uint8_t *state);
inline constexpr fn<"Ai_SuggestedGear_ComputeForZone", Ai_SuggestedGear_ComputeForZone_t> Ai_SuggestedGear_ComputeForZone{}; // L2 experimental
using Ai_TableCursor_Update2D_t = void(void *table, const float *query, float *cursor);
inline constexpr fn<"Ai_TableCursor_Update2D", Ai_TableCursor_Update2D_t> Ai_TableCursor_Update2D{}; // L2 experimental
using Ai_TableCursor_Update2DKeyedA_t = void(void *table, const float *query, float *cursor);
inline constexpr fn<"Ai_TableCursor_Update2DKeyedA", Ai_TableCursor_Update2DKeyedA_t> Ai_TableCursor_Update2DKeyedA{}; // L2 experimental
using Ai_TableCursor_Update2DKeyedB_t = void(void *table, const float *query, float *cursor);
inline constexpr fn<"Ai_TableCursor_Update2DKeyedB", Ai_TableCursor_Update2DKeyedB_t> Ai_TableCursor_Update2DKeyedB{}; // L2 experimental
using Ai_TableCursor_Update3DKeyed_t = void(void *table, const int *query, int *cursor);
inline constexpr fn<"Ai_TableCursor_Update3DKeyed", Ai_TableCursor_Update3DKeyed_t> Ai_TableCursor_Update3DKeyed{}; // L2 experimental
using Ai_Table_CalcSpeedChangeDistance_t = float(float v0, float v1, float max_dist, void *table_a, void *table_b, void *cursor_a, void *cursor_b);
inline constexpr fn<"Ai_Table_CalcSpeedChangeDistance", Ai_Table_CalcSpeedChangeDistance_t> Ai_Table_CalcSpeedChangeDistance{}; // L2 experimental
inline constexpr address<"Ai_Table_InterpolateA"> Ai_Table_InterpolateA{}; // L2 experimental
inline constexpr address<"Ai_Table_InterpolateB"> Ai_Table_InterpolateB{}; // L2 experimental
inline constexpr address<"Ai_Table_InterpolateC"> Ai_Table_InterpolateC{}; // L2 experimental
inline constexpr address<"Ai_Table_InterpolateD"> Ai_Table_InterpolateD{}; // L2 experimental
using Ai_Table_SolveAxis0ForValue_t = float(void *table, float a, float b, float target, int64_t column);
inline constexpr fn<"Ai_Table_SolveAxis0ForValue", Ai_Table_SolveAxis0ForValue_t> Ai_Table_SolveAxis0ForValue{}; // L2 experimental
using Ai_Table_UpdateKeyCache_t = void(void *table, const float *key, float *cache);
inline constexpr fn<"Ai_Table_UpdateKeyCache", Ai_Table_UpdateKeyCache_t> Ai_Table_UpdateKeyCache{}; // L2 experimental
inline constexpr address<"Ai_TestGraph_EvalAtSpeed"> Ai_TestGraph_EvalAtSpeed{}; // L2 experimental
inline constexpr address<"Ai_TestGraph_EvalDistanceSpeedA"> Ai_TestGraph_EvalDistanceSpeedA{}; // L2 experimental
inline constexpr address<"Ai_TestGraph_EvalDistanceSpeedB"> Ai_TestGraph_EvalDistanceSpeedB{}; // L2 experimental
using Ai_TestGraph_EvalDistanceSpeedC_t = float(void *table, float dist, float speed, void *cursor);
inline constexpr fn<"Ai_TestGraph_EvalDistanceSpeedC", Ai_TestGraph_EvalDistanceSpeedC_t> Ai_TestGraph_EvalDistanceSpeedC{}; // L2 experimental
using Ai_TestGraph_EvalTimeSpeed_t = float(void *table, float time, float speed, float *cursor);
inline constexpr fn<"Ai_TestGraph_EvalTimeSpeed", Ai_TestGraph_EvalTimeSpeed_t> Ai_TestGraph_EvalTimeSpeed{}; // L2 experimental
using Ai_TrackLine_FindCornerEndAndStart_t = int(void *result, float car_dist, float query_dist, void *spline_data, int route_idx);
inline constexpr fn<"Ai_TrackLine_FindCornerEndAndStart", Ai_TrackLine_FindCornerEndAndStart_t> Ai_TrackLine_FindCornerEndAndStart{}; // L2 experimental
using Ai_TrackLine_FindNextSideMarker_t = void(void *result, float car_dist, float query_dist, void *spline_data, int route_idx);
inline constexpr fn<"Ai_TrackLine_FindNextSideMarker", Ai_TrackLine_FindNextSideMarker_t> Ai_TrackLine_FindNextSideMarker{}; // L2 experimental
using Ai_TrackLine_FindSideMarkerInCorner_t = void(void *result, float car_dist, float query_dist, void *spline_data, int route_idx);
inline constexpr fn<"Ai_TrackLine_FindSideMarkerInCorner", Ai_TrackLine_FindSideMarkerInCorner_t> Ai_TrackLine_FindSideMarkerInCorner{}; // L2 experimental
inline constexpr address<"Ai_Update_CompoundPaceCurves"> Ai_Update_CompoundPaceCurves{}; // L2 experimental
using Ai_Update_DriverTick_t = void(void *ai_driver, float dt, void *ai_ctx);
inline constexpr fn<"Ai_Update_DriverTick", Ai_Update_DriverTick_t> Ai_Update_DriverTick{}; // L2 experimental
using Ai_Update_VehicleDriver_t = void(uint8_t *ctl, void *ctx);
inline constexpr fn<"Ai_Update_VehicleDriver", Ai_Update_VehicleDriver_t> Ai_Update_VehicleDriver{}; // L2 experimental
using Ai_Write_AdaptiveAiSessionRecord_t = void(void *rec, void *json_writer);
inline constexpr fn<"Ai_Write_AdaptiveAiSessionRecord", Ai_Write_AdaptiveAiSessionRecord_t> Ai_Write_AdaptiveAiSessionRecord{}; // L2 experimental
using Ai_ZoneMap_Find_t = void *(void *zone, uint64_t vehicle_key, uint8_t flags);
inline constexpr fn<"Ai_ZoneMap_Find", Ai_ZoneMap_Find_t> Ai_ZoneMap_Find{}; // L2 experimental
inline constexpr site<"site_Ai_BuildSystem_RecordPushed"> site_Ai_BuildSystem_RecordPushed{}; // L2 experimental
inline constexpr site<"site_Ai_ScFollowFlagCmp"> site_Ai_ScFollowFlagCmp{}; // L2 experimental
inline constexpr site<"site_Ai_TableGripInput"> site_Ai_TableGripInput{}; // L2 experimental
inline constexpr site<"site_Ai_UpdateCars_CornerLineLoad"> site_Ai_UpdateCars_CornerLineLoad{}; // L2 experimental
inline constexpr site<"site_Ai_UpdateCars_NoCornerLine"> site_Ai_UpdateCars_NoCornerLine{}; // L2 experimental

}
