---
title: Adding a livery
description: A complete data addon that adds a special livery as a new choice in the picker — the car's model, the livery row, the unlock row and its ownership key — with the RM Designs Scuderia AlphaTauri ORLEN livery.
---

# Adding a livery

This worked example builds a whole [data addon](./). To add a special livery as an extra choice in one car's picker, you need three changes: copy the car's model into a `wep` archive, add a row to the special-livery table, and add an unlock row that the launcher reports as owned.

Here you'll build RM Designs' Scuderia AlphaTauri ORLEN livery for the 2026 Racing Bulls car. You can still select the stock Racing Bulls liveries.

```
rm-alphatauri-livery/
├── swofty.json
├── db/
│   └── se_livery.json
└── erp/
    ├── livery.json
    └── rm_object_opaque.nefx2.sm51.erp
```

Use RM Designs' archive from their download as `erp/rm_object_opaque.nefx2.sm51.erp`, without modifying it.

## 1. The manifest

```json [swofty.json]
{
  "schema_version": 1,
  "id": "rmdesigns.alphatauri-orlen-livery",
  "name": "Scuderia AlphaTauri ORLEN livery",
  "version": "1.0.0",
  "author": "RM Designs",
  "kind": ["database", "erp"],
  "options": [
    {
      "id": "picker_name",
      "type": "choice",
      "label": "Name in the livery picker",
      "binds": "db",
      "default": "lng_team_tor_hashtag",
      "values": [
        { "value": "lng_team_tor_hashtag", "label": "AlphaTauri" },
        { "value": "Scuderia AlphaTauri ORLEN (RM Designs)", "label": "Literal text, no lookup" }
      ]
    }
  ],
  "compat": { "game": "f1_25", "game_builds": [">=25251167"], "seasons": ["f1_2026"] },
  "operations": {
    "database": ["db/se_livery.json"],
    "erp": ["erp/livery.json"]
  }
}
```

The `picker_name` option gives you a `db` choice to fill <span v-pre>`{{picker_name}}`</span> in the livery row. Using the stock text tag `lng_team_tor_hashtag` makes the name show in every language.

## 2. The livery row and the unlock row

Give your special livery a row in `customisation.custom_se_liveries.se_liveries`. Its `wep_override` tells the car which archive to load: `_barcelona_26` on `toro_rosso_2026` loads `teams/toro_rosso/wep/toro_rosso_barcelona_26.erp`. That stock archive holds no car yet. You'll also need an unlock row to point the inventory at your new livery.

```json [db/se_livery.json]
{
  "schema_version": 1,
  "ops": [
    {
      "op": "add-row",
      "table": "customisation.custom_se_liveries.se_liveries",
      "from": "customisation.custom_se_liveries.se_liveries.se_livery[_metadata_=toro_rosso_miami_2026]",
      "set": {
        "_metadata_": "toro_rosso_rm_alphatauri_2026",
        "id": "22",
        "wep_override": "_barcelona_26",
        "name_loc_tag": "{{picker_name}}",
        "desc_loc_tag": "{{picker_name}}",
        "ui_priority": "22"
      }
    },
    {
      "op": "add-row",
      "table": "unlockable_content.unlockable_items.unlockable_item_list_2.folder_23",
      "from": "unlockable_content.unlockable_items.unlockable_item_list_2.folder_23.unlockable_se_livery[_metadata_=toro_rosso_miami_26]",
      "set": {
        "_metadata_": "toro_rosso_rm_alphatauri_2026",
        "customisation_item": "toro_rosso_rm_alphatauri_2026"
      }
    }
  ]
}
```

The first op copies the stock Miami 2026 livery row, giving your livery a new label, id and archive. The second copies Miami's unlock row and points it at your livery. Leave `statId` out: an unlock row's `statId` must be unique in its unlock list, and the launcher gives the copy a free one and keeps it from build to build ([Unlock items](/reference/ops/database#unlock-items)). Don't pick one by hand. The highest stock id plus one is the next game update's: Steam build 26566308 gave 5280 to its own `toro_rosso_barcelona_26` row, and a `statId` that a stock row holds now moves to a free id with a warning.

`add-row` copies every key of the source row, so the copies need no `fields` list. See [Add a row](./database#_3-add-a-row).

## 3. The car

Now fill the empty `toro_rosso_barcelona_26.erp` `wep` archive. Use `copy-resource` to copy each of the car's resources from RM Designs' archive:

```json [erp/livery.json]
{
  "schema_version": 1,
  "ops": [
    {
      "op": "warn",
      "message": "Scuderia AlphaTauri ORLEN by RM Designs: Racing Bulls 2026 special livery in toro_rosso_barcelona_26"
    },
    {
      "op": "copy-resource",
      "source": "erp/rm_object_opaque.nefx2.sm51.erp",
      "erp": "2025_asset_groups/f1_2026_vehicle_package/teams/toro_rosso/wep/toro_rosso_barcelona_26.erp",
      "resource": "eaid://f1_2026_vehicle_package/teams/rbswapc/idf/rbswapc.idf?lod=all&ib=0"
    }
  ]
}
```

You'll need one `copy-resource` per resource in the full file: the model's index and vertex buffers, its materials and textures, the two `cf_pattern_6` textures, and RM Designs' world and layers. To find what to copy, list the archive's resources with `tools/erp.py` ([Look inside an archive](./textures#look-inside-an-archive)).

## 4. Ownership

Your livery only appears in the picker when the player's inventory marks its unlock as owned. The ownership key comes from the djb2 hash of `eSELivery` followed by the unlock row's `_metadata_` label. For `toro_rosso_rm_alphatauri_2026`, that gives you `0xf01d03c7`.

The launcher handles ownership when you build the profile. It compares the built database with the stock one and writes every `unlockable_se_livery` row an addon adds to a folder under `unlockable_content.unlockable_items.unlockable_item_list_<n>` to `owned_items` in `swofty/settings.json`:

```json
"owned_items": [
  { "key": "0xf01d03c7", "addon": "rmdesigns.alphatauri-orlen-livery", "what": "se_livery 22" }
]
```

SwoftyMod reports these keys as owned in game. If a stock row already gives the same key, it's left out and you get a warning. If two rows give the same key, it's reported once.

## 5. Build and check

Install your addon, enable it, then build:

```sh
swofty addons install ./rm-alphatauri-livery
swofty addons enable rmdesigns.alphatauri-orlen-livery
swofty build
```

Check the build output for the `warn` message, the unlock row's `statId` and one line per owned item:

```text
rmdesigns.alphatauri-orlen-livery: add-row unlockable_content.unlockable_items.unlockable_item_list_2.folder_23.unlockable_se_livery_22 (from unlockable_content.unlockable_items.unlockable_item_list_2.folder_23.unlockable_se_livery[_metadata_=toro_rosso_miami_26]): statId 10000 (the source's is 5274)
items: rmdesigns.alphatauri-orlen-livery unlock row toro_rosso_rm_alphatauri_2026 (se_livery 22) is reported as owned, key 0xf01d03c7
```

In game, you'll find the livery in the Racing Bulls car's picker. If it's missing while the stock Miami and Suzuka 2026 liveries show, the rows loaded but the ownership check hid your livery. Check `owned_items` in the session's `swofty/settings.json`.

To share your livery, package the addon as a `.swoftyaddon` ([Package it](/reference/addons/publishing#package-it)).

## Reference

- [Game database](./database) and [Textures and models](./textures): the two kinds of operation this addon uses.
- [Database operations](/reference/ops/database#operations) and [ERP operations](/reference/ops/erp#operations): every field of `add-row` and `copy-resource`.
- [Addon settings](/reference/addons/settings): the `picker_name` choice.
