---
title: Sessions and weekends
description: Setup modes, weekend formats, session lengths, points, rules profiles, safety car and grid size in code, with the entrant, grid, result and penalty events.
---

# Sessions and weekends

With the session system you set what a race weekend looks like: its sessions, how the grid forms, the points, the rules and penalties, and the safety car. A 2009 season pack, for example, scores 10-8-6-5-4-3-2-1 with no point for the fastest lap, and F1 1990 runs two qualifying sessions and counts each driver's best 11 results.

A **setup mode** is the game's row that a weekend runs under: its format, length, rules and grid all hang off it. Every Grand Prix and Time Trial box shares one setup mode, `swofty_f1`, and each box's own points, DRS, tyre class and rules apply when a session loads in that box. A box gets a setup mode of its own only when it needs a different shape at boot, such as F1 1990's two qualifying sessions.

::: info In 1.6.0
The header `swofty/systems/sessions.hpp` has `sessions::Rules` (the `rules` of a `content::Package`), `sessions::set_control`, `sessions::disqualify`, the `session.player.control` payload `sessions::events::PlayerControl` and the `session.lap` payload `sessions::events::Lap`; `swofty/systems/control.hpp` has `Control` and `ControlState`. The registries on this page (`modes`, `formats`, `lengths`, `points`, `rules_profiles`, `safety_car`, `grids`) and the penalty, results and weekend events ship later; those sections describe that design.
:::

## The header

<Split>

`swofty/systems/sessions.hpp` has seven registries:

- `sessions::modes`: a setup mode, the row everything else hangs off (`session.mode`);
- `sessions::formats`: a weekend structure: sessions, groups, grid rules (`session.weekend_format`);
- `sessions::lengths`: practice, qualifying and race length options (`session.length`);
- `sessions::points`: a standing preset (`session.points`);
- `sessions::rules_profiles`: a rule manager profile and its penalties (`session.rules_profile`);
- `sessions::safety_car`: safety car and red flag frequencies (`session.safety_car`);
- `sessions::grids`: an entrant setup mode: how many cars and teams (`session.grid`).

`sessions::Rules` is not a registry. It's the value a box holds in `content::Package::rules`.

<template #example>
<Example root="swofty-sdk" code-file="include/swofty/systems/sessions.hpp">
<template #code>

```cpp
#pragma once
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/power.hpp>
#include <swofty/systems/tracks.hpp>

namespace swofty::tyres { struct Class; }

namespace swofty::sessions {

enum class Session : uint8_t {                 // the game's session id enum (0..18)
    practice_1 = 1, practice_2, practice_3,
    qualifying_1 = 5, qualifying_2, qualifying_3,
    qualifying_one_shot = 9,                          // one-shot qualifying
    sprint_shootout_1 = 10, sprint_shootout_2, sprint_shootout_3,
    sprint_shootout_one_shot,                         // one-shot sprint shootout
    race = 15, race_2, race_3,
    time_trial,
};

struct Points {
    std::vector<int> points;                   // per place, whole numbers; padded to the grid size
    std::optional<int> fastest_lap;            // fastest_lap_bonus; nullopt: none
    std::array<std::vector<int>, 3> reduced;   // reduced_points_data: races stopped at 75, 50 and 25 % distance
    StatId stat_id = StatId::automatic;        // a game_setup.session_standings row
};
inline Registry<Points> points{"session.points", { .applies = Applies::boot }};

struct GridRule;                               // how a race group's grid forms
GridRule grid_type(std::string_view game_type);                    // one of the game's grid types: "qualifying", ...
GridRule best_of(std::vector<Session> sessions);                   // each car's best time over several sessions

enum class GroupType { practice, qualifying, race };

struct Group {
    GroupType type;
    std::vector<Session> sessions;             // in order
    Text label;                                // the name the group shows; the core scopes it to this mode
    GridRule grid = grid_type("qualifying");   // race groups: how the grid forms
    Ref<Points> standings;                     // race groups: the points
};

struct Format {
    std::vector<Group> groups;                 // practice, qualifying and race groups of one weekend_structure
};
inline Registry<Format> formats{"session.weekend_format", { .applies = Applies::boot }};

struct Length {
    float laps_multiplier = 1.0f;              // race laps from the track's full distance
    int laps_override = 0;                     // fixed laps; 0: use the multiplier
    float tyre_wear_scale = 1.0f;
    int weather_slots = 0;                     // forecast slots across the session
    int flashback_limit = -1;                  // -1: unlimited
};
inline Registry<Length> lengths{"session.length", { .applies = Applies::boot }};

struct RulesProfile {
    bool blue_flags = true;
    int corner_cutting = 2;                    // 0 off .. 3 strict
    bool collisions = true;
    bool drive_through_replacement = false;    // a drive-through becomes a time penalty
    bool jump_starts = true;
    bool parc_ferme = true;
};
inline Registry<RulesProfile> rules_profiles{"session.rules_profile", { .applies = Applies::boot }};

struct SafetyCar {
    std::array<int, 8> max_deployments = { 0, 0, 3, 3, 2, 3, 5, 8 };   // per race length option
    std::array<int, 8> max_red_flags = { 0, 0, 1, 1, 1, 1, 2, 3 };
};
inline Registry<SafetyCar> safety_car{"session.safety_car", { .applies = Applies::boot }};

struct Grid {
    int cars = 22;                             // max_entrants; clamped to the install's grid size, set once at boot
    int teams = 11;                            // max_teams
    bool reuse_entrants = false;
};
inline Registry<Grid> grids{"session.grid", { .applies = Applies::boot }};

struct Mode {
    std::string branding;                      // a game_setup.branding row: logos, MFD, car setup
    Ref<Format> format;                        // weekend_options
    Ref<Length> race_length;
    Ref<RulesProfile> rules;
    Ref<SafetyCar> safety;
    Ref<Grid> grid;                            // entrant_setup_modes
    Ref<power::DrsRule> drs;                   // drs_allowed_lap_group
    bool pitstops = true;                      // enable_pitstops
    bool night_starts = false;                 // night_starts_enabled
    std::vector<Ref<tracks::Track>> restricted_tracks;
    StatId stat_id = StatId::automatic;        // a game_setup_modes row: 31 or lower (a u32 mask)
};
inline Registry<Mode> modes{"session.mode", { .applies = Applies::boot }};

struct Rules {                                 // a box's session rules (content::Package::rules), applied at session load
    Ref<Mode> mode;                            // empty: swofty_f1, the mode every GP/TT box shares
    Ref<Points> points;                        // empty: the mode's
    Ref<power::DrsRule> drs;                   // empty: the mode's
    Ref<tyres::Class> tyre_class;              // the box's class; a track entry (tracks::PackageTrack) overrides it
    Ref<RulesProfile> rules_profile;           // empty: the mode's
    bool refuelling = false;                   // in-race refuelling: every pit stop refills (content.package 1.5 applies it)
    std::optional<float> race_fuel_kg;         // the race start fuel with refuelling on; nullopt: the game's
    std::optional<bool> tyre_warmers;          // content.package 1.4: tyre warmers on or off; nullopt: the mode's
    std::optional<float> refuel_stint_kg;      // content.package 1.5: a pit stop fills to this; nullopt: race_fuel_kg, else full
    std::optional<float> quali_fuel_kg;        // content.package 1.5: qualifying start fuel; nullopt: the game's
    bool race_start_fuel_from_quali = false;   // content.package 1.5: parc ferme, race start fuel = fuel left after qualifying
    bool race_tyre_change = true;              // content.package 1.5: false keeps the tyres at race pit stops (damage excepted)
    std::optional<int> race_time_limit_s;      // content.package 1.5: race time limit in seconds; nullopt: the mode's
    std::string weekend_mode;                  // content.package 1.5: a setup mode whose weekend the box runs; empty: the mode's
    std::optional<std::string> safety_car;     // content.package 1.5: off, reduced, standard, increased; nullopt: the player's
    std::optional<bool> formation_lap;         // content.package 1.5: the formation lap option; nullopt: the player's
    std::optional<std::string> penalties;      // content.package 1.5: off, corner_cutting_only, esports, on; nullopt: the player's
    std::optional<std::string> corner_cutting; // content.package 1.5: regular, strict; nullopt: the player's
    std::vector<std::string> qualifying_tyres; // content.package 1.5: dry options ("softer", "medium", "harder") only for qualifying
};
SWOFTY_RECORD(Rules, mode, points, drs, tyre_class, rules_profile, refuelling, race_fuel_kg, tyre_warmers, refuel_stint_kg, quali_fuel_kg, race_start_fuel_from_quali, race_tyre_change, race_time_limit_s, weekend_mode, safety_car, formation_lap, penalties, corner_cutting, qualifying_tyres)

enum class PenaltyKind { warning, time, drive_through, stop_go, grid, disqualification };

namespace events {

struct Load {                                  // the session about to load (modify); payload on the Events page
    static constexpr const char *point = "session.load";
};

struct EntrantsSelect {                        // Sess_Select_Entrants
    static constexpr const char *point = "session.entrants.select";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    std::span<const Ref<drivers::Identity>> candidates;   // the pool
    uint32_t limit;                            // max_entrants
    std::vector<Ref<drivers::Identity>> *chosen;          // before handlers may fill it
};

struct GridBuild {                             // Game_GridSetup_SetupStartingGrid
    static constexpr const char *point = "session.grid.build";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    const GridRule *rule;                      // the race group's grid rule
    std::array<uint8_t, 32> *vehicle_id;       // the starting order; before handlers may write it
    uint8_t count;
};

struct GroupResult {                           // Sess_Build_GroupResult
    static constexpr const char *point = "session.grid.group_result";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    uint8_t group;                             // the session group whose result is built
    void *result;                              // GroupResult rows: car, best time, position
};

struct PenaltyIssue {                          // Sess_Issue_TimePenalty, _GridPlacePenalty, Sess_Apply_DecidedPenalties
    static constexpr const char *point = "session.penalty.issue";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    uint32_t car;
    PenaltyKind kind;                          // drive_through, stop_go, time, grid, ...
    float amount;                              // seconds or places; before handlers may change it; replace = veto
};

struct Pit {                                   // a car's pit status changes (notify)
    static constexpr const char *point = "session.pit";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    uint32_t car;                              // vehicle index in the session
    uint32_t previous, status;                 // the game's pit status numbers, 0 to 16
};

struct PitStop {                               // the game counts a pit stop for a car (notify)
    static constexpr const char *point = "session.pit_stop";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    uint32_t car;                              // vehicle index in the session
    uint32_t stops;                            // the car's stop count after it
};

// payloads on the Events page
struct WeekendStart    { static constexpr const char *point = "weekend.start"; };
struct ResultsFinalise { static constexpr const char *point = "session.results.finalise"; };
struct StateEnter      { static constexpr const char *point = "session.state.enter"; };
struct SafetyCarDeploy { static constexpr const char *point = "session.safety_car.deploy"; };

} // namespace events

Result set_grid(std::span<const uint8_t> order);                   // during session.grid.build
Result issue_penalty(uint32_t car, PenaltyKind kind, float amount);

} // namespace swofty::sessions
```

</template>
</Example>
</template>
</Split>

## F1 1990's weekend

<Split>

A 1990 weekend has two practice sessions, Friday qualifying, Saturday qualifying and the race. The grid comes from each car's better time over the two qualifying sessions. Points go 9-6-4-3-2-1, with no point for the fastest lap, and only a driver's best 11 results count.

F1 1990 has a setup mode of its own: its weekend has a different shape at boot. The mode is a variant of the 2026 mode and carries all of its 280 keys.

The grid follows the addon's `cars` option, 22 or 26. Grid size is one boot setting per install, and every box and mode is clamped to it, so a 26-car 1990 field needs an install set to 26 cars or more.

<template #example>
<Example root="swofty.f1-1990" code-file="native/src/rules.cpp" data-file="registry/rules.json" files="swofty.json, native/CMakeLists.txt">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/sessions.hpp>
#include <swofty/systems/career.hpp>

using namespace swofty;
using sessions::Session;
using sessions::GroupType;

SWOFTY_REGISTER
{
    auto points = sessions::points.add("f1_1990_race", {
        .points = { 9, 6, 4, 3, 2, 1 },
        .fastest_lap = std::nullopt,
        .reduced = { { { 4, 3, 2, 1, 1 } }, { { 4, 3, 2, 1, 1 } }, { { 4, 3, 2, 1, 1 } } },   // half points, rounded
    });

    auto weekend = sessions::formats.add("two_qualifying", { .groups = {
        { .type = GroupType::practice,   .sessions = { Session::practice_1, Session::practice_2 } },
        { .type = GroupType::qualifying, .sessions = { Session::sprint_shootout_one_shot }, .label = "lng_f1990_qualifying_1" },
        { .type = GroupType::qualifying, .sessions = { Session::qualifying_one_shot },      .label = "lng_f1990_qualifying_2" },
        { .type = GroupType::race,       .sessions = { Session::race },
          .grid = sessions::best_of({ Session::sprint_shootout_one_shot, Session::qualifying_one_shot }),
          .standings = points },
    } });

    auto grid = sessions::grids.variant("f1_1990", sessions::grids["game/f1_26"], [](sessions::Grid &g) {
        g.cars = option<int>("cars");                      // 22 or 26, clamped to the install; applies from the next game start
        g.teams = g.cars / 2;
    });

    sessions::modes.variant("f1_1990", sessions::modes["game/f1_26"], [=](sessions::Mode &m) {
        m.format = weekend;
        m.grid = grid;
    });
}

SWOFTY_ON_LOAD
{
    career::set_dropped_scores("f1_career_1990", 11);     // best 11 results for the drivers, every result for the teams
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "session.points": {
    "f1_1990_race": { "points": [9, 6, 4, 3, 2, 1], "fastest_lap": null,
                      "reduced": [[4, 3, 2, 1, 1], [4, 3, 2, 1, 1], [4, 3, 2, 1, 1]] }
  },
  "session.weekend_format": {
    "two_qualifying": { "groups": [
      { "type": "practice",   "sessions": ["practice_1", "practice_2"] },
      { "type": "qualifying", "sessions": ["sprint_shootout_one_shot"], "label": "lng_f1990_qualifying_1" },
      { "type": "qualifying", "sessions": ["qualifying_one_shot"],      "label": "lng_f1990_qualifying_2" },
      { "type": "race", "sessions": ["race"], "grid": { "best_of": ["sprint_shootout_one_shot", "qualifying_one_shot"] },
        "standings": "swofty.f1-1990/f1_1990_race" }
    ] }
  },
  "session.grid": {
    "f1_1990": { "extends": "game/f1_26", "cars": 26, "teams": 13 }
  },
  "session.mode": {
    "f1_1990": { "extends": "game/f1_26", "format": "swofty.f1-1990/two_qualifying", "grid": "swofty.f1-1990/f1_1990" }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: swofty.f1-1990 (f1_1990_rules.dll) 1 session.points, 1 session.weekend_format, 1 session.grid,
           1 session.mode (early stage, 0.5 ms)
[registry] stat ids: session.mode swofty.f1-1990/f1_1990 -> 16 (pool 5, 7, 16-31)
[registry] applied at boot: session.mode 1 (280 keys from game/f1_26), session.weekend_format 1, session.points 1 (padded to 26),
           session.grid 1 (26 entrants, option cars) (412 database records)
[events] session.grid.build in setup mode swofty.f1-1990/f1_1990: grid rule best_of(sprint_shootout_one_shot, qualifying_one_shot) by the core
[events] career.standings.rebuild in swofty.f1-1990/f1_career_1990: 1 after handler (swofty.f1-1990)
```

With the install set to 22 cars, the core clamps the 1990 grid and says so:

```text
[registry] session.grid swofty.f1-1990/f1_1990: 26 cars clamped to 22, the install's grid size
```

The data version has no dropped scores: those need the handler. A data file can't follow an option either. It states one grid size, or ships one file per value with a guard.

## Rules per box

A box sets its rules in `content::Package::rules`. They apply when a session loads in that box, and every other box in `swofty_f1` keeps its own.

```cpp
// a 2009 pack: 10-8-6-5-4-3-2-1, no fastest-lap point, no DRS
SWOFTY_REGISTER
{
    auto points = sessions::points.add("f1_2009", { .points = { 10, 8, 6, 5, 4, 3, 2, 1 }, .fastest_lap = std::nullopt });
    auto no_drs = power::drs_rules.add("none", { .race_start = 200, .red_flag_restart = 200, .safety_car_end = 200 });

    content::packages.layer(content::packages["community.f1-2009/f1_2009"], [=](content::Package &p) {
        p.rules.points = points;
        p.rules.drs = no_drs;
        p.rules.refuelling = true;
    });
}
```

```text
[events] session.load (grand_prix, community.f1-2009/f1_2009, race, game/silverstone): box rules applied by the core
         (points community.f1-2009/f1_2009, drs community.f1-2009/none, refuelling on)
```

Points per box ship in 1.9.61 (`session.points` 1.0, experimental). Each record becomes one `game_setup.session_standings` preset at boot, a copy of the stock `standard_race` with the record's points (padded with 0 to 26 places), its fastest lap bonus (the game's `BestOfTop10` rule; `null` for none) and the points of a race stopped at 75, 50 and 25 % distance (`reduced`; a missing level gives the full points). Pressing a box whose `rules.points` names a record points the race sessions (race, race_2, race_3) of `swofty_f1`'s non-sprint weekends at that preset; a box without the rule, a stock box or a career load puts the game's presets back:

```text
[points] box f1_1950 (community.f1-1950/f1_1950): swofty_f1 race points 8-6-4-3-2, fastest lap +1 (rules.points
         community.f1-1950/f1_1950, preset swofty_pts_f1_1950_1a2b3c4d) on 1 race session(s)
```

The game holds a session's preset as a pointer it resolves when it parses the mode, and its points recompute reads it on every refresh, so the swap takes effect from the next session. Sprint weekends keep the mode's own points.

A career freezes each season's championship rules when that season starts. A fix to a year's points reaches a running career at its next season.

### Fuel rules per box

F1 25 has no refuelling rule and its pit stop has no fuel service, so the core plays a box's fuel rules itself (`content.package` 1.5). Pressing the box hands its rules to a per-frame fuel plan; a stock box, or leaving the boxes, hands over none.

| Field | What the core does |
|---|---|
| `refuelling: true` | Each car starts the race on `race_fuel_kg` (when set), and every pit stop the game counts refuels it to `refuel_stint_kg`. |
| `race_fuel_kg` | With `refuelling` on: the race start fuel. Without it the game's own race fuel stays. |
| `refuel_stint_kg` | The fuel a stop fills to; empty: `race_fuel_kg`, else a full tank. A stop never drains a car that holds more. |
| `quali_fuel_kg` | The fuel each car starts a qualifying session with. |
| `race_start_fuel_from_quali` | Parc fermé (2003 to 2009): each car starts the race on the fuel it had left at the end of qualifying, `quali_fuel_kg` when it ran none. |
| `race_tyre_change: false` | 2005: one tyre set for qualifying and race. A race pit stop keeps the car's compound and wear; the stop still refuels. |

```cpp
content::packages.layer(content::packages["community.f1-2008/f1_2008"], [](content::Package &p) {
    p.rules.refuelling = true;
    p.rules.refuel_stint_kg = 60;
    p.rules.quali_fuel_kg = 45;
    p.rules.race_start_fuel_from_quali = true;
});

content::packages.layer(content::packages["community.f1-2005/f1_2005"], [](content::Package &p) {
    p.rules.refuelling = true;
    p.rules.race_tyre_change = false;                 // one set for qualifying and race
});
```

```text
[fuel] box f1_2008 (community.f1-2008/f1_2008): qualifying starts on 45.0 kg, race starts on the fuel left after qualifying, a pit stop refuels to 60.0 kg
[fuel] car 3: parc ferme: fuel left after qualifying, 110.0 -> 31.5 kg
[fuel] car 3: pit stop refuel, 6.2 -> 60.0 kg
```

The core writes the FuelTank's `fuel_mass` (the live car field `fuel_mass`, never above the tank's capacity). A start fuel is written again while the car has burned none of it, since the start sequence may load its own fuel after the session loads. The pit stop is the game's stop counter (`PitLaneCarState.pit_stops`), read from the live snapshot, so the fuel plan needs the live snapshot (the `telemetry` feature) and no hook. `race_tyre_change: false` is a wrap on `Car_Tyre_ChangeToNextCompound` (0x142e94040, the `PITLANE CHANGEOVER` log): its pit stop caller `Game_PitLane_ApplyPitstopTyres` calls it with flag 1 and ignores the result, and the wrap skips that call in a race session, so `pitstopComplete` and the rest of the stop run as before. The garage actor's call (flag 0) and every non-race session keep the game's change. A tyre record with a flag in `+0x208 & 0x1f`, which the function feeds into its per-wheel resets, keeps the change as tyre damage (a hypothesis; each skipped or allowed stop is logged with the flags). `SWOFTY_RACE_TYRE_RULE=0` turns the wrap off.

```text
[tyres] vehicle 7 pit stop in the race: tyres kept (box rule race_tyre_change false) (tyre record flags 0)
```

Experimental: tested on the host only. Known limits: the AI does not plan a stop for fuel, so a race fuel too small for the distance can run an AI car dry; the MFD's fuel readout and the garage refuel between qualifying runs are the game's own.

### Grid rules per box

A box's `rules.grid` (`content.package` 1.5) sets how its race grid forms from qualifying. The core applies it when the box is pressed, like the box's DRS and tyre warmer rules; a box without it, or a stock box, keeps the game's grid.

```json
"rules": { "grid": { "qualifying": "best_of", "rows": [3, 2], "rule_107": false } }
```

- `qualifying`: `best_lap` (the game's own order, the default), `best_of` (each driver's best lap over every qualifying-type session of the weekend: F1 1950 to 1995) or `aggregate` (the sum of each driver's best laps of the two fastest qualifying-type sessions: F1 2005). A weekend with one qualifying session ranks `aggregate` by the best lap, so a box that wants a real aggregate also gives the weekend two qualifying sessions (`rules.weekend_mode`, a weekend helper mode like F1 1990's). A driver with only one session's time follows every complete aggregate.
- `rule_107`: a driver slower than 107 % of the pole time starts from the back, behind every driver inside the limit, in the scoring's order; drivers without a time follow them. The game has no non-qualifier, and a car taken out at the start (a retirement or disqualification at the lights) would change how the race handles its entrants, so the back of the grid is the safe form.
- `rows`: cars per grid row, repeating from the front (`[3, 2]` is the 3-2-3 grid of the 1950s, `[4, 3]` the 4-3-4). Pattern row r stands on the track's stock grid row r (its two slots' centre line, so bends and slope follow the track), a 2-car row keeps the stock offsets and a wider row spreads its cars over 1.6 stock spacings, pole on the pole side. A slot the track's grid can't place keeps its own position. See the notes below.

How it works: the race grid ranks from the weekend's Qualifying group result (`Weekend+0x488`, a `std::map<u8 group, GroupResult>`; rows of 0x60 bytes at `+0x08..+0x10` with the group position at row `+0x50`, 0x1c unclassified; the latest session's per-driver best lap at `+0x28..+0x30`). The core wraps `Sess_Record_GroupTimes` (it keeps each qualifying-type session's times, which the next session replaces in the group result) and `Sess_Build_GroupResult`, and rewrites the Qualifying group's row positions through the game's own `Sess_Find_GroupRowPosition`. Sprint shootout times count only on a weekend without a sprint race. Practice sessions have no group times, so practice laps don't count. A save continued without pressing the box keeps the game's grid.

```text
[grid] box f1_1990 (mod.a/f1_1990): grid rule qualifying best_of, 107 % rule off
[grid] grid rules per box armed: a box's rules.grid re-ranks the weekend's qualifying classification
[grid] box f1_1990 (qualifying built): qualifying best_of, 107 % rule off over 2 session(s), 26 drivers; group positions rewritten
[grid]   driver 1/12 times [78.512 78.904] score 78.512 (ranked): position 3 -> 1
```

Grid rows, how the placement works: the engine's grid shape comes from the track's grid markup, one transform per slot. `Sess_VehiclePlacement_PlaceOnSlot` (0x142f343d0, `(placement, vehicle id, type, slot, tolerance)`, type 5 = grid) takes the slot's request from `placement+0x38+type*0x100+slot*8`, refreshes it with `Sess_VehiclePlacement_ResolveTransform` (0x142f62580) and drops the car at the request's transform (`+0x20`, a 4x4 matrix; position at `+0x50`, `+0x54`, `+0x58`, y up, dropped from y + 3 m; request `+0x18` slot, `+0x1c` type). The engine also has a procedural grid (`Game_Read_GridConfig` 0x142f62120: `Grid.CarsInRow`, `Grid.StaggerOffset` 2.0, `Grid.RowOffset` 10.0, `Grid.PoleLR`), but it is read only with the `VTE.enabled` developer setting. With a row pattern the core wraps both: the `PlaceOnSlot` wrap resolves the track's stock grid slots (the call the game makes for each slot anyway), computes the slot's place with `runtime::box_grid::row_position` and arms an override for that one request; the `ResolveTransform` wrap writes the position after the game's resolve, and the request's own position is put back once the car is placed, so the track's slots never change. `SWOFTY_BOX_GRID=0` turns grid rules off.

## Grid rules

`best_of` is a grid rule the core adds to `session.grid.build`. No addon has to write its own. For a rule the core doesn't have, replace the group result yourself:

```cpp
events::on<sessions::events::GroupResult>().in(events::setup_mode("swofty.f1-1990/f1_1990"))
    .replace([](sessions::events::GroupResult &g, const events::Context &) {
        return f1990::rank_reverse_top(g, 8);              // true: handled; the game's result isn't built
    });
```

A group result is built only when the group's last session id is 7, 8, 9, 12, 13 or 14 (mask 0x7380 in 0x140af83a0). A Q1/Q2 knockout can't be a format: it needs a core hook there.

## Lengths, rules and the safety car

```cpp
sessions::lengths.layer(sessions::lengths["game/race_50"], [](sessions::Length &l) { l.flashback_limit = 3; });

auto strict = sessions::rules_profiles.variant("strict", sessions::rules_profiles["game/f1_26"], [](sessions::RulesProfile &r) {
    r.corner_cutting = 3;
    r.drive_through_replacement = false;
});

sessions::safety_car.layer(sessions::safety_car["game/default"], [](sessions::SafetyCar &s) {
    s.max_deployments[7] = 4;                              // the longest race length: 4 safety cars, not 8
});
```

## Who drives a car

Each car has a set of controllers: a human one, an AI one, and flags such as the pit-lane flag. The game switches a car between them when the player retires, when a session is simulated and in the pit lane. After a flashback or a Return to Garage it rebuilds the set, so a pointer kept from before then points at freed memory. The core reads the set fresh every time and applies changes only between rebuilds.

```cpp
namespace swofty::sessions {

enum class Control : int32_t { human, ai };

struct ControlState {                          // read fresh each time; never keep it across frames
    Control mode;
    uint32_t flags;                            // the game's controller flags, bit n = flag n (PIT_LANE is bit 0)
    bool pit_lane() const;
};

enum class Result { ok, late, unavailable, invalid, blocked };

Result set_control(live::Car car, Control mode);   // applied at the next safe moment, never during a rebuild
Result disqualify(live::Car car);                  // Sess_Rules_Disqualify on the session's rules object

namespace events {

struct PlayerControl {                             // Game_Set_VehicleController: before, replace and after
    static constexpr const char *point = "session.player.control";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    cars::live::Car car;
    Control from, to;
    int32_t reason;                                // the controller flag the switch sets or clears (0 to 17; PIT_LANE is 0)
};

} // namespace events
} // namespace swofty::sessions
```

```cpp
// hand the player's car to the AI, then back
sessions::set_control(player, sessions::Control::ai);
sessions::set_control(player, sessions::Control::human);

// keep the player in control in the pit lane
events::on<sessions::events::PlayerControl>().replace([](sessions::events::PlayerControl &c, const events::Context &) {
    auto state = c.car.control();
    return c.to == sessions::Control::ai && state && state->pit_lane();   // true: the switch doesn't happen
});

// drop anything kept about a car's controllers when the game rebuilds them
events::on<cars::events::ControllersRebuilt>().after([](const cars::events::ControllersRebuilt &r, const events::Context &) {
    my_addon::forget(r.car);
});
```

- **`car.control()`** returns the mode and flags, or nothing while the car's controllers are being rebuilt.
- **`sessions::set_control`** works on any car and goes through the game's own switch, so `session.player.control` fires for it. It sets a controller flag of its own (DEV, 6; the config key `session_control_flag` picks another) to hold a car on the AI and clears it to give the car back. Handing a car back to its human fails with `Result::blocked` while any other flag is set (the pit-lane flag is the usual one) and the game is not called. A call made while the game rebuilds its controller sets, or from inside a `session.player.control` handler, is queued and applies on the next frame. `Result::late` means the controllers have not been built yet, or the call came from another thread. Handing a car to the AI resets the car's vehicle systems, which the player sees as a hitch.
- **`sessions::disqualify`** calls `Sess_Rules_Disqualify` (`void (void *rules, uint32_t vehicle)`, L2) for the car, which hands it to the AI with the DISQUALIFIED controller reason and sets the disqualified bit in its result flags. It needs session operations version 2. The core takes the `rules` object from `Sess_RuleManager_Initialise` (`rcx`, hooked from the window stage) and uses it while its vtable pointer is unchanged. `Result::late` means no rule manager has initialised yet, the car has no controller set or the call came from another thread; `Result::invalid` is an index with no controller set or above 254; `Result::unavailable` is a core without version 2 or a build where the binding does not resolve. A call made while the game rebuilds its controller sets or from inside a `session.player.control` handler is queued and applies on the next frame. The race ruleset requests DNF at the session end for every car short of the classification distance without checking for a disqualification, so a disqualified car can show as DNF in the results.
- **`reason`** is the flag index the game passes as the fourth argument of `Game_Set_VehicleController`: the switch sets bit `reason` when `to` is `ai` and clears it when `to` is `human`. A `before` handler may change `to` and `reason`.
- **`car.controllers.rebuilt`** fires after a flashback or a Return to Garage has rebuilt a car's controllers (see [Cars](./cars)).

## A car's lap

`session.lap` fires when a car crosses the line and its lap counter changes, once per crossing for each car of the session. The 1.x name `lap` fires at the same moment, with the car's index in `a`.

```cpp
namespace swofty::sessions::events {
struct Lap {                                   // notify
    static constexpr const char *point = "session.lap";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    cars::live::Car car;                       // car.index(): the vehicle index in the session
};
}
```

```cpp
// count the laps each car starts in qualifying
events::on<sessions::events::Lap>().when(events::session_type({ events::SessionType::qualifying }))
    .after([](const sessions::events::Lap &l, const events::Context &) { my_addon::lap_started(l.car.index()); });
```

- **Where it fires.** The game posts `vehicle_newlap` from its per-car progress update, and the ERS system's handler (`Game_ErsSystem_HandleVehicleNewLap`) looks the car up among its participants. The core's ERS new-lap guard sits on that lookup (`site_Game_ErsNewLap_VehicleLookup`) and fires the event at the participant whose vehicle id is the message's. A participant without a vehicle never fires it, and `SWOFTY_ERS_GUARD=0` turns the event off with the guard.
- **Thread.** The game thread, inside the progress update and the message publish. A handler that changes the session (sends a car to its garage, issues a penalty) queues the work and does it on a later frame.
- **What counts as a crossing.** The counter also changes when a car crosses the line backwards, and when the line runs through the pit lane a car crossing it there fires too. Read the car's pit status when the difference matters.
- **Cost.** The core logs the first fire of a session only.

## Events and operations

```cpp
// a 20-second time penalty instead of every drive-through
events::on<sessions::events::PenaltyIssue>().when(events::session_type({ events::SessionType::race }))
    .before([](sessions::events::PenaltyIssue &p, const events::Context &) {
        if (p.kind == sessions::PenaltyKind::drive_through) {
            p.kind = sessions::PenaltyKind::time;
            p.amount = 20.0f;
        }
    });

// a reverse grid for a sprint, from inside the grid build
events::on<sessions::events::GridBuild>().in(events::content("mod.reverse/sprint")).before(
    [](sessions::events::GridBuild &g, const events::Context &) {
        std::reverse(g.vehicle_id->begin(), g.vehicle_id->begin() + std::min<int>(g.count, 10));
    });

sessions::issue_penalty(car, sessions::PenaltyKind::grid, 3.0f);
```

## Readiness and limits

- **Ready:** the entrant, grid, result and safety car handlers are L2, used by Custom Grid Size and F1 1990.
- **Not in the headers yet:** `issue_penalty` and `PenaltyKind` above are the design. To retire or disqualify a car today, see [Failures, damage and retiring](/std/systems/failures), which also covers what **Restart Session** fires.
- **Partial:** points and penalties. The data is ready, and the runtime code is L1. A box's points apply through the standings lookup, which still needs reverse engineering; until then they come from the setup mode.
- **A box's grid rule (`rules.grid`): offline only.** Scoring and the 107 % rule re-rank the Qualifying group result (L2 bindings, the path F1 1990's best-of grid used); not yet run in game in this form. Row patterns move the grid slots through the placement pair below (L2 experimental bindings); not yet run in game.
- **A box's DRS rule (`rules.drs`) applies from 1.6.4:** the box press points `swofty_f1`'s DRS group at the rule's group. See [DRS per box](./power-units#drs-per-box).
- **Unknown:** flags and red flags, the knockout cut writer, red-flag restarts, penalty decision thresholds at run time, the difficulty preset selector.
- **Who drives a car: implemented, waiting on verification.** `Game_Set_VehicleController` (the fourth argument is a flag index, not a free reason code) and the controller set (human controller at +0x20, AI controller at +0x28, flags at +0x30, mode = dword[+0x10] ^ dword[+0x18]) are L2. The core binds them after the independent verify, and until then `set_control` and `car.control()` report unavailable. Whether a flashback or Return to Garage rebuilds the sets is test T144; `car.controllers.rebuilt` fires after every build of the sets (`Sess_ControllerSystem_BuildSets`, L2), and needs the T144 in-game run.
- **Grid:** 32 vehicle slots, a 27-car clamp at session enter, an AI table of 30, a wheel map that rehashes at the 26th car, replay holders sized for 22 to 24 cars. `sessions::grids` refuses a size the game can't hold, naming the limit.
- **Grid size is one boot setting per install.** The track worlds that hold the grid are picked once at boot for every box, and each box and mode is clamped to that size.
- **Points** are whole numbers in arrays of 23, padded to the grid size. 1990's half points are rounded.
- **Setup modes** take stat ids of 31 or lower. 18 are free (5, 7, 16 to 31), and F1 1990 uses 16. A mode needs the content set and an available mode row.
- **Option restrictions** are tags shared across modes (`gamesetup_f1`).
- **Pit lane bindings: experimental, no std operation yet.** `swofty/game.hpp` has `Game_Pitlane_SetVehicleStatus(pit_lane, car, status)`, which sets a car's pit status (0 to 16: 1 OnTrack, 4 InPitBox, 9 InGarage, 13 LeavingGarage), and `Game_PitLane_PutVehicleInPitBox(pit_lane, car)`, which puts a car in its pit box from any status the way the red flag does for every car. Both take the session's `PitLaneSystem`, the first argument of every pit status change. `Sess_PitStopUpdateJob_Update` (the per-frame pit stop update, which scales the crew sequence by the stop's rate) and `g_Ne_NameTable` (the engine's interned names, where a sequence's name hash resolves) are there too. F1 1990 uses them for its qualifying lap limit and its pit stop phases.

```cpp
#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

void *pit_lane;

SWOFTY_WRAP(Game_Pitlane_SetVehicleStatus, void, void *self, uint8_t car, uint32_t status)
{
    pit_lane = self;                           // the session's PitLaneSystem
    orig(self, car, status);
}

// later, on the game thread: send car 5 back to its pit box
swofty::game::Game_PitLane_PutVehicleInPitBox(pit_lane, 5);
```
