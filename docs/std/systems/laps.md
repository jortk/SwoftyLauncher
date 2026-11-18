---
title: Laps
description: The lap files the Time Trial plugin records, and the two events around writing one.
---

# Laps

While Time Trial boards are on and the player is signed in with Discord, every completed Time Trial lap is recorded to a file under `laps/` in the launcher's data folder. When the data folder can't be written, the file goes under `swofty/laps/` in the game folder instead. An addon reads those files, adds a section to one before it is written, or reacts when one lands.

The recorder is part of the [Time Trial plugin](#the-time-trial-plugin). It runs when the launcher writes `"tt_boards": true` into `swofty/tt.json` and the session is a Time Trial. It samples the player's car at 60 Hz. A lap starts at a line crossing and ends at the next one. The lap out of the garage is not recorded. A lap is dropped when the car is reset or moved, when the session ends, or when it is too short or too long to be a lap.

A completed lap waits up to two seconds after its line crossing for the game's lap result and, when the lap is valid, for the ghost blob the game commits on a personal best. The file is written when both have arrived or when the two seconds have passed. Without the game's lap result the lap's time comes from the clock. `SWOFTY_LAP_GAME_RESULT=0` turns the lap result off and `SWOFTY_LAP_GHOST=0` the ghost blob.

## The file

A lap is one file, `laps/20261110T041011Z-3.swlap`: the UTC time it was written and the lap's number in the session. It is written as a `.tmp` file and renamed. A write that fails in the data folder goes to `swofty/laps/`, and the loader log names the folder and the Windows error code of the failed write. The launcher collects laps from both folders. On an EA app copy under `C:\Program Files\EA Games` the game can't create files in `swofty/`.

```
0   8   magic  "SWLAP\0\1\0"
8   4   u32 header length
12  n   header, JSON, UTF-8
..      sections: 4-byte tag, u32 length, bytes
          "SMPL"  u32 count, u32 stride, then count * stride bytes
          "GHST"  the game's own ghost blob for the lap, when there is one (1 to 131072 bytes, copied verbatim)
          "ATST"  an attestation, appended by a laps.sealing handler
          "END\0" length 0
```

Numbers are little-endian. A reader skips a section whose tag it does not know.

The header names what the core knows and leaves out the rest:

```json
{
  "format": 1,
  "core": "1.8.38",
  "game_build": "<exe sha256>",
  "board": { "content": "swofty.swofty-2022/f1_2022", "track": "bahrain", "weather": "dry", "content_name": "Formula 1 2022" },
  "car": { "team": "game/mercedes", "vehicle": "game/mercedes_2025", "driver": "game/russell",
           "team_stat": 4, "vehicle_stat": 17, "livery_stat": 0 },
  "lap": { "time_ms": 91234, "sectors_ms": [29001, 31002, 31231], "valid": true, "source": "game", "lap_index": 3,
           "started_utc": "2026-11-10T04:10:11Z", "session_time_s": 412.5 },
  "assists": { "brk": 1, "abs": 1, "trc": 2, "drl": 0, "gbx": 3, "ers": 1, "input": 2 },
  "performance": "equal",
  "rate_hz": 60,
  "fields": [
    { "name": "t", "type": "f32", "unit": "s" },
    { "name": "px", "type": "f32", "unit": "m" },
    { "name": "gear", "type": "i8" }
  ],
  "nonce": "<the value of tt_nonce in tt.json>"
}
```

`lap.source` is `game` when the time is the game's own lap record and `clock` when the recorder measured it between two line crossings. `lap.valid` and `lap.sectors_ms` are present when the game's record gives them. The core takes the game's record only when the lap time is finite and between 5 seconds and 1 hour, every sector is positive, and the sectors add up to the lap time within 10 ms when the lap and all three sectors are valid. A record that fails these is logged and the lap's time comes from the clock.

These keys are left out when the core could not read them:

| Key | Meaning |
| --- | --- |
| `lap.session_time_s` | the core's telemetry session clock when the line was crossed, seconds since the session went live |
| `assists` | the raw assist bytes of the game's telemetry lap: `brk`, `abs`, `trc`, `drl`, `gbx` (3 is the game's automatic gearbox), `ers`, and `input` 0 pad, 1 wheel, 2 keyboard, 3 direct |
| `performance` | `realistic` or `equal` |
| `car.team_stat` `car.vehicle_stat` `car.livery_stat` | the game's stat ids of the player's participant |
| `board.content_name` | the title of the era box the content id names |
| `board.track_name` | the track's display name from the core's track base |
| `board.track_addon` `board.track_kind` | on a custom track, the addon id that adds or fills the track and `record` or `older`; left out on a stock track |

`board.weather` is guessed from the track wetness when the lap starts. The core has no read of the Time Trial weather setting. `board.game_board` is the last board name the game's leaderboard screen requested in the session, written when its weather matches the lap's. `ghost_id` is the base64 of the 0x30-byte id block of the last committed personal best, written on laps that carry a `GHST` section. Both keys are left out when unknown.

The ghost blob is built by the game on a personal best only, after the lap result. A lap's `GHST` section is the blob committed within two seconds of its line crossing, so a lap that is not a personal best has none.

A sample is the values of `fields`, in order, packed with no padding. `stride` is the sum of the field sizes. A reader parses samples from `fields` and nothing else, so the core adds a field without a new format number.

```cpp
const uint8_t *sample = smpl + 8 + i * stride;
float t;
std::memcpy(&t, sample + offset_of("t"), sizeof t);
```

The types are `f32`, `f64`, `i8`, `u8`, `i16`, `u16`, `i32` and `u32`. A field the core could not read in the running game is missing from `fields`.

| Field | Meaning |
| --- | --- |
| `t` | seconds since the lap started |
| `dist` | metres driven since the lap started |
| `px` `py` `pz` | world position, metres |
| `vx` `vy` `vz` | world velocity, metres per second |
| `qx` `qy` `qz` `qw` | orientation |
| `throttle` `brake` `steer` | pedals 0 to 1, steering -1 to 1 |
| `gear` `rpm` `speed` | gear (-1 reverse, 0 neutral), engine rpm, km/h |
| `ws_fl` `ws_fr` `ws_rl` `ws_rr` | wheel spin, radians per second |
| `slip_ang_*` `slip_ratio_*` | tyre slip angle and slip ratio per wheel |
| `surf_fl` `surf_fr` `surf_rl` `surf_rr` | the surface under each wheel (the game's surface id, 255 unknown) |
| `g_lat` `g_long` | the chassis g signals |
| `drs` | DRS wing open fraction |

## Custom tracks

A track an addon adds, or a dormant stock row an addon fills, is a custom track. The core reads the session's track code against its track base (`apply::tracks::Base`, filled at boot) and writes `board.track` as:

- a stock track: the code, `bahrain`
- a custom track, kind `record` or `older`: `<addon id>:<code>`, lowercased, `swofty.tracks:ring`

The key has a colon and never a slash. The game's own season (`game/f1_2025`) and every addon season carry the key. When two rows share a code, the track number of the game's last board name picks the row. A code the base does not list, or a base that is not ready, gives the lowercased code with no `board.track_addon` and `board.track_kind`, and the core logs once:

```
[laps] track base: no track with this code; the board key is the track code 'ring' (logged once)
```

```json
"board": { "content": "game/f1_2025", "track": "swofty.tracks:ring", "weather": "dry", "track_name": "The Ring",
           "track_addon": "swofty.tracks", "track_kind": "record" }
```

The mailbox request's `board.track` carries the same key. The core takes the request's track from the board name's track number through the base's stat ids, and from the session's track when the base has no such stat id.

A board is the core's own when its descriptor is 6 or higher or when its track is custom. The request redirect and the guards use that one rule. A request for a stock descriptor on a custom track goes to the mailbox like an addon board, and with guards on a lap on a custom track in the game's own season does not reach the publisher's boards.

## The Time Trial plugin

The launcher places `swofty.tt` for a session in which the player is signed in with Discord: a closed-source DLL that Swofty's server publishes. The launcher reads the offer from `GET /v1/modules`, downloads the file from the CDN link in the offer when the offer has one (`https`, no sign-in sent) and from `GET /v1/modules/file` otherwise or when the CDN copy doesn't arrive or doesn't match, and checks its size, its sha256 and an Ed25519 signature over `swofty-module-v1\0 || name || \0 || platform || \0 || version || \0 || abi || \0 || sha256` against the public key pinned in the launcher. A copy that passes is kept under `modules/tt-game/<platform>/<version>/` in the launcher's data folder. When Swofty's server answers a download with `too many requests` or a server error, times out or can't be reached, the launcher uses the newest cached copy that still verifies. The session places it as `swofty/addons/swofty.tt/swofty_tt.dll`, and the session's rollback removes it with every other placed file. It is not an addon in any profile.

The plugin attaches to the core's lap host, `SwoftyCoreLapHost` in `swofty_api.h`:

```
[tt] attached to the core's lap host (version 1)
module tt-game: not loaded for this session: none published
```

Without the plugin no lap is recorded and the in-game boards are off.

## The integrity module

The launcher loads one more native addon for a session in which Time Trial boards is on and the player is signed in with Discord: `swofty.integrity`, a closed-source DLL that Swofty's server publishes. The launcher fetches it from `GET /v1/tt/integrity`, checks its size and sha256, and checks an Ed25519 signature over `swofty-tt-integrity-v1\0 || version || \0 || sha256` against the public key pinned in the launcher. A copy that passes is kept under `timetrial/integrity/<version>/` in the launcher's data folder, the newest two versions. The session places it as `swofty/addons/swofty.integrity/swofty_integrity.dll` and lists it in `swofty/settings.json` under `plugins`, and the session's rollback removes it with every other placed file.

It is not an addon in any profile, the addon list, the load order or the addon set a lap is uploaded with. It is named in `loader.log` as an addon DLL and in the launcher's session log:

```
integrity: swofty_integrity.dll 2026.11.10.1 under swofty/addons/swofty.integrity
integrity module: not loaded for this session: couldn't reach https://...
```

When the offer is missing, unreachable or fails a check, the session starts without the module and the lap is written without an `ATST` section.

At `laps.sealing` the module appends the `ATST` section: a length-prefixed JSON document and a 32-byte HMAC-SHA256 of it.

```
ATST = u32 json_len, json bytes, 32 bytes mac
json = { "v": 1, "module": "2026.11.10.1", "nonce": "...", "body_sha256": "...", "time_ms": 91234,
         "values": { ... }, "db": { ... }, "modules": [ { "name": "swofty_core.dll", "sha256": "..." } ], ... }
```

`body_sha256` is the hash of the lap file before the section, `values` and `db` are the handling values and database rows read for the player's car, and `modules` lists the DLLs loaded from the game folder. The key of the MAC is held by the server.

## In-game boards

The Time Trial plugin shows addon content on the game's own Time Trial leaderboard screen, the Time Trial track select's View Leaderboards. It is off until the launcher writes `"tt_bridge": true` into `swofty/tt.json`, which it does when Time Trial boards in the game is on. Each step logs one `[boards]` line.

The game names a board `LBV` + the descriptor id (2 digits) + the track (2 digits) + `D` or `W`: `LBV0605D` is descriptor 6, track 5, dry. Descriptors 0 to 5 are the game's own. Descriptors 6 and up are addon boards.

A request for an addon board or a custom track's board goes to the launcher instead of the game's servers. The plugin writes it to `tt/outbox/<seq>.json` in the launcher's data folder (`swofty/tt/outbox/` in the game folder when it can't write the data folder):

```json
{ "seq": 12, "method": "GetTTLeaderboard",
  "args": { "LeaderboardID": "LBV0605D", "Version": 3, "RangeType": 0, "FromIndex": 0, "ToIndex": 9, "TopResultCount": 3, "CrossPlay": 1 },
  "board": { "content": "swofty.swofty-2022/f1_2022", "track": "bahrain", "weather": "dry", "game_board": "LBV0605D" },
  "player": { "ghost_id": "<base64 of the 0x30-byte id block, once a personal best was committed>" } }
```

The launcher answers with `tt/inbox/<seq>.json` beside it, `{ "seq": 12, "status": 200, "body": "<JSON text>" }`. Both sides write a `.tmp` file and rename it, and both ignore files older than 60 seconds. A request with no answer after 8 seconds completes with status 504 and the body `{"Message":"The request timed out","Data":null}`. Requests for the game's own boards go to the game's servers unchanged.

Guards are off. `"tt_guards": true` in `swofty/tt.json` or `SWOFTY_TT_GUARDS=1` turns them on. A lap in addon content or on a custom track then does not reach the publisher's boards.

## Barred addons

The boards server rejects a lap recorded with a barred addon enabled. It lists them, without a sign-in:

```
GET /v1/tt/barred
{ "addons": [
  { "id": "kizikafuka.ai-drive", "name": "AI Drive", "reason": "the AI can drive your car" },
  { "id": "swofty.rich-fuel-mix", "name": "Rich Fuel Mix", "reason": "...", "below": "1.0.3" }
] }
```

An entry with `below` bars the installed versions lower than it. An entry without `below` bars every version. An installed version that is not a version is barred.

The launcher keeps the list in `$DATA/cache/tt-barred.json` and asks for it again once it is five minutes old. A launch does not wait for the server: the wrapper writes the profile's enabled barred addons from that file into `swofty/tt.json`, then asks the server on a thread of its own and writes the key again when the answer differs. With nothing cached and the server unreachable, the key is left out.

```json
{ "tt_boards": true, "tt_barred": [ { "id": "kizikafuka.ai-drive", "name": "AI Drive" } ] }
```

While the game is in Time Trial, in the menus and on track, the core paints one red line at the top of the screen. The main menu and every other mode hide it. The core reads `tt.json` again every two seconds while in Time Trial. `SWOFTY_TT_BARRED=0` turns the line off.

```
[tt-barred] Not allowed in Time Trial, laps won't count: AI Drive
[tt-barred] the present hook arms for the Time Trial barred addons warning
[tt-barred] warning shown: the game is in Time Trial
[tt-barred] warning hidden
```

The launcher's Time Trial page shows the same addons with the reason, and `swofty laps list` prints them:

```
Not allowed in Time Trial, laps won't count: AI Drive
  AI Drive (kizikafuka.ai-drive): the AI can drive your car
```

## Events

`laps.recorded` fires on the flow thread once the file is in place. `laps.sealing` fires just before, on the core's writer thread, with the file as built so far; a handler reads the body it is given and does not read game memory.

```cpp
#include <swofty/systems/laps.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    events::on<laps::events::Recorded>().after([](const laps::events::Recorded &e, const events::Context &) {
        swofty::log("lap file %s, %d ms", std::string(e.path()).c_str(), e.time_ms);
    });

    events::on<laps::events::Sealing>().before([](laps::events::Sealing &e, const events::Context &) {
        uint8_t digest[32] = {};
        hash(e.body, e.body_len, digest);
        e.add_section("ATST", digest, sizeof digest);
    });
}
```

`Recorded::valid` is 1, 0, or -1 when the lap's validity is not known. `Sealing::add_section` refuses the tags `SMPL`, `GHST` and `END\0`, and the core takes at most four sections of at most 1 MiB together. The core adds the `END\0` section after them.

The recorder logs one line per lap in `swofty/loader.log`:

```
[laps] wrote 20261110T041011Z-3.swlap lap 91234 ms (game), 5474 samples, ghost 0 bytes, board game/f1_2025|bahrain|dry
[laps] wrote 20261110T041502Z-4.swlap lap 92011 ms (game), 5510 samples, ghost 0 bytes, board game/f1_2025|swofty.tracks:ring|dry, custom track (record from swofty.tracks)
```

`[laps] timing started` carries the same board and suffix.

The hooks log as they read:

```
[laps] lap result from the game: car 0 record 3 (message lap 3) 91234 ms, sectors 29001 31002 31231 ms, lap valid, sectors 111 valid
[laps] game lap extras: assists brk 1 abs 1 trc 2 drl 0 gbx 3 ers 1, input wheel, performance realistic
[laps] ghost blob from the game: 51284 bytes
```
