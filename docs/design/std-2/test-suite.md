---
title: "Test suite: design and plan"
description: How swofty test reaches each game state without clicking, what already works in the game and the launcher, what needs building or reverse engineering, and the order to build it in.
---

# Test suite: design and plan

The goal: any test, written by an addon author or an agent, starts F1 25 in the state it needs and checks it, with no menu clicking, no playing through races and no risk to the player's own saves. [Testing your addon](/std/testing) is the author's side. This page is how it's built.

It has two halves:

- **The CLI** (`swofty test`) plans the run, takes the game lock, builds a throwaway profile, prepares the save sandbox and the environment, launches the game, collects the results and puts everything back.
- **The test agent** is a core component that runs only in test mode. It takes commands from the CLI over a local mailbox and carries them out inside the game: open a route, start or skip a session, hand the player's car to the AI, read and write values, wait for events, capture frames, save, quit.

## What already works

- **Launch and roll back.** `swofty run --apply --direct` builds a profile into the game folder and puts it back on exit. Session logs, crash reports with symbolised stacks, the data dir's session lock and exact-PID game detection all exist.
- **A race straight from launch.** The game's own `-benchmark <xml>` mode boots into an all-AI race in about 25 seconds with no menus. The XML sets the track, laps, weather, number of cars, the player's grid slot and, through an undocumented `setup_mode`, the box. The AI drives the player's car.
- **Opening menus.** The core already fires the game's UI routes on the home view model (an existing core menu driver does it), and the career hook sees every route the UI fires, so routes can be recorded and replayed.
- **Changing careers.** The Career Editor's live API sets money, resource points, R&D, contracts, points and calendar data in a running career.
- **Reading state.** The car and career structs in the KB, the core's events and log lines, the game's own log channels, `registry/effective.json` and frame capture.
- **Tracing.** The trace plugin installs hooks from KB signatures and logs registers and fields. It ran in the real game for the first time on 2026-10-28, with all 11 hooks installed.

## Environments

- **`boot`**: launch to the main menu. The agent passes the title screen (`title`, below), then waits for the home screen (`home`, below), answering the core's notice on the way.
- **`benchmark`**: write `benchmark/swofty_test.xml` into the game folder for the run and launch with `-benchmark`. The CLI needs to pass game arguments in a direct launch. Race sessions only.
- **`route`**: `boot`, then the agent fires each route, from the screen the game is on (the newest tile screen) or else from the home screen. A route counts only when `Vm_Run_Route` ran it. Picking a tile inside a tile screen is `tile <id>` (below); screens that aren't tile lists (team select, track select, the garage) are driven with the virtual pad and `wait screen=<name>`.
- **`career`**: the save sandbox, the fixture's saves copied into it, then the agent continues the career to `at`: `team_hq` (the default), `weekend_hub` or `session`. After the agent's hello the CLI sends `career` `{"op":"load","slot":<n>|"newest","at":<at>,"mode"?}` (`career load <slot|newest> at <at>` in the shell; the mode comes from the fixture's `holds.careers` when it says which) and waits for its answer; the agent logs `[test] career ready at <at> (slot <n>)`. Every step waits on a flow or screen state with its own time limit and fails naming the step that didn't happen. The steps, from static RE on the Steam build (no signatures yet, so the agent binds them only on the KB build):
  1. **The title** (`title`, below), unless the front end (`StateFrontend`) is already up.
  2. **Route `career`** on the home view model, as the core's menu driver fires routes (L2).
  3. **The career tile.** (In game on 2026-10-30, with the core's season pick: the tile's action is `{type 1, career_dlc_select_driver|_my_team}`, which opens the season pick; its tiles `career_default_content` (2025) and `career_dlc_content` (2026) answer `solo_career_continue_*` when that season has a save, and the agent fires the first that does. The tiles are found by id because the core may subclass the career item's vtable. The rest of this step is the 1.6.0 reading.) fe_career's view model (`vt_Game_FECareerViewModel` 0x1457385b0, L2) is seen at the tile list populate loop like the home one. Its TileList (vm+0x2c8, entries `{node, item}` from +0x78 to +0x80) holds the career items (vtable 0x145703eb0); the item's kind is the NeStr at +0x178 (djb2 0x73b00842 `driver_career`, 0x353c4a02 `my_team_career`). The agent calls the item's GetAction (slot 5, `FeTile_Route_CareerItem` 0x141725620, L2), which writes the wizard data group and returns `{type 1, route solo_career_continue_driver|_my_team}` while the mode has a save, and fires that route as the tile screen does on `select_pressed` (`Vm_Fire_Route(vm->router, vm, &route, 0)`). Without a mode it tries My Team, then Driver Career; a tile whose action is not a continue route fails the load.
  4. **The resume card.** The resume view models (vt 0x1457264a0 driver, 0x145726670 My Team) are seen in `Game_CareerContinue_HandleCommand` (0x141a1db90, L1 with an agree, V1485), which the UI raises `transition_in_finished` into when the screen opens. Of the three cards (vm+0x328+i*0x540; has a save at +0, slot id at +8), the agent picks the slot asked for, or the first with a save for `newest`, and raises `slot_focused` then `slot_pressed` with data `i` through the handler. Presses are ignored while vm+0x1380 is set. `Game_SaveSlotManager_SelectSlot` loads it with no confirm popup.
  5. **Team HQ:** the flow enters `StateTeamHQ` after the press. `at team_hq` is done.
  6. **`weekend_hub`:** Team HQ's advance (`advance` with no day limit, below) to the next event, then the flow enters `StateSession`, the weekend's hub (L0: ts3b's reading; a slot saved inside a weekend may resume there directly, which needs a fixture and one in-game check).
  7. **`session`:** answers `unavailable`. The garage's `goto_track` command (`vt_Game_GarageCockpitViewModel` 0x145739830, slot 10 0x141bc5130, djb2 0xac456bd2, L1) is found, but not what releases the car after it, nor the confirms of its popups (time warning, parc fermé, tyre choice): it needs one trace of `goto_track` in a practice session.
  - There is no deep link to a given slot: the deep-link manager's career activity resumes the mode's current slot only.
- **`session`** (`session.start`): the game's settings-driven dev session. `DevSessionSetupChampionshipComponent` (vtable 0x1456bfed0) reads `DevSessionDataContract.*` keys with `Ne_Config_GetString` (`Game.setup_mode`, `Game.entrant_setup_mode`, `Entrants.entrant_list`, `Track.name`/`Track.layer`/`Track.list`, `Game.session_id` as one of the 19 session names, `Game.laps`) in `Sess_DevSession_Apply` (slot 25, 0x141125e80) and passes the built sessions to the session service at comp+0x398 (vcall +0x268). The switch is comp+0x350, which `Sess_DevSession_Enable` (0x141124ad0) sets to 0 in retail; writing 1 after `Sess_DevSession_Init` (slot 9, 0x1411244f0) turns it on, and a wrap of `Ne_Config_GetString` answers the keys. No key exists for weather, AI difficulty, team or driver, and a Time Trial has no championship, so it needs its own path (`Sess_TimeTrialMode_FillDescriptor`, 0x142936760). The benchmark mode (`Game_BenchmarkGameMode_BuildSessionSettings`, 0x1407a4860) fills SessionSettings itself and runs races only. Open: the order of slots 22 and 25 against the championship build, and the call that starts `StateLoadSession` from the front end. The command validates and fills today and answers `unavailable: needs <row>` until those rows have their verify rows.

## Straight into a Grand Prix: `swofty test session`

```sh
swofty test fixture capture gp-base --from <a copy of userdata/<id>/3059520/remote> --store ~/fixtures
swofty --profile default test session grand_prix cs_f1_2020 silverstone session=race laps=3 team=ferrari driver=vettel \
    --fixture ~/fixtures/gp-base --lock /mnt/work/F1/f1-game.lock.d
```

In game on 2026-11-01 (Steam build, release CLI, profile `default` with Classic Seasons, a fixture made from a copy of the player's save):

- `grand_prix cs_f1_2017 melbourne session=practice1 team=williams` (launcher 1.7.6 build): the cover is up from the first frame, the game boots behind it, and the player sits in the cs17 Williams of Lance Stroll in the garage at Albert Park, Practice 1 (`Look Around / Session Info / Go to Track`). Launch to garage is about 2 minutes: the profile build, the boot, and the session's broadcast intro, which runs under the cover.
- `grand_prix cs_f1_2020 silverstone session=race laps=3 team=ferrari driver=vettel` (1.7.7 build): the cs20 Ferrari of Sebastian Vettel on the Silverstone grid after the race's grid rundown, 3 laps (`[direct] race laps 52 -> 3`), `Car Setup / Start Race`.

No title, legal, main menu or loading screen is visible. The cover shows the start's steps (`Starting F1 25`, `Profile loaded`, `Weekend ready`, `Loading silverstone`, `Starting the session`, `Opening the garage` or `Lining up on the grid`).

The words after the mode:

```text
grand_prix <box> <track> [team=<label part>] [driver=<label part>] [session=<name>] [laps=<1..100>]
```

- `box` is an entrant list label (`cs_f1_2017`, `f1_1990_gp`, `f1_2025`), `track` a track name (`melbourne`, `silverstone`).
- `team=` and `driver=` pick the player's seat: the first entrant of the list whose `entrants.teams` / `entrants.drivers` label contains the text. Without them the player takes the list's first seat.
- `session=` is `practice1`..`practice3`, `qualifying`, `race`, or a game session name (`practice_s`, `qualifying_o`, `race_2`, ...).
- `laps=` sets a race's laps.
- `weather=` and `ai_difficulty=` are accepted and not applied yet.
- Without a box or track the start takes `f1_2025` at `melbourne`. It never continues a saved Grand Prix.
- A box whose entrant list the game did not load fails the start (`[direct] session start failed: the entrant list 'cs_f1_2017' (stat id 114) is not loaded`) and the menu opens.

The CLI writes the request to `<run folder>/direct_start.json` and starts the game with `SWOFTY_DIRECT_START=@<that file>`:

```json
{"target": "session", "mode": "grand_prix", "box": "cs_f1_2017", "track": "melbourne", "session": "practice1", "team": "williams"}
```

The game runs on a copy of the `--profile` profile (the active one without the flag), with the same addons in the same order and the same settings, so addon boxes and their database content are there. `swofty test shell --profile <p>` takes the same copy; `swofty test shell` without `--profile` keeps the minimal profile (core and `--addon` only).

What the core does (`native/src/core/direct_start.cpp`), in order:

1. The cover arms in `room_cover::setup()` for a `session` request and draws from the present hook's first frame.
2. The title passes with the shared virtual pad (`title_skip.cpp`); the profile loads; `StateFTUE`'s finished link is held.
3. The new weekend, on the Grand Prix mode entity E: setup mode `swofty_f1`, the entrant list by name into E+0xe18, one round at the track, the season build, E's components attached, `Sess_Build_DevWeekendStructure` (0x140b025f0) handed to the weekend service (registry component 0x14605ada0, slot 0x268), and the entrants filled from the list (0x142460230; it clears a saved Grand Prix's entrants first).
4. The player's seat: the chosen entrant gets the profile's identity block (`[[[game manager+0x140]+0x1c0]+0x1c0]` slot 0x38, 0x40 bytes to +8) and the human flag (+0x48 = 0x10000000), between the entrants-changing bus messages (0x1424e2b90 with vtable 0x14578d4e8, 0x1424e2f00 with 0x14578d510).
5. The season build leaves the weekend on track stat id 0 (or the saved weekend's), so the core writes the track's stat id to weekend+0x278 straight after the build, before the components attach and the sessions are built, and the box's content is picked (`box_picked`, as a box press does in the menus) so its `car.profile` layers apply. A profile that already holds a Grand Prix keeps its season, so the weekend is the saved one: the core writes the track's stat id to weekend+0x278 and makes session 0 the requested session (weekend sessions at +0x220, 0xa0 apart, id at +8, current index +0x238). Moving the current index to a later session instead crashed a race in `Ai_Build_TeamPerformanceCurves`.
6. `StateFTUE` leaves by its exit link with the game's interrupt exit (kind 0xe) raised on the empty front of the interrupt queue (state +0xbf8, front at +0x30, kind at +0x10), so `StateFTUE::onStateExit` (0x1436d8390) despawns the front end's main scene and blocks until the scene's flags (+0x1048, +0x1049) and component set (+0xf0) are clear, which frees the front end's `PhysicsTerrain` (live counter 0x146cb5d50) before the track's is created; the core clears the kind when that exit returns, since the queue is shared with the parents, and `StateFrontend` (the menu) never starts, and `StateFrontendWorld` takes `session` (0x8c23ce89): `#e2dcf787 > StateSmokeTestSkipFEPreLaunchHold > StateCerberusServer > #d36d1718 > StateLoadSession`.
7. `StateLoadSession`'s exit (0x1436e0e40) waits for its sync point (+0xcd0: request node +0x80, UI node +0xa0), which the loading screen sets in a menu start. 500 ms into the exit the core sets the UI node with `Vm_NodeSet_Bool` and the flow goes on to `StateSession`. Without it the game parks on a black screen after the load (`#8ad771dc` is the normal last child of the load, not the stop).
8. For a race, `Game_ChampionshipGameMode_BuildSessionSettings` (0x1407b7b90) is hooked and the laps written to SessionSettings+0xdf0 (XOR +0xdf8).
9. The front end's main scene (the menus' dark backdrop with red light streaks) is despawned by that `StateFTUE` exit (`Game_FrontendSceneManager_DespawnMainScene` 0x1428d9a90 on the node's +0xd20), as `StateFrontend`'s exit does in a menu start; the despawn the core makes when `StateSession` first updates finds it already gone. The interrupt's fade-out reason (bit 0 of the fade owner's mask) is released with the FrontEnd reason at that point.
10. The cover comes off 1.5 s after the `garage_cockpit` screen opens. A race opens no screen of its own: its cover comes off at the first fade in after the intro video, when the fade owner's reasons (`[[g_Game_Root]+0xa0]+0x7b8`) drop from BroadcastFull to none with the state at 3, about 3 s after `StateSession` started; the broadcast's grid rundown follows and the grid comes two minutes or more later. Without either, the cover comes off 60 s after `StateSession` started updating, and at once when the start fails.

The flow is logged as `[direct] ...` lines: the weekend before and after (`weekend ..., track stat id 0, current session 0 of: practice_1 race`), the seat (`the player drives for cs17_williams as lance_stroll (seat 6)`), `StateLoadSession exiting for 503 ms: its sync point is complete` and `the session's screen garage_cockpit is up`.

## Straight into a Time Trial

```sh
swofty --profile f1-1990 test session time_trial f1_1990_gp silverstone team=ferrari \
    --fixture ~/fixtures/gp-base --lock /mnt/work/F1/f1-game.lock.d
```

```text
time_trial <box> <track> [team=<label part>] [weather=clear|wet]
```

- `box` is a box's Grand Prix list or its Time Trial list (`f1_1990_gp` and `f1_1990_tt` both load `f1_1990_tt`), or any entrant list a setup mode offers: `swofty_f1` when it offers the list, else the first setup mode that does (`setup_mode=` names one).
- `team=` picks the player's car: the list's team whose label is the text, then holds it as a word (`ferrari` finds `ferrari_1990`), then contains it. Without it the player takes the list's first team.
- `weather=` sets the Time Trial's weather (`settings.current_settings.tt_weather.value`, `dry` or `wet`).

The request is the Grand Prix one with `"mode": "time_trial"`:

```json
{"target": "session", "mode": "time_trial", "box": "f1_1990_gp", "track": "silverstone", "team": "ferrari"}
```

`direct_start.cpp` calls the setup that `direct_start_tt.cpp` registers with `runtime::direct_start_register_time_trial`, on the flow thread while `StateFTUE` is held:

1. The Time Trial mode: the game modes context's entry labelled `time_trial` (ctx+0x58..+0x60, mode at entry+0x38, label at mode+0x278, entity E at mode+0x260) made current with `Game_Game_SetCurrentGameMode`. `Game_GameModes_FindByName` returns championship modes only.
2. 1.5 s later, the setup on E as the screens leave it: setup mode `swofty_f1` (or the first that offers the list) with its setter (0x14079f300), the entrant setup mode that lists the box's list into E+0xe10 (the setup mode's entrant setup modes are 12 inline pointers from +0x10, count +0x70) and the list into E+0xe18.
3. The track: E's session settings (slot 0xd8, body 0x148731f80) read the track from 0x1407ddbf0, which looks up the VM string `game_mode_data.current_game_mode.track_info.current_track.track_file_name` in E's track system (E+0xf58, record name +0x78). The core's path setter cannot write that node, so the getter is wrapped: while it still reads the profile's track, it answers the requested record.
4. The car: `Sess_TimeTrialMode_FillDescriptor` (0x142936760) takes the player's vehicle and team from the Time Trial component's hot load holder (the mode's component with vtable 0x1457b13d0, holder at +0x298: vehicle ref +0x2be engaged +0x2c2, team ref +0x2c4 engaged +0x2c8, written in game by the in-session hot load 0x14291a320). The core writes the chosen team's entrant record refs there (+0x58 team, +0x5c vehicle). Without them the fill takes the vehicle from a per-team table built for the list the screens picked, and the session faults in `Ai_Build_TeamPerformanceCurves` with no player vehicle.
5. The flow leaves the front end as for a Grand Prix and loads `TimeTrial` (#ca472e10): intro video, intro cameras, the garage.
6. When `garage_cockpit` opens, the front end's main scene (the menus' dark backdrop with red light streaks) is despawned: `StateFrontend`'s exit does it in a menu start (`Game_FrontendSceneManager_DespawnMainScene` 0x1428d9a90 on the state's +0xd20), and a direct start never enters `StateFrontend`, so the core finds that node (0x957880c6) in the flow graph and calls it on its scene manager.

The log lines are `[direct] Time Trial: ...`: the mode, `setup mode swofty_f1 (...), entrant setup mode ..., list f1_1990_tt (...) for box f1_1990_gp`, `track silverstone (...)`, `team ferrari_1990 (team ref 01f70000, vehicle ref 01fe6f00) in the hot load holder ...`, then `the track getter read ...; it answers the chosen track ...` during the load.

## The agent's commands

- **Skip a session:** post `MessageSetAdvanceDestination` (vtable 0x1457548b8) on the message bus, the same message the game's Simulate Session popup sends. Destination 2 is next session, 3 simulate, 4 skip to a session, 6 quick practice skip; the session type is the race (15) and so on.
- **The home screen (`home`):** the home view model (`FEHomeViewModel`) is seen at the tile list populate loop (`site_FeTile_PopulateLoop`, r14 the list): `Game_FEHomeViewModel_OnScreenActivated` (0x141bb17f0) stores the TileList it builds at vm+0x358 and passes it to `FeTile_Build_List` as `self` (L2), so the agent takes the stack value whose vtable slot 10 is `FeHome_Handle_Command` and whose +0x358 is r14. Other tile screens keep their list at +0x2c8; the home view model's +0x2c8 is another object, which is why 1.6.0's agent never saw the home screen. After the title the core's notice (the Swofty Mod popup, the game's persistent multichoice message) sits over the front end's `frontend_transition` screen and holds the home screen back until it is answered: once the screen has been `frontend_transition` for 4 s, the agent presses the virtual pad's a (held 250 ms, every 2 s) until the home view model is seen, then lets go. `home` answers `{"home": true, "notice_presses": n}`; `route` and `career load` run the same wait first.
- **Tiles (`tile <id>`):** on the newest tile screen, the agent finds the tile by its item id (TileList entries `{node, Tile *}` from +0x78 to +0x80, the item at Tile+0xd0, its id NeStr at item+0x10), calls the item's get_action through its own vtable slot 5 and, for a route action (type 1), fires the route from the screen's view model, as the screen's `select_pressed` does; the answer is `{"tile", "route"}`. Another action type fails naming it. Every tile screen the agent sees is logged with its tile ids: `[test] tile screen <vm> (vtable base+0x...): story, career, mod_menu, f1_world, ...`. Seen in game on 2026-10-30: home (base+0x5738a60), F1 World (base+0x5738720: play, loadout, customisation, trophies, showroom), Career (base+0x57385b0: driver_career, my_team_career, online_career, c_career) and the career season pick (base+0x5738430: career_default_content, career_dlc_content). F1 World's Play cards (`f1_world_activity_container`), Time Trial's season row, team select, track select, the garage and the pause menu are not tile lists and are driven by the pad. Reading the selected tile (TileList+0x58, the `selected_button` node: `FeTile_List_GetSelectedItem`'s body 0x14b76bc60 follows the node's references and finds it among the buttons node's children with 0x14420b270) was tried and is not used: neither the holder's value nor the followed node matched an entry.
- **Pass the title screen without input (`title`):** the title (`StateIntro`, `FeFlow_StateIntro_OnUpdate` 0x1436da4d0, L1) waits for its sync point (state+0xcd0, `sync_points.boot`; `Vm_SyncPoint_IsComplete` 0x1442626c0) to have both bool nodes true: the request node +0x80 and the UI's node +0xa0, which the title's UI sets on a button. It then sends its own request (kind 0x2e) whose callback sets +0xef4, and its next update returns the front-end link. The agent hooks the update (KB build only), and once the title's view exists sets the two nodes with `Vm_NodeSet_Bool` (0x140973e20, `(node, &value, 0, 0)` as the sync point's own writes), so the game's own path runs from there. It logs `[title] set the sync point's ... node`, then `[title] engaged`; a title that doesn't pass answers with the gate it stopped at. In game the title's view only comes up after a press, so the sync point alone never passes it: when the title has not passed 3 s after `title` (or the first career load step), the agent presses the virtual pad's a itself (held 250 ms, released 1750 ms, long enough for a frame to see it while the game runs in the background) until the title engages or the front end is entered, then lets go. On 2026-10-30 the title engaged after 7 presses (`[test] title passed after 7 press(es) of the virtual pad's a`), with the game parked on workspace 42. The press reaches the game through the agent's XInput hook, not the window, so focus doesn't matter.
- **Advance the career calendar (`advance {days}`):** not working in game (2026-10-30): with the hook on slot 12 installed (`"advance": "ok"` in hello), Team HQ's advance panel never reached it while Team HQ showed Advance, so the command answers `no Team HQ advance view model`. That fixture's career also refused Advance until today's tasks were done (Andrew Lung: "finish things here before we can proceed"). The agent now also takes the view model from its onCommandRaised (slot 10), as it sees the resume screens; that is built and not yet run in game. The reading below is static. Team HQ's advance view model (`vt_Game_TeamHQAdvanceViewModel` 0x14577a9c0, L2) is captured at its per-frame update (slot 12, 0x1422ca380, L2). The agent raises `timeline_advance_pressed` (djb2 0xdb427f42) through its onCommandRaised (slot 10, 0x1422c3540, L2) as the Advance button does: PressAdvance (0x1422c6860) checks vm+0x620, canAdvance (+0x758) and advancing (+0x759), shows today's cards instead when there are any, else runs `Career_TeamHqAdvance_Start` (0x1422bb0f0) and the screen's `start_advancing`. The game advances a day per tick to the next event; for `days` the agent raises `timeline_stop_pressed` (0x91c24f96) once the day (the game's own `(u32 [[vm+0x6e0]+0x2b0] >> 5) / 45`, the career clock in minutes over 1440) reaches the start plus `days`. The answer comes when advancing (+0x759) clears, or the view model goes away as the flow leaves Team HQ: `{"from_day", "to_day"}`. Sandbox only.
- **The AI drives the player's car:** `Game_Set_VehicleController_Thunk(system, vehicle id, 1, reason)` with the controller-set flag DEV (index 6 of the game's flag table: 0 PIT_LANE, 5 PAUSE, 6 DEV, 7 BENCHMARK, ...), which the thunk sets in the car's controller set when the third argument is 1 and clears when it is 0. Pass 0 to hand the car back.
- **Read and write car and career values:** the typed accessors `cars::live` uses, so tests and addons share one implementation of the scrambled values and the derived-value rules.
- **Input:** a virtual pad inside the game process (XInput slot 0), for what has no direct call yet.
- **Save now (`save now`):** the game's own profile save, `Game_SaveMgr_RequestProfileSave_Thunk` (0x1436595b0, L2) with the save manager system and `(mgr, &{0, 0}, 1, 0)`, as Team HQ's autosave after an advance. The manager is the system keyed `k_Game_SaveMgrSystemKey` (0x146100ca8) in `g_Game_Manager`'s system map (`[0x14604c2c8]+0x30`; `idx = 0x14030ffd0(map, &key)`, a miss returns the entry count; entries are 0x10 bytes from `[map+0x18]`, the system at +8). It answers once the sandbox has served a FileWrite. Sandbox only: the agent refuses it, `career load` and `advance` unless the save sandbox armed for the run (`sandbox.json` says armed), since the game would otherwise save into the player's own profile.

## The test run

One contract starts the agent and the sandbox, so a test DLL, the agent and the sandbox all arm or none does:

- **The run folder.** For a batch that needs the agent or the sandbox, the CLI makes `<data>/test-runs/<run>-<batch>/` with `run.json`: `{"schema":1,"run":"<id>","token":"<32 hex>","agent":{"v":1,"dlls":["dll/<suite>.dll"]},"saves":"saves","exe_sha256":"..."}`. `saves` (and `saves/remote/`) only when the save sandbox is required, which is every `career` batch and `swofty test shell --fixture`. The agent's mailbox is `agent/`: `commands.jsonl` (the CLI appends one JSON command per line, `{"seq","cmd","args","timeout_ms"}`), `results.jsonl` (the agent appends one line per reply, `{"seq","status":"ok|error|unavailable|timeout","value","error","ms"}`, or event, `{"event":"hello|fired|expect|log|refused",...}`), `dll/` and `frames/`.
- **The environment.** The game starts with `SWOFTY_TEST_RUN=<run folder>` (a Unix path maps to Wine's drive) and `SWOFTY_TEST_TOKEN=<token>`, plus `SWOFTY_EARLY_STAGE=1` with the sandbox, so the core loads before the save device exists.
- **Arming.** The core reads `run.json` once (`save_sandbox.cpp`). With either variable set, a token that isn't `run.json`'s, a missing or malformed `run.json`, or a test DLL path outside `agent/` ends the game with `[saves] TEST SANDBOX FAILED: <why>` (exit 91) before the title can load a save, whether the run asked for the sandbox or not. A good run arms the agent (`[test] agent armed for run <id>: ...; save sandbox required|off`) and, with `saves`, the sandbox. Normal Play sets neither variable, so it never arms either, never loads a test DLL and never installs their hooks.
- **The end.** After the batch the CLI removes the token from `run.json`; the agent sees that within half a second, stops and lets go of the pad. A run folder with the sandbox stays for `fixture capture`; one without is removed after the agent's results are kept as the batch's `agent.jsonl`.

## The save sandbox

The game keeps every career, Grand Prix and setting in one profile save: two encrypted files in Steam's `userdata/<account>/3059520/remote/`, which Steam Cloud syncs. It reads and writes them through Steam's storage API, so turning Steam Cloud off isn't enough: the files would still be written to the player's real folder.

The sandbox is a core feature at the save device's start (`Ne_SaveSystem2Win_Start`, 0x14413af70; the RE is in `re/db/subsystems/core.md#save-sandbox`). It runs only when `swofty test` starts the game with a run token:

- **The run folder.** The CLI makes `<data>/test-runs/<run>-<batch>/` with `run.json` (`{"schema":1,"run","token","saves":"saves","exe_sha256"}`) and starts the game with `SWOFTY_TEST_RUN=<folder>`, `SWOFTY_TEST_TOKEN=<token>` and `SWOFTY_EARLY_STAGE=1`, so the core loads at the game's first mutex, before the save device exists. After the batch the token is removed from `run.json`.
- **The proxy.** The device caches Steam's `ISteamRemoteStorage` at its Steam helper +0x108, written once before Start. The core's wrap of Start checks the device (its vtable slot +0x08 is Start, the helper exists, `m_storeInCloud` +0x3e8 is on, the interface's vtable lies outside the game) and swaps the pointer for a proxy that serves every slot from `saves/remote/`. The device calls FileWrite, FileRead, FileDelete, FileExists, GetFileSize, GetFileTimestamp, GetFileCount, GetFileNameAndSize and the write batch; the proxy serves those and a few more, answers 0 to the rest and logs them, and never calls Steam. The save files are opaque bytes to it. Loads and saves mount into `%TEMP%` and go through the interface, so nothing else is written.
- **The user data folder stays.** The helper's +0x00 (`GetUserDataFolder`) is only the prefix the device strips from its title folder to build remote names, never a path it opens; rewriting it would break the names and redirect nothing.
- **The corrupt-save upload is off.** Device slot +0x58 (`Ne_SaveSystem2Win_WriteInfoXml` 0x144141c00) zips a save and uploads it through Steam's global interface, outside the helper, so in a test run it returns without doing anything.
- **Fail closed.** With a token in the environment, any doubt ends the game, logging `[saves] TEST SANDBOX FAILED: <why>` and writing `sandbox.json` `{"state":"failed"}`: a token that isn't the run's, a binding that doesn't resolve, a wrap that doesn't install, a device or interface that isn't the expected one, or a mount (`Ne_SaveSystem2Win_Mount`, which every load and save passes before any file read) before the sandbox armed. Armed, it logs `[saves] test sandbox armed (run <id>)` and writes `{"state":"armed"}`.
- **Preflight and backstop.** The CLI refuses a career batch unless the core's binding set for this exe's sha256 carries `Ne_SaveSystem2Win_Start`, `_Mount` and `_WriteInfoXml`; the export gate needs an independent `agree` for each, so that is the static proof. After the batch it fails the tests unless `sandbox.json` says armed, and compares the player's Steam save folder listing (names, sizes, times, never contents) before and after.
- **Local-save mode** (`Steam.cloudEnabled` false, the EA layout) is not a sandbox: the device then reads and writes the player's own `userdata/<id>/3059520/local/savegame`. The sandbox refuses it.

## Fixtures

- **What's in one:** the two profile save files, the addon storage that lives outside the save (Progressive's career index, the Career Editor's points sidecar), the launcher profile, and `fixture.json` with the exe build and the addons it needs. About 1 MB.
- **Layout:** `fixture.json` (schema 1: `name`, `description`, `created`, `exe` `{sha256, version}`, `addons` `{id: version}`, `holds` `{careers: [{mode, slot, name, season, round, team, driver}], notes}`, `files` `{path: {size, sha256}}`, `source`), `save/` with the Steam remote names (`savegame@profile#NXDSMWW.BWW` and `#QKRHMYXE`, plus mid-session saves; never highlights or corrupt-save bundles) and `storage/<addon id>/`. A run checks every file against `fixture.json`, copies `save/` into the sandbox and `storage/` into the batch profile's addon storage copy, which the session loads. Each career batch gets its own launcher profile and run folder.
- **Where they live:** a test's `fixture` is a folder next to its test file (`fixtures/my-team-r3`), else a fixture in the store (`$SWOFTY_TEST_FIXTURES`, else `<data>/test-fixtures/`) by the same path or its last part. Fixtures for public addons can ship in the addon's `tests/fixtures/`. The project's own fixtures carry swofty's careers and names, so they stay out of the repo: the store on this machine is `/mnt/gaming/swofty-std16/fixtures/` (set `SWOFTY_TEST_FIXTURES` to it).
- **Capture:** `swofty test fixture capture <name>` copies the newest run's sandbox (or `--run <id>`'s) and the addon storage the run left, with the run's exe and addons; `--from <dir>` takes a copy of a save folder instead and refuses the live one. `--career mode:slot`, `--description` and `--notes` fill `holds`. A fixture is never overwritten. `swofty test fixture list` and `check <name>` verify the files.
- **The first ones:** `swofty-profile-20261028`, swofty's whole profile from the 2026-10-28 backup copy (exe d873a1c8, game 1.0.141.7865; its careers are not listed yet), then a Progressive 2018 career from an existing backup and captures from test runs: a My Team career at the contract window, a Driver Career at the season break, a Grand Prix weekend.
- **Building states instead of capturing them:** the Career Editor's API, plus new operations to advance the calendar, set race results and save.

## Rules

- The test agent and test DLLs load only when `swofty test` builds the profile and passes a token for that run. Normal Play never loads them.
- One game per machine. Tests queue on the existing game lock (`/mnt/work/F1/f1-game.lock.d` here), and `--take-over` closes a running game first.
- The CLI never accepts the offline play terms for the player: a run without them fails with exit code 2.
- Artefacts have size caps. Frames are optional.

## What ships in 1.6.0

- **The CLI:** `swofty test plan`, `run`, `shell` and `fixture capture|list|check`. Environments `none`, `boot` and `benchmark` run in game. `route` and `career` (`team_hq`, `weekend_hub`) are experimental and not working in 1.6.0; `grand_prix` and `time_trial` parse and are listed as needing session setup. The test file schema is in [Testing your addon](/std/testing#the-test-file).
- **The test agent** (`native/src/core/testagent`): the mailbox, `ai_drives_player`, `read`, `set`, `sample`, `pad`, `capture` and `skip` work. `route`, `career load`, `advance`, `save now` and `title` are experimental and not working in 1.6.0. Test DLLs written with `swofty/test.hpp` load only in a test run.
- **Title screen:** the CLI waits 15 seconds for the agent to pass it, then presses the virtual pad.
- **The save sandbox** (`native/src/core/save_sandbox.cpp`): runs in game. The Steam remote-storage proxy is fail-closed on any doubt. A profile load and three saves ran with the player's saves unchanged. Measured: the 0-entry snapshot case of the property-bag pool; the 64 KB and 256 KB cases are not measured. The fixture store is host-tested.
- **Snapshots:** the registry save default is dry in 1.6.0.
- **Checked only in the host tests** (no game run): the CLI's flow against a fake game and a fake agent, the run folder and token contract, the sandbox decision logic and the fixture check.

## In game after 1.6.1 (night 2, 2026-10-30, branch night2/ts)

Runs with the game parked on workspace 42, the fixture `swofty-profile-20261028` in the save sandbox, no human input:

- **Title and notice:** `[title] engaged ...` after the agent's pad presses (7 in the first run), then the notice answered (`{"home":true,"notice_presses":3}` in a `boot` batch) and `[test] home view model 0000000325C13EB0`.
- **Routes:** `route f1_world` from launch answered `{"from":"home"}` in 55 s; `route play` answered `{"from":"screen"}` from F1 World's view model.
- **Time Trial by pad:** right and a on the Play cards, a on F1 2026, McLaren and Australia, `wait session.start` (`[events] session.start (time_trial)`), a for Go to Track and Flying Lap; `ai_drives_player on` drove the car (307 km/h, `read car[0].gearbox.gear_index` = 8); `start`, `down`, `down`, `a` returned to the garage.
- **Career:** `career load newest` reached Team HQ in 58 s from launch: `route career`, `career_dlc_select_my_team`'s season pick, `[test] career: the career_dlc_content content tile's action fired route solo_career_continue_my_team`, `slot_focused and slot_pressed raised on card 0 (slot 2)`, `career.loaded: mode my_team`, `[test] career ready at team_hq`.
- **Save now:** `[test] save now: the sandbox served 2 FileWrite(s)`. The player's Steam and Documents save folders matched their sha256 backup after every run.
- **Not working:** `advance` (above), `capture` (`capture: not finished within 60 s`; a screenshot of the parked window works instead), `tile` on screens that aren't tile lists.

## Build order

1. **CLI only, now.** `swofty test plan` and `run` with the `boot` and `benchmark` environments: pass game arguments in a direct launch, stage the benchmark XML, and check logs, crashes, performance and that the game folder is clean afterwards. Most car-physics traces (T112 to T125) can run unattended this way.
2. **The test agent.** The mailbox, routes from the home screen, skip session, the AI driving the player's car, value reads and writes through `cars::live`, frame capture, the virtual pad and `swofty test shell`.
3. **Saves and careers.** The save sandbox, fixtures, continuing a career into its hub, calendar advance, race results and save now. Then `career` environments.
4. **Any screen and session.** Selection inside screens and the dev session start, for `grand_prix` and `time_trial` from any box, track and car.
5. **CI.** A machine that runs every addon's smoke tests before a launcher release, and publishes the reports.

The reverse engineering behind steps 3 and 4, in order of value: the save sandbox install point, continuing a career, what switches on the dev session, selection inside the Grand Prix and Time Trial screens, what ends a session after a skip, and the race results write.
