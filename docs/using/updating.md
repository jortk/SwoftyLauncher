---
title: Updating
description: Update Swofty Launcher for F1 25 from the update bar or the command line, and keep Simplified for Swofty, addons and mods up to date.
---

# Updating

The launcher keeps itself and Simplified for Swofty, the addon behind the **Mods** page, up to date. You update other addons and mods by installing the new version over the old one.

## Update the launcher

The launcher checks for a new launcher release and a new Simplified for Swofty release when it opens, then every minute while it's open. When either is available, **Play** becomes **Update**. The button names what will update, and the line next to it shows the versions and a link to the release notes.

1. Close F1 25. While the game runs, you'll keep the **Stop** button; **Update** appears once the game exits.
2. Select **Update**. It installs the launcher first, then Simplified for Swofty, and shows the progress on the button. The launcher downloads from cdn.swofty.net, and from GitHub when cdn.swofty.net is unreachable or has no usable release listing. Each launcher download is checked against the SHA-256 its release lists. The launcher update replaces `bin/`, `loader/` and, on Linux, `share/`, and removes the `addons/` folder that launchers before 1.4.0 carried.
3. The launcher restarts on its own when the update finishes, including when only Simplified for Swofty updated. It restarts once, into the new launcher, when both updated.

A download that the connection breaks off resumes from where it stopped. When it keeps breaking, the error says to turn off antivirus HTTPS or web scanning and any VPN or proxy, or to try another network; see [If a download fails](/using/addon-browser#if-a-download-fails).

If a check fails because you're offline, both cdn.swofty.net and GitHub are down or GitHub's rate limit is reached, you can still play. The launcher checks again a minute later, or when GitHub's rate limit resets.

If one part of an update fails, the other part still installs. If the launcher update fails, you'll see the error next to **Retry update** and the old launcher files stay in place. Select **Retry update**, or **Play without updating** to play the version you have. If only Simplified for Swofty fails, **Play** comes back with the error and **Try again** next to it. An update interrupted partway is rolled back the next time you start the launcher.

Running the [installer](/guide/installation#install-with-the-installer) again also installs the latest release, downloaded from the same places. It finds your install and updates it in place, or [moves it](/guide/installation#update-or-move-an-existing-install) to another folder you choose.

If you built from source or installed through Nix, the launcher can't replace its own files. A bar above **Play** names the new version and links to its release page, and **Play** stays available.

### From a terminal

| Command | Does |
|---|---|
| `swofty update` | Install the latest release into this copy of the launcher |
| `swofty update --check` | Only report whether a new release is available |

Restart the launcher after `swofty update`. In a copy built from source or installed through Nix, `swofty update` prints the release page to download from. `swofty update` doesn't update Simplified for Swofty; the launcher does.

## Simplified for Swofty

The launcher keeps [Simplified for Swofty](/using/addons#built-in-addons) up to date through the same **Update** button. If the addon isn't installed, the launcher installs it in the background. Your mod library and each profile's mod order stay as they are.

The launcher installs the newest Simplified for Swofty release that supports the launcher you'll have after the update. When a launcher update is in the same run, Simplified for Swofty is checked against the new launcher, and one **Update** installs both. A Simplified for Swofty release that needs a launcher you can't update to yet isn't offered; a bar above **Play** names it, and **Play** stays available. When an earlier launcher updated itself and Simplified for Swofty still has an update, the restarted launcher installs it on its own.

### Publishing a Simplified for Swofty release

These rules are for Team Simplified, who publish the addon at [TeamSimplified/simplified-swofty-addon](https://github.com/TeamSimplified/simplified-swofty-addon/releases).

- Launchers from 1.3.1 read the 20 newest releases and pick the newest one whose `compat.launcher` accepts them.
- Drafts and pre-releases are never offered.
- Launchers from 1.0.4 to 1.3.0 only read the release GitHub marks as latest.
- When a launcher from 1.0.9 to 1.3.0 can't load the latest release, its **Update** button can't update the launcher either.
- Mark a release as latest only when its `compat.launcher` accepts every launcher from 1.0.4 to 1.3.0, such as `>=1.0.4`.
- Publish a release that needs a newer launcher without marking it as latest: untick **Set as the latest release**, or run `gh release create --latest=false`.
- Launchers from 1.3.1 still find a release that isn't marked as latest.
- A release can need a launcher that isn't out yet; launchers it doesn't accept skip it until one that does is out.
- Attach the package's `swofty.json` to the release as an asset named `swofty.json`, and the launcher reads the requirement from it instead of downloading the package.
- Without that asset, the launcher downloads the package once to read its requirement and keeps it for the install.
- The `swofty.json` inside the package decides at install time.

## Addon updates

The launcher compares your installed addons with the [Addon Browser](/using/addon-browser) when it opens, then every 30 minutes. When an addon that is on in your active profile has a newer version that works with your launcher, **Play** becomes **Update addons** with the number of updates, and the line next to it lists them. Updates for addons that are off do not change **Play**; they show as an update on the Addons page and in the Addon Browser, and become part of **Update addons** when you turn the addon on. With **Update turned-off addons** on in [Settings](/using/settings#general), **Update addons** also updates the installed addons that are off, in every profile including Vanilla.

**Update addons** covers every installed addon that the Addon Browser has a newer version of, including an addon you installed from a file. A mod or track you imported keeps its own files even when an Addon Browser addon has the same id; its row on the **Addons** page shows **Kept separate**.

To keep the versions you have, select **Skip this version** (**Skip these versions**) on the line next to **Update addons**. **Play** comes back, and the next version of each addon is offered again. A skipped version installs from the addon's Addon Browser page.

1. Close F1 25. **Update addons** appears once the game exits.
2. Select **Update addons**. Each update joins the [Downloads queue](/using/addon-browser#downloads), is checked against its SHA-256, and replaces the installed version. Its switch and settings stay as they were in every profile.
3. Select the download card above the Play bar to open **Downloads** and see each update's progress or error. **Play** comes back when the batch finishes.

An addon that fails to update keeps its installed version. Retry it from **Downloads** or use **Try again** beside the update error.

## Addons and mods

| To update | Do |
|---|---|
| An addon from the Addon Browser | Select **Update addons**, as in [Addon updates](#addon-updates). |
| An addon you installed from a file | Select **Update addons** when the Addon Browser has a newer version of it, or [install](/using/addons#install-an-addon) the new version. Either replaces the addon with the same id and keeps its switch and settings in every profile. |
| A mod or track | [Import](/using/mods#import-a-mod-or-track) the new version. It keeps its place and switch in every profile. |

## After a game update

When F1 25 updates to a version Swofty Launcher doesn't have yet, the line next to **Play** reads "This game version (1.0.145.2472) isn't supported by Swofty Launcher yet. Add-ons are off until a launcher update, so the game runs without them."

The launcher identifies the game by the size and build time of `F1_25.exe`. For an exe it doesn't know, **Play** still works and starts F1 25 as the Vanilla profile does: no add-ons are loaded or applied, Launcher saves aren't used, and your Steam saves aren't touched. This is the same for every profile. The play bar's `Saves:` line reads `Steam's own save` with `this game version is not supported yet` under it; your launcher careers are not used on that run and are untouched. See [Which save Play will use](/using/career-saves#which-save-play-will-use).

The line goes away by itself when you update the launcher to a release that supports the version. Don't verify or repair the game files: the update is correct.

Developers porting the launcher to a new game build set `SWOFTY_ALLOW_UNSUPPORTED_BUILD=1` in the environment of `swofty run` to load add-ons into it anyway. The setting isn't in the launcher's settings. `swofty test shell` sets it for the sessions it starts.

## Next

- [Release notes](/releases/1.0.7): what changed in the latest release.
- [Restoring the game](/using/profiles-and-restoring): after a game update.
