---
title: Swofty's addons
description: The addons Swofty publishes in the Addon Browser - the Career Mode Editor, the Driver Manager, R&D Tyres & Thermal Management, Any Team Transfers and the PolyphoniX Native shader, plus xanpavle's F1 25 RE-Optimization - and how to install, update and remove them.
---

# Swofty's addons

Swofty publishes most of these addons in the [Addon Browser](/using/addon-browser), next to everyone else's; F1 25 RE-Optimization is xanpavle's. Install them from there; they arrive switched off in every profile.

| Addon | Id | By |
|---|---|---|
| [Career Mode Editor](/using/career-editor) | `swofty.career-editor` | Swofty |
| [Driver Manager](/using/driver-manager) | `swofty.driver-manager` | Swofty |
| R&D: Tyres & Thermal Management | `swofty.rnd-tyres-thermal` | Swofty |
| [Any Team Transfers](#any-team-transfers) | `swofty.any-team-transfers` | Swofty |
| [PolyphoniX Native](#polyphonix-native) | `swofty.polyphonix-native` | Mark Wazowski's look, rebuilt by Swofty |
| [F1 25 RE-Optimization](#f1-25-re-optimization) | `xanpavle.re-optimization` | [xanpavle](https://github.com/xanpavle) |

## Install, update or remove one

- **Install it:** open the **Addon Browser**, find it, and select **Install**. Follow its progress on the [Downloads page](/using/addon-browser#downloads).
- **Turn it on:** on the **Addons** page, toggle its switch in the profile you want it in, then press **Play**. From a terminal: `swofty addons enable <id>`.
- **Update it:** when Swofty publishes a new version, **Play** becomes **Update addons**. See [Updating](/using/updating#addon-updates).
- **Remove it:** expand its row on the **Addons** page and select **Remove addon**. Install it again from the Addon Browser to get it back.

Launchers before 1.4.0 carried these addons in an `addons/` folder next to `bin/`. Updating to 1.4.0 removes that folder; the installed copies stay installed and update from the Addon Browser.

## F1 25 RE-Optimization

F1 25 RE-Optimization is [xanpavle's](https://github.com/xanpavle) renderer optimisation for F1 25, released under the MIT licence; the source is at [xanpavle/F1-25-RE-Optimization](https://github.com/xanpavle/F1-25-RE-Optimization). It is xanpavle's addon, not Swofty's, and it is not bundled with the launcher: find it in the [Addon Browser](/using/addon-browser) like any other addon. Its settings and what it changes are described on its Addon Browser page.

## Any Team Transfers

In a Driver Career contract negotiation, every team makes you an offer, so Team Select lets you sign for any team on the grid, your current team included. Without it, the game offers two or three teams ranked close to yours, plus your current team when it renews you.

- Team Select opens when your contract ends and you start the contract event in Team HQ. The contract terms, the signing and the move at the end of the season are the game's own.
- A Progressive Career that follows your driver's real career still signs only for his real team.
- It needs Swofty Launcher 1.5.13 or later, on the Steam or EA build of F1 25. On a game build where the function it hooks has changed, offers stay stock and `swofty/loader.log` says why.

## PolyphoniX Native

PolyphoniX Native is a shader: the PolyphoniX Native look from Assetto Corsa's Custom Shaders Patch, by Mark Wazowski, rebuilt for F1 25. It brightens the midtones while white stays white, adds a touch of colour, sets a 6000 K white balance, and gives bright lights a soft glare with a six-point star.

- It applies to the 3D picture before the HUD draws, so the HUD and the menus keep their own colours.
- **B** turns it on and off in game, with a `SHADERS ON` / `SHADERS OFF` toast. **On/off key** picks another key, or **None**.
- Every slider changes the picture straight away under **ADDON SETTINGS** in game.
- It needs Swofty Launcher 1.7.12 or later.

Keys do not fire while you type in a text box in the game, such as naming your team.

| Setting | Id | Default | Does |
|---|---|---|---|
| On/off key | `toggle_key` | `B` | The key that turns it on and off while F1 25 is in front |
| Look strength | `strength` | 100 | From the game's own picture at 0 to the full look at 100 |
| Brightness | `brightness` | 125 | Lifts the midtones; 100 leaves them as they are |
| Saturation | `saturation` | 110 | From grey at 0 to the game's own colour at 100 |
| White balance (K) | `white_balance` | 6000 | Lower is warmer; 6500 is neutral |
| Contrast | `contrast` | 100 | Contrast around mid grey |
| Bloom | `bloom` | 35 | The soft glow around bright lights and sky |
| Star glare | `star` | 30 | The six-point star on the brightest highlights |
| Glare threshold | `threshold` | 82 | How bright a pixel has to be to glow; lower glows more |

Its source is the PolyphoniX Native addon's own repository, and it is built on [post-process passes](/reference/addons/postfx).
