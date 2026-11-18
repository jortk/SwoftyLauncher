---
title: Saves
description: The Saves page under Profiles shows the active profile's save, its careers and versions, copies saves between profiles and to and from Steam, and restores a version.
---

# Saves

**Saves** is a page under **Profiles** in the sidebar. It shows the save of the active profile; switch profiles with the picker at the bottom left to see another profile's save. Vanilla plays on your Steam save.

```text
progressive saves                         [Copy careers to…] [Import from Steam] [Export to Steam]
This profile's own save

CAREERS
Driver Career   Oscar PIASTRI, McLaren                                     3 hours ago
                Season 2, round 4
My Team         Racing Bulls                                    Deleted in game

VERSIONS
After Play            3 hours ago      0.9 MB                          [Pin] [Restore]
Before Play           3 hours ago      0.9 MB                          [Pin] [Restore]
Imported from Steam   2 days ago       0.9 MB                          [Pin] [Restore]

Versions use 2.7 MB of 500 MB · Change · Open folder
```

- **Copy careers to…** copies this profile's whole save over another profile's save. A confirm names both and the target's current save is kept as a version.
- **Import from Steam** copies your Steam save over this profile's save, with the same confirm and kept version.
- **Export to Steam** copies this profile's save over your Steam save. It needs Steam and F1 25 closed.
- A disabled action says why on hover.
- A save that was imported or copied in, with no careers listed yet, shows one row: `Imported Steam save · careers listed after you play this profile once`.
- A career has a chip only when it is **Missing** or **Deleted in game**.

## Versions

Each version is one line: why it was kept (Before Play, After Play, Imported from Steam, Copied from another profile, Launcher start, Changed outside the launcher, Before restore, Restored, Old backup), how long ago (hover for the date), its size, **Pin** and **Restore**.

A restore keeps the save it replaced as **Before restore**, and the careers list shows the restored version's careers until your next Play.

**Pin** keeps a version; retention never removes it. **Unpin** lets retention have it again.

**Restore** opens a panel before anything changes:

```text
You get back   Progressive 2016, Verstappen, Red Bull (season 2016, round 7)
You lose       Classic 2020 (not in that version)
Also           Settings and setups go back to 2026-10-30 22:15 UTC. The save as it is now is kept as a version.
```

Restore waits until F1 25 is closed, and for the Vanilla save until Steam is closed too, as in [Restoring a profile save](/using/profiles-and-restoring#restoring-a-profile-save).

## Using your Steam saves instead

The arrow beside **Play** opens a menu. **Use launcher saves** (ticked by default, absent for Vanilla) turns the profile's own save on or off; with it off the profile plays on your Steam save like Vanilla does. While it is on, **Play once with your Steam saves** runs one session on the Steam save and leaves the setting on:

```text
$ swofty run --steam-saves-once
```

If the core can't use launcher saves it closes F1 25 before the title screen, and the play bar says `Launcher saves could not be used, so F1 25 closed before loading a save`, with the same **Play once with your Steam saves**, **Open logs** and **Dismiss**. Nothing was read or written. The launcher has a save backend for the EA app copy of F1 25. EA app copies of 1.0.145 still run without add-ons.

## Which save Play will use

A line on the play bar names the save the next **Play** uses. With launcher saves in use it reads `Saves: grid launcher save`, with the profile's name. When Play uses the game copy's own save instead, the line turns amber and reads `Saves: Steam's own save (your launcher careers are not used on this run)`, with the reason under it. On the EA app copy it says `the EA app's own save`.

| Reason line | Case |
|---|---|
| `Vanilla profile` | The active profile is Vanilla. |
| `launcher saves are off` | **Use launcher saves** is unticked for the profile. |
| `this game version is not supported yet` | The game's `F1_25.exe` is a build the launcher doesn't know. This holds for every profile. |
| `you chose to play once with your Steam saves` | **Play once with your Steam saves** is starting. |
| `launcher saves are not available on the EA app copy` | The active copy is from the EA app. |

The line is read-only, and `swofty run` makes the same decision. Your launcher careers stay in `saves/<profile>/remote` and are not changed by a run on the Steam save.

## The first time

The first launcher start of 1.7.0 copies your Steam save into every profile's save and opens one dialog:

```text
Your careers have their own saves now
Each profile now has its own save, starting from a copy of your Steam save. Playing from Steam still uses your Steam save, and Vanilla plays on it too.
[Show me]  [OK]
```

**Show me** opens Saves. The dialog is recorded as seen in `saves/imported.json` (`"seen": true`) and never shows again.

## When careers go missing

A banner at the top of the page, and on the **Addons** page, names the missing careers, the profile whose save they went missing from, and the time they were last in the save. **Review the restore** opens that profile's versions with the panel for that version. **Dismiss** hides the banner until the next career goes missing.

When the launcher has no career index to compare, a save that changed outside a launcher session and got smaller shows the older notice, worded as a change in the save, never as lost careers.

## From a terminal

```text
$ swofty saves careers
p:1  Progressive 2016, progressive, Verstappen, Red Bull, season 2016, round 7, needs swofty.progressive, missing-since:20261031-201502-after-session
    last in the save at snapshot 20261030-221500-after-session

$ swofty saves versions
2 version(s) in /home/you/.local/share/swofty/career-vault/steam-843502855, 1.9 MB of 500.0 MB
Steam Cloud now: synced
20261031-201502-after-session  after Play, profile default, 0.9 MB, Steam Cloud: synced, 1 career(s): Classic 2020
20261030-221500-after-session  after Play, profile progressive, 0.9 MB, Steam Cloud: synced, 2 career(s): Progressive 2016, Classic 2020, pinned (holds-missing-career)
    restoring it gets back: Progressive 2016 (season 2016, round 7)

$ swofty saves pin 20261030-221500-after-session
pinned snapshot 20261030-221500-after-session
$ swofty saves unpin 20261030-221500-after-session
unpinned snapshot 20261030-221500-after-session; it stays kept for holds-missing-career
```

Before the core has written a career index, the commands say so and list the versions without careers:

```text
$ swofty saves careers
no careers yet: the vault learns them from the career index F1 25 writes in a launcher session

$ swofty saves versions
1 version(s) in /home/you/.local/share/swofty/career-vault/steam-843502855, 0.4 MB of 500.0 MB
Steam Cloud now: unknown
20261031-153032-manual  backup, profile default, 0.4 MB, Steam Cloud: unknown, careers unknown, pinned (backup)

$ swofty careers
no careers in the index yet: the core lists each career it creates or loads
```

`swofty saves backup`, `restore`, `import` and `export` act on the active profile; pass `--profile <name>` to `swofty` or to `import` and `export` for another. `--source launcher-<profile>` points `list`, `show`, `versions`, `careers`, `pin`, `unpin` and `restore` at a profile's launcher saves instead of the Steam save:

```text
$ swofty saves list --source launcher-default
vault /home/you/.local/share/swofty/career-vault/launcher-default: 1 snapshot(s), 0.4 MB of 500.0 MB
save /home/you/.local/share/swofty/saves/default/remote: 2 file(s), 400416 bytes, Steam Cloud: local only (no Steam Cloud)
20261031-153037-imported  Imported from Steam saves, profile default, 2 file(s), 400416 bytes
```

See [`swofty saves`](/reference/cli#swofty-saves) for every flag and [Saves and the career vault](/reference/saves) for the folders.
