---
title: Addons
description: Install addons, find them with search and filters, turn them on per profile, set their load order, check their conflicts and remove them on the Addons page.
---

# Addons

Everything the launcher loads is an addon: a folder with a `swofty.json`. A data addon changes the game's database, text, archives and files; a code addon adds an engine plugin, a launcher page or both. Mods you download for the game folder become addons through the [Simplified Importer](/using/mods), and changes you make in the [DB Editor](/using/db-editor) save as addons.

## Install an addon

The two cards at the top of the **Addons** page are the ways in:

- **Installing a Swofty Addon?** Drop a `.swoftyaddon`, a zip of an addon or an addon folder on the card, or click it to choose a file. **Choose a folder…** picks an addon folder. You can also drop an addon anywhere on the launcher window.
- **Installing a mod?** **Go to Simplified Importer** opens the [Simplified Importer](/using/mods), which turns the mod into an addon.

A file that isn't an addon, and that no addon such as the Simplified Importer imports, opens a dialog that names it. The dialog lists up to five files. **Go to Simplified Importer** opens the [Simplified Importer](/using/mods), and **Read the addon docs** opens the [addon reference](/reference/addons/) in your browser.

The launcher copies the addon to `$DATA/addons/<id>/`, opens **Addons** and highlights it. Installing an addon with an id that's already installed replaces it. A new addon joins only the active profile, turned off, at the end of the list. Installed from `vanilla`, it joins no profile.

## Find an addon

Type in the search box on the left to search names, ids, authors and descriptions. Under it, the filters narrow the list:

| Filter | Values |
|---|---|
| Category | What the addon is for: Seasons, Tracks, Cars, Helmets, Shaders, Career, AI, Performance, Utility, or Uncategorised. Built-in addons show no category. Shaders are [colour looks](/reference/addons/shader-packs) for the race. |
| Type | What it changes: Database, Archives, Text & UI, Simplified, Files, Code. Simplified is a mod merged into the game's shader packages, as the Simplified Suite loads `.serps` mods. |
| Source | Built in, DB Editor, Imported, Installed. Imported addons come from the [Simplified Importer](/using/mods). |
| State | On, Off, Has conflicts, Update available |

Under its name, each row shows the addon's category, its types and a tag with its source. An addon that turns on an experimental feature of the game's core, such as the [overlay](/reference/std/overlay) or a [shader pack](/reference/addons/shader-packs)'s look, also has a **Uses experimental features** tag, and its panel lists the features. Tags also say what the addon does in game: **Draws over the game** (the overlay) and **Changes the picture** (a shader pack's look). Its panel lists them under **In game**. Each value shows how many addons match. **Clear** removes every filter. An open panel closes when its addon no longer matches.

On a window less than 760 pixels tall, the install cards shrink to one row.

## Turn addons on or off

Use the switch on a row to turn an addon on or off in the active [profile](/using/profiles). An addon keeps its place in the list when it is off; its row is dimmed and has no load order number. The next time you press **Play**, the launcher builds the profile with the change.

Some addons, such as Driver Manager, change what your careers store. Turning one of those off asks first: careers you played with it may not load without it. Select **Turn off anyway** to go ahead, or **Cancel** to leave it on.

**Turn off all** turns off every addon except those that show **Always on** and those that would ask about your saves; the launcher names those. **Undo** on the message that follows turns the same addons back on in their old order.

In the `vanilla` profile the list is read-only and shows every installed addon. Switch to a modded profile to turn addons on.

## Turn on, turn off, move or uninstall several addons

Hold **Ctrl** (**Cmd** on macOS) to show a tick box on every row. Ctrl-click a row, or click its tick box, to tick or untick it. A plain click on a row opens its panel. While at least one addon is ticked, a bar above the list shows how many and has **Turn on**, **Turn off**, **Uninstall** and **Clear**. **Escape** also clears the ticks. The ticks clear after an action from the bar.

- **Turn on** and **Turn off** change the active [profile](/using/profiles) the way a row's switch does. Always-on addons and shader-only addons are skipped, and a message says how many. **Turn off** leaves on the addons whose turning off warns about your saves, and a message names them.
- **Uninstall** asks once for all the ticked addons, then removes them as **Uninstall** in a panel does. Built-in addons are skipped, and a message says how many.

Drag a ticked row to move every ticked addon with it, also those a search, a filter or a closed folder hides. They keep their order and land together where you drop them, in a folder or outside folders, the way one dragged row does. The dragged card shows how many addons move. Ticked addons in Built In stay where they are, and a drop on a ticked row moves nothing. The ticks stay after the move, and **Escape** during the drag cancels it and keeps them. Dragging a row that isn't ticked moves only that row and leaves the ticks as they are.

The list keeps its scroll position after an uninstall. Ticks are not available in the `vanilla` profile.

## Add an addon to a profile

Each modded profile lists only its own addons and the built-in ones. To use an addon you installed from another profile, select **Add addons** above the list, tick the ones you want and confirm, or select **Add to** with the profile's name on its card in the Addon Browser. It joins the profile turned off, at the end of the list.

**Remove from** with the profile's name, in the addon's panel, takes it out of this profile only. It stays installed for your other profiles, and its settings come back if you add it again.

## Folders

The list groups addons into folders:

- **Built In** holds SwoftyMod and Simplified for Swofty. It is always first and its addons can't be moved.
- Each category with an addon gets a folder of the same name, such as **Seasons** or **AI**. A new addon goes into its category's folder.
- Uncategorised addons sit outside the folders.
- **New folder**, above the list, makes a folder of your own. A new folder starts at the end of the list. The pencil on its row renames it, and the bin removes it; its addons go back to their category's folder or out of folders.

Drag an addon onto a folder to put it at the end of that folder, or between two of its rows to put it there. Drag it between two rows outside a folder, or onto **Drop here to move to the end**, to take it out. An addon you move into another folder stays there; one you take out stays out of folders.

Select a folder to close or open it. Folders stay closed between launches, and a search or filter opens them all.

Folders are shared by every profile. The launcher saves them in `$DATA/folders.json`.

## Load order

The list is the load order, top to bottom, and the number on a row is its place among the addons that are on. When two addons change the same thing, the one lower in the list wins. A folder's addons load together. Dragging a folder by its handle moves all of them and keeps their order. Drag a row, or use the arrows in its panel, to move an addon. The arrows move it within its folder. To move several addons at once, tick them and drag one of them; see [Turn on, turn off, move or uninstall several addons](#turn-on-turn-off-move-or-uninstall-several-addons). **Escape** during a drag cancels it. Rows that are always on have no drag handle. SwoftyMod and Simplified for Swofty always load first: nothing can be dragged above them.

When the launcher opens a profile whose addons aren't grouped by folder, it moves each folder's addons up to the first of them and keeps their order. That can change which addon wins between two in different folders.

After a conflict check, a row that is on shows a quiet **Overlaps** badge with the number of addons that also change some of what it changes. The lower one in the list wins, whichever of them an addon's own rows say. An overlap is not a problem, so nothing asks you to fix it. The badges go away when an addon is turned on, turned off or moved, until the next check.

An addon loads after every addon it requires, whatever their places in the list. A row that has a problem shows a warning triangle by its name. Hover it for what is wrong, or select the row to open its panel, which lists the problem with a button where one helps:

```
Classic Career         Needs F1 2019, which is off                 [Turn F1 2019 on]
Classic Career         Needs F1 2020, which isn't installed        [Install F1 2020]
More Driver Models     Changes rows that Classic Seasons adds, so it needs Classic Seasons above it
F1 2012 Season         F1 2012 has more teams than this track has garages (with Pedro's Mugello)
```

- When addons load below something they need, a bar at the top of the page says how many, with one large **Fix load order** button. It reorders the profile with the smallest change that puts every addon below what it needs, and keeps the rest of your order. If a folder would pull an addon back above what it needs, the addon leaves that folder and a message names it. The page checks the new order afterwards and checks conflicts again, so you see the result.
- If addons need each other above themselves, the bar is red, names them and has a **Turn off X** button.
- **Needs X, which is off** and **Needs X, which isn't installed** stop Play. **Turn X on** and **Install X** fix them. A version outside the required range says which version is installed and opens the Addon Browser.
- **Can't be on with X** is a declared conflict. **Turn X off** fixes it.
- **Changes rows that X adds** comes from the profile's last build: the addon's database changes waited for rows X adds. **Fix load order** puts it below X.
- Addons in Built In, such as Simplified for Swofty, don't move and never show an order problem. **Fix load order** leaves them in place, and the addons below them win where they overlap. The build still loads them after every addon they require or load after.
- **Your career K needs X, which is off** shows on X's row when a saved career lists X among the addons it needs. It also shows when X is not installed or outside the range the career needs.
- A clash is a known bad pairing, such as F1 2012 Season with Mugello. The list is in `packages/core/src/order_clashes.json`.
- Problems that **Fix load order** can't help, such as a missing addon, show as a count by the page title: **2 addons need attention**.

The Play log lists a required addon listed below its dependent as a warning. The build loads it first anyway.

The panel of an addon lists the same problems.

A row whose addon asks to remove records the game's database ships with shows a badge such as **12 removals skipped**. The build leaves those records in place, and the panel's **Conflicts** section says so. See [Removing stock records](/reference/ops/database#removing-stock-records).

## Check an addon

Select a row to open its panel: description, author, version, source, whether it works with your game version, what it changes (database values, game assets, text, files, tracks), its settings and its conflicts. **Game version** says **Works with this game version**, or names the game build the addon was made for next to yours. **Advanced** shows the addon's id.

**Conflicts** lists the addons this one overrides and the ones that override it, with what they both change. The list comes from the profile's last build. **Check conflicts**, at the top of the page and in the panel, builds the profile again without starting the game and refreshes the list. The list says it is out of date after an addon is turned on, turned off or moved, or after the addon itself changes. Play waits while a check runs.

Select a conflict to see what it covers, grouped by archive, database table, text or folder. **Details** lists the items, ten at first; **Show all** lists the rest.

An addon from the DB Editor has **Edit in DB Editor**.

## Change its settings

Change a value under **Settings** in the addon's panel. **Reset to defaults** puts every setting back. Settings save to the active profile; you can change them in game too, see [In-game addon settings](/using/swofty-mod).

## Remove an addon

Select **Uninstall** in the addon's panel and confirm. Uninstalling an addon deletes its files and takes it out of every profile. Built-in addons can't be uninstalled.

Removing a DB Editor addon deletes its changes. To keep them, select **Export** › **Share addon (.zip)** in the [DB Editor](/using/db-editor) first.

## Addons that can't be used

An addon made for a newer Swofty Launcher, or one whose `swofty.json` is damaged, is listed under **Can't be used** with the reason, such as "It needs a newer Swofty Launcher (unknown field `hud`)". The launcher leaves it out of builds, and **Uninstall** deletes it. The rest of your addons work as before. Installing such an addon is refused with the same reason.

A profile saved by a newer launcher keeps the settings this launcher doesn't know, and saves them back unchanged.

## Built-in addons

| Addon | What it does |
|---|---|
| **SwoftyMod** | Loads addons in game and adds **ADDON SETTINGS**. It's always on in modded profiles |
| **Simplified for Swofty** | Team Simplified's addon: the [Simplified Importer](/using/mods). The launcher installs it, [keeps it up to date](/using/updating#simplified-for-swofty) and keeps it on |

Every other addon, including [Swofty's own](/using/swofty-addons), comes from the [Addon Browser](/using/addon-browser).

## Careers that need an addon

An addon can own a career mode, such as Progressive Career. A session without that addon can drop its careers from F1 25's save. When the launcher finds careers missing from the save, a banner on the **Addons** page names them and opens the [Saves](/using/career-saves) page, where **Restore** puts back the version that still held them.

## From a terminal

| Command | Does |
|---|---|
| `swofty addons list` | List installed addons and whether each is on in the profile |
| `swofty addons install <path>` | Install a folder, `.swoftyaddon` or zip |
| `swofty addons enable <id>` | Turn an addon on |
| `swofty addons disable <id>` | Turn an addon off |
| `swofty addons remove <id>` | Remove an addon and take it out of every profile |
| `swofty conflicts [--check]` | Show the profile's conflicts from its last build, or build it again first |
| `swofty settings list [id]` | List addon settings with the profile's values |
| `swofty settings set <id> <setting> <value>` | Change a setting |
| `swofty settings reset <id>` | Put an addon's settings back to their defaults |

Add `--profile <name>` to work on a profile other than the active one. For an addon that has affected your saves, `disable` and `remove` print the warning and ask before going ahead; `--yes` skips the question and is required outside a terminal.

## Next

- [Simplified Importer](/using/mods): turn a mod into an addon.
- [DB Editor](/using/db-editor): change the game database and save it as an addon.
- [Saves](/using/career-saves): the careers in your save, and the versions the launcher keeps.
- [Profiles](/using/profiles): keep different sets of addons apart.
- To make an addon, see [Addons](/reference/addons/) in the developer docs.
