---
title: HUD elements
description: Add sprites, texts, level bars, gauges and sweeps to the game's HUD dials with hud.element records and operations.hud, and what the composer does with them.
---

# HUD elements

A `hud.element` record puts art and live values into one of the game's three HUD dials. The build composes the records of every enabled addon into one grown dial scene per dial, one template pass and one editor preview, and writes ready overlay archives.

```json [swofty.json]
"operations": {
  "hud": ["registry/hud.json"]
}
```

## A record in full

A Halo-like dial: a base, a speed text, a boost lamp, a level bar, a battery gauge and an RPM sweep, with the editor preview and an F2 variant.

```jsonc [registry/hud.json]
{
  "hud.element": {
    "halo": {
      "dial": "f1_26",                              // f1_26, f1_25 or f2: the car class dial it grows
      "stock": "park",                              // keep | hide | park (default: park in f1_26, hide elsewhere)
      "sprites": {                                  // PNG art; sizes are multiples of 4
        "base":  { "png": "art/base.png",  "bounds": [-100, -48, 110, 38] },   // x0 y0 x1 y1 in HUD units
        "lit":   { "png": "art/lit.png",   "bounds": [-90, -30, -74, -22] },
        "fill":  { "png": "art/fill.png",  "bounds": [-20, 8, -4, 16] },
        "mask":  { "png": "art/mask.png",  "bounds": [10, -30, 42, 2] },
        "solid": { "png": "art/solid.png", "bounds": [-4, -4, 4, 4] }
      },
      "elements": [
        { "kind": "sprite", "id": "base", "sprite": "base", "layer": 20 },
        { "kind": "group", "id": "std",
          "visible": { "view_model": "view_model_data.hud_halo_gm", "not": true, "default": true },
          "children": [
            { "kind": "sprite", "id": "boost", "sprite": "lit",
              "visible": { "view_model": "view_model_data.hud_halo_boost" } },
            { "kind": "text", "id": "speed", "style": "OSD_24_bold", "align": "centre", "box": 40,
              "cap_height": 9.5, "at": [0, -14.9], "value": { "game": "speed_str" } },
            { "kind": "text", "id": "pos", "style": "17_bold", "align": "right", "box": 14,
              "cap_height": 6, "at": [-40, 12], "value": { "session": "current_car.position" } },
            { "kind": "levels", "id": "dp", "sprite": "lit", "count": 16,
              "show": "view_model_data.hud_halo_dp{nn}" },
            { "kind": "slats", "id": "th", "sprites": ["lit", "lit", "lit"],
              "show": "view_model_data.hud_halo_th{nn}" },
            { "kind": "gauge", "id": "batt", "sprite": "fill",
              "path": [[-20, 12], [-4, 12]], "value": { "game": "ers_stored" } },
            { "kind": "sweep", "id": "rpm", "mask_sprite": "mask", "fill_sprite": "solid",
              "pivot": [0, 0], "start": 225, "arc": [[0, 0], [1, -270]],
              "value": { "game": "revs" }, "tint": { "session": "current_car.nub_colour" } }
          ] }
      ],
      "decisions": [],
      "editor": {
        "preview": { "game.revs": 0.62, "game.speed_str": "135", "game.ers_stored": 0.75,
                     "view_model_data.hud_halo_boost": true, "view_model_data.hud_halo_dp10": true },
        "team_colour": "#27F4D2"
      },
      "variants": {
        "f2": { "dial": "f2", "remove": ["batt"] }
      }
    }
  }
}
```

`swofty hud compose <addon folder> registry/hud.json` writes the archives to a folder and exits 1 when a stock archive that carries a composed resource is left out. `swofty hud check` asserts the stock facts below against the game's original files, with the archives that carry each resource the composer writes and that they all hold the same copy.

## Units and placement

One HUD unit is one pixel of a 640x360 frame, the origin is the dial centre and `v` points down. A record written in Project Halo's tower unit (1/3 px at 1080p) says `"units": "tower"`; the composer reads every `at`, `bounds`, `box`, `pitch`, `gap`, `size` and `cap_height` in that unit (0.03 scene units) and the default `hud` stays 0.018. A sprite's `bounds` is where it sits in that frame, so a sprite's art lines up with the others without an `at`. The record's root sits where the bottom of its box lands on the stock bottom customisation point; the box is `box`, else the bounds of the sprite named `base`, else the bounds of every sprite. That point differs by a few hundred-thousandths between the dials, so a record that must land on one exact spot in every dial sets `root_y`, in its own units, `v` down from the dial centre; a variant's `root_y` overrides it for that dial. The selection box follows the root either way.

```jsonc
"root_y": 16,                                         // the root 16 units below the centre; at is relative to it
"variants": {
  "f2": { "dial": "f2", "root_y": 16.5 }
}
```

Every element hangs off `boundless_customisation`, so the HUD editor's move and resize apply to it.

## Element kinds

```jsonc
{ "kind": "sprite", "id": "a", "sprite": "base", "layer": 20, "at": [0, 0], "rot": 0 }
{ "kind": "text",   "id": "b", "style": "OSD_24_bold", "align": "left|centre|right", "box": 40,
  "cap_height": 9.5, "at": [x, y], "rot": 0, "case": "upper|none", "wrap": "none|clampwidth",
  "value": { "game": "speed_str" } }              // or "literal": "BOOST"
{ "kind": "group",  "id": "c", "hidden": false, "children": [ ] }   // hidden: true = isHidden plus a static visible=false node
{ "kind": "text",   "id": "pb", "style": "OSD_24_regular", "box": 80, "cap_height": 9.8, "colour": "#58d654ff",
  "value": { "view_model": "view_model_data.hud_x_gap" } }         // colour: one fixed colourOverride (text or sprite)
{ "kind": "levels", "id": "d", "sprite": "lit", "count": 16, "from": 38, "to": -48,
  "show": "view_model_data.hud_x_dp{nn}" }        // one windowed copy per level, lit by one leaf each
{ "kind": "slats",  "id": "e", "sprites": ["s1", "s2"], "show": "view_model_data.hud_x_th{nn}" }
{ "kind": "gauge",  "id": "f", "sprite": "fill", "path": [[x0, y0], [x1, y1]], "window": [w, h],
  "value": { "game": "ers_stored" } }              // a window slides over the fill as the value rises
{ "kind": "sweep",  "id": "g", "mask_sprite": "m", "fill_sprite": "solid", "pivot": [0, 0],
  "start": 225, "arc": [[0, 0], [1, -270]], "value": { "game": "revs" } }
{ "kind": "stack",  "id": "h", "dir": "down|up|right|left", "gap": 0, "at": [x, y], "children": [ ] }   // the game's eStacker
{ "kind": "centre", "id": "i", "at": [x, y], "children": [ ] }                                           // the game's eCentre (centreX)
{ "kind": "group",  "id": "r", "repeat": { "count": 22, "var": "N", "pitch": [0, 21.635], "from": 0 }, "children": [ ] }
```

Text origins are the alignment anchor and the baseline; the composer places each text on a rotated, scaled null with the game's measured cap height of 0.672 em, so `cap_height` is the height of a capital in HUD units. Known styles are `17_bold`, `24_bold`, `34_regular`, `OSD_17_bold`, `OSD_17_regular`, `OSD_19_regular`, `OSD_24_bold`, `OSD_24_regular`, `OSD_26_bold`, `OSD_30_bold` and `OSD_34_bold`; `case` is `upper`, `mixed` or `none`. A text with `"literal_id": "lng_lap"` shows the game's own string of that id: its label node writes `localise` true and the id as the text.

`{nn}` in a `show` pattern is the two-digit index, from 01. A `levels` window `i` shows the lower part of the sprite up to `from + (to - from) * i / count`. `arc` is `[fraction, degrees]` keys of the sweep turning about `pivot`, relative to the frame turned to `start` degrees.

## Rows

A `group` with `repeat` is written once and composed `count` times. Copy `i` is the element `<prefix>_<id>NN` at `at + pitch * (i - from)`; every element under it is `<prefix>_<id>NN_<child>`, and every string in the copy's subtree reads the index: `{NN}` is two digits with a leading zero, `{N}` is plain, where `N` is `var`. A `session_car` binding takes `slot_var` in place of `slot`, and the copy's index is the car. Repeats nest two deep and a record makes at most 1024 copies.

```jsonc
{ "kind": "stack", "id": "stack", "dir": "down", "gap": 0,
  "children": [
    { "kind": "group", "id": "top", "visible": { "view_model": "view_model_data.hud_tt_hdr" },
      "children": [
        { "kind": "sprite", "id": "hdr_bg", "sprite": "hdr", "layer": 20 },
        { "kind": "centre", "id": "lapc", "at": [20.0, 8.0],
          "children": [
            { "kind": "text", "id": "lap_word", "style": "OSD_19_regular", "align": "right", "box": 20, "cap_height": 5.9, "literal_id": "lng_lap" },
            { "kind": "text", "id": "lap", "style": "OSD_24_bold", "align": "left", "box": 20, "cap_height": 7.5, "at": [1.5, 0],
              "value": { "view_model": "view_model_data.hud_tt_lap" } } ] },
        { "kind": "group", "id": "chq", "at": [0, 5.73], "visible": { "view_model": "view_model_data.hud_tt_chq" },
          "children": [ { "kind": "sprite", "id": "chq_bg", "sprite": "chq", "layer": 20 } ] } ] },
    { "kind": "group", "id": "rows", "at": [0, 5.73],
      "children": [
        { "kind": "group", "id": "r", "repeat": { "count": 22, "var": "N", "pitch": [0, 21.635] },
          "visible": { "view_model": "view_model_data.hud_tt_r{NN}_on" },
          "children": [
            { "kind": "sprite", "id": "s", "sprite": "strip", "layer": 20 },
            { "kind": "sprite", "id": "me", "sprite": "me", "layer": 22, "visible": { "view_model": "view_model_data.hud_tt_r{NN}_me" } },
            { "kind": "group", "id": "pa", "visible": { "view_model": "view_model_data.hud_tt_r{NN}_up", "not": true, "default": true },
              "children": [
                { "kind": "group", "id": "pb", "visible": { "view_model": "view_model_data.hud_tt_r{NN}_dn", "not": true, "default": true },
                  "children": [
                    { "kind": "text", "id": "pos", "style": "OSD_19_regular", "align": "centre", "box": 9, "cap_height": 5.9, "at": [5.3, 14.6],
                      "value": { "view_model": "view_model_data.hud_tt_r{NN}_pos" } } ] } ] },
            { "kind": "group", "id": "cn", "at": [14.2, 14.6],
              "children": [
                { "kind": "group", "id": "v", "repeat": { "count": 24, "var": "V" },
                  "visible": { "view_model": "view_model_data.hud_tt_r{NN}_c{VV}" },
                  "children": [
                    { "kind": "text", "id": "code", "style": "OSD_24_bold", "align": "left", "box": 24.5, "cap_height": 7.5, "wrap": "clampwidth",
                      "value": { "session_car": "driver.name_code", "slot_var": "V" } },
                    { "kind": "sprite", "id": "bar", "sprite": "bar", "layer": 30,
                      "tint": { "session_car": "nub_colour", "slot_var": "V", "default": "#00000000" } } ] } ] } ] } ] } ] }
```

That record composes `r00` … `r21`, each with `r00_s`, `r00_me`, `r00_pa`, `r00_pb`, `r00_pos`, `r00_cn` and the 24 per-car groups `r00_v00` … `r00_v23` holding `r00_v00_code` and `r00_v00_bar`; the template gets `v_r00`, `v_r00_me`, `v_r00_pa`, `t_r00_pos`, `v_r00_v17`, `t_r00_v17_code` (bound to `#session.cars.car17.driver.name_code`) and `c_r00_v17_bar`. The dial and the scene target take the same group.

`stack` is a clone of the game's `full_stacker` (the stacked standings table) with `stackerType` from `dir` and `gap` in HUD units; `centre` is a clone of the lap counter's `centre_lap_value`. A stacker lays its children out by their drawn bounds: a child whose `visible` is off takes no room and the children after it move up, which is the tower's squeeze under a flag card; a child with `hidden` is refused, so a part that stays hidden goes inside a child. Pre-set each centred text's `at` for a typical string; an eCentre may leave children where they are.

## Bindings

A binding names exactly one source:

```jsonc
{ "view_model": "view_model_data.hud_halo_boost" }   // your own leaf, written with hud::set
{ "game": "revs" }                                   // a value of the dial's own view model
{ "session": "current_car.position" }                // a #session path
{ "session_car": "position", "slot": 5 }             // #session.cars.car5.position
{ "path": "#session.number_of_laps", "ty": "si32" }  // any other path of the game's view model, with its tag
```

`"not": true` negates a bool and `"default"` is the value while the binding is absent. A leaf's type comes from where it is bound: a bool for `visible`, text for a text, a float for a gauge or sweep. `"ty": "i32"` binds an int leaf as text: the core writes int leaves through the game's signed-int setter, so the node is tagged `si32` (`ui32` is the tag of the game's own unsigned values only, and is refused on a leaf).

`path` reads any value the game's view model already has, where `view_model` is for the addon's own leaves. A `visible` reads it as a bool, a `tint` as a colour, a unit's `texture` as text; a text needs `ty` (`text`, `si32` or `ui32`), the tag the game gives the value. When a stock node binds the same path the build checks the tag against it; otherwise `ty` is trusted, and a wrong tag crashes the game, so read it from a stock node that binds the path (`swofty ui` on the template) before declaring it:

```jsonc
{ "kind": "text", "id": "laps", "style": "OSD_24_regular", "box": 30, "cap_height": 9.8,
  "value": { "path": "#session.number_of_laps", "ty": "si32" } }
{ "kind": "group", "id": "pill", "visible": { "path": "#view_model_data.hud.content.lap_times.pill_0.active" },
  "children": [ { "kind": "text", "id": "pill_gap", "style": "OSD_24_regular", "box": 60, "cap_height": 9.8,
                  "value": { "path": "#view_model_data.hud.content.lap_times.pill_0.gap_text", "ty": "text" } } ] }
```

The game values and their tags:

| `game` | tag | use |
|---|---|---|
| `speed_str` | text | text |
| `revs`, `ers_stored` | float | gauge, sweep |
| `om_active`, `is_xm_active`, `has_ers` | bool | visible |
| `ers_mode` | ui32 | decision only |
| `drs_pit_state` | MVCD | decision only |

`session` takes `current_car.position` (ui32, text), `current_car.driver.second_name` (text) and `current_car.nub_colour` (colour, `tint`). `session_car` takes `position` and `driver.second_name`. A property's tag must be the tag of the value it binds: the game takes it on trust, and a text bound to the ui32 position crashed the game while it created the HUD. The build refuses a mismatch, and checks every game binding against the tags the stock template binds.

## Decisions

An element a decision lists is shown by the decision and has no `visible` of its own:

```jsonc
"elements": [
  { "kind": "sprite", "id": "drs_open", "sprite": "drs_open" },
  { "kind": "sprite", "id": "drs_off",  "sprite": "drs_off" },
  { "kind": "sprite", "id": "ers_hotlap", "sprite": "ers_hotlap" }
],
"decisions": [
  { "id": "drs", "type": "mvcd", "game": "drs_pit_state", "initial": "invalid",   // drs_off is on screen until the value first fires
    "states": { "drs_live": ["drs_open"], "invalid": ["drs_off"] } },             // other states hide every listed element
  { "id": "mode", "type": "svcd", "game": "ers_mode", "equals": 2, "ty": "ui32",
    "yes": ["ers_hotlap"], "no": [], "initial": "no" }                            // yes or no on an svcd
]
```

The composer writes an `MVCD` or `SVCD` node under the record's template group and registers and queries it from the dial's own `show` command, unregistering it from `hide`, as the stock DRS widget does. An `mvcd` takes an event for every state of the stock `MVCD` on the same value under the same dial; a state the record doesn't list hides everything.

Each listed element gets a `visible` node the decision's events switch. Its default is what the element shows before the first event: hidden, or shown when the element is in the decision's `initial` state. The dial's `show` queries the decision, so the targets re-sync to the current state every time the dial comes on screen, but a query fires nothing while the value has no state yet (the first frames of a session), and that is the window `initial` covers. The stock dials keep their grey DRS pill on screen the same way: the stock `MVCD` only switches the activated holders, the pill itself is visible by default.

A decision may also run `on` a path or leaf instead of a dial value, listing its own states (a bool has `0` and `1`, as the stock pills' `active` decisions do). That is how a screen record switches on the game's own values; a dial record keeps to `game`:

```jsonc
"decisions": [
  { "id": "tt", "type": "mvcd", "on": { "path": "#view_model_data.hud.content.lap_times.pill_0.active" },
    "states": { "1": ["trial"], "0": [] } },
  { "id": "p1", "type": "svcd", "on": { "path": "#session.current_car.position" }, "ty": "ui32", "equals": 1,
    "yes": ["leader"], "no": [] }
]
```

## Stock parts, variants and the editor

`stock` decides what happens to the dial's `frame`, `fuel`, `holder_central` and `pit_indicator` (or `stock_parts`): `park` moves them 60 units off screen and shrinks them, `hide` has the dial template set their `visible` to false, `keep` leaves them. A variant is a patch of the record for another dial; its `switch` binding shows the record while the leaf is on and the stock parts while it is off:

```jsonc
"variants": {
  "f1_25": { "dial": "f1_25", "remove": ["boost"], "switch": { "view_model": "view_model_data.hud_halo_cls", "default": true } },
  "f2":    { "dial": "f2", "remove": ["batt"] }
}
```

A variant may set its own `stock` (and `stock_parts`) over the record's (schema `1.2`). A record that keeps the 2026 dial's parts and hides the F2 dial's only while its leaf is on:

```jsonc
"stock": "keep",
"variants": {
  "f2": { "dial": "f2", "stock": "hide", "switch": { "view_model": "view_model_data.hud_x_on", "default": false } }
}
```

The f2 dial template then binds `v_stock_holder_central` and the other parts to the leaf negated, so the stock gear, rev and speed readouts show while the leaf is off and give way to the record while it is on; the f1_26 dial keeps its parts. `park` moves a dial's parts for good, so the check refuses it on a variant with a `switch`; a variant without one may park. The variant's `stock_parts` must be parts of its own dial.

A record can carry the `switch` itself (schema `1.4`), so a plain dial record (no variants, no fake base dial) is scoped to a session the addon decides. A variant without its own `switch` inherits the record's; one with its own keeps it. A record that shows on the f1_25 dial only while the addon's leaf is on, and gives the stock parts back while it is off or absent:

```jsonc
"onboard": {
  "dial": "f1_25",
  "stock": "hide",
  "switch": { "view_model": "view_model_data.hud_x_dial", "default": false }
}
```

The switch's `default` (a bool, `true` when absent) is the state while the leaf has not been written: the record's root starts with it and every `v_stock_<part>` with its inverse. With `"default": false` the stock dial shows from the first frame and keeps showing if the addon's native code never runs. `switch` is for dial records: the check refuses it on a screen record (use `show`), on a scene target, and with `stock: park`.

A variant's `add` entries go last at the top level unless one names its place: `before` or `after` an element of the base, or `replace` for one it stands in for. An id the variant removes or replaces is free inside that variant, so a swapped part keeps its id and its spot in the draw order. A variant also carries `decisions` of its own, built for its dial only and after the record's; they may show the variant's own `add` elements, and each dial still needs exactly one stock `MVCD` on the value (`drs_pit_state` has one in the f1_25 and f2 dials). Here the 2025 dial swaps the energy group under the bars and adds a two-state pill the 2026 dial has no value for:

```jsonc
"elements": [
  { "kind": "sprite", "id": "base", "sprite": "base", "layer": 20 },
  { "kind": "group", "id": "energy", "children": [ /* 2026 energy readouts */ ] },
  { "kind": "slats", "id": "brake", "sprites": ["slat", "slat", "slat"], "show": "view_model_data.hud_x_br{nn}" },
  { "kind": "sweep", "id": "rpm", /* ... */ }
],
"variants": {
  "f1_25": {
    "dial": "f1_25",
    "add": [
      { "kind": "group", "id": "energy", "replace": "energy", "children": [ /* 2025 energy readouts */ ] },  // same id, same place: the slats and the sweep stay above it
      { "kind": "group", "id": "pill", "after": "energy", "children": [
          { "kind": "sprite", "id": "pill_idle", "sprite": "pill_grey" },
          { "kind": "sprite", "id": "pill_live", "sprite": "pill_green" } ] }
    ],
    "decisions": [
      { "id": "drs", "type": "mvcd", "game": "drs_pit_state", "initial": "invalid",
        "states": { "invalid": ["pill_idle"], "drs_live": ["pill_live"] } }
    ],
    "root_y": 16
  }
}
```

The check refuses a placement that names no element of the base left in the variant, two placements on one entry, a placement on a base element, a variant decision that shows an element the variant has not got, and a variant decision with a record decision's id.

The HUD editor shows `dial_complete.idf` with no template running: the scene's own state plus its `.uipre` setters. So each record also gets a static copy of the elements that `editor.preview` shows (`"editor": { "rows": 5 }` caps a top-level repeat at five copies there, keyed by the substituted leaf names `view_model_data.hud_tt_r00_on` … `r04_on`) (keys: `game.<name>`, `session.<path>`, `session_car.<field>.<slot>` or the leaf), its sprites pre-tinted with `team_colour`, `.uipre` setters that hide the stock parts and fill the copy's texts, and a `<prefix>_pv_ctl` group that hides the copy in game. The spectator HUD's dial of each class runs the same template groups unless the record sets `"spectator": false`.

## A scene target

A record that grows one of the dials names `dial`. A record that adds readouts to some other stock scene names that scene and the template node that instances it instead:

```jsonc [registry/damage_readouts.json]
{
  "hud.element": {
    "readouts": {
      "scene": "eaid://ui_package/art/osd/components/dial/mfd_car_damage_panel.idf",
      "parent_ui": "hud_mfd_dynamic.mfd.attacher.car_damage_panel",
      "elements": [
        { "kind": "text", "id": "brake_fl", "style": "OSD_17_bold", "align": "left", "box": 30,
          "cap_height": 6, "at": [-60, 40],
          "value":   { "view_model": "ui_menu_data.hud_brake_check_fl" },
          "visible": { "view_model": "ui_menu_data.hud_brake_check_visible" } },
        { "kind": "text", "id": "plank", "cap_height": 6, "at": [0, 50],
          "value": { "view_model": "ui_menu_data.hud_planked_text" } }
      ]
    }
  }
}
```

`scene` is the `eaid://` path of a stock `.idf`, read from `flow_race.erp`. `parent_ui` is the dotted path in the template (root name first) of the node whose component is that scene; the build stops unless the node instances the scene. A record has a `dial` or a `scene` and `parent_ui`, never both.

The core adds the elements under the scene's root element, in HUD units from the scene's origin (`v` down, as in the dials), in a null named for the record's prefix, and one bound node per text or visibility straight under `parent_ui`, named `<prefix>_<name>` (`t_<id>` for a text, `v_<id>` for a group's visibility). There is no `<prefix>_ctl` group and no `show` or `hide` wiring, because the parent is no dial. The scene's other elements and the parent's other children are not touched.

A scene target takes `text` and `group` elements with `value`, `literal`, `visible`, `at`, `style`, `align`, `box`, `case`, `wrap` and `cap_height`. It takes no sprites, decisions, variants, `stock`, `editor` or `box` on the record, and no `layer` or `tint`. Binding types are held to the stock template exactly as for the dials: a game or session value must have the tag the property reads, and a leaf has the type its use gives it. A leaf may live in `view_model_data` or in `ui_menu_data`; the second is the dictionary some addons already write their values to, so their readouts keep their paths.

The scene a scene target grows is read after every other addon's whole replacement of it has been merged ([two addons replacing one UI resource](/reference/ops/erp#two-addons-replacing-one-ui-resource)), so a scene target and an addon that ships its own copy of the scene stay together. The archives are `hud_target_scene.erp` (the grown scene, overlaid on the archives that carry both the template and the scene) and `hud_template.erp`.

## Screens, textures and units

A record is not bound to the dials. With `screen` in place of `dial` its elements go on the standings board, the replay screen's drivers list or the spectator tower, with `show` and `also` naming the leaf that shows them on each screen, `stock_nodes` hiding the game's own parts while they show and `restyle` reshaping the Race Control banner: [HUD screens](/reference/ops/hud-screens). Team logos, the series logo and colour bars come from the game's own textures through the `unit` kind, `texture` bindings and stock-texture sprites: [HUD textures and units](/reference/ops/hud-textures). The rows of a timing tower are the `repeat`, `stack` and `centre` fields above. A group that slides or fades on a bool leaf carries a `motion`: [HUD motion](/reference/ops/hud-motion).

## Bridge addons

Two addons that each replace `hud_dynamic_templates.ui` whole used to leave the later one the winner, and a third addon spliced their copies together by hand (Project Halo + Planked + Brake Check Panel does that). The build now merges whole replacements of the template and of the scenes node by node, and a `hud.element` composition starts from the merged result, so that bridge is no longer needed. See [two addons replacing one UI resource](/reference/ops/erp#two-addons-replacing-one-ui-resource) for the rules and the build lines and warnings.

## What the composer writes

One scene growth per dial and one template pass, whatever the number of addons:

```text
hud_scene.erp      the grown dial scenes, anim clips and string tables, the editor .uipre, the textures
                   overlaid on flow_race, flow_options, persistent and screens/hud_spectator_2
hud_template.erp   hud_dynamic_templates.ui with a <prefix>_ctl group under each dial
                   overlaid on flow_race and flow_options
hud_spectator.erp  hud_spectator_2.ui with the same groups under its three dials
                   overlaid on flow_race and screens/hud_spectator_2
```

Every element and template node name starts with the record's `prefix` (default: the addon id and the record key, lowercase, `_` between words, at most 24 characters), so two addons never share a name. The archives are ready `overlay` operations, so the runtime loader takes the grown scenes as they are.

## Composition order

The composer is the last step that writes these resources, and it builds on what the others leave:

```text
1. the game's original archives            read through the launcher's baseline, never a modified folder
2. addons' own archive operations          replace-resource, put-resource, patch-pkg and overlay on the
                                           template, the spectator file, the dial scenes, their string
                                           tables and .uipre files, applied in load order to a scratch copy
3. the ADDONS MFD page                     the launcher's own edit of hud_dynamic_templates.ui, from the
                                           options of addons that declare `hud.mfd`
4. the composer                            grows the scenes and adds its nodes to the result of 1 to 3
5. the overlay operations                  the composed archives go in after every other addon's
                                           operations, under their own origin ("<addon> (HUD composer)")
```

When two or more addons replace the same template or scene whole in step 2, the scratch copy holds their [node by node merge](/reference/ops/erp#two-addons-replacing-one-ui-resource), not the last one.

So the one template that ends up in the game carries every addon's `<prefix>_ctl` group, the MFD page, and the edit of an addon that replaced `hud_dynamic_templates.ui` (Planked Brake Panel's `replace-resource`, say). Step 3 is applied again by the menu build on the data route; it does nothing the second time, so the page is in the template once. On the runtime route the composed overlay archive already holds the page, so the overlay does not take it away.

The build's `hud:` lines say which addon's change the composed archives were built on top of.

## When another addon's change cannot be carried

A change from step 2 that fails to apply to the scratch copy (a `patch-pkg` that no longer finds its node under the composed dial, say) is not carried. The build goes on and warns, naming both addons:

```text
Planked Brake Panel: its change to a HUD resource cannot be applied under Halo's composed HUD (<reason>); the composed overlay replaces that resource with a copy of the game's own
```

Two limits:

- The composer reads `flow_race.erp`'s copy of each resource. A change that only another archive carries (`flow_options.erp`'s copy of the template) is not seen.
- A whole-archive `put` of `flow_race.erp`, `flow_options.erp`, `persistent.erp` or `screens/hud_spectator_2.erp` is read as that archive, and a later put of one of them drops the composed overlay for it like any other overlay.

A compose error (an unknown game value, a missing sprite, a prefix two records share) stops the build with `the HUD elements do not compose: <reason>`.

## Rules the composer keeps

- `isHidden` on an element does not hide it in game or in the editor; only a template node's `visible` does. A root hidden by default has a `visible` node, and the spectator HUD gets its own groups.
- Stock element ids never move, element names are unique in the scene, and every id is the djb2 hash of its dotted path; the build stops otherwise.
- Every bound property has the tag of its value; decision nodes read the values the stock dial reads.
- Everything hangs off `boundless_customisation`; the selection box (`tl_customisation`, `br_customisation`) is the union of the records' boxes.
- A resource the composer writes goes to every stock archive that carries it, not only to the archives its overlay is made for; an archive that also carries the resource gets an added `<archive>_2.erp` with the composed copies and the archive's new resources. A build that would leave a stock copy beside a composed one stops with `the composed HUD of <addons> would leave the game's own copy of a resource beside the composed one`.
- A record's nodes are checked against the stock files: the template without the added groups and the signals to them is the stock template, node for node.

The anim nodes a gauge or sweep adds are named `zh_<element>...`, which sorts after every stock name in the scene's anim string table.

::: info Credit
The element templates, the placement maths, the clip keys and the binding rules follow Project Halo by nyannoying (https://github.com/nyannoying1337/f1-25-project-halo). Uses work from Project Halo by nyannoying.
:::
