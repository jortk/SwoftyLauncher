---
title: Career
description: Career modes, contents, seasons, calendars, ratings, characters and wizards in code, with the career events and operations.
---

# Career

With the career system you build Driver Career and My Team careers: a career's tile, its seasons and line-ups, the calendar, ratings, contracts, R&D, money and the people in it. Progressive Career, for example, plays Driver Career from 2016 to 2026 with each year's real line-ups, ratings and cars.

A **career mode** is the game's mode row, with its own save container and **data group** (the key that keeps one mode's career tables and save slots apart). A **season** record is one year's line-up. Each season names the **box** it races in: the Grand Prix and Time Trial season build whose cars, looks and rules it uses.

## Career scope

An addon that owns a career mode changes only its own careers. The addon owns a mode when it registers a `career.mode` or `career.content` record, or a content through `career::content::add`, with a data group. Every layer and selector that addon registers on a registry applied at `content`, `session` or `read` takes an implicit `{"data_group": <its group>}` selector. The core enforces it where the store decides which layers are active, so a layer needs no `.in()`:

```cpp
SWOFTY_REGISTER(swofty::Registrar &r)
{
    career::contents.add("progressive", {.mode = "driver_career_progressive", .data_group = "f1_career_progressive"});

    // A mid-season change, in Progressive careers only. Every other career reads 2019 unchanged.
    career::seasons.layer(career::seasons["swofty.progressive/2019"], [](career::Season &s) {
        s.changes.push_back({.round = 13, .slot = 7, .seat = 1, .out = drivers::identities["game/pierre_gasly"],
                             .in = drivers::identities["game/alex_albon"], .kind = career::ChangeKind::swap});
    });
}
```

An addon without a career mode keeps today's behaviour: its layers apply in every career and outside them. A layer on a registry applied at `boot` (`vehicle`, `livery`, `engine_sound`, `team.identity`, `suit` and the others the core lowers into the game database) is not filtered: boot records are lowered once, before any career is loaded, and `.in()` on them is refused for the same reason. Registries applied at `content`, `session` or `read` (`career.season`, `career.driver_ratings`, `content.package`, `hub.tab`, `team.logo`, `track.per_package`, `vehicle.handling`) are filtered.

To apply in every career, pass `Scope::global`:

```cpp
career::seasons.layer(career::seasons["swofty.progressive/2019"], edit, Scope::global);
```

In a data file the same opt-out is `"scope": "global"` on a `$select` entry:

```json
{
  "team.logo": {
    "$select": [
      { "ids": "swofty.classic-seasons-2019/*", "scope": "global", "patch": { "images": { "hud": "logos/hud.png" } } }
    ]
  }
}
```

A calendar can name its career with `data_group`. A `career.calendar` record with one exists only while a career of that group is loaded; one without is visible everywhere. Two career addons each keep their own calendar for the same year under different ids:

```cpp
career::calendars.add("cs_calendar_2019", {.data_group = "cs_career", .rounds = classic_rounds});
career::calendars.add("pg_calendar_2019", {.data_group = "f1_career_progressive", .rounds = progressive_rounds});
```

Database operations (`db/*.json`, `swofty::db`) are global: they write rows into the game database before any career exists. The core cannot scope them. Put team names, colours, logos, kits, calendars and car or sound swaps in registry records and layers, and keep database operations for rows that only your own mode reads, such as its data group's tables.

## The header

<Split>

`swofty/systems/career.hpp` has ten registries. The main ones:

- `career::modes`: a mode row, its save container and data group (`career.mode`);
- `career::contents`: a career tile and the data it clones (`career.content`);
- `career::seasons`: a season's box, line-up and mid-season changes (`career.season`);
- `career::calendars`: an event list (`career.calendar`);
- `career::ratings`: a driver's ratings at the start of a season (`career.driver_ratings`).

A year pack's season has no data group (`career.season` 1.1, launcher 1.9.55): no career races it as it is, and every career mode that covers the year makes it its own with a variant that sets its data group (see [Make a career mode](/make/career)). The core counts a season for a career only when its data group is the career's, so a pack's season never adds a year to a career by itself. A 1.0 core refuses a season with no data group.

The rest: `characters`, `wizards`, `hub_tabs`, and the [R&D trees and the driver market](#r-d-trees-and-the-driver-market) (`rnd_trees`, `markets`).

<template #example>
<Example root="swofty-sdk" code-file="include/swofty/systems/career.hpp">
<template #code>

```cpp
#pragma once
#include <swofty/career.hpp>
#include <swofty/events.hpp>
#include <swofty/registry.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/drivers.hpp>
#include <swofty/systems/teams.hpp>

namespace swofty::career {

enum class Kind { driver_career, my_team };
enum class Freeze { per_season, at_creation };
enum class AfterLast { market, hold, end };
enum class ChangeKind { swap, stand_in, move, replace };
enum class Careers { driver, my_team, both };

struct ModeRecord {
    static constexpr Version version{1, 0};
    std::string data_group;                    // partitions 8 career tables and the save slots: "f1_career_progressive"
    std::string save_container;                // save_data.name: the mode's own container in the profile save
    Text title;                                // the career tile's title
    Kind kind = Kind::driver_career;
    Freeze freeze = Freeze::per_season;        // per_season: each season's line-up, ratings and rules freeze when it starts
    StatId stat_id = StatId::automatic;        // a game_modes.ModeChampionship row
    json::Value extra;
};
SWOFTY_RECORD(ModeRecord, data_group, save_container, title, kind, freeze, stat_id)
inline Registry<ModeRecord> modes{"career.mode", {.applies = Applies::boot}};

struct Content {
    static constexpr Version version{1, 0};
    Ref<ModeRecord> mode;
    Path tile;                                 // the tile image
    Ref<Content> model;                        // clone the 8 data-group tables, championship and components from it
    Ref<Calendar> calendar;                    // the wizard's calendar
    AfterLast after_last = AfterLast::market;  // past the last season record: the market, the last year forever, or the end
    Text title;
    Text description;
    std::string data_group;
    std::string suffix;
    std::string save_container;
    int car_group = -1;
    int badge = 0;
    std::string entitlement;
    Careers careers = Careers::driver;
    json::Value extra;
};
SWOFTY_RECORD(Content, mode, tile, model, calendar, after_last, title, description, data_group, suffix, save_container,
              car_group, badge, entitlement, careers)
inline Registry<Content> contents{"career.content", {.applies = Applies::boot}};

struct SeasonTeam {
    int slot = 0;                              // 1-12, the career team that carries this season's team
    Ref<teams::Identity> team;                 // empty: the slot is empty this season
    std::array<Ref<drivers::Identity>, 2> seats;
};
SWOFTY_RECORD(SeasonTeam, slot, team, seats)

struct SeasonChange {
    int round = 0, slot = 0, seat = 0;         // from this round on, this seat
    Ref<drivers::Identity> out, in;
    ChangeKind kind = ChangeKind::swap;        // swap: the two drivers exchange seats; stand_in: `in` drives until
                                               // `until_round`, then `out` returns; move: `in` was on the grid in
                                               // another seat and moves to this one; replace: `in` takes the seat for
                                               // the rest of the season and `out` leaves the grid
    std::optional<int> until_round;            // stand_in only
};
SWOFTY_RECORD(SeasonChange, round, slot, seat, out, in, kind, until_round)

struct Season {
    static constexpr Version version{1, 1};
    std::string data_group;                    // left out of a year pack's season: a career mode makes it its own with a variant
    int year = 0;
    Ref<::swofty::content::Package> box;       // the box it races in: its cars, looks and rules; .or_else() names the fallback
    Ref<Calendar> calendar;                    // the season's real calendar; empty: the career's
    Keyed<SeasonTeam, &SeasonTeam::slot> teams;
    Keyed<SeasonChange, &SeasonChange::round, &SeasonChange::slot, &SeasonChange::seat> changes;
    json::Value extra;                         // fields from a newer minor, kept for the round trip
};
SWOFTY_RECORD(Season, data_group, year, box, calendar, teams, changes)
inline Registry<Season> seasons{"career.season",
                                {.applies = Applies::read, .snapshot = true, .snapshot_budget = 65536}};

struct Round {
    int round = 0;
    Text race;                                 // the race name: "lng_gp_usa"
    Ref<tracks::Venue> venue;                  // resolves to the enabled track that provides it (the last in load order)
    std::chrono::year_month_day date;          // the game dates weekends from the track data in the mode's year
};
SWOFTY_RECORD(Round, round, race, venue, date)

struct Calendar {
    static constexpr Version version{1, 0};
    Keyed<Round, &Round::round> rounds;        // schedule.event_lists rows
    int seasons = 10;                          // _season_instancecount_: the career length
    std::vector<int> transfer_windows;         // the round after which a window opens
    json::Value extra;
};
SWOFTY_RECORD(Calendar, rounds, seasons, transfer_windows)
inline Registry<Calendar> calendars{"career.calendar", {.applies = Applies::boot}};

struct DriverRatings {
    static constexpr Version version{1, 0};
    Ref<Season> season;
    Ref<drivers::Identity> driver;
    float experience = 0, racecraft = 0, awareness = 0, pace = 0;   // 0..100, one decimal
    json::Value extra;
};
SWOFTY_RECORD(DriverRatings, season, driver, experience, racecraft, awareness, pace)
inline Registry<DriverRatings> ratings{"career.driver_ratings",
                                       {.applies = Applies::read, .snapshot = true, .snapshot_budget = 65536}};

struct CharacterSwap {
    std::string character;
    std::string with;
};
SWOFTY_RECORD(CharacterSwap, character, with)

struct Character {
    static constexpr Version version{1, 0};
    Ref<Character> replaces;                   // the stock character it stands in for, per data group
    Text first_name, last_name;
    Path portrait;                             // the core makes the message_avatars slice
    std::vector<std::string> mute;             // speech lines it mutes: "agent", "agent_cutscene"
    std::string data_group;
    Keyed<CharacterSwap, &CharacterSwap::character> swaps;
    std::map<std::string, std::map<std::string, std::string>> strings;
    json::Value extra;
};
SWOFTY_RECORD(Character, replaces, first_name, last_name, portrait, mute, data_group, swaps, strings)
inline Registry<Character> characters{"career.character", {.applies = Applies::content}};

struct WizardStep {
    std::string route;                         // the setup screen it opens, in flow_career_season_setup
    Text title;
    std::vector<std::string> options;          // the addon options this step sets for the new career
};
SWOFTY_RECORD(WizardStep, route, title, options)

struct WizardRoute {
    std::string from;
    std::string to;
};
SWOFTY_RECORD(WizardRoute, from, to)

struct WizardSwap {
    int at = 0;
    int with = 0;
};
SWOFTY_RECORD(WizardSwap, at, with)

struct Wizard {                                // new-career setup steps, after the game's own
    static constexpr Version version{1, 0};
    Ref<Content> content;                      // the career tile it runs for
    std::vector<WizardStep> steps;
    std::string data_group;
    bool starts_in_f1 = false;
    Keyed<WizardRoute, &WizardRoute::from> tile_routes;
    Keyed<WizardSwap, &WizardSwap::at> replace_steps;
    Keyed<WizardSwap, &WizardSwap::at> force_options;
    Keyed<WizardRoute, &WizardRoute::from> confirm_routes;
    std::vector<std::string> hide_driver_tabs;
    json::Value extra;
};
SWOFTY_RECORD(Wizard, content, steps, data_group, starts_in_f1, tile_routes, replace_steps, force_options, confirm_routes,
              hide_driver_tabs)
inline Registry<Wizard> wizards{"career.wizard", {.applies = Applies::content}};

struct HubTab {
    static constexpr Version version{1, 0};
    std::string name;
    std::string title;
    std::string data_group;
    std::vector<std::string> options;
    json::Value extra;
};
SWOFTY_RECORD(HubTab, name, title, data_group, options)
inline Registry<HubTab> hub_tabs{"hub.tab", {.applies = Applies::read}};

namespace events {

struct SeasonStart {                           // a season begins: the first, then each after the break (notify)
    static constexpr const char *point = "career.season.start";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int32_t year;
    int32_t season_index;
};

struct AddActivity {                           // Game_CareerActivities_AddActivity (cancel)
    static constexpr const char *point = "career.calendar.add_activity";
    static constexpr uint32_t version = 1;
    static constexpr uint8_t contract_event = 0x0f;
    uint32_t size, payload_version;
    void *calendar;                            // the driver calendar the activity goes on
    const void *activity;                      // the activity definition
    uint32_t driver;                           // the calendar's driver, stat id << 16
    uint32_t day;                              // before handlers may move it
    uint8_t type;                              // the activity type
    bool player;                               // the calendar is the human player's
};

struct ContractDecide {
    static constexpr const char *point = "career.contract.decide";
    static constexpr uint32_t version = 1;
    static constexpr uint32_t negotiate = 0x5e3c3c2d;
    static constexpr uint32_t ends_this_season = 0xd2985448;
    static constexpr uint32_t next = 0x7c9b1ec4;
    uint32_t size, payload_version;
    void *state;
    uint32_t link;
};

struct ContractSign {
    static constexpr const char *point = "career.contract.sign";
    static constexpr uint32_t version = 1;
    static constexpr uint32_t work_out_replaced = 0x3fff0000;
    uint32_t size, payload_version;
    void *contracts;
    void *contract;
    uint32_t driver;
    uint32_t replaced;
    uint32_t team;
    uint8_t type;
    bool notify;
};

struct StandingsRebuild {                      // Career_Rebuild_SeasonStandings (modify)
    static constexpr const char *point = "career.standings.rebuild";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    void *championship;                        // the standings the game rebuilt
};

// no payload beyond size and payload_version: Created (career.created), Loaded (career.loaded),
// Unloaded (career.unloaded), SeasonEnd (career.season.end), SeasonBreak (career.season.break), Saving (career.saving)

} // namespace events

struct ApplyReport {                           // what apply_teams and apply_lineup did, seat by seat
    size_t changed = 0, unchanged = 0, refused = 0;
    std::vector<std::string> refusals;         // one line per refused seat: which seat, and why
};

const SwoftyCareerOps *ops();
Result set_slots(std::string_view data_group, const std::vector<uint32_t> &team_stats);
Result apply_teams(const Season &season, ApplyReport *report = nullptr);
Result apply_lineup(const Season &season, ApplyReport *report = nullptr);
Result set_seat(int slot, int seat, Ref<drivers::Identity> driver);
Result set_seat_of_team(uint32_t team_stat, int seat, Ref<drivers::Identity> driver);
Result set_driver_ratings(Ref<drivers::Identity> driver, const DriverRatings &ratings);
Result set_car_number(Ref<drivers::Identity> driver, int number, bool swap = false);
Result swap_drivers(Ref<drivers::Identity> a, Ref<drivers::Identity> b);   // the game's full move sequence
Result set_dropped_scores(std::string_view data_group, int best);   // the loaded career counts each driver's best N results
Result report_season(int year, int season_index);   // the career's owner: the season at season_index is year

} // namespace swofty::career
```

</template>
</Example>
</template>
</Split>

## Progressive's career mode

<Split>

Progressive Career plays Driver Career from 2016 to 2026 with each year's real line-ups and ratings. Its mode is a variant of the 2026 Driver Career with its own data group and save container. Its careers never mix with stock ones, and changes other addons make to the stock mode flow into it.

It reads its own `data/years.json` and registers a season and a set of ratings per year. Each season names its box: Classic Seasons' box for that year, with the stock year as the fallback. At each season start it applies the line-up.

<template #example>
<Example root="swofty.progressive" code-file="native/src/register.cpp" data-file="registry/career.json" files="swofty.json, native/CMakeLists.txt, native/src/years.hpp, native/src/years.cpp, native/src/calendars.cpp, data/years.json, art/tile.png">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/career.hpp>
#include "years.hpp"                               // progressive::years(): data/years.json, its own format

using namespace swofty;

static const events::Selector kProgressive = events::content("swofty.progressive/f1_career_progressive");

SWOFTY_REGISTER
{
    auto mode = career::modes.variant("progressive", career::modes["game/f1_career_2026"], [](career::ModeRecord &m) {
        m.data_group = "f1_career_progressive";
        m.save_container = "driver_career_progressive";
        m.title = "lng_progressive_title";
    });

    career::contents.add("f1_career_progressive", {
        .mode = mode,
        .tile = file("art/tile.png"),
        .model = career::contents["game/f1_career_2026"],
    });

    for (const progressive::Year &y : progressive::years()) {
        auto season = career::seasons.add(std::to_string(y.year), {      // "swofty.progressive/2017"
            .data_group = "f1_career_progressive",
            .year = y.year,
            .box = content::packages[y.box].or_else(content::packages["game/f1_2025"]),   // "swofty.classic-seasons-2017/f1_2017"
            .calendar = career::calendars[y.calendar],                                   // the year's real calendar (calendars.cpp)
            .teams = y.teams(),                    // slot, team, two seats
            .changes = y.changes(),                // mid-season swaps and stand-ins by round
        });
        for (const progressive::Rating &r : y.ratings)
            career::ratings.add(std::format("{}/{}", y.year, r.driver), {
                .season = season,
                .driver = drivers::identities[r.driver],
                .experience = r.experience, .racecraft = r.racecraft, .awareness = r.awareness, .pace = r.pace,
            });
    }
}

SWOFTY_ON_LOAD
{
    events::on<career::events::SeasonStart>().in(kProgressive)
        .after([](const career::events::SeasonStart &e, const events::Context &ctx) {
            auto season = ctx.record<career::Season>();    // this season, frozen as it started
            career::ApplyReport report;
            career::apply_lineup(*season, &report);
            swofty::log("[progressive] %d: %zu seat(s) set, %zu unchanged, %zu refused", e.year,
                        report.changed, report.unchanged, report.refused);
        });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "career.mode": {
    "progressive": { "extends": "game/f1_career_2026", "data_group": "f1_career_progressive",
                     "save_container": "driver_career_progressive", "title": "lng_progressive_title" }
  },
  "career.content": {
    "f1_career_progressive": { "mode": "swofty.progressive/progressive", "tile": "art/tile.png",
                               "model": "game/f1_career_2026" }
  },
  "career.season": {
    "2017": { "data_group": "f1_career_progressive", "year": 2017,
              "box": "swofty.classic-seasons-2017/f1_2017|game/f1_2025", "calendar": "swofty.progressive/2017",
              "teams": [ { "slot": 1, "team": "game/mercedes", "seats": ["game/lewis_hamilton", "game/valtteri_bottas"] } ],
              "changes": [ { "round": 15, "slot": 7, "seat": 1, "out": "game/daniil_kvyat", "in": "game/pierre_gasly" } ] }
  },
  "career.driver_ratings": {
    "2017/game/lewis_hamilton": { "season": "swofty.progressive/2017", "driver": "game/lewis_hamilton",
                                  "experience": 94, "racecraft": 93, "awareness": 90, "pace": 95 }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: swofty.progressive (progressive.dll) 1 career.mode, 1 career.content, 11 career.season,
           242 career.driver_ratings (early stage, 3.8 ms)
[registry] stat ids: career.mode swofty.progressive/progressive -> 82
[registry] applied at boot: career.mode 1, career.content 1 (8 data-group tables cloned from game/f1_career_2026) (1,204 database records)
[registry] career season 2016 starts in swofty.progressive/f1_career_progressive: snapshot of career.season 1,
           career.driver_ratings 22 (3 KB)
[progressive] 2016: 22 seat(s) set, 0 unchanged, 0 refused
```

The data version has no season-start handler. It lists the seasons, and applying a line-up needs code.

## Career contents

A **career content** is a tile in the Addons row of the career menu, under the stock 2025 and 2026 tiles, with its own data group and save slots. A `career.content` record makes one: from a data file with no DLL, from `SWOFTY_REGISTER`, or from the 1.5 call `career::content::add`, which provides the record under the calling addon's id. All three make the same tile.

### From a data file

```json
// registry/career.json in community.f1-2012
{
  "career.mode": {
    "f1_2012": { "data_group": "f1_career_2012", "save_container": "driver_career_2012", "title": "lng_f1_2012_title" }
  },
  "career.content": {
    "f1_2012": { "mode": "community.f1-2012/f1_2012", "suffix": "12", "description": "lng_f1_2012_desc",
                 "careers": "both" }
  }
}
```

- **From the mode.** A field the content leaves out comes from the `career.mode` record its `mode` names: `data_group`, `save_container` and `title`. The 1.6 core has no stock `career.mode` records: the mode is a record of its own, not a variant of `game/f1_career_2026`.
- **Defaults.** `description` defaults to the title, `model` to `game/f1_career_2026` and `careers` to `driver`. `model` is `game/f1_career_2026` or `game/f1_career_2025`.
- **Required.** `suffix`, and a data group, save container and title from the content or its mode.
- **Not read for the tile:** `tile`, `calendar` and `after_last`.
- **Layers.** The tile shows the effective record, so another addon's layer on its title, description, badge or careers shows on the tile.
- **With a call.** When the addon's DLL also calls `career::content::add` with the record's key, the call's content replaces the record's before its first tile. Calls and records share the core's 8 contents.

The tile needs the same game data as a call's tile. The 1.6 core makes none of it from records. The addon's database operations add it:

- a `game_modes.ModeChampionship` row whose `save_data.name` is the save container;
- an `entrants.entrant_lists` row named by the data group, with a `stat_id` below 256;
- the official entrant list `F1CAREER_<suffix>` in a content set. `F2CAREER_<suffix>` and `ICONS_<suffix>` fill the F2 and Icons tabs of Choose Driver.

```text
[career] content 'f1_2012' from community.f1-2012 (career.content record community.f1-2012/f1_2012, no call) registered
         (tile 1 after the stock ones, 2026 model, data group f1_career_2012, suffix 12, mode driver_career_2012): queued
         for the next mixin commit
[career] content 'f1_2012' ready: mode driver_career_2012 statId 83, data group f1_career_2012 (entrant list stat_id 41),
         suffix 12 (F1CAREER_12, F2CAREER, ICONS)
```

A record that is missing a field makes no tile, and the log names the field:

```text
[career] career.content record community.f1-2012/f1_2012 makes no tile: it names no suffix (Choose Driver lists F1CAREER_<suffix>)
```

### From the 1.5 call

```cpp
#include <swofty/career_content.hpp>

namespace content = swofty::career::content;

bool register_tile()
{
    content::add({.id = "f1_2012",
                  .title = "lng_f1_2012_title",
                  .description = "lng_f1_2012_desc",
                  .data_group = "f1_career_2012",
                  .suffix = "12",
                  .mode = "driver_career_2012",          // the save container
                  .model = content::Model::f1_2026,
                  .careers = content::Career::both});
    return true;
}

SWOFTY_ADDON(register_tile)
```

The call provides a `career.content` record with these fields under `community.f1-2012/f1_2012`, and other addons layer it like a data-file record.

## A season's box

A season's box supplies its cars, liveries, sounds and session rules. The box each season uses is chosen when the career is created.

- **Two packs for one year.** When more than one installed pack offers a box for a season's year, the career setup page shows a choice for that year. It defaults to the box the season names, and the choice is frozen at creation.
- **A missing pack at the season break.** If the season's box is gone when its season starts, and the pack brought rules or systems, the career stops at the season break with a guard popup naming the pack. If the pack only brought cars and looks, the season uses the fallback after `.or_else()`, and the pack's tile in ADDON SETTINGS and in the launcher says so. No popup appears.

```text
[registry] career season 2019: box swofty.classic-seasons-2019/f1_2019 is off (cars and looks only); using game/f1_2025
```

## Standings

A season starts with every team on 0 points, listed in a start order. Progressive's start order is the real previous year's final constructors' order, then the slots that year doesn't list in slot order. A slot closed that year is left out.

### A twelfth team in Progressive

A layer that names a team in slot 12 of a Progressive season gives that season 12 teams. Progressive passes slot 12 to `career::apply_teams` with its seats, and lists it in the season-start order after the eleven career slots:

```cpp
career::seasons.layer(career::seasons["swofty.progressive/2016"], [](career::Season &s) {
    career::SeasonTeam twelfth;
    twelfth.slot = 12;
    twelfth.team = teams::identities["mod.f1-2012/marussia"];
    twelfth.seats = { drivers::identities["mod.f1-2012/timo_glock"], drivers::identities["mod.f1-2012/charles_pic"] };
    s.teams.push_back(twelfth);
}, Scope::profile);
```

```text
[progressive]   2016 slot 12 is mod.f1-2012/marussia (a record names the team)
[progressive] standings: season 2016 starts in the real 2015 order: 24 driver(s) (lewis_hamilton first), 12 team(s): mercedes_26, ferrari_26, williams_26, redbull_26, forceindia_26, renault_26, tororosso_26, sauber_26, mclaren_26, gm_cadillac_26, haas_26, marussia
```

Slot 12 is listed by the team row the core made for the record (its key, `marussia`), and only while a row of the career races for that team:

```text
[progressive] standings: season 2016 slot 12 (marussia) is left out of the order: no row of the career races for it
```

### A box with 12 teams

A `content.package` gets its own standings row, `<entrant_list>_teams`, which its entrant list names as the previous season's team standings. The core copies `f1_teams_2024` (10 teams), whatever the package's size, and writes the package's teams into it in package order. When the package has more teams than the copied row, the core adds the rest. The entrant list is a copy of `f1_2025` (20 entrants) and grows the same way. The copied rows carry no F1 2026 content, so a player who does not own it can open the package's track select:

```json
{
  "content.package": {
    "f1_2012": {
      "title": "lng_f1_2012", "entrant_list": "f1_2012", "season": 2012,
      "teams": [
        { "key": "redbull",  "team": "mod.f1-2012/redbull",  "vehicle": "mod.f1-2012/redbull_2012",
          "drivers": [ { "driver": "game/sebastian_vettel", "number": 1 }, { "driver": "game/mark_webber", "number": 2 } ] },
        { "key": "marussia", "team": "mod.f1-2012/marussia", "vehicle": "mod.f1-2012/marussia_2012",
          "drivers": [ { "driver": "mod.f1-2012/timo_glock", "number": 24 }, { "driver": "mod.f1-2012/charles_pic", "number": 25 } ] }
      ]
    }
  }
}
```

```text
[apply] content.package boot:   content.package mod.f1-2012/f1_2012: standings f1_2012_teams has 12 teams; f1_teams_2024 holds 10, the rest are added
[apply] content.package boot:   content.package mod.f1-2012/f1_2012: entrant list f1_2012 has 24 entrants; f1_2025 holds 20, the rest are added
[apply] content.package boot:   content.package mod.f1-2012/f1_2012: entrant list f1_2012, 24 entrants in swofty_f1
```

The package's `database` block names other rows to copy: `standings_from` a standings row, `entrant_list_from` an entrant list. A row it names that already holds every team is copied as it is, and nothing is added.

```json
"database": { "standings_from": "mod_f1_2012_teams", "entrant_list_from": "f1_2025" }
```

## What a career freezes

A career freezes each season's line-ups, ratings and rules when that season starts. Drivers and tracks installed mid-career join at the next season. A fix to Progressive's 2019 line-up reaches a 2016 career when it gets to 2019.

A career's owner can freeze everything at creation instead:

```cpp
career::modes.layer(career::modes["swofty.progressive/progressive"], [](career::ModeRecord &m) {
    m.freeze = career::Freeze::at_creation;
});
```

The career's hub can offer the same choice to the player as a **Freeze content** toggle. Cars, liveries, tracks and sounds are never frozen. A career reads them live from its boxes.

The frozen data is the career's snapshot, saved in its `swofty_registry` entry. Each snapshot registry has a budget of 64 KB per season, and a career has 256 KB in all.

### When a season starts

The core fires `career.created` for the first career loaded after a new-career route. It fires `career.season.start` once per career and year, then activates the season's records and freezes the season.

The season's year comes from one of two places:

- **The career's owner reports it.** The addon that owns the data group, through its career content or its `career.mode` record, calls `career::report_season(year, season_index)` at the season's start. Any other addon gets `Result::refused`. A career an addon owns takes its years from these reports only.
- **The core works it out.** For a career no addon owns, such as the stock 2025 and 2026 careers, the core reads the Championship's `season_index` at an idle Team HQ once the season has its race weekends and a current weekend. The effective `career.season` records of the career's data group, sorted by year, give the year: index 0 is the first year, index 1 the second. With no records, two records for one year or a gap between years, nothing fires and the log says why.

```cpp
// Progressive, at the first idle Team HQ of a season it hasn't applied, once the season has its weekends:
// year is the career's first year plus season_index and its test seasons
career::report_season(year, season_index);
```

```text
[progressive] season 2017: reported to the core as season index 1 of career 1f2a: ok
[registry] career::report_season(2017, 1) from swofty.progressive: ok, career 1f2a season index 1 is 2017; career.season.start
[registry] career season 2017 starts in 1f2a/f1_career_progressive: snapshot of career.season, career.driver_ratings (39 record(s), 12.0 KB -> 2.4 KB compressed); saved with the next profile save
[registry] career 7c01 season index 1: no career.season.start (no career.season record has data group f1_career_2026)
```

A career's id is its mode and its save slot: `<mode>:<slot guid>`, or `<mode>:<slot id>` for a slot without a guid. The mode is the save container name of the career content that owns the data group, else `<data group>.<driver_career|my_team>`. The core reads the slot when the game selects it for a load and keeps the id once `career.loaded` fires. An entry saved under a career's older `dc-<driver>` key moves to the new id the first time that career loads.

A career's snapshot has 256 KB, and the whole entry 4 MB of payload. An entry past 4 MB isn't written with that save.

The core writes the entry with a profile save only with `SWOFTY_REGISTRY_SAVE=on`. Every profile save logs the entry's size. By default each profile save logs the entry it would write and how many bytes it would add, and writes nothing:

```text
[registry] swofty_registry dry: this profile save would write a 2591-byte entry (1 career(s), 1 season snapshot(s)), adding 2591 bytes to the save; nothing written (SWOFTY_REGISTRY_SAVE is unset, and saving defaults to dry)
```

## After the last year

A career can run past the last season record its addon provides: Progressive past 2026, or Classic Career past its last pack. By default (`AfterLast::market`) the career keeps the last year's box, with its cars and looks, and hands the line-up to the game's driver market. The core extends the retirement schedules past their last stock date, December 2035, so drivers keep retiring.

The career's owner can choose otherwise:

```cpp
career::contents.layer(career::contents["swofty.progressive/f1_career_progressive"], [](career::Content &c) {
    c.after_last = career::AfterLast::hold;    // the last year's line-up every season; end: the last year is the final season
});
```

## Calendars

The 1.6.0 core stores `career.calendar` records and applies nothing from them; it logs `[registry] career.calendar: N record(s); this core applies nothing from it`. Likewise `career.mode` records: the core reads a mode's data group, and the save container and title a career tile takes from it (see [Career contents](#career-contents)), and applies nothing else.

```cpp
auto calendar = career::calendars.add("f1_1990", {
    .rounds = {
        { .round = 1, .race = "lng_gp_usa", .venue = tracks::venues["venue/phoenix"].or_else(tracks::venues["venue/las_vegas"]),
          .date = 1990y / 3 / 11 },
        { .round = 4, .race = "lng_gp_monaco", .venue = tracks::venues["venue/monaco"], .date = 1990y / 5 / 27 },
    },
    .seasons = 1,
});
```

The game dates every weekend from the track data in the mode's year. It builds every season's weekends when a career is created, and the save keeps that list, so a calendar with more seasons reaches new careers only.

A history career can race each season's real calendar (`Season::calendar`). The core rebuilds the next season's rounds at the season break and drops venues that aren't installed. The career setup page asks whether to use real calendars or the wizard's calendar every season. New careers default to real calendars.

```text
[registry] career season 2017 calendar: 20 rounds from swofty.progressive/2017; venue/sepang not installed, round dropped
```

## R&D trees and the driver market

A career reads its R&D trees and its driver market from the game database when it is created and each time it loads. `career.rnd.tree` and `career.market` records change those rows at boot, so a career sees them at its next load, whether it was started before or after the record was installed. Neither is frozen per season.

Each game mode reads its own group of rows. Its upgrades component names the upgrades group (`upgrades_path`, for example `upgrades.myteam_2025` for a My Team career) and its driver market component names the market group (`data_group`). A record without a `data_group` writes every group, so it reaches every mode. A record with one writes only that group.

- **Loading a saved career rebuilds its R&D trees.** The game drops each team's saved R&D model, builds it again from the rows of the mode's upgrades group, then puts the saved progress back on the nodes it still finds by label. A new node starts unresearched. A removed node's progress is dropped.
- **The driver market is read again too.** The driver market component reads its group's tables (retirement and firing schedules, team rating requirements) in its data load, at a career's creation and at each load.

### R&D trees

A tree is one team's R&D: its `token` (the 12 stock trees: `mercedes`, `ferrari`, `redbull`, `williams`, `tororosso`, `renault`, `haas`, `mclaren`, `sauber`, `force_india`, `gm_cadillac`, `user_defined`) in an upgrades group. The game has four groups: `2022`, `myteam_2025`, `2026` and `myteam_om_xm`. `gm_cadillac` has a tree in `2026` and `myteam_om_xm` only. A My Team car uses the `user_defined` tree. Each department (`aerodynamics`, `chassis`, `durability`, `engine`) is a list of nodes, and the hub `default` links to each department's first nodes.

Without `data_group`, a record changes the token's tree in every group that has one, and the log has a line per group. The groups' trees differ (`2022` and `myteam_2025` have 24 aerodynamics nodes, `2026` has 25), so a `from` or `after` label one group lacks refuses that node in that group only.

```cpp
career::rnd_trees.add("mercedes_beam_wing", {
    .token = "mercedes",
    .nodes = {
        { .label = "swofty_beam_wing_1", .department = career::UpgradeDepartment::aerodynamics,
          .from = "drag_minor_1", .name = "swofty_beam_wing", .type = "Rear Downforce",
          .size = career::UpgradeSize::minor, .cost = 600, .x = 1, .y = -5,
          .after = { "drag_minor_1" } },                     // drag_minor_1 unlocks it
        { .label = "drag_minor_1", .department = career::UpgradeDepartment::aerodynamics, .cost = 450 },
    },
});
```

- **A new label adds a node.** The node copies the row of `from` (default: the department's first node) and takes the fields you give. It has no links of its own unless you give `links`.
- **A stock label edits that node.** Only the fields you give change. A non-empty `links` replaces the node's links.
- **`after`** names nodes of the same tree, in any department or the hub `default`, that unlock the new node. A node no other node links to never shows.
- **`type`** is one of the upgrade types the stock trees use: `Drag`, `Front Downforce`, `Rear Downforce`, `drs`, `Brakes`, `Chassis Weight Redistribution`, `Chassis Weight Reduction`, `Tyre Wear`, the six `... Durability` types, `Engine Power`, `Fuel Consumption` and `ers`. The game reads any other name as the first type, so the core refuses it.
- **Progress lives in the save by label.** A node removed later loses its progress at the next load. A new node starts unresearched in a running career.

```text
[apply] career.rnd.tree boot:   career.rnd.tree swofty.beamwing/mercedes_beam_wing: 1 node(s) added, 1 edited, 0 refused in the mercedes tree of upgrades group 2022
[apply] career.rnd.tree boot:   career.rnd.tree swofty.beamwing/mercedes_beam_wing: 1 node(s) added, 1 edited, 0 refused in the mercedes tree of upgrades group myteam_2025
[apply] career.rnd.tree boot:   career.rnd.tree swofty.beamwing/mercedes_beam_wing: 1 node(s) added, 1 edited, 0 refused in the mercedes tree of upgrades group 2026
[apply] career.rnd.tree boot:   career.rnd.tree swofty.beamwing/mercedes_beam_wing: 1 node(s) added, 1 edited, 0 refused in the mercedes tree of upgrades group myteam_om_xm
[apply] career.rnd.tree boot:   career.rnd.tree swofty.beamwing/ferrari_beam_wing: node sw_x_1 refused in upgrades group 2026: no node drag_minor_9 in aerodynamics to copy
```

A record covers one tree. Use `extends` to give the other trees the same nodes:

```json
{
  "career.rnd.tree": {
    "mercedes_beam_wing": { "token": "mercedes", "nodes": [ { "label": "swofty_beam_wing_1", "type": "Rear Downforce", "after": [ "drag_minor_1" ] } ] },
    "ferrari_beam_wing": { "extends": "swofty.beamwing/mercedes_beam_wing", "token": "ferrari" }
  }
}
```

### The driver market

A market group (`2025`, `2026`) holds the drivers who retire, the drivers who are fired, and the driver rating each team asks for. The stock game modes read both: some name `2025`, others `2026`. Without `data_group`, a record writes every group.

```cpp
career::markets.add("history", {
    .retirements = std::vector<career::Retirement>{
        { .driver = drivers::identities["game/fernando_alonso"], .retire = 2027y / 12 / 14, .announce = 2027y / 9 / 16 },
        { .driver = drivers::identities["game/lewis_hamilton"], .retire = 2028y / 12 / 14, .announce = 2028y / 9 / 23 },
    },
    .firings = std::vector<career::Firing>{ { .driver = drivers::identities["game/lance_stroll"], .season_index = 1 } },
    .team_ratings = { { .team = teams::identities["game/mclaren_26"], .rating = 97 } },
});
```

- **`retirements`** replaces the drivers of every retirement schedule (6 in stock, of 8 drivers each) with this list. A career uses one schedule. An empty list means nobody retires. Absent keeps the stock schedules.
- **`firings`** replaces the drivers of every firing schedule (4 in stock) the same way. `season_index` 0 is the career's first season.
- **`team_ratings`** sets the driver rating a team asks for. A team without a row gets one. An added team needs one, with its own `entrants.teams` label, for the market to rank it.
- A driver without a `entrants.drivers` row, a team without an `entrants.teams` row, or a group the game doesn't have is left out, and the log says so.

```text
[apply] career.market boot:   career.market swofty.history/history: 2 retirement(s) in 6 schedule(s), 1 firing(s) in 4 schedule(s), 1 team rating(s) set, 0 added in market group 2025
[apply] career.market boot:   career.market swofty.history/history: 2 retirement(s) in 6 schedule(s), 1 firing(s) in 4 schedule(s), 1 team rating(s) set, 0 added in market group 2026
```

Both registries are experimental in 1.6.4 (features `registry_career_rnd_tree` and `registry_career_market`): their shape can change before they are stable.

## Described, in headers that come later

Contracts, R&D departments, facilities, the economy and sponsors are described here. No header has a registry for them yet: `career.contract_rules`, `career.rnd.department`, `career.facility`, `career.economy` and `career.sponsor`.

```cpp
struct RndDepartment {
    Text name;
    Colour colour;
    Path icon;
    float cost_scale = 1.0f;                   // UpgradeDepartmentEntry economy scale
};

struct ContractRules {                         // driver_contracts and career.generic_contracts
    std::map<std::string, std::array<int, 2>> rating_ranges;   // driver_contracts.NCS.rating_ranges
    std::array<int64_t, 2> salary;             // the lowest and highest salary offered
    int secret_meetings = 9;                   // secret meetings a season offers
    bool counteroffers = true;                 // career.generic_contracts counteroffers
};
SWOFTY_RECORD(ContractRules, rating_ranges, salary, secret_meetings, counteroffers)

struct Facility {                              // career.my_team_facilities: Engineering, Personnel, Corporate
    Text name;
    std::string group;                         // "engineering", "personnel", "corporate"
    std::vector<std::string> perk_tree;        // the FacilityPerkTree nodes, in order
    StatId stat_id = StatId::automatic;
};
SWOFTY_RECORD(Facility, name, group, perk_tree, stat_id)

struct Economy {                               // resource points and bank, per data group
    std::string data_group;
    std::map<std::string, float> resource_point_rates;   // resource_points.*
    std::map<std::string, int64_t> budgets;    // bank: the budget per team key
    int64_t initial_cash = 0;                  // bank: starting cash
};
SWOFTY_RECORD(Economy, data_group, resource_point_rates, budgets, initial_cash)

struct Sponsor {                               // career.my_team_sponsors: 15 title and 72 secondary in stock
    Text name;
    Path logo;
    bool title = false;                        // a title sponsor
    std::vector<int> loyalty;                  // the loyalty track: points per level
    StatId stat_id = StatId::automatic;
};
SWOFTY_RECORD(Sponsor, name, logo, title, loyalty, stat_id)
```

- **Contracts:** the rating ranges, secret meetings and counteroffers are tables every career shares (`driver_contracts.NCS`, `career.generic_contracts`); no data group partitions them. The rating each team asks for is the market's `team_ratings`.
- **R&D departments:** the game draws four departments natively. A fifth and more come from native code, as R&D Tyres does. The department economy rows name attribute rows rather than holding numbers.
- **Trees:** found through 12 compiled-in tokens. A 13th tree needs native code.

## Characters and wizards

```cpp
career::characters.add("flavio", {
    .replaces = career::characters["game/elena"],
    .first_name = "lng_f1990_agent_first",
    .last_name = "lng_f1990_agent_last",
    .portrait = file("art/flavio.png"),
    .mute = { "agent", "agent_cutscene" },
});
```

## Events

```cpp
// Progressive: a Follow-your-driver player signs in the first Team HQ, so the contract event never goes on his calendar
events::on<career::events::AddActivity>().in(kProgressive)
    .replace([](career::events::AddActivity &a, const events::Context &) {
        if (a.type != career::events::AddActivity::contract_event || !a.player || progressive::follow_mode() != 1)
            return false;
        return progressive::sign_instead(a);
    });
```

## Operations

```cpp
career::set_slots("f1_career_progressive", { 476, 477, 478, 484, 480, 481, 479, 482, 485, 483, 486 });
career::apply_lineup(*career::seasons.get(career::seasons["swofty.progressive/2017"]), &report);
career::set_driver_ratings(drivers::identities["game/lando_norris"], { .experience = 70, .racecraft = 88, .awareness = 85, .pace = 90 });
career::swap_drivers(drivers::identities["game/alex_albon"], drivers::identities["game/logan_sargeant"]);
career::report_season(2017, 1);
```

The calls run on the core's career operations table (`SwoftyCoreCareerOps`, `SwoftyCareerOps` in `swofty_api.h`, `SWOFTY_CAREER_OPS_VERSION` 7). A call returns `unavailable` on a core whose table is older than the version that added it.

| Table version | Adds | Header call |
| --- | --- | --- |
| 1 | `available`, `driver_stat`, `set_slots`, `apply_lineup`, `set_driver_ratings`, `swap_drivers` | `career::set_slots`, `career::apply_lineup`, `career::set_driver_ratings`, `career::swap_drivers` |
| 2 | `report_season` | `career::report_season` |
| 3 | `apply_teams` | `career::apply_teams` |
| 4 | `set_seat`, `set_car_number`, `set_track_fuel` | `career::set_seat`, `career::set_seat_of_team`, `career::set_car_number`, `tracks::set_track_fuel` |
| 5 | `set_logo` | `teams::set_logo` |
| 6 | `set_look` | `drivers::set_look` |
| 7 | `current_round`, `team_stat`, `set_dropped_scores` | `career::current_round`, `career::set_slots` (by team references), `career::set_dropped_scores` |

`set_seat` takes a 1-based `slot` of the loaded career's data group, `set_seat_of_team` a team stat id; `set_car_number` takes `swap`, which gives the driver who held the number the caller's old one. `set_driver_ratings` writes experience, racecraft, awareness and pace by game stat id 0, 1, 3 and 4.

- **Slots:** a season's `slot` is one of the data group's career team slots. `set_slots` names the team each slot races as, by team stat id, and `apply_lineup` reads the table of the season's `data_group`. Without a table every seat is refused.
- **`apply_teams`:** each slot the season lists (1-12) races as its team; a team with no career rows gets them, built from its donor's. An empty team leaves the slot out of the Championship, and a slot the season doesn't list keeps its team. Call it before `apply_lineup`. It reads only the records of the season's `data_group` and dresses only the rows of that group's slot table. A dressed slot row's loaded team record takes the season team's names, code, colours, base and principal (`team.identity` 1.1); its logo stays the slot row's. When the career unloads, every dressed record goes back to its own. Twelve slots work in Driver Career; twelve with My Team are refused (see [A season's teams](/std/systems/teams#a-season-s-teams)).
- **Added teams in the new-career wizard:** a season whose slot names a team an addon made (its own `team.identity`, not a `game/` row) is an added team. The career's team rows are the ones its career list holds for every start season, so the added team wears its slot's row: the request `{"start": {"data_group": "<group>", "year": <start season>}}` dresses those rows (names, colours) while the wizard shows that start season, and `"year": 0` takes the dress off. The season start dresses the same row the same way, so the career never gets a second row for the team. At boot, each new driver (a `driver.identity` with a donor) of an added team gets a teamless entrant in the first teamless list the career list references, so the career has a row to seat him in.
- **`set_dropped_scores`:** while the loaded career is filed under the data group, the drivers' championship counts each driver's best `best` results; 0 counts every result. The cumulative totals, and so the order, use the counted sum of the best per-round totals, taken from the per-session points, which stay gross. The countback of finishing positions still reads every result, and the constructors' championship counts every result.
- **`set_seat`:** the driver takes race seat 0 or 1 of a slot in Driver Career. A race driver exchanges seats with the seat's driver; a free agent takes the seat and the seat's driver leaves the grid.
- **What `apply_lineup` changes:** each seat's driver moves to it; a race-seat driver whose seat goes to someone else, and who has no seat in the record, leaves the grid. Mid-season `changes` aren't applied by it; `career::apply_round` applies them (see below).
- **What it refuses:** a driver not in the drivers table, a driver without a single career row, a driver named on two seats (the second), the player (never moved) and the player's seat (never given away). The refused seats keep their drivers.

- **Mid-season changes:** `career::current_round()` (table version 7) says which race weekend the loaded career is at: `round` is 1-based and is the next race to run, `rounds` the season's weekend count. It returns nothing while no career is loaded, the season has no built weekends or the career is busy, so call it at an idle Team HQ (`career.hub`). `career::seats_at(season, round)` is the season's seats at that round, from the season's `changes` with `round` up to that round: `replace` puts `in` on the seat for the rest of the season, `swap` puts `in` on the seat and gives his old seat to the seat's previous driver, `move` puts `in` on the seat and leaves his old seat empty until another change fills it, and `stand_in` puts `in` on the seat from its round up to (not including) `until_round`, one race when it is absent, then the previous driver returns. `career::apply_round(season, round)` runs `apply_lineup` on those seats, so a seat already right is unchanged, the player's seat is never given away and the player is never moved. A driver an earlier round released has no seat and takes the next one as a free agent.
- **Team references:** `career::set_slots(data_group, teams)` takes team references (`game/<label>`, or an addon's `team.identity` id) and resolves them to the teams table's stat ids (`team_stat`, table version 7).

A batch of seat changes follows the game's own sequence, in this order:

1. for each driver who leaves the grid, his contract set to state 3;
2. for each driver who moves or leaves, `MessageDriverMoveConfirmed`, then his entrant write;
3. every row read back against the plan, and every write undone on a mismatch;
4. each seated driver without a contract for his team signs one: state 4 or 5, then `MessageDriverContractChanged`;
5. a number for each seated row without one;
6. the line-up changes against the line-up from before the batch, then the three moves-finalised posts (upgrades, recognition, plain).

Any other order crashes the career. Every operation runs on the flow thread.

```text
[career-ops] apply_lineup 2017 (season, f1_career_progressive): 1 seat(s) set, 21 unchanged, 0 refused; 2 row(s) written,
             1 contract(s) signed, 0 number(s) drawn, finalise posted (2 line-up change(s)): move fernando_alonso(7)
             mclaren_26(484) seat 1 -> mercedes_26(476) seat 1; release lewis_hamilton(5) released
```

## Saves

Saves are experimental, since 1.7.0. The core keeps an index of the careers it has loaded, and each addon's own data for each career, in the launcher's data folder. Careers are listed there with the addons they need, so the launcher can warn before Play and tell the career vault which careers an addon toggle would drop.

```cpp
#include <swofty/systems/career.hpp>

namespace saves = swofty::career::saves;

static void remember(const swofty::events::Context &)
{
    auto mine = saves::data("swofty.progressive");           // this addon's data for any career
    auto here = saves::current();                            // "f1_career_progressive:7731"

    saves::need("mod.sochi", "^1.1.0", "calendar round 4");  // the loaded career needs this addon
    saves::describe({.driver = "Max Verstappen", .team = "Red Bull", .season = 2016, .round = 7});

    mine.set(here, swofty::json::parse(R"({"follow":"max","years":[2016,2017]})"), 2);   // version 2
    if (auto stored = mine.get(here); stored && stored->version < 2)
        migrate(stored->value);                              // the addon migrates its own versions
}

swofty::events::on<career::events::Saving>().before(
    [](auto &, const swofty::events::Context &c) { remember(c); });
swofty::events::on<career::events::Missing>().after(
    [](const auto &, const swofty::events::Context &) {
        for (auto &gone : saves::missing())
            log("career %s is in the index and not in the save", gone.career.c_str());
    });
```

### The index

`$DATA/careers/index.json` is the career index of [dependencies](/std/dependencies#careers-remember-what-they-were-made-with), with the fields the core writes beside the dependency record. A field the launcher or an older core wrote stays as it was.

```json
{
  "version": 1,
  "careers": [
    {
      "career": "f1_career_progressive:7731",
      "mode": "driver_career",
      "content": "f1_career_progressive",
      "content_record": "swofty.progressive/f1_career_progressive",
      "driver": "Max Verstappen",
      "team": "Red Bull",
      "season": 2016,
      "round": 7,
      "label": "f1_career_progressive: Max Verstappen, Red Bull, 2016",
      "last_saved": 1793443200000,
      "status": "present",
      "addons": [
        { "id": "swofty.progressive", "why": "career content f1_career_progressive" },
        { "id": "mod.sochi", "need": "^1.1.0", "why": "calendar round 4" }
      ]
    }
  ]
}
```

- **`career`:** the career's id, `<mode>:<slot guid>` (see [When a season starts](#when-a-season-starts)).
- **`mode` and `content`:** the mode kind and the data group the career loaded with. `content_record` is the `career.content` record that owns the group.
- **`driver`, `team`, `season`, `round`:** what the core knows when it writes the record, which is the career's year and round once the hub has run. `saves::describe` sets them from an addon that knows better, and a described value stays for the session. A field nobody knows is left out.
- **`last_saved`:** milliseconds since 1970 at the last `career.saved` of the session in which the career was loaded.
- **`status`:** `present`; `deleted` when the game listed the career's save slot earlier in the session and no longer does; `missing` when a profile load or save did not find it and the session had not listed it.
- **`slot`, `slot_name`, `slot_season`, `slot_weekend`, `last_played`, `hidden`, `driver_raw`, `team_raw`, `vehicle_raw`:** what the game's save slot holds, the fields the Resume Career screen reads. `slot_season` is the season number the game shows, `last_played` is Unix seconds, `hidden` is set for a slot the game keeps out of that screen, and the `_raw` fields are the slot's driver, team and vehicle values. A career that was never loaded has no `driver` or `team` text.
- **`addons`:** the owner of the career's data group, the `needs` of its content record and every `saves::need` call. The launcher reads this list; a version range is optional.
- **When it is written:** after each profile load and each profile save the core lists the save slots of every game mode and adds, updates or marks every career in the index; it also writes at `career.loaded` and before each profile save. A career whose slot the game lists but the session never loaded gets a record from the slot list (`<mode>:<slot key>`), and `career.loaded` renames it to the loaded id.

### Per-career addon data

`saves::data(addon)` is an addon's own JSON for a career, in `$DATA/careers/data/<career>/<addon>.json`. The file holds the addon's version number and the value:

```json
{ "version": 2, "data": { "follow": "max", "years": [2016, 2017] } }
```

- **Versions are the addon's.** `set(career, value, version)` stores the number and `get` returns it with the value. The core never migrates.
- **A career the index does not know** is refused (`Result::refused`), so data never outlives a career's record by being written first.
- **Size:** 8 MB a file. The vault moves the `careers` folder with the save it belongs to.
- **Progressive's `careers.json` and the Career Mode Editor's edits** can move into this store. They keep their own files until their addons change.

### Events

| Event | Kind | Fires |
| --- | --- | --- |
| `career.saving` | modify | before the profile's save containers are written (`site_Game_SaveData_ContainersCount`); a handler sets the loaded career's data |
| `career.saved` | notify | after the containers block was written with no error (`site_Game_SaveData_ContainersDone`) |
| `career.missing` | notify | after a profile load, a career the index knew has no slot in any mode (see below) |

`career.saving` and `career.saved` fire only while the save splice is armed, on the thread that writes the profile. A handler stays short and uses no game calls. Both carry no payload: `saves::current()` names the career.

`career.missing` fires at the end of a good profile load, on the loading thread, when a career of the index has no slot in the `SaveSlotManager` of any mode after `Game_SaveSlotManager_Deserialise`. The core marks those careers missing in the index and `saves::missing()` lists them. A career is matched by its slot guid, or its slot id when the slot has no guid.

### Requirements

A career content declares the addons its careers need beyond its own:

```json
// registry/career.json
{
  "career.content": {
    "f1_career_progressive": {
      "data_group": "f1_career_progressive",
      "needs": [
        { "id": "mod.sochi", "range": "^1.1.0", "why": "calendar round 4" },
        { "id": "swofty.rnd-tyres-thermal", "why": "the Tyres R&D department" }
      ]
    }
  }
}
```

Before Play, the launcher checks every career in the index against the profile's addons. A career whose needed addon is off, missing or the wrong version adds one line to the Play log and a warning to the notice that already shows before Play:

```text
2 careers won't load in this profile: Progressive: Max Verstappen, 2016 needs Sochi Autodrom (mod.sochi), which is off; Driver Career: Lando Norris needs R&D: Tyres & Thermal Management (swofty.rnd-tyres-thermal), which is off
```

The same list tells the vault which careers an addon would drop: turning an addon off names every career whose `addons` lists it.

### In tests

`swofty/fake_saves.hpp` stands in for the core's table, so an addon's tests call `career::saves` without the game:

```cpp
swofty::fake::Saves saves;
saves.install();
saves.load("f1_career_progressive:7731", "driver_career", "f1_career_progressive");
remember(ctx);                                    // the addon's career.saving handler
auto stored = career::saves::data("swofty.progressive").get_current();
saves.drop("f1_career_progressive:7731");        // the next missing() lists it
```

The template `templates/std/career-saves` is such an addon with its tests.

## Readiness and limits

- **The most ready system:** 59 career and 46 game functions at L2, saves at L3. Ready: contracts, line-ups, standings, resource points, upgrade start, cancel and cost, the calendar, activities, the wizard step, save containers.
- **Partial:** Team HQ tabs (L1). Contracts, R&D departments, facilities, sponsors and the economy are described under "Described, in headers that come later".
- **Unknown:** the resource point spend, regulation changes, a 13th team tree.
- **The save heap** is 3 MB. The core keeps career snapshots outside it, in the `swofty_registry` entry, with a budget per registry and per career.
- **R&D:** four departments drawn natively; upgrade types are a table of 19 and a `std::bitset<19>` (ids of 19 and up throw without native code).
- **The 10-season career limit** is calendar data (`_season_instancecount_`), not code. Career days are 16-bit (about 200 seasons). Stock retirement schedules end in December 2035.
- **Career saves** (since 1.7.0) are experimental: `career.saving` and `career.saved` run on the profile-writing thread, and `career.missing` on the profile-loading thread.
- **Saves keep the season list.** Rounds of a season that hasn't started can be replaced, added or removed. The upgrade loader drops instances it doesn't know.
- **Hashed per-team career tables** can't gain rows. **Eight tables** are partitioned by data group.
- **My Team** freezes when the upgrade availability is refreshed over and over. **Follow-career scene paths** crashed new Progressive careers (#411).
