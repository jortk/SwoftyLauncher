---
layout: false
title: Swofty Launcher for F1 25
titleTemplate: Addons and mods on Windows and Linux
description: A launcher and mod loader for F1 25 on Windows and Linux. Addons change the game's data and engine, mods and community tracks drop onto the Mods page, and profiles keep separate setups.

screenshots:
  hero:
    src: /screenshots/hero-launcher.png
    alt: The launcher's Home page with the Swofty Launcher 1.9 news post, the Discord server banner and the changelogs
  gallery:
    - src: /screenshots/gallery-ingame.png
      alt: The F1 25 home menu with ADDON SETTINGS under Career, the "Launched with Swofty Launcher" panel and the F9 prompt
      caption: ADDON SETTINGS in the game's home menu
      shape: square
    - src: /screenshots/gallery-launcher.png
      alt: The Mods page from Team Simplified with the Sepang and Okayama tracks turned on
      caption: Mods and tracks on the Team Simplified page
      shape: wide

hero:
  name: Swofty Launcher for F1 25
  tagline: Bringing <em>mod support</em> to
  game:
    text: F1 25
    link: https://store.steampowered.com/app/3059520/
  download:
    windows: https://cdn.swofty.net/launcher/latest/SwoftyLauncherInstaller.exe
    linux: https://cdn.swofty.net/launcher/latest/SwoftyLauncherInstaller-linux.zip
    both: https://cdn.swofty.net/launcher/latest/SwoftyLauncherInstaller.zip
    manual: /guide/installation
  docs: /guide/

links:
  - { icon: book, text: Documentation, link: /guide/ }
  - { icon: github, text: Source Code, link: https://github.com/Swofty-Developments/SwoftyLauncher }
  - { icon: tag, text: Releases, link: https://github.com/Swofty-Developments/SwoftyLauncher/releases }

intro: Swofty Launcher for F1 25 is a <em>launcher and mod loader</em> for F1 25 on <em>Windows and Linux</em>. Install <em>addons</em> that change the game's data or hook its engine, add <em>mods</em> and <em>community tracks</em>, change <em>addon settings in game</em> and keep separate <em>profiles</em>. Modded play is <em>offline only</em>, and launching from Steam stays vanilla.

platforms:
  - { icon: windows, text: Windows 10 and 11 }
  - { icon: linux, text: Linux with Steam and Proton }
  - { icon: deck, text: Steam Deck }

featuresTitle: Everything in <em>one launcher</em>
features:
  - icon: chip
    title: Addons
    details: Anything with a <em>swofty.json</em>. Data addons change the database, text and archives through <em>operations</em>; code addons use the <em>Advanced Mod Mixin System</em> to edit the engine, from new menus to bigger grids.
    link: /using/addons
  - icon: box
    title: Mods
    details: Content with no code, loaded as the <em>Simplified Suite</em> loads it. Drop <em>.serps</em> packages and zips of <em>.erp</em> archives and other game files on the launcher's <em>Mods</em> page, set their order and see where they overlap.
    link: /using/mods
  - icon: flag
    title: Tracks
    details: Import community track zips, such as <em>Sepang</em> and <em>Okayama</em>, on the <em>Mods</em> page and race on them as extra tracks.
    link: /using/mods#tracks
  - icon: data
    title: DB Editor
    details: Change drivers, teams, line-ups, tracks, the calendar, weather, points and any other database value. Save changes as <em>packages</em>, stack them per profile and share them as zips.
    link: /using/db-editor
  - icon: layers
    title: Profiles
    details: Keep separate setups like <em>Vanilla</em>, <em>26 car grid</em> or <em>Classic tracks</em>, each with its own addons, mods and settings. The game folder goes <em>back to vanilla</em> when you quit.
    link: /using/profiles
  - icon: sliders
    title: In-game addon settings
    details: Change addon options from the game's main menu, in <em>ADDON SETTINGS</em> under Career.
    link: /using/swofty-mod
  - icon: offline
    title: Offline-only modded play
    details: Modded sessions always run <em>offline</em> with multiplayer off. Launching from Steam normally stays <em>completely vanilla</em>.
    link: /reference/safety

developers:
  text: Swofty Launcher for F1 25 is <em>open source</em>, with a hook system built for modders. Addons <em>hook game functions by name</em>, with <em>priorities</em> and <em>chaining</em> so several addons can change the same function.
  actions:
    - { icon: book, text: Modding docs, link: /reference/, style: solid }
    - { icon: chip, text: Plugin SDK, link: /reference/addons/native-plugins, style: outline }
    - { icon: code, text: Game API, link: /reference/game/, style: outline }

cta:
  text: Interested? Read the <em>installation guide</em> to set up Swofty Launcher for F1 25, or the <em>docs</em> to start making addons and mods.
  actions:
    - { icon: download, text: Installation guide, link: /guide/installation }
    - { icon: book, text: Docs, link: /reference/ }
    - { icon: github, text: Source, link: https://github.com/Swofty-Developments/SwoftyLauncher }
---

<Landing>
<template #code>

```cpp
#include <swofty/mixin.hpp>
#include <swofty/game/fe_home.hpp>

using namespace swofty::game;

SWOFTY_WRAP(FeHome_Handle_Command, uint64_t, FEHomeViewModel *vm, NeStr **cmd)
{
    if (swofty::str_eq(*cmd, "example.open_page"))
        return open_page(vm);
    return orig(vm, cmd);
}
```

</template>
</Landing>
