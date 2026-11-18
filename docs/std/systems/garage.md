---
title: Garage art
description: A team's own garage and pit lane textures for one Grand Prix or Time Trial box, as a team.garage record with a folder of DDS or PNG files.
---

# Garage art

Give a team its own garage and pit lane art in one box. Classic Seasons 2020, for example, shows the Renault garage in 2020 Renault colours, while Alpine 2021 to 2024 and the stock 2025 box keep their garages.

A few terms used on this page:

- A **box** is one of the year tiles in Grand Prix or Time Trial. Its season build is a `content.package` record.
- A **garage record** is a `teams::Garage`: the box, the team it dresses and a folder of textures.
- A **stock texture** is a game resource such as `eaid://environment_package/core_assets/objects/textures/garage_liveries/garage_livery_renault_a_01_da.tif.image`. The game carries one copy in every track's archives.

## The header

<Example root="swofty-sdk" code-file="include/swofty/systems/teams.hpp">
<template #code>

```cpp
#include <swofty/registry.hpp>

namespace swofty::teams {

struct Garage {                                    // one team's garage and pit lane art in one box
    Ref<content::Package> package;                 // the box's content package
    Ref<Identity> team;                            // the team identity the package's team entry names
    std::string entry;                             // or the key of the package's team entry, when it names no identity
    Path textures;                                 // a folder of .dds and .png files
    std::string root = "eaid://environment_package/core_assets/objects/textures/";
    bool srgb = true;                              // a .png is encoded as sRGB BC7
};

inline Registry<Garage> garages{"team.garage", { .applies = Applies::boot }};

}
```

</template>
</Example>

A file's path below the folder, without its extension, is appended to `root`. The file `garage_liveries/garage_livery_renault_a_01_da.dds` replaces `garage_livery_renault_a_01_da.tif.image` and its view `garage_livery_renault_a_01_da.tif`. A `.dds` keeps its own format (BC1, BC3, BC5 or BC7) and every mip level it holds. A `.png` is encoded to BC7 with a mip chain down to 1x1.

## The Renault 2020 garage, in code

<Split>

Classic Seasons 2020 keeps its Renault under the package team entry `lotus`.

- **The record names the box.** The art reaches only the asset groups of the cars that package gives the team, so a session of any other box never loads it.
- **The folder mirrors the stock names.** The addon ships `garage/renault_2020/garage_liveries/garage_livery_renault_a_01_da.dds` and the other textures it replaces. The team slot token (`renault_a`) is part of each stock name.
- **The core builds one archive.** The textures are packed into `swofty/addons/swofty.garage/garage_<hash>.erp`, named by a hash of the record, the files and the root. An unchanged boot reuses it.
- **The game opens it as an archive of its own.** The path `2025_asset_groups/swofty_garage/<record>.erp` is redirected to the cached file, and the car's asset groups list it first.

<template #example>
<Example root="swofty.classic-seasons-2020" code-file="native/src/garage.cpp" data-file="registry/garage.json" files="swofty.json, garage/renault_2020/garage_liveries/garage_livery_renault_a_01_da.dds, garage/renault_2020/pitlane/pitstop_gantry_renault_a_d.dds">
<template #code>

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/content.hpp>
#include <swofty/systems/teams.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    teams::garages.add("renault_2020", {
        .package = content::packages["swofty.classic-seasons-2020/f1_2020"],
        .entry = "lotus",
        .textures = file("garage/renault_2020"),
    });
}

SWOFTY_ADDON()
```

</template>
<template #data>

```json
// registry/garage.json
{
  "team.garage": {
    "renault_2020": {
      "package": "swofty.classic-seasons-2020/f1_2020",
      "entry": "lotus",
      "textures": "garage/renault_2020"
    }
  }
}
```

</template>
</Example>
</template>
</Split>

```text
[registry] team.garage swofty.classic-seasons-2020/renault_2020: art of lotus in swofty.classic-seasons-2020/f1_2020 for 1 car group(s): asset_groups/cs_2020_vehicle_package/teams/lotus/wep/lotus.assetgroup
[registry] team.garage swofty.classic-seasons-2020/renault_2020: asset_groups/cs_2020_vehicle_package/teams/lotus/wep/lotus.assetgroup lists 2025_asset_groups/swofty_garage/swofty_classic_seasons_2020_renault_2020.erp
[registry] team.garage swofty.classic-seasons-2020/renault_2020: 53 texture file(s) of garage/renault_2020/ in garage_0f3a9c41d27b6e55.erp (112 MB, built in 9120 ms), 2025_asset_groups/swofty_garage/swofty_classic_seasons_2020_renault_2020.erp -> swofty/addons/swofty.garage/garage_0f3a9c41d27b6e55.erp, 1 of 1 group(s) extended
[assets] group asset_groups/cs_2020_vehicle_package/teams/lotus/wep/lotus.assetgroup: 2025_asset_groups/swofty_garage/swofty_classic_seasons_2020_renault_2020.erp added to its ERP list (now 3)
```

## Naming the team

A package team entry names a team in one of two ways. Set `team` when the entry's `team` is a `teams::Identity` record, and `entry` when the entry has only a `key`, as Classic Seasons' entries do. A package with no entry for the team refuses the record, and so does a team whose cars have no asset group.

## Extracting the stock files

`tools/garage_textures.py` writes the textures of an archive as `.dds` files named like the stock resources:

```sh
python3 tools/garage_textures.py "shader_package_2025/pack.erp" garage/renault_2020 --check
```

`--match` keeps the resources whose name contains a text, and `--root` changes the root. `--check` compares each texture's header and view with the ones the core builds.

## What is known

- **Off by default:** the core leaves `team.garage` records unapplied and logs one line saying so. `SWOFTY_GARAGE_ART=1` applies them.
- **Surface layout:** each surface carries the stock fragments: the 40 byte header, every mip level and a streaming descriptor. The descriptor names `environment_package/.../<name>.tif.2025.mipmaps` below the root, lists no streamed level and is 288 bytes, as large as the stock ones (190 to 232 bytes). `SWOFTY_GARAGE_DESC=0` writes the two fragment surfaces of 1.7.8 to 1.7.10 again.
- **The crash reports:** `Ne_Crt_Memcpy+0x293` under the zstd fragment job (`0x143a6d4b0`) copies a 194 or 216 byte fragment that starts with a length byte of 0x66 and `environment_pac`. That is the third fragment of a stock surface, the streaming descriptor, written to a null destination. A stock surface has three fragments and the archive's had two, so the stock copy of the same name had no buffer for its third.
- **Not reproduced:** Time Trial runs of 90 seconds in `cs_f1_2020` and `cs_f1_2019` (Silverstone, the Renault slot, headless) did not crash with either layout. The 2020 box shows the 2020 Renault monitors and banner in the garage, the 2019 box keeps the Alpine monitors.
- **Built and tested:** the archive builder (names, headers, descriptors, views, mip chains, the cache key) and the lowering to the cars' asset groups run in the host tests.
- **Groups the core cannot extend:** a car group of the stock manifest. Only groups an addon declares, as Classic Seasons' cars do, take the archive.

## Parked cars and central mechanics

Native track addons can place a parked garage rig and read its two central mechanics with `<swofty/garage_placement.hpp>`. The initial backend supports F1 25 Steam build 26566308. It checks the executable digest and resolves its named functions through core when the game code is ready. Other builds return `Result::unavailable`. The active track must have a registered track ID that the current session exposes.

Subscribe to `SWOFTY_EVENT_GARAGE_PARKED_CAR`. Core fires this event on return from the engine's parked-car update. `event->a` carries the zero-based garage and `event->b` the native slot (0 or 1). Supply the complete bank of authored Pit world matrices from the same package. Core checks the track ID, occupied Pit indices, matrix bases and translations. An empty or ambiguous bank, wrong track or stale bay cannot be used.

```cpp
#include <swofty/events.hpp>
#include <swofty/garage_placement.hpp>
#include <string>
#include <vector>

namespace gp = swofty::garage_placement;
struct TrackGarages {
    std::string track;
    std::vector<gp::Pit> pits;
};

void inspect_parked(const SwoftyEvent *event, void *user)
{
    const auto &track = *static_cast<const TrackGarages *>(user);
    gp::visit(track.track.c_str(), track.pits.data(), uint32_t(track.pits.size()),
              uint32_t(event->a), uint32_t(event->b), [](const gp::Bay &bay, void *) {
        gp::WorldMatrix car;
        gp::CrewPositions mechanics;
        if (bay.idle && bay.read_parked_car(car) == gp::Result::ok &&
            bay.read_crew_pelvis(mechanics) == gp::Result::ok) {
            // Compare car.values[12..14], mechanics.mb and mechanics.eb1 with the editor markers.
        }
    });
}

// At addon initialization, keep track and its Pit data alive while subscribed.
// swofty::events::subscribe(SWOFTY_EVENT_GARAGE_PARKED_CAR, inspect_parked, &track, &token);
```

A `Bay` only accepts operations inside its synchronous visitor on the update thread. Retain `bay.generation` to discard calibration when a bay respawns. `read_root` gives the rig world matrix and `read_parked_car` gives the evaluated car pose. Both matrices are row-major with world XYZ in elements 12 through 14. For an idle rig, `set_root` writes the requested world matrix, updates its attached scenes and renderer, then calls the engine's scene refresh. Save the old root to restore it. Check the evaluated car again on later events because the game's cached bone pose can span updates.

`read_crew` reads MB and EB1 actor roots. `read_crew_pelvis` returns the evaluated world XYZ of each actor's animated `Bip01`. Both work independently before placement and after restoration. Compare samples from matching idle animation phases, or average each complete loop when the editor marker represents its mean. `set_crew` takes a world-axis translation in metres for each actor, measured from its native scene definition on every application (at most 3 m horizontally and 0.05 m vertically). Restore with `restore_crew` before relocating the rig. Core also restores an outstanding pair when the garage leaves idle. A failed transaction returns `rolled_back` if restoration succeeded or `rollback_failed` if independent restoration could not be completed. Treat each operation's result separately from the return from `visit`, which only reports delivery of the visitor.

### Individual mechanics and vehicle

The V2 operations use the same scoped visitor. A compiled V1 addon continues to use the original export. A V2 client receives `unavailable` for the new operations on a V1 core.

```cpp
namespace gp = swofty::garage_placement;
// Inside a visitor; run placement when the author requests it.
gp::Mechanics roles;
if (bay.mechanics(roles) == gp::Result::ok && bay.idle) {
    auto role = gp::mechanic_from_name("FL");
    if (role && roles.contains(*role))
        bay.set_mechanic_offset(*role, gp::Position{0.4f, 0.f, 0.1f});
}
```

**One person.** `mechanic_name` and `mechanic_from_name` use canonical package keys. `mechanics` reports the unique roles in the selected crew scene and a separate mask for ambiguous names. The F1 garage scenes examined contain MB, EB1, FJ1, FJ2, FL, FR, RJ, RL, RR and ENG, with native colon-suffixed aliases for several roles. F2's player and competitor garages contain MB, EB1, FJ1, FJ2, RJ and ENG. EB2 is a recognized package key. Its actor is absent from all twelve inspected garage scenes, including scenes carrying EB2 animation clips. Equipment and cameras are not selectable roles.

`read_mechanic` returns the actor's world matrix. `read_mechanic_pelvis` returns its animated `Bip01` position. `set_mechanic` accepts an absolute actor world matrix. `set_mechanic_offset` applies world-axis metres to its native root, preserving its basis. Repeating the same offset is idempotent. `restore_mechanic` restores that role alone. A missing actor returns `missing`, a duplicate returns `ambiguous`, and unready evaluation returns `unavailable`. Successful writes are read back. A failed write tries to restore the pre-call world matrix even when replacing an earlier edit. An active V1 MB/EB1 pair and an individual edit of MB or EB1 return `conflict` on overlap. Other roles can be edited alongside the pair. Native departure restores pending people on the owned bay, with retries on its subsequent update. Session end or unload invalidates the old generation and releases its saved state.

**One parked car.** After any authored whole-bay placement, read the evaluated car using `read_parked_car`. Pass the desired absolute game-space matrix to `set_parked_car`. Only the verified vehicle-only scene receives the change. Its locator, renderer metadata, crew scenes, whole-bay root and other cars keep their own transforms. Independent mechanic edits can coexist. Further whole-bay `set_root` operations require restoring the individual edits first.

An accepted vehicle-scene change returns `pending`. On subsequent parked updates, query `parked_car_result`. Core compares the separately evaluated car position and basis, allowing up to 2 cm of translation error and 0.009 per basis element. If multiple observations over two seconds miss, core restores the scene that preceded that request and checks the car again. This reports `rolled_back` or `rollback_failed`. Another set while verification is pending leaves the request intact. `restore_parked_car` returns the first pre-edit vehicle scene and also awaits an evaluated readback. A departure restores the scene immediately when its ownership still matches. Car verification resumes on its next idle update.

Track addons can use the same API for any registered custom or official GP track. Pass the track's qualified registry ID and its complete bank of Pit matrices for the active layout. The current session must expose that ID. Time Trial currently omits it, so the visit returns `unavailable` there.

The Windows integration fixture exercised twelve F1/F2 cases using garage matrices from official and custom track packages with simulated game objects. It covered a complete garage layout and a pair of individual edits exported after reopening the Blender project. The fixture checked that moving a parked car preserves the rig root and neighbouring actors, that each mechanic can be placed independently, and that departure restores the edits. In-game position measurements are still pending.
