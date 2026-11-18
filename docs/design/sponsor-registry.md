# A sponsor registry

If you're contributing, this is a proposal for a `sponsor` registry in the standard library. The schema is
[`packages/sdk/std/proposals/sponsor/1.0.json`](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/packages/sdk/std/proposals/sponsor/1.0.json), status
experimental. Nothing here is built into the core yet. Everything it describes already runs in a community addon
(Sponsor Studio and its Sponsor Base by JonnieDesigns), so the proposal is about moving known-good behaviour into
the core, not about new research.

## Why the core should own sponsors

Adding a sponsor today means about 20 database operations, 2 textures per decal design, a texture registry
patch in several archives and a shader-archive overlay ([Adding a sponsor](/reference/data/sponsors)).
Three problems involve several addons at once and can't be solved well by one:

1. **Ids.** Every row carries a `statId`/`id`/`sponsor_id`, and saved liveries store `DecalSet.id`. Two addons that
   each took "highest stock id + 1" gave Pirelli and Toyota the same decal id, and the game showed one in place of
   the other. Today each tool has to scan every installed addon for the ids it uses.
   The core already allocates `StatId: "auto"` for other registries and can keep the allocations stable per record.
2. **The starting list.** `initial_secondary_sponsors` is one comma list per career data group. An addon that `set`s
   it removes every other addon's starting sponsors. `db.list-append` fixes this, but authors keep using `set`.
   A registry field (`career.available_from_start`) makes the right operation the only one.
3. **Existing careers.** The game copies that list into a career only when the career is created. A sponsor
   available from the start stays locked in older careers, and only native code can discover it there
   ([below](#native-part)).

## The record

```json [registry/sponsor.json]
{
  "sponsor": {
    "acme": {
      "name": { "literal": "Acme Lubricants" },
      "logo": "sponsors/acme/logo.png",
      "designs": [
        { "key": "p", "image": "sponsors/acme/primary.png" },
        { "key": "n", "image": "sponsors/acme/alternate.png" },
        { "key": "s1", "image": "sponsors/acme/s1.png" }
      ],
      "career": { "discovery": { "title": { "literal": "Acme wants in" } } }
    }
  }
}
```

Stock sponsors are `game/<code>` (128 in F1 25), seeded from `sponsors.sponsors.sponsor`. A layer on one changes its
name, logo, list tile or designs without new rows, the same way `team.logo` layers work on `game/<team>`.

## What the core writes, per field

| field | rows and files |
|---|---|
| record | `sponsors.sponsor`, `DecalGroup` (`id`, `sponsor`, `material_name`, `texture_name`), `MyTeamSponsorUnlockReward`, `unlockable_decal_group` in every list that has the template's |
| `designs[]` | `DecalSet` `<code>_<key>`; `unlockable_decal_set` per list; menu texture 340 × 244 + `group.register` + `TextureArrays` entry in every `ui_package` copy of `ui_dynamic_texture_loading.uidta`; car texture `eaid://ego_package/decals/textures/<code>_<key>_da.tif`, 1024 × 1024 with 11 mips, BC7 sRGB, in an overlay into both `customisation_decals.nefx2.sm51` and `_amd` |
| `name`, design `name` | `lng_sponsor_<code>`, `lng_sponsor_<code>_<key>` in every language (`lang.set *`) |
| `logo`, `list_tile` | `customisation/decals/<code>` (268 × 160); `<code>_list_da` (340 × 244) with `sponsor.texture_name` and `DecalGroup.texture_name` pointed at it |
| `career` | `secondary_sponsor` in every `MyTeamSponsorsDataGroup_<g>`, cloned from the template, every nested template reference rewritten, levels without a design written empty; `default_decals` |
| `career.discovery` | a new `DepartmentEvent` → `Outcome` → `OutcomeModifierMyTeamReward` chain cloned from the template's |
| `career.available_from_start` | `db.list-append` of the code to `initial_secondary_sponsors` in every group; plus the existing-career discovery (native) |
| `emblem` | `BadgeEmblem` (cloned from `BadgeEmblem_17`), unlock rows in the lists holding a plain `BadgeEmblem_01`, `emblem_<code>` texture 512 × 512 under `customisation.cstm_badge_large.emblem` |
| `stat_id`, design `id`, `career.sponsor_id`, `emblem.id` | allocated by the core when `"auto"`, kept per record once used |

Rules the core must keep. The game crashes or misbehaves without them (all seen in game):

- every decal name a row holds (loyalty `decal`, `default_decals`) names an existing design, or is empty;
- cloned rows copy their hash-only numbered fields (`Outcome.outcome_modifier_1..3`), which the key catalogue can't
  name;
- ids are unique across all enabled addons and never change.

## Native part

These behaviours need code in the game process. Sponsor Studio's Sponsor Base does each one today, with byte-checked
signatures on the KB build. The engine research is in [`re/research/sponsors.md`](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/re/research/sponsors.md),
with KB rows proposed in the same contribution. Levels follow PROTOCOL §1; none of the rows is independently
verified (PROTOCOL §5).

| behaviour | field | how | level |
|---|---|---|---|
| discover starting sponsors in existing careers | `career.available_from_start` | once per loaded My Team career, call the game's discover routine `0x1410f43f0(CareerSponsorsComponent*, u16 sponsor_id, u32 team_key)` for codes in the career's starting list (`+0x4e0`) that the player team lacks | L2 static, harness only |
| engine supplier sponsors | `my_team.engines` | hide the sponsor's catalogue tiles and designs unless the team's newest `EngineSuppliersComponent+0xe30` history entry is one of its engines; remove its decals from the livery when the supplier changes; leave them out of the per-decal income | catalogue filter seen in game (owner, 2026-11-01); removal and income offline |
| fixed sponsors | `my_team.fixed` | add the decal to the saved My Team livery at career load and on editor save; locked: grey swap/delete (raise the reserved-slot count at the menu's own check), Duplicate and Mirror, hide from the catalogue grid, not paid | add, greyed swap/delete and income seen in game (owner, 2026-10-30) |
| emblem stickers | `emblem` | list the emblem's designs only while the My Team team badge uses that `BadgeEmblem.id` | L2 static |
| unpaid sponsors | `my_team.paid` | subtract its decals from the weekly per-decal income count | seen in game for fixed sponsors |

In the core this would be one system addon, or a `sponsors` service, that reads the effective sponsor records instead
of Sponsor Studio's per-addon `.cfg` files.

## Open questions

- **Calling into the career from the frame tick.** The discover call and the livery apply run on the core's frame
  tick, a game worker thread. Calling the same functions there worked in the harness only; the game's own callers'
  threads aren't traced.
- **Title sponsors.** They're out of scope: their contracts (`team_primary_sponsors`, goals, core beliefs) are a
  different row family. The cut fourth core belief `YouthDevelopment` is still loaded and translated but unused.
- **Per-decal colour.** The decal node has a saved RGBA colour slot (`+0x18`, default `0xffffffff`) that nothing in the
  UI sets. Whether the renderer uses it was asked by a write test (T016) that hasn't run yet. Until then the
  schema has no colour field.
- **Required areas.** A user-chosen blocker for title-sponsor-like slots needs native guards beyond `my_team.fixed`.
