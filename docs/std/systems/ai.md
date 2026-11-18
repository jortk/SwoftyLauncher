---
title: AI
description: AI vehicle configs, package difficulty maps, baked track packs, global AI settings and difficulty in code, with the AI config events.
---

# AI

With the AI system you change how AI cars race, which baked track data each car drives on, and the global AI settings. AI Racing Tweaks, for example, makes every F1 car keep closer and lose control more like a person.

A car's **AI config** is its race-craft block: aggression, the space it keeps, its steering and pedal controllers. A **track pack** is the folder of baked AI data a car drives on at each track.

::: info In 1.6.0
The header `swofty/systems/ai.hpp` has the `ai.controller.update` payload `ai::events::ControllerUpdate` and nothing else. The registries (`vehicle_configs`, `package_maps`, `track_packs`, `general`, `difficulty`), `ConfigLoad` and the operations ship later; those sections describe that design. `ai.controller.update` needs in-game check T145.
:::

## The header

<Split>

`swofty/systems/ai.hpp` has five registries:

- `ai::vehicle_configs`: the race-craft block of one car (`ai.vehicle_config`);
- `ai::package_maps`: difficulty maps per car package (`ai.package_maps`);
- `ai::track_packs`: which baked AI folder a car uses on each track (`ai.track_pack`);
- `ai::general`: the global AI settings, behaviours and driver variation (`ai.general`);
- `ai::difficulty`: difficulty scaling per career or session (`ai.difficulty_profile`).

<template #example>
<Example root="swofty-sdk" code-file="include/swofty/systems/ai.hpp">
<template #code>

```cpp
#pragma once
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/cars.hpp>
#include <swofty/systems/tracks.hpp>

namespace swofty::ai {

enum class Class { f1, f1_26, f2, classic, safety_car, other };

struct VehicleConfig {                         // entrants.vehicles.vehicle_N.ai_vehicle_config: 221 values in 30 groups
    Ref<cars::Vehicle> vehicle;                // the car it belongs to
    Class cls;                                 // "class" in data files; only ers_use, traction control, trailbraking and race-start rpm differ by class
    struct { float overtake_aggression, defend_aggression; } racecraft;
    struct { Curve vehicle_buffer; } buffer_space;                       // space kept from other cars, by speed
    struct { float proportional_gain, integral_gain, derivative_gain, output_smoothing_half_life; } steering_pid;
    struct { float time_to_half_rotate_steering_wheel; } steering_controller;   // reaction after a hit
    struct { float lateral_traction_slip_angle_cut_in_proportion; } pedal_controller;   // lifts off when sliding
    struct { float angle; } lost_control;                               // where it gives up the car
    struct { Curve grip_atc_strength; } traction_control;
    struct { float aero_speed_reduction_factor, burst_speed_reduction_factor; } damage;   // driving a damaged car
    // ... allow_to_pass, blocking, drafting, defending, overtake, ers_use, swerve, trailbraking, yielding, race starts
};
inline Registry<VehicleConfig> vehicle_configs{"ai.vehicle_config", { .applies = Applies::session }};   // read at session enter

struct PackageMaps {
    Ref<cars::Package> package;                // vehicle_package.ai_vehicle_package_config
    std::map<std::string, Curve> driver_parameter_maps;   // difficulty x characteristic
};
inline Registry<PackageMaps> package_maps{"ai.package_maps", { .applies = Applies::boot }};

enum class Slot { by_pace, fixed };

struct TrackPack {                             // a car points at one through cars::Vehicle::ai (AiSlot::track_pack)
    std::string folder;                        // ai_package_folder: environment_package/tracks/<t>/ai/<folder>/
    Slot slot = Slot::by_pace;                 // by_pace: the core picks the baked car closest in lap time
    Ref<cars::Vehicle> pace_of;                // by_pace: the car whose lap time it matches; empty: the car itself
};
inline Registry<TrackPack> track_packs{"ai.track_pack", { .applies = Applies::session }};

struct General {
    std::map<std::string, float> settings;     // ai.general_ai_settings (5)
    std::vector<std::string> behaviours;       // ai.behaviour_system.behaviours (5)
    std::map<std::string, Curve> variation;    // ai.driver_variation.in_race: 84 form curves
};
inline Registry<General> general{"ai.general", { .applies = Applies::boot }};

struct Difficulty {
    events::Selector within;                   // a career, a content, a session type
    float pace = 1.0f;                         // scale on the difficulty the player picked
};
inline Registry<Difficulty> difficulty{"ai.difficulty_profile", { .applies = Applies::read }};   // needs the slider found (unknown)

namespace events {

struct ConfigLoad {                            // Sess_Load_VehicleAiConfig
    static constexpr const char *point = "ai.config.load";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    StatId vehicle;                            // the car's stat id
    void *config;                              // the live AiHandlingConfig; after handlers may patch its nodes
};

struct ResolveSettings {                       // Ai_Resolve_GeneralSettings
    static constexpr const char *point = "ai.settings.resolve";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    General *settings;                         // before handlers may change it
};

} // namespace events

Result set_vehicle_config(Ref<cars::Vehicle> vehicle, Edit<VehicleConfig> edit);   // before session load
Result assign_track_pack(Ref<cars::Vehicle> vehicle, Ref<TrackPack> pack);   // before session load

} // namespace swofty::ai
```

</template>
</Example>
</template>
</Split>

## AI Racing Tweaks

<Split>

AI Racing Tweaks makes the AI race closer and lose control more like a person. It writes one layer with a selector over every car in the core tag `c/f1_cars`: stock cars, Classic Seasons, F1 1990, and cars added after it was written. A **selector** layers each matching record itself, so the tuning reaches every year (see [How changes reach seasons and careers](/std/inheritance)). It skips the fields a year pack protects, as the next section shows.

Each value comes from one of its options, and changing an option re-runs the edit. Its version for 1.5 and earlier names 599 stock cars one at a time (13,371 database operations), so cars other addons add never get its values.

<template #example>
<Example root="kizi.ai-tweaks" code-file="native/src/racecraft.cpp" data-file="registry/layers.json" files="swofty.json, native/CMakeLists.txt, native/src/contact.cpp">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/events.hpp>
#include <swofty/systems/ai.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto f1 = tagged(&ai::VehicleConfig::vehicle, cars::vehicles.tag("c/f1_cars"));   // f1 and f1_26 cars

    ai::vehicle_configs.layer(f1, [](ai::VehicleConfig &c) {
        c.buffer_space.vehicle_buffer = Curve::flat(50, 500, option<float>("spacing"));    // 0.35 .. 0, default 0.15
        c.pedal_controller.lateral_traction_slip_angle_cut_in_proportion = option<float>("throttle_cut");
        c.lost_control.angle = option<float>("give_up");

        if (auto steer = option<std::string>("steer_correction"); steer != "stock") {
            float k = std::stof(steer) / 100.0f;                                        // 75, 50, 25 % of the game's gains
            c.steering_pid = { -1.5f * k, -0.75f * k, -0.375f * k, 0.4f };
        }
        if (option<std::string>("traction_control") == "off")
            c.traction_control.grip_atc_strength = Curve::flat(0, 1, 0.0f);
    });
}

SWOFTY_ON_LOAD
{
    events::on<ai::events::ConfigLoad>().after([](const ai::events::ConfigLoad &l, const events::Context &) {
        kizi::contact::arm(l.vehicle);                     // the contact handling, in contact.cpp
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
{
  "ai.vehicle_config": {
    "$select": [
      {
        "where": { "class": ["f1", "f1_26"] },
        "patch": {
          "buffer_space": { "vehicle_buffer": "@v2@minx:50,maxx:500,miny:0,maxy:1,0:0.15,1:0.15" },
          "pedal_controller": { "lateral_traction_slip_angle_cut_in_proportion": 0.75 },
          "lost_control": { "angle": 0.3 }
        }
      }
    ]
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] register: kizi.ai-tweaks (kizi_ai_tweaks.dll) 1 selector layer on ai.vehicle_config (early stage, 0.2 ms)
[registry] ai.vehicle_config: selector of kizi.ai-tweaks matched 612 records (519 game/, 80 swofty.classic-seasons-*,
           13 swofty.f1-1990; reads options spacing, throttle_cut, give_up, steer_correction, traction_control)
[registry] option kizi.ai-tweaks/spacing changed: 612 ai.vehicle_config records re-resolved; applies from the next session load
```

The data version holds the default values only, and picks the same cars by class (`f1` and `f1_26`). Following the options takes code.

## Year packs protect their own values

A year pack tunes its cars to race like their year. It marks those fields as **protected** on its own records. Another addon's selector then skips the protected fields, unless the player turns on **Also change year packs** for that addon in its settings. A layer that names the record itself still competes by load order.

```cpp
// Classic Seasons 2016: its per-year race-craft is history, not a default
SWOFTY_REGISTER
{
    for (const auto &team : cs16::kTeams)
        ai::vehicle_configs.protect(ai::vehicle_configs[std::format("swofty.classic-seasons-2016/{}", team)], { "racecraft" });
}
```

```cpp
// another addon: every F1 car overtakes harder
ai::vehicle_configs.layer(tagged(&ai::VehicleConfig::vehicle, cars::vehicles.tag("c/f1_cars")),
                          [](ai::VehicleConfig &c) { c.racecraft.overtake_aggression = 0.85f; });
```

```text
[registry] ai.vehicle_config: selector of mod.aggressive-ai matched 612 records; /racecraft skipped on 11 records protected
           by swofty.classic-seasons-2016 (Also change year packs: off)
```

In a data file, the year pack lists the fields inside its own record: `"$protect": ["racecraft"]`.

## Track packs

The AI drives from baked tables per track, car package and car (acpd, brpd, ers_boost_data, vp_times and more; about 7 MB per team per track). EA's bake tools are compiled out of the game, so a new car borrows the baked set of a stock car. `Slot::by_pace` picks the one closest in lap time, and the car points at the pack:

```cpp
auto pack = ai::track_packs.add("ferrari", { .folder = "f1_2026", .slot = ai::Slot::by_pace });

cars::vehicles.layer(cars::vehicles["swofty.classic-seasons-2017/ferrari"], [=](cars::Vehicle &v) {
    v.ai = { .track_pack = pack, .asset_filename = "auto" };
});
```

```text
[registry] ai.track_pack swofty.classic-seasons-2017/ferrari: slot by pace -> f1_2026/ferrari (0.12 % off)
```

## General settings and difficulty

```cpp
ai::general.layer(ai::general["game/default"], [](ai::General &g) { g.settings["blue_flag_compliance"] = 0.8f; });

ai::difficulty.add("progressive", {
    .within = events::content("swofty.progressive/f1_career_progressive"),
    .pace = 1.02f,
});
```

## The AI controller each frame

Every frame, each AI car's controller decides its throttle, brake and steering. `ai.controller.update` runs around that decision, so several addons can shape the AI's inputs without cloning the controller's vtable or chaining each other by hand. AI Racing Tweaks and AI Drive both change these inputs today.

```cpp
namespace swofty::ai::events {

struct ControllerUpdate {                          // the AI controller's update, every frame for each AI car
    static constexpr const char *point = "ai.controller.update";
    static constexpr uint32_t version = 1;
    uint32_t size, payload_version;
    int32_t vehicle;                               // the car's vehicle index
    float throttle;                                // 0 to 1
    float brake;                                   // 0 to 1
    float steering;                                // -1 to 1

    cars::live::Car car() const { return cars::live::Car(static_cast<uint32_t>(vehicle)); }
};

} // namespace swofty::ai::events
```

```cpp
// read what the game decided for each AI car in races (an after handler gets the payload const)
events::on<ai::events::ControllerUpdate>().when(events::session_type({ events::SessionType::race }))
    .after([](const ai::events::ControllerUpdate &u, const events::Context &) { my_telemetry::brake(u.car(), u.brake); });

// drive the player's car with your own controller while the AI controls it
events::on<ai::events::ControllerUpdate>().replace([](ai::events::ControllerUpdate &u, const events::Context &) {
    if (!my_driver::owns(u.vehicle))
        return false;                          // the game's controller decides
    my_driver::drive(u);                       // fills throttle, brake and steering
    return true;
});
```

- **Phases:** a `before` handler sees the car before the game decides. A `replace` handler can decide instead of the game and returns true when it did. An `after` handler reads the inputs the game wrote; to change them, use a `before` or a `replace` handler. Handlers run by priority, then load order.
- **Every frame:** the point runs for every AI car each frame, so it dispatches directly with no context table, and handlers must be cheap. With no subscriber the core installs nothing.
- **One hook per controller class.** The core hooks the class's vtable once, so addons stop cloning it per controller. An addon made for 1.5 that clones a controller's vtable keeps working as it does today: its clone calls the game's code directly, outside the point, until the addon moves to the point.
- **Only AI cars.** The point runs for a car whose controller set is in AI mode, including the player's car while `sessions::set_control` has handed it to the AI. A handler gets no `events::Context` (the point builds none), so it reads the car through `u.car()`; `when()` and `in()` selectors still apply. A value a handler writes that is not a finite number is dropped.
- **Where it runs.** By default the core writes its dispatcher into slot 1 of the AI controller's vtable while the point has a subscriber, and puts the game's function back with the last one. The core's config key `ai_controller_site` set to `"apply"` runs the point instead as a mid-hook on the record-apply call in `Sess_ControllerSystem_ApplyInputs`: there the record already holds the game's decision, so `before` and `after` see the same values and a `replace` handler decides on top of them. The default follows in-game check T145.

## Racecraft per car, live

Each car has its own copy of its driver config. Its slots are `cars::live` fields (see [Car physics](./car-physics)), so a layer changes one car for as long as it is held. The blocking evaluator re-reads its four distance and fraction floats and eight more of the blocking keys on every tick; `start_blocking_intersection_time` and `merge_back_to_racing_line` have no reader that was found.

| Field | Group | Kind | Range | Stock |
|---|---|---|---|---|
| `ai.blocking.fraction_of_follow_speed` | blocking | float | 0 .. 4 | 0.975 |
| `ai.blocking.fraction_of_follow_speed_at_race_start` | blocking | float | 0 .. 4 | 0.875 |
| `ai.blocking.max_blocking_distance` | blocking | float | 0 .. 1000 | 18 |
| `ai.blocking.max_blocking_distance_at_race_start` | blocking | float | 0 .. 1000 | 18 |
| `ai.blocking.start_blocking_intersection_time` (off) | blocking | float | 0 .. 60 | 1.1 |
| `ai.blocking.start_blocking_maximum_time_to_corner` | blocking | float | 0 .. 60 | 8 |
| `ai.blocking.start_blocking_minimum_time_to_corner` | blocking | curve | 0 .. 60 | curve |
| `ai.blocking.time_to_corner_for_end_merge_rate` | blocking | float | 0 .. 60 | 3.5 |
| `ai.blocking.time_to_corner_for_start_merge_rate` | blocking | float | 0 .. 60 | 7 |
| `ai.blocking.end_blocking_time_to_corner_vs_offset` | blocking | curve | 0 .. 50 | curve |
| `ai.blocking.blocking_offset_from_car_behind` | blocking | curve | 0 .. 50 | curve |
| `ai.blocking.blocking_speed_differential` | blocking | float | 0 .. 100 | 0.6 |
| `ai.blocking.blocking_threat_time_modifier` | blocking | float | -10 .. 10 | 0.175 |
| `ai.blocking.blocking_variable_offset_fraction` | blocking | float | 0 .. 1 | 0.5 |
| `ai.blocking.avoidance_decrease_rate` | blocking | float | 0 .. 100 | 1 |
| `ai.blocking.merge_back_to_racing_line` (off) | blocking | curve | 0 .. 10 | curve |
| `ai.alongside.speed_difference_for_limited_zone_yielding` | alongside | float | 0 .. 500 | 3 |
| `ai.alongside.yield_overtake_rate` | alongside | float | 0 .. 100 | 1 |
| `ai.alongside.min_presence_within_buffer` (off) | alongside | curve | 0 .. 100 | curve |
| `ai.divebomb_defense.max_presence_alongside` (off) | divebomb_defense | curve | 0 .. 100 | curve |
| `ai.divebomb_defense.max_time_to_ignore` (off) | divebomb_defense | float | 0 .. 60 | 5 |
| `ai.divebomb_defense.max_track_offset` (off) | divebomb_defense | float | 0 .. 50 | 1.75 |
| `ai.divebomb_defense.max_track_radius` (off) | divebomb_defense | float | 0 .. 10000 | 160 |
| `ai.offset_variation.max_offset` | offset_variation | float | 0 .. 50 | 1.33 |
| `ai.offset_variation.offset_limit_at_corner_start` | offset_variation | float | 0 .. 50 | 0.2 |
| `ai.offset_variation.offset_limit_blend_distance` | offset_variation | float | 0 .. 1000 | 250 |
| `ai.offset_variation.half_width_scaling_graph` | offset_variation | curve | 0 .. 100 | curve |
| `ai.offset_variation.radius_scalar_graph` | offset_variation | curve | 0 .. 100 | curve |
| `ai.overtake.attacker_close_enough_car_lengths` | overtake | float | 0 .. 50 | 3 |
| `ai.overtake.braking_zone_abort_overtake_time` | overtake | float | 0 .. 60 | 0.6667 |
| `ai.overtake.close_range_car_lengths_to_continue_overtake` | overtake | float | 0 .. 50 | 2 |
| `ai.overtake.close_range_car_lengths_to_start_overtake` | overtake | float | 0 .. 50 | 1.5 |
| `ai.overtake.oppertunist_overtake_time` | overtake | float | 0 .. 60 | 0.25 |
| `ai.overtake.oppertunist_overtake_track_speed_difference` (off) | overtake | float | 0 .. 500 | 12 |
| `ai.overtake.danger_overtake_continue_time_adjustment` | overtake | float | -100 .. 100 | 0.75 |
| `ai.overtake.danger_overtake_full_heat_time_adjustment` (off) | overtake | float | -100 .. 100 | 0.5 |
| `ai.overtake.danger_overtake_initiate_time_adjustment` | overtake | float | -100 .. 100 | 0.5 |
| `ai.overtake.danger_overtake_pull_out_time` | overtake | curve | 0 .. 60 | curve |
| `ai.overtake.regular_overtake_continue_time_adjustment` | overtake | float | -100 .. 100 | 1.25 |
| `ai.overtake.regular_overtake_full_heat_time_adjustment` (off) | overtake | float | -100 .. 100 | 0.95 |
| `ai.overtake.regular_overtake_initiate_time_adjustment` | overtake | float | -100 .. 100 | 0.75 |
| `ai.overtake.regular_overtake_pull_out_time` | overtake | curve | 0 .. 60 | curve |
| `ai.overtake.overlap_percentage` | overtake | curve | 0 .. 100 | curve |
| `ai.overtake.concede_speed_diff` | overtake | curve | -500 .. 500 | curve |
| `ai.overtake.concede_slowdown_speed_diff` | overtake | float | 0 .. 500 | 1.75 |
| `ai.overtake.concede_time_vs_behind` | overtake | curve | 0 .. 60 | curve |
| `ai.space_seeking.added_buffer` | space_seeking | float | 0 .. 100 | 0.05 |
| `ai.space_seeking.speed_below_follow` | space_seeking | float | 0 .. 500 | 4 |
| `ai.space_seeking.first_speed_below_follow` | space_seeking | float | 0 .. 500 | 2 |
| `ai.space_seeking.car_in_front_needed` | space_seeking | int | 0 .. 32 | 2 |
| `ai.space_seeking.remember_crowed` (off) | space_seeking | float | 0 .. 600 | 4 |
| `ai.space_seeking.concede_overtake_blocked_by_crowed_time` | space_seeking | float | 0 .. 600 | 5 |
| `ai.space_seeking.total_queue_needed` | space_seeking | int | 0 .. 32 | 3 |
| `ai.space_seeking.distance_braking_point` | space_seeking | float | 0 .. 1000 | 75 |
| `ai.space_seeking.time_to_switch_sides` | space_seeking | float | 0 .. 60 | 2 |
| `ai.space_seeking.track_distance_needed` | space_seeking | float | 0 .. 1000 | 170 |
| `ai.space_seeking.track_abort_distance` | space_seeking | float | 0 .. 1000 | 50 |
| `ai.space_seeking.cars_moving_away_speed_to_ignore` | space_seeking | float | 0 .. 500 | 5 |
| `ai.space_seeking.can_start_track_radius` | space_seeking | float | 0 .. 10000 | 400 |

Fields marked off report unavailable: no AI behaviour re-reads them per tick (the divebomb defence rows are read only by the human-divebomb behaviour, `alongside.min_presence_within_buffer` and `space_seeking.remember_crowed` have no reader found, the opportunist speed difference is read behind a packed thunk, and nothing reads `blocking.start_blocking_intersection_time` or `blocking.merge_back_to_racing_line` per tick). Stock values are `default_driver`'s; every stock driver has the same.

`-` in Stock: the key catalog lists these keys by name only. `get` returns the game's value for the car.

- **Float and int.** The value is written as the game's own type. `space_seeking.car_in_front_needed` and `space_seeking.total_queue_needed` are int cells: the composed value is rounded to the nearest integer, and the core refuses a cell whose stock value is not an integer from 0 to 65536.
- **Curve.** A layer on a curve field makes the car's curve flat at the composed value: every x evaluates to it. `get` returns the car's curve at x = 0, which is also the base a layer composes on.

```cpp
using namespace swofty;
using cars::live::Cars;
using cars::live::Layer;

static std::map<uint32_t, Layer> lap_one;

SWOFTY_REGISTER
{
    events::on("race.start").after([](std::span<const std::byte>, const events::Context &) {
        for (auto &car : cars::live::cars())
            if (car.is_ai())
                lap_one[car.index()] = Layer::set(Cars::car(car), "ai.blocking.max_blocking_distance", 40.0f);
    });
    events::on<sessions::events::Lap>().after([](const sessions::events::Lap &l, const events::Context &) {
        lap_one.erase(l.car.index());
    });
}
```

Erasing the `Layer` puts the game's value back for that car. The values of the other cars and the game's database stay as they were.

- **Written to no database cell.** The core swaps the car's 8-byte slot in its AI record for a pointer to its own cell and restores the game's pointer at the last release.
- **Race start.** The `_at_race_start` pair is read until the race start state machine reaches its last state, the plain pair after it.
- **Readiness.** The fields answer `Result::unavailable` until the AI system is built and the KB rows `AiSystem.records_begin`, `AiCarRecord.vehicle_index` and `AiCarRecord.driver_config` have independent agree verify rows. Each field also needs the `AiDriverConfig.<group>_<key>` row of its own; the four `blocking` rows have one, the 55 others do not yet.
- **Re-read per tick.** The blocking evaluator (all the blocking keys but `start_blocking_intersection_time` and `merge_back_to_racing_line`), `overtake.overlap_percentage` and the two `alongside` yield floats re-read their slot on every call. For the other fields no consumer was found: whether the game caches the value is unproven, and the float, int or curve kind rests on the key name.
- **Not exposed.** `cede_position.*`, `exit_fallibility.*`, `race_start.*`, the remaining `alongside.*`, `offset_variation.*` and `overtake.*` keys (`danger_overtake_buffer` among them) are not fields.
- **Not verified in game.** Whether the first `session.lap` of a car ends lap 1.

## Events and operations

```cpp
// patch a live config at session enter, for one car
events::on<ai::events::ConfigLoad>().in(events::content("swofty.f1-1990/f1_career_1990"))
    .after([](const ai::events::ConfigLoad &l, const events::Context &) { f1990::soften_starts(l.config); });

ai::set_vehicle_config(cars::vehicles["game/williams_2025"], [](ai::VehicleConfig &c) { c.lost_control.angle = 0.4f; });
ai::assign_track_pack(cars::vehicles["swofty.f1-1990/mclaren_1990"], ai::track_packs["swofty.f1-1990/f1_1990"]);
```

## Readiness and limits

- **Data: ready.** The configs, maps and track packs are database rows and files.
- **Events: partial.** The loaders (`Sess_Load_VehicleAiConfig`, `Ai_Resolve_GeneralSettings`, `Sess_Build_AiSystem`) are L2. The decision code is L1 or unexplored.
- **Unknown:** where the 0 to 110 difficulty slider scales the controllers (so `ai::difficulty` has nothing to apply yet), the grip index source, and the AI path builder.
- **When a changed config applies needs reverse engineering.** The game parses the configs in a startup loop (`Sess_Startup_VehicleAiLoop`) into a map keyed by stat id. The core applies a changed config at the next session load. Whether the game rereads that map for a later session is unverified; if it doesn't, a change applies from the next game start. The same goes for a career season that moves a team to another year's car.
- **`ai.controller.update`: implemented, waiting on verification.** Slot 1 of the AI controller's vtable (`Sess_AiController_Update`, L2) writes the inputs into the record at `[controller+8]`: throttle at +0xC, steering at +0x14 and brake at +0x18 (the names are L1); the vehicle index is at `controller+0x18`. The core's hook binds only after an independent `agree` verify of the table, and whether the game calls slot 1 every frame is test T145, an in-game check. Until a binding resolves the point reports unavailable and its handlers do not run.
- **Baked tables cap pace.** A car faster than its baked set gets nothing from more grip. Physics changes must keep the car inside the baked envelope.
- **A missing AI folder hangs the load.** The core refuses a track pack whose folder isn't there. A box offers a track only where every car in it has AI data (see [Tracks](./tracks)).
- **An added team needs AI data at every track.** A team whose `ai.asset_data` row an addon copies from another team (SWOFTY 2013's Caterham from Lotus) reads its own `ai/<package>/<team>/` folder at each track. An addon ships it for the tracks it knows about. On an added track the build copies the donor team's folder for it ([Installing tracks](/reference/installing-tracks#the-track-archive)), so the team drives with the donor's tyre wear and pace there.
- **The AI table holds 30 cars.**
- **Simulated sessions** use the baked lap times, driver statistics and randomness, not physics or R&D.
- **Third-party natives** that patch raw addresses break on game updates and on the EA build.
