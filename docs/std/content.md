---
title: Seasons and GP/TT content
description: A whole Grand Prix and Time Trial season in code. Content packages and era boxes as typed registries, other addons building on a season in place or as a variant with its own box, each box's tracks and rules, stock files a year pack keeps, and the boxes a career's seasons race in.
---

# Seasons and GP/TT content

This page shows how a Grand Prix or Time Trial season is built, and how other addons build on it. A **box** is one of the year tiles a player picks in Grand Prix or Time Trial. In code, a box is a `content::EraBox` that opens a `content::Package`: the season's entrant list, with its teams, cars and drivers. Every piece is a record another addon can build on.

Classic Seasons 2017, for example, registers its season, its ten cars and its box. An engine sound pack then gives three of those cars a V10, and copies none of Classic Seasons' files.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/content.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::content {

struct Seat {
    Ref<drivers::Identity> driver;                 // the driver
    int number = 0;                                // the car number this season
    Ref<drivers::Look> look;                       // the look this season (driver_customisation)
};

struct Team {
    std::string key;                               // the team's key in the package: "ferrari"
    Ref<teams::Identity> team;                     // the team record
    Ref<cars::Vehicle> vehicle;                    // its car
    Ref<sounds::EngineSound> engine_sound;         // the car's sound; empty: the vehicle's. Applied at boot, to that car
    Ref<liveries::Livery> livery;                  // the car's livery in this package; empty: the vehicle's
    std::vector<Seat> drivers;                     // its seats
};

struct Package {                                   // a GP/TT season build: an entrant list and its rows
    Text title;                                    // the season's name: a lang key ("lng_cs_f1_2017")
    std::string entrant_list;                      // "cs_f1_2017"
    int season = 0;                                // the year
    Ref<career::Calendar> calendar;                // the season's real calendar: first in the box's track list
    Keyed<Team, &Team::key> teams;                 // the grid, matched by key: p.teams.at("ferrari")
    sessions::Rules rules;                         // the box's points, DRS, tyre classes and rules, applied at session load
    std::string setup_mode = "swofty_f1";          // shared by every box; your own only for a different shape at boot
    std::string leaderboard = "box";               // since 1.2: the list's Time Trial leaderboard (see Leaderboards)
    bool virtual_list = false;                     // since 1.3: the list takes no stat id and no setup mode slot (see Virtual lists)
    StatId stat_id = StatId::automatic;            // the entrant list's id
};

struct TimeTrial {
    std::string entrant_list;                      // the list the box selects in Time Trial (default: the package's)
};

struct EraBox {                                    // an entry in the GP/TT era picker
    Ref<Package> package;                          // the package it opens
    Text title;                                    // its title (default: the package's)
    std::string image;                             // "#overrides.home_screen.tiles_solo_6x6[cs_f1_2017]"
    int order = 0;                                 // its place in the Addons row
    TimeTrial time_trial;                          // since 1.1: its Time Trial list
    std::optional<EraOnly> only;                   // since 1.2: grand_prix or time_trial keeps it off the other picker
};

inline Registry<Package> packages{"content.package", { .applies = Applies::content }};   // its entrant list rows are added at boot
inline Registry<EraBox> era_boxes{"era.box", { .applies = Applies::read }};   // when the picker is built

}
```

</template>
</Example>

## Classic Seasons 2017, in code

<Split>

Classic Seasons 2017 registers its season as records, and the core applies them at boot.

- **The era box** is what the player picks. It points at a package.
- **The package** is the season build: the entrant list, the teams, their cars and drivers.
- **The vehicle** is one car, with its sound and livery ([Cars](/std/systems/cars)).
- **The engine sound** is a bundle, plus where its mixing settings and tuning come from ([Sounds](/std/systems/sounds)).

<template #example>
<Example root="swofty.classic-seasons-2017" code-file="native/src/content.cpp" data-file="registry/content.json" files="swofty.json, native/src/cars.cpp, native/src/calendar.cpp, sounds/cs17_ferrari.bdl, sounds/cs17_mercedes.bdl, sounds/cs17_honda.bdl, sounds/cs17_renault.bdl, lang/season.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/career.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/liveries.hpp>
#include <swofty/systems/sounds.hpp>
#include <swofty/systems/teams.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto ferrari_sound = sounds::engine_sounds.add("ferrari", {
        .bundle = file("sounds/cs17_ferrari.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
        .tuning_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    auto ferrari = cars::vehicles.variant("ferrari", cars::vehicles["game/ferrari_2025"], [=](cars::Vehicle &v) {
        v.label = "cs17_ferrari";
        v.engine_sound = ferrari_sound;
        v.livery = liveries::liveries["swofty.classic-seasons-2017/ferrari"];
    });

    auto season = content::packages.add("f1_2017", {
        .title = "lng_cs_f1_2017",
        .entrant_list = "cs_f1_2017",
        .season = 2017,
        .calendar = career::calendars["swofty.classic-seasons-2017/f1_2017"].optional(),   // the real 2017 calendar (calendar.cpp)
        .teams = {
            { .key = "ferrari", .team = teams::identities["swofty.classic-seasons-2017/ferrari"], .vehicle = ferrari,
              .drivers = { { .driver = drivers::identities["game/sebastian_vettel"], .number = 5 },
                           { .driver = drivers::identities["game/kimi_raikkonen"],   .number = 7 } } },
            // mercedes, redbull, force_india, williams, mclaren, toro_rosso, haas, lotus, sauber
        },
    });

    content::era_boxes.add("f1_2017", {
        .package = season,
        .image = "#overrides.home_screen.tiles_solo_6x6[cs_f1_2017]",
        .order = 2017,
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/content.json
{
  "engine_sound": {
    "ferrari": { "bundle": "sounds/cs17_ferrari.bdl", "settings_from": "game/ferrari_2025", "tuning_from": "game/ferrari_2025" }
  },
  "vehicle": {
    "ferrari": { "extends": "game/ferrari_2025", "label": "cs17_ferrari",
                 "engine_sound": "swofty.classic-seasons-2017/ferrari", "livery": "swofty.classic-seasons-2017/ferrari" }
  },
  "content.package": {
    "f1_2017": {
      "title": "lng_cs_f1_2017", "entrant_list": "cs_f1_2017", "season": 2017,
      "calendar": "swofty.classic-seasons-2017/f1_2017?",
      "teams": [
        { "key": "ferrari", "team": "swofty.classic-seasons-2017/ferrari", "vehicle": "swofty.classic-seasons-2017/ferrari",
          "drivers": [ { "driver": "game/sebastian_vettel", "number": 5 }, { "driver": "game/kimi_raikkonen", "number": 7 } ] }
      ]
    }
  },
  "era.box": {
    "f1_2017": { "package": "swofty.classic-seasons-2017/f1_2017",
                 "image": "#overrides.home_screen.tiles_solo_6x6[cs_f1_2017]", "order": 2017 }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: swofty.classic-seasons-2017 (classic_seasons_2017.dll) 4 engine_sound, 10 vehicle, 10 vehicle.package,
           10 team.identity, 1 content.package, 1 era.box (early stage, 1.8 ms)
[registry] applied at boot: engine_sound 4 (4 bundle paths redirected to swofty.classic-seasons-2017), vehicle 10,
           vehicle.package 10, team.identity 10, content.package 1 (entrant list cs_f1_2017, 20 entrants; 1,912 database records)
[era]      era.box swofty.classic-seasons-2017/f1_2017: Addons row, order 2017
```

## The V10 pack builds on it, in place

<Split>

An engine sound pack brings its own sound under its own key, then gives it to three 2017 teams with a layer on the 2017 package. In place, every user of the 2017 package hears it: the 2017 GP/TT box, Progressive Career's 2017 season, anything else that races those cars.

Classic Seasons doesn't know the pack exists, and the pack copies none of Classic Seasons' files.

A car's sound is a database row the game reads at boot. The core applies a team's `engine_sound` at boot, to the car that team drives, and the car has that sound wherever it races. Whether the game reads sound rows again later needs reverse engineering.

<template #example>
<Example root="mod.esp" code-file="native/src/esp.cpp" data-file="registry/layers.json" files="swofty.json, sounds/v10_screamer.bdl, registry/sounds.json">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/sounds.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", {
        .bundle = file("sounds/v10_screamer.bdl"),
        .settings_from = sounds::engine_sounds["game/ferrari_2025"],
        .tuning_from = sounds::engine_sounds["game/ferrari_2025"],
    });

    content::packages.layer(content::packages["swofty.classic-seasons-2017/f1_2017"], [=](content::Package &p) {
        for (const char *team : { "mercedes", "ferrari", "redbull" })
            p.teams.at(team).engine_sound = v10;
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/layers.json (the engine sound itself is in registry/sounds.json)
{
  "content.package": {
    "swofty.classic-seasons-2017/f1_2017": {
      "teams": [
        { "key": "mercedes", "engine_sound": "mod.esp/v10_screamer" },
        { "key": "ferrari",  "engine_sound": "mod.esp/v10_screamer" },
        { "key": "redbull",  "engine_sound": "mod.esp/v10_screamer" }
      ]
    }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: mod.esp (esp.dll) 1 engine_sound, 1 layer on content.package (early stage, 0.4 ms)
[registry] content.package swofty.classic-seasons-2017/f1_2017: edit by mod.esp changed /teams/key=ferrari/engine_sound,
           /teams/key=mercedes/engine_sound, /teams/key=redbull/engine_sound -> mod.esp/v10_screamer
[registry] applied at boot: engine_sound 1 (1 bundle path redirected to mod.esp); content.package
           swofty.classic-seasons-2017/f1_2017 team sounds: 3 cars (cs17_mercedes, cs17_ferrari, cs17_redbull)
```

## Or as a variant, with its own box

<Split>

The pack can instead add a separate "F1 2017 (V10)" entry and leave the real one alone. The variant package starts from the 2017 package. Its three teams drive their own vehicle variants with the V10, and the 2017 box keeps its cars and their sounds. Its era box sits next to the original.

Every team, seat and field the variant doesn't change comes from the base, and later layers on the base flow in. The variant inherits the base package's [calendar and track data](#a-track-is-in-every-year).

The variant's era box shows like the base's box. A box that sets no `image` uses the tile art of the nearest base package's box. A literal `title` is shown as written. A variant vehicle that gets its own label keeps its base car's team row and side rows (names, colours, pit times), so its team shows under the base's name and stays a separate team in team select.

<template #example>
<Example root="mod.esp" code-file="native/src/esp_variant.cpp" data-file="registry/content.json" files="swofty.json, sounds/v10_screamer.bdl, registry/sounds.json, lang/esp.json">
<template #code>

```cpp
#include <map>
#include <string>

#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/sounds.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto v10 = sounds::engine_sounds.add("v10_screamer", { .bundle = file("sounds/v10_screamer.bdl"),
                                                            .settings_from = sounds::engine_sounds["game/ferrari_2025"] });

    // the box's own cars: "mod.esp/ferrari_v10" extends Classic Seasons' 2017 Ferrari
    std::map<std::string, Ref<cars::Vehicle>> v10_cars;
    for (std::string team : { "mercedes", "ferrari", "redbull" })
        v10_cars[team] = cars::vehicles.variant(team + "_v10", cars::vehicles["swofty.classic-seasons-2017/" + team],
            [=](cars::Vehicle &v) {
                v.label = "esp_cs17_" + team;
                v.engine_sound = v10;
            });

    auto base = content::packages["swofty.classic-seasons-2017/f1_2017"];
    auto season = content::packages.variant("f1_2017_v10", base, [=](content::Package &p) {
        p.title = "lng_esp_era_2017_v10";
        p.entrant_list = "esp_f1_2017_v10";
        for (const auto &[team, car] : v10_cars)
            p.teams.at(team).vehicle = car;
    });

    content::era_boxes.add("f1_2017_v10", { .package = season, .order = 2017 });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/content.json
{
  "vehicle": {
    "mercedes_v10": { "extends": "swofty.classic-seasons-2017/mercedes", "label": "esp_cs17_mercedes", "engine_sound": "mod.esp/v10_screamer" },
    "ferrari_v10":  { "extends": "swofty.classic-seasons-2017/ferrari",  "label": "esp_cs17_ferrari",  "engine_sound": "mod.esp/v10_screamer" },
    "redbull_v10":  { "extends": "swofty.classic-seasons-2017/redbull",  "label": "esp_cs17_redbull",  "engine_sound": "mod.esp/v10_screamer" }
  },
  "content.package": {
    "f1_2017_v10": {
      "extends": "swofty.classic-seasons-2017/f1_2017", "title": "lng_esp_era_2017_v10", "entrant_list": "esp_f1_2017_v10",
      "teams": [
        { "key": "mercedes", "vehicle": "mod.esp/mercedes_v10" },
        { "key": "ferrari",  "vehicle": "mod.esp/ferrari_v10" },
        { "key": "redbull",  "vehicle": "mod.esp/redbull_v10" }
      ]
    }
  },
  "era.box": { "f1_2017_v10": { "package": "mod.esp/f1_2017_v10", "order": 2017 } }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] vehicle mod.esp/ferrari_v10: extends swofty.classic-seasons-2017/ferrari; changed /label, /engine_sound
[registry] content.package mod.esp/f1_2017_v10: extends swofty.classic-seasons-2017/f1_2017; changed /title, /entrant_list,
           /teams/key=mercedes/vehicle, /teams/key=ferrari/vehicle, /teams/key=redbull/vehicle
[era]      era.box mod.esp/f1_2017_v10: Addons row, order 2017 (after swofty.classic-seasons-2017/f1_2017)
```

[Events](/std/events) scoped to the 2017 package also run in its variant, "F1 2017 (V10)".

## Keep a stock file

A year pack can pin a stock file with `stock("...")`. The game then reads its original file at that path, whatever an import or an older addon's overlay put there.

Classic Seasons 2024's Red Bull uses the stock Red Bull sound. An imported F1 2012 sound pack replaces the stock bundles with V8s. The 2024 pack pins the stock bundle, and its car keeps the 2024 sound:

```cpp
// swofty.classic-seasons-2024: native/src/sounds.cpp
auto redbull_sound = sounds::engine_sounds.add("redbull", {
    .bundle = stock("audio/2025/bundles/redbull_2025.bdl"),     // the game's original, whatever an import replaced
    .settings_from = sounds::engine_sounds["game/redbull_2025"],
});
```

```json
// registry/sounds.json, without code
{ "engine_sound": { "redbull": { "bundle": { "stock": "audio/2025/bundles/redbull_2025.bdl" },
                                 "settings_from": "game/redbull_2025" } } }
```

Every other box that uses the stock sound still hears the import.

## A track is in every year

<Split>

A track addon goes into the base that every box inherits. Hockenheim appears in the 2017 box too, with its base data.

The box's track list shows the season's real calendar first, in round order: the calendar the package names in `content::Package::calendar`. Every other track that fits the box follows under **More tracks**. A track fits a box when:

- The track has a grid slot for every car the box runs.
- Every car in the box has AI data at that track.
- In a Time Trial box, the track supports Time Trial.

A track that doesn't fit is left out of that box. The track select screen's list builder needs reverse engineering. Until it's understood, the screen shows every track in the base in the game's own order, and a guard stops a track that doesn't fit before the session loads.

Each box also has its own view of each track in `tracks::per_package`, keyed `<package>/<track>`: whether it's offered, its place under More tracks, and per-track data for that box only. An empty field comes from the base track data. The core applies a box's `fuel` and `pit.speed_limit`; its other fields are stored. `fuel.burn_per_lap` is the track's fuel-per-lap figure for the MFD fuel delta, the fuel strategy and refuelling; the car's real burn comes from its power unit (see [What `burn_per_lap` changes](/std/systems/tracks#what-burn-per-lap-changes)).

Classic Seasons 2016 to 2024 each register the season's real calendar and name it in the package, optionally: a round at a venue no stock track provides, such as Sochi in 2017, is dropped until a track addon provides it (see [A calendar that uses it](/std/systems/tracks#a-calendar-that-uses-it-optionally)).

When the box is picked, or a career season that races in the box starts, the core applies its view to the track data the game reads. It puts the view back when that content ends. See [Tracks](/std/systems/tracks).

<template #example>
<Example root="swofty.classic-seasons-2017" code-file="native/src/tracks.cpp" data-file="registry/tracks.json" files="swofty.json, native/src/content.cpp, native/src/calendar.cpp">
<template #code>

```cpp
SWOFTY_REGISTER
{
    // the package names its calendar (content.cpp), and that calendar comes first in the box's list
    auto box = content::packages["swofty.classic-seasons-2017/f1_2017"];

    // 2017 fuel at two tracks; every other track keeps its base data
    tracks::per_package.add("f1_2017/monaco", { .package = box, .fuel = tracks::Fuel{ .burn_per_lap = 1.45f } });
    tracks::per_package.add("f1_2017/spa_francorchamps", { .package = box, .fuel = tracks::Fuel{ .burn_per_lap = 1.80f } });
}
```

</template>
<template #data>

```json
{
  "track.per_package": {
    "f1_2017/monaco":            { "package": "swofty.classic-seasons-2017/f1_2017", "fuel": { "burn_per_lap": 1.45 } },
    "f1_2017/spa_francorchamps": { "package": "swofty.classic-seasons-2017/f1_2017", "fuel": { "burn_per_lap": 1.80 } }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[tracks] box swofty.classic-seasons-2017/f1_2017 (grand_prix): calendar 18 of 20 rounds (venue/sochi: no track; venue/sepang: no track), more tracks 10
[registry] content swofty.classic-seasons-2017/f1_2017 active: 2 scoped changes applied (track data: monaco fuel 1.45, spa_francorchamps fuel 1.80)
[registry] content mod.esp/f1_2017_v10 active: calendar and track data inherited from swofty.classic-seasons-2017/f1_2017
[registry] content swofty.classic-seasons-2017/f1_2017 ended: 2 scoped changes put back
```

A box's list can also target a tag. A **tag** is a shared, named set of records, such as `c/street_circuits`, that any addon can add its tracks to. The F1 1990 box leaves every street circuit out of More tracks, including ones added later:

```cpp
auto street = tracks::tracks.tag("c/street_circuits");
tracks::per_package.layer(where<tracks::PackageTrack>([=](const tracks::PackageTrack &t) {
    return t.package.id() == "swofty.f1-1990/f1_1990" && street.contains(t.track);
}), [](tracks::PackageTrack &t) { t.offered = false; });
```

## Rules per box

Every Grand Prix and Time Trial box shares the `swofty_f1` setup mode. A **setup mode** is the game's rule set a weekend runs under, and the core narrows `swofty_f1` to the box the player pressed. Each box's points, DRS, tyre classes and rules (`content::Package::rules`) apply when a session loads in that box, and every other box keeps its own. See [Sessions](/std/systems/sessions). A box's DRS rule (`rules.drs`) applies from 1.6.4: [DRS per box](/std/systems/power-units#drs-per-box). A box's tyre warmer rule (`rules.tyre_warmers`, `content.package` 1.4) applies the same way: pressing the box writes it into `swofty_f1`'s `has_tyre_warmers`, and a box without the rule, or a stock box, puts the mode's own setting back. `content.package` 1.5 adds `rules.race_time_limit_s` (the race time limit in seconds, written into `swofty_f1`'s `race_time_limit_in_seconds`) and `rules.weekend_mode` (the name of a `game_setup_modes` row: pressing the box exchanges that row's weekend options, its practice, qualifying and race groups and the length options they offer, with `swofty_f1`'s, together with the row's `options_tag`, so the weekend menu offers only the values that tag's `options_system` restrictions leave; the next press of another box or a stock box exchanges them back). A row's `available_in` lists map a chosen menu value to a session group; they don't filter the menu. The helper row has to exist at boot, from the game data or an addon's database rows; it does not need to be listed in any game mode or championship (don't list it: a listed mode is offered by the picker). The game parses a setup mode into its store only when the row's `available` is true, and marks it true at startup only for modes an available game mode lists, so the core sets `available` to true on the row a package's `rules.weekend_mode` names at boot (one `[content]` line), and the era picker finds the helper in that store. When the helper is missing at a box press, the `[weekend]` line names the modes the store holds. A box whose helper row is missing keeps `swofty_f1`'s own weekend and the log says so (`[weekend]`). A box's fuel rules (`rules.refuelling`, `race_fuel_kg`, and from `content.package` 1.5 `refuel_stint_kg`, `quali_fuel_kg`, `race_start_fuel_from_quali` and `race_tyre_change`) are played by the core during the box's sessions: [Fuel rules per box](/std/systems/sessions#fuel-rules-per-box). A box's safety car rule (`rules.safety_car`, `content.package` 1.5: `off`, `reduced`, `standard` or `increased`) sets the game's safety car option when the box is pressed, so a 1959 box races without a safety car. Another box press, or a stock box, puts the player's value back when the option still holds the box's; a value the player picks after the press stays. The same goes for `rules.formation_lap` (a bool: `false` starts the races from the grid without a formation lap), `rules.penalties` (`off`, `corner_cutting_only`, `esports` or `on`, the game's rules and flags option) and `rules.corner_cutting` (`regular` or `strict`), all `content.package` 1.5. The core sets the stock options `safety_car_frequency`, `formation_lap`, `rules_and_penalties_mode` and `corner_cutting_mode` and logs each press under `[box-rules]`. A box's grid rule (`rules.grid`, `content.package` 1.5) sets the qualifying scoring (`best_lap`, `best_of`, `aggregate`) and the 107 % rule of its race grid: [Grid rules per box](/std/systems/sessions#grid-rules-per-box). A box's qualifying tyre rule (`rules.qualifying_tyres`, `content.package` 1.5) names the dry options its cars may use only in qualifying. Pressing the box makes the core refuse those options in race sessions, and the event's per-session preset hands their sets out in qualifying only. See [Qualifying tyres per box](/std/systems/tyres#qualifying-tyres-per-box). A box's points system (`rules.points`, a `session.points` record) applies the same way from 1.9.61: [Rules per box](/std/systems/sessions#rules-per-box). A box's mandatory tyre change rule (`rules.mandatory_tyre_change`, `content.package` 1.6) applies like the tyre warmer rule: `false` clears every race length requirement of `swofty_f1`'s tyre regulations (no two-compound rule, no penalty for it), `true` sets them all, and a box without the rule puts the mode's own requirements back. The game reads them when a race session starts.

Only a box that needs a different shape at boot gets its own setup mode. F1 1990's box has its own mode for its two qualifying sessions.

Grid size is one boot setting per install, and each box is clamped to it.

## Time Trial

A box can select a different entrant list in Time Trial with `time_trial.entrant_list` (`era.box` 1.1). This 2012 box races 24 cars in a Grand Prix and 12 in Time Trial:

```json
// registry/content.json
{
  "content.package": {
    "f1_2012":    { "title": { "literal": "F1 2012" }, "entrant_list": "ex_f1_2012",    "season": 2012, "teams": [ ... ] },
    "f1_2012_tt": { "title": { "literal": "F1 2012" }, "entrant_list": "ex_f1_2012_tt", "season": 2012, "teams": [ ... ] }
  },
  "era.box": {
    "f1_2012": {
      "package": "mod.example/f1_2012",
      "order": 2012,
      "time_trial": { "entrant_list": "ex_f1_2012_tt" }
    }
  }
}
```

The same box from code, with `swofty::era::add_box` (`swofty/era.hpp`):

```cpp
swofty::era::add_box({
    .entrant_list = "ex_f1_2012",
    .title = "lng_ex_f1_2012",
    .order = 2012,
    .time_trial = "ex_f1_2012_tt",                 // empty: entrant_list
});
```

- **The car limit.** A Time Trial session holds at most **23** cars of the list it was picked with. The game adds three cars of its own to a table of 26. A 24-car list crashes Time Trial when the session loads.
- **On the Time Trial picker** the box selects its Time Trial list, and the core narrows `swofty_f1` to it. On the Grand Prix picker the box selects its package's list.
- **The list** is set up like the package's: a second package's list, or a list your database operations add to `swofty_f1`'s entrant lists.
- **No Time Trial list** means the package's list, under the same limit.
- **The check.** Each time the Time Trial picker is built, the core leaves a box off it, with one log line, when its list isn't offered under `swofty_f1` or holds more than 23 cars. At boot, the core's `era.box` report also names a box whose package has more than 23 cars and no Time Trial list.
- **The guard.** The core keeps a list of more than 23 cars out of every Time Trial session, whichever box or addon selected it:
  - A press on a stock box in Time Trial leaves `swofty_f1` offering only its lists of at most 23 cars until the next press, and no list at all when none of them fits. A list whose car count can't be read stays offered.
  - When a Time Trial session loads, the core logs the list the game resolved. When that list, or the first list of its entrant setup mode, holds more than 23 cars, the core loads a list of at most 23 cars from the same entrant setup mode instead. It tries the box pressed first, then that box's Time Trial list, then the first list that fits.
  - The session's entrant setup mode draws at most 23 cars (`max_entrants` above 23, or unset, is held at 23) until the session ends. With no list that fits, this keeps the session at 23 cars of the list.
  - A Time Trial takes more than its own list: one car for each distinct vehicle of every list on the selected list's leaderboard, in every setup mode the Time Trial offers, plus 4 entries of its own, in a table of 26. The stock F1 2025 box draws 11 cars and spawns 15 vehicles. A list copied from `f1_2025` kept its leaderboard (`f1`) before 1.7.20, so F1 2012 Season's 12 cars made it 27. Every package's list has a leaderboard of its own since 1.7.20 ([Leaderboards](#leaderboards)). Each Time Trial load counts those cars the way the game takes them: in the setup modes the Time Trial's override rows swap in (F1 2025's `f1` becomes `f1_custom_team`), each list only where its setup mode takes list cars, and the entrants system's own cars when the mode takes those. It leaves whole lists out, stock lists kept before addon lists, until at most 22 remain. The selected list always stays. A restart keeps the cut, and the held `max_entrants` and list swap; they come back at StateUnloadSession Exiting. With the core's 16 restore slots full, a cut is skipped and logged. Each session trace line in a Time Trial shows the budget's expected vehicles, and a separate line names any step where the fill made a different number. Before the fill (StateLoadSession Entering and Updating) a count is compared only when it changed since the step before, since the previous session's participants are still there.
  - Each session load logs a `session trace` line at StateLoadSession Entering, Updating and Exiting, at StateSession Entering and at the session's vehicle setup: the entrants with their team ids, the list, the setup mode, the session participants and the vehicles.
  - Everything the guard changed is put back when the session unloads (StateUnloadSession Exiting) or a box is pressed.
- A core before launcher 1.6.2 ignores `time_trial` and has no guard.
- **One picker only.** `only` (`era.box` 1.2, launcher 1.8.25) leaves a box off the other picker: `"only": "time_trial"` shows it only on the Time Trial picker, `"only": "grand_prix"` only on the Grand Prix picker. Absent shows it on both. A Grand Prix only box is never checked against the Time Trial car limit. The native call is `Box::only` ([Era picker boxes](/reference/std/era-picker#one-picker-only)).

```json
"era.box": { "preview": { "package": "mod.example/preview", "order": 2008, "only": "time_trial" } }
```
- **Tyre classes.** A car's tyres come from its own compound class (the `compound class` of its car template; a car built on the F2 template gets an F2 class), while the session's tyre sets come from the setup mode's class. A Time Trial at an F1 track narrows an F2 class's dry options to the track's F1 compounds, which it lacks, so only its Wet option is left. The game then refits every such car through a missing option and faults on session enter. Since launcher 1.6.12 the core's tyre slot guard gives the set a slot with the compounds of another option of the car's own class, dry first, so the car runs on its Wet compound instead of crashing, and logs a `[tyres] slot guard` line for the first 8 repairs. A box whose cars race under F1 rules should give them an F1-layout class (`f1`), as F1 1990 does with `Sess_TyreClass_FromName`.

```text
[era]      box ex_f1_2012 from mod.example registered (title lng_ex_f1_2012, image #overrides.home_screen.tiles_solo_6x6[ex_f1_2012], order 2012, Time Trial list ex_f1_2012_tt): live from the next build of the picker
[era]      box ex_f1_2016 from mod.other skipped in Time Trial: its Time Trial list ex_f1_2016 holds 24 cars; Time Trial holds at most 23 (the game adds 3 cars of its own to a table of 26). Give the box a Time Trial entrant list of at most 23 cars
[era]      swofty_f1 keeps ex_f1_2016 out of Time Trial while a stock box is picked: it holds 24 cars; Time Trial holds at most 23 (the game adds 3 cars of its own to a table of 26)
[era]      swofty_f1 keeps bdx_s5_2012 out of Time Trial while a stock box is picked: it holds 24 cars; Time Trial holds at most 23 (the game adds 3 cars of its own to a table of 26); no list of it fits, so it offers none until the next press
[era]      the Time Trial session's list f1_custom_team (leaderboard 0, f1) draws 11 cars from 1 list(s) of its leaderboard; with the game's 4 entries 15 of the 26-slot vehicle table; left out until the session unloads: ex_2012 (12 cars)
[era]      StateUnloadSession Exiting: the lists the Time Trial guards left out are back in 1 entrant setup mode(s)
[era]      Time Trial fill at Sess_Setup_VehicleComponents made 27 vehicles where the budget expected 15
[era]      Time Trial session loads list f1_2025 (20 cars) of setup mode f1 (box pressed: f1_2025)
[era]      the Time Trial session's list ex_f1_2016 (setup mode swofty_f1) holds 24 cars; Time Trial holds at most 23 (the game adds 3 cars of its own to a table of 26); loading ex_f1_2017 instead
[era]      the Time Trial session's list ex_2012 (setup mode ex_2012_mode) holds 24 cars; Time Trial holds at most 23 (the game adds 3 cars of its own to a table of 26); its entrant setup mode offers no list of at most 23 cars; the session draws at most 23 cars (max_entrants 24 -> 23 until the session ends)
[tyres]    slot guard (set visibility): vehicle 3's fitted option 0 has no usable slot in its class's table (count 1, options 4); slot 1 added with the compounds of option 4's slot
```

## Leaderboards

A Time Trial keeps one record per track and weather for each **leaderboard descriptor**, a row of `leaderboards.leaderboard_descriptor` that an entrant list names in its `leaderboard` field. Every list that names the same descriptor shares its cars and its times, and a Time Trial draws the cars of all of them. The game ships `f1`, `f1_26`, `f2`, `f2_26` and `classic`.

Each package gets a descriptor of its own. The core adds one row per package, a copy of `classic` named `swofty_<entrant_list>`, gives it the next free id above the stock ones (kept between builds), and sets the package's list to it:

```json
// registry/content.json
{
  "content.package": {
    "f1_2012": { "title": { "literal": "F1 2012" }, "entrant_list": "ex_f1_2012", "season": 2012, "teams": [ ... ] },
    "f1_2013": { "title": { "literal": "F1 2013" }, "entrant_list": "ex_f1_2013", "season": 2013, "teams": [ ... ],
                 "leaderboard": "stock:classic" },
    "f1_2014": { "title": { "literal": "F1 2014" }, "entrant_list": "ex_f1_2014", "season": 2014, "teams": [ ... ],
                 "leaderboard": "group:v8" },
    "f1_2015": { "title": { "literal": "F1 2015" }, "entrant_list": "ex_f1_2015", "season": 2015, "teams": [ ... ],
                 "leaderboard": "group:v8" }
  }
}
```

The build adds two rows to `leaderboards.leaderboard_descriptor` (stock `f1` is 0, `classic` 1, `f1_26` 2, `f2` 3, `f2_26` 5), `swofty_ex_f1_2012` with `stat_id` 6 and `swofty_group_mod_example_v8` with `stat_id` 7, and the core logs each package's list:

```text
[content] content.package mod.example/f1_2012: entrant list ex_f1_2012, 20 entrants in swofty_f1, stat id 112, leaderboard swofty_ex_f1_2012 (stat id 6), scenario stat id 21
[content] content.package mod.example/f1_2013: entrant list ex_f1_2013, 20 entrants in swofty_f1, stat id 113, leaderboard classic, scenario stat id 22
[content] content.package mod.example/f1_2014: entrant list ex_f1_2014, 20 entrants in swofty_f1, stat id 114, leaderboard swofty_group_mod_example_v8 (stat id 7), scenario stat id 23
[content] content.package mod.example/f1_2015: entrant list ex_f1_2015, 20 entrants in swofty_f1, stat id 115, leaderboard swofty_group_mod_example_v8, scenario stat id 24
```

`leaderboard` takes one of:

- `"box"` (default): the package's own descriptor, `swofty_<entrant_list>`.
- `"addon"`: one descriptor, `swofty_addon_<addon>`, for every package of the addon that says `"addon"`.
- `"group:<name>"`: one descriptor, `swofty_group_<addon>_<name>`, for the addon's packages that name the group.
- `"stock:<label>"`: the game's descriptor with that label; no row is added.

Boxes that share a descriptor share one Time Trial record per track and the cars count together against the table's 22 free vehicle slots, so share only when the lists have cars in common. A box that selects a [Time Trial list](#time-trial) of another package takes that package's descriptor.

- **The id.** A descriptor's id is its row's `stat_id`, a byte, so the pool is 6 to 255. The core keeps the id of each label in `ids.json`, and a label keeps its id when other addons come and go, so a record saved under `LBV<id>` stays on its box.
- **The 22-car check.** The core counts the cars of every list on the selected list's descriptor. A list alone on its descriptor draws only its own cars, and the log names the id: `the Time Trial session's list ex_f1_2012 (leaderboard 6, added) draws 20 cars from 1 list(s) of its leaderboard`.
- **Your own rows.** A package whose `database.entrant_list_set` sets `leaderboard` keeps that label and the core adds no row. Database operations that add a descriptor use `add-row` into the `leaderboards` table from `classic` with a `_metadata_`; the id stops at 255 like the other byte-sized tables ([Stat ids](/reference/ops/database#kept-ids)).
- **Older cores.** A core before 1.7.20 ignores `leaderboard`, and the list keeps the leaderboard of the list it was copied from. Declare `content.package` 1.2 when the box depends on it.
- **Saved times.** A box's personal bests are stored under its descriptor's board name. Whether the game keeps offline bests per descriptor is not yet confirmed in game.

## Virtual lists

The game loads every entrant list below the table's count, keeps its stat id in a byte (255 at most) and lets an entrant setup mode hold 32 lists. A big season pack runs out of both. `"virtual_list": true` (`content.package` 1.3, launcher 1.9.10) makes the package's list take neither:

```json
{ "content.package": { "f5_1959": { "entrant_list": "f5_1959", "season": 1959, "virtual_list": true } } }
```

- The core moves the list's row above the table's count after every record is lowered, and leaves the list out of its setup mode's lists. The game never loads it.
- The box that opens the package shows when `swofty_virtual_list` is offered under `swofty_f1`. When the box is pressed, the core reads the list's row into `swofty_virtual_list` and selects that list. The next press puts the host's own entrants back.
- The database op is [`virtual-list`](/reference/ops/database#virtual-list).
- **Older cores.** A core before 1.9.10 ignores `virtual_list` and loads the list as usual. Declare `content.package` 1.3 when the box depends on it.

## Career seasons race in boxes

A career season names the box it races in with `career::Season::box`, and a fallback with `.or_else()`. The season uses that box's cars, looks and rules.

```cpp
// swofty.progressive: the 2017 season races in Classic Seasons' 2017 box, or the stock 2025 box without it
career::seasons.add("2017", {
    .data_group = "f1_career_progressive",
    .year = 2017,
    .box = content::packages["swofty.classic-seasons-2017/f1_2017"].or_else(content::packages["game/f1_2025"]),
});
```

- **The box for each season is chosen when the career is created.**
- **Two packs for one year:** when more than one installed box covers a year, the career setup page shows a choice for that year. It defaults to the box the career addon's season names. The choice is frozen at creation.
- **A career freezes each season's line-ups, ratings and rules when that season starts.** Cars, liveries, tracks and sounds come live from the season's box. Drivers and tracks installed mid-career join at the next season.
- **A career's owner can freeze everything at creation instead,** with `career::ModeRecord::freeze = career::Freeze::at_creation`. The career's hub can offer the same choice to the player as a **Freeze content** toggle.

See [Career snapshots](/std/scopes#career-snapshots) and [Career](/std/systems/career).

## The content registries

- `era.box`, `content.package`: this page.
- `vehicle`, `vehicle.package`, `vehicle.handling`: [Cars](/std/systems/cars).
- `engine_sound`: [Sounds](/std/systems/sounds).
- `livery`: [Liveries](/std/systems/liveries).
- `helmet`, `suit`, `gloves`, `wearable.season_set`: [Helmets, suits and gloves](/std/systems/wearables).
- `track`, `track.venue`, `track.data`, `track.per_package`: [Tracks](/std/systems/tracks).
- `team.identity`: [Teams](/std/systems/teams). `driver.identity`: [Drivers](/std/systems/drivers).
- `career.content`, `career.season`, `career.driver_ratings`: [Career](/std/systems/career).

Older addons' calls land in some of these. `swofty::era::add_box` registers an `era.box`, and a career content becomes a `career.content` record (see [Compatibility](/std/compatibility)).

## What the player sees

The launcher shows each GP/TT entry with the addons that built it, from the last session's `registry/effective.json` and its [origins](/std/reading):

```text
F1 2017                          Classic Seasons 2017
  Ferrari  engine: V10 Screamer  (mod.esp)
           livery: Monza 2017    (someone.liveries)
F1 2017 (V10)                    mod.esp, on Classic Seasons 2017
```

## Content ownership

`content::owned` says whether the player owns the content behind an entitlement name. `f1_2026` is the F1 2026 season content.

```cpp
#include <swofty/systems/content.hpp>

using swofty::content::Ownership;

switch (swofty::content::owned("f1_2026")) {
case Ownership::owned:     /* the player can use the 2026 cars */ break;
case Ownership::not_owned: /* the player does not own the 2026 content */ break;
case Ownership::unknown:   /* the game has no answer yet: ask again later */ break;
}
```

- **The answer is the game's own.** The core reads it with `FeTile_Check_EntrantListUsable` on the `f1_2026` entrant list, the same call the game uses to fill its session information. It runs on the flow thread; `owned` reads the cached result and is safe from any thread.
- **`unknown`** is returned until the game's front end has run for five seconds, for every name other than `f1_2026`, and on a build where the core does not resolve the game bindings.
- **`not_owned`** is reported once the check has returned false for about a minute in a row. Until then the answer stays `unknown`.
- **`owned` is final** for the session. `not_owned` is rechecked every ten seconds or so and becomes `owned` if the content is installed later.
- **The core logs the answer once** as `[entitlements] f1_2026: owned` or `not owned`.
- **`swofty/entitlements.json`** is written in the game folder when the answer is known: `{"f1_2026": true}` or `{"f1_2026": false}`.

## Readiness and limits

- **Packages and boxes: ready.** The core's era picker (`era_picker.cpp`, up to 64 boxes) and the entrant pipeline are L2, mapped and verified in the game. Classic Seasons ships nine seasons on them.
- **The entrant list must be in `swofty_f1`,** the host setup mode the core narrows to the pressed season.
- **Every team needs a team row the game can see.** A team's entrants and standings name the `entrants.teams` row of its `team` record: a `team.identity` record's row is labelled with its key, and a `game/` team is the game's own row. A team with no `team` names the row labelled like its car, which is how Classic Seasons' teams find their rows. A team whose `team` gives no row the game sees falls back to its car's row when that one exists, and the boot log says so. The game reads that row without checking it exists, so a package with a team whose row is missing would crash the game at boot. A team with no `team` whose car is an addon's own is only reported when its row is missing, since the addon's database operations add rows like Classic Seasons' `cs<year>_` ones. The core refuses a package whose team gives a missing row, or whose `game/` car has none, and names the missing row: `content.package <id>: team <key> names entrants.teams row <label>, which the game database lacks; not applied, so the game can boot`. To add a team, give it a `team` (a `game/` team, or a `team.identity` record with a donor), or give its car a `team_from`. A car whose `entrants.vehicles` row is missing is only reported.
- **Shared lists compose.** Every season appends its entrant list to `content_sets[2025|2026].available_entrant_lists_0` and `entrant_setup_modes[swofty_f1]`. The core merges the appends.
- **Sound rows: boot.** When the game reads `audio.audio_car_data` after boot needs reverse engineering. A team's `engine_sound` applies at boot, to its car everywhere. To change one box only, give the box its own vehicle variant.
- **The box's track list: partial.** The list builder behind the track select screen (`ui.track_select.build`) needs reverse engineering. A guard covers tracks that don't fit.
- **Scale:** one season is about 700 records. Classic Seasons 2017, as an addon for 1.5, builds its season with about 700 operations: 99 add-row, 170 set and 430 put-record. They cover its car packages, `file.put`s for the `cs17_*.bdl` bundles, 430 raw audio records, and the rows for the entrant list, teams and pit times.
- **Content ownership: experimental.** `content::owned` uses the experimental bindings `g_FeTile_Ctx` and `FeTile_Check_EntrantListUsable`, and returns `unknown` on a build where they don't resolve.
