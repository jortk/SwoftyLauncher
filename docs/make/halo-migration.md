---
title: Moving Project Halo onto the std lib
description: Which Project Halo file each std lib part replaces, what stays in the addon, and the line counts before and after.
---

# Moving Project Halo onto the std lib

Project Halo by nyannoying is a native HUD addon for the 2026 car: a dial, a G-meter and a timing tower, driven by the game's memory with no UDP. Its Python build tools grow the dial scenes, its C++ plugin reads the game and writes the HUD's leaves. The std lib now carries the generic parts of both halves. This page maps each Halo file to the std replacement. The counts are from the research of this work, taken with `wc -l` on Halo's repository.

The sections are the build side, the native side and the tests. A new addon that wants a Halo-like dial starts from [Build a HUD element](./hud-element) instead.

## Build side

Halo's build is Python: sprites from a headless browser, BC7 encoding, resource writers, scene growth, template groups, anim clips, the editor preview and the overlay archives. The `hud.element` record replaces the pipeline from PNG sprites onward:

```text
tools/export_native.js, halo.js, export_sprites.py     stay: they draw the sprites (988 lines)
tools/native/textures.py, pngio.py (89)                -> packages/core/src/hud/textures.rs: PNG, edge bleed, content-hash cache
tools/native/bc7.py, bc7_tables.py (309)               -> packages/formats bc7_enc.rs, bit-identical to the launcher's C++ encoder
tools/native/resources.py, emb.py (161)                -> packages/formats surface.rs, emb.rs and the texture links of material.rs
tools/native/erpbuild.py, erpmeta.py (151)             -> the launcher's own ERP writer
tools/native/build_native.py (191)                     -> operations.hud: ready overlay archives
tools/native/halo_native.py, scene/template/clips (880) -> packages/core/src/hud: scene, template, editor, check; animclip.rs, uipre.rs
tools/native/halo_native.py, Geo, _halo, constants (424) stays as data: the layout becomes a hud.element record
tools/native/check_leaves.py (43)                      -> swofty hud check and the build's binding checks
tools/native/scene_dump.py, sim_page.py, sim.js (307)  not replaced: no previewer ships yet
```

What an addon keeps is the layout. Halo's `_halo` function and `Geo` become the record's `sprites`, `elements`, `decisions`, `editor` and `variants`:

```json
{ "hud.element": { "halo": { "dial": "f1_26", "stock": "park", "sprites": { }, "elements": [ ], "variants": { "f1_25": { }, "f2": { } } } } }
```

| | Lines now | After |
|---|---|---|
| Halo build tools | 3,543 | about 1,100 to 1,400: the sprite drawing and a layout that is data |
| Timing tower build tools | 3,860 | about 400: the sprite export and one record; the scene, template, replay, spectator, banner and editor preview builds move |

The composer covers the dial and the tower. `tower.js`'s geometry becomes one `hud.element` record with `"units": "tower"`: `header()`, `row()` and `yellow()` are a `stack` holding a header `group` and a `group` with `repeat` (22 rows, a nested repeat of 24 per-car groups with `slot_var`), the flag cards are groups shown by `hud_tt_chq`, `hud_tt_red`, `hud_tt_sc`, `hud_tt_vsc` and nested NOT groups ([rows](/reference/ops/hud#rows)). `tower_native.py`'s `build_stacked`, `build_template`, `build_replay_*` and `build_spectator_*` are `"screen": "standings"` with `"also": { "replay", "spectator" }` and `show` leaves; `lap_counter_nodes` is a `stock_nodes` entry on `hud_lap_counter.osd_visibility_option` with `while`; `banner_native.py` is `"restyle": { "preset": "banner", "sprite": "<navy fill>" }`; `preview_state()` and `build_uipre` are `editor.rows` and `editor.preview` ([HUD screens](/reference/ops/hud-screens)). `tower_resources.py`'s game textures (`g_f1`, `g_fia`, `g_fia_k`) are sprites with `texture` and `fit`; the team logo eXRefs and `textureOverride` nodes are `unit` elements with a `texture` binding; the colour bars are sprites with a `tint` on `nub_colour` with a transparent default ([HUD textures and units](/reference/ops/hud-textures)). What stays is drawing the sprites (`export_tower.js`, `export_sprites.py`) into PNGs the record names. The Time Trial board's decisions on the game's own pill values are not in the composer yet; a screen record with `decisions` is refused.

## Native side

```text
native/src/game_state.hpp (157), game_state_f1_25.hpp (96), game_state_f1_25.cpp (1,049)
    -> the core's live state: live::car(vehicle), live::session(), live::subject(), live::watched()
native/src/tower_reader.hpp/.cpp (314 + 846), tower_state.hpp/.cpp (277 + 489)
    -> live::order(), live::road_order(), the gap, pit, penalty, flag and classification fields
native/src/halo_track.hpp/.cpp (88 + 453), halo_corners.hpp (502)
    -> live::turn(car) and live::track_turns(); the corner table is live_corners_data.hpp in the core
native/src/leaves.hpp (188)
    -> hud_leaves.hpp: Leaves, OneHot (gear g0..g8), Level (th01..th10, rc01..rc16), rows
native/src/tower_leaves.hpp/.cpp (83 + 246)
    -> timing.hpp TowerLeaves and Leaves::rows with Atomic groups
native/src/tower_model.hpp/.cpp (154 + 329)
    -> timing.hpp Tower (window, arrows, gap modes), hud_format.hpp (gap, lapped, session label)
native/src/hud_model.hpp/.cpp (67 + 172)
    -> ease.hpp (toward, Smoothed, Window), hud_format.hpp speed, timing::car_ahead, hud_frame.hpp Gate and serials
native/src/gmeter.hpp/.cpp (64 + 108)
    -> ease.hpp Median3, Fresh and Smoothed; the indicator and flash rules stay
native/src/diag_log.hpp/.cpp (57 + 500)
    -> log_csv.hpp: the writer thread and the drop counter are the core's; the column lists stay
native/src/halo_hud.cpp (391)
    -> settings_struct.hpp (the 16 options and the 4 Hz poll), hud_frame.hpp (gating, cadences, cost, transition log)
```

The std tower leaf contract (`hud_tt_*`, `cVV_row`, `rNN_*`) is Halo's v5 contract, so a tower template written for Halo binds the same leaf names. A leaf's type is declared once in `Leaves`, and the build refuses a record that binds it with another.

| Halo file group | Lines | Std replaces | Stays in the addon |
|---|---|---|---|
| halo_hud.cpp | 391 | about 260 | about 130 |
| leaves.hpp | 188 | about 130 | about 58 (the View to leaves mapping) |
| tower_leaves.cpp/.hpp | 329 | about 250 | about 80 |
| hud_model.cpp/.hpp | 239 | about 90 | about 150 (power to capsule scale, G-meter wiring, turn lookup) |
| tower_model.cpp/.hpp | 483 | about 400 | about 85 (options to `Config`, debug text) |
| gmeter.cpp/.hpp | 172 | about 50 | about 120 |
| diag_log.cpp/.hpp | 557 | about 380 | about 175 |
| game_state*, tower_reader*, tower_state* | 3,230 | all | none |
| halo_track*, halo_corners.hpp | 1,043 | all | none |
| **Native total** | **6,747** | **about 4,800 (70 %)** | **about 800 to 1,000 of design code, up to about 2,000 with the leaf and options glue** |

The two estimates differ because the research counted the leaf mapping and options glue differently. The 800 to 1,000 figure is the design code alone: the G-meter indicator, the power to capsule scale for 2026 and the 2025 120 kW scale, the View to leaves mapping, and the column lists.

## Tests

Halo's `tests/` is 4,959 lines. `test_tower.cpp` (2,621 lines) builds fake game objects with `VirtualAlloc` and reads them back through `ReadProcessMemory`, which is the memory side that moves to the core. About 1,100 lines check the model and the leaf publisher; with `fake::Live`, `fake::Hud` and `fake::Clock` they shrink to about 300:

```cpp
swofty::fake::Hud core;
swofty::fake::Clock clock;
swofty::fake::Live fake;
fake.add_car(0, 1).gear = 6;
fake.set_player(0);
fake.install();
```

## Vendored headers

Halo vendors 18 SDK headers, about 3,100 lines, and uses four of them (`mixin.hpp`, `hud.hpp`, `events.hpp`, `mem.hpp`). It includes the new ones from the SDK folder the same way; the unused 14 go.

## What does not move

- The G-meter indicator thresholds and the flash rules, the 2026 power to capsule scale, and the 2025 scale.
- The sprite drawing, the leaf names that are Halo's contract, and the layout of each dial class.
- The tower's sprite drawing and the Time Trial board's decisions.
- Fastest lap, purple and green sectors and lap times: Halo has no source for them either. `timing::Car` declares the fields.

## Credits

The ported parts are listed in `THIRD_PARTY.md` of the launcher repository. Uses work from Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>.
