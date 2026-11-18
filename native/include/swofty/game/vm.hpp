#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

struct VmPropHolder {
    uint8_t _pad_0x0[0x8];
    void *node; // L3 stable
};
static_assert(offsetof(VmPropHolder, node) == 0x8);

using Vm_Advance_BuildCardBindings_t = void(void *vm, void *card);
inline constexpr fn<"Vm_Advance_BuildCardBindings", Vm_Advance_BuildCardBindings_t> Vm_Advance_BuildCardBindings{}; // L2 experimental
using Vm_Bind_TeamHqHubPage_t = void(void *vm);
inline constexpr fn<"Vm_Bind_TeamHqHubPage", Vm_Bind_TeamHqHubPage_t> Vm_Bind_TeamHqHubPage{}; // L2 experimental
using Vm_Build_PerkResourcePointsBoostRow_t = int64_t(void *vm, void *a, int64_t perk, void *out, void *c);
inline constexpr fn<"Vm_Build_PerkResourcePointsBoostRow", Vm_Build_PerkResourcePointsBoostRow_t> Vm_Build_PerkResourcePointsBoostRow{}; // L2 experimental
inline constexpr address<"Vm_CareerEmail_BindTemplates"> Vm_CareerEmail_BindTemplates{}; // L2 experimental
inline constexpr address<"Vm_CareerPlayerCard_Init"> Vm_CareerPlayerCard_Init{}; // L2 experimental
inline constexpr address<"Vm_CareerPlayerCard_Shutdown"> Vm_CareerPlayerCard_Shutdown{}; // L2 experimental
inline constexpr address<"Vm_CareerWizard_BuildMyTeamEntryPoint"> Vm_CareerWizard_BuildMyTeamEntryPoint{}; // L2 experimental
using Vm_ContractRow_Destroy_t = void(void *row);
inline constexpr fn<"Vm_ContractRow_Destroy", Vm_ContractRow_Destroy_t> Vm_ContractRow_Destroy{}; // L2 experimental
using Vm_ContractsVM_DestroyBindings_t = void(void *vm);
inline constexpr fn<"Vm_ContractsVM_DestroyBindings", Vm_ContractsVM_DestroyBindings_t> Vm_ContractsVM_DestroyBindings{}; // L2 experimental
inline constexpr address<"Vm_CostCapCard_Bind"> Vm_CostCapCard_Bind{}; // L2 experimental
inline constexpr address<"Vm_CostCapCard_BuildTexts"> Vm_CostCapCard_BuildTexts{}; // L2 experimental
using Vm_Despawn_UpgradeListNodes_t = void(void *list);
inline constexpr fn<"Vm_Despawn_UpgradeListNodes", Vm_Despawn_UpgradeListNodes_t> Vm_Despawn_UpgradeListNodes{}; // L2 experimental
inline constexpr address<"Vm_FacilityUpgradeCard_Bind"> Vm_FacilityUpgradeCard_Bind{}; // L2 experimental
inline constexpr address<"Vm_FinanceHistoryGraph_Bind"> Vm_FinanceHistoryGraph_Bind{}; // L2 experimental
inline constexpr address<"Vm_FinanceHistoryGraph_UpdateCostCapLabelPosition"> Vm_FinanceHistoryGraph_UpdateCostCapLabelPosition{}; // L2 experimental
inline constexpr address<"Vm_FinancePeriod_Bind"> Vm_FinancePeriod_Bind{}; // L2 experimental
inline constexpr address<"Vm_FinanceTransactionEntry_Bind"> Vm_FinanceTransactionEntry_Bind{}; // L2 experimental
inline constexpr address<"Vm_FinanceTransactionRow_SetIcon"> Vm_FinanceTransactionRow_SetIcon{}; // L2 experimental
inline constexpr address<"Vm_Format_LoyaltyRewardIncomeText"> Vm_Format_LoyaltyRewardIncomeText{}; // L2 experimental
using Vm_GetOrCreate_ListChild_t = void **(void *registry, void *parent, const uint32_t *hash);
inline constexpr fn<"Vm_GetOrCreate_ListChild", Vm_GetOrCreate_ListChild_t> Vm_GetOrCreate_ListChild{}; // L2 experimental
inline constexpr address<"Vm_Get_VirtualHelpPromptVisible"> Vm_Get_VirtualHelpPromptVisible{}; // L2 experimental
using Vm_Init_UpgradeLoadoutSlot_t = void *(void *self);
inline constexpr fn<"Vm_Init_UpgradeLoadoutSlot", Vm_Init_UpgradeLoadoutSlot_t> Vm_Init_UpgradeLoadoutSlot{}; // L2 experimental
using Vm_List_AppendChild_t = void **(void *list);
inline constexpr fn<"Vm_List_AppendChild", Vm_List_AppendChild_t> Vm_List_AppendChild{}; // L2 experimental
inline constexpr address<"Vm_MyTeamCorporateDriverCard_ReleaseNodes"> Vm_MyTeamCorporateDriverCard_ReleaseNodes{}; // L2 experimental
inline constexpr address<"Vm_MyTeamDriverCounterOffer_BindNodes"> Vm_MyTeamDriverCounterOffer_BindNodes{}; // L2 experimental
inline constexpr address<"Vm_MyTeamSetupCard_ReleaseNodes_37fc80"> Vm_MyTeamSetupCard_ReleaseNodes_37fc80{}; // L2 experimental
inline constexpr address<"Vm_MyTeamSponsors_Bind"> Vm_MyTeamSponsors_Bind{}; // L2 experimental
inline constexpr address<"Vm_MyTeamUpgradeScreenCards_ReleaseNodes"> Vm_MyTeamUpgradeScreenCards_ReleaseNodes{}; // L2 experimental
inline constexpr address<"Vm_MyTeamUpgradeScreen_Bind"> Vm_MyTeamUpgradeScreen_Bind{}; // L2 experimental
inline constexpr address<"Vm_MyTeamWeekendSummary_Bind"> Vm_MyTeamWeekendSummary_Bind{}; // L2 experimental
inline constexpr address<"Vm_MyTeamWorkforce_Bind"> Vm_MyTeamWorkforce_Bind{}; // L2 experimental
using Vm_NotificationButton_BuildNameCommand_t = void *(void *out, void *src, void *a3, void *a4);
inline constexpr fn<"Vm_NotificationButton_BuildNameCommand", Vm_NotificationButton_BuildNameCommand_t> Vm_NotificationButton_BuildNameCommand{}; // L2 experimental
inline constexpr address<"Vm_Register_CustomisationAndMenuViewModelTypes"> Vm_Register_CustomisationAndMenuViewModelTypes{}; // L2 experimental
using Vm_Release_FinanceTabNodes_t = void(void *self);
inline constexpr fn<"Vm_Release_FinanceTabNodes", Vm_Release_FinanceTabNodes_t> Vm_Release_FinanceTabNodes{}; // L2 experimental
using Vm_Release_Node_t = void(void *registry, void *node);
inline constexpr fn<"Vm_Release_Node", Vm_Release_Node_t> Vm_Release_Node{}; // L2 experimental
using Vm_Release_PerkRowNodes_t = void(void *self);
inline constexpr fn<"Vm_Release_PerkRowNodes", Vm_Release_PerkRowNodes_t> Vm_Release_PerkRowNodes{}; // L2 experimental
using Vm_Release_RnDSlotNodes_t = void(void *self);
inline constexpr fn<"Vm_Release_RnDSlotNodes", Vm_Release_RnDSlotNodes_t> Vm_Release_RnDSlotNodes{}; // L2 experimental
inline constexpr address<"Vm_RenownFormPills_Build"> Vm_RenownFormPills_Build{}; // L2 experimental
inline constexpr address<"Vm_RenownIncreasedPopup_SetLevelNodes"> Vm_RenownIncreasedPopup_SetLevelNodes{}; // L2 experimental
inline constexpr address<"Vm_Resolve_PrefixedNodeByIndex"> Vm_Resolve_PrefixedNodeByIndex{}; // L2 experimental
using Vm_Resolve_RegistryNode_t = void *(void *registry, const char *path, const void *base, void *create);
inline constexpr fn<"Vm_Resolve_RegistryNode", Vm_Resolve_RegistryNode_t> Vm_Resolve_RegistryNode{}; // L2 experimental
using Vm_Set_Bool_t = void *(void *registry, void *parent, const char *name, uint8_t *value, int overwrite);
inline constexpr fn<"Vm_Set_Bool", Vm_Set_Bool_t> Vm_Set_Bool{}; // L2 experimental
using Vm_Set_NodeRef_t = void **(void *registry, void *parent, const char *name, void *value, int overwrite);
inline constexpr fn<"Vm_Set_NodeRef", Vm_Set_NodeRef_t> Vm_Set_NodeRef{}; // L2 experimental
using Vm_Set_RnDDepartmentSummaryRowValue_t = void(void *vm, int row_id);
inline constexpr fn<"Vm_Set_RnDDepartmentSummaryRowValue", Vm_Set_RnDDepartmentSummaryRowValue_t> Vm_Set_RnDDepartmentSummaryRowValue{}; // L2 experimental
using Vm_Set_ShortProperty_WithUnknownName_t = uint64_t(void *binding, int16_t *value, uint32_t listener_arg, char force);
inline constexpr fn<"Vm_Set_ShortProperty_WithUnknownName", Vm_Set_ShortProperty_WithUnknownName_t> Vm_Set_ShortProperty_WithUnknownName{}; // L2 experimental
using Vm_Set_String_t = void *(void *registry, void *parent, const char *name, NeStr **value, int overwrite);
inline constexpr fn<"Vm_Set_String", Vm_Set_String_t> Vm_Set_String{}; // L2 experimental
using Vm_Set_TabEnabled_t = void(uintptr_t tabs, uintptr_t node, uint32_t enabled);
inline constexpr fn<"Vm_Set_TabEnabled", Vm_Set_TabEnabled_t> Vm_Set_TabEnabled{}; // L2 experimental
inline constexpr address<"Vm_SponsorCard_SetAmountDisplayNegative"> Vm_SponsorCard_SetAmountDisplayNegative{}; // L2 experimental
inline constexpr address<"Vm_SponsorCard_SetAmountDisplayPositive"> Vm_SponsorCard_SetAmountDisplayPositive{}; // L2 experimental
inline constexpr address<"Vm_SponsorContractOffer_Bind"> Vm_SponsorContractOffer_Bind{}; // L2 experimental
inline constexpr address<"Vm_SponsorGoalCard_Bind"> Vm_SponsorGoalCard_Bind{}; // L2 experimental
inline constexpr address<"Vm_SponsorGoalsText_Refresh"> Vm_SponsorGoalsText_Refresh{}; // L2 experimental
inline constexpr address<"Vm_SponsorInvoice_Bind"> Vm_SponsorInvoice_Bind{}; // L2 experimental
inline constexpr address<"Vm_SponsorOfferCard_Bind"> Vm_SponsorOfferCard_Bind{}; // L2 experimental
using Vm_SponsorOffer_BuildGoalBindings_t = void(void *vm, void *a2, void *contract, void *a4, void *a5, uint32_t a6, int16_t a7, void *out);
inline constexpr fn<"Vm_SponsorOffer_BuildGoalBindings", Vm_SponsorOffer_BuildGoalBindings_t> Vm_SponsorOffer_BuildGoalBindings{}; // L2 experimental
inline constexpr address<"Vm_SponsorSlots_Bind"> Vm_SponsorSlots_Bind{}; // L2 experimental
using Vm_Sponsor_BindTextureSet_t = uint64_t(void *vm, void *sponsor_card, uint64_t a3, void *a4);
inline constexpr fn<"Vm_Sponsor_BindTextureSet", Vm_Sponsor_BindTextureSet_t> Vm_Sponsor_BindTextureSet{}; // L2 experimental
using Vm_Sponsor_FormatWeeksRemainingLabel_t = uint64_t(void *vm, int32_t weeks);
inline constexpr fn<"Vm_Sponsor_FormatWeeksRemainingLabel", Vm_Sponsor_FormatWeeksRemainingLabel_t> Vm_Sponsor_FormatWeeksRemainingLabel{}; // L2 experimental
inline constexpr address<"Vm_TeamHqAdvanceCard_Bind"> Vm_TeamHqAdvanceCard_Bind{}; // L2 experimental
inline constexpr address<"Vm_TeamHqAdvanceCard_BindNodeRefs"> Vm_TeamHqAdvanceCard_BindNodeRefs{}; // L2 experimental
inline constexpr address<"Vm_TeamHqAdvanceItem_ReleaseNodes_383760"> Vm_TeamHqAdvanceItem_ReleaseNodes_383760{}; // L2 experimental
inline constexpr address<"Vm_TeamHqAdvanceItem_ReleaseNodes_3aaca0"> Vm_TeamHqAdvanceItem_ReleaseNodes_3aaca0{}; // L2 experimental
inline constexpr address<"Vm_TeamHqAdvanceItem_ReleaseNodes_99a580"> Vm_TeamHqAdvanceItem_ReleaseNodes_99a580{}; // L2 experimental
inline constexpr address<"Vm_TeamHqAdvanceItem_ReleaseNodes_99ca90"> Vm_TeamHqAdvanceItem_ReleaseNodes_99ca90{}; // L2 experimental
using Vm_TeamHqAdvance_BuildTimelineBindings_t = void(void *vm);
inline constexpr fn<"Vm_TeamHqAdvance_BuildTimelineBindings", Vm_TeamHqAdvance_BuildTimelineBindings_t> Vm_TeamHqAdvance_BuildTimelineBindings{}; // L2 experimental
inline constexpr address<"Vm_TeamHqCorporateHub_Init"> Vm_TeamHqCorporateHub_Init{}; // L2 experimental
inline constexpr address<"Vm_TeamHqDriverRenewalCard_Bind"> Vm_TeamHqDriverRenewalCard_Bind{}; // L2 experimental
inline constexpr address<"Vm_TeamHqFinanceHistory_Init"> Vm_TeamHqFinanceHistory_Init{}; // L2 experimental
inline constexpr address<"Vm_TeamHqFinanceHistory_InitGraph"> Vm_TeamHqFinanceHistory_InitGraph{}; // L2 experimental
inline constexpr address<"Vm_TeamHqFinances_Bind"> Vm_TeamHqFinances_Bind{}; // L2 experimental
inline constexpr address<"Vm_TeamHqHub_Bind"> Vm_TeamHqHub_Bind{}; // L2 experimental
inline constexpr address<"Vm_TeamHqMyTeam_BuildTabs"> Vm_TeamHqMyTeam_BuildTabs{}; // L2 experimental
using Vm_TeamHqMyTeam_FindBoundStatId_t = uint64_t(void *vm);
inline constexpr fn<"Vm_TeamHqMyTeam_FindBoundStatId", Vm_TeamHqMyTeam_FindBoundStatId_t> Vm_TeamHqMyTeam_FindBoundStatId{}; // L2 experimental
inline constexpr address<"Vm_TeamHqMyTeam_HandleNavigation"> Vm_TeamHqMyTeam_HandleNavigation{}; // L2 experimental
using Vm_TeamHqMyTeam_OnNewDay_ShowPopup_t = void(void *vm, uint32_t flags);
inline constexpr fn<"Vm_TeamHqMyTeam_OnNewDay_ShowPopup", Vm_TeamHqMyTeam_OnNewDay_ShowPopup_t> Vm_TeamHqMyTeam_OnNewDay_ShowPopup{}; // L2 experimental
using Vm_TeamHqMyTeam_RefreshStatIndexRow_t = void(void *vm);
inline constexpr fn<"Vm_TeamHqMyTeam_RefreshStatIndexRow", Vm_TeamHqMyTeam_RefreshStatIndexRow_t> Vm_TeamHqMyTeam_RefreshStatIndexRow{}; // L2 experimental
using Vm_TeamHqMyTeam_UpdateAdvanceEntries_t = void(void *vm);
inline constexpr fn<"Vm_TeamHqMyTeam_UpdateAdvanceEntries", Vm_TeamHqMyTeam_UpdateAdvanceEntries_t> Vm_TeamHqMyTeam_UpdateAdvanceEntries{}; // L2 experimental
using Vm_TeamHqMyTeam_UpdateDayNumber_t = void(void *vm);
inline constexpr fn<"Vm_TeamHqMyTeam_UpdateDayNumber", Vm_TeamHqMyTeam_UpdateDayNumber_t> Vm_TeamHqMyTeam_UpdateDayNumber{}; // L2 experimental
using Vm_TeamHqMyTeam_UpdateStopTime_t = void(void *vm);
inline constexpr fn<"Vm_TeamHqMyTeam_UpdateStopTime", Vm_TeamHqMyTeam_UpdateStopTime_t> Vm_TeamHqMyTeam_UpdateStopTime{}; // L2 experimental
using Vm_TeamHqTimeline_AnyDayBoolB_t = bool(void *vm, uint32_t key);
inline constexpr fn<"Vm_TeamHqTimeline_AnyDayBoolB", Vm_TeamHqTimeline_AnyDayBoolB_t> Vm_TeamHqTimeline_AnyDayBoolB{}; // L2 experimental
using Vm_TeamHqTimeline_GetCurrentDayIndex_t = uint64_t(void *vm);
inline constexpr fn<"Vm_TeamHqTimeline_GetCurrentDayIndex", Vm_TeamHqTimeline_GetCurrentDayIndex_t> Vm_TeamHqTimeline_GetCurrentDayIndex{}; // L2 experimental
inline constexpr address<"Vm_TeamHqUpgradesHub_Init"> Vm_TeamHqUpgradesHub_Init{}; // L2 experimental
inline constexpr address<"Vm_TeamHqWorkforceCapNotification_Build"> Vm_TeamHqWorkforceCapNotification_Build{}; // L2 experimental
inline constexpr address<"Vm_TeamHq_BuildTabs"> Vm_TeamHq_BuildTabs{}; // L2 experimental
inline constexpr address<"Vm_TitleSponsorsItem_SetIcon"> Vm_TitleSponsorsItem_SetIcon{}; // L2 experimental
using Vm_TitleSponsorsLoyalty_RefreshFlags_t = void(void *vm);
inline constexpr fn<"Vm_TitleSponsorsLoyalty_RefreshFlags", Vm_TitleSponsorsLoyalty_RefreshFlags_t> Vm_TitleSponsorsLoyalty_RefreshFlags{}; // L2 experimental
using Vm_WizardTitleSlice_DestroyRange_t = void(void *begin, void *end);
inline constexpr fn<"Vm_WizardTitleSlice_DestroyRange", Vm_WizardTitleSlice_DestroyRange_t> Vm_WizardTitleSlice_DestroyRange{}; // L2 experimental
inline constexpr data<"g_Vm_Registry", void *> g_Vm_Registry{}; // L3 stable

}
