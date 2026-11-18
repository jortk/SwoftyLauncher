#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

using Ui_AssetHandleCache_FindOrCreate_t = void(void *cache, void *key, char *out_created);
inline constexpr fn<"Ui_AssetHandleCache_FindOrCreate", Ui_AssetHandleCache_FindOrCreate_t> Ui_AssetHandleCache_FindOrCreate{}; // L2 experimental
inline constexpr address<"Ui_Bind_ContractHubDriverRow"> Ui_Bind_ContractHubDriverRow{}; // L2 experimental
inline constexpr address<"Ui_Bind_MyTeamActivityMenuItems"> Ui_Bind_MyTeamActivityMenuItems{}; // L2 experimental
inline constexpr address<"Ui_Bind_PopupLevelRow"> Ui_Bind_PopupLevelRow{}; // L2 experimental
inline constexpr address<"Ui_Bind_TopLevelFacilityHeader"> Ui_Bind_TopLevelFacilityHeader{}; // L2 experimental
using Ui_Build_DriverCustomisationWillThumb_t = void *(void *a, void *ctx, uint32_t id, uint8_t f1, uint8_t f2, void *out);
inline constexpr fn<"Ui_Build_DriverCustomisationWillThumb", Ui_Build_DriverCustomisationWillThumb_t> Ui_Build_DriverCustomisationWillThumb{}; // L2 experimental
inline constexpr address<"Ui_Build_RewardAcclaim"> Ui_Build_RewardAcclaim{}; // L2 experimental
inline constexpr address<"Ui_Build_ShowAcclaim"> Ui_Build_ShowAcclaim{}; // L2 experimental
inline constexpr address<"Ui_Build_TeamHqMyTeamTabViewModels"> Ui_Build_TeamHqMyTeamTabViewModels{}; // L2 experimental
inline constexpr address<"Ui_CareerContinueMyTeam_Flow"> Ui_CareerContinueMyTeam_Flow{}; // L2 experimental
inline constexpr address<"Ui_CareerSelectMyTeam_KonnersportDesc"> Ui_CareerSelectMyTeam_KonnersportDesc{}; // L2 experimental
inline constexpr address<"Ui_CareerSelectMyTeam_ManageCareer"> Ui_CareerSelectMyTeam_ManageCareer{}; // L2 experimental
using Ui_CareerSelectMyTeam_SetDriverIconActive_t = void(void *self, uint32_t value);
inline constexpr fn<"Ui_CareerSelectMyTeam_SetDriverIconActive", Ui_CareerSelectMyTeam_SetDriverIconActive_t> Ui_CareerSelectMyTeam_SetDriverIconActive{}; // L2 experimental
using Ui_CareerSetup_BuildScreenBindings_t = void(void *screen);
inline constexpr fn<"Ui_CareerSetup_BuildScreenBindings", Ui_CareerSetup_BuildScreenBindings_t> Ui_CareerSetup_BuildScreenBindings{}; // L2 experimental
inline constexpr address<"Ui_CareerWizardMyTeam_Init"> Ui_CareerWizardMyTeam_Init{}; // L2 experimental
using Ui_CareerWizard_BuildTeamPresetNames_t = uint32_t *(uint32_t *out, char flag, void *wizard);
inline constexpr fn<"Ui_CareerWizard_BuildTeamPresetNames", Ui_CareerWizard_BuildTeamPresetNames_t> Ui_CareerWizard_BuildTeamPresetNames{}; // L2 experimental
using Ui_CareerWizard_ReleaseItemWidgets_t = void(void *wizard, uint8_t flag);
inline constexpr fn<"Ui_CareerWizard_ReleaseItemWidgets", Ui_CareerWizard_ReleaseItemWidgets_t> Ui_CareerWizard_ReleaseItemWidgets{}; // L2 experimental
using Ui_CareerWizard_ResetItemWidgets_t = void(void *wizard);
inline constexpr fn<"Ui_CareerWizard_ResetItemWidgets", Ui_CareerWizard_ResetItemWidgets_t> Ui_CareerWizard_ResetItemWidgets{}; // L2 experimental
inline constexpr address<"Ui_Compare_NodeHandles"> Ui_Compare_NodeHandles{}; // L2 experimental
inline constexpr address<"Ui_Construct_FacilityTileWidget"> Ui_Construct_FacilityTileWidget{}; // L2 experimental
inline constexpr address<"Ui_Copy_ListElementOrEmpty"> Ui_Copy_ListElementOrEmpty{}; // L2 experimental
inline constexpr address<"Ui_Copy_MyTeamNodeArray"> Ui_Copy_MyTeamNodeArray{}; // L2 experimental
inline constexpr address<"Ui_CreateMyTeamCareer_Description"> Ui_CreateMyTeamCareer_Description{}; // L2 experimental
using Ui_DataDrivenMessage_Init_t = int(void *elem, void *rec);
inline constexpr fn<"Ui_DataDrivenMessage_Init", Ui_DataDrivenMessage_Init_t> Ui_DataDrivenMessage_Init{}; // L2 experimental
inline constexpr address<"Ui_DeptEvent_SetGotoButton"> Ui_DeptEvent_SetGotoButton{}; // L2 experimental
inline constexpr address<"Ui_Dispatch_MyTeamActivitiesUiEvent"> Ui_Dispatch_MyTeamActivitiesUiEvent{}; // L2 experimental
using Ui_DriverMarket_SetOpenedFromRenewalPopup_t = void(uint8_t value);
inline constexpr fn<"Ui_DriverMarket_SetOpenedFromRenewalPopup", Ui_DriverMarket_SetOpenedFromRenewalPopup_t> Ui_DriverMarket_SetOpenedFromRenewalPopup{}; // L2 experimental
inline constexpr address<"Ui_EnterMyTeamCareer_Main"> Ui_EnterMyTeamCareer_Main{}; // L2 experimental
using Ui_F1World_BindCardInfo_t = void(void *self, void *card_ref, void *bindings);
inline constexpr fn<"Ui_F1World_BindCardInfo", Ui_F1World_BindCardInfo_t> Ui_F1World_BindCardInfo{}; // L2 experimental
inline constexpr address<"Ui_F1World_BuildUpgradeCardScreen"> Ui_F1World_BuildUpgradeCardScreen{}; // L2 experimental
using Ui_F1World_SetLoadoutItemIconOverride_t = void(void *self, void *card);
inline constexpr fn<"Ui_F1World_SetLoadoutItemIconOverride", Ui_F1World_SetLoadoutItemIconOverride_t> Ui_F1World_SetLoadoutItemIconOverride{}; // L2 experimental
using Ui_F1World_SetLoadoutPillBackgrounds_t = void(void *self, void *card);
inline constexpr fn<"Ui_F1World_SetLoadoutPillBackgrounds", Ui_F1World_SetLoadoutPillBackgrounds_t> Ui_F1World_SetLoadoutPillBackgrounds{}; // L2 experimental
inline constexpr address<"Ui_F1World_SetLoadoutUpgradeEmptyOrUnique"> Ui_F1World_SetLoadoutUpgradeEmptyOrUnique{}; // L2 experimental
inline constexpr address<"Ui_F1World_SetLoadoutUpgradeLevelUnique"> Ui_F1World_SetLoadoutUpgradeLevelUnique{}; // L2 experimental
inline constexpr address<"Ui_F1World_SetLoadoutUpgradeTextures"> Ui_F1World_SetLoadoutUpgradeTextures{}; // L2 experimental
inline constexpr address<"Ui_F1World_SetUpgradeCardSlotType"> Ui_F1World_SetUpgradeCardSlotType{}; // L2 experimental
inline constexpr address<"Ui_F1World_SetUpgradeLevelUniqueBinding"> Ui_F1World_SetUpgradeLevelUniqueBinding{}; // L2 experimental
inline constexpr address<"Ui_F1World_ShowMaxUpgradeLevelWarning"> Ui_F1World_ShowMaxUpgradeLevelWarning{}; // L2 experimental
using Ui_F1World_UpdateUpgradeCardEntry_t = uint64_t(void *self, void *a2, void **entry);
inline constexpr fn<"Ui_F1World_UpdateUpgradeCardEntry", Ui_F1World_UpdateUpgradeCardEntry_t> Ui_F1World_UpdateUpgradeCardEntry{}; // L2 experimental
using Ui_Format_NumberText_KMSuffix_t = void *(void *fmt, void *out, uint32_t value, char sign_flag, char plus_flag, uint8_t decimals, void *loc);
inline constexpr fn<"Ui_Format_NumberText_KMSuffix", Ui_Format_NumberText_KMSuffix_t> Ui_Format_NumberText_KMSuffix{}; // L2 experimental
inline constexpr address<"Ui_Format_RenownText"> Ui_Format_RenownText{}; // L2 experimental
inline constexpr address<"Ui_Format_ResourcePointsAmountText"> Ui_Format_ResourcePointsAmountText{}; // L2 experimental
inline constexpr address<"Ui_Get_AcclaimIsIncreasing"> Ui_Get_AcclaimIsIncreasing{}; // L2 experimental
inline constexpr address<"Ui_Get_AcclaimProperty"> Ui_Get_AcclaimProperty{}; // L2 experimental
inline constexpr address<"Ui_Get_AcclaimValue"> Ui_Get_AcclaimValue{}; // L2 experimental
inline constexpr address<"Ui_Get_UiRegistryEntry_A"> Ui_Get_UiRegistryEntry_A{}; // L2 experimental
inline constexpr address<"Ui_Get_UiRegistryEntry_B"> Ui_Get_UiRegistryEntry_B{}; // L2 experimental
using Ui_Init_CarDriverTeamSelectors_t = uint64_t(void *self);
inline constexpr fn<"Ui_Init_CarDriverTeamSelectors", Ui_Init_CarDriverTeamSelectors_t> Ui_Init_CarDriverTeamSelectors{}; // L2 experimental
inline constexpr address<"Ui_Init_TeamHqMyTeamTabData"> Ui_Init_TeamHqMyTeamTabData{}; // L2 experimental
inline constexpr address<"Ui_Init_TeamHqTabButtonData_B"> Ui_Init_TeamHqTabButtonData_B{}; // L2 experimental
inline constexpr address<"Ui_Link_ResearchRegistryNode"> Ui_Link_ResearchRegistryNode{}; // L2 experimental
inline constexpr address<"Ui_MyTeamButtonDivider_Init"> Ui_MyTeamButtonDivider_Init{}; // L2 experimental
inline constexpr address<"Ui_MyTeamCareerTabDevelopment_Init"> Ui_MyTeamCareerTabDevelopment_Init{}; // L2 experimental
inline constexpr address<"Ui_MyTeamCareerTabResearch_Init"> Ui_MyTeamCareerTabResearch_Init{}; // L2 experimental
inline constexpr address<"Ui_MyTeamCareerTabVehicles_Init"> Ui_MyTeamCareerTabVehicles_Init{}; // L2 experimental
inline constexpr address<"Ui_MyTeamCorporateDriverCard_Spawn"> Ui_MyTeamCorporateDriverCard_Spawn{}; // L2 experimental
inline constexpr address<"Ui_MyTeamCustomisation_BuildPages"> Ui_MyTeamCustomisation_BuildPages{}; // L2 experimental
inline constexpr address<"Ui_MyTeamDecalEditor_Page"> Ui_MyTeamDecalEditor_Page{}; // L2 experimental
inline constexpr address<"Ui_MyTeamDriverSelect_Enter"> Ui_MyTeamDriverSelect_Enter{}; // L2 experimental
inline constexpr address<"Ui_MyTeamHq_DriverRatingsComparisonPanel_Init"> Ui_MyTeamHq_DriverRatingsComparisonPanel_Init{}; // L2 experimental
inline constexpr address<"Ui_MyTeamIntro_Enter"> Ui_MyTeamIntro_Enter{}; // L2 experimental
inline constexpr address<"Ui_MyTeamOverviewOwnerBackground_Enter"> Ui_MyTeamOverviewOwnerBackground_Enter{}; // L2 experimental
inline constexpr address<"Ui_MyTeamRivalries_Page"> Ui_MyTeamRivalries_Page{}; // L2 experimental
inline constexpr address<"Ui_MyTeamSetupGeneric_Option"> Ui_MyTeamSetupGeneric_Option{}; // L2 experimental
inline constexpr address<"Ui_MyTeamSetupGeneric_OutputFirstName"> Ui_MyTeamSetupGeneric_OutputFirstName{}; // L2 experimental
inline constexpr address<"Ui_MyTeamSetupSceneMessage_RegisterHandler"> Ui_MyTeamSetupSceneMessage_RegisterHandler{}; // L2 experimental
inline constexpr address<"Ui_MyTeamTeamIcons_Select"> Ui_MyTeamTeamIcons_Select{}; // L2 experimental
inline constexpr address<"Ui_MyTeamWelcomeToTeamHq_Popup"> Ui_MyTeamWelcomeToTeamHq_Popup{}; // L2 experimental
inline constexpr address<"Ui_NoyearCustomMyTeam_FlowState"> Ui_NoyearCustomMyTeam_FlowState{}; // L2 experimental
inline constexpr address<"Ui_NoyearCustomMyTeam_Init"> Ui_NoyearCustomMyTeam_Init{}; // L2 experimental
using Ui_OnlineCustomItem_GetAction_t = void *(void *item, void *out_action);
inline constexpr fn<"Ui_OnlineCustomItem_GetAction", Ui_OnlineCustomItem_GetAction_t> Ui_OnlineCustomItem_GetAction{}; // L2 experimental
inline constexpr address<"Ui_PaddockPass_OnShow_CloseTransactionQueue"> Ui_PaddockPass_OnShow_CloseTransactionQueue{}; // L2 experimental
inline constexpr address<"Ui_Play_FacilityStageCutsceneByIndex"> Ui_Play_FacilityStageCutsceneByIndex{}; // L2 experimental
using Ui_PresetEventItem_Populate_t = void(void *item);
inline constexpr fn<"Ui_PresetEventItem_Populate", Ui_PresetEventItem_Populate_t> Ui_PresetEventItem_Populate{}; // L2 experimental
inline constexpr address<"Ui_PurchaseItem_ResolveTypeName"> Ui_PurchaseItem_ResolveTypeName{}; // L2 experimental
inline constexpr address<"Ui_PurchaseMessage_ApplySelectedTier"> Ui_PurchaseMessage_ApplySelectedTier{}; // L2 experimental
inline constexpr address<"Ui_PurchaseMessage_OnConfirm"> Ui_PurchaseMessage_OnConfirm{}; // L2 experimental
inline constexpr address<"Ui_PurchasePopup_Init"> Ui_PurchasePopup_Init{}; // L2 experimental
inline constexpr address<"Ui_PurchasePopup_Refresh"> Ui_PurchasePopup_Refresh{}; // L2 experimental
using Ui_Purchase_SelectTierButton_t = void(void *popup, int32_t tier_id);
inline constexpr fn<"Ui_Purchase_SelectTierButton", Ui_Purchase_SelectTierButton_t> Ui_Purchase_SelectTierButton{}; // L2 experimental
inline constexpr address<"Ui_Release_FacilityUpgradeSlotsNodes"> Ui_Release_FacilityUpgradeSlotsNodes{}; // L2 experimental
inline constexpr address<"Ui_Release_RnDDriverUpgradeRowNodes"> Ui_Release_RnDDriverUpgradeRowNodes{}; // L2 experimental
inline constexpr address<"Ui_Release_TopLevelFacilityNodes"> Ui_Release_TopLevelFacilityNodes{}; // L2 experimental
inline constexpr address<"Ui_Resolve_MyTeamTabNode"> Ui_Resolve_MyTeamTabNode{}; // L2 experimental
inline constexpr address<"Ui_RndLaptop_BuildResearchHeader"> Ui_RndLaptop_BuildResearchHeader{}; // L2 experimental
inline constexpr address<"Ui_RndLaptop_SetDevSlotState"> Ui_RndLaptop_SetDevSlotState{}; // L2 experimental
inline constexpr address<"Ui_RndLaptop_SetDevelopUpgrades"> Ui_RndLaptop_SetDevelopUpgrades{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_BuildDevComplete"> Ui_RndNotification_BuildDevComplete{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_BuildDevFailed"> Ui_RndNotification_BuildDevFailed{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_BuildResearchComplete"> Ui_RndNotification_BuildResearchComplete{}; // L2 experimental
using Ui_RndNotification_BuildSpawnables_t = void(void *self, void *a2, void *a3);
inline constexpr fn<"Ui_RndNotification_BuildSpawnables", Ui_RndNotification_BuildSpawnables_t> Ui_RndNotification_BuildSpawnables{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_BuildSupplierUpgrade"> Ui_RndNotification_BuildSupplierUpgrade{}; // L2 experimental
using Ui_RndNotification_ClearSpawnedCards_t = void(void *self);
inline constexpr fn<"Ui_RndNotification_ClearSpawnedCards", Ui_RndNotification_ClearSpawnedCards_t> Ui_RndNotification_ClearSpawnedCards{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_SetDeptCardBg"> Ui_RndNotification_SetDeptCardBg{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_SetDescription"> Ui_RndNotification_SetDescription{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_SetPartCard"> Ui_RndNotification_SetPartCard{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_SetPerfIndex"> Ui_RndNotification_SetPerfIndex{}; // L2 experimental
inline constexpr address<"Ui_RndNotification_SetRegulationInfo"> Ui_RndNotification_SetRegulationInfo{}; // L2 experimental
using Ui_RndNotification_SpawnCardByName_t = void(void *out, void *self, const uint8_t *name, void *a4, void *a5);
inline constexpr fn<"Ui_RndNotification_SpawnCardByName", Ui_RndNotification_SpawnCardByName_t> Ui_RndNotification_SpawnCardByName{}; // L2 experimental
inline constexpr address<"Ui_Rnd_BuildGotoDepartmentButtonA"> Ui_Rnd_BuildGotoDepartmentButtonA{}; // L2 experimental
inline constexpr address<"Ui_Rnd_BuildGotoDepartmentButtonB"> Ui_Rnd_BuildGotoDepartmentButtonB{}; // L2 experimental
inline constexpr address<"Ui_Rnd_OnChallengeApplied"> Ui_Rnd_OnChallengeApplied{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetChallengerIconsSmall"> Ui_Rnd_SetChallengerIconsSmall{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetCompletionTimelineIcons"> Ui_Rnd_SetCompletionTimelineIcons{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetDepartmentColourIconName"> Ui_Rnd_SetDepartmentColourIconName{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetDepartmentName"> Ui_Rnd_SetDepartmentName{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetDepartmentPipColourA"> Ui_Rnd_SetDepartmentPipColourA{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetDepartmentPipColourB"> Ui_Rnd_SetDepartmentPipColourB{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetProgressStateIcon"> Ui_Rnd_SetProgressStateIcon{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetSecretSimultaneousUpgradeN"> Ui_Rnd_SetSecretSimultaneousUpgradeN{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetSecretSimultaneousUpgradesShort"> Ui_Rnd_SetSecretSimultaneousUpgradesShort{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetUnlockInstantText"> Ui_Rnd_SetUnlockInstantText{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetUpgradeCardText"> Ui_Rnd_SetUpgradeCardText{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetUpgradeList"> Ui_Rnd_SetUpgradeList{}; // L2 experimental
inline constexpr address<"Ui_Rnd_SetUpgradeRewards"> Ui_Rnd_SetUpgradeRewards{}; // L2 experimental
inline constexpr address<"Ui_Rnd_ShowTabProgressHistory"> Ui_Rnd_ShowTabProgressHistory{}; // L2 experimental
inline constexpr address<"Ui_Rnd_ShowTabUpgradesInProgress"> Ui_Rnd_ShowTabUpgradesInProgress{}; // L2 experimental
inline constexpr address<"Ui_Set_RnDNodeRefB"> Ui_Set_RnDNodeRefB{}; // L2 experimental
using Ui_Setup_DriverFrontendForPerkAcclaim_t = void(void *vm, void **node);
inline constexpr fn<"Ui_Setup_DriverFrontendForPerkAcclaim", Ui_Setup_DriverFrontendForPerkAcclaim_t> Ui_Setup_DriverFrontendForPerkAcclaim{}; // L2 experimental
inline constexpr address<"Ui_ShopItem_Bind"> Ui_ShopItem_Bind{}; // L2 experimental
using Ui_Show_CareerSessionMessage_t = void(void *self, void *msg, void *listener);
inline constexpr fn<"Ui_Show_CareerSessionMessage", Ui_Show_CareerSessionMessage_t> Ui_Show_CareerSessionMessage{}; // L2 experimental
inline constexpr address<"Ui_SponsorLogoBinding_Build"> Ui_SponsorLogoBinding_Build{}; // L2 experimental
inline constexpr address<"Ui_TeamHqOwnerPerks_Header"> Ui_TeamHqOwnerPerks_Header{}; // L2 experimental
inline constexpr address<"Ui_TeamHqOwnerPerks_Slice"> Ui_TeamHqOwnerPerks_Slice{}; // L2 experimental
inline constexpr address<"Ui_TeamHqPopup_BuildRndResult"> Ui_TeamHqPopup_BuildRndResult{}; // L2 experimental
using Ui_TeamHq_BuildPerkIconBindings_t = void(void *self, void *perk);
inline constexpr fn<"Ui_TeamHq_BuildPerkIconBindings", Ui_TeamHq_BuildPerkIconBindings_t> Ui_TeamHq_BuildPerkIconBindings{}; // L2 experimental
inline constexpr address<"Ui_Update_ContractHubDepartmentRow"> Ui_Update_ContractHubDepartmentRow{}; // L2 experimental
inline constexpr address<"Ui_Update_ContractHubSecretUpgrades"> Ui_Update_ContractHubSecretUpgrades{}; // L2 experimental
inline constexpr address<"Ui_Update_FacilityPhaseLabelNodes"> Ui_Update_FacilityPhaseLabelNodes{}; // L2 experimental
inline constexpr address<"Ui_Update_MyTeamActivitiesMainMenu"> Ui_Update_MyTeamActivitiesMainMenu{}; // L2 experimental
inline constexpr address<"Ui_Update_RnDDepartmentNames"> Ui_Update_RnDDepartmentNames{}; // L2 experimental
inline constexpr address<"Ui_Update_RnDDeptPropertyNodes"> Ui_Update_RnDDeptPropertyNodes{}; // L2 experimental
using Ui_Video_FindResourceOrWarn_t = void *(void *self, void *video_ref);
inline constexpr fn<"Ui_Video_FindResourceOrWarn", Ui_Video_FindResourceOrWarn_t> Ui_Video_FindResourceOrWarn{}; // L2 experimental

}
