---
title: Track packages
description: A community track package in the EGO GameDatabases Editor format, its manifest.json annotated, how the Simplified Importer turns it into a track addon, and how to check the build.
---

# Track packages

A **track package** is a community track in the zip format made for the EGO GameDatabases Editor. The Simplified Importer turns it into a track addon.

## A track zip in full

```text
testring.zip
├── manifest.json            the track code, name and activation, at the root of the zip
├── testring.erp             the track archive
├── mipmaps/                 optional textures
└── ui/
    ├── images.json          track artwork
    └── minimap.json         minimap placement
```

```jsonc [manifest.json]
{
  "track_code": "testring",            // names the track everywhere: letters, digits, _ and -
  "track_name": "Test Ring",           // optional display fields
  "version": "0.1.0",
  "author_name": "tester",
  "erp": "testring.erp",               // the track archive, relative to the package
  "activation": {
    "copy_from_track": "melbourne",    // the base track whose TrackData rows are cloned in every dataset
    "copy_slot": "melbourne",          // the track whose Track row and resource row are cloned; defaults to copy_from_track
    "overrides": [                     // set path to value on the cloned TrackData rows; datasets limits which
      { "datasets": ["f1_2025"], "path": "TyreData_0.compound_harder", "value": "C3" }
    ],
    "track_overrides": [               // set path to value on the cloned Track row
      { "path": "allow_night_start", "value": "False" }
    ],
    "sprint": ["f1_2026"]              // the data sets in which the track's weekend is a sprint weekend; none by default
  },
  "ers": { "HotLapDeployFactor": 0.7 }, // per-track ERS values written into every 2026 car handling file
  "copy_directories": [                // source folders in the package and their destination in the game
    { "source": "mipmaps/", "destination": "asset_groups/environment_package/tracks/testring/mipmaps/" }
  ],
  "ui": { "schema_version": 1, "images_file": "ui/images.json", "minimap_file": "ui/minimap.json" }
}
```

Import it and build:

```sh
swofty mods import testring.zip
swofty build
```

```text
Test Ring is now the addon Test Ring
testring: track data: testring cloned from melbourne in 19 dataset(s): f1, f2, classic, ...
```

The importer turns the zip into the addon `tester.testring` ([The addon's id](#the-addon-s-id)), installs it and turns it on in the active profile. You can also drop the zip on the launcher ([Tracks](/using/mods#tracks)). The addon holds the unpacked package and one op:

```text
$DATA/addons/tester.testring/
├── swofty.json          category Tracks, kind database and erp
├── track/               the unpacked package: manifest.json, testring.erp, mipmaps/, ui/
└── ops/track.json       { "op": "track.add", "source": "track" }
```

`swofty build` activates every `track.add` before the other operations run: activation clones a base track's database rows, installs your track's archive and adds its menu entries ([How tracks are installed](/reference/installing-tracks)).

## How the importer finds a track

- A zip is a track package when it holds `manifest.json` at its root, or holds exactly one zip and no `swofty.json`, `mod.serps` or `F1 25/` folder. A download zip that wraps one package is unwrapped.
- The `manifest.json` needs a `track_code` of letters, digits, `_` and `-`. Without one, the zip imports as an ordinary mod ([`.serps` packages](./serps)).
- The track code becomes the mod's id in the importer's library. When another mod in the library already has that id, the id is `<code>-track`, then `<code>-track-2`. Importing the same track code again reuses its id.
- A track package converts without the game's files.

## The addon's id

The importer asks for the id `mod.<mod id>`. The launcher installs the addon under the first of these ids that no addon other than an import has:

1. `<author>.<mod id>`, when `author_name` is set: `tester` and `testring` give `tester.testring`. The author is lowercased, accents are dropped (`José` gives `jose`), other letters with no plain spelling are left out, and each run of other characters becomes one `-` (`RM Designs` gives `rm-designs`). It is at most 32 characters. An author that gives nothing, or gives `swofty`, `mod`, `local`, `core`, `std`, `imported-mod` or `dbedit`, is skipped.
2. `imported-mod.<mod id>`: `testring` without an `author_name` gives `imported-mod.testring`.

The mod id keeps its library suffix: when another mod in the library has the id `testring`, the track by `tester` gives `tester.testring-track`.

- An import installed before these rules keeps its id, such as `mod.testring`, and importing it again replaces it in place.
- Importing a track again replaces the addon the last import made, under the same id.
- A zip with a `swofty.json` installs as an addon under that file's `id` ([Install an addon](/using/addons#install-an-addon)), not through the importer.
- The Addon Browser never updates an import, even when an Addon Browser addon has the same id. It doesn't install over one either: remove the import on the **Addons** page first. The import's row on the **Addons** page shows **Kept separate**, and the Addon Browser's button reads **Your import v1.0.0 has this id**.
- An import and an Addon Browser addon that add the same track code can both be installed. A build with both on stops: `track testring is added by both imported-mod.testring and mod.testring; turn one of them off on the Addons page`.

## The manifest

- `activation` needs `copy_from_track`, `copy_slot` or both, each naming a stock track. Without `copy_from_track`, `copy_slot` is the base track.
- An override `path` is relative to the row. A segment of it can name a sub-row by number (`TyreData_0`), every sub-row (`TyreData_*`) or the one sub-row whose field matches (`Name[field=value]`, as in [Selecting rows](/reference/ops/database#selecting-rows)). A selector that matches more than one sub-row fails the build.
- An added track runs normal weekends: its `weekend_format` is `default` in every data set, also when its base is a sprint weekend (Zandvoort in `f1_2026`). `sprint` lists the data sets in which it is a sprint weekend instead. The 2023 to 2025 seasons read the `f1` data set and 2026 reads `f1_2026`; a data set where the base track has no weekend format takes its parent's (`f1_2026` from `f1`), and the build warns when `sprint` names one:

```text
mytrack: sprint f1_2025: mytrack has no weekend format in that data set, so it takes its parent data set's
```

- An override `value` is a string, such as `"1.55"` or `"64,64,64"`. A JSON number is also accepted and written as its text: `1.55` sets `"1.55"`.
- Keep every path in the package inside the package.

A build leaves out a package with neither `copy_from_track` nor `copy_slot`, or with a `manifest.json` that doesn't read, and builds the rest of the profile. It warns:

```text
Portimao (imported-mod.portimao): track Portimao is left out; its manifest.json doesn't say which track it replaces or copies. Import a fixed version, or ask its author for one
```

A race weekend set to a track the build leaves out races at its stock track again, with a warning. Importing or installing such a package shows the same warning, and the **Addons** page marks its addon **Track left out**.

A build also checks every record that names a track by its code: race weekends, track settings, the content sets' track lists and the game modes' left-out tracks. A record an addon sets to a track the build doesn't hold, such as a DB Editor change made while a track addon was on, gets its stock track back, and a list drops the track. The build warns, naming the addon that set it and the track addon to turn on:

```text
My Calendar sets race weekend bahrain to istanbul_park, which is off: turn Istanbul Park on; until then this build puts back bahrain
```

A record the game doesn't ship, such as a race weekend an addon adds, keeps its track, and the warning says to change it in DB Editor.

For every table, archive and menu change made during activation, see [How tracks are installed](/reference/installing-tracks).

## The track's name

Menus read `TrackNameData_0.title` and the other `TrackNameData_0` fields on the cloned `TrackData` rows, so they show the base track's names until you override them. A track package carries no text, so point each field at a stock language key with an `activation.overrides` entry:

```json
{ "path": "TrackNameData_0.title", "value": "lng_track_jpn_gp_title_short" }
```

To give the track a name of its own, with new keys and strings, ship it in a data addon with `track.add`, `lang.set` and `db.set` ([A track with its own name](/reference/data/track-names)).

## Using the package elsewhere

The package is the EGO GameDatabases Editor format, so the same zip also installs with the editor. `copy_slot` needs editor 1.6.1 or later. Neither way replaces a stock track: `copy_slot` adds a new track id, and a package with the track code `bahrain24` fills the game's empty `bahrain24` slot.

The editor adds no text, so a new language key such as `lng_my_track_title` shows no name there. For a package that shows a name in both, set `TrackNameData_0.title` to a stock key, such as the base track's own.

## Racing line and other binary XML

The track archive's AI racing line, AI brake settings and splines hold binary XML with string stats beside it ([Binary XML with stats](/reference/ops/erp#binary-xml-with-stats)). When those stats don't match the data, the build writes new stats into the track archive it installs and prints:

```text
testring: fixed the size headers of 1 AI racing line(s) in Test Ring so F1 25 reads them safely
```

When the data starts as binary XML but doesn't parse, the build stops, and Play doesn't start:

```text
Test Ring would crash F1 25: its testring_ai_track.aispline doesn't match the size header the game reads with it, and the launcher can't read the file to fix the header. Turn Test Ring off in this profile, and set any race weekend that uses it back to another track.
```

Export the archive from a tool that writes each resource's stats from its own data, so the package works with other tools too.

F1 25 crashes when an AI car drives a route of the racing line, often the pit lane, on which the passing space never changes from one gate to the next. The build moves one white line on such a route and prints:

```text
testring: fixed the AI pit lane of Test Ring so F1 25 doesn't crash when an AI car drives it
```

A pit lane whose white lines vary along its length, as at the stock tracks, needs no change.

## Check the build

```sh
python3 tools/map_check.py <overlay folder> --game <game folder> --maps "$DATA/addons"
```

Use the folder `swofty build` printed under `$DATA/cache/overlay/<profile>/<buildhash>/` for `<overlay folder>`. `map_check.py` reads that folder's `plan.json` and the track packages under `--maps`: each folder's `track/manifest.json` or `manifest.json`. `--maps` defaults to the Team Simplified library, which holds no track once it has converted, so point it at `$DATA/addons`, where each track addon keeps its package in `track/`. It checks that:

- Every key the game reads on a base row also exists on your track's new row.
- Each rewritten archive and database re-serialises correctly.
- Every ERS setting is present.

See [Checking a build](/reference/installing-tracks#checking-a-build).
