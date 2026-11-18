# Hidden game modes and features (F1 25, build d873a1c8)

Survey of game modes, editions and features that ship in `F1_25.exe` and `gamedatabases.2025.bin` but are not reachable from the normal menus. Agent `vmknolle-hidden`, 2026-11-13.

**Evidence level: offline only.** Everything below comes from the game DB (read with `tools/gamedb.py`, keys = fnv1a64 of the row path) and from static string cross-references in the exe (`re/tools/pe_index.py`). Nothing here was tested in game. "Hidden" means: defined in the data or code, but not listed by the edition or switched off. Whether a mode or feature still works when switched on is open in every case.

## 1. How the game decides what is available

Three layers, outermost first:

1. **Engine settings** (`system/config.xml`, `system/config_sku.xml`, loaded by `Ne_Load_SystemConfigXml` 0x1443c24b0). About 960 dotted names, read through `Ne_Get_SettingBool` 0x143a43b90, `Ne_Get_SettingU32` 0x143a43240, `Ne_Config_GetString` 0x143a44700 and related getters. One of them, `ContentSet.currentContentSet` (read by `Sess_Get_CurrentContentSetName` 0x1413e1cd0, default empty; falls back to a game-DB value), names the edition (content set) to use.
2. **Content set** (`content_sets.content_set_N`). Lists the game modes the edition offers (`available_game_modes_0.game_modes`, `.game_mode_championships`, `.game_mode_time_trials`) and holds a feature-bit row (`feature_list_0`) that `Game_Features_LoadContentSetMask` 0x1413ed470 turns into a 128-bit mask. A set inherits from its `base_content_set`.
3. **Game mode rows** (`game_modes.ModeChampionship_N`, `game_modes.GameMode_N`). Each row has `available` (False in the DB; set at startup), `disabled`, `statId`, `game_setup_modes` and the per-mode options.

## 2. Game modes in the DB

29 `ModeChampionship` rows and 1 `GameMode` row (`benchmark`). *In sets* lists the content sets whose `game_mode_championships` names the mode. Bold rows are not offered by the retail-looking sets (2025, 2025_final_sub, 2026) or only by special editions.

| # | mode | statId | in sets | note |
|---|---|---|---|---|
| 0 | online_custom | 7 | all with modes except f1p, f1a | |
| 1 | event_mode | 0 | 2024, 2025, 2025_final_sub, 2026 | |
| 2 | career_invitationals | 11 | 2024, 2025, 2025_final_sub, 2026 | feature bit `invitationals` is False in every set |
| 3 | champ_invitationals | 12 | 2024, 2025, 2025_final_sub, 2026 | same |
| **4** | **dev** | 72 | every set with modes except f1p | developer championship; setup modes f1, f1_26, classic, f2, f1_generic; no front-end entry found |
| 5 | championships | 13 | 2024, 2025, 2025_final_sub, 2026 | type championship_event |
| 6 | online_weekly_events | 15 | 2024, 2025, 2025_final_sub, 2026 | |
| **7** | **online_championship** | 14 | **none** | defined, never offered |
| 8 | grand_prix_splitscreen | 6 | 2024, 2025, 2025_final_sub, 2026 | |
| 9 | story_mode | 17 | 2025, 2025_final_sub, 2026 | Braking Point |
| 10 | grand_prix_23 | 4 | all with modes except 2024_beta and f1a | the Grand Prix mode |
| **11** | **career_23** | 21 | **none** | F1 23 career row kept in the DB |
| **12** | **career_23_online** | 22 | **none** | same |
| 13-16 | driver_career_24, career_24_online, my_team_career_24, challenge_career_24 | 23-26 | 2024 only | previous-year careers |
| 17-20 | my_team_career_25, driver_career_25, career_25_online, challenge_career_25 | 27-30 | 2025, 2025_final_sub, 2026 | |
| **21** | **pro_dev** | 73 | **f1p** only | developer mode of the "f1p" edition |
| **22** | **pro_online_custom** | 74 | **f1p** only | online lobby of the "f1p" edition (setup modes include f1_elimination) |
| **23** | **story_mode_zenith** | 75 | 2025, 2026 (not 2025_final_sub) | Braking Point variant; feature bits `zenith_chapter_select`, `zenith_ti`, `zenith_only_preview` |
| 24 | rewind_event_mode | 31 | 2024, 2025, 2025_final_sub, 2026 | |
| **25** | **F1CG_solo_battles** | 77 | **none** | setup mode `solo_battle`; feature bit `f1cg` is False in every set |
| 26-27 | driver_career_26, my_team_career_26 | 78-79 | 2026 | season-2026 careers |
| **28** | **f1a_mp** | 80 | **f1a** only | F1 Academy multiplayer (setup mode `f1a_mp`) |
| - | benchmark (`GameMode_0`) | 127 | every set with modes except f1a | `-benchmark` command line (`Sess_Benchmark_InitFromCmdline` 0x1407a2250) |

Time-trial modes: `time_trial` in the normal sets, `solo_testing` only in `f1p`.

## 3. Content sets (editions)

feature_list columns: f1cg / iconic_drivers / invitationals / ir / story_mode (T = True, F = False).

| # | set | base | game modes offered | feature_list |
|---|---|---|---|---|
| 0 | 2024 | - | F1 24 list + dev | F T F F F |
| 1 | 2024_beta | 2024 | online_custom, dev | F T F F F |
| 2 | 2025_with_2024_vehicle_package_override | 2025 | (inherits) | (inherits) |
| 3 | 2025 | - | 2025 list incl. story_mode_zenith + dev | F T F F T |
| 4 | 2025_final_sub | 2025 | 2025 list without story_mode_zenith | F T F F T |
| 5 | 2025_launch | 2025 | (inherits) | F T F F T |
| **6** | **2025_movie** | 2025 | (inherits) | (inherits) |
| **7** | **2025_one_track** | 2025 | (inherits) | (inherits) |
| **8** | **f1p** | 2025 | benchmark; grand_prix_23, pro_dev, pro_online_custom; TT solo_testing | F F F F F |
| **9** | **2024_esports** | 2024 | (inherits) | (inherits) |
| 10 | 2024_beta_extra | 2024_beta | (inherits) | (inherits) |
| 11 | 2026 | - | 2025 list + driver_career_26, my_team_career_26 + dev | F T F F T |
| 12 | 2025_beta | 2025 | benchmark, ModeTimeTrial; online_custom, grand_prix_23, dev | F T F F F |
| 13 | 2025_beta_extra | 2025_beta | (inherits) | (inherits) |
| **14** | **vte** | 2025 | benchmark; online_custom, grand_prix_23, dev | F F F F F |
| **15** | **f1a** | 2026 | f1a_mp, dev | F T F F F |

Which set the retail game runs on was not traced. `ContentSet.currentContentSet` defaults to an empty string (Ne_Config_GetString call at 0x1413e1d42); the fallback path after that call was not followed.

## 4. Feature bits

`g_Game_FeatureNames` 0x1456e7300 names 65 bits (index = bit). The DB `feature_list` rows carry only five columns (`f1cg`, `iconic_drivers`, `invitationals`, `ir`, `story_mode`). No other bit has a column in any content set, and no set has `add_features` / `remove_features` rows, so `Game_Features_LoadContentSetMask` leaves the other 60 bits at their default.

| bit | name | bit | name | bit | name |
|---|---|---|---|---|---|
| 0 | story_mode | 22 | special_liveries | 44 | pds |
| 1 | story_team_select | 23 | ce | 45 | rsa |
| 2 | invitationals | 24 | mtc | 46 | f1w_compendium |
| 3 | f1_l | 25 | multiplayer_dotd | 47 | f1cg |
| 4 | f1_w | 26 | driver_icons_mk2 | 48 | DAISlider |
| 5 | sc_unlock_system | 27 | ti | 49 | fhr |
| 6 | new_ranked | 28 | subt_mk2 | 50 | ab |
| 7 | iconic_drivers | 29 | upd_rw_rp | 51 | session_strategy |
| 8 | support_exporting_session_results | 30 | vde | 52 | ydp |
| 9 | rewards_screen | 31 | vde_mirror_link | 53 | main_menu |
| 10 | post_race_licence_update | 32 | vde_mirror_link_same_projection_area | 54 | show_new_features_screen |
| 11 | fanzone | 33 | driver_number | 55 | f1p_pr |
| 12 | podium_pass_tier_purchases | 34 | manual_pit_stops | 56 | ir |
| 13 | multiple_loadouts | 35 | pro_pit_markers | 57 | f1p_autoadvance |
| 14 | rvr | 36 | eatrax_on | 58 | ftue_stay_in_touch |
| 15 | stepped_tutorials | 37 | tt_leaderboards_on | 59 | race_garage_start |
| 16 | silly_season_cutscene | 38 | show_rivals_tt | 60 | garage_spectating |
| 17 | show_customisable_item_icon | 39 | show_objectives_tt | 61 | zenith_only_preview |
| 18 | show_non_customisable_item_icon | 40 | pro_safety_rating | 62 | tt_ai_level_check |
| 19 | CC | 41 | tss | 63 | vte_sessions |
| 20 | driver_hq | 42 | zenith_chapter_select | 64 | challenge_events |
| 21 | allow_skip_first_season_first_practice | 43 | zenith_ti | | |

Bits that look like unreleased or edition-only features: `invitationals` (False everywhere, yet both invitational modes are listed), `f1cg` (goes with `F1CG_solo_battles`), `ir`, `manual_pit_stops`, `pro_pit_markers`, `pro_safety_rating`, `session_strategy`, `race_garage_start`, `garage_spectating`, `challenge_events`, `vte_sessions`, `f1p_pr`, `f1p_autoadvance` and the `zenith_*` bits. Which code tests each bit is not mapped yet (next step: xref the mask readers).

## 5. Developer settings

69 functions that read a gameplay-relevant setting and were missing from the KB are new rows: 67 setting readers plus `Ne_Load_SystemConfigXml` and `Sess_Get_CurrentContentSetName`.

How the rows were made:

- **Decompiled and named:** each was decompiled in Ghidra (`re/decomp/<addr9>.c`, local only) and named by an analysis pass.
- **Verified:** the names were checked blind by separate verifiers (`verify-hm-0..3` in `verify.tsv`).
- **55 at L2:** verifiers agreed on these. 53 carry a byte signature and a binding (`re/promotions/2026-11-13/`).
- **14 at L1:** 7 partial verdicts with `CORRECTION(V…)` notes and 7 whose role was not clear enough.
- **Dropped:** 13 rows of the first L1 pass. Their "reference" was 4 bytes inside another instruction; every string reference is now checked against an instruction boundary (57 of 1278 raw hits were false).

Highlights:

- **Grid:** `Grid.Override`, `Grid.CarsInRow`, `Grid.RowOffset`, `Grid.StaggerOffset`, `Grid.PoleLR` (`Game_Read_GridOverrideConfig` 0x142f61640, already in the KB).
- **Session shortcuts:** `Qualifying.full` / `.short` / `.one_shot`, `Game.SkipPreRace`, `Game.SkipIntroVideo`, `Game.SkipIntroCameras`, `Game.SkipGridRundown`, `Game.SkipWinnerSequence`, `Game.SkipRewardSequence`, `Game.IgnoreMinimumLoadingTime`.
- **Dev session** (`DevSessionDataContract.*`): track, layer, laps, race length, session length, entrant list, equal performance, staggered start, championship component.
- **Forced cutscenes:** `Game.ChampionshipWon.ForceOn`, `Game.DonutSequence.ForceOn` / `.Endless` / `.Vehicle`, `Game.DriverHQIntro.*.ForceOn`.
- **Photo mode / replay:** `PhotoMode.devEnabled`, `.canPositionCars`, `.allowAllWeatherChanges`, `.allowPhotoModeInPitlane`, `.cameraCollisionAvoidance`, `Replay.FullRaceReplay.Enabled`, `Replay.force_advanced_replay_ui`.
- **Debug camera / capture:** `DebugCamera.*` (speed, fov, shake, wobble), `VideoCapture.*`, `VideoExport.Enabled`, `EnvArtMode.enabled`.
- **Front end:** `UI.EnableAttract`, `UI.DemoEvent`, `UI.PreviewEvent`, `UI.MovieBuild`, `UI.skipDLCUpsell`, `UI.skipFirstTimeBoot`, `UI.overrideMainMenuState`, `UI.EnableCredits`, `UI.DisplayWatermark`.
- **Career:** `Game.NegotiationsAfterEveryWeekend`, `Game.driverSelectOverride`, `Game.teamSelectOverride`, `Game.vehicleSelectOverride`, `Game.teamPositionOverride`.
- **Online / LAN:** `Network.LAN.active`, `Network.Season.CollisionsFirstLapOnly`, `Network.Season.WeatherType`, `Network.Dev.*`.
- **Other:** `SafetyCar.noGhost`, `Testing.EnableSmokeTest`, `TimeManager.fixedTimeStep`, `Game.DRL.IgnoreCarSpeed`, `HudCustomisation.imguiEnabled`, `VTE.imgui`.

## 6. Open questions

1. Is `system/config.xml` read from a loose file, and can a Swofty overlay (`api_overlay_file`) replace it? (Untested.)
2. Which content set does the retail build select, and does setting `ContentSet.currentContentSet` to `vte`, `f1p` or `f1a` switch the edition?
3. Do `dev`, `online_championship`, `career_23` and `F1CG_solo_battles` still have working front-end flows, or only DB rows?
4. Which code reads each feature bit?

## Appendix: settings by group

Every name below is referenced from code (string xref checked against the instruction boundary). Function names and levels come from the KB; `?` means no KB row.

### `Game`

| setting | read in |
|---|---|
| `Game.AlwaysVirtualiseSessionsIndividually` | Game_Championship_VirtualisePendingSessions `0x140bd3b30` (L2), Game_Championship_VirtualiseSession `0x140bd4bc0` (L1) |
| `Game.AutoProfile.enabled` | Game_Read_GameConfig `0x1407f7320` (L1) |
| `Game.AutoProfile.framesToCapture` | Game_Read_GameConfig `0x1407f7320` (L1) |
| `Game.AutoProfile.framesToWait` | Game_Read_GameConfig `0x1407f7320` (L1) |
| `Game.Career.Online.EnableSaving` | Game_TOPLevelUI_TriggerProfileSave `0x140ff7cf0` (L1) |
| `Game.CareerSync.DumpComponents` | Game_CareerOnlineDocumentComp_InternalSerialiseAndCalculateHash `0x140fd0990` (L1) |
| `Game.CareerSync.DumpComponentsVerbose` | Game_CareerOnlineDocumentComp_InternalSerialiseAndCalculateHash `0x140fd0990` (L1) |
| `Game.ChampionshipWon.ForceOn` | Sess_ChampionshipWonDecision_Update `0x142d54f20` (L2) |
| `Game.ClientHello` | no code reference found |
| `Game.Context.TeamLivery` | Game_CutSceneScript_RunCommand `0x14023c600` (L2), Game_LiveryModel_Init `0x142a9d730` (L1), Game_PlayerTeamModelComponent_Initialise `0x142a9f390` (L1) |
| `Game.Context.TrackName` | Game_CameraHelicopter_Reset `0x14020a460` (L2), Game_GridCutSceneScript_SetupDriverTalkToPress `0x140345f60` (L2), Game_Podium2019CutSceneScript_ChooseCutScenesToPlay `0x140356a20` (L1) +1 |
| `Game.Context.overrideEmote` | Game_Podium2019CutSceneScript_ChooseCutScenesToPlay `0x140356a20` (L1) |
| `Game.Context.overrideTeamLivery` | Game_CutSceneScript_RunCommand `0x14023c600` (L2), Game_LiveryModel_Init `0x142a9d730` (L1), Game_PlayerTeamModelComponent_Initialise `0x142a9f390` (L1) |
| `Game.Context.overrideTrackName` | Game_CameraHelicopter_Reset `0x14020a460` (L2), Game_GridCutSceneScript_SetupDriverTalkToPress `0x140345f60` (L2), Game_Podium2019CutSceneScript_ChooseCutScenesToPlay `0x140356a20` (L1) +1 |
| `Game.DRL.IgnoreCarSpeed` | Game_DRLSystem_ResolveDependencies `0x14243a840` (L2) |
| `Game.DecalGenerationService.MandatoryMipsDefault` | Game_DecalGeneration_CreateService `0x141428060` (L1) |
| `Game.DecalGenerationService.MandatoryMipsForLocalHumanPlayer` | Game_DecalGeneration_CreateService `0x141428060` (L1) |
| `Game.DonutSequence.Endless` | Sess_DoughnutSequence_Update `0x142d5a150` (L2) |
| `Game.DonutSequence.ForceOn` | Sess_DoughnutSequence_Enter `0x142d59920` (L2) |
| `Game.DonutSequence.Vehicle` | Sess_DoughnutSequence_Enter `0x142d59920` (L2) |
| `Game.DriverHQIntro.DriverHQIntro.ForceOn` | Game_Read_GameConfig_7 `0x1436c5ef0` (L1) |
| `Game.DriverHQIntro.DriverRatingsMilestone.ForceOn` | Game_Read_GameConfig_7 `0x1436c5ef0` (L1) |
| `Game.DriverHQIntro.DriverWonWorldChampionship.ForceOn` | Game_Read_GameConfig_7 `0x1436c5ef0` (L1) |
| `Game.DriverHQIntro.NewSeasonEstablishingCutscene.ForceOn` | Game_Read_GameConfig_7 `0x1436c5ef0` (L1) |
| `Game.EnableLoading` | Game_Read_GameConfig_6 `0x14365a8b0` (L1) |
| `Game.EnableMarshalLightShaders` | no code reference found |
| `Game.EnablePitExitLightShdaers` | no code reference found |
| `Game.EnablePitExitLights` | no code reference found |
| `Game.EnableRaceUI` | Sess_State_OnStateEnter `0x1436f6740` (L2) |
| `Game.EnableSaveGame` | Game_Read_GameConfig `0x1407f7320` (L1), Game_Read_GameConfig_6 `0x14365a8b0` (L1) |
| `Game.EnableSaveSystemInSkipFE` | Game_Read_GameConfig_6 `0x14365a8b0` (L1) |
| `Game.EnableSaving` | Game_Read_GameConfig_6 `0x14365a8b0` (L1) |
| `Game.F1World.ForceFanzoneChoices` | Game_F1World_AssignDefaultFanzoneChoices `0x14170c8c0` (L2), Game_Read_GameConfig_2 `0x1418cfc90` (L1), Game_Read_GameConfig_3 `0x141b19e80` (L1) |
| `Game.F1World.Ranked.AutoUpdateCadence` | Game_F1WRankedComponent_Init `0x142747e00` (L2) |
| `Game.F1World.SkipFanzoneChoices` | Game_Read_GameConfig_2 `0x1418cfc90` (L1), Game_Read_GameConfig_3 `0x141b19e80` (L1) |
| `Game.F1World.SkipOnboarding` | Game_F1World_ApplySkipOnboarding `0x1427447d0` (L2) |
| `Game.ForceTrackReload` | Sess_Loader_UnloadSession `0x142d48040` (L2), Sess_Loader_UpdateFrontendLoad `0x142d4b4c0` (L1), Game_StateSession_OnStateExit `0x1436f8340` (L2) |
| `Game.GameSessionActivity` | no code reference found |
| `Game.IgnoreMinimumLoadingTime` | Sess_Loader_RequestTrackLoad `0x142da7870` (L2), Sess_SessionTransition_OnStateUpdate `0x1436f9890` (L2), Sess_StoryHub_OnStateEnter `0x1436fb5e0` (L2) |
| `Game.LoadForever` | no code reference found |
| `Game.NegotiationsAfterEveryWeekend` | Game_Unk_14126a1a0 `0x14126a1a0` (L1) |
| `Game.ReturnToGarageTimeLimit` | Game_Check_ReturnToGarageTimeLimit `0x1424ac650` (L2) |
| `Game.SkipCamSequence` | no code reference found |
| `Game.SkipGridRundown` | Game_GridRundown_ShouldPlay `0x140ac6b30` (L1) |
| `Game.SkipIntroCameras` | no code reference found |
| `Game.SkipIntroVideo` | Sess_IntroVideo_ParseParams `0x142d6cc30` (L2) |
| `Game.SkipPreRace` | Sess_PlayCutScene_ParseParams `0x142d78c80` (L2), Sess_PreRaceGridState_OnEnter `0x142d81270` (L2) |
| `Game.SkipRewardSequence` | Game_CVar_Ctor_SkipRewardSequence `0x142c747f0` (L1) |
| `Game.SkipTalkingPoints` | no code reference found |
| `Game.SkipWinnerSequence` | no code reference found |
| `Game.SummonToGameSessionActivity` | no code reference found |
| `Game.Thumbnails.driverPreloadTimeoutInSeconds` | Game_Read_GameConfig_4 `0x142e22c60` (L1) |
| `Game.Thumbnails.enableThumbnailGeneration` | Game_Read_GameConfig_4 `0x142e22c60` (L1) |
| `Game.Thumbnails.preloadTimeoutInSeconds` | Game_Read_GameConfig_4 `0x142e22c60` (L1) |
| `Game.Tutorials.SkipVideoTutorials` | no code reference found |
| `Game.Tyres.WarmUpSetsPerCompound` | Game_TyreAllocation_ValidateWarmUpSets `0x140bce940` (L1) |
| `Game.UseSMSSteer` | Car_Create_F1AssistsSystem `0x1447546c0` (L2) |
| `Game.VehicleDamage.tyres` | Game_Read_VehicleConfig `0x14463a800` (L1) |
| `Game.VehicleSpawning.EnableMemorySnapshots` | Game_Read_GameConfig_5 `0x142f3a5d0` (L1) |
| `Game.VehicleSpawning.MaxConcurrentSpawns` | Game_Read_GameConfig_5 `0x142f3a5d0` (L1) |
| `Game.VehicleSpawning.VehicleTimeout` | Game_Read_GameConfig_5 `0x142f3a5d0` (L1) |
| `Game.WeekendSessionManagerInFirstSession` | Sess_WeekendManager_PrepareTrackLoad `0x142409db0` (L2) |
| `Game.driverSelectOverride` | Sess_Build_SelectableDriverList `0x14245a0d0` (L2) |
| `Game.ecaEnabled` | Game_Eca_OnMessageGameModeChanged `0x14244fe70` (L2) |
| `Game.equalPerformance` | Game_Apply_PresetEqualPerformance `0x1407e1d90` (L2) |
| `Game.forceIgnoreCareerDriverInTT` | Game_Unk_142933c70 `0x142933c70` (L1) |
| `Game.setup_mode` | Game_Apply_PresetSetupMode `0x1407dfb60` (L2) |
| `Game.suppressWarnings` | Game_Read_GameConfig `0x1407f7320` (L1) |
| `Game.teamPositionOverride` | Game_Unk_142933c70 `0x142933c70` (L1) |
| `Game.teamSelectOverride` | Sess_Entrant_BuildCandidate `0x1424a15d0` (L2) |
| `Game.team_name` | Game_Apply_PresetTeamName `0x1407dfda0` (L2) |
| `Game.time_of_day` | Game_Apply_PresetTimeOfDay `0x1407e1040` (L2) |
| `Game.vehicleSelectOverride` | Sess_Collect_FilteredEntrants `0x1424a1c20` (L2) |
| `Game.weather` | Game_Apply_PresetWeather `0x1407e1470` (L2) |

### `UI`

| setting | read in |
|---|---|
| `UI.ChallengeCareer.shouldValidateTracks` | Game_ChallengeCareer_BuildTrackTiles `0x141fc0740` (L2) |
| `UI.DemoEvent` | no code reference found |
| `UI.DisableMessages` | Ui_DataDrivenMessage_Init `0x1442a3070` (L2) |
| `UI.DisplayWatermark` | no code reference found |
| `UI.EnableAttract` | Game_AttractVideoVM_OnScreenActivated `0x1419f4b20` (L1), Game_TitleScreenVM_OnScreenActivated `0x141e9d630` (L2) |
| `UI.EnableCredits` | no code reference found |
| `UI.EnableMouseSupport` | Game_Read_UIConfig `0x1444a1b30` (L1) |
| `UI.ForceFanZoneSeasonResults` | Game_FanZoneSeasonResults_Exit `0x14238da40` (L2) |
| `UI.IsTextProcessorEnabled` | Game_Read_UIConfig `0x1444a1b30` (L1) |
| `UI.L` | no code reference found |
| `UI.MovieBuild` | Game_Read_UIMovieBuildSetting `0x143034b20` (L2) |
| `UI.PreviewEvent` | Game_Read_AudioConfig `0x140a249f0` (L1), Ui_Init_CarDriverTeamSelectors `0x141758910` (L2), Game_CareerContinueDriver_StartNewCareer `0x141a10d50` (L2) +5 |
| `UI.RoundSwipeAnimation` | Ui_F1CsElement_Init `0x1437f47d0` (L1) |
| `UI.ShowInProgressRoute` | Game_Read_UIConfig `0x1444a1b30` (L1) |
| `UI.ShowLoadingTips` | Game_LoadingScreenVM_OnScreenActivated `0x141ca43e0` (L2), Game_PauseAvailabilityVM_OnActivate `0x142bb74b0` (L2) |
| `UI.SkipFanZoneSeasonResults` | Game_FanZoneSeasonResults_Exit `0x14238da40` (L2) |
| `UI.TextEngine` | Game_Read_UIConfig `0x1444a1b30` (L1) |
| `UI.TextProcessorAffectsRTLLanguagesOnly` | Game_Read_UIConfig `0x1444a1b30` (L1) |
| `UI.UnicodePlurals` | no code reference found |
| `UI.UseKeyboardOnlyIconFont` | Game_Read_GameVersionNumberConfig `0x1437d4130` (L1) |
| `UI.defaultTrack` | Game_UITrack_Select_DefaultTrack `0x1438520f0` (L2) |
| `UI.overrideMainMenuState` | Game_FEHomeViewModel_OnScreenActivated `0x141bb17f0` (L1) |
| `UI.skipDLCUpsell` | Game_CareerWizardAddonDrivers_HandleChoice `0x141a25dd0` (L2), Game_MyTeamTeamIcons_HandleChoice `0x141a2d460` (L1), Game_DriverCareerDriverSelect_HandleCommand `0x141a5df70` (L1) +2 |
| `UI.skipFEFullPlatformLoginFlowEnabled` | no code reference found |
| `UI.skipFirstTimeBoot` | Game_FrontendTransitionVM_OnScreenActivated `0x141bb7bf0` (L2), Game_SplashTransitionViewModel_OnScreenActivated `0x141e9b9f0` (L1), FeFlow_Recalculate_TransitionState `0x1436d1500` (L2) |

### `Replay`

| setting | read in |
|---|---|
| `Replay.FullRaceReplay.Enabled` | Game_FullRaceReplaySystem_Init `0x142c45070` (L2) |
| `Replay.disable_onboarding_tutorials` | Game_Read_ReplayConfig `0x142c04a30` (L1) |
| `Replay.force_advanced_replay_ui` | Game_Read_ReplayConfig `0x142c04a30` (L1) |
| `Replay.reset_onboarding_flags` | Game_Read_ReplayConfig `0x142c04a30` (L1) |

### `PhotoMode`

| setting | read in |
|---|---|
| `PhotoMode.allow360VideosWithAnyReplayDuration` | Game_Read_PhotoModeConfig `0x141cea5d0` (L1) |
| `PhotoMode.allowAllWeatherChanges` | Game_Read_PhotoModeConfig `0x141cea5d0` (L1) |
| `PhotoMode.allowPhotoModeInPitlane` | no code reference found |
| `PhotoMode.cameraCollisionAvoidance` | no code reference found |
| `PhotoMode.cameraLockedToBoundingBoxAroundCar` | no code reference found |
| `PhotoMode.cameraLockedToHeightAboveTrack` | no code reference found |
| `PhotoMode.canPositionCars` | no code reference found |
| `PhotoMode.debugNum360VideoFramesToStopAfter` | Game_PhotoMode_InitDependencies `0x142bcd750` (L2) |
| `PhotoMode.devEnabled` | Game_Read_PhotoModeConfig `0x141cea5d0` (L1) |
| `PhotoMode.showCaptureTypeMenuOptions` | Game_Read_PhotoModeConfig `0x141cea5d0` (L1) |
| `PhotoMode.showLogoPositionOffOption` | Game_Read_PhotoModeConfig `0x141cea5d0` (L1) |

### `DevSessionDataContract`

| setting | read in |
|---|---|
| `DevSessionDataContract.ChampionshipComponentEnabled` | Sess_DevSession_ReadChampionshipComponentEnabled `0x1409995c0` (L2), Game_Unk_140affe60 `0x140affe60` (L1), Sess_DevSession_Apply `0x141125e80` (L2) |
| `DevSessionDataContract.Entrants.entrant_list` | ? `0x1407a38e0`, Sess_DevSession_Apply `0x141125e80` (L2) |
| `DevSessionDataContract.ForceStaggeredStart` | Sess_Build_AiSystem `0x1405ff750` (L2) |
| `DevSessionDataContract.Game.DefaultEntrantGarageOverrides` | Game_Read_DevSessionDataContractEntrantsConfig `0x141127eb0` (L1) |
| `DevSessionDataContract.Game.entrant_setup_mode` | Sess_DevSession_Apply `0x141125e80` (L2) |
| `DevSessionDataContract.Game.equalPerformance` | Sess_DevSession_Apply `0x141125e80` (L2) |
| `DevSessionDataContract.Game.laps` | Asset_Load_DevSessionWorlds `0x141124b30` (L2) |
| `DevSessionDataContract.Game.race_length` | Asset_Load_DevSessionWorlds `0x141124b30` (L2) |
| `DevSessionDataContract.Game.sessionLengthInSeconds` | Asset_Load_DevSessionWorlds `0x141124b30` (L2) |
| `DevSessionDataContract.Game.session_id` | Sess_DevSession_Apply `0x141125e80` (L2) |
| `DevSessionDataContract.Game.setup_mode` | Sess_DevSession_Apply `0x141125e80` (L2) |
| `DevSessionDataContract.Track.clear_extra_worlds` | Asset_Load_DevSessionWorlds `0x141124b30` (L2) |
| `DevSessionDataContract.Track.layer` | Sess_DevSession_Apply `0x141125e80` (L2) |
| `DevSessionDataContract.Track.list` | Sess_DevSession_Apply `0x141125e80` (L2) |
| `DevSessionDataContract.Track.name` | Sess_DevSession_Apply `0x141125e80` (L2), Game_UITrack_Apply_DevContractTrack `0x143853ef0` (L2) |
| `DevSessionDataContract.Track.worldAssetGroup` | Sess_DevSession_Apply `0x141125e80` (L2) |

### `ContentSet`

| setting | read in |
|---|---|
| `ContentSet.currentContentSet` | Sess_Get_CurrentContentSetName `0x1413e1cd0` (L2) |

### `Grid`

| setting | read in |
|---|---|
| `Grid.CarsInRow` | Game_Read_GridConfig `0x142f62120` (L1) |
| `Grid.Override` | Game_Read_GridOverrideConfig `0x142f61640` (L1) |
| `Grid.PoleLR` | Game_Read_GridConfig `0x142f62120` (L1) |
| `Grid.RowOffset` | Game_Read_GridConfig `0x142f62120` (L1) |
| `Grid.StaggerOffset` | Game_Read_GridConfig `0x142f62120` (L1) |

### `Qualifying`

| setting | read in |
|---|---|
| `Qualifying.full` | Game_Read_QualifyingConfig `0x142e44b50` (L1) |
| `Qualifying.one_shot` | Game_Read_QualifyingConfig `0x142e44b50` (L1) |
| `Qualifying.short` | Game_Read_QualifyingConfig `0x142e44b50` (L1) |

### `VTE`

| setting | read in |
|---|---|
| `VTE.Sessions.override` | Game_Read_VTEConfig `0x142f7f7b0` (L1) |
| `VTE.enabled` | Game_Read_VteEnabled `0x1407972c0` (L1), Game_Read_GameConfig `0x1407f7320` (L1), Game_Read_GridOverrideConfig `0x142f61640` (L1) +2 |
| `VTE.imgui` | Game_Read_VTEConfig `0x142f7f7b0` (L1) |

### `Testing`

| setting | read in |
|---|---|
| `Testing.EnableSmokeTest` | Render_Gfx_BuildConfigObject `0x140104f70` (L1), Game_ModeChampionship_Build `0x1407d2820` (L1), Game_Game_HandleInterruptEvent `0x1407fd8d0` (L2) +7 |
| `Testing.OverrideGraphicsPreset` | Game_Application_InitialiseHardwareSettings `0x1443c4fa0` (L1) |

### `Story`

| setting | read in |
|---|---|
| `Story.disableCinematicCinemascope` | no code reference found |
| `Story.forcedTitleUpdateValue` | Game_Read_StoryForcedTitleUpdateValueSetting `0x141869690` (L2) |

### `SafetyCar`

| setting | read in |
|---|---|
| `SafetyCar.noGhost` | Game_VehicleGhost_Init `0x142f1b700` (L1) |

### `DebugCamera`

| setting | read in |
|---|---|
| `DebugCamera.enableDpadCommands` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `DebugCamera.fov` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `DebugCamera.invertVerticalLook` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `DebugCamera.quickSpeedMultiplier` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `DebugCamera.shake.isActive` | no code reference found |
| `DebugCamera.shake.positionLimitsInMetres.x` | no code reference found |
| `DebugCamera.shake.positionLimitsInMetres.y` | no code reference found |
| `DebugCamera.shake.positionLimitsInMetres.z` | no code reference found |
| `DebugCamera.speedMultiplier` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `DebugCamera.wobble.isActive` | no code reference found |
| `DebugCamera.wobble.overallSpeedMultipler` | no code reference found |
| `DebugCamera.wobble.rotationLimitsInDegrees.x` | no code reference found |
| `DebugCamera.wobble.rotationLimitsInDegrees.y` | no code reference found |
| `DebugCamera.wobble.rotationLimitsInDegrees.z` | no code reference found |
| `DebugCamera.wobble.rotationSpeedMultipliers.x` | no code reference found |
| `DebugCamera.wobble.rotationSpeedMultipliers.y` | no code reference found |
| `DebugCamera.wobble.rotationSpeedMultipliers.z` | no code reference found |

### `DebugOSD`

| setting | read in |
|---|---|
| `DebugOSD.DisplayFPSonSpeedo` | Game_HudVM_ResolveSystems `0x141d5d910` (L2) |
| `DebugOSD.DisplayResOnERS` | no code reference found |
| `DebugOSD.GameInputModule.showInputProviders` | Game_Read_LeadPlayerConfig_2 `0x142a733f0` (L1) |

### `VideoCapture`

| setting | read in |
|---|---|
| `VideoCapture.enabled` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1), Ne_InputSystem_Init `0x14442f0c0` (L2), Game_ReplaySystem_ReadDbSettings `0x1445d54e0` (L2) |
| `VideoCapture.farMax` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.farMin` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.fovMax` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.fovMin` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.nearMax` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.nearMin` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.rotationSpeedMax` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.rotationSpeedMin` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.translationSpeedMax` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |
| `VideoCapture.translationSpeedMin` | Game_Read_VideoCaptureConfig `0x140ad22a0` (L1) |

### `VideoExport`

| setting | read in |
|---|---|
| `VideoExport.Enabled` | Game_VideoExportModule_Initialise `0x1444b1130` (L2) |

### `EnvArtMode`

| setting | read in |
|---|---|
| `EnvArtMode.enabled` | Sess_PreRace_ResolveComponents `0x142d80950` (L2), Sess_Persistent_OnStateEnter `0x1436e5850` (L2) |

### `HudCustomisation`

| setting | read in |
|---|---|
| `HudCustomisation.disableLegacyScript` | Game_Read_GameVersionNumberConfig `0x1437d4130` (L1), Ui_F1HeElement_Slot138 `0x143808570` (L1) |
| `HudCustomisation.imguiEnabled` | Ui_F1HeElement_ResolveSystems `0x1438072f0` (L1) |

### `Career`

| setting | read in |
|---|---|
| `Career.Online.KickFailingSynchronisation` | Game_CareerOnlineDocumentComponent_Initialise `0x140fcd6b0` (L2) |

### `TimeManager`

| setting | read in |
|---|---|
| `TimeManager.cap` | Game_Read_TimeManagerConfig `0x1443d1bc0` (L1) |
| `TimeManager.capValue` | Game_Read_TimeManagerConfig `0x1443d1bc0` (L1) |
| `TimeManager.fixedTimeStep` | Game_Read_TimeManagerConfig `0x1443d1bc0` (L1), Game_TimeManager_OnResetFixedTimeStep `0x1443da4d0` (L2) |
| `TimeManager.mode` | Game_Read_TimeManagerConfig `0x1443d1bc0` (L1) |
| `TimeManager.smoothing` | Game_Read_TimeManagerConfig `0x1443d1bc0` (L1) |

### `Network.LAN`

| setting | read in |
|---|---|
| `Network.LAN.active` | Ui_OnlineCustomItem_Init `0x14172c5a0` (L2), Ui_OnlineCustomItem_GetAction `0x14172c840` (L2) |

### `Network.Season`

| setting | read in |
|---|---|
| `Network.Season.AIDifficulty` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.AIDrivers` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.AISkillRatingMax` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.AISkillRatingMin` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.AbsAllowed` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.BrakingAssistsAllowed` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.CanReturnToGarage` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.CarPerformanceType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.CarSetupOptions` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.CollisionsFirstLapOnly` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.CollisionsOn` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.ContentDomain` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.Enabled` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.FormationLap` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.FormationLapType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.GearBoxAllowed` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.Ghosting` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.ParcFermeRulesOn` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.PitAssistAllowed` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.PracticeType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.QualifyingType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.RaceLength` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.RaceStarts` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.RaceStartsAllowed` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.RaceType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.RacingLineAllowed` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.SafetyCarFrequency` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.SafetyCarType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.Scenario` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.StartingGridType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.TimeOfDay` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.TimeOfDayType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.Track` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.TractionControlAllowed` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.TyreCarcass` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.VehicleDamageType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.Versus` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.WeatherType` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Season.ersModeAllowed` | Game_Read_NetworkConfig `0x143040da0` (L1) |

### `Network.Dev`

| setting | read in |
|---|---|
| `Network.Dev.AllowMultiplayerRestart` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.DepartedTimeOut` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.FailSteamAuthentication` | Game_OnlineLogInTask_Init `0x143369930` (L2) |
| `Network.Dev.ForceCrashOnGameModeDeselectionDuringMultiplayerSession` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ForceDirectP2P` | no code reference found |
| `Network.Dev.ForceLOD` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ForceSpectate` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.LeaveMultiplayerOnGameModeDeselection` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.PacketCapture.AutoPacketCapture` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.PacketCapture.SendToElasticSearch` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.PacketCapture.SendToLogFileServer` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.PitEveryLaps` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.RandomisePlayerCars` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.Robodriver` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowAuthorityManager` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowBandwidthGraph` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowBandwidthStats` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowConnectionMap` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowConnectionQuality` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowContentDomainVerification` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowFontHeight` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowHopperMatching` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowPlayerSet` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressAnalysis` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressBandwidth` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressCalculator` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressClock` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressHistory` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressInfo` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressLogger` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressObservers` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowProgressTrackspace` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowReadyMonitor` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSafetyRating` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSavedChampionshipState` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionAttribs` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionDrivers` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionFormationLap` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionMigration` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionNextTrackVoting` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionPlayers` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionRaceInfo` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionRaceStatus` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionSeason` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionStartLights` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionStats` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionTeams` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSessionVoting` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowSkillRatingSimulator` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowVehicleControllers` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowVehicleControlsState` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowVehicleGhostingState` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowVehicleLaptimes` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowVehicleMechanicalState` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowVehiclePenaltyState` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowVehiclePitState` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowVehicleProgressState` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowXpos` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.ShowYpos` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.TestVehicleDelta` | Game_Read_NetworkConfig `0x143040da0` (L1) |
| `Network.Dev.TraceLogging` | Game_Read_NetworkConfig `0x143040da0` (L1) |
