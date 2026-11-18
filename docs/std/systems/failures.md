---
title: Failures, damage and retiring
description: Reported findings on F1 25's mechanical failures, terminal damage, garage repairs, result statuses and session restarts, from the Terminal Damage addon. Not yet independently verified.
---

# Failures, damage and retiring

The std library has no operation for scripted failures, retiring a car or disqualifying one yet. `sessions::issue_penalty` and `sessions::PenaltyKind` on [Sessions](/std/systems/sessions) are the design and aren't in the SDK headers. Until they are, an addon that needs these reads and calls the game directly.

::: warning Reported, not verified
Everything on this page comes from the author of the Terminal Damage addon (`nick.terminal-damage-probe`), who checked it in game with that addon on the Steam build (`F1_25.exe` SHA-256 `d873a1c8…`). It isn't in the knowledge base as verified and has no independent check under [`re/PROTOCOL.md`](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/re/PROTOCOL.md). Treat it as a starting point: offsets hold for that build only, and the core's bindings don't use them.
:::

Addresses are absolute with image base `0x140000000`. Sources: [issue #691](https://github.com/Swofty-Developments/SwoftyLauncher/issues/691) and [the addon's code](https://github.com/nyannoying1337/f1-25-terminal-damage).

Two of these addresses have a different name in the knowledge base, which came from its own reading of the code:

| Address | Name below | Name in `re/db/functions.tsv` |
|---|---|---|
| `0x1409b2940` | Failure planner | `Ai_CannedAccident_ApplyFailure` (L1) |
| `0x142bf50a0` | RepairTimes manager update | `Game_RepairTimes_FinishFadeIn` (L2) |

## Failure scheduler

The scheduler is at `0x1409b9aa0`, with a wrapper at `0x1409bbcf0`. It runs once per session start, not per lap.

- It rolls three Mersenne Twister numbers per car into a list on its manager (`[mgr+0x1b8..0x1c0]`, stride `0x18`). `+8` is the threshold the failure chance must beat.
- The career seed is per race weekend, so restarting the race or the game gives the same outcome, and only the timing differs.
- The player's loop is skipped when the `faults_and_failures_override` or `faults_and_failures` option says so. A missing option counts as enabled.

A rolled breakdown fills the car's failure object (`[mgr+0x198..0x1a0]`, stride `0x10`, pointer at `+8`):

| Offset | Field |
|---|---|
| `+0x78` | Car index |
| `+0xa0` | Part: 0 ICE, 1 MGU-K, 2 MGU-H, 3 ES, 4 TC, 5 CE, 6 gearbox |
| `+0xa1` | State |
| `+0xa4` | Lap, from 0 |
| `+0xa8` | Lap fraction |
| `+0xb1` | Branch |

The breakdown moves through its states like this:

1. State 0 moves to state 1 when the lap and fraction are reached.
2. State 1 calls the planner, which announces the failure on the radio.
3. State 2 hands the car over with `VehicleControllerSystem::setController(car, AI, reason 4 RETIRED)`. From then on the player can't retire themselves.

Temporary fault records are `0x34` bytes each, in the failure object's vector, registered by `0x1409b37a0`: `+0` type, `+4` lap, `+8` lap fraction, `+0xc` duration. Types seen working in game are 1 gearbox, 3 gear jammed and 5 DRS. Type 0 (engine) registers but had no visible effect.

Controller reasons seen in `setController`: 0 `PIT_LANE`, 2 `DISQUALIFIED`, 3 `NOT_IN_SESSION`, 4 `RETIRED`, 10 `SESSION_NOT_STARTED`, 11 `SAFETY_CAR`.

## Disqualifying a car

`Sess_Rules_Disqualify` (`0x143571440`, `void (void *rules, uint32_t vehicle)`) works mid-race, not only for the formation-lap timer. Called on the game thread, it hands the car to the AI with reason 2. `rules` is the `rcx` of `RuleManager::onSessionEnter` (`0x1435676a0`). A later retire request still sets `RETIRED` on the same car.

## Session results

`Sess_Result_ChangeVehicleStatus` (`0x142c7bae0`) takes the result in `rcx`:

| Offset | Field |
|---|---|
| `+0x00` | Vehicle (u8) |
| `+0x18` | Status |
| `+0x19` | Reason |
| `+0x11d`, `+0x11e`, `+0x11f` | Pending request: status, reason, flag |

- **Status:** 2 active, 3 finished, 4 DNF, 5 disqualified, 6 not classified, 7 retired.
- **Reason:** 1 retired, 3 terminal damage, 6 black flagged, 8 canned event failure.

At the session end, the race ruleset (`0x142d2b7b0`, loop at `0x142d2c26c`) requests DNF for every car short of the classification distance without checking for a disqualification, so a disqualified car shows as DNF in the results. To keep it, hook `Sess_Result_ChangeVehicleStatus`: when the car's status is 5 and the pending status is 4, set the pending status back to 5.

## Restart Session

**Restart Session** in the pause menu is not a session start. It fires no session end, no session start and no controller change with reason 3. What fires is `CarbonDamageSystem::onSessionRestart` and a rebuild of all controllers, so `car.controllers.rebuilt` and `car.built` fire again. An addon that keeps per-session state must reset it from those two.

The pause menu offers no **Return to garage** while the player's car is broken. Only the retire and restart entries remain.

## Car speed without UDP

The game's car telemetry writer (`0x142cf9820`) reads the speed from the car object. Check each pointer for null: the chain isn't valid while the car is being built.

```cpp
uintptr_t physics = *(uintptr_t *)(car + 0x558);
uintptr_t rigid   = *(uintptr_t *)(physics + 0x340);
uintptr_t body    = *(uintptr_t *)(rigid + 0x18);
float *v = (float *)(body + 0x220);                    // velocity in m/s: 4 floats, w is 0
float kmh = sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) * 3.6f;
```

## Terminal damage

`VehicleTerminalDamageSystem::updateVehicle(system, u8 vehicle)` at `0x142f16490` runs every frame for every car. A record is at `system + 0x50 + vehicle * 0x70`, and the count is at `system + 0xc20`.

- `record+8 -> +0x58` is the car object, and `car+0x530` is the engine.
- `+0x10 + i*4` is the u32 state of part `i`, of 11. State 5 is terminal.
- `+0x68` is the u32 overall state.

| Part index | Part |
|---|---|
| 0 | Engine |
| 1 | Power unit |
| 2 | Gearbox |
| 3 | Wheel lost |
| 4 | Suspension |
| 5 | Tyres (likely) |
| 6 | Aero (not mapped) |
| 7 | Sidepods |
| 8 | Floor |
| 9 | Rear wing |
| 10 | Diffuser |

The game breaks a power unit part by writing 1.0 into its failure value:

| Part | Address |
|---|---|
| ICE | engine `+0x254c` |
| TC | engine `+0x2550` |
| ES | engine `+0x2554` |
| MGU-H | engine `+0x2558` |
| MGU-K | engine `+0x255c` |
| CE | engine `+0x2560` |
| Gearbox | `car+0x538`, then `+0x674` |

The game's red flag repair (loop at `0x1435847da`) restores aero and body parts only, never wheels or suspension.

## Suspension and wheels

`F1SuspensionDamage` is at `car+0x5d0`.

- `0x1446568e0` returns 3 bits per corner, in the order RL, RR, FL, FR (corner `i` is `(packed >> 3*i) & 7`): 0 ok, 1 puncture, 2 broken suspension, 3 to 5 wheel detached.
- `0x144656a80(suspension_damage, 0)` reattaches and repairs.
- `0x144660cd0(car+0x5c8, 0)` repairs the front wing.

## Garage repairs

The garage's repair timer and **Skip Repair Time** read the RepairTimes manager. Its records span `+0x48` to `+0x50`, with a stride of `0x58`:

| Record offset | Field |
|---|---|
| `+0x10` | Active (u8) |
| `+0x16` | Vehicle index (u8) |
| `+0x20` to `+0x28` | Job list: each job is `{u32 kind, float seconds}` |

| Function | Address |
|---|---|
| Manager update, `(manager, float dt)` | `0x142bf50a0` |
| Job finished, `(record, u32 kind)` | `0x142bf03e0` |
| AddRepair thunk | `0x142bf0200` |

- Only the first job counts down. At 0 the manager calls "job finished" and erases the job.
- Kinds 3 to 16 reset the car's terminal damage record and the matching damage component when they finish. Kind 9 is an ICE replacement. Kind 16 is a detached wheel, which the game adds itself (`0x14e42a440`) only when a wheel part's `+0x38` flag is set.
- To add a job, call AddRepair, which deduplicates by kind, then set the job's seconds and `record+0x10 = 1`.
- To remove all jobs, set `record+0x28 = record+0x20`.

## Leaving the garage

The garage menu's view model update is at `0x141bc0940`, `(rcx view model, xmm1 dt, r8d)`. It writes every button's bool node each frame with `Vm_NodeSet_Bool` (`0x140973e20`). To turn a button off, call the original update first, then set the node to false with the same checks the update makes.

| View model offset | Field |
|---|---|
| `+0x2b0` | RepairTimes manager |
| `+0x33b` | Repairs pending; the game hides "go to track" while it's set |
| `+0x390` | `goto_track_visible` |
| `+0x3d0` | `drive_out_enabled` |
| `+0x3f0` | `flying_lap_enabled` |
| `+0x470` | `pp_flying_lap_enabled` |
| `+0x490` | `pp_drive_out_enabled` |

Refusing `MessageRequestVehicleExitGarage` (handler `0x142a41120`) on its own leaves the menu camera stuck in the car, so turn the buttons off and refuse the message only as a fallback.

`Game_Pitlane_SetVehicleStatus(pit_lane, car, 9)` delivers the pit state message synchronously, inside the call. A `session.pit` handler can't tell an addon's teleport from the car driving into its garage unless the addon sets its own flag around the call.

## Pausing

`on_frame` keeps running in the pause menu, with the real seconds since the last frame, and an addon that counts game time with `on_frame` keeps counting while the player is paused: subscribe to `game.pause` and `game.resume`, or poll `swofty::view::paused()` ([pause and resume](/reference/std/events#pause-and-resume)). `updateVehicle` above doesn't run for the player's car in the pause menu, so Terminal Damage counts time only while that update ran in the last 500 ms.

## Suspension damage layers

These `f1_suspension_damage.*` layers worked in a Montreal race on the player's car, through `SwoftyCarOps` version 2, alongside Heavier Damage 1.1.0 on Normal:

| Field | Op | Value | Priority |
|---|---|---|---|
| `wheel_detach_scale` | `scale` | 2 | 50 |
| `parts_detach_scale` | `scale` | 2 | 50 |
| `front_damage_scale` | `set` | 0.001 | 50 |
| `rear_damage_scale` | `set` | 0.0001 | 50 |

`get` returned the composed values at once, and 30 seconds later the car's own `F1SuspensionDamage` held them too. After a 23 m/s hit on the rear left, the game's damage on that corner was 0.31.

## What's still missing

[Issue #691](https://github.com/Swofty-Developments/SwoftyLauncher/issues/691) asks for these in the std library. None of them exists yet:

- `sessions::retire(car)` and `sessions::disqualify(car)`.
- A getter for the session rules object.
- A `failure.planned` event with the car, part and lap, and a veto.
- A way to change the failure roll without patching bytes.
- A car speed read that doesn't need UDP.
