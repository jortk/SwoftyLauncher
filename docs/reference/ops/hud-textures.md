---
title: HUD textures and units
description: Team logos, series logos and colour bars drawn through the game's own textures, with hud.element unit elements, texture bindings and stock-texture sprites.
---

# HUD textures and units

A `hud.element` record can draw the game's own art: a team logo or the series logo as a `unit` (an instance of the game's logo scene, its texture set by a bound value), a bar in the team's colour, and the F1 or FIA logo as a sprite without a PNG.

## A tower row with a team logo, a colour bar and the series logo

```jsonc [registry/tower.json]
{
  "hud.element": {
    "tower": {
      "screen": "standings",
      "sprites": {
        "f1":    { "texture": "eaid://ui_package/art/osd/textures/lap_counter/osd_customisation/lc_f1_logo.tif",
                   "fit": [34.2, 12.0] },                                     // the game's F1 logo, scaled into a 34.2 x 12 box
        "fia_k": { "texture": "eaid://ui_package/art/osd/textures/message_system/ms_fia_logo.tif",
                   "fit": [8.0, 8.0], "tint": "#000000" },                    // the FIA badge, black
        "bar":   { "png": "art/bar.png", "bounds": [0, -6, 2, 6] }           // a white quad the colour bar tints
      },
      "elements": [
        { "kind": "group", "id": "hdr", "visible": { "view_model": "view_model_data.hud_tt_hdr" },
          "children": [
            { "kind": "unit", "id": "logo", "unit": "series_logo", "at": [8.3, 1.9], "size": 20.0,
              "texture": { "screen": "series_logo" } },                       // F1 or F2: whatever the lap counter shows
            { "kind": "sprite", "id": "fia", "sprite": "fia_k", "layer": 25, "at": [40, 8] }
          ] },
        { "kind": "group", "id": "r", "repeat": { "count": 22, "var": "N", "pitch": [0, 21.635] },
          "visible": { "view_model": "view_model_data.hud_tt_r{NN}_on" },
          "children": [
            { "kind": "unit", "id": "lg", "unit": "team_logo", "at": [11.0, 10.8], "size": 14.1,
              "visible": { "view_model": "view_model_data.hud_tt_r{NN}_bdg" },
              "texture": { "view_model": "view_model_data.hud_tt_r{NN}_badge" } },
            { "kind": "group", "id": "v", "repeat": { "count": 24, "var": "V" },
              "visible": { "view_model": "view_model_data.hud_tt_r{NN}_c{VV}" },
              "children": [
                { "kind": "text", "id": "code", "style": "OSD_24_bold", "align": "left", "box": 24.5,
                  "cap_height": 7.5, "at": [14.2, 14.6], "wrap": "clampwidth",
                  "value": { "session_car": "driver.name_code", "slot_var": "V" } },
                { "kind": "sprite", "id": "bar", "sprite": "bar", "layer": 30, "at": [12, 14.6],
                  "tint": { "session_car": "nub_colour", "slot_var": "V", "default": "#00000000" } }
              ] }
          ] }
      ]
    }
  }
}
```

The composer writes, for that record:

- in the scene, one eXRef of `driver_position_team_logo.idf` per row (`<prefix>_r00_lg` … `<prefix>_r21_lg`), scaled so its square quad is 14.1 HUD units wide and centred on `at`; one eXRef of `car_category_logo.idf` in the header with its top-left corner on `at` and 20 units wide; a white quad per row per car for the bar; a quad per stock-texture sprite whose material links the game's `.tif` (no texture of its own is written);
- in the template, `x_r00_lg` … nodes setting `textureOverride` on `[<prefix>_r00_lg, team_logo]` from `#view_model_data.hud_tt_r00_badge`, with the tag the stock pills bind `team_badge_texture` with; `x_logo` setting `textureOverride` on `[<prefix>_logo, logo_icon]` from the screen's `logo_texture` leaf, default `#overrides.osd.timing_tower.timing_tower_logos.lc_f1_logo`; `c_r00_v00_bar` … nodes setting `colourOverride` from `#session.cars.car0.nub_colour`, transparent while the game has no colour.

## Units

```jsonc
{ "kind": "unit", "id": "lg", "unit": "team_logo", "at": [11.0, 10.8], "size": 14.1,
  "visible": { "view_model": "view_model_data.hud_tt_r{NN}_bdg" },
  "texture": { "view_model": "view_model_data.hud_tt_r{NN}_badge" } }
```

A unit is an instance of one of the game's own logo scenes, cloned from the stock reference the game itself draws it with (the same `fileRef`, xref id and material binding, never `isHidden`). `size` is the width of the unit's quad in HUD units; the composer scales the instance uniformly and offsets it by the quad's own position inside the unit.

| `unit` | Scene | `at` is the quad's | Texture tag read from |
|---|---|---|---|
| `team_logo` | `units/driver_position_table/driver_position_team_logo.idf`, quad `team_logo`, a square | centre | the five stock pills' `team_badge_image.textureOverride <- team_badge_texture`, which must agree |
| `series_logo` | `units/lap_counter/car_category_logo.idf`, quad `logo_icon`, 1.44 x 0.52 scene units | top-left corner | `hud_lap_counter.osd_visibility_option.logo.textureOverride <- logo_texture` |

A unit takes `at`, `size`, `visible` and `texture`; no `layer` (the quad keeps the unit's own, 30), no `sprite`, no `value`. The editor preview draws no units, as the game's own preview draws no logos.

## Texture bindings

```jsonc
"texture": { "view_model": "view_model_data.hud_tt_r{NN}_badge" }           // a text leaf: a texture path or ""
"texture": { "view_model": "view_model_data.hud_x_logo", "default": "#overrides.osd.timing_tower.team_badges_hud.ferrari" }
"texture": { "screen": "series_logo" }                                         // the screen's own logo_texture leaf
```

`texture` makes an `x_<id>` template node with `textureOverride` on the layout `[<unit element>, <quad>]`. Its tag is never assumed: the composer reads it from the stock node at build time and refuses a leaf declared as anything but text. A `view_model` leaf defaults to `""` (no texture) unless `default` names one. `{ "screen": "series_logo" }` binds the leaf the screen's own lap counter reads: `#view_model_data.hud.content.lap_counter.logo_texture` on the HUD and on a dial, `#replay_flow.replays_and_highlights.logo_texture` on the replay screen, the screen view model's `logo_texture` on the spectator screen; its default is the F1 logo, the value the game writes in an F1 session.

## Colour bars

```jsonc
{ "kind": "sprite", "id": "bar", "sprite": "bar", "layer": 30,
  "tint": { "session_car": "nub_colour", "slot_var": "V", "default": "#00000000" } }
```

`tint` on a sprite makes a `c_<id>` node with `colourOverride` from the bound colour. `default` is the colour while the game's value is absent, as `#RRGGBB` or `#RRGGBBAA`; the stock nodes use white, so leave it out for a tinted sprite and set `#00000000` for a bar that must vanish when there is no colour. `slot_var` names the var of a `repeat` the binding sits in; the composer substitutes the car's index, so `#session.cars.car0.nub_colour` … `car23` are bound one per copy (the game names its cars `car0` … `car23`, never zero-padded). A `slot_var` outside a repeat fails the build.

## Stock-texture sprites

```jsonc
"f1":    { "texture": "eaid://ui_package/art/osd/textures/lap_counter/osd_customisation/lc_f1_logo.tif", "fit": [34.2, 12.0] },
"fia":   { "texture": "eaid://ui_package/art/osd/textures/message_system/ms_fia_logo.tif", "fit": [8.0, 8.0] },
"fia_k": { "texture": "eaid://ui_package/art/osd/textures/message_system/ms_fia_logo.tif", "fit": [8.0, 8.0], "tint": "#000000" }
```

A sprite with `texture` in place of `png` draws one of the game's own textures. The composer writes only a material (the stock fill material with `me_diffuseTex` linked to that `.tif`) and the quad's binding; `tint` sets the material's `me_colour`, which multiplies the texture. `fit` is the box `[w, h]` in HUD units the texture is scaled into keeping its aspect, centred on the element's `at`; the pixel size comes from the texture's surface header in the stock archive (`lc_f1_logo.tif` is 144 x 52, `ms_fia_logo.tif` 60 x 40). `bounds` may replace `fit` to place the quad by hand.

## Game values the tower binds

| Binding | Path | Tag | Checked against |
|---|---|---|---|
| `{ "session_car": "driver.name_code", "slot_var": "V" }` | `#session.cars.car<V>.driver.name_code` | `stri` | no stock node binds it; the table is trusted |
| `{ "session_car": "nub_colour", "slot_var": "V" }` | `#session.cars.car<V>.nub_colour` | `rgba` | the stock `#session.current_car.nub_colour` binding |
| `{ "session": "number_of_laps" }` | `#session.number_of_laps` | `si32` | the stock lap counter's binding |
| `{ "session": "time_remaining_str" }` | `time_remaining_str` | `stri` | the stock session timer; its formatter (`hud_session_timer_dynamic.time_remaining_formatter`) is cloned into the record's group as `clock_fmt`, re-formatted from the group's `trigger` and, with the record's `clock`, written to the record's own leaf and re-formatted by a `start` bool ([HUD screens](/reference/ops/hud-screens#the-session-clock)) |
| `{ "path": "#session.number_of_laps", "ty": "si32" }` | the path as written | the declared `ty` | the stock node that binds the path, when one does; otherwise trusted |

`si32` is the tag of the game's signed ints and of an addon's own int leaves (`"ty": "i32"` on a `view_model` leaf; the core writes them through the game's signed-int setter). `ui32` is the game's unsigned values only, and is refused on a leaf.

## What `swofty hud check` asserts

- `driver_position_row.idf` has one `team_logo` eXRef of `driver_position_team_logo.idf`, and `lap_counter.idf` one `car_category_logo` eXRef of `car_category_logo.idf`: the unit's `fileRef`, xref id and material binding, no children, geoms or `isHidden`.
- Each unit draws one quad on its own mesh, turned a quarter about x, under ancestors that only translate, with an absolute layer; the team logo's is a centred square.
- The five stock pills bind `team_badge_texture` with one tag; `hud_lap_counter.osd_visibility_option.logo` is the one node under `hud_lap_counter` binding `logo_texture`.
- The stock template binds `#session.current_car.nub_colour` as `rgba`, `#session.number_of_laps` as `si32`, and no node binds `#session.current_car.driver.name_code`.
- `hud_session_timer_dynamic` has one `time_remaining_formatter` UITF.
- `lc_f1_logo.tif` and `ms_fia_logo.tif` have readable surface headers.
