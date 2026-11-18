---
title: Steam and EA app copies
description: Pick which copy of F1 25 Swofty Launcher plays when you have it on Steam and in the EA app, add a game folder, and set up the EA app copy on Linux.
---

# Steam and EA app copies

From 1.3.1, the launcher plays any copy of F1 25 you have: Steam's, the EA app's, or a game folder you add. **Play**, `swofty run`, `swofty apply` and `swofty build` use the active copy.

## Switch copies

The bottom-left of the launcher shows the active copy: its version, a **STEAM** or **EA APP** badge, and how it runs.

1. Select the game name next to the cover art.
2. Under **Play this copy**, select the copy you want. Each one shows its store, version and folder, and the active one has a check.

The launcher remembers your choice. If the active copy is uninstalled, the launcher plays the first copy it finds instead.

## While F1 25 is running

The launcher matches a running F1 25 to a copy by the folder its `F1_25.exe` runs from.

- **Stop** closes the active copy's game only.
- While another copy's game runs, the play bar says which store it's from and **Play** is unavailable. You can still switch copies, and **Switch to it** makes the running copy the active one.
- The launcher never closes a running F1 25 it can't match to the active copy.
- `swofty run --restart` closes a running F1 25 only when it's the copy being played.

## Add a game folder

1. Select the game name, then **Add another folder…**. When no copy is found, select **Locate game** on the **Play** button instead.
2. Type the folder's path, or select **Browse…** and choose it. It's the folder with `F1_25.exe`.
3. Select **Add and play this copy**.

The folder becomes the active copy. A folder with `__Installer/installerdata.xml` gets the **EA APP** badge.

## Where the launcher finds copies

| Copy | Found from |
|---|---|
| Steam | The Steam library that has app 3059520 |
| EA app on Windows | `Install Dir` under `HKEY_LOCAL_MACHINE\SOFTWARE\Codemasters\F1_25`, then the Windows uninstall entries, then the EA app's download folders, then `Program Files\EA Games` |
| EA app on Linux | Added with **Add another folder…** |
| Any other folder | Added with **Add another folder…** |

An EA app copy needs `F1_25.exe` and `__Installer/installerdata.xml` in its folder.

**Play** refuses a copy in any other folder: modded sessions run only on a copy Steam or the EA app installed. See [Copies Swofty plays](/reference/launch#copies-swofty-plays).

The EA app's download folders are the `downloadinplacedir` lines in `%ProgramData%\EA Desktop\machine.ini` and `%LOCALAPPDATA%\Electronic Arts\EA Desktop\user_<id>.ini`.

An added folder that no longer has the game shows as **Folder not found** in the switcher, and **Play** uses the first copy found instead.

## The store starts with the game

**Play** starts the store the active copy came from:

- **Steam copy:** the launcher starts Steam and waits for it to sign in. See [Steam](/reference/launch#steam).
- **EA app copy:** the launcher starts the EA app, installs your profile into the game folder, then asks the EA app to launch F1 25. See [EA app](/reference/launch#ea-app).

To turn this off, select the arrow next to **Play** and clear **Start Steam automatically** or **Start the EA app automatically**. One setting covers both stores.

## Repair the game files

Each store puts its own game files back:

| Copy | Repair |
|---|---|
| Steam | **Verify game files in Steam** in the launcher opens `steam://validate/3059520`, Steam's **Verify integrity of game files** for F1 25. In Steam, it's under right-click F1 25 > **Properties** > **Installed Files** |
| EA app | **EA app** > **Library** > **F1 25** > **Manage** > **Repair**. The EA app has no link that starts a repair |

When files in the Steam copy were changed outside Swofty, the launch stops and **Your game files have been edited** offers **Verify game files in Steam**. See [Game files changed outside Swofty](/reference/launch#game-files-changed-outside-swofty).

Play starts when one of the game's DLLs is not Steam's copy, as with an FSR 4 build of `amd_fidelityfx_dx12.dll`, a VR runtime's `openvr_api.dll` or a DLSS swap. It stays as you put it, and Verify integrity of game files in Steam puts Steam's copy back. A DLL missing from the game folder still stops the launch. `F1_25.exe` and `steam_api64.dll` must still be Steam's.

An EA app copy is not checked against a list of its files. When the launcher finds files in its folder that it didn't put there, the same modal shows the EA app's repair steps instead.

## A game folder only administrators can change

The EA app installs F1 25 in `C:\Program Files\EA Games\F1 25` by default, a folder Windows lets only administrators change. Swofty can't add your mods there while Swofty Launcher runs normally. **Play** then stops before it changes anything and shows **F1 25 needs administrator rights**, naming the folder:

- **Restart as administrator** opens Swofty Launcher again with administrator rights, after Windows asks you to allow it. This one minimizes and closes once the administrator copy has run for 6 seconds. If you decline, or the administrator copy closes before then, this one stays open and says so. `state/launcher-app.log` in the data folder records each step.
- Or move F1 25 to a folder outside Program Files with **EA app** > **F1 25** > **Manage** > **Move**, not File Explorer, then press **Play** again.

F1 25 started from the EA app can run without administrator rights even when Swofty Launcher runs as administrator and starts the EA app itself. The game then can't create files in its folder. The loader writes its log to `state/loader-fallback.log` in the data folder, and addons that save files through Swofty keep them in `addon-storage/<id>/` in the data folder for that session. The session says so on `[loader]` and `[warning]` lines in `state/wrapper-run.log`. An addon that writes in the game folder by itself can't save there; moving F1 25 out of Program Files with **EA app** > **F1 25** > **Manage** > **Move** fixes that. Restart as administrator from your own Windows account: an administrator account with a different user name has its own Swofty data folder.

`swofty run --apply` and `swofty apply` stop with the same message. Steam library folders can be changed without administrator rights.

## Programs set to run as administrator

On the EA app copy, **Play** stops before it changes anything and shows **A program is set to run as administrator** when Windows won't start a program the session needs. The EA app runs without administrator rights, and Windows starts a program set to **Run this program as an administrator** only for a program that has them (error 740). The dialog names each file, usually one of these:

- `EALauncher.exe` or `EADesktop.exe` in `C:\Program Files\Electronic Arts\EA Desktop\EA Desktop`, the programs the launcher hands the launch link to
- `F1_25.exe` or `EAAntiCheat.GameServiceLauncher.exe` in the game folder, for a modded session

The EA app starts F1 25, so for the game's files running Swofty Launcher as administrator doesn't help. For each file the dialog names:

1. Right-click it and select **Properties**.
2. Open **Compatibility** and untick **Run this program as an administrator**.
3. Select **Change settings for all users** and untick it there too.
4. Select **OK**, then **Play** again.

When Windows refuses one of them after the launch link, the same dialog ends the launch at once, and the game folder goes back to vanilla. See [Programs set to run as administrator](/reference/launch#programs-set-to-run-as-administrator).

## An EA app copy moved by hand

The EA app starts F1 25 only from the folder it has on record: `Install Dir` under `HKEY_LOCAL_MACHINE\SOFTWARE\Codemasters\F1_25` (and the other EA keys) and the F1 25 uninstall entry. A copy moved in File Explorer stays on record at its old folder, and the EA app shows F1 25 as not installed.

Before **Play** builds anything for an EA app copy, it compares the copy's folder with that record. When the EA app has F1 25 in another folder, **Play** stops with **The EA app has F1 25 in another folder**, naming both folders:

- Move the folder back to the one the EA app has, then press **Play** again.
- To keep F1 25 somewhere else, move it back first, then use **EA app** > **F1 25** > **Manage** > **Move**.
- When the game is in the recorded folder, the **Play the copy in** button, which names that folder, switches to that copy.

`swofty run` and `swofty game launch` stop with the same message, and name `swofty game use "<folder>"` when the game is in the recorded folder. With no record in the registry, as on Linux, there is no check.

## The EA app copy on Linux

On Linux, the launcher hands the EA app a launch link through a command you set. Set `ea_launch_command` under `"launch"` in `$DATA/config.json`, with `{url}` where the link goes. This example is for a script whose `run` starts the EA app in its Proton prefix and whose `wine` runs a program in the running EA app's prefix:

```json
{
  "launch": {
    "ea_launch_command": "u={url}; L='C:\\Program Files\\Electronic Arts\\EA Desktop\\EA Desktop\\EALauncher.exe'; if [ -z \"$u\" ]; then exec setsid -f /path/to/ea-app.sh run \"$L\"; else exec /path/to/ea-app.sh wine \"$L\" \"$u\"; fi",
    "ea_prefix": "/path/to/EAApp/pfx"
  }
}
```

- `ea_launch_command` runs with `sh -c`. With a link, it must reach the EA app that is already running, in the same Wine prefix.
- When the EA app is closed, **Play** runs `ea_launch_command` with `{url}` left out to start it, waits up to 90 seconds for `EADesktop.exe`, then 10 seconds more before it hands over the link. When that command exits with an error, the launch link starts the EA app instead.
- The EA app must keep running after `swofty` exits. A sandbox that ends with its parent, such as `steam-run`, needs `setsid -f` in front of the command that starts the EA app.
- `ea_prefix` is the EA app's Wine prefix: the folder with `user.reg`, or the Proton folder that holds `pfx`. Before it starts the EA app, the launcher writes the in-game loader's DLL setting there and leaves it in place. Wine reads it only when the EA app starts, so the first modded **Play** with the EA app open closes the EA app and starts it again.

Without `ea_launch_command`, **Play** on the EA app copy stops with a message naming the setting.

## From the command line

```sh
swofty game list
swofty game use steam
swofty game use ea
swofty game use "/games/F1 25"
swofty game add "/games/F1 25"
swofty game launch --vr
```

- `swofty game list` shows each copy's store, version and folder, with `*` next to the active one.
- `swofty game use` picks the active copy by store (`steam`, `ea`) or folder.
- `swofty game add` adds a folder and makes it the active copy.
- `swofty game launch` asks the EA app to start the EA app copy as it's installed, without your profile or offline play. Add `--vr` for VR.
- `--game <dir>` on any command plays that folder for one run.
- `swofty doctor` lists every copy and marks the active one.
