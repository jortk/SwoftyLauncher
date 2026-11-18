#pragma once
#include <swofty/game/base.hpp>
#include <swofty/game/game.hpp>

namespace swofty::game {

struct AiController {
    uint8_t _pad_0x0[0x8];
    struct InputRecord *input; // L2 experimental
    void *owner; // L2 experimental
    uint8_t vehicle; // L2 experimental
};
static_assert(offsetof(AiController, input) == 0x8);
static_assert(offsetof(AiController, owner) == 0x10);
static_assert(offsetof(AiController, vehicle) == 0x18);

struct ControllerSet {
    uint8_t _pad_0x0[0x10];
    uint32_t mode_masked; // L2 experimental
    uint8_t _pad_0x14[0x4];
    uint32_t mode_key; // L2 experimental
    uint8_t _pad_0x1c[0x4];
    void *human_controller; // L2 experimental
    void *ai_controller; // L2 experimental
    uint32_t flags; // L2 experimental
    uint8_t vehicle; // L2 experimental
};
static_assert(offsetof(ControllerSet, mode_masked) == 0x10);
static_assert(offsetof(ControllerSet, mode_key) == 0x18);
static_assert(offsetof(ControllerSet, human_controller) == 0x20);
static_assert(offsetof(ControllerSet, ai_controller) == 0x28);
static_assert(offsetof(ControllerSet, flags) == 0x30);
static_assert(offsetof(ControllerSet, vehicle) == 0x34);

struct EntrantSetupMode {
    uint8_t _pad_0x0[0x230];
    uint64_t selectable_list_count; // L2 experimental
};
static_assert(offsetof(EntrantSetupMode, selectable_list_count) == 0x230);

struct EntrantsManager {
    uint8_t _pad_0x0[0x2b0];
    GameSetupMode *game_setup_mode; // L2 experimental
    uint8_t _pad_0x2b8[0x28];
    EntrantRecord *session_begin; // L2 experimental
    EntrantRecord *session_end; // L2 experimental
};
static_assert(offsetof(EntrantsManager, game_setup_mode) == 0x2b0);
static_assert(offsetof(EntrantsManager, session_begin) == 0x2e0);
static_assert(offsetof(EntrantsManager, session_end) == 0x2e8);

struct GameSetupMode {
    uint8_t _pad_0x0[0x1e0];
    void *drs_allowed_lap_group; // L2 experimental
};
static_assert(offsetof(GameSetupMode, drs_allowed_lap_group) == 0x1e0);

struct GameWeatherSystem {
    uint8_t _pad_0x0[0x1d98];
    TimeOfDaySystem *tod; // L2 experimental
    uint8_t _pad_0x1da0[0x8];
    TimeOfDayOwner *tod_owner; // L2 experimental
    uint8_t _pad_0x1db0[0x152];
    uint8_t weekend_valid; // L2 experimental
    uint8_t _pad_0x1f03[0x1];
    uint32_t season_index; // L2 experimental
    uint32_t weekend_index; // L2 experimental
    uint8_t _pad_0x1f0c[0x4];
    uint32_t base_seed; // L2 experimental
    uint32_t forced_seed; // L2 experimental
    uint32_t weekend; // L2 experimental
    uint8_t _pad_0x1f1c[0x101e];
    uint8_t dataset_dynamic[8]; // L2 experimental
};
static_assert(offsetof(GameWeatherSystem, tod) == 0x1d98);
static_assert(offsetof(GameWeatherSystem, tod_owner) == 0x1da8);
static_assert(offsetof(GameWeatherSystem, weekend_valid) == 0x1f02);
static_assert(offsetof(GameWeatherSystem, season_index) == 0x1f04);
static_assert(offsetof(GameWeatherSystem, weekend_index) == 0x1f08);
static_assert(offsetof(GameWeatherSystem, base_seed) == 0x1f10);
static_assert(offsetof(GameWeatherSystem, forced_seed) == 0x1f14);
static_assert(offsetof(GameWeatherSystem, weekend) == 0x1f18);
static_assert(offsetof(GameWeatherSystem, dataset_dynamic) == 0x2f3a);

struct InputRecord {
    uint8_t _pad_0x0[0x4];
    int32_t gear_request; // L2 experimental
    uint8_t _pad_0x8[0x4];
    float brake; // L2 experimental
    float clutch; // L2 experimental
    float steering; // L2 experimental
    float throttle; // L2 experimental
};
static_assert(offsetof(InputRecord, gear_request) == 0x4);
static_assert(offsetof(InputRecord, brake) == 0xc);
static_assert(offsetof(InputRecord, clutch) == 0x10);
static_assert(offsetof(InputRecord, steering) == 0x14);
static_assert(offsetof(InputRecord, throttle) == 0x18);

struct RedFlagSystem {
    uint8_t _pad_0x0[0x40];
    uint8_t state; // L2 experimental
};
static_assert(offsetof(RedFlagSystem, state) == 0x40);

struct SpectatorDirector {
    uint8_t _pad_0x0[0xd0];
    uint8_t ui_active; // L2 experimental
    uint8_t active; // L2 experimental
    uint8_t _pad_0xd2[0x1];
    uint8_t should_quit; // L2 experimental
    uint8_t _pad_0xd4[0x2];
    uint8_t current_driver; // L2 experimental
    uint8_t _pad_0xd7[0x1];
    int32_t current_camera; // L2 experimental
    uint8_t requested_driver; // L2 experimental
    uint8_t _pad_0xdd[0x3];
    int32_t requested_camera; // L2 experimental
};
static_assert(offsetof(SpectatorDirector, ui_active) == 0xd0);
static_assert(offsetof(SpectatorDirector, active) == 0xd1);
static_assert(offsetof(SpectatorDirector, should_quit) == 0xd3);
static_assert(offsetof(SpectatorDirector, current_driver) == 0xd6);
static_assert(offsetof(SpectatorDirector, current_camera) == 0xd8);
static_assert(offsetof(SpectatorDirector, requested_driver) == 0xdc);
static_assert(offsetof(SpectatorDirector, requested_camera) == 0xe0);

struct TimeOfDayClock {
    uint8_t _pad_0x0[0x22];
    uint8_t hours_24; // L2 experimental
    uint8_t _pad_0x23[0x45];
    double time_a; // L2 experimental
    double time_b; // L2 experimental
    uint8_t _pad_0x78[0x8];
    float time_scale; // L2 experimental
    float length; // L2 experimental
    uint8_t _pad_0x88[0x8];
    float fixed_fraction; // L2 experimental
    uint8_t _pad_0x94[0x1f4];
};
static_assert(offsetof(TimeOfDayClock, hours_24) == 0x22);
static_assert(offsetof(TimeOfDayClock, time_a) == 0x68);
static_assert(offsetof(TimeOfDayClock, time_b) == 0x70);
static_assert(offsetof(TimeOfDayClock, time_scale) == 0x80);
static_assert(offsetof(TimeOfDayClock, length) == 0x84);
static_assert(offsetof(TimeOfDayClock, fixed_fraction) == 0x90);
static_assert(sizeof(TimeOfDayClock) == 0x288);

struct TimeOfDayOwner {
    uint8_t _pad_0x0[0x2c8];
    void *rain_render; // L2 experimental
    TimeOfDaySystem *tod; // L2 experimental
    uint8_t _pad_0x2d8[0x2c];
    int32_t tod_version; // L2 experimental
    uint8_t _pad_0x308[0xc];
    float rain_clamp_knee; // L2 experimental
    float rain_intensity; // L2 experimental
};
static_assert(offsetof(TimeOfDayOwner, rain_render) == 0x2c8);
static_assert(offsetof(TimeOfDayOwner, tod) == 0x2d0);
static_assert(offsetof(TimeOfDayOwner, tod_version) == 0x304);
static_assert(offsetof(TimeOfDayOwner, rain_clamp_knee) == 0x314);
static_assert(offsetof(TimeOfDayOwner, rain_intensity) == 0x318);

struct WeatherState {
    uint32_t type; // L2 experimental
    float intensity; // L2 experimental
    float end; // L2 experimental
    float transition; // L2 experimental
};
static_assert(offsetof(WeatherState, type) == 0x0);
static_assert(offsetof(WeatherState, intensity) == 0x4);
static_assert(offsetof(WeatherState, end) == 0x8);
static_assert(offsetof(WeatherState, transition) == 0xc);
static_assert(sizeof(WeatherState) == 0x10);

struct WeatherTransition {
    uint8_t _pad_0x0[0x10];
    char *name; // L2 experimental
    uint8_t _pad_0x18[0x28];
    float from_intensity; // L2 experimental
    float to_intensity; // L2 experimental
    uint32_t steps; // L2 experimental
    uint8_t _pad_0x4c[0x4];
};
static_assert(offsetof(WeatherTransition, name) == 0x10);
static_assert(offsetof(WeatherTransition, from_intensity) == 0x40);
static_assert(offsetof(WeatherTransition, to_intensity) == 0x44);
static_assert(offsetof(WeatherTransition, steps) == 0x48);
static_assert(sizeof(WeatherTransition) == 0x50);

struct WeatherDataset {
    WeatherTransition transitions[37]; // L2 experimental
    WeatherState states[38]; // L2 experimental
    uint32_t state_total; // L2 experimental
    uint8_t _pad_0xdf4[0x4];
    uint32_t state_count; // L2 experimental
    uint8_t _pad_0xdfc[0x4];
};
static_assert(offsetof(WeatherDataset, transitions) == 0x0);
static_assert(offsetof(WeatherDataset, states) == 0xb90);
static_assert(offsetof(WeatherDataset, state_total) == 0xdf0);
static_assert(offsetof(WeatherDataset, state_count) == 0xdf8);
static_assert(sizeof(WeatherDataset) == 0xe00);

struct WeatherTypeRange {
    float weight; // L2 experimental
    uint8_t _pad_0x4[0x8];
    float intensity_min; // L2 experimental
    float intensity_max; // L2 experimental
};
static_assert(offsetof(WeatherTypeRange, weight) == 0x0);
static_assert(offsetof(WeatherTypeRange, intensity_min) == 0xc);
static_assert(offsetof(WeatherTypeRange, intensity_max) == 0x10);
static_assert(sizeof(WeatherTypeRange) == 0x14);

struct TimeOfDaySystem {
    uint8_t _pad_0x0[0x2c8];
    TimeOfDayClock clocks[10]; // L2 experimental
    uint8_t _pad_0x1c18[0xa3c];
    uint8_t scripted_scenario; // L2 experimental
    uint8_t _pad_0x2655[0x6f];
    uint8_t active_dataset; // L2 experimental
    uint8_t _pad_0x26c5[0x13];
    WeatherDataset datasets[10]; // L2 experimental
    uint8_t _pad_0xb2d8[0x64];
    uint32_t seed; // L2 experimental
    uint8_t _pad_0xb340[0x10];
    float dataset_length[10]; // L2 experimental
    WeatherTypeRange type_ranges[6]; // L2 experimental
    uint8_t _pad_0xb3f0[0x18];
    uint32_t max_random_states; // L2 experimental
    uint8_t _pad_0xb40c[0x8];
    uint32_t current_state; // L2 experimental
    float state_fraction; // L2 experimental
    uint8_t _pad_0xb41c[0xa];
    uint8_t exact_timing; // L2 experimental
    uint8_t _pad_0xb427[0x2];
    uint8_t ready_a; // L2 experimental
    uint8_t ready_b; // L2 experimental
    uint8_t dirty; // L2 experimental
    uint8_t dirty_frame; // L2 experimental
    uint8_t random_override; // L2 experimental
    uint8_t _pad_0xb42e[0x4];
    uint8_t busy; // L2 experimental
    uint8_t _pad_0xb433[0x3];
    uint8_t override_active; // L2 experimental
    uint8_t _pad_0xb437[0x1];
    int32_t curve_version; // L2 experimental
};
static_assert(offsetof(TimeOfDaySystem, clocks) == 0x2c8);
static_assert(offsetof(TimeOfDaySystem, scripted_scenario) == 0x2654);
static_assert(offsetof(TimeOfDaySystem, active_dataset) == 0x26c4);
static_assert(offsetof(TimeOfDaySystem, datasets) == 0x26d8);
static_assert(offsetof(TimeOfDaySystem, seed) == 0xb33c);
static_assert(offsetof(TimeOfDaySystem, dataset_length) == 0xb350);
static_assert(offsetof(TimeOfDaySystem, type_ranges) == 0xb378);
static_assert(offsetof(TimeOfDaySystem, max_random_states) == 0xb408);
static_assert(offsetof(TimeOfDaySystem, current_state) == 0xb414);
static_assert(offsetof(TimeOfDaySystem, state_fraction) == 0xb418);
static_assert(offsetof(TimeOfDaySystem, exact_timing) == 0xb426);
static_assert(offsetof(TimeOfDaySystem, ready_a) == 0xb429);
static_assert(offsetof(TimeOfDaySystem, ready_b) == 0xb42a);
static_assert(offsetof(TimeOfDaySystem, dirty) == 0xb42b);
static_assert(offsetof(TimeOfDaySystem, dirty_frame) == 0xb42c);
static_assert(offsetof(TimeOfDaySystem, random_override) == 0xb42d);
static_assert(offsetof(TimeOfDaySystem, busy) == 0xb432);
static_assert(offsetof(TimeOfDaySystem, override_active) == 0xb436);
static_assert(offsetof(TimeOfDaySystem, curve_version) == 0xb438);

struct WeatherAndTimeOfDayComponent {
    uint8_t _pad_0x0[0x2a0];
    GameWeatherSystem *weather_system; // L2 experimental
    uint8_t _pad_0x2a8[0x1c];
    uint32_t random_seed; // L2 experimental
};
static_assert(offsetof(WeatherAndTimeOfDayComponent, weather_system) == 0x2a0);
static_assert(offsetof(WeatherAndTimeOfDayComponent, random_seed) == 0x2c4);

struct WeatherSlotBlock {
    float length_s; // L2 experimental
    uint32_t count; // L2 experimental
    float hold_s[6]; // L2 experimental
    float trans_s[6]; // L2 experimental
    uint8_t random[6]; // L2 experimental
    uint8_t type[6]; // L2 experimental
};
static_assert(offsetof(WeatherSlotBlock, length_s) == 0x0);
static_assert(offsetof(WeatherSlotBlock, count) == 0x4);
static_assert(offsetof(WeatherSlotBlock, hold_s) == 0x8);
static_assert(offsetof(WeatherSlotBlock, trans_s) == 0x20);
static_assert(offsetof(WeatherSlotBlock, random) == 0x38);
static_assert(offsetof(WeatherSlotBlock, type) == 0x3e);
static_assert(sizeof(WeatherSlotBlock) == 0x44);

struct WeatherSessionSettings {
    uint8_t _pad_0x0[0x4];
    uint32_t session_type; // L2 experimental
    uint8_t _pad_0x8[0xb08];
    uint8_t tod_flag_a; // L2 experimental
    uint8_t tod_flag_b; // L2 experimental
    uint8_t _pad_0xb12[0x2];
    uint32_t mode; // L2 experimental
    uint8_t exact_timing; // L2 experimental
    uint8_t tod_start_preset; // L2 experimental
    uint8_t _pad_0xb1a[0x2];
    float time_scale; // L2 experimental
    uint32_t single_count; // L2 experimental
    uint8_t _pad_0xb24[0x44];
    uint8_t single_random[6]; // L2 experimental
    uint8_t single_types[6]; // L2 experimental
    WeatherSlotBlock slots[8]; // L2 experimental
};
static_assert(offsetof(WeatherSessionSettings, session_type) == 0x4);
static_assert(offsetof(WeatherSessionSettings, tod_flag_a) == 0xb10);
static_assert(offsetof(WeatherSessionSettings, tod_flag_b) == 0xb11);
static_assert(offsetof(WeatherSessionSettings, mode) == 0xb14);
static_assert(offsetof(WeatherSessionSettings, exact_timing) == 0xb18);
static_assert(offsetof(WeatherSessionSettings, tod_start_preset) == 0xb19);
static_assert(offsetof(WeatherSessionSettings, time_scale) == 0xb1c);
static_assert(offsetof(WeatherSessionSettings, single_count) == 0xb20);
static_assert(offsetof(WeatherSessionSettings, single_random) == 0xb68);
static_assert(offsetof(WeatherSessionSettings, single_types) == 0xb6e);
static_assert(offsetof(WeatherSessionSettings, slots) == 0xb74);

using Sess_Apply_ContentAvailability_t = void(void *self, void *availability);
inline constexpr fn<"Sess_Apply_ContentAvailability", Sess_Apply_ContentAvailability_t> Sess_Apply_ContentAvailability{}; // L2 experimental
inline constexpr address<"Sess_Apply_DecidedPenalties"> Sess_Apply_DecidedPenalties{}; // L2 experimental
using Sess_Assign_GarageSlotsByTeamOrder_t = void(void *out_assignments, void *entrants, void *garages, void *team_order, void *fixed_teams);
inline constexpr fn<"Sess_Assign_GarageSlotsByTeamOrder", Sess_Assign_GarageSlotsByTeamOrder_t> Sess_Assign_GarageSlotsByTeamOrder{}; // L2 experimental
inline constexpr address<"Sess_AutoCrossRuleset_PlaceVehicles"> Sess_AutoCrossRuleset_PlaceVehicles{}; // L2 experimental
inline constexpr address<"Sess_Build_ChampionshipEntrantSelection"> Sess_Build_ChampionshipEntrantSelection{}; // L2 experimental
using Sess_Build_GroupResult_t = void(void *weekend, uint8_t group, void *result);
inline constexpr fn<"Sess_Build_GroupResult", Sess_Build_GroupResult_t> Sess_Build_GroupResult{}; // L2 experimental
inline constexpr address<"Sess_Build_SafetyCarLogicGraph"> Sess_Build_SafetyCarLogicGraph{}; // L2 experimental
using Sess_Build_SelectableDriverList_t = NeVecU32 *(NeVecU32 *out, void *ctx, void *entrants, const void *filter, void *alloc, uint8_t flag);
inline constexpr fn<"Sess_Build_SelectableDriverList", Sess_Build_SelectableDriverList_t> Sess_Build_SelectableDriverList{}; // L2 experimental
using Sess_Build_VehicleTyreInfo_t = void *(void *out, void *system, uint8_t vehicle, uint32_t flags, const void *ctx, float scale);
inline constexpr fn<"Sess_Build_VehicleTyreInfo", Sess_Build_VehicleTyreInfo_t> Sess_Build_VehicleTyreInfo{}; // L2 experimental
inline constexpr address<"Sess_ChallengeRuleset_UpdateLapDelta"> Sess_ChallengeRuleset_UpdateLapDelta{}; // L2 experimental
using Sess_ChampionshipWonDecision_Update_t = uint32_t *(void *state, uint32_t *route);
inline constexpr fn<"Sess_ChampionshipWonDecision_Update", Sess_ChampionshipWonDecision_Update_t> Sess_ChampionshipWonDecision_Update{}; // L2 experimental, SessionStateChampionshipWonDecision.
inline constexpr address<"Sess_Classification_CompareEntries"> Sess_Classification_CompareEntries{}; // L2 experimental
using Sess_Collect_AvailableTracks_t = void(void *track_data_system, const void *stat_id_bytes, void *out_ptr_vec);
inline constexpr fn<"Sess_Collect_AvailableTracks", Sess_Collect_AvailableTracks_t> Sess_Collect_AvailableTracks{}; // L2 experimental
inline constexpr address<"Sess_Collect_CareerSetupEntrantLists"> Sess_Collect_CareerSetupEntrantLists{}; // L2 experimental
inline constexpr address<"Sess_Collect_EntrantsForSetupMode"> Sess_Collect_EntrantsForSetupMode{}; // L2 experimental
inline constexpr address<"Sess_Collect_FilteredEntrants"> Sess_Collect_FilteredEntrants{}; // L2 experimental
using Sess_Collect_GameModeSetupModes_t = void *(void *out, void *allocator);
inline constexpr fn<"Sess_Collect_GameModeSetupModes", Sess_Collect_GameModeSetupModes_t> Sess_Collect_GameModeSetupModes{}; // L2 experimental
inline constexpr address<"Sess_Complete_Update"> Sess_Complete_Update{}; // L2 experimental
inline constexpr address<"Sess_ContentAvailability_Rebuild"> Sess_ContentAvailability_Rebuild{}; // L2 experimental
inline constexpr address<"Sess_ContentList_AddUniqueByte"> Sess_ContentList_AddUniqueByte{}; // L2 experimental
inline constexpr address<"Sess_ContentList_AddUniqueTriple"> Sess_ContentList_AddUniqueTriple{}; // L2 experimental
inline constexpr address<"Sess_ControllerSet_ClearSessionNotStarted"> Sess_ControllerSet_ClearSessionNotStarted{}; // L2 experimental
inline constexpr address<"Sess_ControllerSystem_ApplyInputs"> Sess_ControllerSystem_ApplyInputs{}; // L2 experimental
inline constexpr address<"Sess_ControllerSystem_BuildSets"> Sess_ControllerSystem_BuildSets{}; // L2 experimental
inline constexpr address<"Sess_ControllerSystem_DestroySets"> Sess_ControllerSystem_DestroySets{}; // L2 experimental
inline constexpr address<"Sess_Controllers_ReleaseForRaceStart"> Sess_Controllers_ReleaseForRaceStart{}; // L2 experimental
inline constexpr address<"Sess_CopyOptionsToState"> Sess_CopyOptionsToState{}; // L2 experimental
inline constexpr address<"Sess_DelayOnLoadingScreen_Update"> Sess_DelayOnLoadingScreen_Update{}; // L2 experimental
inline constexpr address<"Sess_DoughnutSequence_Enter"> Sess_DoughnutSequence_Enter{}; // L2 experimental
inline constexpr address<"Sess_DoughnutSequence_Update"> Sess_DoughnutSequence_Update{}; // L2 experimental
inline constexpr address<"Sess_Doughnut_SetWinnerText"> Sess_Doughnut_SetWinnerText{}; // L2 experimental
inline constexpr address<"Sess_DriveThroughPenalty_ReceiveData"> Sess_DriveThroughPenalty_ReceiveData{}; // L2 experimental
inline constexpr address<"Sess_DriveThroughPenalty_SendData"> Sess_DriveThroughPenalty_SendData{}; // L2 experimental
inline constexpr address<"Sess_DriveThroughPenalty_SendData_2"> Sess_DriveThroughPenalty_SendData_2{}; // L2 experimental
inline constexpr address<"Sess_EditMode_OnStateEnter"> Sess_EditMode_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_EditMode_OnStateExit"> Sess_EditMode_OnStateExit{}; // L2 experimental
inline constexpr address<"Sess_ElimRuleset_FinaliseResults"> Sess_ElimRuleset_FinaliseResults{}; // L2 experimental
inline constexpr address<"Sess_ElimRuleset_ForceFinishAll"> Sess_ElimRuleset_ForceFinishAll{}; // L2 experimental
inline constexpr address<"Sess_ElimRuleset_RetireVehicle"> Sess_ElimRuleset_RetireVehicle{}; // L2 experimental
inline constexpr address<"Sess_ElimTracker_AddVehicle"> Sess_ElimTracker_AddVehicle{}; // L2 experimental
inline constexpr address<"Sess_EntrantFilterSet_CopyCtor"> Sess_EntrantFilterSet_CopyCtor{}; // L2 experimental
inline constexpr address<"Sess_EntrantVec_PushBack"> Sess_EntrantVec_PushBack{}; // L2 experimental
inline constexpr address<"Sess_Entrant_BuildCandidate"> Sess_Entrant_BuildCandidate{}; // L2 experimental
inline constexpr address<"Sess_Entrant_IsTeamDriverMatch"> Sess_Entrant_IsTeamDriverMatch{}; // L2 experimental
inline constexpr address<"Sess_Entrant_PassesFilter"> Sess_Entrant_PassesFilter{}; // L2 experimental
inline constexpr address<"Sess_Entrants_RemoveDuplicates"> Sess_Entrants_RemoveDuplicates{}; // L2 experimental
using Sess_Find_GroupRowPosition_t = int32_t *(void *weekend, void *result, uint32_t driver_key);
inline constexpr fn<"Sess_Find_GroupRowPosition", Sess_Find_GroupRowPosition_t> Sess_Find_GroupRowPosition{}; // L2 experimental
inline constexpr address<"Sess_Flags_ApplyVehicleYellow"> Sess_Flags_ApplyVehicleYellow{}; // L2 experimental
inline constexpr address<"Sess_Flags_GetSecondsSinceEvent"> Sess_Flags_GetSecondsSinceEvent{}; // L2 experimental
inline constexpr address<"Sess_Flags_PlayZoneFlagEvent"> Sess_Flags_PlayZoneFlagEvent{}; // L2 experimental
inline constexpr address<"Sess_Flags_ResetSessionTypeIndex"> Sess_Flags_ResetSessionTypeIndex{}; // L2 experimental
inline constexpr address<"Sess_Flags_SetVehicleYellow"> Sess_Flags_SetVehicleYellow{}; // L2 experimental
inline constexpr address<"Sess_Flags_ShowBlueFlag"> Sess_Flags_ShowBlueFlag{}; // L2 experimental
inline constexpr address<"Sess_Flow_ParseXml"> Sess_Flow_ParseXml{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_CheckAllParked"> Sess_FormationLapLogic_CheckAllParked{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_Construct"> Sess_FormationLapLogic_Construct{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_IsActiveOtherType"> Sess_FormationLapLogic_IsActiveOtherType{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_IsActiveType0"> Sess_FormationLapLogic_IsActiveType0{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_IsRunning"> Sess_FormationLapLogic_IsRunning{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_Reset"> Sess_FormationLapLogic_Reset{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_SubscribePhotoModeDisabled"> Sess_FormationLapLogic_SubscribePhotoModeDisabled{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_SubscribePhotoModeEnabled"> Sess_FormationLapLogic_SubscribePhotoModeEnabled{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_SubscribeSafetyCarDeployment"> Sess_FormationLapLogic_SubscribeSafetyCarDeployment{}; // L2 experimental
inline constexpr address<"Sess_FormationLapLogic_Update"> Sess_FormationLapLogic_Update{}; // L2 experimental
inline constexpr address<"Sess_FormationLapStartLights_OnStateEnter"> Sess_FormationLapStartLights_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_FormationLapType_Update"> Sess_FormationLapType_Update{}; // L2 experimental
inline constexpr address<"Sess_FormationLap_OnStateExit"> Sess_FormationLap_OnStateExit{}; // L2 experimental
using Sess_FormationLap_Update_t = uint32_t *(void *state, uint32_t *route, double dt);
inline constexpr fn<"Sess_FormationLap_Update", Sess_FormationLap_Update_t> Sess_FormationLap_Update{}; // L2 experimental
inline constexpr address<"Sess_FreezeAllVehicles"> Sess_FreezeAllVehicles{}; // L2 experimental
using Sess_GameWeatherSystem_SetActiveSessionType_t = void(void *gws, int session_type, float time_scale);
inline constexpr fn<"Sess_GameWeatherSystem_SetActiveSessionType", Sess_GameWeatherSystem_SetActiveSessionType_t> Sess_GameWeatherSystem_SetActiveSessionType{}; // L2 experimental
using Sess_GameWeatherSystem_SetupSession_t = void(void *ws, void *settings, void *config);
inline constexpr fn<"Sess_GameWeatherSystem_SetupSession", Sess_GameWeatherSystem_SetupSession_t> Sess_GameWeatherSystem_SetupSession{}; // L2 experimental, GameWeatherSystem::setupSession.
using Sess_GameWeatherSystem_UpdateRandomSeed_t = void(void *ws, uint32_t seed, uint32_t weekend);
inline constexpr fn<"Sess_GameWeatherSystem_UpdateRandomSeed", Sess_GameWeatherSystem_UpdateRandomSeed_t> Sess_GameWeatherSystem_UpdateRandomSeed{}; // L2 experimental, GameWeatherSystem::updateRandomSeed.
inline constexpr address<"Sess_GarageSlotVec_PushBack"> Sess_GarageSlotVec_PushBack{}; // L2 experimental
inline constexpr address<"Sess_GarageState_OnStateExit"> Sess_GarageState_OnStateExit{}; // L2 experimental
inline constexpr address<"Sess_GarageState_RefreshHudVisibility"> Sess_GarageState_RefreshHudVisibility{}; // L2 experimental
inline constexpr address<"Sess_Garage_HasPendingName"> Sess_Garage_HasPendingName{}; // L2 experimental
inline constexpr address<"Sess_Get_CarDriverName"> Sess_Get_CarDriverName{}; // L2 experimental
using Sess_Get_CurrentContentSetName_t = NeInlineStr128 *(NeInlineStr128 *out);
inline constexpr fn<"Sess_Get_CurrentContentSetName", Sess_Get_CurrentContentSetName_t> Sess_Get_CurrentContentSetName{}; // L2 experimental
using Sess_Get_PlayerVehicleIndex_t = uint8_t(void *ctx, int local_player);
inline constexpr fn<"Sess_Get_PlayerVehicleIndex", Sess_Get_PlayerVehicleIndex_t> Sess_Get_PlayerVehicleIndex{}; // L2 experimental
using Sess_GridRundown_Update_t = uint32_t *(void *state, uint32_t *route, double dt);
inline constexpr fn<"Sess_GridRundown_Update", Sess_GridRundown_Update_t> Sess_GridRundown_Update{}; // L2 experimental
using Sess_InGamePQ_Update_t = uint32_t *(void *state, uint32_t *route, double dt);
inline constexpr fn<"Sess_InGamePQ_Update", Sess_InGamePQ_Update_t> Sess_InGamePQ_Update{}; // L2 experimental
inline constexpr address<"Sess_InGameRace_OnStateExit"> Sess_InGameRace_OnStateExit{}; // L2 experimental
inline constexpr address<"Sess_InGameRace_Update"> Sess_InGameRace_Update{}; // L2 experimental
inline constexpr address<"Sess_InGame_OnStateExitHideHud"> Sess_InGame_OnStateExitHideHud{}; // L2 experimental
inline constexpr address<"Sess_InGame_SubscribeMessages"> Sess_InGame_SubscribeMessages{}; // L2 experimental
inline constexpr address<"Sess_InGame_UnsubscribeMessages"> Sess_InGame_UnsubscribeMessages{}; // L2 experimental
using Sess_InGame_UpdateBase_t = uint32_t *(void *state, uint32_t *route, double dt);
inline constexpr fn<"Sess_InGame_UpdateBase", Sess_InGame_UpdateBase_t> Sess_InGame_UpdateBase{}; // L2 experimental
inline constexpr address<"Sess_IntroVideo_CanFinish"> Sess_IntroVideo_CanFinish{}; // L2 experimental
inline constexpr address<"Sess_IntroVideo_OnStateEnter"> Sess_IntroVideo_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_IntroVideo_OnVideoDoneFadeIn"> Sess_IntroVideo_OnVideoDoneFadeIn{}; // L2 experimental
using Sess_IntroVideo_ParseParams_t = uint64_t(void *state, void *params);
inline constexpr fn<"Sess_IntroVideo_ParseParams", Sess_IntroVideo_ParseParams_t> Sess_IntroVideo_ParseParams{}; // L2 experimental, SessionStateIntroVideo
inline constexpr address<"Sess_IntroVideo_PushAllocRequest"> Sess_IntroVideo_PushAllocRequest{}; // L2 experimental
using Sess_Invalidate_Lap_Thunk_t = void(void *rules, uint32_t vehicle, uint32_t reason, char mode);
inline constexpr fn<"Sess_Invalidate_Lap_Thunk", Sess_Invalidate_Lap_Thunk_t> Sess_Invalidate_Lap_Thunk{}; // L2 experimental
using Sess_Issue_DriveThrough_t = void(void *rules, uint8_t *incident, uint32_t reason);
inline constexpr fn<"Sess_Issue_DriveThrough", Sess_Issue_DriveThrough_t> Sess_Issue_DriveThrough{}; // L2 experimental
inline constexpr address<"Sess_Issue_GridPlacePenalty"> Sess_Issue_GridPlacePenalty{}; // L2 experimental
inline constexpr address<"Sess_Issue_PostRaceTimePenalty"> Sess_Issue_PostRaceTimePenalty{}; // L2 experimental
using Sess_Issue_TimePenalty_t = void(void *rules, uint8_t *incident, uint32_t reason, uint32_t seconds);
inline constexpr fn<"Sess_Issue_TimePenalty", Sess_Issue_TimePenalty_t> Sess_Issue_TimePenalty{}; // L2 experimental
using Sess_Issue_Warning_t = void(void *rules, uint8_t *incident, uint32_t reason);
inline constexpr fn<"Sess_Issue_Warning", Sess_Issue_Warning_t> Sess_Issue_Warning{}; // L2 experimental
inline constexpr address<"Sess_LapRecord_Construct"> Sess_LapRecord_Construct{}; // L2 experimental
inline constexpr address<"Sess_LapRecords_SortByTime"> Sess_LapRecords_SortByTime{}; // L2 experimental
inline constexpr address<"Sess_LapSim_PrepareVehicleParams"> Sess_LapSim_PrepareVehicleParams{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_BuildSectorComparison"> Sess_LapTimes_BuildSectorComparison{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_CheckPersonalBestSector"> Sess_LapTimes_CheckPersonalBestSector{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_GetCarData"> Sess_LapTimes_GetCarData{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_IsNewFastestLap"> Sess_LapTimes_IsNewFastestLap{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_OnNewSector"> Sess_LapTimes_OnNewSector{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_ReportNewLapTime"> Sess_LapTimes_ReportNewLapTime{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_ReportSplit"> Sess_LapTimes_ReportSplit{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_Reset"> Sess_LapTimes_Reset{}; // L2 experimental
inline constexpr address<"Sess_LapTimes_SendLapHistory"> Sess_LapTimes_SendLapHistory{}; // L2 experimental
inline constexpr address<"Sess_Load_GameSetupBranding"> Sess_Load_GameSetupBranding{}; // L2 experimental
inline constexpr address<"Sess_Load_PhysicalTyreCompounds"> Sess_Load_PhysicalTyreCompounds{}; // L2 experimental
inline constexpr address<"Sess_Load_RuleSettingsProfiles"> Sess_Load_RuleSettingsProfiles{}; // L2 experimental
using Sess_Load_Teams_t = void(void *entrants_system, uint8_t set);
inline constexpr fn<"Sess_Load_Teams", Sess_Load_Teams_t> Sess_Load_Teams{}; // L2 experimental
inline constexpr address<"Sess_Load_TrackDatabase"> Sess_Load_TrackDatabase{}; // L2 experimental
inline constexpr address<"Sess_Load_TyreCompoundClasses"> Sess_Load_TyreCompoundClasses{}; // L2 experimental
inline constexpr address<"Sess_Load_VisualTyreCompounds"> Sess_Load_VisualTyreCompounds{}; // L2 experimental
inline constexpr address<"Sess_Loader_BuildTrackRegionList"> Sess_Loader_BuildTrackRegionList{}; // L2 experimental
inline constexpr address<"Sess_Loader_PlayGridPlayerShotCutscene"> Sess_Loader_PlayGridPlayerShotCutscene{}; // L2 experimental
inline constexpr address<"Sess_Loader_PrepareGridThumbnails"> Sess_Loader_PrepareGridThumbnails{}; // L2 experimental
inline constexpr address<"Sess_Loader_RemoveListener"> Sess_Loader_RemoveListener{}; // L2 experimental
inline constexpr address<"Sess_Loader_RequestCareerHubLoad"> Sess_Loader_RequestCareerHubLoad{}; // L2 experimental
inline constexpr address<"Sess_Loader_RequestCareerHubUnload"> Sess_Loader_RequestCareerHubUnload{}; // L2 experimental
inline constexpr address<"Sess_Loader_RequestLoadThumbnails"> Sess_Loader_RequestLoadThumbnails{}; // L2 experimental
inline constexpr address<"Sess_Loader_RequestSessionLoad"> Sess_Loader_RequestSessionLoad{}; // L2 experimental
inline constexpr address<"Sess_Loader_RequestSessionUnload"> Sess_Loader_RequestSessionUnload{}; // L2 experimental
inline constexpr address<"Sess_Loader_RequestTrackLoad"> Sess_Loader_RequestTrackLoad{}; // L2 experimental
inline constexpr address<"Sess_Loader_RequestTrackUnload"> Sess_Loader_RequestTrackUnload{}; // L2 experimental
inline constexpr address<"Sess_Loader_SetupTrackAssets"> Sess_Loader_SetupTrackAssets{}; // L2 experimental
inline constexpr address<"Sess_Loader_StartFrontendAssetLoad"> Sess_Loader_StartFrontendAssetLoad{}; // L2 experimental
inline constexpr address<"Sess_Loader_UnloadSession"> Sess_Loader_UnloadSession{}; // L2 experimental
inline constexpr address<"Sess_Loader_Update"> Sess_Loader_Update{}; // L2 experimental
inline constexpr address<"Sess_LocationCutscene_HandleStoryEvent"> Sess_LocationCutscene_HandleStoryEvent{}; // L2 experimental
inline constexpr address<"Sess_LocationCutscene_SetInputBlockBit"> Sess_LocationCutscene_SetInputBlockBit{}; // L2 experimental
inline constexpr address<"Sess_LocationLoadCutscene_OnStateUpdate"> Sess_LocationLoadCutscene_OnStateUpdate{}; // L2 experimental
inline constexpr address<"Sess_Mark_EntrantListAvailable"> Sess_Mark_EntrantListAvailable{}; // L2 experimental
inline constexpr address<"Sess_MarshalLights_Set"> Sess_MarshalLights_Set{}; // L2 experimental
inline constexpr address<"Sess_MarshalLights_UpdateZoneShader"> Sess_MarshalLights_UpdateZoneShader{}; // L2 experimental
inline constexpr address<"Sess_MarshalZoneMonitor_Init"> Sess_MarshalZoneMonitor_Init{}; // L2 experimental
inline constexpr address<"Sess_MarshalZone_CheckFlags"> Sess_MarshalZone_CheckFlags{}; // L2 experimental
inline constexpr address<"Sess_MarshalZone_UpdateZoneVehicles"> Sess_MarshalZone_UpdateZoneVehicles{}; // L2 experimental
inline constexpr address<"Sess_MisconductManager_Ctor"> Sess_MisconductManager_Ctor{}; // L2 experimental
inline constexpr address<"Sess_MisconductTable_Ctor"> Sess_MisconductTable_Ctor{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_AddTableRow"> Sess_Misconduct_AddTableRow{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_CheckEnvironmentContact"> Sess_Misconduct_CheckEnvironmentContact{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_CheckWallRiding"> Sess_Misconduct_CheckWallRiding{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_DumpTable"> Sess_Misconduct_DumpTable{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_EvaluateInfringement"> Sess_Misconduct_EvaluateInfringement{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_GetDriverState"> Sess_Misconduct_GetDriverState{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_GetWatchState"> Sess_Misconduct_GetWatchState{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_HandBackIllegalOvertake"> Sess_Misconduct_HandBackIllegalOvertake{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_IsWatchedVehicleActive"> Sess_Misconduct_IsWatchedVehicleActive{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_OnCarToCarCollision"> Sess_Misconduct_OnCarToCarCollision{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_RegisterReporter"> Sess_Misconduct_RegisterReporter{}; // L2 experimental
inline constexpr address<"Sess_Misconduct_RemoveWatchedIncident"> Sess_Misconduct_RemoveWatchedIncident{}; // L2 experimental
inline constexpr address<"Sess_MultipleWarningsPenalty_ReceiveData"> Sess_MultipleWarningsPenalty_ReceiveData{}; // L2 experimental
inline constexpr address<"Sess_MultipleWarningsPenalty_SendData"> Sess_MultipleWarningsPenalty_SendData{}; // L2 experimental
inline constexpr address<"Sess_MultipleWarningsPenalty_SendData_2"> Sess_MultipleWarningsPenalty_SendData_2{}; // L2 experimental
inline constexpr address<"Sess_NetJournalMonitorState_OnExit"> Sess_NetJournalMonitorState_OnExit{}; // L2 experimental
inline constexpr address<"Sess_ObfValue_SetWithHistory"> Sess_ObfValue_SetWithHistory{}; // L2 experimental
inline constexpr address<"Sess_OnTrack_SetMessageHandlers"> Sess_OnTrack_SetMessageHandlers{}; // L2 experimental
inline constexpr address<"Sess_OsqTeleportToStart"> Sess_OsqTeleportToStart{}; // L2 experimental
inline constexpr address<"Sess_OvertakeLog_SendAllRecords"> Sess_OvertakeLog_SendAllRecords{}; // L2 experimental
inline constexpr address<"Sess_PQRuleset_FinaliseResults"> Sess_PQRuleset_FinaliseResults{}; // L2 experimental
inline constexpr address<"Sess_Parse_CarStateTransform"> Sess_Parse_CarStateTransform{}; // L2 experimental
using Sess_Parse_EntrantList_t = void(int *ordinal, const GdbTableCursor *row, void *list, void *driver_registry);
inline constexpr fn<"Sess_Parse_EntrantList", Sess_Parse_EntrantList_t> Sess_Parse_EntrantList{}; // L2 experimental
inline constexpr address<"Sess_Parse_EntrantTemplateRef"> Sess_Parse_EntrantTemplateRef{}; // L2 experimental
inline constexpr address<"Sess_Parse_PlayerDescription"> Sess_Parse_PlayerDescription{}; // L2 experimental
inline constexpr address<"Sess_Parse_PlayerDriverRef"> Sess_Parse_PlayerDriverRef{}; // L2 experimental
inline constexpr address<"Sess_PenaltyLog_FormatInfringementText"> Sess_PenaltyLog_FormatInfringementText{}; // L2 experimental
inline constexpr address<"Sess_PenaltyLog_FormatPenaltyText"> Sess_PenaltyLog_FormatPenaltyText{}; // L2 experimental
inline constexpr address<"Sess_PenaltyLog_Print"> Sess_PenaltyLog_Print{}; // L2 experimental
inline constexpr address<"Sess_PenaltyLog_ReceiveAddRecord"> Sess_PenaltyLog_ReceiveAddRecord{}; // L2 experimental
inline constexpr address<"Sess_PenaltyLog_RemoveVehicle"> Sess_PenaltyLog_RemoveVehicle{}; // L2 experimental
inline constexpr address<"Sess_PenaltyLog_SendAllRecords"> Sess_PenaltyLog_SendAllRecords{}; // L2 experimental
inline constexpr address<"Sess_PenaltyLog_SendRecord"> Sess_PenaltyLog_SendRecord{}; // L2 experimental
inline constexpr address<"Sess_PenaltyLog_SendRecord_2"> Sess_PenaltyLog_SendRecord_2{}; // L2 experimental
inline constexpr address<"Sess_Penalty_ApplyBlackFlagDsq"> Sess_Penalty_ApplyBlackFlagDsq{}; // L2 experimental
inline constexpr address<"Sess_PersistStream_LoadFromFile"> Sess_PersistStream_LoadFromFile{}; // L2 experimental
using Sess_Persistent_OnStateEnter_t = bool(void *state);
inline constexpr fn<"Sess_Persistent_OnStateEnter", Sess_Persistent_OnStateEnter_t> Sess_Persistent_OnStateEnter{}; // L2 experimental, StatePersistent.
using Sess_PitStopUpdateJob_Update_t = void(void *job);
inline constexpr fn<"Sess_PitStopUpdateJob_Update", Sess_PitStopUpdateJob_Update_t> Sess_PitStopUpdateJob_Update{}; // L2 experimental
inline constexpr address<"Sess_PlayCutScene_OnStateEnter"> Sess_PlayCutScene_OnStateEnter{}; // L2 experimental
using Sess_PlayCutScene_ParseParams_t = uint64_t(void *state, void *params);
inline constexpr fn<"Sess_PlayCutScene_ParseParams", Sess_PlayCutScene_ParseParams_t> Sess_PlayCutScene_ParseParams{}; // L2 experimental, SessionStatePlayCutScene.
inline constexpr address<"Sess_PlayerDescList_Append"> Sess_PlayerDescList_Append{}; // L2 experimental
inline constexpr address<"Sess_PlayerTerminalDamage_Enter"> Sess_PlayerTerminalDamage_Enter{}; // L2 experimental
inline constexpr address<"Sess_PostRaceSceneSetup_OnStateEnter"> Sess_PostRaceSceneSetup_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_PreLights_OnStateEnter"> Sess_PreLights_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_PreLights_Update"> Sess_PreLights_Update{}; // L2 experimental
inline constexpr address<"Sess_PreRaceGridState_OnEnter"> Sess_PreRaceGridState_OnEnter{}; // L2 experimental
using Sess_PreRaceToRace_Update_t = uint32_t *(void *state, uint32_t *route);
inline constexpr fn<"Sess_PreRaceToRace_Update", Sess_PreRaceToRace_Update_t> Sess_PreRaceToRace_Update{}; // L2 experimental
inline constexpr address<"Sess_PreRace_OnStateExit"> Sess_PreRace_OnStateExit{}; // L2 experimental
inline constexpr address<"Sess_PreRace_PlayGridPlayerShotCutscene"> Sess_PreRace_PlayGridPlayerShotCutscene{}; // L2 experimental
using Sess_PreRace_ResolveComponents_t = int(void *state, void *serviceProvider, void *componentMap);
inline constexpr fn<"Sess_PreRace_ResolveComponents", Sess_PreRace_ResolveComponents_t> Sess_PreRace_ResolveComponents{}; // L2 experimental, SessionStatePreRace
using Sess_PreRace_Update_t = uint32_t *(void *state, uint32_t *route, double dt);
inline constexpr fn<"Sess_PreRace_Update", Sess_PreRace_Update_t> Sess_PreRace_Update{}; // L2 experimental
inline constexpr address<"Sess_Progress_ComputeTimeGap"> Sess_Progress_ComputeTimeGap{}; // L2 experimental
inline constexpr address<"Sess_Progress_SyncResultRecord"> Sess_Progress_SyncResultRecord{}; // L2 experimental
using Sess_PuPanel_FillComponent_t = void(void *vm, uint64_t a2, uint32_t slot, uint8_t a4, uint8_t a5, void *entry);
inline constexpr fn<"Sess_PuPanel_FillComponent", Sess_PuPanel_FillComponent_t> Sess_PuPanel_FillComponent{}; // L2 experimental
inline constexpr address<"Sess_PursuitRuleset_FinaliseResults"> Sess_PursuitRuleset_FinaliseResults{}; // L2 experimental
inline constexpr address<"Sess_PursuitRuleset_ResetState"> Sess_PursuitRuleset_ResetState{}; // L2 experimental
inline constexpr address<"Sess_RC_AheadOfLeadUpdate"> Sess_RC_AheadOfLeadUpdate{}; // L2 experimental
inline constexpr address<"Sess_RC_HandleAheadOfLeadData"> Sess_RC_HandleAheadOfLeadData{}; // L2 experimental
inline constexpr address<"Sess_RC_SendAheadOfLeadData"> Sess_RC_SendAheadOfLeadData{}; // L2 experimental
inline constexpr address<"Sess_RaceRuleset_GetTimeRemaining"> Sess_RaceRuleset_GetTimeRemaining{}; // L2 experimental
inline constexpr address<"Sess_RaceRuleset_OnLapCompleted"> Sess_RaceRuleset_OnLapCompleted{}; // L2 experimental
inline constexpr address<"Sess_RaceRuleset_SetTimeLimit"> Sess_RaceRuleset_SetTimeLimit{}; // L2 experimental
inline constexpr address<"Sess_RaceState_ResetTimingAndTowState"> Sess_RaceState_ResetTimingAndTowState{}; // L2 experimental
inline constexpr address<"Sess_RaceState_VehicleHasNamedEntry"> Sess_RaceState_VehicleHasNamedEntry{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_BuildPitPlan"> Sess_RaceStrategy_BuildPitPlan{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_BuildRedFlagStints"> Sess_RaceStrategy_BuildRedFlagStints{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_CopyPlanStints"> Sess_RaceStrategy_CopyPlanStints{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_EvaluateStintTyre"> Sess_RaceStrategy_EvaluateStintTyre{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_FinalisePlan"> Sess_RaceStrategy_FinalisePlan{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_GetCurrentTyreSet"> Sess_RaceStrategy_GetCurrentTyreSet{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_Initialise"> Sess_RaceStrategy_Initialise{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_IsPlanValid"> Sess_RaceStrategy_IsPlanValid{}; // L2 experimental
using Sess_RaceStrategy_RedFlagRecompute_t = void(void *processor, void *race_session_data);
inline constexpr fn<"Sess_RaceStrategy_RedFlagRecompute", Sess_RaceStrategy_RedFlagRecompute_t> Sess_RaceStrategy_RedFlagRecompute{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_SetPlanType"> Sess_RaceStrategy_SetPlanType{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_SortCandidatesA"> Sess_RaceStrategy_SortCandidatesA{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_SortCandidatesB"> Sess_RaceStrategy_SortCandidatesB{}; // L2 experimental
inline constexpr address<"Sess_RaceStrategy_SortCandidatesC"> Sess_RaceStrategy_SortCandidatesC{}; // L2 experimental
using Sess_RaceStrategy_UpdateVehicle_t = void(void *proc, void *race_session_data);
inline constexpr fn<"Sess_RaceStrategy_UpdateVehicle", Sess_RaceStrategy_UpdateVehicle_t> Sess_RaceStrategy_UpdateVehicle{}; // L2 experimental
inline constexpr address<"Sess_Race_ComputeUnlapLapState"> Sess_Race_ComputeUnlapLapState{}; // L2 experimental
inline constexpr address<"Sess_Read_DeploymentPoints"> Sess_Read_DeploymentPoints{}; // L2 experimental
inline constexpr address<"Sess_Read_LoosePartsRules"> Sess_Read_LoosePartsRules{}; // L2 experimental
using Sess_Record_GroupTimes_t = void(void *weekend, uint8_t group, void *entries);
inline constexpr fn<"Sess_Record_GroupTimes", Sess_Record_GroupTimes_t> Sess_Record_GroupTimes{}; // L2 experimental
inline constexpr address<"Sess_Recordable_Register"> Sess_Recordable_Register{}; // L2 experimental
inline constexpr address<"Sess_Recordables_AddUnique"> Sess_Recordables_AddUnique{}; // L2 experimental
inline constexpr address<"Sess_RedFlagFreezeVehicles_Enter"> Sess_RedFlagFreezeVehicles_Enter{}; // L2 experimental
inline constexpr address<"Sess_RedFlagPerformRepairs_Update"> Sess_RedFlagPerformRepairs_Update{}; // L2 experimental
using Sess_RedFlagRaceStrategy_Enter_t = uint64_t(void *state);
inline constexpr fn<"Sess_RedFlagRaceStrategy_Enter", Sess_RedFlagRaceStrategy_Enter_t> Sess_RedFlagRaceStrategy_Enter{}; // L2 experimental
using Sess_RedFlagRaceStrategy_Update_t = uint32_t *(void *state, uint32_t *route);
inline constexpr fn<"Sess_RedFlagRaceStrategy_Update", Sess_RedFlagRaceStrategy_Update_t> Sess_RedFlagRaceStrategy_Update{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSimulate_Enter"> Sess_RedFlagSimulate_Enter{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSimulate_ResetEntries"> Sess_RedFlagSimulate_ResetEntries{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSystem_Deploy"> Sess_RedFlagSystem_Deploy{}; // L2 experimental, RedFlagSystem
inline constexpr address<"Sess_RedFlagSystem_OnSessionBeginGameplay"> Sess_RedFlagSystem_OnSessionBeginGameplay{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSystem_OnStartLightsBegin"> Sess_RedFlagSystem_OnStartLightsBegin{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSystem_PerformRepairs"> Sess_RedFlagSystem_PerformRepairs{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSystem_PrepareRestart"> Sess_RedFlagSystem_PrepareRestart{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSystem_ResetCarStatus"> Sess_RedFlagSystem_ResetCarStatus{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSystem_SimulateSessionTime"> Sess_RedFlagSystem_SimulateSessionTime{}; // L2 experimental
inline constexpr address<"Sess_RedFlagSystem_SnapshotDamage"> Sess_RedFlagSystem_SnapshotDamage{}; // L2 experimental
inline constexpr address<"Sess_RedFlagToRestart_Enter"> Sess_RedFlagToRestart_Enter{}; // L2 experimental
inline constexpr address<"Sess_RedFlagToRestart_Exit"> Sess_RedFlagToRestart_Exit{}; // L2 experimental
inline constexpr address<"Sess_RedFlagToRestart_Update"> Sess_RedFlagToRestart_Update{}; // L2 experimental
inline constexpr address<"Sess_RedFlag_InitSimVehicleState"> Sess_RedFlag_InitSimVehicleState{}; // L2 experimental
inline constexpr address<"Sess_RedFlag_ResetVehicleTyreState"> Sess_RedFlag_ResetVehicleTyreState{}; // L2 experimental
inline constexpr address<"Sess_RedFlag_SimulateUnlapLaps"> Sess_RedFlag_SimulateUnlapLaps{}; // L2 experimental
inline constexpr address<"Sess_Register_EntrantSetupMode"> Sess_Register_EntrantSetupMode{}; // L2 experimental
using Sess_Register_RaceRecordables_t = void(void *game);
inline constexpr fn<"Sess_Register_RaceRecordables", Sess_Register_RaceRecordables_t> Sess_Register_RaceRecordables{}; // L2 experimental
inline constexpr address<"Sess_Registry_GetFormationLapLogic"> Sess_Registry_GetFormationLapLogic{}; // L2 experimental
inline constexpr address<"Sess_ResetReplay_OnStateEnter"> Sess_ResetReplay_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_ResetReplay_Update"> Sess_ResetReplay_Update{}; // L2 experimental
inline constexpr address<"Sess_Reset_OnTrackRuntimeState"> Sess_Reset_OnTrackRuntimeState{}; // L2 experimental
inline constexpr address<"Sess_ResolveEntityHandle"> Sess_ResolveEntityHandle{}; // L2 experimental
inline constexpr address<"Sess_Resolve_DriverRef"> Sess_Resolve_DriverRef{}; // L2 experimental
inline constexpr address<"Sess_Resolve_PlayerVehicleRef"> Sess_Resolve_PlayerVehicleRef{}; // L2 experimental
inline constexpr address<"Sess_Resolve_SelectionDomainId"> Sess_Resolve_SelectionDomainId{}; // L2 experimental
inline constexpr address<"Sess_Resolve_TrackLayerId"> Sess_Resolve_TrackLayerId{}; // L2 experimental
inline constexpr address<"Sess_Restart_ResetGridLineup"> Sess_Restart_ResetGridLineup{}; // L2 experimental
inline constexpr address<"Sess_ResultRecord_FillFromLapHistory"> Sess_ResultRecord_FillFromLapHistory{}; // L2 experimental
inline constexpr address<"Sess_Result_ChangeVehicleStatus"> Sess_Result_ChangeVehicleStatus{}; // L2 experimental
inline constexpr address<"Sess_Result_DecodeVehicleResult"> Sess_Result_DecodeVehicleResult{}; // L2 experimental
inline constexpr address<"Sess_Result_RequestDNF"> Sess_Result_RequestDNF{}; // L2 experimental
inline constexpr address<"Sess_Result_RequestStatus"> Sess_Result_RequestStatus{}; // L2 experimental
inline constexpr address<"Sess_Result_RequestVehicleStatus"> Sess_Result_RequestVehicleStatus{}; // L2 experimental
inline constexpr address<"Sess_RetirementSequence_OnEnter"> Sess_RetirementSequence_OnEnter{}; // L2 experimental
inline constexpr address<"Sess_RetirementSequence_PlayTerminalDamage"> Sess_RetirementSequence_PlayTerminalDamage{}; // L2 experimental
inline constexpr address<"Sess_RetirementSequence_ShouldEnd"> Sess_RetirementSequence_ShouldEnd{}; // L2 experimental
inline constexpr address<"Sess_RetirementSequence_Update"> Sess_RetirementSequence_Update{}; // L2 experimental
inline constexpr address<"Sess_RuleManager_HandleSessionEvents"> Sess_RuleManager_HandleSessionEvents{}; // L2 experimental
using Sess_RuleManager_Initialise_t = uint32_t(void *self, void *systems, void *components);
inline constexpr fn<"Sess_RuleManager_Initialise", Sess_RuleManager_Initialise_t> Sess_RuleManager_Initialise{}; // L2 experimental
inline constexpr address<"Sess_RuleManager_ResolveSubsystems"> Sess_RuleManager_ResolveSubsystems{}; // L2 experimental
inline constexpr address<"Sess_Rules_ApplyBlackFlag"> Sess_Rules_ApplyBlackFlag{}; // L2 experimental
inline constexpr address<"Sess_Rules_ApplyPostSessionPenalties"> Sess_Rules_ApplyPostSessionPenalties{}; // L2 experimental
inline constexpr address<"Sess_Rules_ClearVehicleRuleFlags"> Sess_Rules_ClearVehicleRuleFlags{}; // L2 experimental
inline constexpr address<"Sess_Rules_CornerCuttingTimeGained_2"> Sess_Rules_CornerCuttingTimeGained_2{}; // L2 experimental
using Sess_Rules_Disqualify_t = void(void *rules, uint32_t vehicle);
inline constexpr fn<"Sess_Rules_Disqualify", Sess_Rules_Disqualify_t> Sess_Rules_Disqualify{}; // L2 experimental
inline constexpr address<"Sess_Rules_DisqualifyForPenaltyPoints"> Sess_Rules_DisqualifyForPenaltyPoints{}; // L2 experimental
inline constexpr address<"Sess_Rules_GetBlackFlagSecondsLeft"> Sess_Rules_GetBlackFlagSecondsLeft{}; // L2 experimental
using Sess_Rules_IsPracticeQualifying_t = bool(void *owner_with_session_at_0x1b8);
inline constexpr fn<"Sess_Rules_IsPracticeQualifying", Sess_Rules_IsPracticeQualifying_t> Sess_Rules_IsPracticeQualifying{}; // L2 experimental
inline constexpr address<"Sess_Rules_IssueDisqualification"> Sess_Rules_IssueDisqualification{}; // L2 experimental
inline constexpr address<"Sess_Rules_LookupPenaltyValue"> Sess_Rules_LookupPenaltyValue{}; // L2 experimental
inline constexpr address<"Sess_Rules_OnPaceCarLapForVehicle"> Sess_Rules_OnPaceCarLapForVehicle{}; // L2 experimental
inline constexpr address<"Sess_Rules_PaceCarNewLap"> Sess_Rules_PaceCarNewLap{}; // L2 experimental
inline constexpr address<"Sess_Rules_QueueInfringementEvent"> Sess_Rules_QueueInfringementEvent{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadBlueFlagRules"> Sess_Rules_ReadBlueFlagRules{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadCollisionExpectedSpeed"> Sess_Rules_ReadCollisionExpectedSpeed{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadCollisionSettings"> Sess_Rules_ReadCollisionSettings{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadCornerCuttingSettings"> Sess_Rules_ReadCornerCuttingSettings{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadFormationLapMiniGame"> Sess_Rules_ReadFormationLapMiniGame{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadOffTrackTimeSettings"> Sess_Rules_ReadOffTrackTimeSettings{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadPerClassSettings"> Sess_Rules_ReadPerClassSettings{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadPitWhiteLine"> Sess_Rules_ReadPitWhiteLine{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadRedFlagRepairTimes"> Sess_Rules_ReadRedFlagRepairTimes{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadSafetyCarPacePenalties"> Sess_Rules_ReadSafetyCarPacePenalties{}; // L2 experimental
inline constexpr address<"Sess_Rules_ReadYellowFlagOvertaking"> Sess_Rules_ReadYellowFlagOvertaking{}; // L2 experimental
inline constexpr address<"Sess_Rules_RemoveInvalidOvertake"> Sess_Rules_RemoveInvalidOvertake{}; // L2 experimental
inline constexpr address<"Sess_Rules_SendBlackFlagTimerToOsd"> Sess_Rules_SendBlackFlagTimerToOsd{}; // L2 experimental
inline constexpr address<"Sess_Rules_SetCarGhostReason"> Sess_Rules_SetCarGhostReason{}; // L2 experimental
inline constexpr address<"Sess_Ruleset_FinaliseResults"> Sess_Ruleset_FinaliseResults{}; // L2 experimental
inline constexpr address<"Sess_Ruleset_FinaliseResultsDefault"> Sess_Ruleset_FinaliseResultsDefault{}; // L2 experimental
inline constexpr address<"Sess_Ruleset_ForceFinishAllDefault"> Sess_Ruleset_ForceFinishAllDefault{}; // L2 experimental
inline constexpr address<"Sess_Ruleset_SetupExtraTime"> Sess_Ruleset_SetupExtraTime{}; // L2 experimental
inline constexpr address<"Sess_Ruleset_UpdateRetiredCarFade"> Sess_Ruleset_UpdateRetiredCarFade{}; // L2 experimental
inline constexpr address<"Sess_SafetyCarDeployment_OnSessionRestart"> Sess_SafetyCarDeployment_OnSessionRestart{}; // L2 experimental
inline constexpr address<"Sess_SafetyCarDeployment_PreSessionRestart"> Sess_SafetyCarDeployment_PreSessionRestart{}; // L2 experimental
inline constexpr address<"Sess_SafetyCarEmergencyGhosting_Init"> Sess_SafetyCarEmergencyGhosting_Init{}; // L2 experimental
inline constexpr address<"Sess_SafetyCarLogic_Subscribe"> Sess_SafetyCarLogic_Subscribe{}; // L2 experimental
inline constexpr address<"Sess_SafetyCarTether_ApplyAssistOverrides"> Sess_SafetyCarTether_ApplyAssistOverrides{}; // L2 experimental
inline constexpr address<"Sess_SafetyCarTether_IsVehicleAttached"> Sess_SafetyCarTether_IsVehicleAttached{}; // L2 experimental
using Sess_SafetyCarTether_UpdateProgress_t = void(void *tether_entry);
inline constexpr fn<"Sess_SafetyCarTether_UpdateProgress", Sess_SafetyCarTether_UpdateProgress_t> Sess_SafetyCarTether_UpdateProgress{}; // L2 experimental
using Sess_SafetyCarTether_UpdateTarget_t = void(void *tether_entry, float gap);
inline constexpr fn<"Sess_SafetyCarTether_UpdateTarget", Sess_SafetyCarTether_UpdateTarget_t> Sess_SafetyCarTether_UpdateTarget{}; // L2 experimental
inline constexpr address<"Sess_SafetyCarTether_UpdateVehicle"> Sess_SafetyCarTether_UpdateVehicle{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_BuildGroupedCarList"> Sess_SafetyCar_BuildGroupedCarList{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_CanVehicleRejoin"> Sess_SafetyCar_CanVehicleRejoin{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_ChangeState"> Sess_SafetyCar_ChangeState{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_ComputeTargetSpeed"> Sess_SafetyCar_ComputeTargetSpeed{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_IntersectionTime"> Sess_SafetyCar_IntersectionTime{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_IsVehicleInDeltaWindow"> Sess_SafetyCar_IsVehicleInDeltaWindow{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_OnLapChanged"> Sess_SafetyCar_OnLapChanged{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_ReadDeploymentFrequency"> Sess_SafetyCar_ReadDeploymentFrequency{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_ReadDynamicObstruction"> Sess_SafetyCar_ReadDynamicObstruction{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_ReadMaxRedFlags"> Sess_SafetyCar_ReadMaxRedFlags{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_UnlapGate"> Sess_SafetyCar_UnlapGate{}; // L2 experimental
inline constexpr address<"Sess_SafetyCar_VehicleTransition"> Sess_SafetyCar_VehicleTransition{}; // L2 experimental
inline constexpr address<"Sess_ScCond_HazardDetected_Update"> Sess_ScCond_HazardDetected_Update{}; // L2 experimental
inline constexpr address<"Sess_ScCond_MinLapsOut_Update"> Sess_ScCond_MinLapsOut_Update{}; // L2 experimental
inline constexpr address<"Sess_ScCond_OffTrack_Update"> Sess_ScCond_OffTrack_Update{}; // L2 experimental
inline constexpr address<"Sess_ScCond_PitAreaClear_Update"> Sess_ScCond_PitAreaClear_Update{}; // L2 experimental
inline constexpr address<"Sess_ScCond_TriggerVsc_Update"> Sess_ScCond_TriggerVsc_Update{}; // L2 experimental
using Sess_ScDebrisAccumulator_Update_t = void(void *acc);
inline constexpr fn<"Sess_ScDebrisAccumulator_Update", Sess_ScDebrisAccumulator_Update_t> Sess_ScDebrisAccumulator_Update{}; // L2 experimental
inline constexpr address<"Sess_ScDebris_SumPartVolumes_B"> Sess_ScDebris_SumPartVolumes_B{}; // L2 experimental
inline constexpr address<"Sess_ScRedFlagDeploy_Update"> Sess_ScRedFlagDeploy_Update{}; // L2 experimental
inline constexpr address<"Sess_ScState_Deploy_Enter"> Sess_ScState_Deploy_Enter{}; // L2 experimental
inline constexpr address<"Sess_ScState_Deploy_ReadTunables"> Sess_ScState_Deploy_ReadTunables{}; // L2 experimental
inline constexpr address<"Sess_ScState_Return_Enter"> Sess_ScState_Return_Enter{}; // L2 experimental
inline constexpr address<"Sess_ScState_ReturnedToPit_Enter"> Sess_ScState_ReturnedToPit_Enter{}; // L2 experimental
inline constexpr address<"Sess_ScState_ReturnedToPit_Exit"> Sess_ScState_ReturnedToPit_Exit{}; // L2 experimental
inline constexpr address<"Sess_ScState_VscDeploy_Enter"> Sess_ScState_VscDeploy_Enter{}; // L2 experimental
inline constexpr address<"Sess_ScState_VscEnding_Enter"> Sess_ScState_VscEnding_Enter{}; // L2 experimental
inline constexpr address<"Sess_ScState_VscEnding_Exit"> Sess_ScState_VscEnding_Exit{}; // L2 experimental
inline constexpr address<"Sess_ScState_Vsc_Enter"> Sess_ScState_Vsc_Enter{}; // L2 experimental
inline constexpr address<"Sess_ScStoppedCar_MapEmplace"> Sess_ScStoppedCar_MapEmplace{}; // L2 experimental
inline constexpr address<"Sess_ScriptHooks_FireSessionEvent"> Sess_ScriptHooks_FireSessionEvent{}; // L2 experimental
inline constexpr address<"Sess_SessionClock_SetTime"> Sess_SessionClock_SetTime{}; // L2 experimental
inline constexpr address<"Sess_SessionExit_OnStateEnter"> Sess_SessionExit_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_SessionStateIntroCameras_OnEnter"> Sess_SessionStateIntroCameras_OnEnter{}; // L2 experimental
inline constexpr address<"Sess_SessionStateMidSessionSaveCutscene_OnEnter"> Sess_SessionStateMidSessionSaveCutscene_OnEnter{}; // L2 experimental
using Sess_SessionTransition_OnStateUpdate_t = uint32_t *(void *state, uint32_t *out_link);
inline constexpr fn<"Sess_SessionTransition_OnStateUpdate", Sess_SessionTransition_OnStateUpdate_t> Sess_SessionTransition_OnStateUpdate{}; // L2 experimental, StateSessionTransition::onStateUpdate
inline constexpr address<"Sess_Session_ResetForRestart"> Sess_Session_ResetForRestart{}; // L2 experimental
inline constexpr address<"Sess_Set_RowsAvailable"> Sess_Set_RowsAvailable{}; // L2 experimental
using Sess_Set_SafetyCarDeploymentType_t = void(void *sc_state, const void *sc_deployment_msg);
inline constexpr fn<"Sess_Set_SafetyCarDeploymentType", Sess_Set_SafetyCarDeploymentType_t> Sess_Set_SafetyCarDeploymentType{}; // L2 experimental
using Sess_Setup_VehicleComponents_t = void(void *module);
inline constexpr fn<"Sess_Setup_VehicleComponents", Sess_Setup_VehicleComponents_t> Sess_Setup_VehicleComponents{}; // L2 experimental
inline constexpr address<"Sess_SortAssignGarages_Thunk"> Sess_SortAssignGarages_Thunk{}; // L2 experimental
inline constexpr address<"Sess_SortEntrantsByGarageOrder"> Sess_SortEntrantsByGarageOrder{}; // L2 experimental
inline constexpr address<"Sess_SpectatorDirector_SetActive"> Sess_SpectatorDirector_SetActive{}; // L2 experimental
inline constexpr address<"Sess_SpectatorDirector_Update"> Sess_SpectatorDirector_Update{}; // L2 experimental
inline constexpr address<"Sess_SpectatorHud_Hide"> Sess_SpectatorHud_Hide{}; // L2 experimental
inline constexpr address<"Sess_SpectatorHud_Show"> Sess_SpectatorHud_Show{}; // L2 experimental
inline constexpr address<"Sess_SpeedLimitPenalty_ReceiveData"> Sess_SpeedLimitPenalty_ReceiveData{}; // L2 experimental
inline constexpr address<"Sess_SpeedLimitPenalty_SendData"> Sess_SpeedLimitPenalty_SendData{}; // L2 experimental
inline constexpr address<"Sess_StartLightsMachine_SetProxy"> Sess_StartLightsMachine_SetProxy{}; // L2 experimental
inline constexpr address<"Sess_StartLightsSync_MustAdvance"> Sess_StartLightsSync_MustAdvance{}; // L2 experimental
inline constexpr address<"Sess_StartLights_Enter"> Sess_StartLights_Enter{}; // L2 experimental
inline constexpr address<"Sess_StartLights_Init"> Sess_StartLights_Init{}; // L2 experimental
inline constexpr address<"Sess_Start_VehiclesInGarages"> Sess_Start_VehiclesInGarages{}; // L2 experimental
inline constexpr address<"Sess_State_EnterOnTrackCommon"> Sess_State_EnterOnTrackCommon{}; // L2 experimental
inline constexpr address<"Sess_State_HandleFlowInterrupt"> Sess_State_HandleFlowInterrupt{}; // L2 experimental
inline constexpr address<"Sess_State_OnPauseRequested"> Sess_State_OnPauseRequested{}; // L2 experimental
inline constexpr address<"Sess_State_Pause"> Sess_State_Pause{}; // L2 experimental
inline constexpr address<"Sess_State_Unpause"> Sess_State_Unpause{}; // L2 experimental
inline constexpr address<"Sess_StopGoPenalty_ReceiveData"> Sess_StopGoPenalty_ReceiveData{}; // L2 experimental
inline constexpr address<"Sess_StopGoPenalty_SendData"> Sess_StopGoPenalty_SendData{}; // L2 experimental
inline constexpr address<"Sess_StopGoPenalty_SendData_2"> Sess_StopGoPenalty_SendData_2{}; // L2 experimental
using Sess_StoryHub_OnStateEnter_t = bool(void *state);
inline constexpr fn<"Sess_StoryHub_OnStateEnter", Sess_StoryHub_OnStateEnter_t> Sess_StoryHub_OnStateEnter{}; // L2 experimental, StateStoryHub::onStateEnter
inline constexpr address<"Sess_StoryPreLights_Update"> Sess_StoryPreLights_Update{}; // L2 experimental
inline constexpr address<"Sess_StoryRollingStart_OnStateEnter"> Sess_StoryRollingStart_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_StoryStartLights_OnStateEnter"> Sess_StoryStartLights_OnStateEnter{}; // L2 experimental
inline constexpr address<"Sess_Strategy_TyreClassTimeLoss"> Sess_Strategy_TyreClassTimeLoss{}; // L2 experimental
inline constexpr address<"Sess_Strategy_UpdateTeammatePairs"> Sess_Strategy_UpdateTeammatePairs{}; // L2 experimental
inline constexpr address<"Sess_TimeOfDayOwner_BindRainCurves"> Sess_TimeOfDayOwner_BindRainCurves{}; // L2 experimental
inline constexpr address<"Sess_TimeOfDayOwner_EvalRainIntensity"> Sess_TimeOfDayOwner_EvalRainIntensity{}; // L2 experimental
inline constexpr address<"Sess_TodStitch_BuildDefaultInstance"> Sess_TodStitch_BuildDefaultInstance{}; // L2 experimental
inline constexpr address<"Sess_TodStitch_GetWetDeltaAtPos"> Sess_TodStitch_GetWetDeltaAtPos{}; // L2 experimental
inline constexpr address<"Sess_TodStitch_MergeInstanceSingle"> Sess_TodStitch_MergeInstanceSingle{}; // L2 experimental
inline constexpr address<"Sess_TodStitch_MergeInstancesMulti"> Sess_TodStitch_MergeInstancesMulti{}; // L2 experimental
using Sess_Tod_BuildSessionDataset_Thunk_t = void(void *tod_owner, const uint8_t *types, const float *hold_s, const float *trans_s, const uint8_t *random, uint32_t count, float len_s, uint16_t session, uint8_t use_count);
inline constexpr fn<"Sess_Tod_BuildSessionDataset_Thunk", Sess_Tod_BuildSessionDataset_Thunk_t> Sess_Tod_BuildSessionDataset_Thunk{}; // L2 experimental
inline constexpr address<"Sess_Tod_BuildWeatherScenario"> Sess_Tod_BuildWeatherScenario{}; // L2 experimental
inline constexpr address<"Sess_Tod_RandomiseTransitions"> Sess_Tod_RandomiseTransitions{}; // L2 experimental
inline constexpr address<"Sess_Tod_ResetDatasetStates"> Sess_Tod_ResetDatasetStates{}; // L2 experimental
inline constexpr address<"Sess_Tod_StitchWeatherScenario"> Sess_Tod_StitchWeatherScenario{}; // L2 experimental
using Sess_TyreAllocation_OnSessionStart_t = void(void *component);
inline constexpr fn<"Sess_TyreAllocation_OnSessionStart", Sess_TyreAllocation_OnSessionStart_t> Sess_TyreAllocation_OnSessionStart{}; // L2 experimental
inline constexpr address<"Sess_TyreAllocation_SwapSets"> Sess_TyreAllocation_SwapSets{}; // L2 experimental
using Sess_TyreClass_FromName_t = void *(void *classes, const void *name, uint8_t check_weekend_format);
inline constexpr fn<"Sess_TyreClass_FromName", Sess_TyreClass_FromName_t> Sess_TyreClass_FromName{}; // L2 experimental
inline constexpr address<"Sess_TyreCompoundClasses_SortEntries"> Sess_TyreCompoundClasses_SortEntries{}; // L2 experimental
inline constexpr address<"Sess_TyreCompoundClasses_SortEntriesB"> Sess_TyreCompoundClasses_SortEntriesB{}; // L2 experimental
using Sess_Tyres_ClosestAvailableCompound_t = uint32_t(void *tyre_rules, uint8_t vehicle, const uint32_t *preference, float wear_scale);
inline constexpr fn<"Sess_Tyres_ClosestAvailableCompound", Sess_Tyres_ClosestAvailableCompound_t> Sess_Tyres_ClosestAvailableCompound{}; // L2 experimental
inline constexpr address<"Sess_Veh_UpdateWheelMap"> Sess_Veh_UpdateWheelMap{}; // L2 experimental
inline constexpr address<"Sess_VehiclePlacement_BroadcastGrid"> Sess_VehiclePlacement_BroadcastGrid{}; // L2 experimental
inline constexpr address<"Sess_VehiclePlacement_FormatFlags"> Sess_VehiclePlacement_FormatFlags{}; // L2 experimental
inline constexpr address<"Sess_VehiclePlacement_HandleGridRequest"> Sess_VehiclePlacement_HandleGridRequest{}; // L2 experimental
inline constexpr address<"Sess_VehiclePlacement_PlaceOnSlot"> Sess_VehiclePlacement_PlaceOnSlot{}; // L2 experimental
inline constexpr address<"Sess_VehiclePlacement_ResolveTransform"> Sess_VehiclePlacement_ResolveTransform{}; // L2 experimental
inline constexpr address<"Sess_VehicleProgress_IsPastLapFraction"> Sess_VehicleProgress_IsPastLapFraction{}; // L2 experimental
using Sess_VehicleTable_EmplaceAt_t = void *(void *table, const void *record);
inline constexpr fn<"Sess_VehicleTable_EmplaceAt", Sess_VehicleTable_EmplaceAt_t> Sess_VehicleTable_EmplaceAt{}; // L2 experimental
inline constexpr address<"Sess_ViewReplay_EnterPhotoMode"> Sess_ViewReplay_EnterPhotoMode{}; // L2 experimental
inline constexpr address<"Sess_VirtualLapsRuleset_FinaliseResults"> Sess_VirtualLapsRuleset_FinaliseResults{}; // L2 experimental
inline constexpr address<"Sess_VirtualLapsRuleset_ForceFinishAll"> Sess_VirtualLapsRuleset_ForceFinishAll{}; // L2 experimental
inline constexpr address<"Sess_VirtualLaps_BuildOrderLists"> Sess_VirtualLaps_BuildOrderLists{}; // L2 experimental
inline constexpr address<"Sess_VirtualLaps_EraseLocalEntries"> Sess_VirtualLaps_EraseLocalEntries{}; // L2 experimental
inline constexpr address<"Sess_VirtualLaps_UpdateVehicleScore"> Sess_VirtualLaps_UpdateVehicleScore{}; // L2 experimental
inline constexpr address<"Sess_VirtualPerformance_CalcPace"> Sess_VirtualPerformance_CalcPace{}; // L2 experimental
inline constexpr address<"Sess_VirtualSession_EstimateLapTime"> Sess_VirtualSession_EstimateLapTime{}; // L2 experimental
inline constexpr address<"Sess_VirtualSession_FinishVirtualising"> Sess_VirtualSession_FinishVirtualising{}; // L2 experimental
inline constexpr address<"Sess_VirtualSession_SimulateEntrantStep"> Sess_VirtualSession_SimulateEntrantStep{}; // L2 experimental
inline constexpr address<"Sess_VirtualSession_Update"> Sess_VirtualSession_Update{}; // L2 experimental
using Sess_WeatherComponent_SetSystemRandomSeed_t = void(void *component);
inline constexpr fn<"Sess_WeatherComponent_SetSystemRandomSeed", Sess_WeatherComponent_SetSystemRandomSeed_t> Sess_WeatherComponent_SetSystemRandomSeed{}; // L2 experimental, WeatherAndTimeOfDayComponent::setWeatherSystemRandomSeed
inline constexpr address<"Sess_Weather_CheckTrackWorldComponents"> Sess_Weather_CheckTrackWorldComponents{}; // L2 experimental
inline constexpr address<"Sess_Weather_GetQualiSegmentStartOffset"> Sess_Weather_GetQualiSegmentStartOffset{}; // L2 experimental
inline constexpr address<"Sess_Weather_PickRandomType"> Sess_Weather_PickRandomType{}; // L2 experimental
inline constexpr address<"Sess_WeekendManager_PrepareTrackLoad"> Sess_WeekendManager_PrepareTrackLoad{}; // L2 experimental
using Sess_WeekendOptions_Refresh_t = void(void *comp, bool force, bool post);
inline constexpr fn<"Sess_WeekendOptions_Refresh", Sess_WeekendOptions_Refresh_t> Sess_WeekendOptions_Refresh{}; // L2 experimental
using Sess_WeekendSetup_LayoutWeekends_t = void(void *comp);
inline constexpr fn<"Sess_WeekendSetup_LayoutWeekends", Sess_WeekendSetup_LayoutWeekends_t> Sess_WeekendSetup_LayoutWeekends{}; // L2 experimental
inline constexpr address<"Sess_Weekend_RebuildUniqueEntrantList"> Sess_Weekend_RebuildUniqueEntrantList{}; // L2 experimental
using Sess_Weekend_Start_t = void(void *weekend, const void *entrants, bool begin);
inline constexpr fn<"Sess_Weekend_Start", Sess_Weekend_Start_t> Sess_Weekend_Start{}; // L2 experimental, game::championship::Weekend::start(const
inline constexpr address<"Sess_WinnerSequence_Enter"> Sess_WinnerSequence_Enter{}; // L2 experimental
inline constexpr address<"Sess_WinnerSequence_IsCarExcluded"> Sess_WinnerSequence_IsCarExcluded{}; // L2 experimental
inline constexpr address<"Sess_WinnerSequence_IsPlayerWinner"> Sess_WinnerSequence_IsPlayerWinner{}; // L2 experimental
using Sess_WinnerSequence_Update_t = uint32_t *(void *state, uint32_t *route, double dt);
inline constexpr fn<"Sess_WinnerSequence_Update", Sess_WinnerSequence_Update_t> Sess_WinnerSequence_Update{}; // L2 experimental
inline constexpr data<"g_Sess_InfringementTypeNames", const char*[6]> g_Sess_InfringementTypeNames{}; // L2 experimental
inline constexpr data<"g_Sess_RedFlagCarStatusNames", const char*[5]> g_Sess_RedFlagCarStatusNames{}; // L2 experimental
inline constexpr data<"g_Sess_ResultStatusNames", const char*[8]> g_Sess_ResultStatusNames{}; // L2 experimental
inline constexpr data<"g_Sess_TyreRulesActive", uint8_t> g_Sess_TyreRulesActive{}; // L2 experimental
inline constexpr data<"g_Sess_TyreRulesMode", uint8_t> g_Sess_TyreRulesMode{}; // L2 experimental
inline constexpr data<"g_Sess_TyreRulesSessionType", uint32_t> g_Sess_TyreRulesSessionType{}; // L2 experimental
inline constexpr data<"g_Sess_VehicleCount", uint32_t> g_Sess_VehicleCount{}; // L2 experimental
inline constexpr data<"g_Sess_VehicleTable", void *[]> g_Sess_VehicleTable{}; // L2 experimental
inline constexpr data<"k_Sess_TyreComponentType", void *> k_Sess_TyreComponentType{}; // L2 experimental
inline constexpr site<"site_Sess_AiRecords_NameHash"> site_Sess_AiRecords_NameHash{}; // L2 experimental
inline constexpr call<"site_Sess_AiVehicleRegisterCall"> site_Sess_AiVehicleRegisterCall{}; // L2 experimental
inline constexpr call<"site_Sess_ControllerSystem_ApplyRecordCall"> site_Sess_ControllerSystem_ApplyRecordCall{}; // L2 experimental
inline constexpr site<"site_Sess_GarageSlots_Done"> site_Sess_GarageSlots_Done{}; // L2 experimental
inline constexpr site<"site_Sess_GarageSlots_GarageLoad"> site_Sess_GarageSlots_GarageLoad{}; // L2 experimental
inline constexpr call<"site_Sess_InGamePlayerFinishedAiCall"> site_Sess_InGamePlayerFinishedAiCall{}; // L2 experimental
inline constexpr site<"site_Sess_InGameRaceRetirementDriver"> site_Sess_InGameRaceRetirementDriver{}; // L2 experimental
inline constexpr site<"site_Sess_InGameRaceRetirementDriverJoin"> site_Sess_InGameRaceRetirementDriverJoin{}; // L2 experimental
inline constexpr call<"site_Sess_InGameSessionCompleteAiCall"> site_Sess_InGameSessionCompleteAiCall{}; // L2 experimental
inline constexpr site<"site_Sess_PitEvalListNext"> site_Sess_PitEvalListNext{}; // L2 experimental
inline constexpr site<"site_Sess_PitEvalListSlot"> site_Sess_PitEvalListSlot{}; // L2 experimental
inline constexpr site<"site_Sess_PitEvalOwnSlot"> site_Sess_PitEvalOwnSlot{}; // L2 experimental
inline constexpr site<"site_Sess_PitEvalReturnFalse"> site_Sess_PitEvalReturnFalse{}; // L2 experimental
inline constexpr site<"site_Sess_PitRankEpilogue"> site_Sess_PitRankEpilogue{}; // L2 experimental
inline constexpr site<"site_Sess_PitRankListNext"> site_Sess_PitRankListNext{}; // L2 experimental
inline constexpr site<"site_Sess_PitRankListSlot"> site_Sess_PitRankListSlot{}; // L2 experimental
inline constexpr site<"site_Sess_PitRankOwnSlot"> site_Sess_PitRankOwnSlot{}; // L2 experimental
inline constexpr site<"site_Sess_PuPanel_SlotHolder"> site_Sess_PuPanel_SlotHolder{}; // L2 experimental
inline constexpr site<"site_Sess_SessionEnter_VehicleLookup"> site_Sess_SessionEnter_VehicleLookup{}; // L2 experimental
inline constexpr site<"site_Sess_TimeTrialLeaderboard_BoundTabChild"> site_Sess_TimeTrialLeaderboard_BoundTabChild{}; // L2 experimental
inline constexpr site<"site_Sess_TimeTrialLeaderboard_BoundTabDone"> site_Sess_TimeTrialLeaderboard_BoundTabDone{}; // L2 experimental
inline constexpr site<"site_Sess_TimeTrialLeaderboard_TabChild"> site_Sess_TimeTrialLeaderboard_TabChild{}; // L2 experimental
inline constexpr site<"site_Sess_TimeTrialLeaderboard_TabDone"> site_Sess_TimeTrialLeaderboard_TabDone{}; // L2 experimental
inline constexpr site<"site_Sess_TyreVisSlotLookup"> site_Sess_TyreVisSlotLookup{}; // L2 experimental
inline constexpr site<"site_Sess_TyreVisTailCall"> site_Sess_TyreVisTailCall{}; // L2 experimental
inline constexpr site<"site_Sess_TyreWearReturn"> site_Sess_TyreWearReturn{}; // L2 experimental
inline constexpr site<"site_Sess_TyreWearSlotLookup"> site_Sess_TyreWearSlotLookup{}; // L2 experimental
inline constexpr site<"site_Sess_VirtualSession_EstimateTail"> site_Sess_VirtualSession_EstimateTail{}; // L2 experimental
inline constexpr site<"site_Sess_VirtualSession_EstimateTyreQuery"> site_Sess_VirtualSession_EstimateTyreQuery{}; // L2 experimental
inline constexpr site<"site_Sess_WheelMapInsertCall"> site_Sess_WheelMapInsertCall{}; // L2 experimental
inline constexpr vtable<"vt_Sess_AiController", 4> vt_Sess_AiController{}; // L2 experimental
inline constexpr vtable<"vt_Sess_CareerHubLocation", 11> vt_Sess_CareerHubLocation{}; // L2 experimental, SessionStateCareerHubLocation
inline constexpr vtable<"vt_Sess_ContractNegotiations", 11> vt_Sess_ContractNegotiations{}; // L2 experimental, StateContractNegotiations
inline constexpr vtable<"vt_Sess_ControllerSystem", 54> vt_Sess_ControllerSystem{}; // L2 experimental
inline constexpr vtable<"vt_Sess_CutsceneLocation", 11> vt_Sess_CutsceneLocation{}; // L2 experimental, SessionStateCutsceneLocation
inline constexpr vtable<"vt_Sess_CutsceneLocationLoad", 11> vt_Sess_CutsceneLocationLoad{}; // L2 experimental, SessionStateCutsceneLocationLoad
inline constexpr vtable<"vt_Sess_CutsceneLocationUnload", 11> vt_Sess_CutsceneLocationUnload{}; // L2 experimental, SessionStateCutsceneLocationUnload
inline constexpr vtable<"vt_Sess_DelayOnLoadingScreen", 11> vt_Sess_DelayOnLoadingScreen{}; // L2 experimental, SessionStateDelayOnLoadingScreen
inline constexpr vtable<"vt_Sess_DriverCareerContractNegotiations", 11> vt_Sess_DriverCareerContractNegotiations{}; // L2 experimental, StateDriverCareerContractNegotiations
inline constexpr vtable<"vt_Sess_DriverCareerContractNegotiationsIntro", 11> vt_Sess_DriverCareerContractNegotiationsIntro{}; // L2 experimental, StateDriverCareerContractNegotiationsIntro
inline constexpr vtable<"vt_Sess_DriverCareerContractNegotiationsOutro", 11> vt_Sess_DriverCareerContractNegotiationsOutro{}; // L2 experimental, StateDriverCareerContractNegotiationsOutro
inline constexpr vtable<"vt_Sess_DriverCareerContractNegotiationsTeamSelect", 11> vt_Sess_DriverCareerContractNegotiationsTeamSelect{}; // L2 experimental, StateDriverCareerContractNegotiationsTeamSelect
inline constexpr vtable<"vt_Sess_DriverHQIntro", 11> vt_Sess_DriverHQIntro{}; // L2 experimental, StateDriverHQIntro
inline constexpr vtable<"vt_Sess_EndOfSeasonCutscene", 11> vt_Sess_EndOfSeasonCutscene{}; // L2 experimental, StateEndOfSeasonCutscene
inline constexpr vtable<"vt_Sess_F1CGIntro", 11> vt_Sess_F1CGIntro{}; // L2 experimental, StateF1CGIntro
inline constexpr vtable<"vt_Sess_F1CGOutro", 11> vt_Sess_F1CGOutro{}; // L2 experimental, StateF1CGOutro
inline constexpr vtable<"vt_Sess_F1CGUpdate", 11> vt_Sess_F1CGUpdate{}; // L2 experimental, StateF1CGUpdate
inline constexpr vtable<"vt_Sess_Frontend", 11> vt_Sess_Frontend{}; // L2 experimental, StateFrontend
inline constexpr vtable<"vt_Sess_FrontendWorld", 11> vt_Sess_FrontendWorld{}; // L2 experimental, StateFrontendWorld
inline constexpr vtable<"vt_Sess_GetEventToken", 11> vt_Sess_GetEventToken{}; // L2 experimental, StateGetEventToken
inline constexpr vtable<"vt_Sess_HumanController", 4> vt_Sess_HumanController{}; // L2 experimental
inline constexpr vtable<"vt_Sess_InGamePlaying", 11> vt_Sess_InGamePlaying{}; // L2 experimental, SessionStateInGamePlaying
inline constexpr vtable<"vt_Sess_Intro", 11> vt_Sess_Intro{}; // L2 experimental, StateIntro
inline constexpr vtable<"vt_Sess_Launch", 11> vt_Sess_Launch{}; // L2 experimental, StateLaunch
inline constexpr vtable<"vt_Sess_LoadSession", 11> vt_Sess_LoadSession{}; // L2 experimental, StateLoadSession
inline constexpr vtable<"vt_Sess_LocationLoad", 11> vt_Sess_LocationLoad{}; // L2 experimental, SessionStateLocationLoad
inline constexpr vtable<"vt_Sess_LocationUnload", 11> vt_Sess_LocationUnload{}; // L2 experimental, SessionStateLocationUnload
inline constexpr vtable<"vt_Sess_OSQSyncReady", 11> vt_Sess_OSQSyncReady{}; // L2 experimental, StateOSQSyncReady
inline constexpr vtable<"vt_Sess_OnlineConnect", 11> vt_Sess_OnlineConnect{}; // L2 experimental, StateOnlineConnect
inline constexpr vtable<"vt_Sess_OnlineFrontend", 11> vt_Sess_OnlineFrontend{}; // L2 experimental, StateOnlineFrontend
inline constexpr vtable<"vt_Sess_OnlineLobby", 11> vt_Sess_OnlineLobby{}; // L2 experimental, StateOnlineLobby
inline constexpr vtable<"vt_Sess_Persistent", 11> vt_Sess_Persistent{}; // L2 experimental, StatePersistent
inline constexpr vtable<"vt_Sess_PlayCutScene", 11> vt_Sess_PlayCutScene{}; // L2 experimental, SessionStatePlayCutScene
inline constexpr vtable<"vt_Sess_PostLoadDataSync", 11> vt_Sess_PostLoadDataSync{}; // L2 experimental, StatePostLoadDataSync
inline constexpr vtable<"vt_Sess_PreRace", 11> vt_Sess_PreRace{}; // L2 experimental, SessionStatePreRace
inline constexpr vtable<"vt_Sess_PressInterview", 11> vt_Sess_PressInterview{}; // L2 experimental, SessionStatePressInterview
inline constexpr vtable<"vt_Sess_PressInterviewBase", 11> vt_Sess_PressInterviewBase{}; // L2 experimental, SessionStatePressInterviewBase
inline constexpr vtable<"vt_Sess_QualifyingSyncReady", 11> vt_Sess_QualifyingSyncReady{}; // L2 experimental, StateQualifyingSyncReady
inline constexpr vtable<"vt_Sess_RaceRuleset", 60> vt_Sess_RaceRuleset{}; // L2 experimental
inline constexpr vtable<"vt_Sess_ReadyMonitor", 11> vt_Sess_ReadyMonitor{}; // L2 experimental, StateReadyMonitor
inline constexpr vtable<"vt_Sess_RedFlagSystem", 30> vt_Sess_RedFlagSystem{}; // L2 experimental, RedFlagSystem
inline constexpr vtable<"vt_Sess_ReplaySessionLoaded", 11> vt_Sess_ReplaySessionLoaded{}; // L2 experimental, SessionStateReplaySessionLoaded
inline constexpr vtable<"vt_Sess_ScAheadOfLead", 11> vt_Sess_ScAheadOfLead{}; // L2 experimental
inline constexpr vtable<"vt_Sess_ScCanReturn", 10> vt_Sess_ScCanReturn{}; // L2 experimental
inline constexpr vtable<"vt_Sess_ScCarsGrouped", 11> vt_Sess_ScCarsGrouped{}; // L2 experimental
inline constexpr vtable<"vt_Sess_ScDebugDeploy", 10> vt_Sess_ScDebugDeploy{}; // L2 experimental
inline constexpr vtable<"vt_Sess_ScForceReturn", 10> vt_Sess_ScForceReturn{}; // L2 experimental
inline constexpr vtable<"vt_Sess_ScRaceLapsRemaining", 10> vt_Sess_ScRaceLapsRemaining{}; // L2 experimental
inline constexpr vtable<"vt_Sess_ScTriggerRedFlag", 10> vt_Sess_ScTriggerRedFlag{}; // L2 experimental
inline constexpr vtable<"vt_Sess_ScTriggerVsc", 10> vt_Sess_ScTriggerVsc{}; // L2 experimental
inline constexpr vtable<"vt_Sess_SecretMeeting", 11> vt_Sess_SecretMeeting{}; // L2 experimental, StateSecretMeeting
inline constexpr vtable<"vt_Sess_SecretMeetingOutro", 11> vt_Sess_SecretMeetingOutro{}; // L2 experimental, StateSecretMeetingOutro
inline constexpr vtable<"vt_Sess_Session", 11> vt_Sess_Session{}; // L2 experimental, StateSession
inline constexpr vtable<"vt_Sess_SessionBaseState", 11> vt_Sess_SessionBaseState{}; // L2 experimental, SessionBaseState
inline constexpr vtable<"vt_Sess_SessionTransition", 11> vt_Sess_SessionTransition{}; // L2 experimental, StateSessionTransition
inline constexpr vtable<"vt_Sess_SillySeasonCutscene", 11> vt_Sess_SillySeasonCutscene{}; // L2 experimental, StateSillySeasonCutscene
inline constexpr vtable<"vt_Sess_SmokeTestSkipFEPreLaunchHold", 11> vt_Sess_SmokeTestSkipFEPreLaunchHold{}; // L2 experimental, StateSmokeTestSkipFEPreLaunchHold
inline constexpr vtable<"vt_Sess_SpectatorReadyMonitor", 11> vt_Sess_SpectatorReadyMonitor{}; // L2 experimental, StateSpectatorReadyMonitor
inline constexpr vtable<"vt_Sess_StartLoadingFlow", 11> vt_Sess_StartLoadingFlow{}; // L2 experimental, SessionStateStartLoadingFlow
inline constexpr vtable<"vt_Sess_StoryHub", 11> vt_Sess_StoryHub{}; // L2 experimental, StateStoryHub
inline constexpr vtable<"vt_Sess_StoryHubDepartmentEvents", 11> vt_Sess_StoryHubDepartmentEvents{}; // L2 experimental, StateStoryHubDepartmentEvents
inline constexpr vtable<"vt_Sess_StoryHubDepartmentEventsBase", 11> vt_Sess_StoryHubDepartmentEventsBase{}; // L2 experimental, StateStoryHubDepartmentEventsBase
inline constexpr vtable<"vt_Sess_StoryHubDepartmentEventsIntro", 11> vt_Sess_StoryHubDepartmentEventsIntro{}; // L2 experimental, StateStoryHubDepartmentEventsIntro
inline constexpr vtable<"vt_Sess_StoryHubDepartmentEventsOutro", 11> vt_Sess_StoryHubDepartmentEventsOutro{}; // L2 experimental, StateStoryHubDepartmentEventsOutro
inline constexpr vtable<"vt_Sess_SynchroniseCloseDocument", 11> vt_Sess_SynchroniseCloseDocument{}; // L2 experimental, StateSynchroniseCloseDocument
inline constexpr vtable<"vt_Sess_TeamHQ", 11> vt_Sess_TeamHQ{}; // L2 experimental, StateTeamHQ
inline constexpr vtable<"vt_Sess_TeamHQToSession", 11> vt_Sess_TeamHQToSession{}; // L2 experimental, StateTeamHQToSession
inline constexpr vtable<"vt_Sess_TeamHQUpdate", 11> vt_Sess_TeamHQUpdate{}; // L2 experimental, StateTeamHQUpdate
inline constexpr vtable<"vt_Sess_TyreAllocation", 11> vt_Sess_TyreAllocation{}; // L2 experimental, StateTyreAllocation
inline constexpr vtable<"vt_Sess_UnloadRewardSequence", 11> vt_Sess_UnloadRewardSequence{}; // L2 experimental, SessionStateUnloadRewardSequence
inline constexpr vtable<"vt_Sess_Video", 11> vt_Sess_Video{}; // L2 experimental, SessionStateVideo

}
