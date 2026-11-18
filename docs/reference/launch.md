---
title: Launching the game
description: How Swofty Launcher for F1 25 starts a modded session on Windows and Linux, where the logs go, and what happens when the game exits.
---

# Launching the game

Modded sessions run offline. To get set up, follow [Windows](/guide/windows) or [Linux and Steam Deck](/guide/linux).

## Steam

F1 25 from Steam needs the Steam client running and signed in. When you select **Play** or run `swofty run` while Steam is closed, the launcher starts Steam in the background, waits for it to sign in, then launches the game. The status line next to **Play** reads **Starting Steam…**, then **Waiting for Steam to sign in…**.

When Steam doesn't come up, the launch stops with one of these messages:

| Message | What to do |
|---|---|
| `Steam didn't start within 90 s; open Steam and press Play again` | Open Steam yourself, then select **Play** again |
| `Steam is open but not signed in; sign in to Steam and press Play again` | Sign in to Steam, then select **Play** again |
| `could not start Steam (...); open Steam and press Play again` | Steam wasn't found or exited with an error. Open Steam yourself, then select **Play** again |

On Linux, when Steam runs but never reports signing in, the launch goes ahead after 90 seconds.

To turn this off, select the arrow next to **Play** and clear **Start Steam automatically**, or set `"start_store": false` under `"launch"` in `$DATA/config.json` (`"start_steam"` still works). For a single launch, run `swofty run --no-start-store`. With it off, the launch goes ahead whether Steam runs or not. If Steam is open and the launch still waits for it, turn the setting off and tell the developers with [Send Logs to Developers](/using/send-logs).

The launcher doesn't start Steam for a game folder outside a Steam library, or in Steam Deck Game Mode.

| | Running and signed in | Started with |
|---|---|---|
| Windows | `HKCU\Software\Valve\Steam\ActiveProcess`: `pid` is a running `steam.exe` and `ActiveUser` isn't 0. When `ActiveUser` stays 0, it's signed in once the newest `Timestamp` in `config/loginusers.vdf` is after that `steam.exe` started and at least 20 seconds old | `SteamExe` or `SteamPath` from `HKCU\Software\Valve\Steam`, with `-silent`; otherwise `steam://open/main` |
| Linux | `~/.steam/steam.pid` or `registry.vdf` names a running `steam` process. It's signed in when `ActiveUser` in `registry.vdf` isn't 0, or when `config/loginusers.vdf` changed after Steam started | `steam -silent` |
| Flatpak Steam | The same files under `~/.var/app/com.valvesoftware.Steam/` | `flatpak run com.valvesoftware.Steam -silent` |

`swofty run` and the launcher's log (`$DATA/state/launcher-run.log`) show each step on `[steam]` lines:

```text
[steam] detected: not running
[steam] starting Steam: /usr/bin/steam -silent
[steam] waiting for sign-in (pid 4242, ActiveUser 0) at 3.5 s
[steam] ready after 9.0 s (pid 4242, ActiveUser 12345678)
```

`swofty steam` shows the same check on its own, and `swofty steam --start` runs the start and the wait without launching the game. `swofty doctor` shows it in its `steam client` row.

## EA app

From 1.3.1, **Play** on an [EA app copy](/using/game-copies) runs in this order:

1. The launcher starts the EA app when it's closed, waits up to 90 seconds for `EADesktop.exe`, then waits 10 seconds more. On Windows it runs `EALauncher.exe`; when the launcher runs as administrator, `explorer.exe` starts it, and the EA app runs with your own rights, not as administrator. On Linux it runs `ea_launch_command` without the link. The status line reads **Starting the EA app…**.
2. It installs your profile and offline play into the game folder. With the vanilla profile, offline play stays out: `EAAntiCheat.GameServiceLauncher.exe` stays as the EA app installed it, and EA AntiCheat runs.
3. It hands the EA app the launch link `origin2://game/launch?offerIds=<ids>&autoDownload=1`, with `&cmdParams=-vr` for **Play in VR**. The IDs are the `<contentID>` values in the game folder's `__Installer/installerdata.xml`. The status line reads **Starting F1 25 through the EA app…**.
4. It waits for F1 25 to start and close, including a second copy the EA app starts, then rolls the game folder back.

When F1 25 hasn't started 5 minutes after the link, the launcher opens the EA app and the status line reads **Press Play on F1 25 in the EA app**. Pressing **Play** there starts the modded session. After 10 more minutes without the game, the launch stops and the game folder rolls back. The `[ea app]` line after the link says whether offline play's file is still in place. When the EA app took it out to repair F1 25, the status line reads **The EA app is repairing F1 25** instead, and F1 25 starts without mods after the repair.

### Programs set to run as administrator

The EA app runs without administrator rights, and Windows won't start a program set to **Run this program as an administrator** for it (error 740). Before the launcher starts the EA app or changes the game folder, it reads that setting for you and for all users, under `HKEY_CURRENT_USER` and `HKEY_LOCAL_MACHINE` in `Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers`. The launch stops there with **A program is set to run as administrator** when one of these is set:

- the program that hands the EA app the launch link, usually `EALauncher.exe`, while the launcher runs without administrator rights. The message also names `EADesktop.exe` and `EALauncher.exe` wherever they are set.
- `F1_25.exe` or offline play's `EAAntiCheat.GameServiceLauncher.exe` in the game folder, for a modded session, while the EA app runs without administrator rights. Running the launcher as administrator doesn't change that, and the message says so.

The message names each file, with what to untick: right-click the file, open **Properties > Compatibility**, untick **Run this program as an administrator**, and untick it under **Change settings for all users** too. Then select **Play** again.

When Windows refuses a start the check didn't see, the launch stops at once with the same message: when the launch link's hand-off or the EA app itself fails with error 740, or when offline play's session launcher logs `CreateProcessW failed, error 740` after the link. The launcher doesn't wait the 5 and 10 minutes for F1 25 then. The game folder rolls back and the session's lock is freed before the message shows, so the next **Play** starts straight away.

When the EA app repairs F1 25 before it starts the game, the repair puts EA's `EAAntiCheat.GameServiceLauncher.exe` over offline play's file, and F1 25 starts without mods. Once F1 25 has started, the launcher puts offline play's file back, keeps EA's file as the backup the roll-back restores, and opens **The EA app repaired F1 25**. Close F1 25 and select **Play** in the launcher again. An EA app that asks to repair F1 25 on every **Play** checks that file on that PC, and modded play doesn't start through it; the vanilla profile still plays.

| | Running | Launch link handed to |
|---|---|---|
| Windows | An `EADesktop.exe` process | The `origin2` handler in `HKEY_CLASSES_ROOT\origin2\shell\open\command`; otherwise `EALauncher.exe` from `HKEY_LOCAL_MACHINE\SOFTWARE\Electronic Arts\EA Desktop` or `Program Files\Electronic Arts\EA Desktop\EA Desktop` |
| Linux | An `EADesktop.exe` process under Wine | The `ea_launch_command` setting; see [The EA app copy on Linux](/using/game-copies#the-ea-app-copy-on-linux) |

On Windows, `ea_launch_command` replaces the handler when it's set. The EA app registers the handler with an unquoted path, and the launcher runs everything up to `.exe` as the program.

When the EA app doesn't come up or the launch link doesn't reach it, the launch stops with one of these messages:

| Message | What to do |
|---|---|
| `the EA app didn't start within 90 s; open the EA app and press Play again` | Open the EA app yourself, then select **Play** again |
| `could not start the EA app (...); open the EA app and press Play again` | Open the EA app yourself, then select **Play** again |
| `could not run <program> to hand the launch link to the EA app (...); open the EA app and press Play again` | Open the EA app yourself, then select **Play** again |
| `EALauncher.exe is set to run as administrator, so Windows won't start it for an EA app session (error 740). ...` | Untick **Run this program as an administrator** on each file the message names; see [Programs set to run as administrator](#programs-set-to-run-as-administrator) |
| `the EA app is closed and starting it is turned off ...` | Open the EA app, or turn **Start the EA app automatically** back on |
| `the EA app was not found on this computer ...` | Install or repair the EA app |
| `... Swofty starts it through the EA app with the launch.ea_launch_command setting ...` | Set `ea_launch_command` on Linux |

`swofty run` and the launcher's log show each step on `[ea app]` lines:

```text
[ea app] detected: closed; starting the EA app
[ea app] running after 6.5 s
[ea app] asking the EA app to launch F1 25: origin2://game/launch?offerIds=16425845,16425845_oa&autoDownload=1
[ea app] F1 25 started (pid 5120)
```

`swofty doctor` shows whether the EA app runs and how the link reaches it, in its `ea app` and `ea app hand-off` rows. An EA app copy always starts through the EA app; `swofty run --direct` is for Steam copies. `swofty game launch` hands the EA app the launch link for the copy as it's installed, without your profile or offline play.

## Copies Swofty plays

Modded sessions run only on a copy Steam or the EA app installed. Before a session or `swofty apply` changes anything, the launcher checks that:

- The copy is a Steam copy or an EA app copy. A copy in any other folder is refused.
- The game folder has none of the files Steam emulators and cracks leave: `steam_settings`, `steam_emu.ini`, `steam_api64_o.dll`, `steam_interfaces.txt`, `ColdClientLoader.ini`, `steamclient_loader_x64.exe`, `steamclient64.dll`, `OnlineFix64.dll`, `OnlineFix.ini`, `cream_api.ini`, `CODEX64.dll` and `SmartSteamEmu.ini`. A file in Steam's list for the install is never one of them.
- `F1_25.exe` and `steam_api64.dll` match the size and SHA-1 in Steam's list, on a Steam copy whose list the launcher reads. A changed one opens **Your game files have been edited**; see [Game files changed outside Swofty](#game-files-changed-outside-swofty).
- An EA app copy starts through the EA app.

`swofty run` and `swofty apply` stop with the reason:

```text
swofty: this copy of F1 25 wasn't installed by Steam or the EA app, so it can't be played modded; Swofty runs only on a copy you own on Steam or the EA app
```

## Game files changed outside Swofty

Before a modded session builds anything, the launcher checks the Steam copy against Steam's list of the files it installed. When files were changed outside Swofty, usually by a mod copied into the game folder by hand, F1 25 is not started and nothing in the game folder is changed. A DLL other than `steam_api64.dll` that differs from Steam's copy is logged and doesn't stop the launch; a missing one does. [Checking against Steam](/reference/restore#checking-against-steam) lists what the check compares.

The launcher opens **Your game files have been edited**. It says how many files changed, names the first six, and shows the fix:

1. Select **Verify game files in Steam**. It opens `steam://validate/3059520`, Steam's **Verify integrity of game files** for F1 25, which puts Steam's copies back. In Steam, it's under right-click F1 25 > **Properties** > **Installed Files**.
2. When Steam has finished, select **Play** again.

**What to do** on the play bar reopens the modal. An EA app copy is not checked against a list of the EA app's files; see [Repair the game files](/using/game-copies#repair-the-game-files).

`swofty run` and `swofty apply` stop with the same fix and the same link:

```text
swofty: common.erp, ferrari.erp, force_india.erp and 49 more files are not the copy Steam installed, usually after a mod was copied into the game folder by hand; nothing in the game folder was changed; verify the game files in Steam to put them back (open steam://validate/3059520, or F1 25 > Properties > Installed Files > Verify integrity of game files), then try again
```

## Game files missing from the game folder

A game file the profile replaces that is missing from the game folder is put back from the launcher's backup of the original, and the install goes on. With no backup, F1 25 is not started and nothing in the game folder is changed. The launcher opens **Some game files are missing**, names the files, and shows the fix for the copy's version:

| `F1_25.exe` version | Fix |
| --- | --- |
| Older than 1.0.145.2472 | Update F1 25 in Steam or the EA app |
| 1.0.145.2472 or newer | Verify the game files in Steam, or repair them in the EA app |
| Unknown | Update F1 25, then verify the game files |

`swofty run` and `swofty apply` stop with the same fix:

```text
swofty: nico_rosberg_head_d.tif.2024.mipmaps, nico_rosberg_head_n.tif.2024.mipmaps and nico_rosberg_head_s.tif.2024.mipmaps are missing from the game folder; nothing in the game folder was changed; verify the game files in Steam (open steam://validate/3059520, or F1 25 > Properties > Installed Files > Verify integrity of game files) or repair the game in the EA app to put them back, then try again
```

## Addon files missing or damaged

A build that stops on an addon file that is missing or shorter than it should be names the addon and the file:

```text
swofty: Igora Drive (mod.miami24) is missing its file track/miami24.erp. Turn Igora Drive off in this profile, or remove it on the Addons page and install or import it again.
```

A game file that is shorter than it should be is named with the fix to verify the game files in Steam or repair them in the EA app. An addon install that fails, for example on a full drive, leaves the addon as it was before the install: an addon that wasn't installed stays uninstalled, and an update keeps the previous version.

## An older F1 25

Before a modded session, `swofty run` and the launcher's **Play** compare `F1_25.exe`'s version with 1.0.145.2472, the latest update. On an older copy the launcher shows **F1 25 needs the latest update** and the session goes on; `swofty run` prints the same on a `[warning]` line. Mods may not load on an older copy.

## Linux: the Steam wrapper

Set Steam's launch option to `swofty run -- %command%` so its Play button starts Swofty Launcher for F1 25. The launcher uses your configured Proton version and records session logs.

To go back to Steam's normal launch, clear the option.

Without the wrapper in the launch option, the launcher's **Play** and `swofty run --direct` start Proton themselves. Proton reads wheels and controllers through SDL2. When the system has no 64-bit SDL2, as under NixOS `steam-run`, the launcher links Steam's own copy into `$DATA/state/runtime-libs` and adds it to the game's library path: the 64-bit client's SDL3 (`steamrt64/libSDL3.so.0`) with the SDL2 layer from its runtime, or the old `ubuntu12_32/steam-runtime` SDL2 on clients that still have it. `swofty run` then prints `controllers: this system has no SDL2`.

## Windows

Select **Play** in the launcher or run `swofty run`. If Steam is closed, it [starts first](#steam). To follow the logs in the launcher, select the arrow next to **Play** and choose **Play with console**.

On a Steam copy, modded sessions don't start while `F1_25.exe` is set to **Run this program as an administrator** and the launcher runs normally: Windows won't start the game unelevated (error 740), and the launch stops with a message naming the setting. To fix it, right-click `F1_25.exe` in the game folder, open **Properties > Compatibility** and untick the box, or untick it under **Change settings for all users** when it's set for all users. You can also run the launcher as administrator. On an EA app copy, the EA app starts F1 25, and running the launcher as administrator doesn't help: untick the setting. See [Programs set to run as administrator](#programs-set-to-run-as-administrator). `swofty doctor` shows the setting in its `run as admin` row.

The launcher doesn't pass its `__COMPAT_LAYER`, the variable Windows uses to hand compatibility settings on to the programs a program starts, to F1 25, to the offline play session launcher, or to the EA app when it starts the EA app. The launch log says `environment: not passing the Windows compatibility layer __COMPAT_LAYER=<layers> to the game` when it drops one. A `__COMPAT_LAYER` under `"env"` in `"launch"` in `$DATA/config.json` is still passed. Each session's `env.txt` lists the environment the launcher starts the game with.

Modded sessions also don't start while the game folder is one this process can't create files in, such as the EA app's `C:\Program Files\EA Games\F1 25`, and the launcher runs normally. The check creates and deletes a file in the game folder and in `2025_asset_groups/ui_package`, the first folder an install writes in. A file that Windows puts in `%LOCALAPPDATA%\VirtualStore` instead counts as one it can't create. The launch stops before the build with a message naming the folder; the launcher offers **Restart as administrator**. See [A game folder only administrators can change](/using/game-copies#a-game-folder-only-administrators-can-change).

## VR

**Play in VR** and `swofty run --vr` pass `-vr` to the game, the argument of Steam's VR launch option for F1 25. On Linux, SteamVR starts first when it isn't running, and the Steam wrapper passes on the arguments Steam gives the game, so Steam's **Launch in SteamVR mode** works too. See [Playing in VR](/using/vr).

## Logs

You'll find session logs in `$DATA/sessions/<id>/`. Run `swofty logs` for the latest summary, or use `swofty run` to follow the logs as you play.

If Proton can't create the game process, you'll see `Proton could not start the game process (CreateProcess error N)` instead of an exit code in `swofty run`, `swofty logs` and the launcher.

Each session's `end.json` says how the game ended, with its exit code (`null` when the session couldn't read it) and the reason:

| `end` | When |
|---|---|
| `quit` | The game exited with code 0. |
| `crash` | The game exited with a crash code (`0xC0000000` and up, other than -1), or `loader.log` shows an unhandled exception before the exit. |
| `stopped` | **Stop** in the launcher, `swofty run --restart` or a stop signal to the wrapper closed the game. |
| `unknown` | Anything else, such as -1 with no unhandled exception, or an EA app launch with no exit code. F1 25's crash handler exits with -1, and the game also leaves -1 when it fails while closing. |

The launcher shows a crash for `crash`, and for `unknown` with a non-zero exit code. It doesn't show one after **Stop**.

When the game exits, however it exits, the session puts the game folder back to vanilla. To check, run `swofty verify`. See [Restoring the game](/using/profiles-and-restoring) for the cases that need `swofty restore`.

## Next

- [Offline play](/reference/safety): what a modded session turns off.
- [Backups and recovery](/reference/restore): how the session rolls the game folder back.
