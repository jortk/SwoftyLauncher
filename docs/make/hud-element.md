---
title: Build a HUD element
description: Put your own dial in the race HUD with PNG sprites, a hud.element record, live state reads and leaves written from native code, and test it on the host without the game.
---

# Build a HUD element

You'll build a small dial for the 2026 car's HUD: a speed text, a gear text, an RPM sweep, an ERS bar and a pit lamp. The speed, the RPM and the ERS bar come from the game's own HUD values. The gear and the pit lamp are leaves your plugin writes from the live state. The finished addon is [`templates/hud-dial`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/hud-dial).

A HUD element has three parts:

```text
hud.element record + PNG sprites    build time    what the dial looks like and which value drives each part
swofty/live.hpp                     runtime       the cars, session, classification and gaps, read from the game
swofty/hud_leaves.hpp               runtime       your own values, published as typed leaves the record binds
```

## The folder

```text
hud-dial/
├── swofty.json
├── registry/hud.json            the hud.element record
├── art/                         base.png ring.png solid.png fill.png lamp.png
├── tools/make_art.py            writes the five PNGs, standard library only
└── native/
    ├── src/dial.hpp             the leaves and the per-frame logic
    ├── src/addon.cpp            hooks the frame callback
    ├── test/dial_test.cpp       host test over fake::Live and fake::Hud
    ├── test/run_test.sh
    └── build.sh                 zig c++ to native/hud_dial.dll
```

```json [swofty.json]
{
  "schema_version": 1,
  "id": "example.hud-dial",
  "name": "HUD Dial",
  "version": "0.1.0",
  "kind": ["ui", "native"],
  "compat": { "game": "f1_25", "game_builds": [">=25251167"] },
  "requires": [{ "id": "swofty.core", "version": ">=0.1.0" }],
  "experimental_features": ["telemetry"],
  "operations": {
    "hud": ["registry/hud.json"],
    "native": [{ "dll": "native/hud_dial.dll" }]
  }
}
```

`telemetry` is the feature that turns the live state on. An addon that draws only game values, with no `native` entry, needs neither it nor a DLL.

## The record

Sprites are PNGs whose sizes are multiples of 4, drawn at 3 pixels per HUD unit. `bounds` is where each sprite sits in the 640x360 HUD frame, with the origin at the dial centre and `v` pointing down. The record binds each part to a source:

```json [registry/hud.json]
{
  "hud.element": {
    "dial": {
      "dial": "f1_26",
      "stock": "park",
      "sprites": {
        "base":  { "png": "art/base.png",  "bounds": [-60, -36, 60, 36] },
        "ring":  { "png": "art/ring.png",  "bounds": [-24, -24, 24, 24] },
        "solid": { "png": "art/solid.png", "bounds": [-4, -4, 4, 4] },
        "fill":  { "png": "art/fill.png",  "bounds": [-40, 24, 40, 32] },
        "lamp":  { "png": "art/lamp.png",  "bounds": [-56, -30, -40, -22] }
      },
      "elements": [
        { "kind": "sprite", "id": "base", "sprite": "base", "layer": 20 },
        { "kind": "text", "id": "gear", "style": "17_bold", "align": "centre", "box": 12,
          "cap_height": 6, "at": [0, -6], "value": { "view_model": "view_model_data.hud_dial_gear" } },
        { "kind": "text", "id": "speed", "style": "OSD_24_bold", "align": "centre", "box": 30,
          "cap_height": 9.5, "at": [0, 10], "value": { "game": "speed_str" } },
        { "kind": "sweep", "id": "rpm", "mask_sprite": "ring", "fill_sprite": "solid",
          "pivot": [0, 0], "start": 225, "arc": [[0, 0], [1, -270]], "value": { "game": "revs" } },
        { "kind": "gauge", "id": "ers", "sprite": "fill",
          "path": [[-40, 28], [40, 28]], "value": { "game": "ers_stored" } },
        { "kind": "sprite", "id": "pit", "sprite": "lamp",
          "visible": { "view_model": "view_model_data.hud_dial_pit" } }
      ],
      "editor": {
        "preview": { "game.revs": 0.62, "game.speed_str": "212", "game.ers_stored": 0.75,
                     "view_model_data.hud_dial_gear": "6", "view_model_data.hud_dial_pit": true },
        "team_colour": "#27F4D2"
      },
      "variants": {
        "f2": { "dial": "f2", "remove": ["ers"] },
        "f1_25": {
          "dial": "f1_25",
          "add": [
            { "kind": "group", "id": "drs", "after": "base", "children": [
                { "kind": "sprite", "id": "drs_idle", "sprite": "lamp", "at": [48, -26] },
                { "kind": "sprite", "id": "drs_live", "sprite": "lamp", "at": [48, -26] } ] }
          ],
          "decisions": [
            { "id": "drs", "type": "mvcd", "game": "drs_pit_state", "initial": "invalid",
              "states": { "invalid": ["drs_idle"], "drs_live": ["drs_live"] } }
          ]
        }
      }
    }
  }
}
```

- `"game": "speed_str"`, `"revs"` and `"ers_stored"` are values of the dial's own view model. The build checks each against the tags the stock template binds.
- `"view_model": "view_model_data.hud_dial_gear"` is a leaf your plugin writes. A text binds a text leaf, `visible` binds a bool leaf. The build refuses a leaf bound with another type than the one it declares.
- `"stock": "park"` moves the stock frame, fuel and pit parts off screen. The F2 variant removes the ERS bar, because an F2 car has no ERS.
- A dial record that should show only in some sessions takes a record-level `"switch": { "view_model": "<leaf>", "default": false }` (schema 1.4): your native code writes the leaf, the stock dial shows while it is off or never written, and `"stock": "hide"` hides the stock parts only while it is on. See [HUD](/reference/ops/hud#stock-parts-variants-and-the-editor).
- The 2025 variant adds a DRS pill right behind the base in the draw order (`after`) and a decision of its own on `drs_pit_state`, a value the 2026 dial has no stock decision for. `initial` keeps the grey pill on screen until the value first fires. A variant may also `replace` an element of the base under the same id, and set `root_y` when its root must land on one exact spot.
- `editor.preview` is what the HUD editor shows, since the editor runs no template.

The full record format, element kinds, decisions and the rules the composer keeps are in [HUD elements](/reference/ops/hud).

## The sprites

`tools/make_art.py` writes the five PNGs with `zlib` and `struct`, so there is nothing to install:

```sh
python3 tools/make_art.py
```

Replace them with your own art at the same sizes, or at any size that is a multiple of 4.

## The leaves

`swofty/hud_leaves.hpp` declares each leaf once with its type. The core keeps the last value written, sends only changes, and refills the leaves after the game rebuilds its view model, so the plugin has no refresh timer.

```cpp [native/src/dial.hpp]
#pragma once
#include <optional>
#include <swofty/hud_format.hpp>
#include <swofty/hud_frame.hpp>
#include <swofty/hud_leaves.hpp>
#include <swofty/live.hpp>
#include <swofty/mixin.hpp>

namespace dial {

namespace hud = swofty::hud;
namespace live = swofty::live;

struct Leaves {
    hud::Leaves L{"view_model_data.hud_dial_"};
    hud::TextLeaf gear = L.text("gear", 2);
    hud::BoolLeaf pit = L.boolean("pit");
};

inline const char *gear_text(int gear, char out[2])
{
    if (gear < 0)
        return "R";
    if (gear == 0)
        return "N";
    out[0] = static_cast<char>('0' + (gear > 9 ? 9 : gear));
    out[1] = 0;
    return out;
}

class Dial {
public:
    void frame(double dt)
    {
        std::optional<live::Car> me = live::available() ? live::subject() : std::nullopt;
        std::optional<live::Session> session = live::session();

        hud::Gate gate;
        gate.live = me.has_value() && session.has_value();
        if (session) {
            gate.session_serial = session->generation;
            gate.subject_serial = session->subject_generation;
            gate.time_trial = hud::format::is_time_trial(session->type);
        }

        if (!frame_.begin(gate, dt)) {
            blank();
            return;
        }

        char digit[2];
        if (live::has(*me, SWOFTY_LIVE_HAVE_GEAR))
            leaves_.L.set(leaves_.gear, gear_text(me->gear, digit));
        leaves_.L.set(leaves_.pit, live::in_pit(*me));
        leaves_.L.flush();

        if (frame_.end()) {
            auto cost = frame_.cost();
            swofty::log("[hud-dial] frame %.0f us mean, %.0f us max over %d frames", cost.mean_us, cost.max_us, cost.frames);
        }
    }

private:
    void blank()
    {
        leaves_.L.set(leaves_.gear, "");
        leaves_.L.set(leaves_.pit, false);
        leaves_.L.flush();
    }

    Leaves leaves_;
    hud::Frame frame_;
};

}
```

```cpp [native/src/addon.cpp]
#include <swofty/events.hpp>
#include <swofty/mixin.hpp>

#include "dial.hpp"

namespace {

dial::Dial the_dial;

void on_frame(double dt, void *)
{
    the_dial.frame(dt);
}

}

SWOFTY_ON_LOAD
{
    swofty::events::on_frame(on_frame);
}

SWOFTY_ADDON()
```

- `live::subject()` is the watched car while the spectator HUD is up, else the player's. `hud::Frame` resets on its serial, so a spectator switch is one `changed_subject()` call away.
- `live::has(*me, SWOFTY_LIVE_HAVE_GEAR)` says the gear was read this frame. A clear bit means the field is zero, so the dial leaves the last text instead of drawing a wrong gear.
- `Frame::end()` is true every 10 seconds and reports the frame's mean and worst cost.

See [Live state](/reference/std/live), [HUD leaves](/reference/std/hud-leaves) and [HUD frame](/reference/std/hud-frame) for the other reads: gaps, the classification, turn numbers, rows, one-hot and level groups, cadences.

## Test it without the game

`fake::Live` answers `swofty::live::*` from a car you build, and `fake::Hud` records every leaf the core would get, with the core's own rules, so a type mismatch or a write outside the HUD prefixes fails the test.

```cpp [native/test/dial_test.cpp]
#include <cassert>
#include <swofty/fake_hud.hpp>
#include <swofty/fake_live.hpp>

#include "../src/dial.hpp"

int main()
{
    swofty::fake::Hud core;
    swofty::fake::Clock clock;
    swofty::fake::Live fake;
    dial::Dial dial;

    fake.session().type = 15;
    auto &car = fake.add_car(0, 1);
    car.gear = 6;
    car.have |= SWOFTY_LIVE_HAVE_GEAR;
    fake.set_order({0});
    fake.set_player(0);
    fake.install();

    dial.frame(0.016);
    assert(core.text("view_model_data.hud_dial_gear") == "6");

    core.clear();
    dial.frame(0.016);
    assert(core.writes().empty());          // unchanged leaves are not rewritten

    fake.set_available(false);              // no session: the dial blanks
    dial.frame(0.016);
    assert(core.text("view_model_data.hud_dial_gear").empty());
}
```

```sh
native/test/run_test.sh
```

```text
hud-dial test: ok
```

## Build, check and install

```sh
native/build.sh                           # native/hud_dial.dll
python3 tools/std/validate.py --data --addon example.hud-dial registry/hud.json
swofty hud check                          # the stock dial facts the composer relies on
swofty hud compose . registry/hud.json --id example.hud-dial --out hud-out
swofty addons install .
swofty addons enable example.hud-dial
swofty run --apply
```

`swofty hud compose` writes the same overlay archives the build writes, so you can read what the composer made. `swofty hud check` reads the game's original files and needs a game folder.

## Rows

A timing tower is one row design composed many times. The record writes the row once as a `group` with `repeat`; the composer makes the copies, appends the index to every name and substitutes `{NN}` (two digits) and `{N}` (plain) in every binding of the copy. A nested repeat gives each row its per-car groups, and `slot_var` makes a `session_car` binding read the car the index names. The rows sit in a `stack`, the game's own stacker: the header above them grows when a flag card shows and the rows move down with it.

```json
{
  "hud.element": {
    "tower": {
      "dial": "f1_26",
      "units": "tower",
      "box": [0, 0, 120, 320],
      "sprites": {
        "hdr":   { "png": "art/hdr.png",   "bounds": [0, 0, 92.4, 10.5] },
        "strip": { "png": "art/strip.png", "bounds": [0, 0, 2.3, 12.981] },
        "panel": { "png": "art/panel.png", "bounds": [2.3, 0, 92.4, 12.981] },
        "me":    { "png": "art/me.png",    "bounds": [0, 0, 92.4, 12.981] },
        "bar":   { "png": "art/bar.png",   "bounds": [0, -3.6, 1.2, 3.6] },
        "chq":   { "png": "art/chq.png",   "bounds": [0, 0, 92.4, 12.981] }
      },
      "elements": [
        { "kind": "stack", "id": "stack", "dir": "down", "gap": 0,
          "children": [
            { "kind": "group", "id": "top", "visible": { "view_model": "view_model_data.hud_tt_hdr" },
              "children": [
                { "kind": "sprite", "id": "hdr_bg", "sprite": "hdr", "layer": 20 },
                { "kind": "centre", "id": "lapc", "at": [46.2, 7.4],
                  "children": [
                    { "kind": "text", "id": "lap_word", "style": "OSD_19_regular", "align": "right", "box": 12,
                      "cap_height": 3.5, "literal_id": "lng_lap" },
                    { "kind": "text", "id": "lap", "style": "OSD_24_bold", "align": "left", "box": 12,
                      "cap_height": 4.5, "at": [1.0, 0], "value": { "view_model": "view_model_data.hud_tt_lap" } } ] },
                { "kind": "group", "id": "chq", "at": [0, 10.5], "visible": { "view_model": "view_model_data.hud_tt_chq" },
                  "children": [ { "kind": "sprite", "id": "chq_bg", "sprite": "chq", "layer": 20 } ] } ] },
            { "kind": "group", "id": "rows", "at": [0, 3.435],
              "children": [
                { "kind": "group", "id": "r", "repeat": { "count": 22, "var": "N", "pitch": [0, 12.981] },
                  "visible": { "view_model": "view_model_data.hud_tt_r{NN}_on" },
                  "children": [
                    { "kind": "sprite", "id": "s", "sprite": "strip", "layer": 20 },
                    { "kind": "sprite", "id": "p", "sprite": "panel", "layer": 20 },
                    { "kind": "sprite", "id": "me", "sprite": "me", "layer": 22,
                      "visible": { "view_model": "view_model_data.hud_tt_r{NN}_me" } },
                    { "kind": "group", "id": "pa", "visible": { "view_model": "view_model_data.hud_tt_r{NN}_up", "not": true, "default": true },
                      "children": [
                        { "kind": "group", "id": "pb", "visible": { "view_model": "view_model_data.hud_tt_r{NN}_dn", "not": true, "default": true },
                          "children": [
                            { "kind": "text", "id": "pos", "style": "OSD_19_regular", "align": "centre", "box": 5.4,
                              "cap_height": 3.54, "at": [3.2, 8.76], "value": { "view_model": "view_model_data.hud_tt_r{NN}_pos" } } ] } ] },
                    { "kind": "group", "id": "cn", "at": [8.5, 8.76],
                      "children": [
                        { "kind": "group", "id": "v", "repeat": { "count": 24, "var": "V" },
                          "visible": { "view_model": "view_model_data.hud_tt_r{NN}_c{VV}" },
                          "children": [
                            { "kind": "text", "id": "code", "style": "OSD_24_bold", "align": "left", "box": 14.7,
                              "cap_height": 4.5, "wrap": "clampwidth", "value": { "session_car": "driver.name_code", "slot_var": "V" } },
                            { "kind": "sprite", "id": "bar", "sprite": "bar", "layer": 30, "at": [-1.6, 0],
                              "tint": { "session_car": "nub_colour", "slot_var": "V", "default": "#00000000" } } ] } ] },
                    { "kind": "text", "id": "gap", "style": "OSD_19_regular", "align": "right", "box": 20,
                      "cap_height": 3.54, "at": [90.0, 8.76], "value": { "view_model": "view_model_data.hud_tt_r{NN}_gap" } } ] } ] } ] }
      ],
      "editor": {
        "rows": 5,
        "preview": {
          "view_model_data.hud_tt_hdr": true, "view_model_data.hud_tt_lap": "10",
          "view_model_data.hud_tt_r00_on": true, "view_model_data.hud_tt_r00_pos": "10", "view_model_data.hud_tt_r00_gap": "+1.2",
          "view_model_data.hud_tt_r01_on": true, "view_model_data.hud_tt_r01_pos": "11", "view_model_data.hud_tt_r01_gap": "+0.9",
          "view_model_data.hud_tt_r02_on": true, "view_model_data.hud_tt_r02_pos": "12", "view_model_data.hud_tt_r02_gap": "+0.5", "view_model_data.hud_tt_r02_me": true,
          "view_model_data.hud_tt_r03_on": true, "view_model_data.hud_tt_r03_pos": "13", "view_model_data.hud_tt_r03_gap": "+1.4",
          "view_model_data.hud_tt_r04_on": true, "view_model_data.hud_tt_r04_pos": "14", "view_model_data.hud_tt_r04_gap": "+0.7"
        }
      }
    }
  }
}
```

`"units": "tower"` reads every number above in Project Halo's tower unit (1/3 px at 1080p; row pitch 12.981), so Halo's `tower.js` geometry copies over unchanged. The composer writes `tower_r00` … `tower_r21` with `tower_r00_s`, `tower_r00_pos`, `tower_r00_v00` … `tower_r00_v23`, `tower_r00_v17_code`, `tower_r00_v17_bar`, and the template nodes `v_r00`, `t_r00_pos`, `v_r00_v17`, `t_r00_v17_code` (`#session.cars.car17.driver.name_code`), `c_r00_v17_bar` (`#session.cars.car17.nub_colour`, default transparent). The HUD editor shows five rows from `editor.preview`; the std lib's `timing::TowerLeaves` publishes `hud_tt_r00_on` … `hud_tt_r21_c23`, so the row leaves need no code of your own. Team logos and colour bars are the `unit` kind and `texture` bindings, in [HUD textures and units](/reference/ops/hud-textures); the standings board, the replay screen and the spectator tower are the `screen` field, with the stock board and lap counter hidden while the tower shows, in [HUD screens](/reference/ops/hud-screens). The HUD editor shows the standings copy with `editor.rows` rows from `editor.preview`.

## What is experimental

::: warning Not seen in game yet
The HUD composer, `SwoftyHud` version 2, the leaf table and the memory-backed live state are covered by host tests, by builds against the stock dial scenes and by round trips of the written files. None of them has run in the game yet. Where each part lands on the dial is the first thing to check.
:::

- **Live state is experimental.** It needs `"experimental_features": ["telemetry"]`, and its calls answer empty without a session.
- **Live groups are verified one by one.** A group of the live state reads only when every site it needs resolves on the running build and its rows in the knowledge base are verified. Tyres, game mode, team badges, the track name, the spectator gap mode, the pit-lane flags behind `pit_status` and `pit_stops`, and the DRS bit are off until their rows land. They read under `SWOFTY_LIVE_FORCE=1` on a test machine. The core logs each group it leaves off once.
- **Not read at all yet:** active aero mode, Boost and Overtake state, DRS allowed, the pit limiter, driver name and team colour, the session clock and lap total, lap and sector times, tyre temperatures. Their `have` bits stay clear.
- **The EA build** has no sites for the live state until its bindings exist, so the calls answer empty there.
- **`gear` here is a leaf of your own.** The game's own HUD values the record can bind are `speed_str`, `revs`, `ers_stored`, `om_active`, `is_xm_active`, `has_ers`, `ers_mode` and `drs_pit_state`.

The live state's [readiness section](/reference/std/live#when-a-group-reads) lists the groups and what turns each on.

::: info Credit
The element templates, the placement maths and the leaf helpers follow Project Halo by nyannoying (https://github.com/nyannoying1337/f1-25-project-halo). Uses work from Project Halo by nyannoying.
:::
