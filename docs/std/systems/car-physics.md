---
title: Car physics and handling
description: How a car's handling is built from its handling file, every part you can change, when a change reaches the car, how to change a car live during a session, and how R&D, setup, damage and AI fit in.
---

# Car physics and handling

::: warning Experimental
Live car physics (`cars::live`, layers, `car.profile`) is experimental in 1.7.0. Nothing in it has run in game yet, and its shape can change before it is stable. A field whose game code has no independent verification yet stays off and logs `<field> off: needs <row>`.
:::

Every car on track is built from its **handling file**: the `.vtf` in the car's package (`teams/ferrari/handling/ferrari.vtf`). The file has one section per part of the car, such as Engine, F1Aero or F1WingDamage, and each section is a list of named values. Your addon changes those values in three ways:

- **Layer the handling record.** The change reaches every car built from that handling file, from the next session.
- **Change a car live.** Through `cars::live`, during a session, on one car or many, with the car's team and driver in hand.
- **Edit an upgrade block.** The R&D upgrades are blocks inside the same file, so changing one changes what R&D does.

A damage addon shows all three. It offers Light, Normal and Heavy damage in ADDON SETTINGS, and a change reaches the next race with no restart. A Live option goes further: it changes the cars on track at once.

```cpp
#include <swofty/registry.hpp>
#include <swofty/systems/cars.hpp>

using namespace swofty;

SWOFTY_REGISTER
{
    auto f1 = tagged<cars::Handling>(cars::handling.tag("c/f1_cars"));

    // Every F1 car built from now on: next session, every year.
    cars::handling.layer(f1, [](cars::Handling &h) {
        auto level = option<std::string>("wing_damage");          // "light", "normal", "heavy"
        if (level == "heavy") {
            h.wing_damage.speed_threshold = 2.0f;
            h.wing_damage.damage_scale = 0.000175f;
            h.wing_damage.full_detach_scale = 0.4f;
        }
    });
}
```

## The parts of a car

A built car has 23 parts, one per section of its handling file. `cars::Handling` has a member for each, named after the section, and each part has a field for every value in that section, named after the key. Here are the parts most addons change:

```cpp
namespace swofty::cars {

struct WingDamage {                        // F1WingDamage: the front wing
    float speed_threshold;                 // impacts below this speed do nothing (stock 3)
    float damage_scale;                    // damage per unit of impact (stock 0.0001)
    float chassis_damage_multiple;         // share of a body hit that reaches the wing (stock 0.3)
    float side_detach_scale;               // damage at which one side breaks off (stock 0.5)
    float full_detach_scale;               // damage at which the wing comes off (stock 1)
};

struct SuspensionDamage {                  // F1SuspensionDamage
    float speed_threshold;                 // stock 7
    float front_damage_scale;              // stock 0.001
    float rear_damage_scale;               // stock 0.0001
    float wheel_detach_scale;              // stock 1
    float parts_detach_scale;              // at least 1.5 x wheel_detach_scale: the game raises a lower value
    float linear_fracture_limit;           // stock 50
    float angular_fracture_limit;          // stock 30
};

struct Engine {                            // Engine
    Curve revs_power_spline;               // power in W against engine speed in rad/s (2025 Ferrari: 550 kW at 1191.5 rad/s)
    float idle_rotation_rate;              // rad/s
    float limiter_rotation_rate;           // rad/s
    float mass, inertia;
    float wear_ice_rate, wear_tc_rate, wear_mguh_rate, wear_ce_rate;   // power unit wear per second of use
    // the throttle maps, engine braking, temperatures and the rest of the section
};

struct FuelEcu {                           // FuelECU: one value per fuel mode (lean, standard, rich, max)
    std::array<float, 4> fuel_burn_scale, fuel_power_scale, fuel_revs_scale, temperature_scale;
};

struct Ers {                               // EnergyRecoverySystem
    float max_deploy_power;                // W (2025: 126.3 kW, 2026: 350 kW)
    float energy_store_capacity;           // J (4 MJ)
    float exhaust_power_coefficient, overtake_power_efficiency, max_brake_torque_mguk;
    std::array<ErsMode, 4> modes;          // none, medium, overtake, hotlap: deploy per gear
};

struct F1Aero {                            // F1Aero
    float front_coeff_lift_multiplier, rear_coeff_lift_multiplier;
    float body_drag_coeff_min, body_drag_coeff_max;
    Curve front_coeff_lift_vs_ride_height, rear_coeff_lift_vs_ride_height;
    // wing settings, DRS, slipstream, brake duct and radiator terms, ride-height clamps
};

struct Chassis {                           // Chassis
    float mass;                            // kg
    float moi_x, moi_y, moi_z;             // moments of inertia
    float com_y, com_z;                    // centre of mass
};

}
```

The other parts work the same way: `gearbox` (ratios R, N, 1st to 9th, final drive, shift times), `fuel_tank`, `brake_ecu` (brake torques, ABS and traction control curves), `steering`, `core_aero` (slipstreaming), `differential`, `suspension` (springs, dampers, anti-roll bars, camber and toe, and the tyre and brake records per axle), `turbo`, `tuning` (the setup ranges), `assists`, `plank`, `tyre_damage` and `body_damage`.

A key the handling file doesn't have keeps the game's default.

## When a change reaches the car

The game reads a handling file once when the file loads. It then builds each car from it when the car is put on track, which happens at every session load. A handling layer applies at that build, so:

- A layer reaches every car built after it, in every year, without a restart. Changing an option reaches the next session.
- A car that's already on track keeps its values until it's built again. Use `cars::live` to change it now.
- The game rebuilds a car's parts from the file during some events, such as handing a car from the player to the AI. Your layer is part of every rebuild, so it survives them.
- With Equal Performance on, every car is built from the base car's file, so a layer on the base car reaches all of them.

The core logs each build:

```text
[cars] built vehicle 0 (game/ferrari_2025, team game/ferrari, driver game/charles_leclerc): 23 parts, 3 layers
[cars] vehicle 0 handling: F1WingDamage /speed_threshold 3 -> 2 (william09leo.heavier-damage-realism)
```

## Changing a car live

`cars::live` gives you each car on track during a session: its index and the game's car. `car.control()` reads who drives it, human or AI, and its controller flags, fresh on every call (see [Who drives a car](./sessions#who-drives-a-car)), and `car.is_ai()` reads whether the AI drives it right now (the player's car too, while the AI has it).

Read and write a value with `get`, `set` and `scale`, by `Field` or by its Handling path. The core finds the car's part, decodes and re-encodes the game's scrambled values, and refuses a value outside the field's range with `Result::invalid`:

```cpp
#include <swofty/systems/cars.hpp>

using namespace swofty;
using cars::live::Field;

for (cars::live::Car &car : cars::live::cars()) {
    if (car.is_ai())
        continue;
    car.scale(Field::steering_lock, 1.2f);            // 20 % more steering lock, next frame
    car.set("fuel_tank.fuel_mass", 50.0f);            // kg, up to the tank's capacity
    car.set(Field::wing_full_detach_scale, 0.6f);     // the wing comes off sooner, next hit
}
```

A write lasts until the game rebuilds, resets or replays the car's parts. To keep a value, make a layer.

### Fields

```text
steering.max_steering_lock             rad, 0.01 .. 1.5708
gearbox.max_clutch_torque              multiple of engine torque, 0 .. 100
gearbox.final_drive_ratio              1 .. 20, rebuilds the ratio table
gearbox.gear_ratio_1 .. gear_ratio_9   0.1 .. 20, rebuilds the ratio table
differential.diff_on_throttle          0 .. 1, until the setup is applied again
differential.diff_off_throttle         0 .. 1
turbo.max_boost                        bar; no effect on a stock car
fuel_tank.fuel_mass                    kg, up to the tank's capacity
engine.vtf_limiter_rotation_rate       rad/s, 100 .. 2500, re-runs the derived limiter
f1_wing_damage.*                       speed_threshold, damage_scale, chassis_damage_multiple, side_detach_scale, full_detach_scale
f1_suspension_damage.*                 speed_threshold, front/rear_damage_scale, wheel/parts_detach_scale, linear/angular_fracture_limit

fuel_ecu.fuel_burn_scale.<mix>         0 .. 10, fuel flow of lean, standard, rich or max; a write re-runs the active mix
fuel_ecu.fuel_power_scale.<mix>        0 .. 4, engine power
fuel_ecu.fuel_revs_scale.<mix>         0.5 .. 2, divides the power spline's rev lookup and scales the limiter
fuel_ecu.temperature_scale.<mix>       0 .. 4

aero.lift_scale                        0 .. 10, multiplies the body lift, read every step
aero.drag_scale                        0 .. 10, multiplies the body drag
aero.slipstream_drag                   0 .. 1, the air density the body drag sees in a full tow (lower: a stronger tow); read every step
aero.slipstream_front_wing             0 .. 1, the same for the front wing
aero.slipstream_rear_wing              0 .. 1, the same for the rear wing
aero.slipstream_lift                   0 .. 1, the same for the body lift
aero.slipstream_car_width              0.1 .. 10, the width of the wake the car leaves
engine.power_scale                     0 .. 4, the engine drive torque, written to no game data
engine.limiter_offset_rpm              negative: the torque is cut that many rpm under the limiter
ai.grip_scale                          0 .. 4, the AI's table grip input, dry weather only
ai.blocking.fraction_of_follow_speed   0 .. 4, per car, layers only: the blocking evaluator's fraction once the race start is over (stock 0.975)
ai.blocking.fraction_of_follow_speed_at_race_start      0 .. 4, the same before it (stock 0.875)
ai.blocking.max_blocking_distance      0 .. 1000, per car, layers only: how far behind an attacker still counts as a threat, after the race start
ai.blocking.max_blocking_distance_at_race_start         0 .. 1000, the same before it
ai.<group>.<key>                       43 more driver config slots, see the table in AI
tyres.front.grip_scale                 off: the tyre solver never reads GripMultiplier
tyres.rear.grip_scale
chassis.ballast_kg                     -300 .. 600, added to the VTF mass, then the chassis reset (fuel mass drops out until the next fuel update)
brakes.bias                            0 .. 1, rewritten when a setup is applied
brakes.pressure                        0 .. 4
suspension.front.camber  .toe  .anti_roll  .ride_height_offset
suspension.rear.camber   .toe  .anti_roll  .ride_height_offset
```

`engine.power_scale`, `engine.limiter_offset_rpm` and `ai.grip_scale` are the core's own handlers: nothing is written to the car, and a car with no layer on them runs the game's code unchanged. The suspension fields are copied into the solver every frame, so a write reaches the car on the next frame.

The `ai.<group>.<key>` fields are the per-car AI driver config slots: `blocking`, `alongside`, `divebomb_defense`, `offset_variation`, `overtake` and `space_seeking`, 59 in all, listed with their kind and range in [AI](./ai#racecraft-per-car-live). They are per car and take layers only (`set` answers `Result::unavailable`). The core never writes the game's database cell: it points the car's slot in its AI record at an own cell holding the composed value, and puts the game's pointer back when the car's last layer on the field goes. The base of a layer is the game's value for that car. `get` returns the value the AI reads. The fields answer `Result::unavailable` until the AI system is built and has stopped growing, and a layer made earlier applies then. A session change forgets every swap and leaves the old session's records alone; the owned cells are not freed. The `_at_race_start` pair is read while the race start state machine is before its last state, the plain pair after it.

A curve field's own cell is a copy of the game's curve record made flat: step 0, an all-zero sample table, and min, last value and max set to the composed value, so every x evaluates to it. `get` on a curve field returns the curve at x = 0. `ai.space_seeking.car_in_front_needed` and `ai.space_seeking.total_queue_needed` are int cells: the composed value is rounded to the nearest integer. A cell whose stock value does not fit its kind (a denormal or non-finite float, an int outside 0 to 65536, a curve record with a non-finite scale, offset, step, last value, min or max) is refused and left alone. A field turns on once its `AiDriverConfig.<group>_<key>` KB row has an independent agree verify row; the 43 added with `alongside`, `divebomb_defense`, `offset_variation`, `overtake` and `space_seeking` have none yet.

A car's index is the game's vehicle id: dense from 0 in the order the cars were built, the safety car included. A `Car` from `cars()` also carries the game's car, and a call on it answers `Result::late` once that car is gone from its slot. `set` runs on the game's flow thread, so call it from a frame, setting or event handler; from another thread it answers `Result::late`. The values are read from Steam 1.0.141, Steam 1.0.145.2472 and the EA app's 1.0.145: on any other build of the game, the EA app's 1.0.141 among them, `cars::live::available()` is false and `cars()` is empty.

## Layers

A layer is a value the core keeps and applies again after every `car.built`, `car.reset`, R&D upgrade replay and setup apply, so it outlives the game's own resets. It is removed when the `Layer` is destroyed:

```cpp
using namespace swofty;
using cars::live::Cars;
using cars::live::Layer;

static Layer power, lift, fuel;

SWOFTY_REGISTER
{
    power = Layer::scale(Cars::all(), "engine.power_scale", 0.9725f, /*priority*/ 0);
    lift = Layer::scale(Cars::ai(), "aero.lift_scale", 0.9f, 10);
    fuel = Layer::set(Cars::player(), "fuel_tank.fuel_mass", 60.0f);
}
```

Layers of one field compose by priority, lowest first, then in the order they were made: `set` replaces the value, `scale` multiplies it and `add` adds to it. The base is the game's value after its last reset (1 for `engine.power_scale` and `ai.grip_scale`, 0 for `engine.limiter_offset_rpm`). A composed value outside the field's range is clamped.

`Cars::all()` reaches every car, those built later too. `Cars::ai()`, `Cars::player()` and `Cars::car(car)` name the cars on track when the selector is made. `Layer::result()` is `Result::unavailable` when the field is off.

The C table (`SwoftyCarOps` version 2) has the same calls:

```c
const SwoftyCarOps *ops = SwoftyCoreCarOps();
uint32_t field = 0, token = 0;
ops->field("aero.drag_scale", &field);
ops->layer("my.addon", SWOFTY_CAR_ALL, field, SWOFTY_CAR_OP_SCALE, 0.96f, 0, &token);
ops->unlayer(token);
```

## Car profiles

`car.profile` records put layers on the cars a scope matches, from data, with no code. The core reads them at `car.built`, `car.reset`, the upgrade replay and whenever the session changes, drops its earlier profile layers for the car and adds the matching profiles' again. A career freezes the season's profiles at season start. A box names none: a profile's `within` selects the box, as a `rules.drs` record does.

```json
{
  "car.profile": {
    "race_engine": {
      "within": [{ "data_group": "f1_career_1990" }],
      "when": ["practice", "race"],
      "power_scale": 0.9725,
      "limiter_offset_rpm": -300,
      "ai_grip_scale": 0.87
    }
  }
}
```

That record is all of F1 1990's two native DLLs: the race engine (power x0.9725 and a cut 300 rpm under the limiter outside qualifying) and the AI's grip (x0.87 outside qualifying, in the dry).

```cpp
namespace swofty::cars {
struct Profile {
    json::Value within;                         // selector array: content, data_group, setup_mode, vehicle_package, ...
    std::vector<events::SessionType> when;      // empty: every session
    CarSelect cars;                             // indices; vehicle_package, vehicle and team wait for the car identity
    bool ai_only = false, player_only = false;
    int priority = 0;
    float power_scale = 1, limiter_offset_rpm = 0, lift_scale = 1, drag_scale = 1;
    float grip_scale = 1, ai_grip_scale = 1;
    std::optional<uint32_t> fuel_modes;         // off until the fuel mode call is verified
    json::Value extra;                          // every other key is a live field path
};
}
```

Every key the record does not name is a live field path. A number sets it; an object with `set`, `scale` or `add` composes:

```json
{
  "car.profile": {
    "heavy_aero_cars": {
      "when": ["race"],
      "ai_only": true,
      "aero.drag_scale": { "scale": 1.05 },
      "brakes.bias": 0.56,
      "chassis.ballast_kg": { "add": 12 }
    }
  }
}
```

Profiles compose by priority, then by record id. A profile whose `cars` names a vehicle package, vehicle or team is skipped and logged until the car's identity is mapped.

## Events

```cpp
events::on<cars::events::Built>().after([](const cars::events::Built &e, const events::Context &) {});
events::on<cars::events::Reset>().after([](const cars::events::Reset &e, const events::Context &) {});
events::on<cars::events::UpgradesReplayed>().after([](const cars::events::UpgradesReplayed &e, const events::Context &) {
    float progress = e.progress;                // the development progress the block was replayed at
});
events::on<cars::events::SetupApplied>().after([](const cars::events::SetupApplied &e, const events::Context &) {});

events::on<cars::events::PartsRegister>().after([](const cars::events::PartsRegister &e, const events::Context &) {
    e.add(cars::Part{.node = "frontwing_fl_03", .detach_at = 0.9f});     // one more piece of the front wing
});

events::on<cars::events::Damage>().after([](const cars::events::Damage &e, const events::Context &) {
    if (e.part == cars::events::Damage::front_wing_left)
        log("car %u lost %.2f of its left front wing", e.car.index(), e.amount);
});

events::on<sessions::events::Pit>().after([](const sessions::events::Pit &e, const events::Context &) {
    log("car %u: pit status %u -> %u", e.car, e.previous, e.status);
});
events::on<sessions::events::PitStop>().after([](const sessions::events::PitStop &e, const events::Context &) {
    log("car %u has made %u stop(s)", e.car, e.stops);
});
```

- `car.built` fires once for every car the game builds: at session load and when a restart builds the cars again.
- `car.reset` fires when the game re-reads a car's parts without building it again: during loading (twice for every car) and when it hands a car to the AI or back (T120).
- `car.upgrades.replayed` fires after the game replays an R&D upgrade block onto a car. The replay resets `aero.lift_scale`, `aero.drag_scale` and the fuel mode.
- `car.setup.applied` fires after a setup is written to the car: the differential, suspension, brakes and centre of mass.
- `car.parts.register` fires after the game has registered the front wing's own 11 pieces, once per car (not on a reset or an upgrade replay). `add` registers one more piece on that wing: `node` is the model node, `detach_at` the multiplier `k` the piece comes off at (it comes off when `k` times the wing's full detach scale is at most the left and right damage together), and a node the car's model doesn't have adds nothing. `debris` is stored, not applied.
- `car.damage` fires after the front wing takes a hit that adds damage: `part` is `front_wing_left` or `front_wing_right` and `amount` the damage the hit added to that side. It fires on the next frame, like the events above.
- `session.pit` fires when a car's pit status changes, with the status it had and the one the game stored. `session.pit_stop` fires on the change where the game adds one to the car's stop count. A stop's stationary time and tyre change are off: they need a row for the PitStopInstance's car.

The game builds cars on a loading thread, so the core holds each event and fires it on the next frame, on the flow thread, where `set` works. The core applies its layers and profiles to the car first, so a handler sees the layered values. A car already gone from its slot by then gets none.

`Reset::upgrade` is always false from the core; the upgrade replay is its own event.

A suspension hit has no event yet. `swofty/game.hpp` has the experimental binding `Car_SuspensionDamage_OnCollision(susp_damage, contact)`, which the game calls for every impact on a car: `susp_damage` is the car's F1SuspensionDamage system and `contact` the game's contact record, whose first field is the car that was hit. The function reads the speed threshold and the front and rear damage scales on every hit, so a change made before `orig` applies to that hit. The game calls it with the car's collision lock held, so keep the handler short.

```cpp
#include <swofty/game.hpp>
#include <swofty/mixin.hpp>

#include <atomic>

std::atomic<uint32_t> suspension_hits[32];

SWOFTY_WRAP(Car_SuspensionDamage_OnCollision, void, void *susp_damage, void *contact)
{
    auto *car = *static_cast<swofty::game::CarLive **>(contact);  // the car that was hit
    suspension_hits[car->vehicle_id % 32]++;
    orig(susp_damage, contact);                                    // the game adds this hit's damage
}
```

## R&D upgrades

An R&D upgrade isn't a number in an array. Each upgrade type is a block inside the handling file, such as `radUpgradeEngine` or `radUpgradeFrontDownforce`, with the same sections and keys as the rest of the file. When the career fits an upgrade, the game reads the block on top of the car's values, scaled by the development progress `p` from 0 to 1:

- `*`: the value is multiplied by `(v - 1) * p + 1`.
- `+` and `-`: `v * p` is added or taken away.
- `/`: the value is divided by `(v - 1) * p + 1`.
- No operator, or `=`: the value is set to `v`, whatever the progress.

A block applies once per car build. The stock 2025 blocks change, among others:

```text
radUpgradeEngine            Engine/RevsPowerSpline +, FuelTank/FuelBsfcMin x0.9
radUpgradeFrontDownforce    F1Aero/FrontCoeffLiftMultiplier x1.26, FrontWingCoL1 x1.1
radUpgradeDrag              F1Aero/BodyDragCoeffMin x0.87, BodyDragCoeffMax x0.89
radUpgradeWeightReduction   Chassis/Mass -18
radUpgradeTyreWear          Suspension Tyre/UpgradeWearFactor x0.825
radUpgradeWearICE           Engine/WearICERate x0.5
```

A block is a list of changes, each a section, a key, an operator and a value:

```cpp
namespace swofty::cars {
struct UpgradeChange { std::string section, key; char op; float value; };   // op: '=', '+', '-' or '*'
struct RadUpgrade { std::string name; std::vector<UpgradeChange> changes; };
}
```

Edit a block with the same layer, or add a new one for a department of your own:

```cpp
cars::handling.layer(f1, [](cars::Handling &h) {
    cars::RadUpgrade &engine = h.upgrade("radUpgradeEngine");        // the block, created if it's missing
    engine.set("FuelTank", "FuelBsfcMin", '*', 0.85f);               // a bigger fuel saving at full progress
    engine.set("Engine", "WearICERate", '*', 0.8f);
});
```

## Setup

The setup screen's sliders pick a point between a minimum and a maximum from the `tuning` part, then write it into the differential, suspension, brakes, engine braking, centre of mass and fuel load. The wing sliders are the exception: their range is the wing's own setting count in `f1_aero`. Widen or move a range with a layer:

```cpp
cars::handling.layer(f1, [](cars::Handling &h) {
    h.tuning.differential_on_throttle_multiple_min = 0.4f;   // a more open differential at the low end
    h.tuning.brake_bias_max = 0.7f;
});
```

## AI cars

AI cars use the same handling file, the same physics and the same R&D upgrades as the player. The difficulty setting changes how the AI drives, not its car: there's no hidden grip, power or drag boost. The AI-only values are in the handling file, so you can change them like any other:

- `brake_ecu`: the AI's stability control (`asc_strength_for_ai`, off in 2025) and `brake_migration_torque_ai`.
- `suspension` tyres: the AI's slip limits and grip factors on a wet or fast tyre.
- An extra tyre wear factor the player's car doesn't have.
- AI cars run their tyre contact every third physics step, where the player's runs every step.

For how the AI drives, see [AI](/std/systems/ai).

## Readiness

The parts, their fields and how they're stored are mapped from the game's code and checked independently. An in-game trace confirmed that cars are built once at session load and torn down when it ends, that the game rebuilds every car twice while a session loads and rebuilds one car when control of it changes, and the live values of engine speed, fuel, ERS, tyre wear, mass and the aero coefficients. Still to confirm in game: DRS, the fuel-mix and setup controls, career R&D upgrades, and how a car's index matches its participant. The `cars::live` fields are mapped from the game's code and checked on fake cars in the core's host test; none of them is tested in game yet. Their offsets are checked for Steam 1.0.141, Steam 1.0.145.2472 and the EA app's 1.0.145: on any other build, a game patch included, `cars::live` is off.

These fields are on; each rests on a verified row:

```text
engine.power_scale, engine.limiter_offset_rpm, ai.grip_scale
aero.lift_scale, aero.drag_scale, aero.slipstream_*
brakes.bias, brakes.pressure
suspension.front/rear.camber, toe, anti_roll, ride_height_offset
chassis.ballast_kg
car.parts.register, car.upgrades.replayed, car.setup.applied
```

A ballast change rebuilds the car's mass without the fuel; the fuel mass comes back at the next fuel update.

These stay off:

```text
tyres.*.grip_scale     the tyre solver (Car_Suspension_SolveWheels) never reads CarSuspTyreType.grip_multiplier;
                       only the AI snapshot (0x1405ee660) and force feedback do, so a write changes neither the car's grip nor the lap time
refit_tyres            returns unavailable until a car-to-tyre-component lookup is bound
```

Credit: the front wing damage, suspension damage, fuel ECU and ERS layouts were cross-checked against findings shared by Will (william09leo), the author of Heavier Damage.

The author of Terminal Damage reports four `f1_suspension_damage.*` layers working in game; see [Suspension damage layers](/std/systems/failures#suspension-damage-layers). That report isn't an independent verification.

## Fuel mixes

Each car's FuelECU holds four values per mix (lean, standard, rich, max): fuel flow, engine power, a rev factor and a temperature factor. The `fuel_ecu.*` fields above write one value; the core then runs the car's active mix again so the engine and the fuel flow follow. Which mixes the MFD offers is the game's own: a mix is selectable while its availability entry is 0.

```cpp
using namespace swofty;

static cars::live::Layer rich_power = cars::live::Layer::set(cars::live::Cars::all(), "fuel_ecu.fuel_power_scale.rich", 1.05f);

SWOFTY_ON_LOAD
{
    cars::live::unlock_fuel_mixes("my.addon", uint32_t(cars::live::FuelMix::rich));
}
```

`set_fuel_mix(car, mix)` switches a car to a mix through the game's own mode switch (the one the core uses after a `fuel_ecu.*` write). It answers `Result::invalid` for a mix the MFD does not offer the car, `Result::late` off the flow thread and `Result::unavailable` on a core older than car ops version 4; `set_fuel_mix_available()` tells ahead. The game resets the mix to Standard at a qualifying lap start; re-apply the player's choice on the lap event:

```cpp
using namespace swofty;

static std::optional<cars::live::FuelMix> chosen;

SWOFTY_ON_LOAD
{
    events::on<sessions::events::Lap>().after([](const sessions::events::Lap &l, const events::Context &) {
        if (chosen)
            cars::live::set_fuel_mix(l.car.index(), *chosen);
    });
}
```

A change the game makes cannot be told from the player's: the core has no mix-changed event, so the addon records the mix it wants (here `chosen`, taken from `fuel_mix(car)` while the player drives) and re-applies it.

`unlock_fuel_mixes` clears the availability entries of the mixes in the mask for every car, every frame, until the mask is 0 again. `fuel_mix(car)` reads a car's current mix. The availability table (`g_EngineModeAvail`, KB notes in car.md) has no independent verify row; the core reads it through checked memory and answers `Result::unavailable` on a build without checked car layouts.

## Handling records

::: warning Experimental
Applying `vehicle.handling` records at build is experimental (launcher 1.7.0). The records, the typed sections and the launcher's build step can change before they are stable.
:::

The launcher applies the `vehicle.handling` records in an addon's data files to the handling files the build ships. The change is in the file before the game starts, so every car built from it has it, in every session, with no code running in game. A GearBox change, a removed ERS and a raised suspension in one file:

```json
{
  "vehicle.handling": {
    "ferrari_1990": {
      "vtf": { "stock": "2025_asset_groups/f1_2026_vehicle_package/teams/ferrari/wep/ferrari.erp" },

      "gearbox": {
        "final_drive_ratio": 3.9,
        "ratios": [3.0, 2.2, 1.7, 1.3, 1.0, 0.8],
        "up_speed_multiple": [0.8, 0.9, 0.95, 0.96, 0.96],
        "down_speed_multiple": [0.6, 0.7, 0.8, 0.85, 0.9]
      },

      "ers": null,

      "suspension": {
        "front": { "rest_length": 0.083, "min_height": 0.01, "bump_stop_height": 0.035 },
        "rear":  { "rest_length": 0.2,   "min_height": 0.01, "bump_stop_height": 0.045 }
      },
      "edits": [
        { "path": "suspension.front.anti_roll_constant", "op": "scale", "value": 0.85 },
        { "path": "suspension.rear.anti_roll_constant",  "op": "scale", "value": 1.45 },
        { "path": "chassis.mass", "op": "set", "value": 620 }
      ],

      "chassis": { "ffb_scale": 1.3, "tyre_physics_group": "F1_1990" },
      "f1_aero": { "drs": { "hardware": false }, "slipstream": { "drag": 0.65, "front_wing": 0.6 } },
      "turbo": { "mode": "off" }
    }
  }
}
```

```text
handling swofty.f1-1990/ferrari_1990 -> ferrari.vtf: 112 change(s)
handling ferrari.vtf: GearBox: 6 gears, ratios past the top gear 0, 4 shift-speed multiple(s) past it removed
handling ferrari.vtf: GearBox: AllowGearRatioTuning false, so the car drives these ratios and not the setup's
handling ferrari.vtf: EnergyRecoverySystem: section removed, 3 part(s) dropped from the radUpgrade blocks
handling ferrari.vtf: SuspensionSystem/RestLength: 0.1 -> 0.2
handling ferrari.vtf: Chassis/Mass: 771 -> 620
```

The build logs each change per handling file in the build log. A key or section the file doesn't have is a warning on the build and nothing else.

### Which file a record changes

The record's `vtf` names the archive. Every VTF in it is changed, or only the ones `vtf.resources` lists.

```json
{ "vtf": { "stock": "2025_asset_groups/f1_2026_vehicle_package/teams/ferrari/wep/ferrari.erp" } }
{ "vtf": "cars/handling/ferrari_handling.erp" }
{ "vtf": { "erp": "cars/handling/ferrari_handling.erp", "resources": ["ferrari.vtf"] } }
```

- `stock`: the game's archive. The build ships a changed copy.
- A path in your addon: the archive your `file.put` op puts in the game folder. The build changes the copy it puts.
- A record that names no `vtf` changes nothing and the build warns.

Layers apply in load order as a merge patch: `"extends"` on your own records, a key with a `/` for a layer on another addon's record, and `"$select"` entries with `ids`, `where` and `when`. `when` reads the addon's options at build, so an option changes the file at the next Play. `tag` and `in` entries are skipped at build. Layers written in native code run in game, after the build, and don't reach the file.

A vehicle record with a `tyre_group` and a `handling` sets `chassis.tyre_physics_group` on that handling record when the record doesn't set it.

### Equal performance

With equal performance on, the game takes every section of a car's handling from its base car, not from the team file. The car layer names the base in `CarPhysics2.baseVtfAsset`. A layer without one, such as every F1 25 2025 car, gets the game's F1 2025 base car.

An addon whose records name team files in their own package (`eaid://sw_2013_vehicle_package/teams/redbull/handling/redbull.vtf`) and that ships that package's base, `teams/fom_car/handling/fom_car.vtf`, gets two more changes at build:

- The base car takes each value most of the package's records share, so equal performance drives one car of the season.
- Every `-carmodel.layer` in the records' archives that has a `vtfAsset` and no `baseVtfAsset` gets `baseVtfAsset` set to that base.

An addon that ships no base, such as a livery pack for a season, uses the base of the addon that ships it. A record that names the base file itself sets it; the build leaves it as the record sets it.

```text
handling swofty.swofty-seasons-2013/sw_2013_vehicle_package equal performance -> fom_car.vtf: 31 change(s)
handling mclaren-carmodel.layer: CarPhysics2/baseVtfAsset: eaid://sw_2013_vehicle_package/teams/fom_car/handling/fom_car.vtf, so equal performance drives the season's base car
```

### Typed sections

Each field maps to the VTF element of the same name in snake_case: `front_coeff_lift_multiplier` is `FrontCoeffLiftMultiplier`. The fields whose element is named differently:

```text
gearbox.final_drive_ratio       GearBox/FinalDriveRatio
gearbox.ratios                  GearRatio1st .. 9th
gearbox.up_speed_multiple       GearUpSpeedMultiple1st ..
gearbox.down_speed_multiple     GearDnSpeedMultiple2nd ..
turbo.mode                      Engine/TurboMode (off, full)
turbo.max_boost                 Turbo/MaxBoost
differential.drive_type         Differential/DriveType (rwd, fwd, awd = 4WD)
suspension.front.rest_x         SuspensionSystem/Suspension name=Front/LocalRestPositionX
suspension.front.tyre_radius    SuspensionSystem/Tyre name=Front/TyreRadius
suspension.front.grip_multiplier   SuspensionSystem/Tyre name=Front/GripMultiplier
suspension.front_tyre.<key>     any key of the Tyre element, front_brake.<key> of the Brake element
f1_aero.slipstream.drag         F1Aero/m_aeroSlipStreamAirDensityScaleDrag
f1_aero.slipstream.car_width    F1Aero/m_aeroSlipStreamCarWidth
f1_aero.drs.angle               F1Aero/DRSRotationAngleRadians
ers.max_deploy_power            EnergyRecoverySystem/Common/MaxDeployPower
tuning.chassis_com_z            F1Tuning/ChassisCOMZMin and ChassisCOMZMax
```

A section holds any other key of its VTF section too: `engine.wear_ice_rate`, `fuel_tank.fuel_bsfc_min`, `steering.max_steering_lock`. A curve is an array of `[x, y]` pairs and replaces the spline's vertices:

```json
{ "engine": { "limiter_rotation_rate": 1180, "revs_power_spline": [[0, 0], [314.2, 34562], [1151.9, 415319], [1570.8, 285922]] } }
```

`tuning` fields are `Range` pairs, `{ "min": 0.1, "max": 1.0 }`. `differential_on_throttle_multiple_min` and `brake_bias_max` from 1.6.0 still work.

### Gear count

`gearbox.ratios.size()` is the gear count. The build writes the ratios, writes 0 for the gears past the top one and removes the shift-speed multiples past it, the layout of the EA F2 car. `up_speed_multiple` and `down_speed_multiple` hold one value per shift: up for gears 1 to n-1, down for gears 2 to n.

A record that sets `ratios` or `final_drive_ratio` also sets `AllowGearRatioTuning` to false in the GearBox section, as the EA F2 car does. While it is true, the game's default for F1 cars, the gearbox takes its ratios and final drive from the car setup and ignores the handling file's.

```json
{ "gearbox": { "ratios": [3.1, 2.3, 1.8, 1.4, 1.1, 0.9] } }
```

### Removing a section

`"ers": null` removes the EnergyRecoverySystem section and the ERS parts of every radUpgrade block. The blocks stay, empty. In a layer, `ers` is the one field where `null` removes the section instead of removing the field from the record. `std::optional<Ers> ers` is `nullopt` in code for the same.

`remove` takes any section name and drops its parts from the radUpgrade blocks the same way:

```json
{ "remove": ["turbo", "ers"] }
```

`f1_aero.drs.hardware: false` removes the DRS keys from F1Aero, sets the flap angle, open speed and the DRS wing factors to the safety car's values and empties `radUpgradeDRS`.

### Set, scale and add

`edits` run after the typed sections. `path` is `section.key`, or `section.front.key` for an axle. `op` is `set`, `scale` or `add`:

```json
{ "edits": [
  { "path": "fuel_tank.fuel_bsfc_min", "op": "scale", "value": 0.9 },
  { "path": "suspension.rear.rest_length", "op": "add", "value": 0.03 },
  { "path": "brake_ecu.brake_front", "op": "scale", "value": 0.74 }
] }
```

`rad_upgrades` records change the radUpgrade blocks the same way: `{ "name": "radUpgradeEngine", "changes": [{ "section": "fuel_tank", "key": "fuel_bsfc_min", "op": "*", "value": 0.9 }] }`.

### The centre of mass

The centre of mass is at `-com_z`. The game's static axle loads put `(-com_z - rear_z) / wheelbase` of the weight on the front axle, so the stock 2026 `com_z` of +0.171 on axles at +1.7 and -1.7 m is 45 % on the front. `cars::front_weight(handling)` reads it from the record:

```cpp
cars::Handling h;
h.suspension.front.rest_z = 1.7f;
h.suspension.rear.rest_z = -1.7f;
h.chassis.com_z = 0.171f;
float front = cars::front_weight(h);      // 0.4497
```

For 41 % on the front axle, `-com_z = 0.41 * wheelbase + rear_z`, so on those axles `com_z = -(0.41 * 3.4 - 1.7) = 0.306`.

### Vehicles

A vehicle record sets three more things:

```json
{
  "vehicle": {
    "ferrari_1990": {
      "donor": "game/ferrari_2025",
      "handling": "swofty.f1-1990/ferrari_1990",
      "drs_capable": false,
      "engine": "game/cosworth_f1_v8",
      "tyre_group": "F1_1990"
    }
  }
}
```

- `drs_capable` is written into the vehicle row at boot. Absent keeps the donor's.
- `engine` is the vehicle row's engine, an `entrants.engines` label. A `power::Supplier` record (`power.supplier`) can name its donor and limiter; the core writes no engine row from it yet, so rev lights stay the donor's.
- `tyre_group` is the Chassis `TyrePhysicsGroup` of the car's handling file, one group per team. The group's compounds are the tyre files' work.

### Tyres

`swofty/systems/tyres.hpp` holds the first slice, as records the core stores and applies nothing from: `tyre.physical` (a compound with its `group`, `code`, class and grip factors), `tyre.visual`, `tyre.class`, `tyre.allocation` and `tyre.rules`. What reaches the game today is the group above.

```cpp
tyres::physical.add("f1_1990_soft", tyres::Physical{ .group = "F1_1990", .code = "c5", .race_grip = 0.96f });
```

### What isn't applied

- Native-code layers on `vehicle.handling` reach the game after the build.
- Fields the 2026 stock file has no element for: `fuel_tank.fuel_mass`, `ers.exhaust_power_coefficient`, `ers.overtake_power_efficiency`, `turbo.mode: lag_spoof`. The build warns.
- `ers.modes`, `damage_scale`, `class` and the radUpgrade splines.
- The ConvexHull vertices.
- Archives written by `erp.*` ops. Put the archive with `file.put`.

## Template

`templates/std/car-physics` is a working addon with all three layers: a `car.profile` record (F1 1990's race engine and AI grip), a `vehicle.handling` record (six gears, no ERS) and live layers made on `car.built`, with a `car.parts.register` handler. Its test runs on the host against a fake car table:

```cpp
static Layer lift, ballast;

void make_layers()
{
    if (!lift.ok())
        lift = Layer::scale(Cars::all(), "aero.lift_scale", 0.95f, 0);
    if (!ballast.ok())
        ballast = Layer::add(Cars::player(), "chassis.ballast_kg", 12.0f, 0);
}

SWOFTY_REGISTER
{
    events::on<cars::events::Built>().after([](const cars::events::Built &, const events::Context &) { make_layers(); });
}
```
