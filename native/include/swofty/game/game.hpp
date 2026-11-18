#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct ActivityDef {
    uint8_t _pad_0x0[0x10];
    uint8_t type; // L2 experimental
};
static_assert(offsetof(ActivityDef, type) == 0x10);

struct CalendarActivity {
    uint8_t _pad_0x0[0xa];
    uint16_t day; // L2 experimental
    uint8_t _pad_0xc[0x4];
    ActivityDef *activity; // L2 experimental
    uint8_t _pad_0x18[0x8];
};
static_assert(offsetof(CalendarActivity, day) == 0xa);
static_assert(offsetof(CalendarActivity, activity) == 0x10);
static_assert(sizeof(CalendarActivity) == 0x20);

struct ChampionshipHolder {
    uint8_t _pad_0x0[0x260];
    CareerEntity *entity; // L2 experimental
};
static_assert(offsetof(ChampionshipHolder, entity) == 0x260);

struct DriverCalendar {
    uint8_t _pad_0x0[0x50];
    void *activities_begin; // L2 experimental
    void *activities_end; // L2 experimental
    uint8_t _pad_0x60[0x60];
    uint32_t driver; // L2 experimental
    uint8_t _pad_0xc4[0x4];
};
static_assert(offsetof(DriverCalendar, activities_begin) == 0x50);
static_assert(offsetof(DriverCalendar, activities_end) == 0x58);
static_assert(offsetof(DriverCalendar, driver) == 0xc0);
static_assert(sizeof(DriverCalendar) == 0xc8);

struct DriverCareerTeamSelectViewModel {
    uint8_t _pad_0x0[0xf20];
    uint32_t selected_team_key; // L2 experimental
};
static_assert(offsetof(DriverCareerTeamSelectViewModel, selected_team_key) == 0xf20);

struct DriverSecretMeetings {
    uint8_t _pad_0x0[0x10];
    void *current_allocator; // L2 experimental
    SecretMeeting *current; // L2 experimental
    uint8_t _pad_0x20[0x8];
    void *previous_begin; // L2 experimental
    void *previous_end; // L2 experimental
    uint8_t _pad_0x38[0x8];
    uint32_t driver; // L2 experimental
    uint8_t _pad_0x44[0x4];
};
static_assert(offsetof(DriverSecretMeetings, current_allocator) == 0x10);
static_assert(offsetof(DriverSecretMeetings, current) == 0x18);
static_assert(offsetof(DriverSecretMeetings, previous_begin) == 0x28);
static_assert(offsetof(DriverSecretMeetings, previous_end) == 0x30);
static_assert(offsetof(DriverSecretMeetings, driver) == 0x40);
static_assert(sizeof(DriverSecretMeetings) == 0x48);

struct GameManager {
    uint8_t _pad_0x0[0xc0];
    GameModeContext *mode_context; // L2 experimental
};
static_assert(offsetof(GameManager, mode_context) == 0xc0);

struct GameMessageBusHolder {
    uint8_t _pad_0x0[0x8];
    NeMessageBus *bus; // L2 experimental
};
static_assert(offsetof(GameMessageBusHolder, bus) == 0x8);

struct GameModeContext {
    uint8_t _pad_0x0[0x88];
    ChampionshipHolder *championship_holder; // L2 experimental
};
static_assert(offsetof(GameModeContext, championship_holder) == 0x88);

struct SaveSlotInfo {
    uint64_t slot_id; // L2 experimental
    uint64_t slot_guid; // L2 experimental
    uint8_t _pad_0x10[0x10];
    char *name; // L2 experimental
    uint8_t _pad_0x28[0x80];
    uint8_t game_setup_mode; // L2 experimental
    uint8_t _pad_0xa9[0x3];
    uint32_t info_season; // L2 experimental
    uint32_t info_weekend; // L2 experimental
    uint8_t _pad_0xb4[0xa];
    uint16_t info_team; // L2 experimental
    uint16_t info_driver; // L2 experimental
    uint16_t info_vehicle; // L2 experimental
    uint8_t _pad_0xc4[0x4];
    uint64_t info_date; // L2 experimental
    uint8_t _pad_0xd0[0xa1];
    uint8_t type_flags; // L2 experimental
};
static_assert(offsetof(SaveSlotInfo, slot_id) == 0x0);
static_assert(offsetof(SaveSlotInfo, slot_guid) == 0x8);
static_assert(offsetof(SaveSlotInfo, name) == 0x20);
static_assert(offsetof(SaveSlotInfo, game_setup_mode) == 0xa8);
static_assert(offsetof(SaveSlotInfo, info_season) == 0xac);
static_assert(offsetof(SaveSlotInfo, info_weekend) == 0xb0);
static_assert(offsetof(SaveSlotInfo, info_team) == 0xbe);
static_assert(offsetof(SaveSlotInfo, info_driver) == 0xc0);
static_assert(offsetof(SaveSlotInfo, info_vehicle) == 0xc2);
static_assert(offsetof(SaveSlotInfo, info_date) == 0xc8);
static_assert(offsetof(SaveSlotInfo, type_flags) == 0x171);

struct SaveSlotManager {
    uint8_t _pad_0x0[0x268];
    SaveSlotInfo **slots_begin; // L2 experimental
    SaveSlotInfo **slots_end; // L2 experimental
    uint8_t _pad_0x278[0x8];
    SaveSlotInfo *current; // L2 experimental
};
static_assert(offsetof(SaveSlotManager, slots_begin) == 0x268);
static_assert(offsetof(SaveSlotManager, slots_end) == 0x270);
static_assert(offsetof(SaveSlotManager, current) == 0x280);

struct ScheduleComponent {
    uint8_t _pad_0x0[0x410];
    ScheduleSeason *seasons_begin; // L2 experimental
    ScheduleSeason *seasons_end; // L2 experimental
};
static_assert(offsetof(ScheduleComponent, seasons_begin) == 0x410);
static_assert(offsetof(ScheduleComponent, seasons_end) == 0x418);

struct ScheduleSeason {
    uint8_t _pad_0x0[0x40];
    DriverCalendar *driver_calendars_begin; // L2 experimental
    DriverCalendar *driver_calendars_end; // L2 experimental
    uint8_t _pad_0x50[0x30];
};
static_assert(offsetof(ScheduleSeason, driver_calendars_begin) == 0x40);
static_assert(offsetof(ScheduleSeason, driver_calendars_end) == 0x48);
static_assert(sizeof(ScheduleSeason) == 0x80);

struct SecretMeeting {
    uint32_t id; // L2 experimental
    uint8_t _pad_0x4[0x10];
    uint8_t reason; // L2 experimental
    uint8_t _pad_0x15[0xb];
    uint32_t team; // L2 experimental
};
static_assert(offsetof(SecretMeeting, id) == 0x0);
static_assert(offsetof(SecretMeeting, reason) == 0x14);
static_assert(offsetof(SecretMeeting, team) == 0x20);
static_assert(sizeof(SecretMeeting) == 0x24);

struct TodCurveBinding {
};

struct TrackDataRecord {
    uint8_t _pad_0x0[0x1];
    uint8_t index; // L2 experimental
    uint8_t _pad_0x2[0xce];
    char *name; // L2 experimental
    uint8_t _pad_0xd8[0xc30];
};
static_assert(offsetof(TrackDataRecord, index) == 0x1);
static_assert(offsetof(TrackDataRecord, name) == 0xd0);
static_assert(sizeof(TrackDataRecord) == 0xd08);

struct TrackDataSet {
    uint8_t _pad_0x0[0x8];
    void *tracks_begin; // L2 experimental
    void *tracks_end; // L2 experimental
    uint8_t _pad_0x18[0x8];
    uint32_t mode_mask; // L2 experimental
    uint32_t season_mask; // L2 experimental
    uint32_t format_mask; // L2 experimental
    uint8_t _pad_0x2c[0x4];
};
static_assert(offsetof(TrackDataSet, tracks_begin) == 0x8);
static_assert(offsetof(TrackDataSet, tracks_end) == 0x10);
static_assert(offsetof(TrackDataSet, mode_mask) == 0x20);
static_assert(offsetof(TrackDataSet, season_mask) == 0x24);
static_assert(offsetof(TrackDataSet, format_mask) == 0x28);
static_assert(sizeof(TrackDataSet) == 0x30);

struct TrackDataSystem {
    uint8_t _pad_0x0[0x10];
    void *arena; // L2 experimental
    uint8_t _pad_0x18[0x10];
    void *records_begin; // L2 experimental
    void *records_end; // L2 experimental
    uint8_t _pad_0x38[0x30];
    TrackDataSet *sets_begin; // L2 experimental
    TrackDataSet *sets_end; // L2 experimental
};
static_assert(offsetof(TrackDataSystem, arena) == 0x10);
static_assert(offsetof(TrackDataSystem, records_begin) == 0x28);
static_assert(offsetof(TrackDataSystem, records_end) == 0x30);
static_assert(offsetof(TrackDataSystem, sets_begin) == 0x68);
static_assert(offsetof(TrackDataSystem, sets_end) == 0x70);

struct TrackDataTrack {
    TrackDataRecord *record; // L2 experimental
    uint8_t _pad_0x8[0x58];
    float chance_of_using_softer_preset; // L2 experimental
    float chance_of_using_harder_preset; // L2 experimental
    uint8_t _pad_0x68[0x70];
    uint8_t enforce_pitstops_in_wet_weather; // L2 experimental
    uint8_t _pad_0xd9[0x7];
    void *number_of_laps_slot; // L2 experimental
    uint8_t _pad_0xe8[0x8];
    float fuel_burn_per_lap; // L2 experimental
    uint8_t _pad_0xf4[0x9];
    uint8_t is_sprint_weekend; // L2 experimental
    uint8_t _pad_0xfe[0x2];
    uint8_t race_fuel[156]; // L2 experimental
    uint8_t _pad_0x19c[0x4];
    uint64_t race_fuel_count; // L2 experimental
    uint8_t _pad_0x1a8[0xc];
    float qualifying_one_shot; // L2 experimental
    uint8_t _pad_0x1b8[0x4];
    float qualifying_short; // L2 experimental
    float qualifying_full; // L2 experimental
    uint8_t _pad_0x1c4[0x34];
    int32_t race_start_time[2]; // L2 experimental
    uint8_t _pad_0x200[0x1c];
    float pit_speed_limit; // L2 experimental
    float pit_stop_time_dry; // L2 experimental
    uint8_t _pad_0x224[0x6c];
};
static_assert(offsetof(TrackDataTrack, record) == 0x0);
static_assert(offsetof(TrackDataTrack, chance_of_using_softer_preset) == 0x60);
static_assert(offsetof(TrackDataTrack, chance_of_using_harder_preset) == 0x64);
static_assert(offsetof(TrackDataTrack, enforce_pitstops_in_wet_weather) == 0xd8);
static_assert(offsetof(TrackDataTrack, number_of_laps_slot) == 0xe0);
static_assert(offsetof(TrackDataTrack, fuel_burn_per_lap) == 0xf0);
static_assert(offsetof(TrackDataTrack, is_sprint_weekend) == 0xfd);
static_assert(offsetof(TrackDataTrack, race_fuel) == 0x100);
static_assert(offsetof(TrackDataTrack, race_fuel_count) == 0x1a0);
static_assert(offsetof(TrackDataTrack, qualifying_one_shot) == 0x1b4);
static_assert(offsetof(TrackDataTrack, qualifying_short) == 0x1bc);
static_assert(offsetof(TrackDataTrack, qualifying_full) == 0x1c0);
static_assert(offsetof(TrackDataTrack, race_start_time) == 0x1f8);
static_assert(offsetof(TrackDataTrack, pit_speed_limit) == 0x21c);
static_assert(offsetof(TrackDataTrack, pit_stop_time_dry) == 0x220);
static_assert(sizeof(TrackDataTrack) == 0x290);

inline constexpr address<"Game_AccelerationDecelerationForcesCameraModifier_OnEnable"> Game_AccelerationDecelerationForcesCameraModifier_OnEnable{}; // L2 experimental
inline constexpr address<"Game_ActorAssetGroupManager_Update"> Game_ActorAssetGroupManager_Update{}; // L2 experimental
inline constexpr address<"Game_ActorManager_Initialise"> Game_ActorManager_Initialise{}; // L2 experimental
inline constexpr address<"Game_AdaptiveAi_RecordSessionFinished"> Game_AdaptiveAi_RecordSessionFinished{}; // L2 experimental
inline constexpr address<"Game_AiManager_Initialise"> Game_AiManager_Initialise{}; // L2 experimental
inline constexpr address<"Game_AiPitDecision_UpdateDroppedPlayerCar"> Game_AiPitDecision_UpdateDroppedPlayerCar{}; // L2 experimental
using Game_AiRoute_BuildGraph_t = uint64_t(void *ai_manager);
inline constexpr fn<"Game_AiRoute_BuildGraph", Game_AiRoute_BuildGraph_t> Game_AiRoute_BuildGraph{}; // L2 experimental
using Game_AiRoute_CompileMarkerChannel_t = void(void *route, uint32_t *out_buf);
inline constexpr fn<"Game_AiRoute_CompileMarkerChannel", Game_AiRoute_CompileMarkerChannel_t> Game_AiRoute_CompileMarkerChannel{}; // L2 experimental
inline constexpr address<"Game_Analytics_BakedLiverySponsor_WriteJson"> Game_Analytics_BakedLiverySponsor_WriteJson{}; // L2 experimental
using Game_Analytics_Json_WriteFloatField_t = void(void *writer, const char *prefix, float value);
inline constexpr fn<"Game_Analytics_Json_WriteFloatField", Game_Analytics_Json_WriteFloatField_t> Game_Analytics_Json_WriteFloatField{}; // L2 experimental
using Game_Analytics_Json_WriteStringField_t = void(void *writer, const char *key, const char *value);
inline constexpr fn<"Game_Analytics_Json_WriteStringField", Game_Analytics_Json_WriteStringField_t> Game_Analytics_Json_WriteStringField{}; // L2 experimental
using Game_Analytics_RecordMyTeamSecondDriverContract_t = bool(void *analytics, void *contract_event);
inline constexpr fn<"Game_Analytics_RecordMyTeamSecondDriverContract", Game_Analytics_RecordMyTeamSecondDriverContract_t> Game_Analytics_RecordMyTeamSecondDriverContract{}; // L2 experimental
inline constexpr address<"Game_Analytics_UnregisterMessageHandlers"> Game_Analytics_UnregisterMessageHandlers{}; // L2 experimental
using Game_Analytics_WriteBaseCtx_t = void(void *base_ctx, void *json_writer);
inline constexpr fn<"Game_Analytics_WriteBaseCtx", Game_Analytics_WriteBaseCtx_t> Game_Analytics_WriteBaseCtx{}; // L2 experimental
using Game_Analytics_WriteCommonHeaderFields_t = void(void *record, void *json_writer, void *ctx);
inline constexpr fn<"Game_Analytics_WriteCommonHeaderFields", Game_Analytics_WriteCommonHeaderFields_t> Game_Analytics_WriteCommonHeaderFields{}; // L2 experimental
inline constexpr address<"Game_Analytics_WriteRndUpgradeInfo"> Game_Analytics_WriteRndUpgradeInfo{}; // L2 experimental
inline constexpr address<"Game_AnimLayers_EvaluateBlend"> Game_AnimLayers_EvaluateBlend{}; // L2 experimental
inline constexpr address<"Game_AnimLayers_EvaluateFallback"> Game_AnimLayers_EvaluateFallback{}; // L2 experimental
inline constexpr address<"Game_AnimLayers_EvaluateSingle"> Game_AnimLayers_EvaluateSingle{}; // L2 experimental
inline constexpr address<"Game_AnimatedDriver_Initialise"> Game_AnimatedDriver_Initialise{}; // L2 experimental
using Game_Apply_PresetEqualPerformance_t = void(void *game_mode, const char *prefix);
inline constexpr fn<"Game_Apply_PresetEqualPerformance", Game_Apply_PresetEqualPerformance_t> Game_Apply_PresetEqualPerformance{}; // L2 experimental
using Game_Apply_PresetSetupMode_t = void(void *game_mode, const char *prefix);
inline constexpr fn<"Game_Apply_PresetSetupMode", Game_Apply_PresetSetupMode_t> Game_Apply_PresetSetupMode{}; // L2 experimental
using Game_Apply_PresetTeamName_t = void(void *game_mode, const char *prefix);
inline constexpr fn<"Game_Apply_PresetTeamName", Game_Apply_PresetTeamName_t> Game_Apply_PresetTeamName{}; // L2 experimental
using Game_Apply_PresetTimeOfDay_t = void(void *game_mode, const char *prefix);
inline constexpr fn<"Game_Apply_PresetTimeOfDay", Game_Apply_PresetTimeOfDay_t> Game_Apply_PresetTimeOfDay{}; // L2 experimental
using Game_Apply_PresetTrackName_t = void(void *game_mode, const char *prefix);
inline constexpr fn<"Game_Apply_PresetTrackName", Game_Apply_PresetTrackName_t> Game_Apply_PresetTrackName{}; // L2 experimental
using Game_Apply_PresetWeather_t = void(void *game_mode, const char *prefix);
inline constexpr fn<"Game_Apply_PresetWeather", Game_Apply_PresetWeather_t> Game_Apply_PresetWeather{}; // L2 experimental
inline constexpr address<"Game_AudioAutodetect_Initialise"> Game_AudioAutodetect_Initialise{}; // L2 experimental
inline constexpr address<"Game_AudioEngineBundle_Initialise"> Game_AudioEngineBundle_Initialise{}; // L2 experimental
inline constexpr address<"Game_AutoResetToTrack_StartTimer"> Game_AutoResetToTrack_StartTimer{}; // L2 experimental
inline constexpr address<"Game_AutoResetToTrack_Teleport"> Game_AutoResetToTrack_Teleport{}; // L2 experimental
inline constexpr address<"Game_BenchmarkGameMode_BuildSessionSettings"> Game_BenchmarkGameMode_BuildSessionSettings{}; // L2 experimental
inline constexpr address<"Game_BetaCode_OnEntered"> Game_BetaCode_OnEntered{}; // L2 experimental
inline constexpr address<"Game_Breakable_SpawnPieceRenderModel"> Game_Breakable_SpawnPieceRenderModel{}; // L2 experimental
inline constexpr address<"Game_BroadcastViewVehicleVM_OnActivate"> Game_BroadcastViewVehicleVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_BroadcastViewVehicleVM_OnDeactivate"> Game_BroadcastViewVehicleVM_OnDeactivate{}; // L2 experimental
using Game_Build_PassiveObjectiveKeyName_t = void *(void *out, int index);
inline constexpr fn<"Game_Build_PassiveObjectiveKeyName", Game_Build_PassiveObjectiveKeyName_t> Game_Build_PassiveObjectiveKeyName{}; // L2 experimental
inline constexpr address<"Game_BusySpinner_StopAndFadeOutUi"> Game_BusySpinner_StopAndFadeOutUi{}; // L2 experimental
inline constexpr address<"Game_CameraActor_ProcessPendingCameraChanges"> Game_CameraActor_ProcessPendingCameraChanges{}; // L2 experimental
inline constexpr address<"Game_CameraActor_SyncClipTarget"> Game_CameraActor_SyncClipTarget{}; // L2 experimental
inline constexpr address<"Game_CameraCollection_FindByName"> Game_CameraCollection_FindByName{}; // L2 experimental
inline constexpr address<"Game_CameraCollection_FindByUid"> Game_CameraCollection_FindByUid{}; // L2 experimental
using Game_CameraHelicopter_Reset_t = void(void *self);
inline constexpr fn<"Game_CameraHelicopter_Reset", Game_CameraHelicopter_Reset_t> Game_CameraHelicopter_Reset{}; // L2 experimental
inline constexpr address<"Game_CarCustomisation_ApplyAllCustomisation"> Game_CarCustomisation_ApplyAllCustomisation{}; // L2 experimental
inline constexpr address<"Game_CarCustomisation_ApplyDriverNumbers"> Game_CarCustomisation_ApplyDriverNumbers{}; // L2 experimental
inline constexpr address<"Game_CarCustomisation_ApplyLiveryPreview"> Game_CarCustomisation_ApplyLiveryPreview{}; // L2 experimental
inline constexpr address<"Game_CarCustomisation_MarkCarDirtyOff"> Game_CarCustomisation_MarkCarDirtyOff{}; // L2 experimental
inline constexpr address<"Game_CarCustomisation_SetupWritableCamera"> Game_CarCustomisation_SetupWritableCamera{}; // L2 experimental
inline constexpr address<"Game_CarDecals_BuildList"> Game_CarDecals_BuildList{}; // L2 experimental
using Game_CareerActivities_AddActivity_t = void(void *calendar, const void *activity, uint32_t day);
inline constexpr fn<"Game_CareerActivities_AddActivity", Game_CareerActivities_AddActivity_t> Game_CareerActivities_AddActivity{}; // L2 experimental
using Game_CareerActivities_RemoveActivity_t = void(void *calendar, const void *activity, uint16_t day);
inline constexpr fn<"Game_CareerActivities_RemoveActivity", Game_CareerActivities_RemoveActivity_t> Game_CareerActivities_RemoveActivity{}; // L2 experimental
using Game_CareerContinue_HandleCommand_t = uint64_t(void *vm, NeStr **cmd, void *value);
inline constexpr fn<"Game_CareerContinue_HandleCommand", Game_CareerContinue_HandleCommand_t> Game_CareerContinue_HandleCommand{}; // L2 experimental
using Game_CareerContinue_StartNewCareer_t = void(void *vm);
inline constexpr fn<"Game_CareerContinue_StartNewCareer", Game_CareerContinue_StartNewCareer_t> Game_CareerContinue_StartNewCareer{}; // L2 experimental
inline constexpr address<"Game_CareerEventResourceManager_DeactivateExtraWorld"> Game_CareerEventResourceManager_DeactivateExtraWorld{}; // L2 experimental
inline constexpr address<"Game_CareerEventResourceManager_ShutdownExtraWorlds"> Game_CareerEventResourceManager_ShutdownExtraWorlds{}; // L2 experimental
inline constexpr address<"Game_CareerEvent_OnEventKey"> Game_CareerEvent_OnEventKey{}; // L2 experimental
inline constexpr address<"Game_CareerEvent_OnSequenceEvent"> Game_CareerEvent_OnSequenceEvent{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_CloseLaptop"> Game_CareerEventsSequencer_CloseLaptop{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_Destroy"> Game_CareerEventsSequencer_Destroy{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_Initialise"> Game_CareerEventsSequencer_Initialise{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_IsPausableA"> Game_CareerEventsSequencer_IsPausableA{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_IsPausableB"> Game_CareerEventsSequencer_IsPausableB{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_OnFadedOutForExit"> Game_CareerEventsSequencer_OnFadedOutForExit{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_OnHubClosed"> Game_CareerEventsSequencer_OnHubClosed{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_OpenLaptop"> Game_CareerEventsSequencer_OpenLaptop{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_SetAllEnabled"> Game_CareerEventsSequencer_SetAllEnabled{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_SetLoadingEnabled"> Game_CareerEventsSequencer_SetLoadingEnabled{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_SpawnDriverScene"> Game_CareerEventsSequencer_SpawnDriverScene{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_TriggerFadeOutForExit"> Game_CareerEventsSequencer_TriggerFadeOutForExit{}; // L2 experimental
inline constexpr address<"Game_CareerEventsSequencer_UnloadAll"> Game_CareerEventsSequencer_UnloadAll{}; // L2 experimental
inline constexpr address<"Game_CareerHubRndAreaScript_Initialise"> Game_CareerHubRndAreaScript_Initialise{}; // L2 experimental
using Game_CareerOnlineDocumentComponent_Initialise_t = uint32_t(void *self, void *ctx, void *components);
inline constexpr fn<"Game_CareerOnlineDocumentComponent_Initialise", Game_CareerOnlineDocumentComponent_Initialise_t> Game_CareerOnlineDocumentComponent_Initialise{}; // L2 experimental
using Game_CareerSetup_ResetEntrants_t = void(void *setup);
inline constexpr fn<"Game_CareerSetup_ResetEntrants", Game_CareerSetup_ResetEntrants_t> Game_CareerSetup_ResetEntrants{}; // L2 experimental
using Game_CareerWizardAddonDrivers_HandleChoice_t = void(void *vm, uint32_t cmdHash);
inline constexpr fn<"Game_CareerWizardAddonDrivers_HandleChoice", Game_CareerWizardAddonDrivers_HandleChoice_t> Game_CareerWizardAddonDrivers_HandleChoice{}; // L2 experimental
using Game_CareerWizard_PushNextStep_t = void(void *wizard);
inline constexpr fn<"Game_CareerWizard_PushNextStep", Game_CareerWizard_PushNextStep_t> Game_CareerWizard_PushNextStep{}; // L2 experimental
inline constexpr address<"Game_CareerWizard_ResolveOrExit"> Game_CareerWizard_ResolveOrExit{}; // L2 experimental
using Game_CareerWizard_StartsInF1_t = bool(void *entrants, void *wizard_data, const char *entrant_list);
inline constexpr fn<"Game_CareerWizard_StartsInF1", Game_CareerWizard_StartsInF1_t> Game_CareerWizard_StartsInF1{}; // L2 experimental
inline constexpr address<"Game_Career_FadeOutForSessionRequest"> Game_Career_FadeOutForSessionRequest{}; // L2 experimental
inline constexpr address<"Game_Career_HideDriversOnExit"> Game_Career_HideDriversOnExit{}; // L2 experimental
using Game_Career_IsDriverCareerMode_t = bool(const void *championship);
inline constexpr fn<"Game_Career_IsDriverCareerMode", Game_Career_IsDriverCareerMode_t> Game_Career_IsDriverCareerMode{}; // L2 experimental
inline constexpr address<"Game_Career_SpawnMainDriverPose"> Game_Career_SpawnMainDriverPose{}; // L2 experimental
using Game_ChallengeCareer_BuildTrackTiles_t = void(void *vm);
inline constexpr fn<"Game_ChallengeCareer_BuildTrackTiles", Game_ChallengeCareer_BuildTrackTiles_t> Game_ChallengeCareer_BuildTrackTiles{}; // L2 experimental
inline constexpr address<"Game_ChallengeEvent_HandleCommand"> Game_ChallengeEvent_HandleCommand{}; // L2 experimental
inline constexpr address<"Game_ChallengeEvent_OnTabActivated"> Game_ChallengeEvent_OnTabActivated{}; // L2 experimental
inline constexpr address<"Game_ChallengeEvent_OnTabDeactivated"> Game_ChallengeEvent_OnTabDeactivated{}; // L2 experimental
inline constexpr address<"Game_ChampSession_Reset"> Game_ChampSession_Reset{}; // L2 experimental
inline constexpr address<"Game_ChampionshipGameMode_BuildSessionSettings"> Game_ChampionshipGameMode_BuildSessionSettings{}; // L2 experimental
inline constexpr address<"Game_Championship_AddSessionDefaultLength"> Game_Championship_AddSessionDefaultLength{}; // L2 experimental
inline constexpr address<"Game_Championship_AddSessionFromDef"> Game_Championship_AddSessionFromDef{}; // L2 experimental
inline constexpr address<"Game_Championship_AddSessionWithFormat"> Game_Championship_AddSessionWithFormat{}; // L2 experimental
inline constexpr address<"Game_Championship_ResetSessionsForRestart"> Game_Championship_ResetSessionsForRestart{}; // L2 experimental
using Game_Championship_VirtualisePendingSessions_t = uint64_t(void *self);
inline constexpr fn<"Game_Championship_VirtualisePendingSessions", Game_Championship_VirtualisePendingSessions_t> Game_Championship_VirtualisePendingSessions{}; // L2 experimental
using Game_Check_ReturnToGarageTimeLimit_t = bool(void *self);
inline constexpr fn<"Game_Check_ReturnToGarageTimeLimit", Game_Check_ReturnToGarageTimeLimit_t> Game_Check_ReturnToGarageTimeLimit{}; // L2 experimental
inline constexpr address<"Game_Check_TutorialStepTriggerPresent"> Game_Check_TutorialStepTriggerPresent{}; // L2 experimental
inline constexpr address<"Game_CinematicDirector_SendCinematicData"> Game_CinematicDirector_SendCinematicData{}; // L2 experimental
inline constexpr address<"Game_CinematicPitStop_Start"> Game_CinematicPitStop_Start{}; // L2 experimental
inline constexpr address<"Game_CinematicPitStop_Stop"> Game_CinematicPitStop_Stop{}; // L2 experimental
inline constexpr address<"Game_CinematicSafetyCar_FadeIn"> Game_CinematicSafetyCar_FadeIn{}; // L2 experimental
inline constexpr address<"Game_CinematicSafetyCar_FadeOut"> Game_CinematicSafetyCar_FadeOut{}; // L2 experimental
inline constexpr address<"Game_CinematicSafetyCar_HideHud"> Game_CinematicSafetyCar_HideHud{}; // L2 experimental
inline constexpr address<"Game_CinematicSafetyCar_RunThenFadeIn"> Game_CinematicSafetyCar_RunThenFadeIn{}; // L2 experimental
inline constexpr address<"Game_CinematicSafetyCar_Stop"> Game_CinematicSafetyCar_Stop{}; // L2 experimental
inline constexpr address<"Game_CollisionBasedShakeCameraModifier_OnEnable"> Game_CollisionBasedShakeCameraModifier_OnEnable{}; // L2 experimental
using Game_ContractTeamSelect_OnEnter_t = uint64_t(void *state);
inline constexpr fn<"Game_ContractTeamSelect_OnEnter", Game_ContractTeamSelect_OnEnter_t> Game_ContractTeamSelect_OnEnter{}; // L2 experimental
using Game_ContractTeamSelect_OnExit_t = uint64_t(void *state);
inline constexpr fn<"Game_ContractTeamSelect_OnExit", Game_ContractTeamSelect_OnExit_t> Game_ContractTeamSelect_OnExit{}; // L2 experimental
using Game_ContractTeamSelect_OnUpdate_t = uint32_t *(void *state, uint32_t *link);
inline constexpr fn<"Game_ContractTeamSelect_OnUpdate", Game_ContractTeamSelect_OnUpdate_t> Game_ContractTeamSelect_OnUpdate{}; // L2 experimental
inline constexpr address<"Game_ControllerState_SyncHudHideReason13"> Game_ControllerState_SyncHudHideReason13{}; // L2 experimental
using Game_CreateSessionDlcSelectTab_BuildTiles_t = void(void *view_model);
inline constexpr fn<"Game_CreateSessionDlcSelectTab_BuildTiles", Game_CreateSessionDlcSelectTab_BuildTiles_t> Game_CreateSessionDlcSelectTab_BuildTiles{}; // L2 experimental
using Game_CreateSessionDlcSelectTab_FillTile_t = void *(void *tile, void *node, const char *title, const char *command, const char *image);
inline constexpr fn<"Game_CreateSessionDlcSelectTab_FillTile", Game_CreateSessionDlcSelectTab_FillTile_t> Game_CreateSessionDlcSelectTab_FillTile{}; // L2 experimental
using Game_CreateSessionDlcSelectTab_HandleCommand_t = char *(void *view_model, void *command);
inline constexpr fn<"Game_CreateSessionDlcSelectTab_HandleCommand", Game_CreateSessionDlcSelectTab_HandleCommand_t> Game_CreateSessionDlcSelectTab_HandleCommand{}; // L2 experimental
inline constexpr address<"Game_CreditsSelectVM_OnScreenActivated"> Game_CreditsSelectVM_OnScreenActivated{}; // L2 experimental
inline constexpr address<"Game_CrewWorkingOnCar_Initialise"> Game_CrewWorkingOnCar_Initialise{}; // L2 experimental
using Game_Currency_ShowReceivedPopup_t = bool(void *self, int32_t newBalance);
inline constexpr fn<"Game_Currency_ShowReceivedPopup", Game_Currency_ShowReceivedPopup_t> Game_Currency_ShowReceivedPopup{}; // L2 experimental
inline constexpr address<"Game_CustomDriverNetworkMessageHandler_AddReplicatedDriver"> Game_CustomDriverNetworkMessageHandler_AddReplicatedDriver{}; // L2 experimental
inline constexpr address<"Game_CustomDriverNetworkMessageHandler_FindOrCreateDriverStatId"> Game_CustomDriverNetworkMessageHandler_FindOrCreateDriverStatId{}; // L2 experimental
inline constexpr address<"Game_CustomDriverNetworkMessageHandler_HandleResponse"> Game_CustomDriverNetworkMessageHandler_HandleResponse{}; // L2 experimental
inline constexpr address<"Game_CustomDriverNetworkMessageHandler_RemoveCustomDriver"> Game_CustomDriverNetworkMessageHandler_RemoveCustomDriver{}; // L2 experimental
inline constexpr address<"Game_CustomDriverNetworkMessageHandler_RemovePlayerReplData"> Game_CustomDriverNetworkMessageHandler_RemovePlayerReplData{}; // L2 experimental
inline constexpr address<"Game_CustomDriverNetworkMessageHandler_ReplaceReplicatedDriver"> Game_CustomDriverNetworkMessageHandler_ReplaceReplicatedDriver{}; // L2 experimental
inline constexpr address<"Game_CustomDriverNetworkMessageHandler_UpdateReplicatedDriver"> Game_CustomDriverNetworkMessageHandler_UpdateReplicatedDriver{}; // L2 experimental
inline constexpr address<"Game_CustomisationDecals_DeleteFiles"> Game_CustomisationDecals_DeleteFiles{}; // L2 experimental
inline constexpr address<"Game_CustomisationDecals_Destroy"> Game_CustomisationDecals_Destroy{}; // L2 experimental
inline constexpr address<"Game_CustomisationDecals_DestroyMaterialOverride"> Game_CustomisationDecals_DestroyMaterialOverride{}; // L2 experimental
inline constexpr address<"Game_CustomisationDecals_DestroySetProxy"> Game_CustomisationDecals_DestroySetProxy{}; // L2 experimental
inline constexpr address<"Game_CustomisationDecals_LoadConfig"> Game_CustomisationDecals_LoadConfig{}; // L2 experimental
inline constexpr address<"Game_CustomisationDecals_OpenFile"> Game_CustomisationDecals_OpenFile{}; // L2 experimental
inline constexpr address<"Game_CustomisationDecals_UnloadAssetBatch"> Game_CustomisationDecals_UnloadAssetBatch{}; // L2 experimental
inline constexpr address<"Game_CustomisationOptionsVM_OnActivate"> Game_CustomisationOptionsVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_CustomisationOptionsVM_OnDeactivate"> Game_CustomisationOptionsVM_OnDeactivate{}; // L2 experimental
inline constexpr address<"Game_CutSceneParams_SetCameraIndex"> Game_CutSceneParams_SetCameraIndex{}; // L2 experimental
inline constexpr address<"Game_CutSceneParams_SetCutSceneName"> Game_CutSceneParams_SetCutSceneName{}; // L2 experimental
inline constexpr address<"Game_CutSceneParams_SetupDataFromEntrant"> Game_CutSceneParams_SetupDataFromEntrant{}; // L2 experimental
inline constexpr address<"Game_CutSceneParams_SetupFromSpecificInfoFallbacks"> Game_CutSceneParams_SetupFromSpecificInfoFallbacks{}; // L2 experimental
inline constexpr address<"Game_CutSceneParams_SetupLeadEngineerFromTeam"> Game_CutSceneParams_SetupLeadEngineerFromTeam{}; // L2 experimental
inline constexpr address<"Game_CutSceneScriptCmd_SetStateLoading"> Game_CutSceneScriptCmd_SetStateLoading{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_ActivateExtraWorld"> Game_CutSceneScript_ActivateExtraWorld{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_ActivatePrimaryWorld"> Game_CutSceneScript_ActivatePrimaryWorld{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_AddDriverBodyToAssetGroupPaths"> Game_CutSceneScript_AddDriverBodyToAssetGroupPaths{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_AddDriverBodyToAssetGroupPaths_2"> Game_CutSceneScript_AddDriverBodyToAssetGroupPaths_2{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_AdvanceStartTimeOffset"> Game_CutSceneScript_AdvanceStartTimeOffset{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_ApplyDriverPartOptions"> Game_CutSceneScript_ApplyDriverPartOptions{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_DeactivatePrimaryWorld"> Game_CutSceneScript_DeactivatePrimaryWorld{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_Initialise"> Game_CutSceneScript_Initialise{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_LoadAssetGroups"> Game_CutSceneScript_LoadAssetGroups{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_ResetStartTimeOffset"> Game_CutSceneScript_ResetStartTimeOffset{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_ResolveDriverOutfitAssets"> Game_CutSceneScript_ResolveDriverOutfitAssets{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_RunCommand"> Game_CutSceneScript_RunCommand{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_SetGarageLocation"> Game_CutSceneScript_SetGarageLocation{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_SetGridLocation"> Game_CutSceneScript_SetGridLocation{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_SetStateFinished"> Game_CutSceneScript_SetStateFinished{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_SpawnDynamicDriverActor"> Game_CutSceneScript_SpawnDynamicDriverActor{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_SpawnGarageGantry"> Game_CutSceneScript_SpawnGarageGantry{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_SpawnLeadEngineer"> Game_CutSceneScript_SpawnLeadEngineer{}; // L2 experimental
inline constexpr address<"Game_CutSceneScript_StopWorldNotification"> Game_CutSceneScript_StopWorldNotification{}; // L2 experimental
inline constexpr address<"Game_CutsceneContext_ParseSceneContext"> Game_CutsceneContext_ParseSceneContext{}; // L2 experimental
inline constexpr address<"Game_CutsceneManager_IsPreparingScenes"> Game_CutsceneManager_IsPreparingScenes{}; // L2 experimental
inline constexpr address<"Game_CutsceneManager_SyncAssetGroups"> Game_CutsceneManager_SyncAssetGroups{}; // L2 experimental
inline constexpr address<"Game_CutsceneManager_WaitTextureStreaming"> Game_CutsceneManager_WaitTextureStreaming{}; // L2 experimental
using Game_DRLSystem_OnMessagePlayerDroppedIn_t = int(void *drl_system, void *msg);
inline constexpr fn<"Game_DRLSystem_OnMessagePlayerDroppedIn", Game_DRLSystem_OnMessagePlayerDroppedIn_t> Game_DRLSystem_OnMessagePlayerDroppedIn{}; // L2 experimental, DRLSystem::onMessagePlayerDroppedIn.
using Game_DRLSystem_ResolveDependencies_t = uint32_t(void *self, void *registry, void *systemMap);
inline constexpr fn<"Game_DRLSystem_ResolveDependencies", Game_DRLSystem_ResolveDependencies_t> Game_DRLSystem_ResolveDependencies{}; // L2 experimental
using Game_DRS_OnSessionLoad_t = void(void *drs);
inline constexpr fn<"Game_DRS_OnSessionLoad", Game_DRS_OnSessionLoad_t> Game_DRS_OnSessionLoad{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_CreateComponents"> Game_DataDrivenCutSceneScript_CreateComponents{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_Destructor"> Game_DataDrivenCutSceneScript_Destructor{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_FireEvent"> Game_DataDrivenCutSceneScript_FireEvent{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_IsPreloadSeqDataComplete"> Game_DataDrivenCutSceneScript_IsPreloadSeqDataComplete{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_IsStateUnloaded"> Game_DataDrivenCutSceneScript_IsStateUnloaded{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_OnShutDownComplete"> Game_DataDrivenCutSceneScript_OnShutDownComplete{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_PostInitialise"> Game_DataDrivenCutSceneScript_PostInitialise{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_SetStateStop"> Game_DataDrivenCutSceneScript_SetStateStop{}; // L2 experimental
inline constexpr address<"Game_DataDrivenCutSceneScript_Shutdown"> Game_DataDrivenCutSceneScript_Shutdown{}; // L2 experimental
inline constexpr address<"Game_DataGathererPlayers_Update"> Game_DataGathererPlayers_Update{}; // L2 experimental
inline constexpr address<"Game_DecalFileDeleteOp_DoJob"> Game_DecalFileDeleteOp_DoJob{}; // L2 experimental
inline constexpr address<"Game_DecalFileLoadOp_OnLoaded"> Game_DecalFileLoadOp_OnLoaded{}; // L2 experimental
inline constexpr address<"Game_DecalGeneration_CacheMaterialHandle"> Game_DecalGeneration_CacheMaterialHandle{}; // L2 experimental
inline constexpr address<"Game_DecalGeneration_ReadBackCompressed"> Game_DecalGeneration_ReadBackCompressed{}; // L2 experimental
inline constexpr address<"Game_DecalGeneration_SetDecalComponent"> Game_DecalGeneration_SetDecalComponent{}; // L2 experimental
inline constexpr address<"Game_DeptEventViewModel_Update"> Game_DeptEventViewModel_Update{}; // L2 experimental
using Game_DifficultySystem_ApplySettingsToUi_t = void(void *self, int *settings, int flag);
inline constexpr fn<"Game_DifficultySystem_ApplySettingsToUi", Game_DifficultySystem_ApplySettingsToUi_t> Game_DifficultySystem_ApplySettingsToUi{}; // L2 experimental
using Game_DriverCareerTeamSelect_OnMessage_t = void(void *vm, const uint32_t *const *name);
inline constexpr fn<"Game_DriverCareerTeamSelect_OnMessage", Game_DriverCareerTeamSelect_OnMessage_t> Game_DriverCareerTeamSelect_OnMessage{}; // L2 experimental
inline constexpr address<"Game_DriverCareer_SpawnDriverPair"> Game_DriverCareer_SpawnDriverPair{}; // L2 experimental
inline constexpr address<"Game_DriverCareer_TeamMateSelectSpawn"> Game_DriverCareer_TeamMateSelectSpawn{}; // L2 experimental
using Game_DriverContractsMyTeamComponent_OpenContractWindow_t = uint64_t(void *comp);
inline constexpr fn<"Game_DriverContractsMyTeamComponent_OpenContractWindow", Game_DriverContractsMyTeamComponent_OpenContractWindow_t> Game_DriverContractsMyTeamComponent_OpenContractWindow{}; // L2 experimental
using Game_DriverContractsMyTeamComponent_ProgressDay_t = void(void *comp);
inline constexpr fn<"Game_DriverContractsMyTeamComponent_ProgressDay", Game_DriverContractsMyTeamComponent_ProgressDay_t> Game_DriverContractsMyTeamComponent_ProgressDay{}; // L2 experimental
using Game_DriverContractsMyTeamComponent_SetContractWindowOpenedFlag_t = uint64_t(void *comp);
inline constexpr fn<"Game_DriverContractsMyTeamComponent_SetContractWindowOpenedFlag", Game_DriverContractsMyTeamComponent_SetContractWindowOpenedFlag_t> Game_DriverContractsMyTeamComponent_SetContractWindowOpenedFlag{}; // L2 experimental
using Game_DriverContracts_AdvanceSecretMeeting_t = void(void *contracts, uint32_t driver, uint8_t notify);
inline constexpr fn<"Game_DriverContracts_AdvanceSecretMeeting", Game_DriverContracts_AdvanceSecretMeeting_t> Game_DriverContracts_AdvanceSecretMeeting{}; // L2 experimental
using Game_DriverContracts_BuildOffers_t = void(void *contracts, uint32_t driver);
inline constexpr fn<"Game_DriverContracts_BuildOffers", Game_DriverContracts_BuildOffers_t> Game_DriverContracts_BuildOffers{}; // L2 experimental
using Game_DriverContracts_OfferSecretMeeting_t = void(void *contracts, const void *meeting, uint32_t team, uint32_t driver, uint8_t reason, uint8_t notify);
inline constexpr fn<"Game_DriverContracts_OfferSecretMeeting", Game_DriverContracts_OfferSecretMeeting_t> Game_DriverContracts_OfferSecretMeeting{}; // L2 experimental
inline constexpr address<"Game_DriverCustomisationReplicationManager_ReplaceCustomisation"> Game_DriverCustomisationReplicationManager_ReplaceCustomisation{}; // L2 experimental
inline constexpr address<"Game_DriverCustomisationReplicationManager_ReplaceCustomisatn_2"> Game_DriverCustomisationReplicationManager_ReplaceCustomisatn_2{}; // L2 experimental
inline constexpr address<"Game_DriverHqAgentIntroTransition_Initialise"> Game_DriverHqAgentIntroTransition_Initialise{}; // L2 experimental
inline constexpr address<"Game_DriverHqContractsTransition_Initialise"> Game_DriverHqContractsTransition_Initialise{}; // L2 experimental
inline constexpr address<"Game_DriverInCarIdle03_Initialise"> Game_DriverInCarIdle03_Initialise{}; // L2 experimental
inline constexpr address<"Game_DriverInCarIdle05_Initialise"> Game_DriverInCarIdle05_Initialise{}; // L2 experimental
inline constexpr address<"Game_DriverProfileTabVM_OnTabActivated"> Game_DriverProfileTabVM_OnTabActivated{}; // L2 experimental
inline constexpr address<"Game_DriverProfileTabVM_OnTabDeactivated"> Game_DriverProfileTabVM_OnTabDeactivated{}; // L2 experimental
inline constexpr address<"Game_DriverSatInCar06_PreStart"> Game_DriverSatInCar06_PreStart{}; // L2 experimental
inline constexpr address<"Game_DriverSelectGridVM_OnActivate"> Game_DriverSelectGridVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_DriverSelectGridVM_OnDeactivate"> Game_DriverSelectGridVM_OnDeactivate{}; // L2 experimental
using Game_DriverSelectHeader_SetTabState_t = void(void *header, uint64_t tab, uint8_t content_visible, uint8_t enabled);
inline constexpr fn<"Game_DriverSelectHeader_SetTabState", Game_DriverSelectHeader_SetTabState_t> Game_DriverSelectHeader_SetTabState{}; // L2 experimental
inline constexpr address<"Game_DriverTalkToEngineer02_Initialise"> Game_DriverTalkToEngineer02_Initialise{}; // L2 experimental
inline constexpr address<"Game_DriverTalksToPress_Initialise"> Game_DriverTalksToPress_Initialise{}; // L2 experimental
inline constexpr address<"Game_DynObjAchievePosition_Activate"> Game_DynObjAchievePosition_Activate{}; // L2 experimental
inline constexpr address<"Game_DynObjAchievePosition_UpdateStatus"> Game_DynObjAchievePosition_UpdateStatus{}; // L2 experimental
inline constexpr address<"Game_DynObjCriterion_UpdateGapAhead"> Game_DynObjCriterion_UpdateGapAhead{}; // L2 experimental
inline constexpr address<"Game_DynObjCriterion_UpdateGapBehind"> Game_DynObjCriterion_UpdateGapBehind{}; // L2 experimental
inline constexpr address<"Game_DynObjCriterion_UpdateGapCurve"> Game_DynObjCriterion_UpdateGapCurve{}; // L2 experimental
inline constexpr address<"Game_DynObjCriterion_UpdatePositionCurve"> Game_DynObjCriterion_UpdatePositionCurve{}; // L2 experimental
inline constexpr address<"Game_DynObjDefendPosition_AccumulateTime"> Game_DynObjDefendPosition_AccumulateTime{}; // L2 experimental
inline constexpr address<"Game_DynObjDefendPosition_CheckRivalChange"> Game_DynObjDefendPosition_CheckRivalChange{}; // L2 experimental
inline constexpr address<"Game_DynObjDefendPosition_Update"> Game_DynObjDefendPosition_Update{}; // L2 experimental
inline constexpr address<"Game_DynObjGapCurve_Evaluate"> Game_DynObjGapCurve_Evaluate{}; // L2 experimental
inline constexpr address<"Game_DynObjGapCurve_Update"> Game_DynObjGapCurve_Update{}; // L2 experimental
inline constexpr address<"Game_DynamicObjective_GetLapsLeft"> Game_DynamicObjective_GetLapsLeft{}; // L2 experimental
inline constexpr address<"Game_DynamicObjective_TryActivatePosition"> Game_DynamicObjective_TryActivatePosition{}; // L2 experimental
inline constexpr address<"Game_EALoginTask_OnStart"> Game_EALoginTask_OnStart{}; // L2 experimental
inline constexpr address<"Game_EALoginTask_Update"> Game_EALoginTask_Update{}; // L2 experimental
using Game_Eca_OnMessageGameModeChanged_t = uint64_t(void *self, void *msg);
inline constexpr fn<"Game_Eca_OnMessageGameModeChanged", Game_Eca_OnMessageGameModeChanged_t> Game_Eca_OnMessageGameModeChanged{}; // L2 experimental
using Game_EngineMgmt_AddGridPenalty_t = void(void *engine_management, const uint32_t *driver, uint32_t places, uint32_t reason);
inline constexpr fn<"Game_EngineMgmt_AddGridPenalty", Game_EngineMgmt_AddGridPenalty_t> Game_EngineMgmt_AddGridPenalty{}; // L2 experimental
using Game_EngineMgmt_ApplySessionWear_t = void(void *engine_management, uint32_t driver, float amount, uint8_t flag);
inline constexpr fn<"Game_EngineMgmt_ApplySessionWear", Game_EngineMgmt_ApplySessionWear_t> Game_EngineMgmt_ApplySessionWear{}; // L2 experimental
inline constexpr address<"Game_EngineMgmt_ClearRaceGridPenalties"> Game_EngineMgmt_ClearRaceGridPenalties{}; // L2 experimental
inline constexpr address<"Game_EngineMgmt_OnRaceSessionEnded"> Game_EngineMgmt_OnRaceSessionEnded{}; // L2 experimental
inline constexpr address<"Game_EngineMgmt_SyncGridPenaltyLog"> Game_EngineMgmt_SyncGridPenaltyLog{}; // L2 experimental
inline constexpr address<"Game_EntrantsManager_ClaimFreeEntrant"> Game_EntrantsManager_ClaimFreeEntrant{}; // L2 experimental
inline constexpr address<"Game_EntrantsManager_HandleCustomSELiveryReplace"> Game_EntrantsManager_HandleCustomSELiveryReplace{}; // L2 experimental
inline constexpr address<"Game_EntrantsManager_OnMutableRoleChanged"> Game_EntrantsManager_OnMutableRoleChanged{}; // L2 experimental
inline constexpr address<"Game_EntrantsManager_RegisterEntrant"> Game_EntrantsManager_RegisterEntrant{}; // L2 experimental
inline constexpr address<"Game_EntrantsManager_ResolveAndCloseDocument"> Game_EntrantsManager_ResolveAndCloseDocument{}; // L2 experimental
inline constexpr address<"Game_EntrantsManager_ResolveEntrants"> Game_EntrantsManager_ResolveEntrants{}; // L2 experimental
using Game_EntrantsUi_BuildCharacterRef_t = void *(void *out, const uint8_t *character, void *node);
inline constexpr fn<"Game_EntrantsUi_BuildCharacterRef", Game_EntrantsUi_BuildCharacterRef_t> Game_EntrantsUi_BuildCharacterRef{}; // L2 experimental
using Game_EntrantsUi_OnCharacterListLoaded_t = int64_t(void *ctx, const void *message);
inline constexpr fn<"Game_EntrantsUi_OnCharacterListLoaded", Game_EntrantsUi_OnCharacterListLoaded_t> Game_EntrantsUi_OnCharacterListLoaded{}; // L2 experimental
inline constexpr address<"Game_EventDispatcher_SendPositionEvent"> Game_EventDispatcher_SendPositionEvent{}; // L2 experimental
inline constexpr address<"Game_EventObjectives_OnTargetPositionDone"> Game_EventObjectives_OnTargetPositionDone{}; // L2 experimental
inline constexpr address<"Game_EventObjectives_ShowTargetPosition"> Game_EventObjectives_ShowTargetPosition{}; // L2 experimental
inline constexpr address<"Game_ExcitementEventPair_TraceLog"> Game_ExcitementEventPair_TraceLog{}; // L2 experimental
inline constexpr address<"Game_F1CGRuleset_ForceFinishAll"> Game_F1CGRuleset_ForceFinishAll{}; // L2 experimental
inline constexpr address<"Game_F1CGRuleset_OnVehicleStatusRequest"> Game_F1CGRuleset_OnVehicleStatusRequest{}; // L2 experimental
using Game_F1DataFeedSystem_Init_t = int(void *self, void *serviceProvider, void *componentMap);
inline constexpr fn<"Game_F1DataFeedSystem_Init", Game_F1DataFeedSystem_Init_t> Game_F1DataFeedSystem_Init{}; // L2 experimental, F1DataFeedSystem
inline constexpr address<"Game_F1WLoadout_Initialise"> Game_F1WLoadout_Initialise{}; // L2 experimental
using Game_F1WRankedComponent_Init_t = int(void *self, void *serviceProvider, void *componentMap);
inline constexpr fn<"Game_F1WRankedComponent_Init", Game_F1WRankedComponent_Init_t> Game_F1WRankedComponent_Init{}; // L2 experimental, F1WRankedComponent
inline constexpr address<"Game_F1WorldActivityContainer_GoHome"> Game_F1WorldActivityContainer_GoHome{}; // L2 experimental
inline constexpr address<"Game_F1WorldActivityContainer_OnExit"> Game_F1WorldActivityContainer_OnExit{}; // L2 experimental
using Game_F1WorldActivityContainer_SelectEntrantList_t = void(void *container, const char *entrant_list);
inline constexpr fn<"Game_F1WorldActivityContainer_SelectEntrantList", Game_F1WorldActivityContainer_SelectEntrantList_t> Game_F1WorldActivityContainer_SelectEntrantList{}; // L2 experimental
inline constexpr address<"Game_F1WorldGoals_HandleCommand"> Game_F1WorldGoals_HandleCommand{}; // L2 experimental
inline constexpr address<"Game_F1WorldGoals_OnTabActivated"> Game_F1WorldGoals_OnTabActivated{}; // L2 experimental
inline constexpr address<"Game_F1WorldGoals_OnTabDeactivated"> Game_F1WorldGoals_OnTabDeactivated{}; // L2 experimental
inline constexpr address<"Game_F1WorldGoals_PopulateGoalList"> Game_F1WorldGoals_PopulateGoalList{}; // L2 experimental
inline constexpr address<"Game_F1WorldHub_BuildPlayTiles"> Game_F1WorldHub_BuildPlayTiles{}; // L2 experimental
using Game_F1WorldHub_HandleCommand_t = uint64_t(void *vm, NeStr **cmd, void *value);
inline constexpr fn<"Game_F1WorldHub_HandleCommand", Game_F1WorldHub_HandleCommand_t> Game_F1WorldHub_HandleCommand{}; // L2 experimental
inline constexpr address<"Game_F1WorldHub_OnPopupResult"> Game_F1WorldHub_OnPopupResult{}; // L2 experimental
inline constexpr address<"Game_F1WorldHub_RestoreLastActivity"> Game_F1WorldHub_RestoreLastActivity{}; // L2 experimental
inline constexpr address<"Game_F1WorldHub_RouteToActivity"> Game_F1WorldHub_RouteToActivity{}; // L2 experimental
inline constexpr address<"Game_F1WorldHub_SelectTab"> Game_F1WorldHub_SelectTab{}; // L2 experimental
inline constexpr address<"Game_F1WorldRewards_PopulateXpScreen"> Game_F1WorldRewards_PopulateXpScreen{}; // L2 experimental
inline constexpr address<"Game_F1WorldScreenVM_Update"> Game_F1WorldScreenVM_Update{}; // L2 experimental
inline constexpr address<"Game_F1WorldScreen_NavigateEp1_2"> Game_F1WorldScreen_NavigateEp1_2{}; // L2 experimental
inline constexpr address<"Game_F1WorldScreen_NavigateEp1_4"> Game_F1WorldScreen_NavigateEp1_4{}; // L2 experimental
inline constexpr address<"Game_F1WorldScreen_RefreshAndNavigate"> Game_F1WorldScreen_RefreshAndNavigate{}; // L2 experimental
using Game_F1World_ApplySkipOnboarding_t = void(void *self);
inline constexpr fn<"Game_F1World_ApplySkipOnboarding", Game_F1World_ApplySkipOnboarding_t> Game_F1World_ApplySkipOnboarding{}; // L2 experimental
using Game_F1World_AssignDefaultFanzoneChoices_t = void(void *self);
inline constexpr fn<"Game_F1World_AssignDefaultFanzoneChoices", Game_F1World_AssignDefaultFanzoneChoices_t> Game_F1World_AssignDefaultFanzoneChoices{}; // L2 experimental
inline constexpr address<"Game_F1World_NavigateWithEntryPoint1"> Game_F1World_NavigateWithEntryPoint1{}; // L2 experimental
inline constexpr address<"Game_F1osHttpTask_Ctor_Vt14583cd40"> Game_F1osHttpTask_Ctor_Vt14583cd40{}; // L2 experimental
inline constexpr address<"Game_F1osTask_CollectResults_Vt145835c00"> Game_F1osTask_CollectResults_Vt145835c00{}; // L2 experimental
inline constexpr address<"Game_F1osTask_Ctor_Vt145835090"> Game_F1osTask_Ctor_Vt145835090{}; // L2 experimental
inline constexpr address<"Game_F1osTask_FinishIfNoPending_GetFriendsList"> Game_F1osTask_FinishIfNoPending_GetFriendsList{}; // L2 experimental
inline constexpr address<"Game_F1osTask_FinishLocked_PlayerSearch"> Game_F1osTask_FinishLocked_PlayerSearch{}; // L2 experimental
inline constexpr address<"Game_F1osTask_OnChildFinished_Vt145835c00"> Game_F1osTask_OnChildFinished_Vt145835c00{}; // L2 experimental
inline constexpr address<"Game_F1osTask_OnJoinFailed_Vt145832400"> Game_F1osTask_OnJoinFailed_Vt145832400{}; // L2 experimental
inline constexpr address<"Game_F1osTask_OnResultSetFinished_Vt145832400"> Game_F1osTask_OnResultSetFinished_Vt145832400{}; // L2 experimental
inline constexpr address<"Game_F1osTask_ProcessFriendsReply_Vt145834f90"> Game_F1osTask_ProcessFriendsReply_Vt145834f90{}; // L2 experimental
inline constexpr address<"Game_F1osTask_RegisterEntrant_Vt1458323b0"> Game_F1osTask_RegisterEntrant_Vt1458323b0{}; // L2 experimental
inline constexpr address<"Game_F1osTask_ResolveNames_GetFriendsList"> Game_F1osTask_ResolveNames_GetFriendsList{}; // L2 experimental
inline constexpr address<"Game_F1osTask_SetFinishedReleaseRef"> Game_F1osTask_SetFinishedReleaseRef{}; // L2 experimental
inline constexpr address<"Game_F1osTask_SetFinished_B"> Game_F1osTask_SetFinished_B{}; // L2 experimental
inline constexpr address<"Game_F1osTask_SpawnChild_Vt1458323b0"> Game_F1osTask_SpawnChild_Vt1458323b0{}; // L2 experimental
inline constexpr address<"Game_F1osTask_SpawnChild_Vt145835c00"> Game_F1osTask_SpawnChild_Vt145835c00{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_FinalisePackagePTask"> Game_F1osTasksystem_CreateTask_FinalisePackagePTask{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_GetPPackagesData"> Game_F1osTasksystem_CreateTask_GetPPackagesData{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_HoldingPageHint"> Game_F1osTasksystem_CreateTask_HoldingPageHint{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_PurchaseVIP"> Game_F1osTasksystem_CreateTask_PurchaseVIP{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_RequestPackagePTask"> Game_F1osTasksystem_CreateTask_RequestPackagePTask{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_Vt145834f90"> Game_F1osTasksystem_CreateTask_Vt145834f90{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_Vt145835090"> Game_F1osTasksystem_CreateTask_Vt145835090{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_Vt1458352e0"> Game_F1osTasksystem_CreateTask_Vt1458352e0{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_Vt145835490"> Game_F1osTasksystem_CreateTask_Vt145835490{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_Vt145835780"> Game_F1osTasksystem_CreateTask_Vt145835780{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_Vt145835ba0"> Game_F1osTasksystem_CreateTask_Vt145835ba0{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_Vt14583c2a0"> Game_F1osTasksystem_CreateTask_Vt14583c2a0{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_CreateTask_Vt14583d000"> Game_F1osTasksystem_CreateTask_Vt14583d000{}; // L2 experimental
inline constexpr address<"Game_F1osTasksystem_Update"> Game_F1osTasksystem_Update{}; // L2 experimental
inline constexpr address<"Game_F1os_CreateHttpRequest"> Game_F1os_CreateHttpRequest{}; // L2 experimental
inline constexpr address<"Game_F1os_CreateOnlineServiceTask"> Game_F1os_CreateOnlineServiceTask{}; // L2 experimental
inline constexpr address<"Game_F1os_CreateTask_GetFriendsLeaderboard"> Game_F1os_CreateTask_GetFriendsLeaderboard{}; // L2 experimental
inline constexpr address<"Game_F1os_CreateTask_GetLeaderboard"> Game_F1os_CreateTask_GetLeaderboard{}; // L2 experimental
inline constexpr address<"Game_F1os_CreateTask_Login"> Game_F1os_CreateTask_Login{}; // L2 experimental
inline constexpr address<"Game_F1os_CreateTask_ReconcileUserEntitlements"> Game_F1os_CreateTask_ReconcileUserEntitlements{}; // L2 experimental
inline constexpr address<"Game_F1os_CreateTask_RefreshSessionToken"> Game_F1os_CreateTask_RefreshSessionToken{}; // L2 experimental
inline constexpr address<"Game_F1os_CreateTask_RequestPackageP"> Game_F1os_CreateTask_RequestPackageP{}; // L2 experimental
inline constexpr address<"Game_F1os_Create_CompleteEvent"> Game_F1os_Create_CompleteEvent{}; // L2 experimental
inline constexpr address<"Game_F1os_Create_CraftBlueprint"> Game_F1os_Create_CraftBlueprint{}; // L2 experimental
inline constexpr address<"Game_F1os_Create_GetCCLeaderboard"> Game_F1os_Create_GetCCLeaderboard{}; // L2 experimental
inline constexpr address<"Game_F1os_Create_RemoveMember"> Game_F1os_Create_RemoveMember{}; // L2 experimental
inline constexpr address<"Game_F1os_Create_SessionBrowserSearchSessions"> Game_F1os_Create_SessionBrowserSearchSessions{}; // L2 experimental
inline constexpr address<"Game_F1os_Create_SetPlayerInfo"> Game_F1os_Create_SetPlayerInfo{}; // L2 experimental
inline constexpr address<"Game_F1os_Create_StartEvent"> Game_F1os_Create_StartEvent{}; // L2 experimental
inline constexpr address<"Game_F1os_DestroyOnlineServiceTaskBase"> Game_F1os_DestroyOnlineServiceTaskBase{}; // L2 experimental
inline constexpr address<"Game_F1os_DestroyTaskBody_GetFriendsLeaderboard"> Game_F1os_DestroyTaskBody_GetFriendsLeaderboard{}; // L2 experimental
inline constexpr address<"Game_F1os_F1CGTask_CheckLogout"> Game_F1os_F1CGTask_CheckLogout{}; // L2 experimental
inline constexpr address<"Game_F1os_F1CGTask_OnRequestResult"> Game_F1os_F1CGTask_OnRequestResult{}; // L2 experimental
inline constexpr address<"Game_F1os_F1CGTask_Reset"> Game_F1os_F1CGTask_Reset{}; // L2 experimental
inline constexpr address<"Game_F1os_F1CGTask_Update"> Game_F1os_F1CGTask_Update{}; // L2 experimental
inline constexpr address<"Game_F1os_F1WorldTask_CheckLogout"> Game_F1os_F1WorldTask_CheckLogout{}; // L2 experimental
inline constexpr address<"Game_F1os_F1WorldTask_OnRequestResult"> Game_F1os_F1WorldTask_OnRequestResult{}; // L2 experimental
inline constexpr address<"Game_F1os_F1WorldTask_Reset"> Game_F1os_F1WorldTask_Reset{}; // L2 experimental
inline constexpr address<"Game_F1os_F1WorldTask_Update"> Game_F1os_F1WorldTask_Update{}; // L2 experimental
inline constexpr address<"Game_F1os_LogF"> Game_F1os_LogF{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_ClaimRewards"> Game_F1os_LogLargePayload_ClaimRewards{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_GetActiveChallengeCareer"> Game_F1os_LogLargePayload_GetActiveChallengeCareer{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_GetCalendar"> Game_F1os_LogLargePayload_GetCalendar{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_GetCareerBackupData"> Game_F1os_LogLargePayload_GetCareerBackupData{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_GetCompendiumRewards"> Game_F1os_LogLargePayload_GetCompendiumRewards{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_GetInbox"> Game_F1os_LogLargePayload_GetInbox{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_GetStorefront"> Game_F1os_LogLargePayload_GetStorefront{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_SessionBrowserGetSessionByID"> Game_F1os_LogLargePayload_SessionBrowserGetSessionByID{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_SessionBrowserSearchSessions"> Game_F1os_LogLargePayload_SessionBrowserSearchSessions{}; // L2 experimental
inline constexpr address<"Game_F1os_LogLargePayload_UpgradeCard"> Game_F1os_LogLargePayload_UpgradeCard{}; // L2 experimental
inline constexpr address<"Game_F1wAction_Destroy_EmptyAction"> Game_F1wAction_Destroy_EmptyAction{}; // L2 experimental
using Game_FEHomeViewModel_OnUpdate_t = void(FEHomeViewModel *vm, float dt);
inline constexpr fn<"Game_FEHomeViewModel_OnUpdate", Game_FEHomeViewModel_OnUpdate_t> Game_FEHomeViewModel_OnUpdate{}; // L2 experimental
inline constexpr address<"Game_FacilityHeads_CollectAssetGroupDescs"> Game_FacilityHeads_CollectAssetGroupDescs{}; // L2 experimental
inline constexpr address<"Game_FacilityHeads_RegisterCommandMessages"> Game_FacilityHeads_RegisterCommandMessages{}; // L2 experimental
inline constexpr address<"Game_FacilityHeads_RegisterCommandMessages_2"> Game_FacilityHeads_RegisterCommandMessages_2{}; // L2 experimental
inline constexpr address<"Game_FakeOption_GetMaxIndex"> Game_FakeOption_GetMaxIndex{}; // L2 experimental
inline constexpr address<"Game_FakeOption_SetValue"> Game_FakeOption_SetValue{}; // L2 experimental
inline constexpr address<"Game_FanZoneSeasonResults_Exit"> Game_FanZoneSeasonResults_Exit{}; // L2 experimental
inline constexpr address<"Game_FanZoneStandingsVM_OnScreenDeactivated"> Game_FanZoneStandingsVM_OnScreenDeactivated{}; // L2 experimental
inline constexpr address<"Game_FanZoneStandingsVM_OnSkip"> Game_FanZoneStandingsVM_OnSkip{}; // L2 experimental
inline constexpr address<"Game_FanZone_BuildStandingsDriverList"> Game_FanZone_BuildStandingsDriverList{}; // L2 experimental
inline constexpr address<"Game_FanzonesOnboardingVM_FadeIn"> Game_FanzonesOnboardingVM_FadeIn{}; // L2 experimental
inline constexpr address<"Game_FeaturedEventModeComponent_SetupStartingGrid"> Game_FeaturedEventModeComponent_SetupStartingGrid{}; // L2 experimental, FeaturedEventModeComponent::setupStartingGrid
using Game_Fill_PerformanceComparison_t = void(void *comparison);
inline constexpr fn<"Game_Fill_PerformanceComparison", Game_Fill_PerformanceComparison_t> Game_Fill_PerformanceComparison{}; // L2 experimental
inline constexpr address<"Game_FlowState_EnterBootLogout"> Game_FlowState_EnterBootLogout{}; // L2 experimental
inline constexpr address<"Game_FormationLapPresentation_BuildTimeline"> Game_FormationLapPresentation_BuildTimeline{}; // L2 experimental
inline constexpr address<"Game_FreeLookCameraModifier_Update"> Game_FreeLookCameraModifier_Update{}; // L2 experimental
inline constexpr address<"Game_FriendsList_OnGetFriendsDone"> Game_FriendsList_OnGetFriendsDone{}; // L2 experimental
inline constexpr address<"Game_FrontEnd_CloseFadeOut"> Game_FrontEnd_CloseFadeOut{}; // L2 experimental
inline constexpr address<"Game_FrontendLobbyVM_OnScreenActivated"> Game_FrontendLobbyVM_OnScreenActivated{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCameraManager_GetSequencePosition"> Game_FrontendSceneCameraManager_GetSequencePosition{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCameraManager_SetActiveCamera"> Game_FrontendSceneCameraManager_SetActiveCamera{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCameraManager_Update"> Game_FrontendSceneCameraManager_Update{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCarManager_SetCarOverride"> Game_FrontendSceneCarManager_SetCarOverride{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCarManager_SpawnCarAtPlacement"> Game_FrontendSceneCarManager_SpawnCarAtPlacement{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCarManager_SpawnCarAtTransform"> Game_FrontendSceneCarManager_SpawnCarAtTransform{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCar_SetCustomDecalNodes"> Game_FrontendSceneCar_SetCustomDecalNodes{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCar_SetCustomLivery"> Game_FrontendSceneCar_SetCustomLivery{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCar_SetVisibilityInternal"> Game_FrontendSceneCar_SetVisibilityInternal{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCar_SetupComponents"> Game_FrontendSceneCar_SetupComponents{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCar_SpawnFromLiveryData"> Game_FrontendSceneCar_SpawnFromLiveryData{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCar_SpawnFromLiveryData2"> Game_FrontendSceneCar_SpawnFromLiveryData2{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentDriver_ChangeDriver"> Game_FrontendSceneComponentDriver_ChangeDriver{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentDriver_NeedsRespawn"> Game_FrontendSceneComponentDriver_NeedsRespawn{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentDriver_SetAnimOverlay"> Game_FrontendSceneComponentDriver_SetAnimOverlay{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentDriver_Spawn"> Game_FrontendSceneComponentDriver_Spawn{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentDriver_UpdateDecals"> Game_FrontendSceneComponentDriver_UpdateDecals{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentObject_DespawnComplete"> Game_FrontendSceneComponentObject_DespawnComplete{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentObject_LogState"> Game_FrontendSceneComponentObject_LogState{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentObject_NeedsRespawn"> Game_FrontendSceneComponentObject_NeedsRespawn{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentObject_SetVisible"> Game_FrontendSceneComponentObject_SetVisible{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponentObject_Spawn"> Game_FrontendSceneComponentObject_Spawn{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneComponent_StopDriverAnimOverlay"> Game_FrontendSceneComponent_StopDriverAnimOverlay{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCustomObject_SpawnObject"> Game_FrontendSceneCustomObject_SpawnObject{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCustomObject_Update"> Game_FrontendSceneCustomObject_Update{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_OnSubsceneChanged"> Game_FrontendSceneCutsceneManager_OnSubsceneChanged{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_PlayCutscene"> Game_FrontendSceneCutsceneManager_PlayCutscene{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_PreInitCutscene"> Game_FrontendSceneCutsceneManager_PreInitCutscene{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_PreInitCutscenes"> Game_FrontendSceneCutsceneManager_PreInitCutscenes{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_PreloadCutscene"> Game_FrontendSceneCutsceneManager_PreloadCutscene{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_PreloadSubsceneCutscenes"> Game_FrontendSceneCutsceneManager_PreloadSubsceneCutscenes{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_StopActiveCutscene"> Game_FrontendSceneCutsceneManager_StopActiveCutscene{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_StopAllCutscenes"> Game_FrontendSceneCutsceneManager_StopAllCutscenes{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_StopCutsceneEntry"> Game_FrontendSceneCutsceneManager_StopCutsceneEntry{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneCutsceneManager_Update"> Game_FrontendSceneCutsceneManager_Update{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_BindActorsToSequence"> Game_FrontendSceneDriver_BindActorsToSequence{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_ChooseSequence"> Game_FrontendSceneDriver_ChooseSequence{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_Despawn"> Game_FrontendSceneDriver_Despawn{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_FillHelmetPreviewData"> Game_FrontendSceneDriver_FillHelmetPreviewData{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_SetPartsVisible"> Game_FrontendSceneDriver_SetPartsVisible{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_SpawnActors"> Game_FrontendSceneDriver_SpawnActors{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_SpawnActorsAlt"> Game_FrontendSceneDriver_SpawnActorsAlt{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_Update"> Game_FrontendSceneDriver_Update{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_UpdatePfx"> Game_FrontendSceneDriver_UpdatePfx{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneDriver_UpdateTransform"> Game_FrontendSceneDriver_UpdateTransform{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_GetDriverHelmetData"> Game_FrontendSceneManager_GetDriverHelmetData{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_ProcessPendingCarChanges"> Game_FrontendSceneManager_ProcessPendingCarChanges{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_ProcessPendingDriverChanges"> Game_FrontendSceneManager_ProcessPendingDriverChanges{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_ResetComponents"> Game_FrontendSceneManager_ResetComponents{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetCarVisible"> Game_FrontendSceneManager_SetCarVisible{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetDriverAnimationOverlay"> Game_FrontendSceneManager_SetDriverAnimationOverlay{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetDriverVisible"> Game_FrontendSceneManager_SetDriverVisible{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetDriverVisibleById"> Game_FrontendSceneManager_SetDriverVisibleById{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetObjectVisible"> Game_FrontendSceneManager_SetObjectVisible{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetPendingSwitch"> Game_FrontendSceneManager_SetPendingSwitch{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetupCarInternal"> Game_FrontendSceneManager_SetupCarInternal{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetupDriverInternal"> Game_FrontendSceneManager_SetupDriverInternal{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SetupObjectInternal"> Game_FrontendSceneManager_SetupObjectInternal{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_StopCutscene"> Game_FrontendSceneManager_StopCutscene{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SwitchScene"> Game_FrontendSceneManager_SwitchScene{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneManager_SwitchSceneFromPreset"> Game_FrontendSceneManager_SwitchSceneFromPreset{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneObject_RespawnAll"> Game_FrontendSceneObject_RespawnAll{}; // L2 experimental
inline constexpr address<"Game_FrontendScenePreview_Refresh"> Game_FrontendScenePreview_Refresh{}; // L2 experimental
inline constexpr address<"Game_FrontendScenePreview_SelectEntry"> Game_FrontendScenePreview_SelectEntry{}; // L2 experimental
inline constexpr address<"Game_FrontendScenePreview_UpdateTick"> Game_FrontendScenePreview_UpdateTick{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneScreenF_Destructor"> Game_FrontendSceneScreenF_Destructor{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneScreen_Destructor"> Game_FrontendSceneScreen_Destructor{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneScript_AddFzMultiActorGroup"> Game_FrontendSceneScript_AddFzMultiActorGroup{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneScript_AddToMultiActorAnim"> Game_FrontendSceneScript_AddToMultiActorAnim{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneScript_RemoveFzMultiActorGroup"> Game_FrontendSceneScript_RemoveFzMultiActorGroup{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneScript_SpawnSequenceForActor"> Game_FrontendSceneScript_SpawnSequenceForActor{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneScript_SpawnSequences"> Game_FrontendSceneScript_SpawnSequences{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneSystem_ChangeMainCamera"> Game_FrontendSceneSystem_ChangeMainCamera{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneSystem_PreloadScene"> Game_FrontendSceneSystem_PreloadScene{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneSystem_ProcessCommand"> Game_FrontendSceneSystem_ProcessCommand{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneSystem_SetSubscene"> Game_FrontendSceneSystem_SetSubscene{}; // L2 experimental
inline constexpr address<"Game_FrontendSceneUi_SetMappedDriverVisible"> Game_FrontendSceneUi_SetMappedDriverVisible{}; // L2 experimental
using Game_FrontendTransitionVM_OnScreenActivated_t = void(void *vm);
inline constexpr fn<"Game_FrontendTransitionVM_OnScreenActivated", Game_FrontendTransitionVM_OnScreenActivated_t> Game_FrontendTransitionVM_OnScreenActivated{}; // L2 experimental
inline constexpr address<"Game_FuelDynamicObjective_Activate"> Game_FuelDynamicObjective_Activate{}; // L2 experimental
inline constexpr address<"Game_FuelDynamicObjective_Activate_2"> Game_FuelDynamicObjective_Activate_2{}; // L2 experimental
using Game_FullRaceReplaySystem_Init_t = int(void *self, void *serviceProvider, void *componentMap);
inline constexpr fn<"Game_FullRaceReplaySystem_Init", Game_FullRaceReplaySystem_Init_t> Game_FullRaceReplaySystem_Init{}; // L2 experimental
inline constexpr address<"Game_GameModes_BuildMode"> Game_GameModes_BuildMode{}; // L2 experimental
inline constexpr address<"Game_GameReplaySystem_SubscribeRewindTutorial"> Game_GameReplaySystem_SubscribeRewindTutorial{}; // L2 experimental
inline constexpr address<"Game_GameSessionConfig_Destroy"> Game_GameSessionConfig_Destroy{}; // L2 experimental
inline constexpr address<"Game_GameSessionManager_Connect"> Game_GameSessionManager_Connect{}; // L2 experimental
inline constexpr address<"Game_GameSessionManager_Destroy"> Game_GameSessionManager_Destroy{}; // L2 experimental
inline constexpr address<"Game_GameSessionManager_ReportJoinError"> Game_GameSessionManager_ReportJoinError{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingCareer_CreateSession"> Game_GameSessionMatchingCareer_CreateSession{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingCareer_JoinSession"> Game_GameSessionMatchingCareer_JoinSession{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingCareer_OnCreateSessionComplete"> Game_GameSessionMatchingCareer_OnCreateSessionComplete{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingCustom_SearchSession"> Game_GameSessionMatchingCustom_SearchSession{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingCustom_SearchSessionWithPresence"> Game_GameSessionMatchingCustom_SearchSessionWithPresence{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingF1WSocialGP_Cancel"> Game_GameSessionMatchingF1WSocialGP_Cancel{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingF1WSocialGP_CreateSession"> Game_GameSessionMatchingF1WSocialGP_CreateSession{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingF1WSocialGP_CreateSessionForMembers"> Game_GameSessionMatchingF1WSocialGP_CreateSessionForMembers{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingF1WSocialGP_JoinSession"> Game_GameSessionMatchingF1WSocialGP_JoinSession{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingF1WSocialGP_OnCreateSessionComplete"> Game_GameSessionMatchingF1WSocialGP_OnCreateSessionComplete{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatchingHopperCommon_OnSessionCreateComplete"> Game_GameSessionMatchingHopperCommon_OnSessionCreateComplete{}; // L2 experimental
inline constexpr address<"Game_GameSessionMatching_ApplySessionAttributes"> Game_GameSessionMatching_ApplySessionAttributes{}; // L2 experimental
inline constexpr address<"Game_GameSessionMigrationManager_ClearHaveResults"> Game_GameSessionMigrationManager_ClearHaveResults{}; // L2 experimental
inline constexpr address<"Game_GameSessionMigrationManager_OnMutableRoleChanged"> Game_GameSessionMigrationManager_OnMutableRoleChanged{}; // L2 experimental
inline constexpr address<"Game_GameSessionMigrationManager_SendSelfState"> Game_GameSessionMigrationManager_SendSelfState{}; // L2 experimental
inline constexpr address<"Game_GameSessionMigrationManager_SetPeerStatus"> Game_GameSessionMigrationManager_SetPeerStatus{}; // L2 experimental
inline constexpr address<"Game_GameSessionUI_Shutdown"> Game_GameSessionUI_Shutdown{}; // L2 experimental
using Game_Game_HandleInterruptEvent_t = uint64_t(void *game, const void *event);
inline constexpr fn<"Game_Game_HandleInterruptEvent", Game_Game_HandleInterruptEvent_t> Game_Game_HandleInterruptEvent{}; // L2 experimental
inline constexpr address<"Game_Game_OnRetireLocalPlayersMessage"> Game_Game_OnRetireLocalPlayersMessage{}; // L2 experimental
using Game_Game_SetCurrentGameMode_t = uint64_t(void *mode_context, void *mode);
inline constexpr fn<"Game_Game_SetCurrentGameMode", Game_Game_SetCurrentGameMode_t> Game_Game_SetCurrentGameMode{}; // L2 experimental
inline constexpr address<"Game_GarageCrewInstanceLogic_StateSendCarToTrack_P3"> Game_GarageCrewInstanceLogic_StateSendCarToTrack_P3{}; // L2 experimental, GarageCrewInstanceLogic::StateSendCarToTrack_P3
inline constexpr address<"Game_GarageCrewInstance_PlayOverlaySequence"> Game_GarageCrewInstance_PlayOverlaySequence{}; // L2 experimental
inline constexpr address<"Game_GarageCrewInstance_PlaySequence"> Game_GarageCrewInstance_PlaySequence{}; // L2 experimental
inline constexpr address<"Game_GarageModule_Initialise"> Game_GarageModule_Initialise{}; // L2 experimental
inline constexpr address<"Game_GarageMonitorTvFeedVM_OnActivate"> Game_GarageMonitorTvFeedVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_GarageMonitorTvFeedVM_OnDeactivate"> Game_GarageMonitorTvFeedVM_OnDeactivate{}; // L2 experimental
inline constexpr address<"Game_GarageReturn_ComputeTimeSkip"> Game_GarageReturn_ComputeTimeSkip{}; // L2 experimental
inline constexpr address<"Game_Garage_FadeInAfterReturn"> Game_Garage_FadeInAfterReturn{}; // L2 experimental
inline constexpr address<"Game_Garage_FadeOutForExit"> Game_Garage_FadeOutForExit{}; // L2 experimental
inline constexpr address<"Game_Garage_LeaveGarageShowHud"> Game_Garage_LeaveGarageShowHud{}; // L2 experimental
inline constexpr address<"Game_Garage_OnFadedOutInRace"> Game_Garage_OnFadedOutInRace{}; // L2 experimental
inline constexpr address<"Game_Garage_ReturnVehicleToPitLaneEntry"> Game_Garage_ReturnVehicleToPitLaneEntry{}; // L2 experimental
using Game_Get_ModelNodeId_t = uint32_t(const char *name);
inline constexpr fn<"Game_Get_ModelNodeId", Game_Get_ModelNodeId_t> Game_Get_ModelNodeId{}; // L2 experimental
inline constexpr address<"Game_Get_TutorialStepValue"> Game_Get_TutorialStepValue{}; // L2 experimental
inline constexpr address<"Game_GettingReadyToRace_Initialise"> Game_GettingReadyToRace_Initialise{}; // L2 experimental
inline constexpr address<"Game_GhostLapHandler_CancelTasks"> Game_GhostLapHandler_CancelTasks{}; // L2 experimental
inline constexpr address<"Game_GhostLapHandler_Ctor"> Game_GhostLapHandler_Ctor{}; // L2 experimental
inline constexpr address<"Game_GhostLapHandler_Dtor"> Game_GhostLapHandler_Dtor{}; // L2 experimental
inline constexpr address<"Game_GhostLapHandler_EvaluatePerformanceState"> Game_GhostLapHandler_EvaluatePerformanceState{}; // L2 experimental
inline constexpr address<"Game_GhostLapHandler_FreeSessionRivalStorage"> Game_GhostLapHandler_FreeSessionRivalStorage{}; // L2 experimental
inline constexpr address<"Game_GhostLapHandler_OnSessionRivalSearchCompleted"> Game_GhostLapHandler_OnSessionRivalSearchCompleted{}; // L2 experimental
inline constexpr address<"Game_GhostLapHandler_Update"> Game_GhostLapHandler_Update{}; // L2 experimental
inline constexpr address<"Game_GhostLapsStorageSystem_Connect"> Game_GhostLapsStorageSystem_Connect{}; // L2 experimental
inline constexpr address<"Game_GhostLapsStorageSystem_Dtor"> Game_GhostLapsStorageSystem_Dtor{}; // L2 experimental
inline constexpr address<"Game_GhostLapsStorageSystem_OnSessionExit"> Game_GhostLapsStorageSystem_OnSessionExit{}; // L2 experimental
inline constexpr address<"Game_GhostLapsStorageSystem_OnSessionRestart"> Game_GhostLapsStorageSystem_OnSessionRestart{}; // L2 experimental
inline constexpr address<"Game_GhostRivalFinder_Finish"> Game_GhostRivalFinder_Finish{}; // L2 experimental
inline constexpr address<"Game_GhostRivalFinder_OnEnumerateRivalsCompleted"> Game_GhostRivalFinder_OnEnumerateRivalsCompleted{}; // L2 experimental
inline constexpr address<"Game_GhostRivalFinder_SetState"> Game_GhostRivalFinder_SetState{}; // L2 experimental
inline constexpr address<"Game_GridCutSceneScript_SetupDriverTalkToPress"> Game_GridCutSceneScript_SetupDriverTalkToPress{}; // L2 experimental
inline constexpr address<"Game_GridDriverAdjustGloves_Initialise"> Game_GridDriverAdjustGloves_Initialise{}; // L2 experimental
inline constexpr address<"Game_GridDriverTalkToEngineer_Initialise"> Game_GridDriverTalkToEngineer_Initialise{}; // L2 experimental
inline constexpr address<"Game_GridLeadEngineer_Initialise"> Game_GridLeadEngineer_Initialise{}; // L2 experimental
inline constexpr address<"Game_GridPlayerShot_Initialise"> Game_GridPlayerShot_Initialise{}; // L2 experimental
inline constexpr address<"Game_GridReveal_SetStateLoading"> Game_GridReveal_SetStateLoading{}; // L2 experimental
using Game_GridRundown_ShouldPlay_t = bool(uint8_t variant, void *a2, void *a3, void *settings_ctx, void *a5);
inline constexpr fn<"Game_GridRundown_ShouldPlay", Game_GridRundown_ShouldPlay_t> Game_GridRundown_ShouldPlay{}; // L2 experimental
inline constexpr address<"Game_GridSafetyCarShotGscs_Initialise"> Game_GridSafetyCarShotGscs_Initialise{}; // L2 experimental
inline constexpr address<"Game_GridSafetyCarShot_Initialise"> Game_GridSafetyCarShot_Initialise{}; // L2 experimental
inline constexpr address<"Game_GridSetup_DriverBasedGrid"> Game_GridSetup_DriverBasedGrid{}; // L2 experimental
inline constexpr address<"Game_GridSetup_FindQualiResult"> Game_GridSetup_FindQualiResult{}; // L2 experimental
inline constexpr address<"Game_GridSetup_FindQualiResult_2"> Game_GridSetup_FindQualiResult_2{}; // L2 experimental
inline constexpr address<"Game_GridSetup_SetupStartingGrid"> Game_GridSetup_SetupStartingGrid{}; // L2 experimental
using Game_GridSlot_PlaceVehicle_t = void(void *grid, uint32_t slot, const float *xform, void *a4, float yaw, void *a6, void *a7, void *a8);
inline constexpr fn<"Game_GridSlot_PlaceVehicle", Game_GridSlot_PlaceVehicle_t> Game_GridSlot_PlaceVehicle{}; // L2 experimental
inline constexpr address<"Game_HandleUserEntitlementsUpdateTask_OnChildDone"> Game_HandleUserEntitlementsUpdateTask_OnChildDone{}; // L2 experimental
using Game_Handle_GarageCrewEvent_Thunk_t = uint64_t(void **cars, void *event);
inline constexpr fn<"Game_Handle_GarageCrewEvent_Thunk", Game_Handle_GarageCrewEvent_Thunk_t> Game_Handle_GarageCrewEvent_Thunk{}; // L2 experimental
using Game_Handle_SpecGarageCrewEvent_Thunk_t = uint64_t(void *self, void *event);
inline constexpr fn<"Game_Handle_SpecGarageCrewEvent_Thunk", Game_Handle_SpecGarageCrewEvent_Thunk_t> Game_Handle_SpecGarageCrewEvent_Thunk{}; // L2 experimental
inline constexpr address<"Game_HomeScreen_OnHomeScreenInfoResult"> Game_HomeScreen_OnHomeScreenInfoResult{}; // L2 experimental
inline constexpr address<"Game_HomeScreen_RequestHomeScreenInfo"> Game_HomeScreen_RequestHomeScreenInfo{}; // L2 experimental
inline constexpr address<"Game_HudDriverCardVM_OnBound"> Game_HudDriverCardVM_OnBound{}; // L2 experimental
inline constexpr address<"Game_HudDriverListVM_OnBound"> Game_HudDriverListVM_OnBound{}; // L2 experimental
inline constexpr address<"Game_HudFeaturedDriverVM_OnBound"> Game_HudFeaturedDriverVM_OnBound{}; // L2 experimental
inline constexpr address<"Game_HudFeaturedDriverVM_OnUnbound"> Game_HudFeaturedDriverVM_OnUnbound{}; // L2 experimental
using Game_HudVM_ResolveSystems_t = uint32_t(void *self, void *registry, void *systemMap);
inline constexpr fn<"Game_HudVM_ResolveSystems", Game_HudVM_ResolveSystems_t> Game_HudVM_ResolveSystems{}; // L2 experimental
inline constexpr address<"Game_InCarDriver_EvaluateAnimLayers"> Game_InCarDriver_EvaluateAnimLayers{}; // L2 experimental
inline constexpr address<"Game_InGameMenuVM_OnReturnToGarage"> Game_InGameMenuVM_OnReturnToGarage{}; // L2 experimental
inline constexpr address<"Game_InboxTabVM_OnTabActivated"> Game_InboxTabVM_OnTabActivated{}; // L2 experimental
inline constexpr address<"Game_InboxTabVM_OnTabDeactivated"> Game_InboxTabVM_OnTabDeactivated{}; // L2 experimental
inline constexpr address<"Game_Input_OnControlSchemeAdded"> Game_Input_OnControlSchemeAdded{}; // L2 experimental
using Game_InstantRace_SetupTrackList_t = void(void *comp);
inline constexpr fn<"Game_InstantRace_SetupTrackList", Game_InstantRace_SetupTrackList_t> Game_InstantRace_SetupTrackList{}; // L2 experimental
inline constexpr address<"Game_InviteManager_ChangeState"> Game_InviteManager_ChangeState{}; // L2 experimental
inline constexpr address<"Game_InviteManager_Join"> Game_InviteManager_Join{}; // L2 experimental
inline constexpr address<"Game_InviteManager_OnCareerLoadedSoC"> Game_InviteManager_OnCareerLoadedSoC{}; // L2 experimental
inline constexpr address<"Game_InviteManager_OnJoinCompleted"> Game_InviteManager_OnJoinCompleted{}; // L2 experimental
inline constexpr address<"Game_InviteManager_OnJoinCompleted_2"> Game_InviteManager_OnJoinCompleted_2{}; // L2 experimental
inline constexpr address<"Game_LapSectors_InitFromTrack"> Game_LapSectors_InitFromTrack{}; // L2 experimental
inline constexpr address<"Game_LeaderboardRowVM_Populate"> Game_LeaderboardRowVM_Populate{}; // L2 experimental
inline constexpr address<"Game_LiveryAssignVM_OnActivate"> Game_LiveryAssignVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_LiveryAssignVM_OnDeactivate"> Game_LiveryAssignVM_OnDeactivate{}; // L2 experimental
inline constexpr address<"Game_Livery_SetAllowSponsorsAndBadges"> Game_Livery_SetAllowSponsorsAndBadges{}; // L2 experimental
inline constexpr address<"Game_Load_MyTeamVehicleBases"> Game_Load_MyTeamVehicleBases{}; // L2 experimental
inline constexpr address<"Game_Load_ShopAndCustomisationLayers"> Game_Load_ShopAndCustomisationLayers{}; // L2 experimental
using Game_Load_TrackDatabaseFull_t = void(void *records_vec, void *db, void *a3, void *a4, void *a5);
inline constexpr fn<"Game_Load_TrackDatabaseFull", Game_Load_TrackDatabaseFull_t> Game_Load_TrackDatabaseFull{}; // L2 experimental
inline constexpr address<"Game_LoadingScreenVM_OnScreenActivated"> Game_LoadingScreenVM_OnScreenActivated{}; // L2 experimental
inline constexpr address<"Game_MatchingContext_OnPostRaceResultSyncEnter"> Game_MatchingContext_OnPostRaceResultSyncEnter{}; // L2 experimental
inline constexpr address<"Game_MatchingContext_OnSessionEnter"> Game_MatchingContext_OnSessionEnter{}; // L2 experimental
inline constexpr address<"Game_MatchingContext_OnSessionLobby"> Game_MatchingContext_OnSessionLobby{}; // L2 experimental
inline constexpr address<"Game_MatchingContext_OnSessionRestart"> Game_MatchingContext_OnSessionRestart{}; // L2 experimental
inline constexpr address<"Game_MatchingContext_OnStartRematch"> Game_MatchingContext_OnStartRematch{}; // L2 experimental
inline constexpr address<"Game_MatchingContext_SetRankedSearchParams"> Game_MatchingContext_SetRankedSearchParams{}; // L2 experimental
inline constexpr address<"Game_MatchingContext_SetupRankedGameSession"> Game_MatchingContext_SetupRankedGameSession{}; // L2 experimental
inline constexpr address<"Game_MidSession_SaveReplaySnapshot"> Game_MidSession_SaveReplaySnapshot{}; // L2 experimental
inline constexpr address<"Game_ModelProxyList_Resolve"> Game_ModelProxyList_Resolve{}; // L2 experimental
inline constexpr address<"Game_MpSeasonCompleteVM_OnActivate"> Game_MpSeasonCompleteVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_MultiVotingManager_CastVote"> Game_MultiVotingManager_CastVote{}; // L2 experimental
inline constexpr address<"Game_MultiVotingManager_HandleBallot"> Game_MultiVotingManager_HandleBallot{}; // L2 experimental
inline constexpr address<"Game_MultiVotingManager_HandleStatus"> Game_MultiVotingManager_HandleStatus{}; // L2 experimental
using Game_MusicOptions_CreateOption_t = void *(void *mgr, uint64_t optionId);
inline constexpr fn<"Game_MusicOptions_CreateOption", Game_MusicOptions_CreateOption_t> Game_MusicOptions_CreateOption{}; // L2 experimental, EAMusicOptionsManager
using Game_MyTeamAttributeSet_GetValue_t = float(void *attr_set, uint32_t team_key, void *attr_id);
inline constexpr fn<"Game_MyTeamAttributeSet_GetValue", Game_MyTeamAttributeSet_GetValue_t> Game_MyTeamAttributeSet_GetValue{}; // L2 experimental
using Game_MyTeamCarCustomisation_BuildPreviewState_t = void(void *self);
inline constexpr fn<"Game_MyTeamCarCustomisation_BuildPreviewState", Game_MyTeamCarCustomisation_BuildPreviewState_t> Game_MyTeamCarCustomisation_BuildPreviewState{}; // L2 experimental
inline constexpr address<"Game_MyTeamCarCustomisation_Enter"> Game_MyTeamCarCustomisation_Enter{}; // L2 experimental
using Game_MyTeamCarCustomisation_GetVideoWallContext_t = uint64_t(void *self);
inline constexpr fn<"Game_MyTeamCarCustomisation_GetVideoWallContext", Game_MyTeamCarCustomisation_GetVideoWallContext_t> Game_MyTeamCarCustomisation_GetVideoWallContext{}; // L2 experimental
inline constexpr address<"Game_MyTeamContract_SetDriversVisible"> Game_MyTeamContract_SetDriversVisible{}; // L2 experimental
inline constexpr address<"Game_MyTeamContract_ShowDriverComparison"> Game_MyTeamContract_ShowDriverComparison{}; // L2 experimental
using Game_MyTeamCustomisation_PlayVideoWallVideo_t = uint64_t(void *self, void *a2, void *a3, uint32_t kind);
inline constexpr fn<"Game_MyTeamCustomisation_PlayVideoWallVideo", Game_MyTeamCustomisation_PlayVideoWallVideo_t> Game_MyTeamCustomisation_PlayVideoWallVideo{}; // L2 experimental
using Game_MyTeamCustomisation_ResolveVideoTypeByHash_t = uint8_t *(uint8_t *out, int32_t name_hash, void *ctx);
inline constexpr fn<"Game_MyTeamCustomisation_ResolveVideoTypeByHash", Game_MyTeamCustomisation_ResolveVideoTypeByHash_t> Game_MyTeamCustomisation_ResolveVideoTypeByHash{}; // L2 experimental
inline constexpr address<"Game_MyTeamCustomisation_SpawnScene"> Game_MyTeamCustomisation_SpawnScene{}; // L2 experimental
inline constexpr address<"Game_MyTeamDriverMarket_Close"> Game_MyTeamDriverMarket_Close{}; // L2 experimental
inline constexpr address<"Game_MyTeamDriverMarket_Open"> Game_MyTeamDriverMarket_Open{}; // L2 experimental
inline constexpr address<"Game_MyTeamHq_SetupSceneForState"> Game_MyTeamHq_SetupSceneForState{}; // L2 experimental
inline constexpr address<"Game_MyTeamHq_SpawnFoyerCar"> Game_MyTeamHq_SpawnFoyerCar{}; // L2 experimental
inline constexpr address<"Game_MyTeamHq_SpawnFoyerDrivers"> Game_MyTeamHq_SpawnFoyerDrivers{}; // L2 experimental
inline constexpr address<"Game_MyTeamIconsDlcRemoved_Handler"> Game_MyTeamIconsDlcRemoved_Handler{}; // L2 experimental
inline constexpr address<"Game_MyTeamIntroScript_PollAssetsLoaded"> Game_MyTeamIntroScript_PollAssetsLoaded{}; // L2 experimental
inline constexpr address<"Game_MyTeamIntroScript_PreStart"> Game_MyTeamIntroScript_PreStart{}; // L2 experimental
using Game_MyTeamIntro_InitMaterialSlotStatics_t = void(void *obj, void *arg);
inline constexpr fn<"Game_MyTeamIntro_InitMaterialSlotStatics", Game_MyTeamIntro_InitMaterialSlotStatics_t> Game_MyTeamIntro_InitMaterialSlotStatics{}; // L2 experimental
inline constexpr address<"Game_MyTeamIntro_PreloadFacility"> Game_MyTeamIntro_PreloadFacility{}; // L2 experimental
inline constexpr address<"Game_MyTeamIntro_RegisterCinematicAssets"> Game_MyTeamIntro_RegisterCinematicAssets{}; // L2 experimental
inline constexpr address<"Game_MyTeamIntro_ReleaseMaterialSlots"> Game_MyTeamIntro_ReleaseMaterialSlots{}; // L2 experimental
inline constexpr address<"Game_MyTeamSetup_FlowMain"> Game_MyTeamSetup_FlowMain{}; // L2 experimental
inline constexpr address<"Game_MyTeamTeamAttributeSet_FindOrCreate"> Game_MyTeamTeamAttributeSet_FindOrCreate{}; // L2 experimental
inline constexpr address<"Game_NetFormationLap_BroadcastAll"> Game_NetFormationLap_BroadcastAll{}; // L2 experimental
inline constexpr address<"Game_NetFormationLap_HandleSessionState"> Game_NetFormationLap_HandleSessionState{}; // L2 experimental
inline constexpr address<"Game_NetFormationLap_SendToConnection"> Game_NetFormationLap_SendToConnection{}; // L2 experimental
inline constexpr address<"Game_NetFormationLap_SetVehicleLocal"> Game_NetFormationLap_SetVehicleLocal{}; // L2 experimental
inline constexpr address<"Game_NetFormationLap_Update"> Game_NetFormationLap_Update{}; // L2 experimental
inline constexpr address<"Game_NetMarshalZones_HandleMessage"> Game_NetMarshalZones_HandleMessage{}; // L2 experimental
inline constexpr address<"Game_NetMarshalZones_HostBroadcast"> Game_NetMarshalZones_HostBroadcast{}; // L2 experimental
inline constexpr address<"Game_NetMarshalZones_OnHostMigrated"> Game_NetMarshalZones_OnHostMigrated{}; // L2 experimental
inline constexpr address<"Game_NetMarshalZones_Update"> Game_NetMarshalZones_Update{}; // L2 experimental
inline constexpr address<"Game_NetPlayerSync_OnMutableRoleChanged"> Game_NetPlayerSync_OnMutableRoleChanged{}; // L2 experimental
inline constexpr address<"Game_NetPlayerSync_OnPlayerSetEntryDiscarded"> Game_NetPlayerSync_OnPlayerSetEntryDiscarded{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_SendCameraSelect"> Game_NetSafetyCarCinematic_SendCameraSelect{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_SendCarsPassedPits"> Game_NetSafetyCarCinematic_SendCarsPassedPits{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_SendIntroEnd"> Game_NetSafetyCarCinematic_SendIntroEnd{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_SendLapsRemaining"> Game_NetSafetyCarCinematic_SendLapsRemaining{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_SendNamedState"> Game_NetSafetyCarCinematic_SendNamedState{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_SendStart"> Game_NetSafetyCarCinematic_SendStart{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_UpdateCameraSelect"> Game_NetSafetyCarCinematic_UpdateCameraSelect{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_UpdateCarsPassedPits"> Game_NetSafetyCarCinematic_UpdateCarsPassedPits{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_UpdateIntroEnd"> Game_NetSafetyCarCinematic_UpdateIntroEnd{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_UpdateLapsRemaining"> Game_NetSafetyCarCinematic_UpdateLapsRemaining{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_UpdateNamedState"> Game_NetSafetyCarCinematic_UpdateNamedState{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCarCinematic_UpdateStart"> Game_NetSafetyCarCinematic_UpdateStart{}; // L2 experimental
inline constexpr address<"Game_NetSafetyCar_UnregisterStale"> Game_NetSafetyCar_UnregisterStale{}; // L2 experimental
inline constexpr address<"Game_NetStartLights_HandleStateMessage"> Game_NetStartLights_HandleStateMessage{}; // L2 experimental
inline constexpr address<"Game_NetStartLights_HandleTimeOffset"> Game_NetStartLights_HandleTimeOffset{}; // L2 experimental
inline constexpr address<"Game_NetStartLights_OnRoleChanged"> Game_NetStartLights_OnRoleChanged{}; // L2 experimental
inline constexpr address<"Game_NetworkReady_OnPauseSendSuperReady"> Game_NetworkReady_OnPauseSendSuperReady{}; // L2 experimental
inline constexpr address<"Game_NetworkReady_OnUnpauseSendSuperReady"> Game_NetworkReady_OnUnpauseSendSuperReady{}; // L2 experimental
inline constexpr address<"Game_NetworkReady_OnUnreadyMessage"> Game_NetworkReady_OnUnreadyMessage{}; // L2 experimental
inline constexpr address<"Game_NetworkReady_SendSuperReady"> Game_NetworkReady_SendSuperReady{}; // L2 experimental
inline constexpr address<"Game_NetworkReady_SetRelevantSet"> Game_NetworkReady_SetRelevantSet{}; // L2 experimental
inline constexpr address<"Game_NetworkSubsystem_SelectTitleConfig"> Game_NetworkSubsystem_SelectTitleConfig{}; // L2 experimental
inline constexpr address<"Game_NetworkUI_StartRankedDivisionSearch"> Game_NetworkUI_StartRankedDivisionSearch{}; // L2 experimental
inline constexpr address<"Game_NetworkUI_StartRankedRematching"> Game_NetworkUI_StartRankedRematching{}; // L2 experimental
inline constexpr address<"Game_Network_ApplyInitialPitAndGridState"> Game_Network_ApplyInitialPitAndGridState{}; // L2 experimental
inline constexpr address<"Game_Network_ApplyingCustomGrid"> Game_Network_ApplyingCustomGrid{}; // L2 experimental
using Game_Network_CategoryOptionUsing_t = void(void *self);
inline constexpr fn<"Game_Network_CategoryOptionUsing", Game_Network_CategoryOptionUsing_t> Game_Network_CategoryOptionUsing{}; // L2 experimental
using Game_Network_SendingRedFlagVehicleStatusUpdate_t = void(void *red_flag_state, const double *dt);
inline constexpr fn<"Game_Network_SendingRedFlagVehicleStatusUpdate", Game_Network_SendingRedFlagVehicleStatusUpdate_t> Game_Network_SendingRedFlagVehicleStatusUpdate{}; // L2 experimental
using Game_Network_SetScenario_t = void(void *session, uint8_t stat_id, const void *spec);
inline constexpr fn<"Game_Network_SetScenario", Game_Network_SetScenario_t> Game_Network_SetScenario{}; // L2 experimental
inline constexpr address<"Game_Network_TrackSetMatched"> Game_Network_TrackSetMatched{}; // L2 experimental
inline constexpr address<"Game_Objective_CheckFinishPosition"> Game_Objective_CheckFinishPosition{}; // L2 experimental
inline constexpr address<"Game_Objective_CheckOvertakeTarget"> Game_Objective_CheckOvertakeTarget{}; // L2 experimental
inline constexpr address<"Game_Objective_CheckPositionTarget"> Game_Objective_CheckPositionTarget{}; // L2 experimental
inline constexpr address<"Game_OnMessageDriverPlayerChanged"> Game_OnMessageDriverPlayerChanged{}; // L2 experimental
inline constexpr address<"Game_OnlineCareer_FadeOutForSessionRequest"> Game_OnlineCareer_FadeOutForSessionRequest{}; // L2 experimental
inline constexpr address<"Game_OnlineCareer_OnFirstRequestResult"> Game_OnlineCareer_OnFirstRequestResult{}; // L2 experimental
inline constexpr address<"Game_OnlineCareer_OnSecondRequestResult"> Game_OnlineCareer_OnSecondRequestResult{}; // L2 experimental
inline constexpr address<"Game_OnlineDocumentComponent_Create"> Game_OnlineDocumentComponent_Create{}; // L2 experimental
inline constexpr address<"Game_OnlineDocumentSubComponent_Ctor"> Game_OnlineDocumentSubComponent_Ctor{}; // L2 experimental
using Game_OnlineLogInTask_Init_t = int(void *self, void *serviceProvider, void *componentMap);
inline constexpr fn<"Game_OnlineLogInTask_Init", Game_OnlineLogInTask_Init_t> Game_OnlineLogInTask_Init{}; // L2 experimental, OnlineServerLogInTask
inline constexpr address<"Game_OnlineRole_HandleSessionDataRequest"> Game_OnlineRole_HandleSessionDataRequest{}; // L2 experimental
inline constexpr address<"Game_OnlineRole_OnConnectionVerified"> Game_OnlineRole_OnConnectionVerified{}; // L2 experimental
inline constexpr address<"Game_OnlineServerLogInTask_FinishWithState"> Game_OnlineServerLogInTask_FinishWithState{}; // L2 experimental
inline constexpr address<"Game_OnlineServerLogInTask_OnError"> Game_OnlineServerLogInTask_OnError{}; // L2 experimental
inline constexpr address<"Game_OnlineServerLogInTask_OnStart"> Game_OnlineServerLogInTask_OnStart{}; // L2 experimental
inline constexpr address<"Game_OnlineServerLogInTask_SetFinished"> Game_OnlineServerLogInTask_SetFinished{}; // L2 experimental
inline constexpr address<"Game_OnlineServerLogInTask_SetFinishedReleaseRef"> Game_OnlineServerLogInTask_SetFinishedReleaseRef{}; // L2 experimental
inline constexpr address<"Game_OnlineServer_TriggerLogout"> Game_OnlineServer_TriggerLogout{}; // L2 experimental
inline constexpr address<"Game_OnlineSubsystemTaskOnServiceBase_Ctor"> Game_OnlineSubsystemTaskOnServiceBase_Ctor{}; // L2 experimental
inline constexpr address<"Game_OnlineSystemEADPAsyncTask_Update"> Game_OnlineSystemEADPAsyncTask_Update{}; // L2 experimental
inline constexpr address<"Game_OnlineSystemGetFriendRequestListTask_OnPlayerCards"> Game_OnlineSystemGetFriendRequestListTask_OnPlayerCards{}; // L2 experimental
inline constexpr address<"Game_OnlineSystemGetFriendRequestListTask_Update"> Game_OnlineSystemGetFriendRequestListTask_Update{}; // L2 experimental
inline constexpr address<"Game_OptionsColourCorrectionVM_OnScreenActivated"> Game_OptionsColourCorrectionVM_OnScreenActivated{}; // L2 experimental
inline constexpr address<"Game_OptionsCrossplayVM_OnScreenActivated"> Game_OptionsCrossplayVM_OnScreenActivated{}; // L2 experimental
using Game_OptionsStore_FindOption_t = uint64_t(void *map, const char **name);
inline constexpr fn<"Game_OptionsStore_FindOption", Game_OptionsStore_FindOption_t> Game_OptionsStore_FindOption{}; // L2 experimental
inline constexpr address<"Game_OptionsSystem_ResolveOptionValue"> Game_OptionsSystem_ResolveOptionValue{}; // L2 experimental
inline constexpr address<"Game_OptionsUI_ApplyFilterLevelCollectChanges"> Game_OptionsUI_ApplyFilterLevelCollectChanges{}; // L2 experimental
inline constexpr address<"Game_PagedBuffer_Transfer"> Game_PagedBuffer_Transfer{}; // L2 experimental
inline constexpr address<"Game_PagedBuffer_UsePageAt"> Game_PagedBuffer_UsePageAt{}; // L2 experimental
inline constexpr address<"Game_ParcFermeCutScene_Initialise"> Game_ParcFermeCutScene_Initialise{}; // L2 experimental
inline constexpr address<"Game_Particles_BindMeshModel"> Game_Particles_BindMeshModel{}; // L2 experimental
inline constexpr address<"Game_Particles_CreateEffectNode"> Game_Particles_CreateEffectNode{}; // L2 experimental
inline constexpr address<"Game_Particles_CreateStoredEffect"> Game_Particles_CreateStoredEffect{}; // L2 experimental
inline constexpr address<"Game_Particles_FindVehicleIndex"> Game_Particles_FindVehicleIndex{}; // L2 experimental
inline constexpr address<"Game_Particles_RemoveEffect"> Game_Particles_RemoveEffect{}; // L2 experimental
inline constexpr address<"Game_Particles_RestoreLongLivedParticles"> Game_Particles_RestoreLongLivedParticles{}; // L2 experimental
inline constexpr address<"Game_Particles_RestoreParticleSlot"> Game_Particles_RestoreParticleSlot{}; // L2 experimental
inline constexpr address<"Game_Particles_StoreParticleSlot"> Game_Particles_StoreParticleSlot{}; // L2 experimental
inline constexpr address<"Game_Particles_UpdateEmitters"> Game_Particles_UpdateEmitters{}; // L2 experimental
inline constexpr address<"Game_PassiveObjectives_MyTeamAttributeModifier"> Game_PassiveObjectives_MyTeamAttributeModifier{}; // L2 experimental
using Game_PauseAvailabilityVM_OnActivate_t = void(void *vm);
inline constexpr fn<"Game_PauseAvailabilityVM_OnActivate", Game_PauseAvailabilityVM_OnActivate_t> Game_PauseAvailabilityVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_PelvisHeightModifier_Init"> Game_PelvisHeightModifier_Init{}; // L2 experimental
inline constexpr address<"Game_PhotoModeHelper_CaptureFrame"> Game_PhotoModeHelper_CaptureFrame{}; // L2 experimental
using Game_PhotoMode_InitDependencies_t = int(void *self, void *serviceProvider);
inline constexpr fn<"Game_PhotoMode_InitDependencies", Game_PhotoMode_InitDependencies_t> Game_PhotoMode_InitDependencies{}; // L2 experimental
inline constexpr address<"Game_PitBoxCarActor_Update"> Game_PitBoxCarActor_Update{}; // L2 experimental
inline constexpr address<"Game_PitCrewActor_Update"> Game_PitCrewActor_Update{}; // L2 experimental
inline constexpr address<"Game_PitDriveThrough_OnNoAnimFadeOut"> Game_PitDriveThrough_OnNoAnimFadeOut{}; // L2 experimental
inline constexpr address<"Game_PitLaneSystem_Multiplayer_handlePitLaneEventMessage"> Game_PitLaneSystem_Multiplayer_handlePitLaneEventMessage{}; // L2 experimental, PitLaneSystem::multiplayer_handlePitLaneEventMessage
inline constexpr address<"Game_PitLane_EnterControlRelease"> Game_PitLane_EnterControlRelease{}; // L2 experimental
inline constexpr address<"Game_PitLane_EnterLimitZone"> Game_PitLane_EnterLimitZone{}; // L2 experimental
inline constexpr address<"Game_PitLane_ExitControlRelease"> Game_PitLane_ExitControlRelease{}; // L2 experimental
inline constexpr address<"Game_PitLane_ExitLimitZone"> Game_PitLane_ExitLimitZone{}; // L2 experimental
inline constexpr address<"Game_PitLane_HandleEnterPitBox"> Game_PitLane_HandleEnterPitBox{}; // L2 experimental
inline constexpr address<"Game_PitLane_HoldInGarage"> Game_PitLane_HoldInGarage{}; // L2 experimental
inline constexpr address<"Game_PitLane_HoldInPitBox"> Game_PitLane_HoldInPitBox{}; // L2 experimental
inline constexpr address<"Game_PitLane_IllegalPitEntry"> Game_PitLane_IllegalPitEntry{}; // L2 experimental
using Game_PitLane_MoveDownSpline_t = void(void *pitlane, uint8_t car, void *teleport_req, bool alt_spline, float dt, bool p6, bool blend, bool p8, bool p9);
inline constexpr fn<"Game_PitLane_MoveDownSpline", Game_PitLane_MoveDownSpline_t> Game_PitLane_MoveDownSpline{}; // L2 experimental
inline constexpr address<"Game_PitLane_OnGarageCrewMessage"> Game_PitLane_OnGarageCrewMessage{}; // L2 experimental
using Game_PitLane_PutVehicleInPitBox_t = void(void *pitlane, uint8_t vehicle);
inline constexpr fn<"Game_PitLane_PutVehicleInPitBox", Game_PitLane_PutVehicleInPitBox_t> Game_PitLane_PutVehicleInPitBox{}; // L2 experimental
inline constexpr address<"Game_PitLane_ResetCarToTrack"> Game_PitLane_ResetCarToTrack{}; // L2 experimental
inline constexpr address<"Game_PitLane_SetCrewReady"> Game_PitLane_SetCrewReady{}; // L2 experimental
inline constexpr address<"Game_PitLane_SetDoPracticePitStop"> Game_PitLane_SetDoPracticePitStop{}; // L2 experimental
inline constexpr address<"Game_PitLane_SetDoPracticePitStop_2"> Game_PitLane_SetDoPracticePitStop_2{}; // L2 experimental
inline constexpr address<"Game_PitLane_VehicleEntering"> Game_PitLane_VehicleEntering{}; // L2 experimental
inline constexpr address<"Game_PitLane_VehicleEnteringInternal"> Game_PitLane_VehicleEnteringInternal{}; // L2 experimental
inline constexpr address<"Game_PitLane_VehicleExiting"> Game_PitLane_VehicleExiting{}; // L2 experimental
inline constexpr address<"Game_PitStopActionObserver_SetTransitionSequence"> Game_PitStopActionObserver_SetTransitionSequence{}; // L2 experimental, PitStopActionObserver::setTransitionSequence
inline constexpr address<"Game_PitStopInstance_PlaySequence"> Game_PitStopInstance_PlaySequence{}; // L2 experimental, PitStopInstance::playSequence
inline constexpr address<"Game_PitStopLogic_ChangeState"> Game_PitStopLogic_ChangeState{}; // L2 experimental
inline constexpr address<"Game_PitStopLogic_NeSequenceClipTargetImpl"> Game_PitStopLogic_NeSequenceClipTargetImpl{}; // L2 experimental, PitStopLogic::NeSequenceClipTargetImpl
inline constexpr address<"Game_PitStopLogic_SetTransition"> Game_PitStopLogic_SetTransition{}; // L2 experimental
inline constexpr address<"Game_PitStopLogic_StateInGarage"> Game_PitStopLogic_StateInGarage{}; // L2 experimental
inline constexpr address<"Game_PitStopLogic_StateJobComplete"> Game_PitStopLogic_StateJobComplete{}; // L2 experimental
inline constexpr address<"Game_Pitlane_Multiplayer_resetToTrack"> Game_Pitlane_Multiplayer_resetToTrack{}; // L2 experimental
inline constexpr address<"Game_Pitlane_Multiplayer_teleportToGarage"> Game_Pitlane_Multiplayer_teleportToGarage{}; // L2 experimental
inline constexpr address<"Game_Pitlane_Multiplayer_triggerGarageDriveOut"> Game_Pitlane_Multiplayer_triggerGarageDriveOut{}; // L2 experimental
inline constexpr address<"Game_Pitlane_Multiplayer_vehicleEnteringPitLane"> Game_Pitlane_Multiplayer_vehicleEnteringPitLane{}; // L2 experimental
using Game_Pitlane_SetVehicleStatus_t = void(void *self, uint8_t vehicle, uint32_t status);
inline constexpr fn<"Game_Pitlane_SetVehicleStatus", Game_Pitlane_SetVehicleStatus_t> Game_Pitlane_SetVehicleStatus{}; // L2 experimental
inline constexpr address<"Game_PitstopCutScene_Initialise"> Game_PitstopCutScene_Initialise{}; // L2 experimental
using Game_Place_Vehicle_t = void(void *car, const char *tag, int priority, const float *mtx44_a, const float *mtx44_b, float dt, uint32_t flags);
inline constexpr fn<"Game_Place_Vehicle", Game_Place_Vehicle_t> Game_Place_Vehicle{}; // L2 experimental
inline constexpr address<"Game_PlatformAuthorisationTask_OnStart"> Game_PlatformAuthorisationTask_OnStart{}; // L2 experimental
inline constexpr address<"Game_PlatformAuthorisationTask_Update"> Game_PlatformAuthorisationTask_Update{}; // L2 experimental
inline constexpr address<"Game_PlayerManager_HandleControllerUnplugged"> Game_PlayerManager_HandleControllerUnplugged{}; // L2 experimental
inline constexpr address<"Game_PlayerSetSubsystem_CreateHumanPlayers"> Game_PlayerSetSubsystem_CreateHumanPlayers{}; // L2 experimental, PlayerSetSubsystem::createHumanPlayers
using Game_PlayerSetSubsystem_HandleUpdatePresenceNameMessage_t = void(void *player_set, void *msg);
inline constexpr fn<"Game_PlayerSetSubsystem_HandleUpdatePresenceNameMessage", Game_PlayerSetSubsystem_HandleUpdatePresenceNameMessage_t> Game_PlayerSetSubsystem_HandleUpdatePresenceNameMessage{}; // L2 experimental, PlayerSetSubsystem::handleUpdatePresenceNameMessage
inline constexpr address<"Game_PlayerSetSubsystem_Init"> Game_PlayerSetSubsystem_Init{}; // L2 experimental
inline constexpr address<"Game_PlayerSetSubsystem_OnNewConnection"> Game_PlayerSetSubsystem_OnNewConnection{}; // L2 experimental
inline constexpr address<"Game_PlayerSetSubsystem_UpdatePresence"> Game_PlayerSetSubsystem_UpdatePresence{}; // L2 experimental
inline constexpr address<"Game_PlayerSet_ChangePlayerConnectionId"> Game_PlayerSet_ChangePlayerConnectionId{}; // L2 experimental
inline constexpr address<"Game_PlayerSet_ChangePlayerIsWatching"> Game_PlayerSet_ChangePlayerIsWatching{}; // L2 experimental
inline constexpr address<"Game_PlayerSet_SetEntriesPlaying"> Game_PlayerSet_SetEntriesPlaying{}; // L2 experimental
inline constexpr address<"Game_PlayerSet_SetEntryLocked"> Game_PlayerSet_SetEntryLocked{}; // L2 experimental
inline constexpr address<"Game_Pod2019FirstScript_Initialise"> Game_Pod2019FirstScript_Initialise{}; // L2 experimental
inline constexpr address<"Game_Pod2019FirstScript_OnShutDownComplete"> Game_Pod2019FirstScript_OnShutDownComplete{}; // L2 experimental
inline constexpr address<"Game_Pod2019FirstScript_Shutdown"> Game_Pod2019FirstScript_Shutdown{}; // L2 experimental
inline constexpr address<"Game_Pod2019First_ReleasePodiumThumbnails"> Game_Pod2019First_ReleasePodiumThumbnails{}; // L2 experimental
inline constexpr address<"Game_Pod2019First_RequestPodiumThumbnails"> Game_Pod2019First_RequestPodiumThumbnails{}; // L2 experimental
inline constexpr address<"Game_PodiumCarFiller_Initialise"> Game_PodiumCarFiller_Initialise{}; // L2 experimental
inline constexpr address<"Game_PodiumParcFermeScript_SetStateLoading"> Game_PodiumParcFermeScript_SetStateLoading{}; // L2 experimental
inline constexpr address<"Game_PodiumParcFerme_Initialise"> Game_PodiumParcFerme_Initialise{}; // L2 experimental
inline constexpr address<"Game_PostraceReactionsD01Fail_Initialise"> Game_PostraceReactionsD01Fail_Initialise{}; // L2 experimental
inline constexpr address<"Game_PostraceReactionsD02Success_Initialise"> Game_PostraceReactionsD02Success_Initialise{}; // L2 experimental
inline constexpr address<"Game_PostraceReactionsD03Success_Initialise"> Game_PostraceReactionsD03Success_Initialise{}; // L2 experimental
inline constexpr address<"Game_PracticeProgrammeSystem_IsolatePlayer"> Game_PracticeProgrammeSystem_IsolatePlayer{}; // L2 experimental
inline constexpr address<"Game_PracticeProgrammeSystem_OnCompareVehicleChanged"> Game_PracticeProgrammeSystem_OnCompareVehicleChanged{}; // L2 experimental
inline constexpr address<"Game_PracticeProgrammeSystem_OnCompareVehicleChanged_2"> Game_PracticeProgrammeSystem_OnCompareVehicleChanged_2{}; // L2 experimental
inline constexpr address<"Game_PracticeProgrammes_Init"> Game_PracticeProgrammes_Init{}; // L2 experimental
using Game_PracticeTrackAcclimatisation_RefreshState_t = void(void *self);
inline constexpr fn<"Game_PracticeTrackAcclimatisation_RefreshState", Game_PracticeTrackAcclimatisation_RefreshState_t> Game_PracticeTrackAcclimatisation_RefreshState{}; // L2 experimental
using Game_Predict_StrategyWetness_t = float(void *self, const float *window3);
inline constexpr fn<"Game_Predict_StrategyWetness", Game_Predict_StrategyWetness_t> Game_Predict_StrategyWetness{}; // L2 experimental
inline constexpr address<"Game_ProLeaguesVM_OnExit"> Game_ProLeaguesVM_OnExit{}; // L2 experimental
inline constexpr address<"Game_ProLeaguesVM_OnLeaguesResult"> Game_ProLeaguesVM_OnLeaguesResult{}; // L2 experimental
inline constexpr address<"Game_ProfanityFilter_CheckWord"> Game_ProfanityFilter_CheckWord{}; // L2 experimental
inline constexpr address<"Game_ProfanityFilter_FilterText"> Game_ProfanityFilter_FilterText{}; // L2 experimental
inline constexpr address<"Game_ProfanityFilter_MatchList"> Game_ProfanityFilter_MatchList{}; // L2 experimental
inline constexpr address<"Game_ProgressBasedFocusTowardsTrackCentreCameraModifier_OnEnable"> Game_ProgressBasedFocusTowardsTrackCentreCameraModifier_OnEnable{}; // L2 experimental
inline constexpr address<"Game_ProgressBasedRollCameraModifier_OnEnable"> Game_ProgressBasedRollCameraModifier_OnEnable{}; // L2 experimental
inline constexpr address<"Game_ProgressBasedStaticZoomCameraModifier_OnEnable"> Game_ProgressBasedStaticZoomCameraModifier_OnEnable{}; // L2 experimental
inline constexpr address<"Game_ProgressBasedTargetInhibitorCameraBehaviour_OnEnable"> Game_ProgressBasedTargetInhibitorCameraBehaviour_OnEnable{}; // L2 experimental
inline constexpr address<"Game_ProgressBasedTargetOffsetCameraModifier_OnEnable"> Game_ProgressBasedTargetOffsetCameraModifier_OnEnable{}; // L2 experimental
inline constexpr address<"Game_ProgressFloatInterpolator_Evaluate"> Game_ProgressFloatInterpolator_Evaluate{}; // L2 experimental
inline constexpr address<"Game_ProximityBasedShakeCameraModifier_OnEnable"> Game_ProximityBasedShakeCameraModifier_OnEnable{}; // L2 experimental
inline constexpr address<"Game_QualObjReaction01_Initialise"> Game_QualObjReaction01_Initialise{}; // L2 experimental
inline constexpr address<"Game_RaceHelper_GetVehicleByPosition"> Game_RaceHelper_GetVehicleByPosition{}; // L2 experimental
inline constexpr address<"Game_RaceHelper_GetVehicleIdByChampPos"> Game_RaceHelper_GetVehicleIdByChampPos{}; // L2 experimental
inline constexpr address<"Game_RaceHelper_GetVehicleIndexByChampPos"> Game_RaceHelper_GetVehicleIndexByChampPos{}; // L2 experimental
using Game_RaceMarshalSystem_Update_t = void(void *system, double *dt);
inline constexpr fn<"Game_RaceMarshalSystem_Update", Game_RaceMarshalSystem_Update_t> Game_RaceMarshalSystem_Update{}; // L2 experimental
inline constexpr address<"Game_RaceSession_FinaliseAndSetLapsPct"> Game_RaceSession_FinaliseAndSetLapsPct{}; // L2 experimental
using Game_RaceStrategyVm_Populate_t = void(void *vm);
inline constexpr fn<"Game_RaceStrategyVm_Populate", Game_RaceStrategyVm_Populate_t> Game_RaceStrategyVm_Populate{}; // L2 experimental
inline constexpr address<"Game_RacingVehicles_Initialise"> Game_RacingVehicles_Initialise{}; // L2 experimental
inline constexpr address<"Game_RainfallSystem_Initialise"> Game_RainfallSystem_Initialise{}; // L2 experimental
inline constexpr address<"Game_RankedDivision_StartSearch"> Game_RankedDivision_StartSearch{}; // L2 experimental
inline constexpr address<"Game_RankedDivision_UpdateSearchAvailability"> Game_RankedDivision_UpdateSearchAvailability{}; // L2 experimental
inline constexpr address<"Game_RankedMatchingProcessor_CreateSession"> Game_RankedMatchingProcessor_CreateSession{}; // L2 experimental
inline constexpr address<"Game_RankedMatchingProcessor_Search"> Game_RankedMatchingProcessor_Search{}; // L2 experimental
using Game_Read_GridConfig_t = void(void *placement);
inline constexpr fn<"Game_Read_GridConfig", Game_Read_GridConfig_t> Game_Read_GridConfig{}; // L2 experimental
using Game_Read_QualifyingConfig_t = void(void *track_data_system);
inline constexpr fn<"Game_Read_QualifyingConfig", Game_Read_QualifyingConfig_t> Game_Read_QualifyingConfig{}; // L2 experimental
inline constexpr address<"Game_Read_RendererTrackInformationSystemConfig"> Game_Read_RendererTrackInformationSystemConfig{}; // L2 experimental
using Game_Read_StoryForcedTitleUpdateValueSetting_t = bool(void *ctx);
inline constexpr fn<"Game_Read_StoryForcedTitleUpdateValueSetting", Game_Read_StoryForcedTitleUpdateValueSetting_t> Game_Read_StoryForcedTitleUpdateValueSetting{}; // L2 experimental
using Game_Read_TrackInfoConfig_t = void(void *self);
inline constexpr fn<"Game_Read_TrackInfoConfig", Game_Read_TrackInfoConfig_t> Game_Read_TrackInfoConfig{}; // L2 experimental
using Game_Read_UIMovieBuildSetting_t = uint64_t(void *self);
inline constexpr fn<"Game_Read_UIMovieBuildSetting", Game_Read_UIMovieBuildSetting_t> Game_Read_UIMovieBuildSetting{}; // L2 experimental
using Game_Refresh_PerformanceComparison_t = void(void *comparison);
inline constexpr fn<"Game_Refresh_PerformanceComparison", Game_Refresh_PerformanceComparison_t> Game_Refresh_PerformanceComparison{}; // L2 experimental
using Game_RendererModule_Initialise_t = int(void *module);
inline constexpr fn<"Game_RendererModule_Initialise", Game_RendererModule_Initialise_t> Game_RendererModule_Initialise{}; // L2 experimental
inline constexpr address<"Game_Renderer_CreateRenderPasses"> Game_Renderer_CreateRenderPasses{}; // L2 experimental
inline constexpr address<"Game_RepairTimes_FadeIn"> Game_RepairTimes_FadeIn{}; // L2 experimental
inline constexpr address<"Game_RepairTimes_FadeOut"> Game_RepairTimes_FadeOut{}; // L2 experimental
inline constexpr address<"Game_RepairTimes_FinishFadeIn"> Game_RepairTimes_FinishFadeIn{}; // L2 experimental
inline constexpr address<"Game_RepairTimes_OnVehicleStatusFadeIn"> Game_RepairTimes_OnVehicleStatusFadeIn{}; // L2 experimental
inline constexpr address<"Game_ReplayBuffer_WriteStats"> Game_ReplayBuffer_WriteStats{}; // L2 experimental
inline constexpr address<"Game_ReplayFile_ReadFooter"> Game_ReplayFile_ReadFooter{}; // L2 experimental
inline constexpr address<"Game_ReplaySystem_LoadMidSessionData"> Game_ReplaySystem_LoadMidSessionData{}; // L2 experimental
inline constexpr address<"Game_ReplaySystem_LoadSnapshotFile"> Game_ReplaySystem_LoadSnapshotFile{}; // L2 experimental
using Game_ReplaySystem_ReadDbSettings_t = void(void *sys);
inline constexpr fn<"Game_ReplaySystem_ReadDbSettings", Game_ReplaySystem_ReadDbSettings_t> Game_ReplaySystem_ReadDbSettings{}; // L2 experimental
inline constexpr address<"Game_ReplaySystem_SaveMidSessionData"> Game_ReplaySystem_SaveMidSessionData{}; // L2 experimental
inline constexpr address<"Game_Replay_LoadSavedReplayJob"> Game_Replay_LoadSavedReplayJob{}; // L2 experimental
inline constexpr address<"Game_Replay_RestoreMainModuleBuffers"> Game_Replay_RestoreMainModuleBuffers{}; // L2 experimental
inline constexpr address<"Game_ResultsContainerVM_OnActivate"> Game_ResultsContainerVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_ResultsContainerVM_OnDeactivate"> Game_ResultsContainerVM_OnDeactivate{}; // L2 experimental
inline constexpr address<"Game_RetrieveConsumableEntitlementsTask_Ctor"> Game_RetrieveConsumableEntitlementsTask_Ctor{}; // L2 experimental
inline constexpr address<"Game_RetrieveConsumableEntitlementsTask_RequestSteamPrices"> Game_RetrieveConsumableEntitlementsTask_RequestSteamPrices{}; // L2 experimental
inline constexpr address<"Game_RewardSequence_PlayCutscene"> Game_RewardSequence_PlayCutscene{}; // L2 experimental
inline constexpr address<"Game_RndPropScene_Spawn"> Game_RndPropScene_Spawn{}; // L2 experimental
using Game_Rnd_CalcUpgradeTypeProgress_t = float(void *const *instances, uint8_t type, uint8_t state, const void *meta);
inline constexpr fn<"Game_Rnd_CalcUpgradeTypeProgress", Game_Rnd_CalcUpgradeTypeProgress_t> Game_Rnd_CalcUpgradeTypeProgress{}; // L2 experimental
using Game_Rnd_FillVehicleUpgrades_t = void(void *component, uint8_t car, uint8_t a3, uint8_t a4);
inline constexpr fn<"Game_Rnd_FillVehicleUpgrades", Game_Rnd_FillVehicleUpgrades_t> Game_Rnd_FillVehicleUpgrades{}; // L2 experimental
inline constexpr address<"Game_RttTaskManager_CleanupTask"> Game_RttTaskManager_CleanupTask{}; // L2 experimental
inline constexpr address<"Game_RttTaskManager_OnTaskComplete"> Game_RttTaskManager_OnTaskComplete{}; // L2 experimental
inline constexpr address<"Game_RttTaskManager_ProcessCompletedTasks"> Game_RttTaskManager_ProcessCompletedTasks{}; // L2 experimental
inline constexpr address<"Game_RttTaskManager_QueueTask"> Game_RttTaskManager_QueueTask{}; // L2 experimental
inline constexpr address<"Game_RttTaskManager_StartTask"> Game_RttTaskManager_StartTask{}; // L2 experimental
inline constexpr address<"Game_SSL_FetchKeyMaterial"> Game_SSL_FetchKeyMaterial{}; // L2 experimental
using Game_SaveData_DeserialiseContainer_t = int *(SaveDataContainer *self, int *result, void *bag, uint8_t reset_on_error, uint64_t groups);
inline constexpr fn<"Game_SaveData_DeserialiseContainer", Game_SaveData_DeserialiseContainer_t> Game_SaveData_DeserialiseContainer{}; // L3 stable
using Game_SaveData_SerialiseContainer_t = int *(SaveDataContainer *self, int *result, void *bag, uint64_t groups);
inline constexpr fn<"Game_SaveData_SerialiseContainer", Game_SaveData_SerialiseContainer_t> Game_SaveData_SerialiseContainer{}; // L2 experimental
using Game_SaveData_SerialiseContainer_2_t = int *(SaveDataContainer *self, int *result, void *stream, void *a4, uint64_t groups, void *a6);
inline constexpr fn<"Game_SaveData_SerialiseContainer_2", Game_SaveData_SerialiseContainer_2_t> Game_SaveData_SerialiseContainer_2{}; // L2 experimental
using Game_SaveData_WriteBag_t = int(NeBagCodec *codec, void *bag);
inline constexpr fn<"Game_SaveData_WriteBag", Game_SaveData_WriteBag_t> Game_SaveData_WriteBag{}; // L3 stable
using Game_SaveMgr_RequestProfileSave_Thunk_t = bool(void *save_mgr, void *callback, uint8_t a3, uint8_t a4);
inline constexpr fn<"Game_SaveMgr_RequestProfileSave_Thunk", Game_SaveMgr_RequestProfileSave_Thunk_t> Game_SaveMgr_RequestProfileSave_Thunk{}; // L2 experimental
using Game_SaveSlotManager_Deserialise_t = int(void *iface, void *bag, void *ctx);
inline constexpr fn<"Game_SaveSlotManager_Deserialise", Game_SaveSlotManager_Deserialise_t> Game_SaveSlotManager_Deserialise{}; // L2 experimental
using Game_SaveSlotManager_SelectSlot_t = int(void *manager, uint64_t slot_id, bool force, uint32_t a4);
inline constexpr fn<"Game_SaveSlotManager_SelectSlot", Game_SaveSlotManager_SelectSlot_t> Game_SaveSlotManager_SelectSlot{}; // L2 experimental
inline constexpr address<"Game_SceneManagerSpawn_MergeScene"> Game_SceneManagerSpawn_MergeScene{}; // L2 experimental
using Game_SceneManagerSpawn_SpawnScene_t = void *(void *mgr, const uint8_t *opts, void *scene, const float *mtx44, void *ov_a, void *ov_b, uint64_t p7);
inline constexpr fn<"Game_SceneManagerSpawn_SpawnScene", Game_SceneManagerSpawn_SpawnScene_t> Game_SceneManagerSpawn_SpawnScene{}; // L2 experimental
inline constexpr address<"Game_SceneManager_DestroyScene"> Game_SceneManager_DestroyScene{}; // L2 experimental
using Game_Scene_UpdateInstanceTransformsBatch_t = void(void *scene, uint8_t actor_update_flag);
inline constexpr fn<"Game_Scene_UpdateInstanceTransformsBatch", Game_Scene_UpdateInstanceTransformsBatch_t> Game_Scene_UpdateInstanceTransformsBatch{}; // L2 experimental
inline constexpr address<"Game_ScoreBreakdownVM_OnActivate"> Game_ScoreBreakdownVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_ScoreBreakdownVM_OnDeactivate"> Game_ScoreBreakdownVM_OnDeactivate{}; // L2 experimental
inline constexpr address<"Game_SeasonDocument_CloseDocument"> Game_SeasonDocument_CloseDocument{}; // L2 experimental
inline constexpr address<"Game_SeasonDocument_HandleRebuildSeason"> Game_SeasonDocument_HandleRebuildSeason{}; // L2 experimental
inline constexpr address<"Game_SeasonDocument_SetTrackSetNameCustom"> Game_SeasonDocument_SetTrackSetNameCustom{}; // L2 experimental
using Game_SeasonLength_ApplyPreset_t = void(void *component, uint8_t preset);
inline constexpr fn<"Game_SeasonLength_ApplyPreset", Game_SeasonLength_ApplyPreset_t> Game_SeasonLength_ApplyPreset{}; // L2 experimental
using Game_SeasonLength_PushCalendarEntry_t = uint64_t(void *calendar, const void *entry);
inline constexpr fn<"Game_SeasonLength_PushCalendarEntry", Game_SeasonLength_PushCalendarEntry_t> Game_SeasonLength_PushCalendarEntry{}; // L2 experimental
inline constexpr address<"Game_SeasonLobbyUi_SetTrackSetByMode"> Game_SeasonLobbyUi_SetTrackSetByMode{}; // L2 experimental
inline constexpr address<"Game_SeasonLobby_ResetTrackSet"> Game_SeasonLobby_ResetTrackSet{}; // L2 experimental
inline constexpr address<"Game_SeasonUI_SetupChampionshipVariables"> Game_SeasonUI_SetupChampionshipVariables{}; // L2 experimental
inline constexpr address<"Game_SecondWorld_RequestLoad"> Game_SecondWorld_RequestLoad{}; // L2 experimental
inline constexpr address<"Game_SecretMeetings_Initialise"> Game_SecretMeetings_Initialise{}; // L2 experimental
inline constexpr address<"Game_SessionHud_OnPlayerDroppedIn"> Game_SessionHud_OnPlayerDroppedIn{}; // L2 experimental
inline constexpr address<"Game_SessionHud_OnSessionEnter"> Game_SessionHud_OnSessionEnter{}; // L2 experimental
inline constexpr address<"Game_SessionHud_OnSessionExit"> Game_SessionHud_OnSessionExit{}; // L2 experimental
inline constexpr address<"Game_SessionResults_PopulateTable"> Game_SessionResults_PopulateTable{}; // L2 experimental
inline constexpr address<"Game_SessionResults_ReleaseThumbnails"> Game_SessionResults_ReleaseThumbnails{}; // L2 experimental
inline constexpr address<"Game_SessionSettings_Reset"> Game_SessionSettings_Reset{}; // L2 experimental
inline constexpr address<"Game_SessionSettings_SetSessionId"> Game_SessionSettings_SetSessionId{}; // L2 experimental
inline constexpr address<"Game_SessionState_EnableOnDemandTextureStreaming"> Game_SessionState_EnableOnDemandTextureStreaming{}; // L2 experimental
inline constexpr address<"Game_SessionStatsTracker_OnPlayerDroppedIn"> Game_SessionStatsTracker_OnPlayerDroppedIn{}; // L2 experimental
inline constexpr address<"Game_SessionStatsTracker_ResetForSession"> Game_SessionStatsTracker_ResetForSession{}; // L2 experimental
inline constexpr address<"Game_SessionVector_EmplaceBack"> Game_SessionVector_EmplaceBack{}; // L2 experimental
using Game_Set_VehicleController_Thunk_t = void(void *system, uint64_t vehicle, uint32_t ai, int32_t flag);
inline constexpr fn<"Game_Set_VehicleController_Thunk", Game_Set_VehicleController_Thunk_t> Game_Set_VehicleController_Thunk{}; // L2 experimental
inline constexpr address<"Game_SettingsControlsScreenViewModel_ApplyFilterRebuildList"> Game_SettingsControlsScreenViewModel_ApplyFilterRebuildList{}; // L2 experimental
inline constexpr address<"Game_SettingsControlsScreenViewModel_RebindAxis"> Game_SettingsControlsScreenViewModel_RebindAxis{}; // L2 experimental
inline constexpr address<"Game_SettingsTabbingBaseViewModel_ApplyFilterRebuildCategories"> Game_SettingsTabbingBaseViewModel_ApplyFilterRebuildCategories{}; // L2 experimental
inline constexpr address<"Game_SettingsTabbingBaseViewModel_ApplyFilterRefresh"> Game_SettingsTabbingBaseViewModel_ApplyFilterRefresh{}; // L2 experimental
inline constexpr address<"Game_ShaderPackage_RegisterSponsorScreenEffects"> Game_ShaderPackage_RegisterSponsorScreenEffects{}; // L2 experimental
inline constexpr address<"Game_Showroom_HideInfoPanel"> Game_Showroom_HideInfoPanel{}; // L2 experimental
inline constexpr address<"Game_Showroom_ShowInfoPanel"> Game_Showroom_ShowInfoPanel{}; // L2 experimental
inline constexpr address<"Game_SillySeasonScript_PollAssetsLoaded"> Game_SillySeasonScript_PollAssetsLoaded{}; // L2 experimental
inline constexpr address<"Game_SillySeason_Initialise"> Game_SillySeason_Initialise{}; // L2 experimental
inline constexpr address<"Game_SkinnedActor_PopulateSlotArray"> Game_SkinnedActor_PopulateSlotArray{}; // L2 experimental
inline constexpr address<"Game_SocialHubFriendsVM_OnFriendSelected"> Game_SocialHubFriendsVM_OnFriendSelected{}; // L2 experimental
inline constexpr address<"Game_SocialHubFriendsVM_OnInviteResult"> Game_SocialHubFriendsVM_OnInviteResult{}; // L2 experimental
inline constexpr address<"Game_SocialNotifications_Update"> Game_SocialNotifications_Update{}; // L2 experimental
inline constexpr address<"Game_Social_OnFriendSearchDone"> Game_Social_OnFriendSearchDone{}; // L2 experimental
inline constexpr address<"Game_Sort_SponsorEntries_AdjustHeap"> Game_Sort_SponsorEntries_AdjustHeap{}; // L2 experimental
inline constexpr address<"Game_Sort_SponsorEntries_Introsort"> Game_Sort_SponsorEntries_Introsort{}; // L2 experimental
inline constexpr address<"Game_Sort_SponsorEntries_MedianPivot"> Game_Sort_SponsorEntries_MedianPivot{}; // L2 experimental
inline constexpr address<"Game_SpecialistsCutSceneScript_Initialise"> Game_SpecialistsCutSceneScript_Initialise{}; // L2 experimental
inline constexpr address<"Game_SpecialistsRewards_Initialise"> Game_SpecialistsRewards_Initialise{}; // L2 experimental
using Game_SpeechSystem_PostLine_t = void(void *speech, void *line, uint8_t retrigger);
inline constexpr fn<"Game_SpeechSystem_PostLine", Game_SpeechSystem_PostLine_t> Game_SpeechSystem_PostLine{}; // L2 experimental
inline constexpr address<"Game_SpeedBasedFoVZoomCameraModifier_OnEnable"> Game_SpeedBasedFoVZoomCameraModifier_OnEnable{}; // L2 experimental
inline constexpr address<"Game_SponsorLoadout_BuildList"> Game_SponsorLoadout_BuildList{}; // L2 experimental
inline constexpr address<"Game_StabilisationCameraModifier_OnEnable"> Game_StabilisationCameraModifier_OnEnable{}; // L2 experimental
inline constexpr address<"Game_StatEvent_OnLapCompleted"> Game_StatEvent_OnLapCompleted{}; // L2 experimental
inline constexpr address<"Game_StatEvent_OnRaceFinished"> Game_StatEvent_OnRaceFinished{}; // L2 experimental
inline constexpr address<"Game_StatEvent_OnSectorCompleted"> Game_StatEvent_OnSectorCompleted{}; // L2 experimental
inline constexpr address<"Game_StatEvent_SubscribeMessageHandlers"> Game_StatEvent_SubscribeMessageHandlers{}; // L2 experimental
inline constexpr address<"Game_StatEvent_UnsubscribeMessageHandlers"> Game_StatEvent_UnsubscribeMessageHandlers{}; // L2 experimental
inline constexpr address<"Game_StatEvent_WriteCompleteF1WRace"> Game_StatEvent_WriteCompleteF1WRace{}; // L2 experimental
inline constexpr address<"Game_StatEvent_WriteF1WeekendFinished"> Game_StatEvent_WriteF1WeekendFinished{}; // L2 experimental
inline constexpr address<"Game_StatEvent_WriteFacilityUpgraded"> Game_StatEvent_WriteFacilityUpgraded{}; // L2 experimental
inline constexpr address<"Game_StatEvent_WriteFeaturedEventsCompleted"> Game_StatEvent_WriteFeaturedEventsCompleted{}; // L2 experimental
inline constexpr address<"Game_StatEvent_WriteFreeTimeBetweenRaces"> Game_StatEvent_WriteFreeTimeBetweenRaces{}; // L2 experimental
inline constexpr address<"Game_StatEvent_WriteRacesFinished"> Game_StatEvent_WriteRacesFinished{}; // L2 experimental
inline constexpr address<"Game_StatEvent_WriteResourcesPoints"> Game_StatEvent_WriteResourcesPoints{}; // L2 experimental
inline constexpr address<"Game_StatEvent_WriteRnDAndFacilityUpgrades"> Game_StatEvent_WriteRnDAndFacilityUpgrades{}; // L2 experimental
inline constexpr address<"Game_StateSession_OnStateExit"> Game_StateSession_OnStateExit{}; // L2 experimental
inline constexpr address<"Game_Store_SubscribeMessages"> Game_Store_SubscribeMessages{}; // L2 experimental
inline constexpr address<"Game_Store_UnsubscribeMessages"> Game_Store_UnsubscribeMessages{}; // L2 experimental
inline constexpr address<"Game_StoryHubCall_HandleInput"> Game_StoryHubCall_HandleInput{}; // L2 experimental
inline constexpr address<"Game_StoryHubCall_HangUp"> Game_StoryHubCall_HangUp{}; // L2 experimental
inline constexpr address<"Game_StoryHubCall_SetPaused"> Game_StoryHubCall_SetPaused{}; // L2 experimental
inline constexpr address<"Game_StoryHubCall_Update"> Game_StoryHubCall_Update{}; // L2 experimental
inline constexpr address<"Game_StoryHubObjectives_OnAlwaysOnChanged"> Game_StoryHubObjectives_OnAlwaysOnChanged{}; // L2 experimental
inline constexpr address<"Game_StoryHubObjectives_OnAnimEvent"> Game_StoryHubObjectives_OnAnimEvent{}; // L2 experimental
inline constexpr address<"Game_StoryHubObjectives_PlayBonusCloseAnim"> Game_StoryHubObjectives_PlayBonusCloseAnim{}; // L2 experimental
inline constexpr address<"Game_StoryHubObjectives_PlayCloseAnim"> Game_StoryHubObjectives_PlayCloseAnim{}; // L2 experimental
inline constexpr address<"Game_StoryHubObjectives_PlayOpenAnim"> Game_StoryHubObjectives_PlayOpenAnim{}; // L2 experimental
inline constexpr address<"Game_StoryHubObjectives_Refresh"> Game_StoryHubObjectives_Refresh{}; // L2 experimental
inline constexpr address<"Game_StoryHubObjectives_StartFadeTimer"> Game_StoryHubObjectives_StartFadeTimer{}; // L2 experimental
inline constexpr address<"Game_StoryHubObjectives_Update"> Game_StoryHubObjectives_Update{}; // L2 experimental
inline constexpr address<"Game_StoryHub_BuildCinematicsList"> Game_StoryHub_BuildCinematicsList{}; // L2 experimental
inline constexpr address<"Game_StoryHub_BuildConversationList"> Game_StoryHub_BuildConversationList{}; // L2 experimental
inline constexpr address<"Game_StoryModeObjective_Update"> Game_StoryModeObjective_Update{}; // L2 experimental
inline constexpr address<"Game_StoryModeSelect_OnBeginFadeOut"> Game_StoryModeSelect_OnBeginFadeOut{}; // L2 experimental
inline constexpr address<"Game_StoryPressInterview_Initialise"> Game_StoryPressInterview_Initialise{}; // L2 experimental
inline constexpr address<"Game_StoryTeamHub_Initialise"> Game_StoryTeamHub_Initialise{}; // L2 experimental
inline constexpr address<"Game_StreamingAssetLoadTask_Update"> Game_StreamingAssetLoadTask_Update{}; // L2 experimental
inline constexpr address<"Game_TabHostVM_OnCommand"> Game_TabHostVM_OnCommand{}; // L2 experimental
inline constexpr address<"Game_TabHostVM_OnTabActivated"> Game_TabHostVM_OnTabActivated{}; // L2 experimental
inline constexpr address<"Game_TabHostVM_OnTabDeactivated"> Game_TabHostVM_OnTabDeactivated{}; // L2 experimental
inline constexpr address<"Game_TalkingPoints_UpdateBroadcastFades"> Game_TalkingPoints_UpdateBroadcastFades{}; // L2 experimental
using Game_TeamHQAdvanceViewModel_OnCommandRaised_t = uint64_t(void *vm, NeStr **cmd, void *data);
inline constexpr fn<"Game_TeamHQAdvanceViewModel_OnCommandRaised", Game_TeamHQAdvanceViewModel_OnCommandRaised_t> Game_TeamHQAdvanceViewModel_OnCommandRaised{}; // L2 experimental, game::TeamHQAdvanceViewModel::onCommandRaised,
using Game_TeamHQAdvanceViewModel_PressAdvance_t = void(void *vm);
inline constexpr fn<"Game_TeamHQAdvanceViewModel_PressAdvance", Game_TeamHQAdvanceViewModel_PressAdvance_t> Game_TeamHQAdvanceViewModel_PressAdvance{}; // L2 experimental
using Game_TeamHQAdvanceViewModel_ShowTodayCards_t = bool(void *vm);
inline constexpr fn<"Game_TeamHQAdvanceViewModel_ShowTodayCards", Game_TeamHQAdvanceViewModel_ShowTodayCards_t> Game_TeamHQAdvanceViewModel_ShowTodayCards{}; // L2 experimental
using Game_TeamHQAdvanceViewModel_StopAdvance_t = void(void *vm);
inline constexpr fn<"Game_TeamHQAdvanceViewModel_StopAdvance", Game_TeamHQAdvanceViewModel_StopAdvance_t> Game_TeamHQAdvanceViewModel_StopAdvance{}; // L2 experimental
using Game_TeamHQAdvanceViewModel_Update_t = uint64_t(void *vm);
inline constexpr fn<"Game_TeamHQAdvanceViewModel_Update", Game_TeamHQAdvanceViewModel_Update_t> Game_TeamHQAdvanceViewModel_Update{}; // L2 experimental
inline constexpr address<"Game_TeamHQTimeline_AnimateDeptEvent"> Game_TeamHQTimeline_AnimateDeptEvent{}; // L2 experimental
using Game_Teleport_Vehicle_t = void(void *car, const char *name, float *prev_mtx, float *new_mtx, float dt, uint32_t flags);
inline constexpr fn<"Game_Teleport_Vehicle", Game_Teleport_Vehicle_t> Game_Teleport_Vehicle{}; // L2 experimental
inline constexpr address<"Game_TerminalDamage_Initialise"> Game_TerminalDamage_Initialise{}; // L2 experimental
inline constexpr address<"Game_ThrustmasterLcd_UpdateSectors"> Game_ThrustmasterLcd_UpdateSectors{}; // L2 experimental
inline constexpr address<"Game_Thrustmaster_SetWheelRange"> Game_Thrustmaster_SetWheelRange{}; // L2 experimental
using Game_TileList_PushChildSelection_Thunk_t = void(void *widget, void *child);
inline constexpr fn<"Game_TileList_PushChildSelection_Thunk", Game_TileList_PushChildSelection_Thunk_t> Game_TileList_PushChildSelection_Thunk{}; // L2 experimental
inline constexpr address<"Game_TimeAttackGates_CreateGateModel"> Game_TimeAttackGates_CreateGateModel{}; // L2 experimental
using Game_TimeManager_OnResetFixedTimeStep_t = int(void *ctx, const void *msg);
inline constexpr fn<"Game_TimeManager_OnResetFixedTimeStep", Game_TimeManager_OnResetFixedTimeStep_t> Game_TimeManager_OnResetFixedTimeStep{}; // L2 experimental
inline constexpr address<"Game_TimeOfDayDebug_Update"> Game_TimeOfDayDebug_Update{}; // L2 experimental
inline constexpr address<"Game_TimeTrialMode_Shutdown"> Game_TimeTrialMode_Shutdown{}; // L2 experimental
inline constexpr address<"Game_TimingBoard_FormatPositionText"> Game_TimingBoard_FormatPositionText{}; // L2 experimental
using Game_TitleScreenVM_OnScreenActivated_t = void(void *vm);
inline constexpr fn<"Game_TitleScreenVM_OnScreenActivated", Game_TitleScreenVM_OnScreenActivated_t> Game_TitleScreenVM_OnScreenActivated{}; // L2 experimental
inline constexpr address<"Game_TodData_FindParam"> Game_TodData_FindParam{}; // L2 experimental
inline constexpr address<"Game_TodData_FindScenarioParam"> Game_TodData_FindScenarioParam{}; // L2 experimental
inline constexpr address<"Game_TodParamBlock_SetScenario"> Game_TodParamBlock_SetScenario{}; // L2 experimental
inline constexpr address<"Game_TrackAcclimatisation_CalcTyreTarget"> Game_TrackAcclimatisation_CalcTyreTarget{}; // L2 experimental
inline constexpr address<"Game_TrackCameraShots_StopShot"> Game_TrackCameraShots_StopShot{}; // L2 experimental
using Game_TrackData_GetTrack_t = void *(void *system, const void *track_ref, const uint8_t *query);
inline constexpr fn<"Game_TrackData_GetTrack", Game_TrackData_GetTrack_t> Game_TrackData_GetTrack{}; // L2 experimental
inline constexpr address<"Game_TrackExtraYacht_Initialise"> Game_TrackExtraYacht_Initialise{}; // L2 experimental
inline constexpr address<"Game_TrackModelComponent_Initialise"> Game_TrackModelComponent_Initialise{}; // L2 experimental
inline constexpr address<"Game_TrackSelectVM_OnScreenActivated"> Game_TrackSelectVM_OnScreenActivated{}; // L2 experimental
inline constexpr address<"Game_TrackSpaceDataResource_Load"> Game_TrackSpaceDataResource_Load{}; // L2 experimental
inline constexpr address<"Game_TrackSpaceTracker_Reset"> Game_TrackSpaceTracker_Reset{}; // L2 experimental
inline constexpr address<"Game_TrackSpace_GetDistanceAtPos"> Game_TrackSpace_GetDistanceAtPos{}; // L2 experimental
inline constexpr address<"Game_TrackSpace_LoadXml"> Game_TrackSpace_LoadXml{}; // L2 experimental
inline constexpr address<"Game_TrackSpace_RangeOverlaps"> Game_TrackSpace_RangeOverlaps{}; // L2 experimental
using Game_TrackSpline_FindSegmentAtDistance_t = void *(void *spline, float distance);
inline constexpr fn<"Game_TrackSpline_FindSegmentAtDistance", Game_TrackSpline_FindSegmentAtDistance_t> Game_TrackSpline_FindSegmentAtDistance{}; // L2 experimental
inline constexpr address<"Game_TrackSpline_ResolveRouteEnds"> Game_TrackSpline_ResolveRouteEnds{}; // L2 experimental
inline constexpr address<"Game_TransactionSystem_OnRequestPurchaseComplete_A"> Game_TransactionSystem_OnRequestPurchaseComplete_A{}; // L2 experimental, game::GameTransactionSystem::onRequestPurchaseComplete(PurchaseError)
inline constexpr address<"Game_TransactionSystem_OnRequestPurchaseComplete_B"> Game_TransactionSystem_OnRequestPurchaseComplete_B{}; // L2 experimental, game::GameTransactionSystem::onRequestPurchaseComplete(PurchaseError))
inline constexpr address<"Game_TrophyCabinetTabVM_OnActivate"> Game_TrophyCabinetTabVM_OnActivate{}; // L2 experimental
using Game_TyreAllocationComponent_SetPresetChoiceForHumanDriver_t = void(void *component, uint32_t preset, int local_player);
inline constexpr fn<"Game_TyreAllocationComponent_SetPresetChoiceForHumanDriver", Game_TyreAllocationComponent_SetPresetChoiceForHumanDriver_t> Game_TyreAllocationComponent_SetPresetChoiceForHumanDriver{}; // L2 experimental, TyreAllocationComponent::setPresetChoiceForHumanDriver
inline constexpr address<"Game_TyreSystem_SwapTyreModel"> Game_TyreSystem_SwapTyreModel{}; // L2 experimental
inline constexpr address<"Game_TyreWearDynamicObjective_Activate"> Game_TyreWearDynamicObjective_Activate{}; // L2 experimental
inline constexpr address<"Game_TyreWearRelevanceCriterion_Update"> Game_TyreWearRelevanceCriterion_Update{}; // L2 experimental
inline constexpr address<"Game_UDPPreferences_OnLocalPreferenceChange"> Game_UDPPreferences_OnLocalPreferenceChange{}; // L2 experimental
inline constexpr address<"Game_UDPPreferences_OnLocalShowPlayerNameChange"> Game_UDPPreferences_OnLocalShowPlayerNameChange{}; // L2 experimental
inline constexpr address<"Game_UIConnectionManager_ShowConnectionLostPopup"> Game_UIConnectionManager_ShowConnectionLostPopup{}; // L2 experimental
inline constexpr address<"Game_UIThumbnailSystem_FindDriverCustomisationWill"> Game_UIThumbnailSystem_FindDriverCustomisationWill{}; // L2 experimental
using Game_UITrack_Apply_DevContractTrack_t = void(void *self, void *cfg);
inline constexpr fn<"Game_UITrack_Apply_DevContractTrack", Game_UITrack_Apply_DevContractTrack_t> Game_UITrack_Apply_DevContractTrack{}; // L2 experimental
using Game_UITrack_FindTrackInfoWithFallback_t = void(void *contract, const char *track_name);
inline constexpr fn<"Game_UITrack_FindTrackInfoWithFallback", Game_UITrack_FindTrackInfoWithFallback_t> Game_UITrack_FindTrackInfoWithFallback{}; // L2 experimental
using Game_UITrack_Select_DefaultTrack_t = void(void *self, void *cfg);
inline constexpr fn<"Game_UITrack_Select_DefaultTrack", Game_UITrack_Select_DefaultTrack_t> Game_UITrack_Select_DefaultTrack{}; // L2 experimental
inline constexpr address<"Game_UI_BroadcastGridRundownViewModel"> Game_UI_BroadcastGridRundownViewModel{}; // L2 experimental
inline constexpr address<"Game_UI_BroadcastGridRundownViewModel_2"> Game_UI_BroadcastGridRundownViewModel_2{}; // L2 experimental
inline constexpr address<"Game_UiMessageScreen_Destructor"> Game_UiMessageScreen_Destructor{}; // L2 experimental
inline constexpr address<"Game_VehicleAntiStallSystem_GetStateByPlayerId"> Game_VehicleAntiStallSystem_GetStateByPlayerId{}; // L2 experimental
inline constexpr address<"Game_VehicleAntiStallSystem_IsLaunchPending"> Game_VehicleAntiStallSystem_IsLaunchPending{}; // L2 experimental
inline constexpr address<"Game_VehicleAntiStallSystem_IsStalled"> Game_VehicleAntiStallSystem_IsStalled{}; // L2 experimental
inline constexpr address<"Game_VehicleAntiStallSystem_IsStartPromptActive"> Game_VehicleAntiStallSystem_IsStartPromptActive{}; // L2 experimental
inline constexpr address<"Game_VehicleCustomisationReplicationManager_AddReplicatedPlayer"> Game_VehicleCustomisationReplicationManager_AddReplicatedPlayer{}; // L2 experimental
inline constexpr address<"Game_VehicleLapProgress_CommitNetLapDelta"> Game_VehicleLapProgress_CommitNetLapDelta{}; // L2 experimental
inline constexpr address<"Game_VehiclePitlaneManager_ApplyVehicleRemotePitStatusInternal"> Game_VehiclePitlaneManager_ApplyVehicleRemotePitStatusInternal{}; // L2 experimental, VehiclePitlaneManager::applyVehicleRemotePitStatusInternal
using Game_VehiclePlacementSystem_Connect_t = uint32_t(void *self, void *owner, void *registry);
inline constexpr fn<"Game_VehiclePlacementSystem_Connect", Game_VehiclePlacementSystem_Connect_t> Game_VehiclePlacementSystem_Connect{}; // L2 experimental
inline constexpr address<"Game_VehiclePlacementSystem_HandleEncodedGridMessage"> Game_VehiclePlacementSystem_HandleEncodedGridMessage{}; // L2 experimental, VehiclePlacementSystem::handleEncodedGridMessage
inline constexpr address<"Game_VehicleProgressManager_AddVehicleSync"> Game_VehicleProgressManager_AddVehicleSync{}; // L2 experimental
inline constexpr address<"Game_VehicleProgressManager_RefreshAuthority"> Game_VehicleProgressManager_RefreshAuthority{}; // L2 experimental
inline constexpr address<"Game_VehicleProgressSystem_EraseNetProgressRecords"> Game_VehicleProgressSystem_EraseNetProgressRecords{}; // L2 experimental
inline constexpr address<"Game_VehicleProgressSystem_GetDistanceData"> Game_VehicleProgressSystem_GetDistanceData{}; // L2 experimental
inline constexpr address<"Game_VehicleProgressSystem_GetLapProgress"> Game_VehicleProgressSystem_GetLapProgress{}; // L2 experimental
inline constexpr address<"Game_VehicleProgressSystem_OnVehiclePlacedOnGrid"> Game_VehicleProgressSystem_OnVehiclePlacedOnGrid{}; // L2 experimental, VehicleProgressSystem::onVehiclePlacedOnGrid
inline constexpr address<"Game_VehicleProgressSystem_OnVehicleRemoved"> Game_VehicleProgressSystem_OnVehicleRemoved{}; // L2 experimental
inline constexpr address<"Game_VehicleProgressSystem_SendOnHostMigration"> Game_VehicleProgressSystem_SendOnHostMigration{}; // L2 experimental
inline constexpr address<"Game_VehicleProgressSystem_SendProgressToConnection"> Game_VehicleProgressSystem_SendProgressToConnection{}; // L2 experimental
inline constexpr address<"Game_VehicleProgressSystem_SubscribeAndClearNet"> Game_VehicleProgressSystem_SubscribeAndClearNet{}; // L2 experimental
inline constexpr address<"Game_VehicleRaceStart_SendRaceStartMessages"> Game_VehicleRaceStart_SendRaceStartMessages{}; // L2 experimental
inline constexpr address<"Game_VendorA_Initialise"> Game_VendorA_Initialise{}; // L2 experimental
inline constexpr address<"Game_VfxManager_Initialise"> Game_VfxManager_Initialise{}; // L2 experimental
using Game_Vfx_UpdateWheelTyreInstance_t = void(void *studio, uint8_t vehicle_id, uint32_t wheel, void *tyre_vis, void *tyre_asset, const void *wheel_params);
inline constexpr fn<"Game_Vfx_UpdateWheelTyreInstance", Game_Vfx_UpdateWheelTyreInstance_t> Game_Vfx_UpdateWheelTyreInstance{}; // L2 experimental
inline constexpr address<"Game_VictoryVO_DetectWinnerAndNotify"> Game_VictoryVO_DetectWinnerAndNotify{}; // L2 experimental
inline constexpr address<"Game_VictoryVO_OnSpeechFinished"> Game_VictoryVO_OnSpeechFinished{}; // L2 experimental
inline constexpr address<"Game_VictoryVO_PlayVictoryVO"> Game_VictoryVO_PlayVictoryVO{}; // L2 experimental
using Game_VideoExportModule_Initialise_t = int(void *module);
inline constexpr fn<"Game_VideoExportModule_Initialise", Game_VideoExportModule_Initialise_t> Game_VideoExportModule_Initialise{}; // L2 experimental
inline constexpr address<"Game_VirtualHelpPageVM_OnActivate"> Game_VirtualHelpPageVM_OnActivate{}; // L2 experimental
inline constexpr address<"Game_VirtualHelpPageVM_OnDeactivate"> Game_VirtualHelpPageVM_OnDeactivate{}; // L2 experimental
inline constexpr address<"Game_VirtualPerformance_Initialise"> Game_VirtualPerformance_Initialise{}; // L2 experimental
inline constexpr address<"Game_VirtualSession_Simulate"> Game_VirtualSession_Simulate{}; // L2 experimental
inline constexpr address<"Game_VoPlaySoundEffectNode_Execute"> Game_VoPlaySoundEffectNode_Execute{}; // L2 experimental
inline constexpr address<"Game_VotingManager_CastVote"> Game_VotingManager_CastVote{}; // L2 experimental
inline constexpr address<"Game_VotingManager_Destroy"> Game_VotingManager_Destroy{}; // L2 experimental
inline constexpr address<"Game_VotingManager_HandleStatus"> Game_VotingManager_HandleStatus{}; // L2 experimental
inline constexpr address<"Game_VotingManager_SetStatus"> Game_VotingManager_SetStatus{}; // L2 experimental
inline constexpr address<"Game_WonDriverChampionship_Initialise"> Game_WonDriverChampionship_Initialise{}; // L2 experimental
inline constexpr address<"Game_WonDriverChampionship_Initialise_2"> Game_WonDriverChampionship_Initialise_2{}; // L2 experimental
inline constexpr address<"Game_WrinkleModifierMorphRig_Parse"> Game_WrinkleModifierMorphRig_Parse{}; // L2 experimental
using Game_Write_AnalyticsJsonObject_t = void *(void *out, void *ctx, int64_t rec, int64_t writer);
inline constexpr fn<"Game_Write_AnalyticsJsonObject", Game_Write_AnalyticsJsonObject_t> Game_Write_AnalyticsJsonObject{}; // L2 experimental
inline constexpr data<"g_Game_F1WorldEntryPoint", uint32_t> g_Game_F1WorldEntryPoint{}; // L2 experimental
inline constexpr data<"g_Game_FlowLinkStay", uint32_t> g_Game_FlowLinkStay{}; // L2 experimental
inline constexpr data<"g_Game_Manager", void *> g_Game_Manager{}; // L2 experimental
inline constexpr data<"g_Game_MessageBus", void *> g_Game_MessageBus{}; // L2 experimental
inline constexpr data<"g_Game_OptionsStore", void *> g_Game_OptionsStore{}; // L2 experimental
inline constexpr data<"g_Game_RaceStartSystem", void*> g_Game_RaceStartSystem{}; // L2 experimental
inline constexpr data<"g_Game_Root", void *> g_Game_Root{}; // L2 experimental
inline constexpr data<"g_Game_Scenarios", void *> g_Game_Scenarios{}; // L2 experimental
inline constexpr data<"g_Game_TrackLength", float> g_Game_TrackLength{}; // L2 experimental
inline constexpr data<"g_Game_VehicleRecordManager", void *> g_Game_VehicleRecordManager{}; // L2 experimental
inline constexpr data<"g_Game_VehicleStateRoot", void *> g_Game_VehicleStateRoot{}; // L2 experimental
inline constexpr data<"g_Game_WeatherSurfaceState", void*> g_Game_WeatherSurfaceState{}; // L2 experimental
inline constexpr data<"k_Game_TrackListComponentDesc", void *> k_Game_TrackListComponentDesc{}; // L2 experimental, TrackListComponent
inline constexpr site<"site_Game_AcclimPenalty_Entry"> site_Game_AcclimPenalty_Entry{}; // L2 experimental
inline constexpr site<"site_Game_AcclimPenalty_Return"> site_Game_AcclimPenalty_Return{}; // L2 experimental
inline constexpr call<"site_Game_CareerContinue_DriverSlotTitle"> site_Game_CareerContinue_DriverSlotTitle{}; // L2 experimental
inline constexpr site<"site_Game_CareerEventsSequencerFirstRecord"> site_Game_CareerEventsSequencerFirstRecord{}; // L2 experimental
inline constexpr site<"site_Game_CareerEventsSequencerPlayerDriver"> site_Game_CareerEventsSequencerPlayerDriver{}; // L2 experimental
inline constexpr site<"site_Game_CareerEventsSequencerPlayerDriverJoin"> site_Game_CareerEventsSequencerPlayerDriverJoin{}; // L2 experimental
inline constexpr site<"site_Game_CareerEventsSequencerRecordsJoin"> site_Game_CareerEventsSequencerRecordsJoin{}; // L2 experimental
inline constexpr site<"site_Game_CareerEventsSequencerTeamMateDriver"> site_Game_CareerEventsSequencerTeamMateDriver{}; // L2 experimental
inline constexpr site<"site_Game_CareerEventsSequencerTeamMateDriverJoin"> site_Game_CareerEventsSequencerTeamMateDriverJoin{}; // L2 experimental
inline constexpr site<"site_Game_CareerWizardConfirm_NextRoute"> site_Game_CareerWizardConfirm_NextRoute{}; // L2 experimental
inline constexpr site<"site_Game_CareerWizard_BackRoute"> site_Game_CareerWizard_BackRoute{}; // L2 experimental
inline constexpr call<"site_Game_DriverSelectHeader_BuildTabs"> site_Game_DriverSelectHeader_BuildTabs{}; // L2 experimental
inline constexpr site<"site_Game_DriverSelect_BackRoute"> site_Game_DriverSelect_BackRoute{}; // L2 experimental
inline constexpr call<"site_Game_DriverSelect_BadgeFormat"> site_Game_DriverSelect_BadgeFormat{}; // L2 experimental
inline constexpr site<"site_Game_DriverSelect_NumberResult"> site_Game_DriverSelect_NumberResult{}; // L2 experimental
inline constexpr call<"site_Game_DriverSelect_TeamLogoFormat"> site_Game_DriverSelect_TeamLogoFormat{}; // L2 experimental
inline constexpr site<"site_Game_ErsNewLap_NextVehicle"> site_Game_ErsNewLap_NextVehicle{}; // L2 experimental
inline constexpr site<"site_Game_ErsNewLap_VehicleLookup"> site_Game_ErsNewLap_VehicleLookup{}; // L2 experimental
inline constexpr site<"site_Game_GarageCrew_SceneRead"> site_Game_GarageCrew_SceneRead{}; // L2 experimental
inline constexpr site<"site_Game_GarageCrew_SceneTest"> site_Game_GarageCrew_SceneTest{}; // L2 experimental
inline constexpr site<"site_Game_GarageCrew_SpawnVector"> site_Game_GarageCrew_SpawnVector{}; // L2 experimental
inline constexpr site<"site_Game_GarageSetups_EntryStep"> site_Game_GarageSetups_EntryStep{}; // L2 experimental
inline constexpr site<"site_Game_GarageSetups_NotFound"> site_Game_GarageSetups_NotFound{}; // L2 experimental
inline constexpr site<"site_Game_GhostlapTrace_RivalLogBoard"> site_Game_GhostlapTrace_RivalLogBoard{}; // L2 experimental
inline constexpr site<"site_Game_GhostlapTrace_RivalLogDone"> site_Game_GhostlapTrace_RivalLogDone{}; // L2 experimental
inline constexpr site<"site_Game_GrandPrixContinueVm_F1wCarCheck"> site_Game_GrandPrixContinueVm_F1wCarCheck{}; // L2 experimental
inline constexpr site<"site_Game_GrandPrixContinueVm_F1wWarningCheck"> site_Game_GrandPrixContinueVm_F1wWarningCheck{}; // L2 experimental
inline constexpr site<"site_Game_MyTeamVehicleBases_AiConfigLoad"> site_Game_MyTeamVehicleBases_AiConfigLoad{}; // L2 experimental
inline constexpr site<"site_Game_MyTeamVehicleBases_NextBase"> site_Game_MyTeamVehicleBases_NextBase{}; // L2 experimental
inline constexpr site<"site_Game_MyTeamVehicleBases_SaveRecord"> site_Game_MyTeamVehicleBases_SaveRecord{}; // L2 experimental
inline constexpr site<"site_Game_PitLane_BoxPoseFalse"> site_Game_PitLane_BoxPoseFalse{}; // L2 experimental
inline constexpr site<"site_Game_PitLane_BoxSlotRead"> site_Game_PitLane_BoxSlotRead{}; // L2 experimental
inline constexpr site<"site_Game_PitStops_CrewTemplateRead"> site_Game_PitStops_CrewTemplateRead{}; // L2 experimental
inline constexpr site<"site_Game_PitStops_CrewTemplateSkip"> site_Game_PitStops_CrewTemplateSkip{}; // L2 experimental
inline constexpr site<"site_Game_SaveData_ContainersChildren"> site_Game_SaveData_ContainersChildren{}; // L3 stable
inline constexpr site<"site_Game_SaveData_ContainersCount"> site_Game_SaveData_ContainersCount{}; // L3 stable
inline constexpr site<"site_Game_SaveData_ContainersDone"> site_Game_SaveData_ContainersDone{}; // L2 experimental
inline constexpr site<"site_Game_SaveData_UnclaimedChild"> site_Game_SaveData_UnclaimedChild{}; // L3 stable
inline constexpr site<"site_Game_SaveSlot_Selected"> site_Game_SaveSlot_Selected{}; // L2 experimental
inline constexpr site<"site_Game_TeamSelect_NumberResult"> site_Game_TeamSelect_NumberResult{}; // L2 experimental
inline constexpr call<"site_Game_TeamSelect_TeamLogoFormat"> site_Game_TeamSelect_TeamLogoFormat{}; // L2 experimental
inline constexpr site<"site_Game_TheatreList_EntryStep"> site_Game_TheatreList_EntryStep{}; // L2 experimental
inline constexpr site<"site_Game_TheatreList_EntryStepAlt"> site_Game_TheatreList_EntryStepAlt{}; // L2 experimental
inline constexpr site<"site_Game_TheatreList_NoSlot"> site_Game_TheatreList_NoSlot{}; // L2 experimental
inline constexpr site<"site_Game_TheatreList_NoSlotAlt"> site_Game_TheatreList_NoSlotAlt{}; // L2 experimental
inline constexpr site<"site_Game_TheatreRows_RowOffset"> site_Game_TheatreRows_RowOffset{}; // L2 experimental
inline constexpr site<"site_Game_ThrustmasterLcd_EntrantRecord"> site_Game_ThrustmasterLcd_EntrantRecord{}; // L2 experimental
inline constexpr site<"site_Game_ThrustmasterLcd_SectorTail"> site_Game_ThrustmasterLcd_SectorTail{}; // L2 experimental
inline constexpr site<"site_Game_TileListSelect_ChildNext"> site_Game_TileListSelect_ChildNext{}; // L2 experimental
inline constexpr site<"site_Game_TileListSelect_ChildStep"> site_Game_TileListSelect_ChildStep{}; // L2 experimental
inline constexpr site<"site_Game_TimeTrialLeaderboard_BoardRead"> site_Game_TimeTrialLeaderboard_BoardRead{}; // L2 experimental
inline constexpr site<"site_Game_TimeTrialLeaderboard_BoardsDone"> site_Game_TimeTrialLeaderboard_BoardsDone{}; // L2 experimental
inline constexpr site<"site_Game_TimeTrial_SetReferenceListBoard"> site_Game_TimeTrial_SetReferenceListBoard{}; // L2 experimental
inline constexpr site<"site_Game_TimeTrial_SetReferenceLogBoard"> site_Game_TimeTrial_SetReferenceLogBoard{}; // L2 experimental
inline constexpr site<"site_Game_TimeTrial_SetReferenceLogDone"> site_Game_TimeTrial_SetReferenceLogDone{}; // L2 experimental
inline constexpr site<"site_Game_TimeTrial_SetReferenceNoList"> site_Game_TimeTrial_SetReferenceNoList{}; // L2 experimental
inline constexpr vtable<"vt_Game_CareerDLCSelectViewModel", 19> vt_Game_CareerDLCSelectViewModel{}; // L2 experimental
inline constexpr vtable<"vt_Game_CareerWizardSettingsSelectViewModel", 19> vt_Game_CareerWizardSettingsSelectViewModel{}; // L2 experimental, CareerWizardSettingsSelectViewModel.
inline constexpr vtable<"vt_Game_DriverCareerDriverSelectViewModel", 22> vt_Game_DriverCareerDriverSelectViewModel{}; // L2 experimental
inline constexpr vtable<"vt_Game_DriverCareerSlotPredicate", 6> vt_Game_DriverCareerSlotPredicate{}; // L2 experimental
inline constexpr vtable<"vt_Game_FECareerViewModel", 19> vt_Game_FECareerViewModel{}; // L2 experimental, game::FECareerViewModel,
inline constexpr vtable<"vt_Game_SaveDataRootContainer", 7> vt_Game_SaveDataRootContainer{}; // L3 stable
inline constexpr vtable<"vt_Game_TrackInformationModule", 17> vt_Game_TrackInformationModule{}; // L2 experimental, TrackInformationModule:
inline constexpr vtable<"vt_Game_TrackListComponent", 79> vt_Game_TrackListComponent{}; // L2 experimental, TrackListComponent

}
