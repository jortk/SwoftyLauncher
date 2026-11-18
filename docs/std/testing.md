---
title: Testing your addon
description: Write tests that start the game in a known state, such as a race at a chosen track, a career at the contract window or a given menu, run steps, check what happened and report, with one command and no clicking.
---

# Testing your addon

`swofty test` starts F1 25 in the state your test needs, runs your steps, checks the results and writes a report. A test never clicks through menus or plays through races to get somewhere: it names where it starts, and the launcher and the core take it there.

```sh
swofty test run ./my-addon            # every test the addon ships
swofty test run ./my-addon -t smoke   # only the tests tagged smoke
swofty test plan ./my-addon           # what would run, and how many game starts it takes
```

```text
swofty test: 6 tests in 3 game starts
  benchmark bahrain 3 laps .......... tyre_temps_rise_on_track    pass   (1 m 52 s)
  career fixtures/my-team-r3 ........ rnd_tab_hidden_in_team_hq   pass
                                      rnd_upgrade_applies          FAIL  expected engine.max_torque > 493, got 493
  route laptop_rnd .................. rnd_page_lists_departments  pass
5 passed, 1 failed  ·  report: swofty-test/20261028-1402/report.json, junit.xml
```

## A test

A test lives in your addon's `tests/` folder. The test file says where each test starts, what it does and what must be true afterwards.

<Split>

Each test has:

- **An environment**: where the game starts. A race with AI, a career from a saved fixture, a menu, or just the main menu.
- **Steps**: what happens once it's there. Wait for an event, read values, open a menu, skip a session.
- **Checks**: what must be true. A log line, a value, no crash, the frame rate.

Tests with the same environment share one game start, so a suite of twenty career tests starts the game once.

<template #example>
<Example root="swofty.rnd-tyres-thermal" data-file="tests/thermal.test.json" files="swofty.json, tests/native/tests.cpp, tests/native/tests.dll, tests/fixtures/my-team-r3/fixture.json">
<template #data>

```json
{
  "schema": 1,
  "suite": "swofty.rnd-tyres-thermal",
  "tests": [
    {
      "id": "tyre_temps_rise_on_track",
      "tags": ["smoke"],
      "env": { "kind": "benchmark", "track": "bahrain", "laps": 3, "cars": 10 },
      "steps": [
        { "wait": { "event": "race.start" }, "timeout_s": 90 },
        { "sample": { "read": "car[0].tyres[fl].temperature", "every_ms": 500, "for_s": 60, "as": "fl" } }
      ],
      "assert": [
        { "series": "fl", "rises_by": 8 },
        { "log": "\\[rnd-tyres\\] thermal model on" },
        { "no_crash": true }
      ]
    },
    {
      "id": "rnd_tab_hidden_in_team_hq",
      "env": { "kind": "career", "fixture": "fixtures/my-team-r3", "at": "team_hq" },
      "fn": "rnd_tab_hidden"
    }
  ]
}
```

</template>
</Example>
</template>
</Split>

## Environments

| `kind` | Starts at | Use it for |
|---|---|---|
| `boot` | the main menu | menus, tiles, addon settings, anything at startup |
| `benchmark` | a race with AI cars, straight from launch | physics, AI, sounds, tyres, anything on track |
| `route` | a menu, opened by its route | a screen your addon adds or changes |
| `career` | a saved career, loaded into Team HQ, a weekend or a session | careers: contracts, R&D, seasons, calendars |
| `session` | a Time Trial or Grand Prix session, started from the main menu | year packs, tracks, rules per box |
| `grand_prix`, `time_trial` | planned: the same, one kind per mode | |

```json
{ "kind": "benchmark", "track": "monaco", "laps": 2, "cars": 20, "weather": "rain", "box": "swofty.classic-seasons-2017/f1_2017" }
{ "kind": "route", "routes": ["career", "team_hq", "contracts"] }
{ "kind": "career", "fixture": "fixtures/progressive-2018-r10", "at": "weekend_hub" }
{ "kind": "session", "mode": "time_trial", "box": "mod.season-2008/f1_2008", "track": "albert_park" }
{ "kind": "session", "mode": "grand_prix", "box": "swofty.classic-seasons-2017/f1_2017", "track": "melbourne", "session": "qualifying", "laps": 5 }
```

A `route` environment fires its routes in order, each from the screen the game is on or from the home screen: `["f1_world", "play"]` opens F1 World and then its Play screen. A `session` environment asks the agent to start the session and waits for the load (below). Until the agent can, a `route` test reaches a Time Trial or Grand Prix with `pad` steps.

In a `benchmark` race the AI drives every car, including the player's. For a test that needs the player's controls, such as the MFD or DRS, a step drives the car with the virtual pad.

## Steps

```json
{ "wait": { "event": "career.season.start" }, "timeout_s": 120 }
{ "route": "contracts" }
{ "skip": "session" }
{ "advance": { "days": 7 } }
{ "ai_drives_player": true }
{ "pad": { "press": "mfd_fuel", "then": "right" } }
{ "set": { "car[player].f1_aero.lift_scale": 1.05 } }
{ "read": "career.money", "as": "money_before" }
{ "capture": "frame", "name": "team_hq" }
{ "save": "now" }
{ "home": true }
{ "tile": "career_dlc_content" }
{ "wait": { "screen": "f1_world_activity_container" }, "timeout_s": 30 }
```

`home` waits for the main menu, answering the launcher's notice on the way. `tile` runs a tile on the screen the game is on, by its id, as selecting it does: its route fires. Tile screens are the home screen, F1 World, Career and the career season pick; their tile ids are in `loader.log` (`[test] tile screen ...: story, career, mod_menu, f1_world, ...`). Other screens (F1 World's Play cards, Time Trial's season row, team select, track select, the garage, the pause menu) take `pad` steps. `wait` with `screen` waits until that screen is open; each screen that opens is logged as `[test] screen <name>`, so a run's `loader.log` lists the names to wait for.

```json
{
  "id": "time_trial_session",
  "env": { "kind": "route", "routes": ["f1_world", "play"] },
  "steps": [
    { "pad": { "press": "right" } },
    { "pad": { "press": "a", "ms": 250 } },
    { "pad": { "press": "a", "ms": 250 } },
    { "pad": { "press": "a", "ms": 250 } },
    { "pad": { "press": "a", "ms": 250 } },
    { "wait": { "event": "session.start" }, "timeout_s": 150 },
    { "pad": { "press": "a", "ms": 250 } },
    { "pad": { "press": "a", "ms": 250 } },
    { "ai_drives_player": true },
    { "sample": { "read": "car[player].gearbox.gear_index", "every_ms": 500, "for_s": 20, "as": "gear" } }
  ],
  "assert": [{ "series": "gear", "rises_by": 3 }, { "no_crash": true }]
}
```

That is Play, Time Trial, the first season, the first team, the first track, then Go to Track and Flying Lap in the garage, with the AI driving the player's car. Pause and Return to Garage are `start`, `down`, `down`, `a`.

A `pad` press waits until the screen has held still for 1.2 s, so it lands once the screen takes input. A `wait` for a screen does the same for 1 s. Both take a `settle_ms` to change that, `wait` takes `ms` to sleep, and `pad` takes `retry` to press again when the screen did not change:

```json
{ "wait": { "ms": 3000 } }
{ "wait": { "screen": "track_select", "settle_ms": 1500 }, "timeout_s": 60 }
{ "pad": { "press": "a", "ms": 700, "settle_ms": 2000, "retry": 3, "retry_after_ms": 4000 } }
```

With `retry`, the step passes once the screen's name changes, presses again after `retry_after_ms` while it has not, and fails after the last press.

`skip` and `advance` use the game's own simulate and calendar code, so a test can get from the first race of a season to the season break in seconds. `read` and `set` use the same names as `cars::live` and the system pages.

## Checks

```json
{ "log": "\\[career\\] season 2019 starts", "min": 1 }
{ "value": "car[0].chassis.mass", "between": [790, 830] }
{ "value": "money_after", "greater_than": "money_before" }
{ "event": "car.reset", "count": 2 }
{ "no_crash": true }
{ "perf": { "p99_ms": { "max": 40 } } }
```

## The test file

A file named `*.test.json` in the addon's `tests/` folder holds one suite. Every key is checked when the file is read, and an unknown key is an error naming it.

```json
{
  "schema": 1,
  "suite": "swofty.rnd-tyres-thermal",
  "defaults": {
    "timeout_s": 300,
    "addons": ["swofty.rnd-tyres-thermal", "swofty.core"],
    "settings": { "swofty.rnd-tyres-thermal": { "thermal_model": true } }
  },
  "tests": [
    {
      "id": "thermal_off_by_default",
      "tags": ["smoke"],
      "fresh": true,
      "env": { "kind": "boot" },
      "settings": { "swofty.rnd-tyres-thermal": { "thermal_model": false } },
      "timeout_s": 60,
      "steps": [{ "wait": { "log": "\\[rnd-tyres\\] thermal model off" }, "timeout_s": 30 }],
      "assert": [{ "no_crash": true }]
    }
  ]
}
```

- **`defaults`:** `timeout_s`, `addons` and `settings` for every test in the file. A test's own `settings` add to them.
- **`addons`:** the addons the test's profile switches on. The default is the addon whose folder holds the file.
- **`settings`:** addon settings (`addon.option` values) set for the test's profile.
- **`fresh`:** the test gets its own game start instead of sharing one with tests of the same environment.
- **`fn`:** the name of a `SWOFTY_TEST` function in the addon's test DLL, in place of `steps` and `assert`.

The environments take these keys:

```json
{ "kind": "none" }
{ "kind": "boot" }
{ "kind": "benchmark", "track": "bahrain", "laps": 3, "cars": 10, "weather": "dry", "box": "...", "grid_pos": 1, "camera": "...", "driver": "..." }
{ "kind": "route", "routes": ["career", "team_hq"] }
{ "kind": "career", "fixture": "fixtures/my-team-r3", "at": "team_hq", "slot": 1 }
{ "kind": "session", "mode": "grand_prix", "box": "...", "track": "...", "team": "...", "driver": "...", "session": "race", "weather": "clear", "laps": 5, "ai_difficulty": 90, "fixture": "..." }
```

`none` starts the game and is ready at the loader's attached line, before any menu. `boot` and `route` are ready at the main menu. `benchmark` needs `track`, `route` needs `routes`, `career` needs `fixture`, `session` needs `mode`.

## Sessions

```json
{ "kind": "session", "mode": "grand_prix", "box": "swofty.classic-seasons-2017/f1_2017", "track": "melbourne", "session": "qualifying_1" }
```

```text
> session time_trial cs_f1_2017 melbourne
> session grand_prix swofty.classic-seasons-2017/f1_2017 melbourne session=race laps=5 weather=wet ai_difficulty=90
```

`mode` is `time_trial` or `grand_prix`. `session` is `practice1` to `practice3`, `qualifying`, `sprint_shootout`, `race`, `race_2`, `time_trial`, or one of the game's own names (`practice_1`, `qualifying_o`, `sprint_shootout_s`, and so on); a Time Trial takes only `time_trial`, a Grand Prix anything else, `race` by default. `laps` is 1 to 100 and only for a Grand Prix, `ai_difficulty` 0 to 110, `weather` `clear`, `wet` or `rain`. With `fixture` the run is sandboxed like a `career` environment.

The agent runs `session.start` from the main menu: it passes the title, checks the track against the track base and the box (a track the box can't run is refused with the guard's reason), fills the game's dev session settings and waits for the flow to enter `StateLoadSession`, then answers `{"event":"session.start","mode":...,"track":...,"session":...}` and logs `[test] session.start <mode> <track>`. A command it can't carry out answers `unavailable: session.start: needs <row>` naming the first KB row the build lacks.

## Tests in code

A test can also be a C++ function in a test DLL your addon ships next to the test file. It runs inside the game, a step at a time, without blocking it.

```cpp
#include <swofty/test.hpp>
#include <swofty/systems/career.hpp>

SWOFTY_TEST(rnd_tab_hidden)(swofty::test::Context &t) -> swofty::test::Task
{
    co_await t.at("team_hq", 60);
    t.expect(!t.read<bool>("ui.team_hq.tabs.rnd.visible"), "the R&D tab is hidden");
    co_await t.route("laptop_rnd");
    co_await t.log("\\[rnd-tyres\\] blocked", 5);
    co_await t.capture("team_hq");
}

SWOFTY_TESTS()
```

The test DLL loads only during `swofty test`, never in normal play.

## Fixtures

A fixture is a saved game in a known state: a career after ten races, a My Team at the contract window, a Grand Prix weekend. Capture one from a running test session:

```sh
swofty test shell --fixture swofty-profile-20261028   # start the game in test mode, in the save sandbox
> career load 1                         # or play to the state you want
> save now
> quit
swofty test fixture capture progressive-2018-r10 --run <the run the shell named>
```

```text
fixtures/progressive-2018-r10/
  fixture.json        the game build, the addons and versions it needs, what it holds
  save/               the game's save files
  storage/            addon storage that lives outside the save
```

Outside the shell, `swofty test fixture capture <name>` captures the last test run's saves, and `--from <folder>` a copy of a save folder. `swofty test fixture list` and `swofty test fixture check <name>` check a fixture's files against its `fixture.json`. A test names a fixture by a folder next to its test file, or by name in your fixture store (`SWOFTY_TEST_FIXTURES`, else `test-fixtures` in the launcher's data folder).

```json
{ "kind": "career", "fixture": "fixtures/progressive-2018-r10", "at": "team_hq", "slot": 2 }
```

`at` is `team_hq` (the default), `weekend_hub` or `session`. `slot` picks the career by its save slot when the fixture holds more than one; without it the newest is continued. `session` isn't available.

`career load`, `advance` and `save now` run only in the save sandbox: in a `career` environment, or in `swofty test shell --fixture <name>`. Without a fixture the shell runs on your own saves, so those commands are refused. `career load` passes the title and the notice, opens Career, runs the mode's tile, picks the season whose tile continues a saved career and presses the resume card: from launch to Team HQ takes about a minute. `advance` doesn't work yet (below); a step can press Advance with the pad instead.

Tests always run on a copy of a fixture in a separate save folder. They never read or write your own saves, and nothing they save reaches Steam Cloud: if the launcher can't make sure of that for your game's build, the test doesn't start.

## What runs today

- **Runs in game:** `none`, `boot`, `benchmark`, `route` and `career` at `team_hq`. The agent passes the title screen and the launcher's notice by pressing the virtual pad itself, so a `boot` run ends on the main menu with no clicking, parked on another workspace. `route` fires routes from the home screen and from F1 World and Career; `tile` runs tiles on those screens; `career load` reaches Team HQ from a fixture; `save now` saves into the sandbox; `ai_drives_player` drives the player's car on track; `pad` drives every other screen, including starting a Time Trial session and returning to the garage.
- **Save sandbox:** runs. Loading a fixture profile and saving three times in the sandbox leaves the player's saves unchanged.
- **Not working yet:** `advance` (the agent never sees Team HQ's advance panel update), `capture` (no frame within its time limit), the `career` environment's `weekend_hub` and `session`, and `title` alone without the pad.
- **Planned, not runnable:** `session` answers `unavailable` until its KB rows are verified and the call that starts the load is found; `grand_prix` and `time_trial` environments. `swofty test plan` lists those two as "needs session setup inside the game's screens".
- **Steps and checks the launcher reads itself** (`wait`, and the `log`, `event`, `no_crash` and `perf` checks) work from the session's `loader.log`. Steps that read or write the running game (`read`, `sample`, `set`, `pad`, `capture`, and the `value` and `series` checks) go through the test agent in the game.

## Running tests

- One game runs at a time on a machine, so tests queue for it. `swofty test run` waits up to 30 minutes for the game to be free (`--wait 0` doesn't wait), and with `--take-over` it closes a running game first.
- Every run uses its own launcher profile with only the addons the tests name, and puts the game folder back afterwards.
- The report is JSON and JUnit, with logs, frames and the values sampled. The exit code is 0 when everything passed, 1 when a test failed, 2 when the game couldn't be started and 3 when the game was still busy after the wait.
