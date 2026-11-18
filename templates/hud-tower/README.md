# HUD tower

A starting point for a timing tower for Swofty Launcher for F1 25: one `hud.element` record that replaces the track-position board with a 30-row standings tower, on the HUD, the replay screen and the spectator screen. Copy this directory, rename it, and change `id` in `swofty.json`.

## Contents

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: `operations.hud` |
| `registry/tower@1.1.json` | The record: the header, the flag cards, 30 rows of 30 per-car slots, a Time Trial line, the editor preview, the lap counter and flag box hides |
| `art/*.png` | Six flat sprites, written by `tools/make_art.py` |

## What the record shows

- `screen: standings` with `show` and `also` puts the same elements on the three screens; `also.<screen>.hide` keeps the Time Trial line off the replay and spectator copies.
- The header draws the series logo through the game's own unit (`unit: series_logo`), the FIA badge from a game texture tinted black (`sprites.fia`), the lap count from `#session.number_of_laps` (a `path` binding tagged `si32`) and the session clock through the stock formatter (`clock`).
- Each row is one `repeat` of 30, each holding 30 per-car slots whose code and colour bar read the session's cars (`session_car` with `slot_var`); the team logo is a `unit: team_logo` with a `texture` binding.
- `colour` gives a text or sprite one fixed colour (the green personal-best copy of the gap, the flag cards).
- `stock_nodes` hides six parts of the lap counter and wraps four more that stock nodes already target, and hides the flag box, while the tower shows.
- `decisions` with `on` switches the Time Trial line on the game's own `pill_0.active`.
- `editor.rows: 5` and `editor.preview` give the HUD customisation menu a static five-row copy.

The leaves are `view_model_data.mytower_*`. Write them from native code through `swofty/hud_leaves.hpp`, as the `hud-dial` template does, or from the std lib's timing model.

## Build

```sh
python3 tools/make_art.py
swofty hud compose . registry/tower@1.1.json out/
swofty addons install .
swofty addons enable example.hud-tower
```

The reference [HUD screens](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/docs/reference/ops/hud-screens.md) and [HUD textures and units](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/docs/reference/ops/hud-textures.md) describe every field.
