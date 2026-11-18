---
title: Teams
description: Teams in code - team identity, shared constructor ids, logos per context and pit times as typed registries, with the logo and name events and the team operations.
---

# Teams

Rename teams, change their colours and logos, or add a team the game doesn't have. Team Names, for example, lets the player rename Mercedes and pick its colours on a launcher page, and its code applies those choices as edits on the stock `teams::Identity` record.

A few terms used on this page:

- A **team** is a `teams::Identity` record: one row in the game's `entrants.teams` table, with its names, colours, logo and the side data the game finds by its label.
- A **constructor** is a real Formula 1 team, with an id under `constructor/` that the core owns. Every pack that adds the 2017 Toro Rosso says its team is `constructor/toro_rosso`.
- A **logo** is a `teams::Logo` record: one image for each place a logo shows.
- A **layer** changes values on a record that already exists. A layer can be **profile-scope**: it applies only in the launcher profile that set it.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/teams.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::teams {

using Result = swofty::Result;

enum class LogoContext : int32_t { driver_select, team_select, hud, broadcast, hub };

struct Constructor {                               // a real team; the core owns the constructor/ ids
    static constexpr Version version{1, 0};
    std::string name;                              // "Scuderia Toro Rosso"
    std::string country;                           // "italy"
    Ref<Identity> team;                            // resolved: the last enabled team in load order that is this constructor
    json::Value extra;
};
SWOFTY_RECORD(Constructor, name, country, team)

struct Colours {
    Colour menu;                                   // ui_team_colour: menus and the timing tower
    Colour primary;                                // primary_colour
    Colour secondary;                              // secondary_colour
    Colour tertiary;                               // tertiary_colour
};
SWOFTY_RECORD(Colours, menu, primary, secondary, tertiary)

struct Names {
    Text title;                                    // display_name
    Text short_name;                               // display_name_short
    Text super_short;                              // display_name_super_short
    std::string code;                              // display_name_code: "MER"
};
SWOFTY_RECORD(Names, title, short_name, super_short, code)

struct Logo {
    static constexpr Version version{1, 0};
    std::map<LogoContext, Path> images;            // one image per context; a slice each, not a whole array
    json::Value extra;
};
SWOFTY_RECORD(Logo, images)

struct Identity {                                  // one entrants.teams row
    static constexpr Version version{1, 1};
    Ref<Identity> donor;                           // the row it copies, all 30 keys with story_info (new teams)
    Ref<Constructor> constructor;                  // the real team it is; empty: a fictional team
    std::vector<Ref<Constructor>> provides;        // the constructor/ ids this team provides (shared ids)
    Names names;                                   // the team's names
    Colours colours;                               // the team's colours
    std::optional<Colours> wear;                   // team_wear_*: empty follows colours (the kit follows primary)
    std::string livery_name;                       // the label HUD badges are found by
    Ref<Logo> logo;                                // its logos
    std::string audio_id;                          // audio_id
    int prestige = 0;                              // prestige
    std::string nationality;                       // nationality
    StatId stat_id = StatId::automatic;            // team ids run to 16382
    std::string team_boss;                         // team_boss: the principal, an entrants.characters label ("toto_wollf")
    Text team_base;                                // team_base: the base, a lang key or a literal (1.1)
    json::Value extra;
};
SWOFTY_RECORD(Identity, donor, constructor, provides, names, colours, wear, livery_name, logo, audio_id, prestige,
              nationality, stat_id, team_boss, team_base)

struct PitTimes {                                  // one pit_lane.pit_times row, found by team label
    static constexpr Version version{1, 0};
    Ref<Identity> team;
    Ref<PitTimes> from;                            // copy another team's row
    json::Value extra;
};
SWOFTY_RECORD(PitTimes, team, from)

inline Registry<Constructor> constructors{"team.constructor", { .applies = Applies::boot }};
inline Registry<Identity> identities{"team.identity", { .applies = Applies::boot }};   // names and colours also on read
inline Registry<Logo> logos{"team.logo", { .applies = Applies::read }};
inline Registry<PitTimes> pit_times{"team.pit_times", { .applies = Applies::boot }};

Result set_logo(Ref<Identity> team, Ref<Logo> logo);     // from now on, every context
Result set_name(Ref<Identity>, Names);                   // the loaded team's names; unavailable
Result set_colours(Ref<Identity>, Colours);              // needs the Sess_Load_Teams record layout; unavailable

}

namespace swofty::teams::events {

struct LogoResolve {                               // the two logo format sites and the HUD badge: before
    static constexpr const char *point = "team.logo.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    LogoContext context;                           // where the logo shows
    const char *team_id;
    const char *livery_name_str;
    const char *key_str;
    int32_t (*set_key_fn)(void *user, const char *key);
    void *user;

    Ref<Identity> team() const { return swofty::detail::payload_ref<Identity>(team_id); }
    std::string_view livery_name() const { return swofty::detail::payload_str(livery_name_str); }
    std::string_view key() const { return swofty::detail::payload_str(key_str); }
    Result set_key(std::string_view key)
    {
        if (!set_key_fn)
            return Result::unavailable;
        std::string k(key);
        return swofty::detail::result(set_key_fn(user, k.c_str()));
    }
};

struct NameResolve {                               // display-name lookups: before (needs RE)
    static constexpr const char *point = "team.name.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    const char *team_id;
    const char *name_str;
    bool name_literal;
    int32_t (*set_name_fn)(void *user, const char *name, bool literal);
    void *user;

    Ref<Identity> team() const { return swofty::detail::payload_ref<Identity>(team_id); }
    Text name() const
    {
        std::string_view s = swofty::detail::payload_str(name_str);
        return name_literal ? Text::literal(s) : Text::key(s);
    }
    Result set_name(const Text &name)
    {
        if (!set_name_fn)
            return Result::unavailable;
        return swofty::detail::result(set_name_fn(user, name.str().c_str(), name.is_literal()));
    }
};

}
```

</template>
</Example>

## Team Names, in code

<Split>

Team Names lets the player rename teams and change their colours from a launcher page. The page writes the player's choices to the addon's own storage. The addon's code reads them in the game and applies them as profile-scope edits on the stock team records.

- **A name is written as text** in every language (`Text::literal`), and the core makes the lang key.
- **The kit follows the primary colour** unless `wear` is set.
- **Classic Career and Progressive change the same 2026 teams.** Load order decides each field, and the conflicts report names both addons.

<template #example>
<Example root="custom.team-names" code-file="native/src/names.cpp" data-file="registry/layers.json" files="swofty.json, bin/team-names-helper.exe, launcher/main.go">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/storage.hpp>
#include <swofty/systems/teams.hpp>

#include "choices.hpp"   // parses the page's choices.json

using namespace swofty;

SWOFTY_REGISTER
{
    std::string json;
    if (storage::read("choices.json", json) != storage::Result::ok)
        return;

    for (const team_names::Choice &c : team_names::parse(json)) {      // { team, title, short, tag, code, colours }
        teams::identities.layer(teams::identities["game/" + c.team], [c](teams::Identity &t) {
            if (c.title)  t.names.title = Text::literal(*c.title);
            if (c.short_) t.names.short_name = Text::literal(*c.short_);
            if (c.tag)    t.names.super_short = Text::literal(*c.tag);
            if (c.code)   t.names.code = *c.code;
            if (c.colours) t.colours = *c.colours;
        }, Scope::profile);
    }
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/layers.json: the same choices, written by hand
{
  "team.identity": {
    "game/mercedes_2026": {
      "names": { "title": { "text": "Mercedes-AMG Petronas" }, "super_short": { "text": "Mercedes" }, "code": "MER" },
      "colours": { "menu": "#00D2BE", "primary": "#00D2BE", "secondary": "#0C0C0C", "tertiary": "#C8CCCE" }
    }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: custom.team-names (team_names.dll) 3 layers on team.identity (early stage, 0.4 ms)
[registry] team.identity game/mercedes_2026: edit by custom.team-names (profile) changed /names/title, /names/super_short,
           /colours/primary -> #00D2BE
[registry] team.identity game/racing_bulls_2026: custom.team-names and career.classic-career both set /names/title;
           career.classic-career wins (later in load order)
[registry] applied at boot: team.identity 3 (3 rows, 33 records; 6 strings in 11 languages)
```

## A new team

A new team copies a stock row. The core copies every key, `story_info` included, writes its pit times and allocates the stat id from the install's store:

```cpp
auto toro_rosso = teams::identities.add("toro_rosso", {
    .donor = teams::identities["game/tororosso"],
    .constructor = teams::constructors["constructor/toro_rosso"],
    .provides = { teams::constructors["constructor/toro_rosso"] },
    .names = { .title = Text::key("lng_cs_team_17_toro_rosso"), .code = "STR" },
    .colours = { .menu = 0xFF0A1E3C, .primary = 0xFF0A1E3C, .secondary = 0xFFC8102E },
    .livery_name = "cs17_toro_rosso",
    .logo = teams::logos["swofty.classic-seasons-2017/toro_rosso_2017"],
});
teams::pit_times.add("toro_rosso", { .team = toro_rosso, .from = teams::pit_times["game/tororosso"] });
```

A team's principal and base are `team_boss` and `team_base` (`team.identity` 1.1, launcher 1.6.2). `team_boss` names a row of `entrants.characters` by its label, and the game's loaded team record keeps that character's stat id. `team_base` is the base text, a lang key or a literal. A field the record leaves out keeps the donor's value. A 1.6.0 or 1.6.1 core keeps both in the record's `extra` and writes the donor's values.

```json
// registry/teams.json
{
  "team.identity": {
    "caterham": {
      "donor": "game/williams",
      "constructor": "constructor/caterham",
      "provides": ["constructor/caterham"],
      "names": { "title": { "literal": "Caterham F1 Team" }, "short_name": { "literal": "Caterham" }, "code": "CAT" },
      "colours": { "menu": "#00573F", "primary": "#00573F", "secondary": "#F2C230", "tertiary": "#FFFFFF" },
      "livery_name": "career2012_caterham",
      "logo": "creator.career-2012/caterham",
      "nationality": "malaysia",
      "team_boss": "toto_wollf",
      "team_base": { "literal": "Leafield, United Kingdom" }
    }
  }
}
```

```cpp
auto caterham = teams::identities.add("caterham", {
    .donor = teams::identities["game/williams"],
    .names = { .title = Text::literal("Caterham F1 Team"), .code = "CAT" },
    .livery_name = "career2012_caterham",
    .team_boss = "toto_wollf",
    .team_base = Text::literal("Leafield, United Kingdom"),
});
```

```text
[teams] dress f1_career_2012 slot 10: game/williams_26 as creator.career-2012/caterham: names and colours written to the loaded record of stat id 486
```

## A season's teams

`career::apply_teams` gives each of a season's slots (1 to 12) its team. A slot whose row is the team's own row races as it is. A slot whose row is another team's row is dressed: the loaded team record of the slot row takes the season team's names, code, colours, base (`team_base`) and principal (`team_boss`, through the character row's stat id). Twelve slots work in Driver Career; twelve slots with My Team (13 teams) are refused.

- **Scoped to the data group.** `apply_teams` reads only the `career.season` records of the season's `data_group`, and dresses only the rows of that group's slot table (`career::set_slots`).
- **Put back on unload.** When a career unloads, the core puts every dressed record back in its own names, colours, base and principal. A loaded team record is shared by every career of the game session. The next `apply_teams` (Progressive's, at each idle Team HQ) dresses them again.
- **Logos follow the row.** The logo arrays are keyed by the team row's label. A dressed slot shows its slot row's logo; a team shows its own logo on a slot whose row is its own row.
- **A principal the game can't find** (no `entrants.characters` row with that label) leaves the record's principal and logs it.

```json
// registry/seasons.json: 2012 with 12 teams, each with its own record
{
  "career.season": {
    "2012": {
      "data_group": "f1_career_2012",
      "year": 2012,
      "teams": [
        { "slot": 1, "team": "creator.career-2012/red_bull", "seats": ["person/sebastian_vettel", "person/mark_webber"] },
        { "slot": 10, "team": "creator.career-2012/caterham", "seats": ["person/heikki_kovalainen", "person/vitaly_petrov"] },
        { "slot": 12, "team": "creator.career-2012/hrt", "seats": ["person/pedro_de_la_rosa", "person/narain_karthikeyan"] }
      ]
    }
  }
}
```

```text
[career-ops] apply_teams slot 10: team row williams_26 (stat id 486) is dressed as creator.career-2012/caterham
[teams] 12 dressed team record(s) back in their own names, colours, base and principal (the career unloaded)
```

## Real teams

A `constructor/` id works as a team reference: `teams::identities["constructor/toro_rosso"]` resolves to the last enabled team in load order that is that constructor. Career addons and line-ups name constructors, not one pack's keys, and they work with whichever pack provides the team. Each pack's team is still its own row with its own stat id.

The stock team rows, and the rows a season addon adds to `entrants.teams` through database ops, are `game/<row label>` records on every profile, whether or not any addon has team records. `stat_id` on one returns the row's own stat id.

A team says which constructor ids it provides in `provides`. The stock rows of the game's real teams provide their own ids (`game/williams` provides `constructor/williams`), so a pack's team that provides the same id wins by coming later in load order. When two enabled addons provide the same id, the core logs it and the conflicts report lists the loser (see [Shared ids](/std/providers#shared-ids)).

```cpp
// a 2017 season slot that takes Toro Rosso from whichever 2017 pack provides it
career::SeasonTeam slot = { .slot = 7, .team = teams::identities["constructor/toro_rosso"] };
```

```text
[registry] team.constructor constructor/toro_rosso: provided by swofty.classic-seasons-2017/toro_rosso
```

## Logos per context

<Split>

Each logo record holds one image per context. A context takes a single image of any size: the core scales it to the slice size of every array that context feeds (the team select context feeds two arrays of different sizes). The core builds the texture arrays from every logo record, slice by slice. Classic Seasons, car ports and season packs each change their own teams' slices, and all of them apply.

Progressive changes logos per season through `team.logo.resolve`. The game formats logos every frame on the HUD and in menus. The hook point runs when content becomes active or a session loads, once per team and context. The core builds a table from the results, and the game reads the table each frame.

<template #example>
<Example root="swofty.progressive" code-file="native/src/logos.cpp" files="swofty.json, logos/williams_2017_driver_select.png">
<template #code>

```cpp
#include <swofty/events.hpp>
#include <swofty/systems/teams.hpp>

using namespace swofty;

SWOFTY_ON_LOAD
{
    events::on<teams::events::LogoResolve>().in(events::owner("swofty.progressive"))
        .before([](teams::events::LogoResolve &l, const events::Context &ctx) {
            auto key = progressive::season_logo(l.team(), ctx.career()->year, l.context);
            if (!key.empty())
                l.set_key(key);
        });
}

SWOFTY_ADDON()
```

</template>
</Example>
</template>
</Split>

```text
[events] team.logo.resolve (driver_select, game/williams, 2017): swofty.progressive -> progressive_williams_2017
```

```cpp
teams::set_logo(teams::identities["game/williams"], teams::logos["swofty.progressive/williams_2017"]);
```

## Readiness and limits

- **Partial.** The logo format sites (`site_Game_DriverSelect_TeamLogoFormat`, `site_Game_TeamSelect_TeamLogoFormat`) are L2 and shipped. The HUD badge builders are L1. The team record layout behind `Sess_Load_Teams` is L1, and `set_colours` waits on it.
- **Unknown:** a single logo-key resolver (logos are formatted at several sites) and the display-name lookups behind `team.name.resolve`.
- **Stat ids:** teams use ids up to 16382 (14 bits in the game's team keys, 0x3fff = none), from the install's store.
- **A new team row needs all 30 keys,** `story_info` included, or the game reads garbage.
- **Side data** is found by team label. The core copies the donor's rows, relabelled, in every data group that holds one: pit times, initial cash, facilities, driver market, recognition, renown and form, primary sponsors and showroom history. Team customisation per season (suit, gloves) belongs to the wearables records, animation team data follows the vehicle, and the team icons and story standings are not copied.
- **Constructors:** a `team.constructor` record names the team that is that constructor, the last enabled one in load order. A team reference by constructor resolves to that team's row label.
- **Logos:** at boot the core reads the current `ui_package` archives of the game folder (so other addons' edits are kept), encodes each `team.logo` PNG to a BC7 slice (`apply/content/bc7.hpp`, byte for byte the output of Classic Seasons' `logos.py`), appends the slices and keys to the stock texture arrays and to the key lists of `overrides.uidb`, and writes the changed archives to `swofty/addons/swofty.logos/` (named by a hash of the records, the PNGs and the archives, so an unchanged boot reuses them). The archives the build reads, `databases.erp` and the archives that carry a changed texture array, count by a hash of their bytes; every other archive counts by its path and size and is not read. `inputs.stamps` in that folder names the archives the last build read and keeps each hash under the size and write time it was taken at, and only one of those whose size or write time differs is read again (`team.logo: N input archive(s) (M MB) hashed in T ms`); an archive copied in again with the same bytes keeps the key. A game update that changes an archive the build reads, or the size of any other archive, builds the logos again. The game's opens of those archives are redirected to the copies through the file route; nothing in the game folder changes. An array that holds more slices than keys, because addons appended or replaced slices without listing keys, gets placeholder keys (`swofty_pad_<slice>`) for the extra slices first, so every new key sits on its own slice; the build logs the placeholders (`team.logo: <array> holds N slices for M keys ...`), the slice each new key lands on (`team.logo: <array> slice N = <key> (copy of <donor>, slice M)`) and, on an unchanged boot, `team.logo: N cached archive(s) of <id> reused`; the core copies these lines into loader.log with an `[apply]` prefix. A logo whose image is the stock key of another team copies that team's slice. A context takes one image. A context the record gives no image takes the team's own image of another context (a timing tower array the other tower context's first, every other array the driver select image first) and logs `team.logo: <team> uses its <context> image for <context>`; a team with an image file gets its own timing tower key. Only a team with no image file at all takes its donor's slice for the contexts it gives nothing for (`team.logo: <team> uses <donor>'s <context> logo`). When it feeds arrays whose slices differ in size, the core scales the image to each array's slice size, keeping its aspect ratio, centred on transparent padding (area average when shrinking, bilinear when growing, on straight alpha), and logs `team.logo: <key> scaled WxH -> SxS for <array>`; an image already of that size is used as it is. `set_logo` records the team's logo for the session (career operations table version 5).
- **Logo routes per context:**

| Context | Array | Route |
| --- | --- | --- |
| driver_select, hub | `teams.driver_select_team_logos` (23 archives) | grown array, one slice and one key per team label |
| team_select | `team_logos.driver_career_2024_team_select` (4 archives), `teams.mp_team_select_logos` | grown array |
| hud | `osd.timing_tower.team_badges_hud` (1 archive) | grown array, key `timing_tower_texture_path` |
| broadcast | `osd.timing_tower.team_badges_broadcast`, `teams.badges_broadcast` (8 archives), `broadcast_ui_generic.grid_team_logos` | grown array, key `timing_tower_texture_path` for the timing tower |

- **Time Trial tower badge:** a Time Trial session reads the timing tower badge from the vehicle's own team row, which the core copies from the base vehicle's team row. When a `content.package` team entry names both that vehicle and a `team.identity`, and the identity's logo gives a timing tower badge (a `hud` or `broadcast` file, or a stock key for either), the copied row takes that badge key as its `timing_tower_texture_path` and logs `vehicle team <label>: ... timing tower badge <key> from <identity>`. A vehicle no package entry pairs with an identity keeps the base row's badge.
- **Applied in game:** a `team.identity` record built its row from a donor with side rows, and five logo slices were built into 33 archives and routed. A team shows in team select through a content package that lists it; a team in no package has no tile. Career with a new team (`career::apply_teams`) has not been run in game.
- **Logo timing:** the redirect has to be installed before the game first opens a changed archive. The file route logs `the game opened this path before` when it was not; that case is not yet handled with a fallback to the donor's badge. The `eaid://` texture route (`TeamRecord.show_custom_badge_in_UI` with an added `ui_package/art/textures/<name>` resource) fits the HUD and broadcast pills only and is not run in game.
- **Dressing:** `career::apply_teams` dresses a slot row as the season's team through the team applier. The names, code, colours, base and principal are written into the loaded team record (`TeamRecord.team_base` +0x3a8, `team_boss` +0x581, L2) once the core reaches the entrants system's team slots; until then the request is queued and the report counts it as waiting. A career unload puts every dressed record back (see [A season's teams](#a-season-s-teams)).
- **Grid:** `team_position` and garages limit how many teams a session holds. A track whose world has fewer garages than a box has teams is left out of that box (see [Tracks](/std/systems/tracks)).
- **Colours** are signed ARGB in the database, and the core converts them.
