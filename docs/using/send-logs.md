---
title: Send your logs to the developers
description: Send the launcher and game logs to the Swofty Launcher developers in one click, with your Discord username and what went wrong.
---

# Send your logs to the developers

When something goes wrong, you can send your logs to the developers straight from the launcher.

1. Select **Send Logs to Developers** at the bottom of the sidebar, above **ACTIVE PROFILE**.
2. Enter your Discord username. The developers use it to reply to you.
3. Under **What went wrong?**, describe what happened and what you were doing when it went wrong, in 10 to 1000 characters.
4. Select **Send**. Until both fields are filled in, **Send** is greyed out and a hint under each field says what's missing.

When your data folder has no launch log and no game session, the dialog warns: **We don't see a game launch in your logs yet. Press Play and reproduce the problem first, then send logs.** You can still send, and `summary.txt` then says that no game session was found.

The dialog shows the progress, then **Sent — thanks!**. If the logs could not be sent, it shows the reason, such as no connection to Discord or the HTTP status Discord returned, and **Try again**. The launcher remembers your username for next time.

## When F1 25 crashes

When a game session ends in a crash, the launcher opens a full-window **F1 25 crashed** screen. It reads what the game left behind: the session's `loader.log`, the crash report and the files in the game folder.

- When it recognises the crash, the screen names the cause and the steps to fix it, with **Send Logs to Developers** and **Got it**.
- When it doesn't, the screen says **We couldn't tell why it crashed**, with **Send Logs to Developers**, **Join the support Discord** and **Close**.

| Cause | What the launcher looks for |
|---|---|
| F1 25 needs more free space | F1 25's own **Insufficient Storage** dialog before it closed. The screen names the drive, the space F1 25 needs there and how much to free |
| Windows blocked Swofty's files from loading | Exit code `0xc0e90002`, from Smart App Control or an app control policy |
| F1 25's game memory filled up | `Ne heap error 0xc` in the `GAME_DEFAULT` heap. The screen names the imported tracks on in the profile, Sepang first, and the number of Classic Seasons years. The step to turn a track off says that a career whose next round is at that track needs the round changed with the Career Mode Editor first |
| Windows ran out of memory | `Ne heap error 0x58` or `0x5`, or an `allocateAligned` / `useAddressSpace` failure: RAM and the page file are used up |
| F1 25 froze and closed itself | F1 25's own 30-second hang watchdog closed the game. The screen names an addon that kept the game busy while it froze, a graphics driver error just before it, and graphics mods in the game folder |
| Manual Pit Lane crashed F1 25 in the pit lane | A crash in F1 25's code just as Manual Pit Lane (`kizi.pitlane`) kept control of the car or read a garbage control mode for it, as happens on Return to Garage or a flashback in the pit lane |
| F1 25 + 26 Aggressive AI (Alternate) 3.0.0 crashed F1 25 while a session loaded | Version 3.0.0 of F1 25 + 26 Aggressive AI (Alternate) (`kizi.f1-25-more-aggresive-ai-2`) on in the profile, and a crash in or called from one of the three game functions that load its track files. The screen says to uninstall the addon and install it again from the Addon Browser, or to turn it off |
| Skip Cutscenes (up to 1.9.4) crashed F1 25 version 1.0.145 | Skip Cutscenes (`dromodf.skip-cutscenes`) up to version 1.9.4 on in the profile, the game exe is the 1.0.145 build (size and PE timestamp), and the crash is in `skip_cutscenes.dll` or the loader log says `vtable not found`. The screen says to turn it off on the Addons page until its author releases an update |
| Your career uses a track or season that is off | A crash in F1 25's code whose nearest named function on the stack is `Sess_Loader_SetupTrackAssets`, at most 10 seconds after `career.loaded` with no `career.unloaded` between, while a track or season addon is installed but off in the profile. The screen names the addon when one is off, and lists them all when several are |
| Your graphics driver stopped responding | `DXGI_ERROR_DEVICE_REMOVED` (`0x887a0005`) in the crash. When Swofty's in-game overlay drew just before, the screen names the addons it drew for. It also names full car packs on at once that replace each other's cars, and frame generation when it is on |
| F1 25 ran out of graphics memory | `E_OUTOFMEMORY` (`0x8007000e`) in the crash |
| A program that hooks into games | The crash in RivaTuner Statistics Server, Windhawk, Medal, Discord's overlay, OBS Studio's game capture, the NVIDIA app's overlay, the Steam overlay, Special K, or Windows' force feedback driver for a wheel (`pid.dll`). With no unhandled exception and no crash report, the last exception the mod loader saw counts when the game's exit code matches it |
| Graphics mods in the game folder | ReShade, OptiScaler, Special K or an ASI loader in the game folder, and a crash in graphics code or before the game's window opened |
| A crash in the graphics driver | The crash in an NVIDIA, AMD or Intel driver DLL |
| A crash in frame generation | The crash in DLSS Frame Generation or XeSS frame generation |

Under Wine or Proton, the graphics driver, graphics memory and Windows memory checks are skipped.

A game that stopped itself with exit code `0xc0000409`, `0xc0000602` or `0xc0000417` (a fast-fail) shows as a crash. The screen and the **Crash** section of `summary.txt` say it in one sentence, with the last lines of the session's `loader.log` when no crash report was written:

```text
Crash
No crash report was found (the game did not write one this session)
F1 25 stopped itself after detecting a corrupted stack or an invalid call (0xc0000409).
Last loader.log lines:
  [crash] crash log armed: fatal exceptions (first 16 in full, deduplicated by address), the top-level filter, ExitProcess hooked, TerminateProcess hooked
```

A session counts as a crash when it ended with a crash exit code, an exception the mod loader saw or F1 25's **Insufficient Storage** dialog, including a game the EA app started. A game that closes with another code, such as -1, shows as closed, not crashed. The screen opens by itself for a crash from the last 15 minutes. The play bar keeps **Crashed** with the cause, and **What happened** opens the screen again. `state/wrapper-run.log` gets a `[crash] known problem <id>: <title>` line, or `[crash] no known problem matches this crash`.

## Send the zip yourself

The launcher keeps a copy of the zip in `$DATA/logs/`, the newest 10. To send it to someone else, such as the author of a track or a mod:

1. Send your logs as above. The zip is kept whether or not they were sent.
2. Under the result, select **Click here to open them in File Explorer** (on Linux, **in your file manager**). The folder opens with the zip selected.
3. Drag the zip into Discord or a message.

On Linux, the launcher asks your file manager to select the zip over D-Bus (`org.freedesktop.FileManager1`). Without one that answers, it opens the `logs` folder.

## What gets sent

The logs go to the developers' Discord as one zip file, `swofty-logs-<username>-<time>.zip`. A large zip goes in several parts.

| In the zip | Contents |
|---|---|
| `summary.txt` | Your username and message, the launcher version, your operating system, CPU and memory (on Windows also the page file and the commit limit), the game's version and folder, a line when no launch log or game session was found, the addons on in the profile that draw over the game and, for each session, whether the overlay drew and which addons drew, a **Graphics and memory** section (each GPU's name, driver version and dedicated video memory, the PC's RAM and commit limit, and the newest per-minute line with RAM, commit, the game's working set and private memory, and video memory used against its budget, all from `loader.log`), a **Smoothness** section (for each loader log, how many minutes got each frame pacing verdict, the newest minute's pacing, presents, GPU, CPU, other processes and verdict lines, the roughest minute's pacing line and the newest display line; see [Frame pacing](/reference/std/logging#frame-pacing)), a summary of the latest crash report, the game files Windows keeps for you in `%LOCALAPPDATA%\VirtualStore` instead of the game folder, the summary and the per-minute pacing table of the latest performance report, and the active profile's addons by name: those on in load order, then the rest, each with its version, id, where it came from (built in, DB Editor, imported from which mod file, or installed) and the settings changed from their defaults |
| `crash/crash-<time>.txt`, `crash/crash-<time>.dmp` | The newest crash report the game wrote, from the game folder or from `state` in the data folder when the game couldn't write in its folder, and its minidump, when there is one. A report the loader logged but that was never saved is noted in the summary with the path from the log. The minidump is left out, noted in the summary, when it is too big for the zip |
| `crash/earlier/crash-<time>.txt`, `crash/dumps.txt` | Up to four crash reports before the newest, so a hang that follows an earlier crash still shows both, and a list of every report and minidump found with their sizes. Only the newest minidump is sent |
| `game/loader.log`, `game/perf.log` | The loader log and the performance report of the game that's running |
| `state/*.log` | The launcher's own logs, such as `launcher-run.log` and `wrapper-run.log`, `launcher-app.log` with each start of the launcher, whether it ran as administrator, what **Restart as administrator** did, each action in the launcher window that failed with its addon, files and error, and any panic, and, for a game started through the EA app, whether the launcher and the EA app run as administrator, the moment the launch link is sent, every `F1_25.exe` or EAAntiCheat process that appears afterwards with how long it lived and its exit code, and what was seen when the wait for the game timed out, and `loader-fallback.log`, the loader's log when the game couldn't write in its folder |
| `ea-app/*.log` | On Windows, the last 256 KB of the four newest logs the EA app wrote in `%LOCALAPPDATA%\Electronic Arts\EA Desktop\Logs` and `%ProgramData%\EA Desktop\Logs` |
| `windows/application-events.txt` | On Windows, up to 40 entries from the last 24 hours of the Application event log that name `F1_25.exe`, EAAntiCheat or the EA app, such as Application Error and Windows Error Reporting entries |
| `sessions/<time>/` | The logs and small files of your three latest game sessions, such as `loader.log`, `perf.log`, `env.txt`, `exit_code` and `end.json` |
| `addons/<id>/helper.log` | Each addon helper's log |
| `addons.json` | The id, name, version, source, category, types, origin and experimental features of every installed addon, and each profile's addons with their names and settings |
| `conflicts.json` | From the active profile's last build: each pair of addons that change the same thing, which one wins, and the records, resources, files and text they both change |
| `config.json` | The launcher's settings |
| `game/career-editor/snapshot.json` | The Career Mode Editor's snapshot, when the game folder has one |

The logs can include folder paths with your computer's user name, your hardware, and your installed mods and profiles. Passwords, tokens and keys in `config.json`, in addon settings and in each session's `env.txt` are replaced with `[redacted]`. Each file is cut to its last 5 MB, and the whole zip holds at most 40 MB before compression.

A bot in the developers' Discord reads each report with Anthropic's Claude API to suggest a cause, so the zip's contents are also sent to Anthropic.

## From the command line

```sh
swofty logs send --name <your Discord username> --message "What happened and what you were doing"
```

`--message` takes 10 to 1000 characters, and the command stops without it unless you add `--dry-run`. When your data folder has no launch log and no game session, the command warns first. The command keeps the zip in `$DATA/logs/` too and prints its path. To write the zip and list what it holds without sending it, add `--dry-run`. See [`swofty logs send`](/reference/cli#swofty-logs-send).

Builds from source don't include the developers' Discord address. There, the button reads **Save Logs**: it saves the zip to `$DATA/logs/` without sending it, then offers the same link to the zip. `swofty logs send` only works with `--dry-run`.
