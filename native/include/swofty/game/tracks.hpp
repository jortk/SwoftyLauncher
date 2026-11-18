#pragma once
#include <swofty/game/base.hpp>

namespace swofty::game {

using Trk_CareerWizardTrackSelectVm_OnCommandRaised_t = int *(void *vm, uint32_t **cmd);
inline constexpr fn<"Trk_CareerWizardTrackSelectVm_OnCommandRaised", Trk_CareerWizardTrackSelectVm_OnCommandRaised_t> Trk_CareerWizardTrackSelectVm_OnCommandRaised{}; // L2 experimental
using Trk_GrandPrixTrackSelectVm_OnCommandRaised_t = void **(void *vm, uint32_t **cmd);
inline constexpr fn<"Trk_GrandPrixTrackSelectVm_OnCommandRaised", Trk_GrandPrixTrackSelectVm_OnCommandRaised_t> Trk_GrandPrixTrackSelectVm_OnCommandRaised{}; // L2 experimental, HomeSoloTrackSelectViewModel::onCommandRaised
using Trk_SettingsSelectTrackTabVm_OnCommandRaised_t = char *(void *vm, uint32_t **cmd);
inline constexpr fn<"Trk_SettingsSelectTrackTabVm_OnCommandRaised", Trk_SettingsSelectTrackTabVm_OnCommandRaised_t> Trk_SettingsSelectTrackTabVm_OnCommandRaised{}; // L2 experimental
using Trk_TimeTrialTrackSelectVm_OnCommandRaised_t = uint64_t(void *vm, uint32_t **cmd);
inline constexpr fn<"Trk_TimeTrialTrackSelectVm_OnCommandRaised", Trk_TimeTrialTrackSelectVm_OnCommandRaised_t> Trk_TimeTrialTrackSelectVm_OnCommandRaised{}; // L2 experimental, TrackSelectViewModel::onCommandRaised
inline constexpr site<"site_Trk_TrackData_SetMatched"> site_Trk_TrackData_SetMatched{}; // L2 experimental

}
