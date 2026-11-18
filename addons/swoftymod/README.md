# SwoftyMod

SwoftyMod (`swofty.core`) is the built-in addon. It adds core functionality for addon loading and customisation in Swofty Launcher for F1 25. It is always enabled and cannot be disabled or removed.

It provides:

* **ADDON SETTINGS** directly under Career on the home screen, added by a `MidHook` in `swofty_core.dll` (`native/src/core/`)
* the **F9 / R3 prompt** "Addon Settings", a listener and a tooltip entry
* the bottom-left "Launched with Swofty Launcher" panel, with the F9 or R3 glyph and "to open settings"
* the `mod_menu` route
* the ADDON SETTINGS screen, with a tile per enabled addon and track and a page for each. Tiles show the launcher's mark for SwoftyMod, Team Simplified's for Simplified for Swofty, the launcher's track badge for tracks and its box for the rest (`packages/core/src/ui/icons.rs`)
* the Swofty startup intro: `videos/cm_f1_sting.bk2` replaced by a 6 second Swofty logo video (`startup_intro/`, made with `startup_intro/tools/make_intro.py`; the video and generator are vmknolle's work) and the intro subtitle line set to "SWOFTY". An addon that replaces the same file or line wins. See [Make a startup video](../../docs/make/startup-video.md)
* its own settings, `show_launcher_panel`, `split_season_picker` (off keeps the Grand Prix, Time Trial and split-screen season picker on the game's single row, without the Built In and Addons rows or addon seasons) and `startup_intro` (off keeps the game's startup video; applies the next time you press Play)

This directory holds the manifest, the startup intro and the effect library, never game files. The launcher builds the UI at build time from the player's own copy of the game data (`packages/core/src/ui/`):

| Archive in `2025_asset_groups/ui_package/` | Edits |
|---|---|
| `flow_frontend.erp` | `fe_home.ui`: the F9 / R3 listener, the `mod_menu` tooltip, the launcher panel text and signals |
| `persistent.erp` | the `mod_menu` route, a route group per screen, `options_flow` entries and loading regions for the screens |
| `databases.erp` | the `mod_menu`, `mod_menu_offline` and `swofty_adjust` tooltips, and the `ui_menu_data.swofty` state |
| `flow_persistent.erp` | the online services popup, shown once as the SwoftyMod changelog |

The screens go to `swofty/ui/` (`swofty_settings.ui`, one `swofty_addon_<n>.ui` per enabled addon, one `swofty_map_<n>.ui` per track) and are listed in the `ui` block of `swofty/settings.json`. `swofty_core.dll` registers them and drives the option rows of `swofty_addon_<n>` pages.

The DLLs are produced by `native/build.sh` and are not committed; the launcher installs the whole `native/dist/` set (`version.dll`, `swofty_core.dll`, `swofty_gamelog.dll`, `bindings.json`, `symbols.tsv`, `log_channels.tsv`) for each modded session. See [`native/README.md`](../../native/README.md).

`packages/core/tests/real_menus.rs` compares the output byte for byte with a reference build named by `SWOFTY_MENU_ORACLE`.
