---
title: Settings
description: Settings holds the launcher's own options in five categories, shows where Swofty keeps its files and how much room they take, and clears old session logs.
---

# Settings

**Settings**, at the bottom of the sidebar, has a category list on the left and the chosen category on the right. Press Tab to move through the list and Enter to open a category.

| Category | What it holds |
|---|---|
| **General** | Language, starting your game's store, profiles and addon updates |
| **Display** | PC or TV mode and the interface scale |
| **Storage** | Where your addons and saves are, finding them again, their size, and cleanup |
| **Advanced** | Developer mode and clearing old logs automatically |
| **About** | The launcher's version, update status and links |

## General

**Language** is a drop-down listing each language by its own name: English, Deutsch, Español, 한국어, Português (Brasil), Русский or 简体中文. It changes straight away. The sidebar, the **Play** bar, **Profiles**, **Settings** and the crash screen are translated so far, and everything else stays in English, including the causes the crash screen names. Addons and F1 25 keep their own language.

**Start Steam automatically** (or the EA app) starts your game's store when you press **Play**, as the same item in the **Play** menu does.

**Use profiles** is on by default. Turned off, the launcher hides the profile sidebar, profile names in the **Play** bar, the **Addons** heading and the **Saves** page, and the **Copy careers to** action, and you can no longer switch profiles. It keeps working on the profile that is active when you turn it off, or on `default` (else your first modded profile) if that was `vanilla`. Turning it on again brings the full interface back, and your profiles are never deleted.

**Update turned-off addons** is off by default: **Update addons** in the **Play** bar updates only the addons that are on in your active profile, and updates for the others never take the place of **Play**. Turned on, **Update addons** also updates every installed addon that is off, in every profile including Vanilla, and their updates turn **Play** into **Update addons** too (see [Addon updates](/using/updating#addon-updates)). It covers addons the Addon Browser lists. Simplified for Swofty, the launcher's own updates and the addons that ship with the launcher update as before, and an install or update still installs or updates the requirements it needs, on or off. `config.json` keeps it as:

```json
{
  "update_off_addons": true
}
```

**Home screen video** shows the latest Swofty video in the bottom right of the F1 25 home screen, with **RB** (**V** on a keyboard) to watch it. It is on by default. Turned off, the home screen has no video card from the next time you press **Play**, and the launcher no longer fetches the latest video.

**Ad privacy choices** opens the ad privacy form again, to change how ads in the launcher may use your data. It shows only to players who see ads.

## Display

**Display mode** switches between **PC mode**, the normal layout, and **TV mode**, which scales the whole launcher up by 30% and widens the scrollbars, for handhelds and couch play. The welcome wizard asks for it, with your language, the first time the launcher opens and once after an update that changes the wizard. Each choice applies as you pick it, Esc or **Skip** closes the wizard, and you can change both here at any time.

**Interface scale** makes the whole launcher larger or smaller on top of the display mode. **Auto** is the default and keeps the size the display mode gives. The slider runs from 1% to 400% in steps of 1%, and the percentage multiplies the display mode's size: TV mode at 75% is about 98% of the PC layout. The change applies when you let go of the slider, and the launcher keeps it for next time. **Reset to automatic** returns to Auto. Below 60% a **Reset interface scale** button stays in the bottom right corner at a fixed size. On a Steam Deck in gaming mode, TV mode with **90%** or **75%** fits the 1280x800 screen with room to spare.

## Storage

Storage has three sections. Moving the launcher to another drive never moves your addons and saves.

| Section | Row | What it shows |
|---|---|---|
| **Location** | **Your addons and saves** | The folder where your addons, profiles, backups and logs live, shown with its path (see [where your files are stored](/guide/installation#where-your-files-are-stored)). **Open** opens it in File Explorer, or your file manager on Linux. **Move…** moves it to another folder or drive. |
| | **Find my addons** | **Find** looks in the places an old copy may be and offers to use one (see below). It says **Nothing found** when there is none. |
| **Usage** | **Size** | The folder's total size and the free space on its drive. When F1 25's own drive is another one, also the free space there: the drive with your Windows profile, or on Linux the drive with the game's Proton prefix (its Steam library). |
| | **Breakdown** | How much each part takes, largest first: addons, the game backup, session logs, the cache, the career vault, save snapshots, addon data and the rest. |
| **Cleanup** | **Session logs** | How many game sessions are kept. **Clear old** removes the old ones (see below). |
| | **Game drive copy** | The size of the copy of your mods that Play keeps on F1 25's drive (see below). **Clear** deletes it. |

When either drive has less than 3 GB free, **Usage** says so: F1 25 won't start with less than 2 GB free on its `C:` drive, which under Proton is the prefix.

Moved addons and saves are measured where they really are.

**Refresh**, in the **Usage** heading, measures the folder again. Measuring walks every file, so a large addon collection takes a few seconds. Storage measures when you open it.

### Clearing old session logs

Every game session leaves its logs in `sessions` in the data folder, and nothing removed them before. **Clear old** keeps the newest 20 sessions and every session from the last 2 days, and removes the rest. It doesn't run while F1 25 is open. The button names how many sessions it would remove and how much room that frees; it reads **Nothing to clear** when every session is new enough to keep.

**Clear old** asks you to confirm before it deletes anything. The launcher keeps every log unless you turn on **Clear old logs** under Advanced: with it on, the launcher clears old logs by the same rule each time it starts, without asking. It is off by default, so updating never deletes logs.

### Moving your addons and saves

**Move…** next to **Your addons and saves** moves your addons, profiles, backups and logs to another folder or drive without the installer.

1. Select **Move…** and choose a folder. A folder that has files in it gets a `Swofty` folder inside it.
2. The launcher shows where the data is now, where it will go, its size and the free space there. Select **Move** to start.
3. A full-window screen shows the progress and the file being copied. **Cancel** stops the copy and removes it; your addons and saves stay where they were. Nothing else in the launcher works while it runs, and the window doesn't close.
4. When the copy checks out, the launcher restarts, switches to the new folder, deletes the old copy and shows a message. The old location keeps working: Swofty finds your addons and saves at the same path as before.

On the same drive the folder is renamed and nothing is copied. A copy is checked by size, and by content for the small files. The launcher's own logs under `state` and `logs` grow while it copies; for those the copy has to match the start of the original, and the restart copies the rest.

The launcher refuses to start the move while F1 25 is open, a build is running or downloads are queued. It refuses a whole drive, a folder inside or around the launcher's own folder, inside or around the F1 25 folder, inside the folder they are in now, one that isn't empty, one it can't write to, and one on a drive without room for the data plus a margin. When `SWOFTY_DATA_DIR` sets the folder, it refuses and says so: change the variable instead.

If the launcher or the computer stops during a move, the next start finishes it or puts everything back. When the old copy can't be deleted, the message names it so you can delete it yourself.

### Finding your addons and saves

When the launcher starts with none of your own addons, it looks in a fixed list of places, and a prompt appears when it finds a folder that has addons of yours and a config, profiles or saves: **We found your addons and saves in** the folder, with the number of addons, its size and when it was last used. Several finds are listed newest first.

- **Use them** points Swofty at that folder and restarts the launcher. The empty folder it made is kept beside it as `Swofty.fresh-<date>`, and the launcher never deletes it. A folder inside the launcher's own folder is moved out to the usual place instead.
- **Not now** closes the prompt until the next start.
- **Don't ask again** turns the prompt off. **Find** under **Find my addons** still works.

The places are the folders next to your Swofty folder called `Swofty.old`, `Swofty.old-2` and `Swofty.fresh-…`, the places your addons and saves were moved from (recorded in `swofty-data-history.json` beside the Swofty folder), the launcher's folder and the `Swofty` and `SwoftyData` folders in it, and on Windows the folders on each drive whose name starts with `Swofty`, in the drive's top folder and in `Games` and `Program Files`. It doesn't search inside other folders. Addons that ship with the launcher don't count.

When `SWOFTY_DATA_DIR` sets the folder, **Use them** refuses and says so.

### The game drive copy

A hard link joins two names for one file and only works inside one drive. Play keeps copies in `swofty-cache/overlay` in the game folder and hard-links the game's files to them. Which files it keeps there depends on where your addons and saves are:

- On another drive than F1 25, every file Play places.
- On F1 25's drive, the loose files that Play would otherwise copy, such as addon files. Built output still links straight from your addons and saves. A game that writes to a linked file changes the cached copy and never the addon's original.

The first Play copies every file; later Plays copy only files that are new or whose size or modified time changed, and delete files that are no longer placed. A cached copy the game wrote to is copied again.

The copy stays when the game goes back to stock. It needs the size of those files on F1 25's drive plus 2 GB to spare; with less room, Play copies each file straight into the game folder and the Play log says why. **Clear** removes the copy at any time and doesn't run while F1 25 is open; the next Play rebuilds it.

### Low space when you press Play

When you press **Play** with less than 3 GB free on F1 25's own drive (the drive with your Windows profile, or on Linux the drive with the game's Proton prefix) or on the drive with your addons and saves, the launcher shows a warning. The game still starts: under 2 GB, F1 25 refuses on its own with "cannot run with less than 2048 free storage space".

## Advanced

| Switch | What it does |
|---|---|
| **Developer mode** | Shows the [Tools](/using/tools) page with the `swofty dev` actions, as the same item in the **Play** menu does |
| **Clear old logs** | Off by default. On: clears old session logs when the launcher starts, as **Clear old** does, without asking. Off keeps every log |

## About

Shows the launcher's version, whether an update is available, and links to the documentation and the Discord server.
