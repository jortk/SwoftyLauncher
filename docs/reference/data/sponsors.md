---
title: Adding a sponsor
description: The rows, textures and texts a data addon adds for a new My Team secondary sponsor — sponsor, decal group, decal designs, unlock rows, career sponsor rows, discovery event, menu and car textures — plus the pitfalls that crash the game or break other addons.
---

# Adding a sponsor

A secondary sponsor in F1 25 is a brand the player can place as decals on the My Team car, and one that pays per decal and gains loyalty. You can add one with a data addon. It takes **database rows** in eight tables, **two copies of every decal image** (one for the menus and one for the car), a **registry entry** per menu image and the **texts**.

This page describes the stock data model and the order the rows must be added in. It also lists the mistakes that crash the game. Everything here comes from Sponsor Studio's exports, which pass the launcher's own database, package and texture checks. Each section says what was also [seen in game](#what-was-seen-in-game).

Clone each row from a stock sponsor. Avolo (`avolo`) is a good template: an ordinary secondary sponsor with two start decals and four loyalty decals, its own discovery event and rows in both career data groups.

## The rows

| # | table | one row per | key fields you set |
|---|---|---|---|
| 1 | `sponsors.sponsors.sponsor` | brand | `_metadata_` (the sponsor code), `statId`, `display_name`, `texture_name` (list tile) |
| 2 | `customisation.custom_decal_sets.decal_groups.DecalGroup` | brand | `_metadata_`, `id`, `sponsor`, `name_loc_tag`, `material_name`, `texture_name` |
| 3 | `customisation.custom_decal_sets.decal_sets.DecalSet` | decal design | `_metadata_` (`<code>_<suffix>`), `id`, `decal_group`, `material_name`, `texture_name` (`<code>_<suffix>_da`), `name_loc_tag` |
| 4 | `unlockable_content.unlockable_items.unlockable_item_list_<n>.folder_<m>.unlockable_decal_set` | design, in every list that holds the template's design | `_metadata_`, `customisation_item`, `statId` |
| 5 | `unlockable_content.…unlockable_decal_group` | brand, likewise | `_metadata_` (`dg_<code>`), `customisation_item`, `statId` |
| 6 | `career.my_team_rewards.reward_definitions.MyTeamSponsorUnlockReward` | brand | `_metadata_`, `sponsor`, `statId` |
| 7 | `career.my_team_sponsors.MyTeamSponsorsDataGroup_<g>.my_team_sponsors.secondary_sponsor` | brand **and** career data group (`_0`, `_1`) | `_metadata_`, `sponsor`, `sponsor_id`, `default_decals`, loyalty levels |
| 8 | discovery event (below) | brand | |

Decal designs are named `<code>_p` (primary) and `<code>_n` (alternate). Further start designs are `<code>_d3`, `<code>_d4` and so on, and the loyalty designs are `<code>_s1` … `<code>_s4`. Clone each one from the Avolo design of the same role (`avolo_p`, `avolo_n`, `avolo_s1` …).

```jsonc [db/sponsor.json]
{
  "schema_version": 1,
  "fields": { /* explicit field lists, see "Clone every field" */ },
  "ops": [
    { "op": "add-row", "table": "sponsors.sponsors", "from": "sponsors.sponsors.sponsor[_metadata_=avolo]", "copy": "all",
      "set": { "_metadata_": "acme", "statId": "129", "display_name": "lng_sponsor_acme", "texture_name": "acme_n_da" } },
    { "op": "add-row", "table": "customisation.custom_decal_sets.decal_groups",
      "from": "customisation.custom_decal_sets.decal_groups.DecalGroup[_metadata_=avolo]", "copy": "all",
      "set": { "_metadata_": "acme", "id": "131", "sponsor": "acme", "name_loc_tag": "lng_sponsor_acme",
               "material_name": "acme", "texture_name": "acme_n_da" } },
    { "op": "add-row", "table": "customisation.custom_decal_sets.decal_sets",
      "from": "customisation.custom_decal_sets.decal_sets.DecalSet[_metadata_=avolo_p]", "copy": "all",
      "set": { "_metadata_": "acme_p", "id": "754", "decal_group": "acme", "material_name": "acme_p_da",
               "texture_name": "acme_p_da", "name_loc_tag": "lng_sponsor_acme_p" } },
    // … acme_n, acme_s1..s4, the unlock rows, the reward …
    { "op": "add-row", "table": "career.my_team_sponsors.MyTeamSponsorsDataGroup_1.my_team_sponsors",
      "from": "career.my_team_sponsors.MyTeamSponsorsDataGroup_1.my_team_sponsors.secondary_sponsor[_metadata_=avolo]", "copy": "all",
      "set": { "_metadata_": "acme", "sponsor": "acme", "sponsor_id": "357", "default_decals": "acme_p,acme_n" } }
  ]
}
```

### The career sponsor row

`secondary_sponsor` exists once per career data group: `MyTeamSponsorsDataGroup_0` and `_1`, the two car years. Add the brand to both. Its `sponsor_id` lives in its own id space, separate from `sponsors.sponsor.statId`. Use one value for the brand in both groups.

The row you cloned still names Avolo in its nested loyalty track (`sponsor_loyalty_track_0.sponsor_loyalty_level_<n>.decal`, `.livery`, `.rewards`). Rewrite every nested value that contains `avolo`. A loyalty level whose design you don't add must get an **empty** `decal`, not a name that doesn't exist (see [pitfalls](#pitfalls-that-crash-the-game)).

`default_decals` lists the designs the player owns as soon as the sponsor is discovered. The game does not cap it at two: stock sponsors simply all have two. Loyalty levels with the same `required_xp` unlock together, but the UI's level numbers then skip.

### Discovery

A sponsor is **locked** until the career discovers it ("discover first" on its tile). Stock sponsors are discovered by a career department event. Each brand has its own event in `career.department_events.DepartmentEvents_2` (group `SponsorDiscovery`):

```text
DepartmentEvent (title, description, outcomes)
  -> Outcome (hidden_outcome_modifiers)
    -> OutcomeModifierMyTeamReward (rewards)
      -> MyTeamSponsorUnlockReward (sponsor)
```

The modifier table, `career.department_events.DepartmentEvents_2.outcome_modifiers.OutcomeModifierMyTeamReward`, is known to the launcher's key catalogue only by its hash `{e938c8a7d8a1e402}`. Clone Avolo's event, outcome and modifier under new labels. Point the modifier at your reward, and give the event its own `title` (and `description`) language key. Don't append your reward to Avolo's modifier: that unlocks your sponsor together with Avolo, under Avolo's title.

### Available from the start

`career.my_team_sponsors.MyTeamSponsorsDataGroup_<g>.initial_secondary_sponsors` is a comma list of sponsor codes that a new My Team career discovers at once. **Append to it with `db.list-append`** (launcher 1.3.6 or newer); never `set` it:

```json
{ "op": "list-append", "select": "career.my_team_sponsors.MyTeamSponsorsDataGroup_0", "field": "initial_secondary_sponsors", "value": "acme" }
```

A `set` writes the whole list. If two addons do this, the one that loads last silently removes the other's starting sponsors, and they stay locked in every new career. That's what happened to the engine-supplier sponsors of one addon when a sticker addon also set the list.

The game applies this list **only when a career is created**. A sponsor added to it later stays locked in careers that already exist. Data can't change that. The discovery is stored in the career save. See [sponsors.md in re/](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/re/research/sponsors.md#discover) for the game's discover routine, which a native plugin can call at career load.

## The textures

Every decal design needs **two** textures:

| use | archive (game path) | resource | size | levels |
|---|---|---|---|---|
| menu (catalogue, carousel) | `2025_asset_groups/ui_package/texture_arrays/customisation/sponsor_decals/<code>_<suffix>_da.erp` | `eaid://ui_package/art/textures/customisation/sponsor_decals/<code>_<suffix>_da.tif` | 340 × 244 | 1 |
| car | merged into `shader_package_2025/customisation_decals.nefx2.sm51.erp` **and** `…sm51_amd.erp` | `eaid://ego_package/decals/textures/<code>_<suffix>_da.tif` | 1024 × 1024 | 11 (resident) |
| brand logo | `…/customisation/decals/<code>.erp` | `eaid://ui_package/art/textures/customisation/decals/<code>.tif` | 268 × 160 | 1 |

- Build each one by copying Avolo's surface and view resources (`avolo_<suffix>_da`, `avolo`) and replacing the name and pixels. The game's own sponsor textures are **BC7 sRGB**, and BC3 also loads. Resize with premultiplied alpha and bleed the colour into the transparent edge, or the decal gets a dark fringe on the car.
- Only the car copies carry the full mip chain. The car decal is baked from the `ego_package` texture, not from the menu one: an addon that ships only the menu images shows the logo in the menus and nothing on the car.
- Deliver the car textures with an [`erp.overlay`](/reference/ops/erp) into **both** shader archives (`sm51` and `sm51_amd`). The overlay keeps the archives' shader resources.
- Register each menu archive as an asset group (`group.register`, template = the Avolo design's `.assetgroup`). Add a `TextureArrays` entry for it to the dynamic texture registry `eaid://ui_package/art/texture_arrays/ui_dynamic_texture_loading.uidta`, in **every** top-level `ui_package` archive that carries that registry (an `erp.patch-pkg` `clone-entry` of Avolo's entry in `customisation.sponsor_decals`, setting `AssetGroupPath` and `ResourceId`).

### The sponsor list tile

`sponsors.sponsor.texture_name` and `DecalGroup.texture_name` name the image on the sponsor's tile in the My Team catalogue. The two are equal for 126 of the 128 stock sponsors. For 99 of them the image is one of the sponsor's own decal designs: 79 `s1`, 10 `s2`, 6 `s3`, 2 `s4` and 2 `n`. A sponsor can also have its own tile image: add a `<code>_list_da` menu texture (340 × 244, registered like a decal) and point both fields at it. This works for stock sponsors as well.

The tile's logo is a fixed 3.4 × 2.44 quad inside a 4.15 × 2.32 window (`tiles_team_hq/3x4_decal_catalogue.idf`). A bigger texture doesn't fill the tile. Scaling the logo's `eXRef` uniformly by 4.15 / 3.4 (`erp.patch-pkg` `set-value` on `…children[name=decal_catalogue].scale`) makes it as wide as the tile. The window then shows rows 27–217 of the 244.

## The texts

| key | text |
|---|---|
| `lng_sponsor_<code>` | the brand name (sponsor `display_name`, decal group `name_loc_tag`) |
| `lng_sponsor_<code>_<suffix>` | the name of each design |
| the discovery event's `title`/`description` keys | the career message |

Use `lang.set` with `"language": "*"` for the name in every language, then one `lang.set` per translated language. The game's language codes are the ones in its `.lng` file names, not ISO codes.

## A team badge emblem

`customisation.custom_player_badges.emblems.BadgeEmblem` holds the emblems of the Team Badge editor:

- Clone `BadgeEmblem_17` (full colour, no colour masks) and set `id`, `loc_tag` and `ui_name` (`emblem_<code>`).
- Add an unlock row to each `unlockable_badge_emblem` list that holds the default emblem `BadgeEmblem_01` as a plain row (not one with `_ItemLockServerGeneric_instancecount_`).
- The texture is `…/customisation/cstm_badge_large/emblem/emblem_<code>.erp` (512 × 512), registered like a decal under `customisation.cstm_badge_large.emblem` in the texture registry.

The team badge stores the emblem by its `id`, not its `statId`.

## Ids

Every row above carries an id (`statId`, `id`, `sponsor_id`). Saved liveries store decal design ids (`DecalSet.id`), so:

- **Never reuse an id another addon uses.** Two addons that each took "highest stock id + 1" gave the same `DecalSet.id` to Pirelli and Toyota. The game then showed one addon's decal in place of the other's.
- **Never change an id once the addon is in use**, or placed decals turn into other decals in saved careers.
- Unlock-item `statId`s must be unique within each unlock list (`unlockable_item_list_<n>`, across all its folders), not just within one folder. Leave `statId` out of the unlock rows' `add-row` (rows 4 and 5): the launcher gives each one a free id, the same at every build, and moves a `statId` you set that the game or an addon earlier in the load order already holds ([Unlock items](/reference/ops/database#unlock-items)). An addon that already ships unlock `statId`s can keep them; leaving them out in an update gives those rows a new id once.

The launcher doesn't allocate the other ids (`DecalSet.id`, the sponsor's and the reward's `statId`, `sponsor_id`) yet. Keep each addon's ids fixed once it ships, and pick new ones that avoid both the game's ids and those of every installed addon. The [sponsor registry proposal](/design/sponsor-registry) moves that job into the core.

## Clone every field

The launcher's key catalogue doesn't know the row shapes of `unlockable_decal_group` and `secondary_sponsor` (the nested loyalty track). With `"copy": "all"`, list the fields of each cloned table in the op file's `fields` object, taken from the stock row. The build then copies them and reports the shape warnings as notes.

Some stock rows also carry numbered fields that the catalogue can't name at all. Only their hash is in the database. A `DepartmentEvent` `Outcome` row, for example, has `outcome_modifier_1` … `outcome_modifier_3`. Find them by hashing `<row>.<field><n>` against the database, and copy them too ([pitfalls](#pitfalls-that-crash-the-game)).

## Pitfalls that crash the game

All three were seen in game.

| symptom | cause | rule |
|---|---|---|
| Crash while the game starts, in the sponsor loader | a `secondary_sponsor` loyalty `decal` (or `default_decals`) named a design that doesn't exist; here a comma list typed into a single-decal field | every decal name must be an existing `DecalSet` `_metadata_`; a level without a design gets `""` |
| Crash when a My Team career starts | a cloned discovery `Outcome` lacked `outcome_modifier_1..3`; the department events reader looks each one up and uses the result without checking it | copy the hash-only numbered fields of every cloned row |
| One addon's decal shows in place of another's | two addons used the same `DecalSet.id` | [unique, stable ids](#ids) |

## What was seen in game

- **Seen in game** (owner's tests, 2026-10-30 to 2026-11-02): new sponsors listed in the My Team catalogue and placed on the livery, own list tiles, and the three failures above. Their causes were traced statically from the crash addresses and logs; the fixes are checked offline.
- **Offline only** (passes the launcher's checks, not yet seen in game): the filled tile scale, the `list-append` fix and an emblem in the badge editor.

## Reference

- Sponsor Studio (the tool these exports come from): source and releases by JonnieDesigns.
- The engine side (what the catalogue, editor and career code do with these rows): [`re/research/sponsors.md`](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/re/research/sponsors.md).
- [Database operations](./database), [ERP operations](/reference/ops/erp), [In-game text](./text).
