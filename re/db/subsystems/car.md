# car: per-car performance / setup / physics model

Owner: see `subsystems.tsv` (car-re). Scope: the live car object (`Car`) and its 23 VTF systems, the VTF load and reset
pipeline, the XOR-obfuscated values, the physics step, every system's data and runtime fields, the R&D upgrade replay,
setup, assists, damage, and the AI-only physics data. Steam build base 0x140000000, sha256 d873a1c8….

The 2026-10-28 campaign (lanes car-a object/lifecycle, car-b powertrain, car-c chassis/aero/suspension/tyres, car-d
performance/setup/assists/damage/AI) wrote this map. Struct files: `CarLive.h` and the `Car*.h` files it points at.
Real values come from the 2025 and 2026 Ferrari handling files. The copy several lanes used holds four extra upgrade
blocks from the R&D Tyres addon (`radUpgradeTyreThermal`, `radUpgradeBrakeCooling`, `radUpgradeTyreLife`,
`radUpgradePUCooling`). Everything else in it matches the stock `teams/ferrari/wep/ferrari.erp` (sha256 10883ee0…,
checked by the lead); the stock 2025 Ferrari has 18 upgrade blocks.

## Summary {#summary}

- **The car.** `CarLive` (C++ `Car`, 0x9f60 bytes, no vtable) is built once per car spawn by `Car_Chassis_Build`. It
  holds 23 systems (Engine, GearBox, FuelTank, FuelECU, BrakeECU, Chassis, Steering, CoreAero, F1Aero, Differential,
  SuspensionSystem, DebugSystem, F1Tuning, F1Assists, Turbo, EnergyRecoverySystem, ConvexHull, WheelProxies, F1Plank,
  F1WingDamage, F1SuspensionDamage, F1TyreDamage, F1BodyDamage) cached at +0x530..+0x5e0. Each system is
  `{vtable, CarLive *car at +8, allocator, data object at +0x18}` (ConvexHull keeps it at +0x20: fetch it through the
  system vt+0x38); struct offsets are reader offsets + 0x18.
- **When the VTF is read.** The `.vtf` resource is parsed once per load. Each car reads its sections once, at create.
  A reset path re-reads every section into the same systems and drops the applied upgrades (controller change,
  resettle/teleport). R&D upgrades are `radUpgrade<Type>` blocks replayed through the same readers, once per block per
  car build, scaled by progress; the game re-inits every system after each block.
- **Obfuscation.** Most handling numbers are `CarObfFloat`: two u32s 8 bytes apart, value bits = a ^ key, with a
  random 64-bit key per field per car (MT19937). Write `a = bits ^ key`. Suspension, tyre and brake records (except six Debris* XOR pairs per
  Tyre record and seven thermal XOR pairs per Brake record), ERS, Turbo, fuel mass, F1Tuning ranges and several aero
  runtime fields are plain floats.
- **Derived values.** Max torque and the limiter (Engine Init), gear tables (GearBox Reset 0x220), body drag and wing
  coefficients (F1Aero OnReset 0x80, 0x80|0x2200), mass/COM/MOI (Chassis OnReset 0x200) and brake torques per wheel
  (`Car_BrakeEcu_ApplyBias`) are derived. Edit the raw field, then re-run the owner.
- **Live knobs.** F1Aero `lift_scale` +0x19a4 and `drag_scale` +0x19a0 (plain, 1.0, applied every step), fuel mode
  (`Car_FuelEcu_SetMode_Thunk`), fuel mass (FuelTank +0x100), ERS mode (+0x1a68), tyre `UpgradeWearFactor`, the
  setup targets written by `Car_Setup_ApplyTuning`.
- **Aero outputs.** F1Aero +0x19f0 / +0x19ec are the front / rear body lift coefficients (`front_lift_coeff` /
  `rear_lift_coeff`: 0.72 / 0.96 parked, 0.79 / 1.11 at speed), not forces. +0x19e8 `total_drag` is the drag force
  in N (-2402 at speed). Seen live in T115 (#runtime).
- **Runtime (2026-10-28).** In a Grand Prix every car is built once at load (21 incl. the safety car), the whole
  grid is reset twice during loading, the player's car is reset again whenever its controller changes, and every car
  is destroyed at session end. `is_ai` (+0x124) only selects the AI physics rate; its meaning "AI-controlled right now" is NOT confirmed (see the is_ai note under #runtime). See #runtime.
- **AI.** No difficulty-keyed grip, power or drag multiplier exists. AI-only physics is data: BrakeECU ASC*AI,
  `BrakeMigrationTorqueAI`, tyre *AI downslope and slip limits, an AI tyre-wear multiplier (CarTyreState +0x84),
  and slower substep/tyre-contact rates (`Vehicle.FineUpdateRateAI`, `TyreContactSubStepCycleAI`).
- **Open (L3).** T112–T125 (see #tests); the 2026-10-28 run passed T113, T118–T120, T122 and T124, answered part of
  T112, T115, T117, T121 and T125, and deferred T116 and T123 (#runtime).

## The objects {#objects}

| object | size | where it lives | made by |
|---|---|---|---|
| `CarVtfAsset` (vt `0x145c04a30`) | 0x50 | one per loaded `.vtf` resource, shared by every car built from it | `Car_Load_Vtf` 0x14475a250 (resource loader slot `0x145c04c70`) |
| `CarPhysicsRecord` (vt `0x145bf73a0`, chassis-runtime entity component) | ≥0xa08 | the vehicle entity; `[veh+0x20]` from the manager list | entity system |
| `CarLive` (cxx `Car`, no vtable) | 0x9f60 | `CarPhysicsRecord+0x58`, `g_Sess_VehicleTable[vehicle_id]` | `Car_Chassis_Build` 0x144628190 (record vt slot 7) |
| systems (one per VTF section) | per type (Engine 0x25b8) | `CarLive+0x3a0[count +0x398]`, cached by name at `+0x530..+0x5e0` | `Car_Create_<X>System`, via `Car_Systems_FindOrCreate` |
| `CarDynamics` (vt `0x145bff4d0`) | 0xa80 | `CarLive+0x900`, `+0x18` = CarLive | `Car_Chassis_Build` |

`CarPhysicsRecord` (the chassis-runtime component, vt 0x145bf73a0) also sits at `[upgrade_component+0x18]`. `CarWheelState` (old name, `CarPhysicsRecord+0x58`) is CarLive: its `+0xe8` is `vehicle_id`, not a tyre-set index, and its `+0x580` is the SuspensionSystem system. The lead should fold `CarWheelState.h` into `CarLive.h`.

Each system: `+0 vtable`, `+8 CarLive*`, `+0x10 allocator`, `+0x18` the section data object with its own 4-slot vtable `{dtor, GetName, Car_Read_<X>(node), Car_Write_<X>(node)}`. Field offsets in the lane structs are from the system pointer stored in CarLive; reader offsets are 0x18 less. Note: the first CarLive.h I wrote (11:42) said the car pointer is at system+0x18; it is at **system+0x8** (CarDynamics is the one with +0x18).

System vtable (same layout for all 23, `vt_Car_EngineSystem`): `+0 dtor`, `+8 Read(node)` (forwards to data vt+0x10), `+0x10 Write(node)`, `+0x18 OnReset(flags)` (Engine: `Car_Tuning_SetEngineBrakingAndRadiator`, Chassis: `Car_Tuning_SetChassisZCom`, GearBox: `Car_Tuning_SetFinalDrive`; called with -1 once at create = apply every setup value), `+0x20 ?`, `+0x28 GetPhaseCallback(out {fn,ctx}, phase 0..6)`, `+0x30 Init(0)` (F1WingDamage: `Car_FrontWing_Init`), `+0x38/+0x40 GetData` (= this+0x18).

| section | system vt | data vt | reader (data vt+0x10) | writer (+0x18) |
|---|---|---|---|---|
| Engine | 0x145bfa630 | 0x145bfa1b0 | Car_Read_Engine 0x144685680 | Car_Write_Engine 0x144686830 |
| GearBox | 0x145bfb250 | 0x145bfa9e0 | Car_Read_GearBox 0x1446a39a0 | 0x1446a2610 |
| FuelTank | 0x145bfa980 | 0x145bfa8c0 | 0x14468ea90 | 0x14468e760 |
| FuelECU | 0x145bfa870 (car-b) | 0x145bfa6f0 | see car-b | Car_Write_FuelECU 0x144689bf0 |
| BrakeECU | 0x145bf9bb0 | 0x145bf9730 | 0x1446752f0 | 0x144674a30 |
| Chassis | 0x145bf9ca0 | 0x145bf9c10 | 0x144676730 | 0x144676460 |
| Steering | 0x145bfb400 | 0x145bfb310 | 0x1446a7070 | 0x1446a6e00 |
| CoreAero | 0x145bf9f20 | 0x145bf9eb0 | 0x14467d080 | 0x14467cfd0 |
| Differential | 0x145bfa120 | 0x145bfa040 | 0x14467ddf0 | 0x14467daa0 |
| SuspensionSystem | 0x145bfbab0 | 0x145bfba10 | 0x1446aa090 | 0x1446a8870 |
| Turbo | 0x145bfbe80 | 0x145bfbdc0 | 0x1446b6580 | 0x1446b6630 |
| WheelProxies | 0x145bf8950 | 0x145bf88e0 | 0x14464e710 | 0x14464e630 |
| ConvexHull | 0x145bf8760 | 0x145bf8620 | 0x14464aaa0 | 0x14464a7f0 |
| F1Aero | 0x145bfca70 | 0x145bfc370 | 0x1446c8560 | 0x1446c73f0 |
| F1Assists | 0x145bfd380 | 0x145bfcbe0 | 0x1446cb880 (KB name Car_PadAssist_LoadSteeringSplines; it is the F1Assists reader) | 0x1446cc750 |
| F1Tuning | 0x145bfefb0 | 0x145bfe030 | 0x1446d1660 | 0x1446d4470 |
| F1Plank | 0x145bf8aa0 | 0x145bf8a50 | 0x144652aa0 | 0x144652930 |
| F1SuspensionDamage | 0x145bf8bd0 | 0x145bf8af0 | 0x1446533e0 | 0x144653070 |
| F1WingDamage | 0x145bf90a0 | 0x145bf9030 | 0x14465f580 | 0x14465f450 |
| F1BodyDamage | 0x145bf8a00 | 0x145bf89a0 | 0x144650010 | 0x14464fe90 |
| F1TyreDamage | 0x145bf8ea0 | 0x145bf8d00 | 0x144657290 (KB name Car_TyreDeformation_LoadNodeGraph; it is the F1TyreDamage reader) | 0x144657040 |
| EnergyRecoverySystem | 0x145bfdf40 | 0x145bfdaf0 | 0x1446cfcd0 | 0x1446cf6c0 |
| DebugSystem | 0x145bf9ff0 | 0x145bf9f90 | 0x14467d5d0 | 0x14467d490 |

System sizes (factory allocations from `[car+0xd0]`): Engine 0x25b8, GearBox 0x700, FuelTank 0x110, FuelECU 0x170, BrakeECU 0x2208, Chassis 0x450, Steering 0x140, CoreAero 0x48, Differential 0x100, SuspensionSystem 0xd8, Turbo 0x308, WheelProxies 0x80, ConvexHull 0x140, F1Aero 0x1a08, F1Assists 0x2620, F1Tuning 0x2b0, F1Plank 0x58, F1SuspensionDamage 0x140, F1WingDamage 0xd0, F1BodyDamage 0xa0, F1TyreDamage 0x288, EnergyRecoverySystem 0x1a78, DebugSystem 0x1c0. `CarSpline` is 0x2a8 bytes inline (33 uniform samples max).

## VTF pipeline {#vtf}

1. **Resource load (once per `.vtf` file).** `Car_Load_Vtf` builds `CarVtfAsset`: parses the BXML bytes into a document (`+0x10`, sized from the TSMD stats fragment when the buffer starts with `TSMD` v1), reads `Chassis/HandlingVersion` (`pro` → 100; `handlingYYYY` → clamp(YYYY, 2018, 4048) − 2017, so 2025 → 8; else 1) into `+0x18`, then registers 23 section descriptors `{name, factory}` (`Car_Vtf_AddSection_Thunk`). Nothing per-car happens here.
2. **Car create (once per car spawn).** `Car_Chassis_Build` (chassis-runtime component slot 7; the F1 subclass slot `0x1446379f0` calls it first, then checks the moving-wing nodes):
   - allocates CarLive and CarDynamics;
   - registers the car: `vehicle_id = g_Sess_VehicleCount++`, `g_Sess_VehicleTable[id] = car` (`site_Car_RegisterVehicleId`);
   - `car+0x4a8 = [[phys+0x9e8]+0x28]` (the CarVtfAsset), `car+0x9f40 = asset.handling_version`;
   - `Car_Assets_CreateSystemForPerformanceMode(asset, car+0x380)`: per descriptor, find the section node by name in the team VTF. If the car is equal-performance (`car+0x12c != 0`) and the asset has a base asset (`[asset+0x48]+0x28`), the section comes from the base VTF instead (log `Creating/resetting system using base asset for equal performance`). Then `Car_Systems_FindOrCreate` creates the system (factory) the first time and calls its `Read(node)` = `Car_Read_<X>`;
   - every system: `vt+0x30 Init(0)`, then `vt+0x28` for phases 0..6 fills the per-phase `{fn, ctx}` arrays at `car+0x4f8..+0x528` (counts `+0x4dc..+0x4f4`), then `vt+0x18(-1)` (apply setup), then `Car_Build_VtfComponents` caches the 23 pointers. `site_Car_LiveCarBuilt` (0x144629969, sig `car.live_car_built`) is the instruction after that: one hit per car, everything valid.
3. **Reset (existing car).** `Car_Reset_VehicleSystems(mgr, id, arg)` → `Car_Systems_Reset_Thunk(car+0x4a0)` → body 0x1555bdf80: runs CreateSystemForPerformanceMode again (the same system objects re-read every section from the VTF, no realloc), and destroys the applied-upgrade vector at `car+0x4c0..+0x4c8`. Callers are three `.data1` bodies: 0x148d925f0 (thunk 0x1409ed200, a per-vehicle request with the id at +0xa8), 0x1519eb350 (body of `Game_Set_VehicleController_Thunk` 0x14389cc90: changing who controls a car, e.g. player to AI, re-reads its VTF), and `Car_Physics_ResettleVehicles_Body` (0x1519ed8e0: reset, teleport the physics record to a stored pose, write `F1Assists+0x213c`, then one `Car_Physics_AdvanceCar(car, 1e-4)`). Which user actions reach it is T120. Id 0xff resets nothing in `Car_Reset_VehicleSystems`: 0x142f44fc0 (jmp 0x14f82f190), called on both paths, re-applies one vehicle's per-mode upgrade and returns at once for 0xff. The all-vehicles reset is the `.data1` function 0x14f824410 (via jmp 0x142f445d0), which calls `Car_Systems_Reset_Thunk` per car at 0x14f824536.
4. **Upgrades.** `Car_Upgrades_ApplyToVehicle(car+0x4a0, block, perc)` (career lane L2) looks up an upgrade block in the VTF document and re-runs the section readers on it. `Game_Read_VtfFloat` on an upgrade node applies its `op` to the value already stored: `mul` v·((x−1)·lerp+1), `add` v+x·lerp, `sub` v−x·lerp, `div` v/((x−1)·lerp+1), no op = overwrite (logs `Direct overwrite while upgrading`), with lerp = `g_Car_UpgradeLerp` (perc0to1). After a reset the base values come back and the upgrades have to be applied again.
5. **Teardown.** `Car_Destroy_Live` (0x14473cf70) removes the car from the table by **swap-with-last**: the last car takes the freed slot and its `+0xe8` is rewritten (`site_Car_VehicleTableRemove`, sig `car.vehicle_table_remove`).

When it runs: the VTF parse runs once per resource load; the per-car read (create) runs once per car spawn; the reset path re-reads the VTF into live systems (T119/T120 pin which game events cause which).

## XOR obfuscation and splines {#obf}

`CarObfFloat` is 16 bytes: u32 `a` at +0, u32 `b` at +8, float bits = a ^ b (the decoder reads only the low dwords). Writers draw a 64-bit key per field, `(mt() << 32) | mt()`, from `g_Car_ObfRng` (MT19937 at 0x146108c00, seeded once from QueryPerformanceCounter in `Car_Create_EngineSystem`; next = 0x143929b90; ~1000 call sites across the readers and system constructors). Encoding a new value in memory: keep `b`, write `a = bits(f) ^ b` (low dword). Some field offsets are also obfuscated in code (the store offset is decoded from a `.data1` constant, e.g. F1Plank ExtentX `0x151687bf4 ^ 0x243caf2f`, DebugSystem OverrideTyresRear `0x15218ddac + 0xb70c5131`), so a value's offset can't always be read from the reader's immediates.

`CarSpline` (`Car_Spline_EvalObfuscated` 0x142e88290): `sample_count` (+0x90) stores n-1, the last index (-1 in point mode), so the table holds [+0x90]+1 samples. ≥ 0 → uniform table: index = x·scale(+0x60) + offset(+0x70), clamped, lerp between inline samples from +0x98 (one CarObfFloat each). < 0 → point list `[+0x8, +0x10)` of 32-byte `{x, y}` CarObfFloat pairs. To change a curve in memory, re-encode every sample (or the scale/offset).

The small sections are plain floats: F1Plank (`Offset{X,Y,Z}` at system+0x2c..+0x34, `Extent{X,Y,Z}` +0x38..+0x40), WheelProxies (`CreateProxies` +0x28, `ExtraWidthFront/Rear` +0x30/+0x34), DebugSystem (`RaiseCar` +0x28, tyre override strings +0x30/+0x58). ConvexHull's offsets look like CarObfFloat pairs (L1). The SuspensionSystem records are plain floats except the XOR pairs in Tyre records (six Debris* keys, record +0x70..+0xc0) and Brake records (Density..CoolingHoleRadius, +0x30..+0x90); see #suspension.

## Physics step order {#step}

`Car_Physics_RegisterHooks_Body` (0x1552dac30) registers six callbacks into the physics world `[owner+0xe8]`, in this order (unregistered by `Car_Physics_UnregisterHooks` 0x1446439d0):

1. 0x144641510 (list 0x144458c80)
2. **`Car_Physics_PreAdvance`**: for every car in `g_Sess_VehicleTable` (it branches on count < 0x20 and pads its per-car stack arrays to 32 entries; 0x1c and 0x11 are only SIMD path thresholds), `Car_Physics_AdvanceCar(car, dt)`, sequential or through parallelForEach (`Car_Physics_AdvanceCarJob`).
3. 0x144642660 (per car: passes chassis +0x340 and chassis +0x170/+0x180 to 0x1440c2080/0x1440c2290, likely the rigid-body sync)
4. **`Car_Physics_AdvanceStage`**: phase-5 callbacks of every car, parallelForEach `Car advanceStage`
5. **`Car_Physics_RunPhase6`**: phase-6 callbacks, sequential
6. **`Car_Physics_PostVehicleUpdate`**: parallelForEach `postPhysicsVehicleUpdate` → 0x144642e20 per car

`Car_Physics_AdvanceCar(car, dt)` (0x14473fd80):

| step | what runs |
|---|---|
| phase 0 | Chassis `Car_Failsafe_LimitVelocity`, SuspensionSystem 0x1446b43d0, ConvexHull 0x14464dbe0, F1Assists `Car_PadAssist_DriftAssist`, F1Plank 0x144652e80, WheelProxies 0x14464f310 |
| 0x144741700 | called at 0x14474049c (not read) |
| phase 1 | Chassis 0x144678300, F1Aero 0x1446cb540, ERS 0x1446d1210, F1SuspensionDamage 0x1446563e0, F1WingDamage 0x1446610f0 |
| copy | chassis +0x170/+0x180 → +0x2e0/+0x2f0 (previous transform) |
| phase 2 (dt) | Engine 0x144688610, Steering 0x1446a78d0, SuspensionSystem 0x1446b4610 |
| CarDynamics | `Car_Dynamics_Update` (vt+8) once (sets substeps = max(1, (int)(dt·360)) from the setting `Vehicle.FineUpdateRate` (`g_Car_PhysicsRateHz` 0x145faa60c), or `Vehicle.FineUpdateRateAI` (0x145faa610) when car+0x124 (`is_ai`) == 1; ends with the GearBox update 0x1446a6b60 at tick dt and 0x1446db280), then `dyn+0xa4c` substeps, each calling either vt+0x10 `Car_Physics_StepVehicle` (`site_Car_DynamicsSubstep` 0x144740b66) or vt+0x18 0x1446e42c0, a reduced step without aero (0x144740b96; taken when byte 0x146ce89d8 != 0 or CarLive+0x128 == 1); then vt+0x20 `Car_Suspension_PublishWheelState` once after the loop (0x144740bb9) |
| phase 3 | SuspensionSystem 0x1446b4f10 |
| phase 4 | Chassis 0x144678660, SuspensionSystem 0x1446b4f80, FuelTank 0x14468edb0, F1TyreDamage 0x14465c610 |

Later in the tick: phase 5 (Chassis 0x1446788e0, WheelProxies 0x14464f720, F1SuspensionDamage 0x1446566b0, F1WingDamage 0x144661070, F1BodyDamage 0x1446525c0), then phase 6 (Chassis 0x14467a940, SuspensionSystem 0x1446b5550). GearBox, BrakeECU, CoreAero, Differential, Turbo, F1Tuning and DebugSystem return no phase callback: the powertrain, brakes and aero are read inside the CarDynamics substeps (`Car_Powertrain_Step`, `Car_Physics_StepVehicle`) through the CarLive pointers. FuelECU's phase slot was not checked.

Rate: the hooks run once per physics-world tick; inside, each car integrates at 360 Hz substeps (`dyn+0xa48` = dt/substeps). The rates and the tyre-contact cycles (`Vehicle.TyreContactSubStepCyclePlayer` = 1, `...AI` = 3, 0x145faa614/0x145faa618) are game settings registered by `Game_Read_VehicleConfig` 0x14463a800, so they can be changed through the settings system or by writing the globals (untested).

Class name: the live car is `Car` (`Game_Place_Vehicle` 0x14473d810 logs `Car::teleportRequest ... -> %p` with CarLive as its first argument and locks `car+0x98`, the critical section `Car_Chassis_Build` initialises). The physics owner with the six hooks is most likely `VehicleModule` (its `VehicleModule::update` string is used at 0x144643899, next to the hook functions); not confirmed.

## Vehicle table and participants {#vehicles}

- `g_Sess_VehicleTable` (0x146ce86e0) holds CarLive pointers. The writer is `Car_Chassis_Build` in `.xtls` (0x144629637..0x14462965x), so the session note "no writer in .xtls" is out of date. `Car_Physics_PreAdvance` branches on count < 0x20 and pads its per-car stack arrays to 32 entries (memsets 0x14464182c..0x144641864); 0x1c and 0x11 are SIMD path thresholds, not bounds.
- `vehicle_id` (CarLive+0xe8) is a dense slot (create order; swap-with-last on destroy). The entrants lane's `EntrantRecord+0x82` (init 0xff; the tyre-allocation note calls it the vehicle index) is the likely participant → vehicle_id link (T121).
- Runtime (2026-10-28 Grand Prix, T121): ids 0..20 in create order; id 20 is the safety car (1734 kg, no ERS, 20 systems, handling version 1), so a 20-car race has 21 CarLive objects. `is_ai` (+0x124) read 1 for every car, the player's included. Role confirmed (V8748): it selects the AI physics rate and tyre-contact cycle in Car_Dynamics_Update. NOT confirmed: "AI-controlled right now" (the "game took the idle player's car over" reading was a guess); do not use it to pick the player or tell a human from an AI car. The player's car is most likely id 19: the only one with controller-change resets and its own setup (brake pressure 0.97, 10 kg fuel in the first ApplyTuning). The participant mapping is still open.
- `CarVehicleManager` (Sess_Veh_UpdateWheelMap's object) lists per-vehicle objects at `+0x110..+0x118`; item `+0x20` = CarPhysicsRecord. `Car_Reset_VehicleSystems` uses another manager: entity list `[+0x28..+0x30]` by id, and `Car_Physics_ResettleVehicles_Body` reads physics records at `[mgr+0xb0 + id*8]` bounded by `[mgr+0x180]`, which leaves room for exactly 26 records if the array is inline (grid campaign: check).
- Upgrade component → CarLive: the career vehicle-upgrade component (`site_Career_ApplyUpgradeBlockEntry` 0x14fa63050, comp+0x28 = map<u8 type, map<u8, {progress, mass, bhp}>>, comp+0x21 flag) reaches the car as `[[comp+0x18]+0x58]`: comp+0x18 is the vehicle's CarPhysicsRecord, and the body tail-jumps to `Car_Upgrades_ApplyToVehicle(CarLive+0x4a0, block, flag, perc)` (0x14fa6316a..0x14fa6317b).
- CarLive → CarPerfVehicle / VehicleIdentity / AI config: no pointer found in CarLive in this round. The per-vehicle object in the manager list (≥0x9cb0 bytes, tyre component fields at +0x9bc0/+0x9c68) is the candidate for CarPerfVehicle (attr array +0xd10, stride 0xc40).

## Changing handling: the car object {#addon-object}

1. **Edit the VTF before load** (best): the resource is parsed once, and every create or reset reads from it. No obfuscation to deal with. Equal-performance cars read the base VTF.
2. **Write the systems after create**: hook `site_Car_LiveCarBuilt` (rdi = CarLive) or walk `g_Sess_VehicleTable`, then write fields in the systems. Spline and obfuscated coefficients need CarObfFloat re-encoding. Any reset (`Car_Systems_Reset_Thunk`) re-reads the VTF and wipes such writes, so re-apply after it (hook the thunk's return). Values derived at Init (`vt+0x30`) or setup apply (`vt+0x18(-1)`) won't follow a raw write; call the system's `vt+0x18(-1)` afterwards (needs L3).
3. **Hook the reader**: wrap a `Car_Read_<X>` (data vt+0x10) and edit after the original; this covers create, reset and upgrades. One reader is shared by every car, so filter on the system's `+8` car.
4. **Hook the step**: phase callbacks are plain `{fn, ctx}` pairs at `car+0x4f8..+0x528`. They can be swapped per car without patching code (data write; behaviour-hook rules apply). `site_Car_DynamicsSubstep` is the substep.

Needs L3: T119 (create/teardown timing), T120 (which actions reset the systems), T121 (vehicle_id ↔ participant).

## Powertrain {#powertrain}

Lane car-b. Structs `CarEngine.h`, `CarTurbo.h`, `CarGearBox.h`, `CarFuelTank.h`, `CarFuelEcu.h`, `CarErs.h`,
`CarErsMode.h`, `CarDifferential.h`, offsets from the system pointers at CarLive +0x530/+0x538/+0x540/+0x548/+0x578/
+0x5a0/+0x5a8.

### Object layout and the system vtable {#runtime}

Every powertrain component is a "system" object made by its factory (`Car_Create_<X>System`, registered by name in
`Car_Load_Vtf`):

| component | factory | alloc | system vtable | data vtable | reader (data slot +0x10) |
|---|---|---|---|---|---|
| Engine | 0x1447484f0 | 0x25b8 | 0x145bfa630 | 0x145bfa1b0 | Car_Read_Engine 0x144685680 |
| GearBox | 0x144749270 -> .data1 0x1555c4f90 | 0x700 | 0x145bfb250 | 0x145bfa9e0 | Car_Read_GearBox 0x1446a39a0 |
| FuelTank | 0x144749420 | 0x110 | 0x145bfa980 | 0x145bfa8c0 | Car_Read_FuelTank_Thunk 0x14468ea90 -> 0x1554035d0 |
| FuelECU | 0x14474bf70 -> .data1 0x1555cc570 | 0x170 | 0x145bfa870 | 0x145bfa6f0 | Car_Read_FuelECU_Thunk 0x144689fb0 -> 0x1553fb310 |
| Differential | 0x1447518c0 | 0x100 | 0x145bfa120 | 0x145bfa040 | Car_Read_Differential_Thunk 0x14467ddf0 -> 0x1553d2720 |
| Turbo | 0x1447539f0 -> .data1 0x1555ded40 | 0x308 | 0x145bfbe80 | 0x145bfbdc0 | Car_Read_Turbo_Thunk 0x1446b6580 -> 0x15543fad0 |
| ERS | 0x144759840 | 0x1a78 | 0x145bfdf40 | 0x145bfdaf0 | Car_Read_EnergyRecovery 0x1446cfcd0 |

System head: +0 system vtable, +8 CarLive, +0x10 allocator, +0x18 data vtable, +0x20 data allocator, data fields
from +0x28. The reader/writer get `system+0x18`, so **struct offset = reader offset + 0x18** (e.g. Car_Read_Engine
writes Mass at +0x2f8, the struct has it at +0x310). car-a's CarComponentBase.h agrees (car at +8).

System vtable slots as used by this lane (from the vtables above and Car_Upgrades_ApplyToVehicle 0x144742c70):
- +0x08 Read(node) and +0x10 Write(node): forwarders 0x14475ab50 / 0x14475ab80 to the data object (ERS has its own
  +0x08, 0x1446d1430, same shape).
- +0x18 **Reset(flags)**. Engine = 0x1446878d0 (KB name Car_Tuning_SetEngineBrakingAndRadiator), GearBox =
  0x1446a4c20 (Car_Tuning_SetFinalDrive), FuelECU 0x14468e550, FuelTank 0x14468ed50, ERS 0x1446d0c40, Turbo
  0x1446b67f0, Differential 0x14467e010 (setup diff sliders). Flag bits seen: 0x10 engine state, 0x20 gearbox state, 0x40 refuel,
  0x200 full reset, 0x400 temperature, 0x800 PU wear, 0x1000 gearbox faults, 0x2000 re-apply setup (F1Tuning) and force
  fuel mode Standard. The resets gate on the system bit first: the Engine Reset runs nothing without 0x10 (test dl,0x10
  at 0x1446878ef), so its 0x400, 0x800 and 0x2000 need 0x10; the GearBox Reset runs nothing without 0x20 (test
  dil,0x20 at 0x1446a4c40), so its 0x1000 and 0x2000 need 0x20; the ERS Reset fires only when both 0x10 and 0x200 are
  set; the Differential Reset (0x2000) writes through car+0x578, the car's Differential, not its own this.
- +0x20 RegisterChannels: debug/telemetry channels with getter lambdas into car+0x148. The getters name the runtime
  fields (rotation_rate, rpm, temperature, wear_*, torque, energy, deploy_power, mass, ...). Every runtime field name in
  the structs comes from these strings.
- +0x28 GetPhaseCallback(out, phase): Engine phase 2 -> Car_Engine_StepInput 0x144688610; ERS phase 1 ->
  Car_Ers_StepWear 0x1446d1210; FuelTank phase 4 -> Car_FuelTank_StepBurn 0x14468edb0. GearBox, FuelECU, Turbo and
  Differential return none: they are driven from Car_Powertrain_Step.
- +0x30 Init (derived values): Engine 0x144687d60 -> 0x1553f5460.
- +0x38 / +0x40 GetData: return this+0x18.

### When values are read and when derived values are rebuilt {#load}

Readers run for the base VTF section at load and again for each `Upgrade` block. `Car_Upgrades_ApplyToVehicle`
(career lane, L2) finds `<Upgrade name=block>`, and for each child section calls that system's +0x08 Read with
g_Car_UpgradeLerp = perc0to1 and g_Car_UpgradeLerpActive = (perc != 1). After the block it calls **every** system's
+0x30 Init and then +0x18 Reset with flags `((param_3 ^ 1) << 9) | 0x20bd`. So after an upgrade replay the engine
derived values are recomputed, the fuel mode is forced to Standard (0x2000), setup sliders are re-applied, and with
param_3 == 0 the 0x200 reset also refills ERS energy and puts the gearbox in neutral. Fuel is not refilled (0x40 not
in 0x20bd).

Game_Read_VtfFloat ops (0x14464e760): `op="*"` multiply, `+` add, `-` subtract (x - v), `/` divide, `=` or no op =
overwrite (logged "Direct overwrite while upgrading"); with the lerp active `*` becomes ((v-1)p+1)x, `+` x+vp, `-`
x-vp. Missing key returns 5 and leaves the field (so the constructor default stands).

### Engine {#engine}

Data: see CarEngine.h (77 fields). Key units: RevsPowerSpline is **power in W against rotation rate in rad/s** (Ferrari
2025 peak 550060 W at 1191.5 rad/s = 11378 rpm). IdleRotationRate / LimiterRotationRate are rad/s (Ferrari 418.9 /
1371.8). Defaults come from Car_Engine_DataCtor 0x14467e900 (Mass 100, Inertia 0.3, idle 115.19, limiter 890.12,
InitialTemperature 323.15 K, wear rates 0, ...). Almost everything is a CarObfFloat; RadiatorSlipStreamFactor is
plain. EngineLayout and TurboMode are enums (TurboMode Off 0, LagSpoof 1, Full 2).

Runtime (+0x2508..+0x25b8): rotation_rate +0x2538, temperature +0x253c, kers_energy +0x2540, load +0x2544, torque
+0x2548, PU wear +0x254c ICE, +0x2550 TC, +0x2554 ES, +0x2558 MGU-H, +0x255c MGU-K, +0x2560 CE (0..1, "damage" =
max), throttle_lagged +0x2570, throttle +0x2574, engine_braking_fraction +0x252c, radiator_opening +0x25b0.

Derived (Car_Engine_Init_Body 0x1553f5460 and Car_Engine_ComputeDerived_Thunk 0x144689a70): +0x2518 max_torque =
max over the power-spline points of P x power_scale / (rad/s x revs_scale); +0x2528 limiter = LimiterRotationRate x
revs_scale; +0x2508 = power_scale x RevsPowerSpline+0x30. Both read the FuelECU active scales, and the FuelECU
Reset/SetMode re-run 0x144689a70. The power lookup itself (Car_Powertrain_Step) reads the spline live, so editing
spline samples changes power immediately; max_torque and the limiter need the re-run.

Steps: Car_Engine_StepInput (phase 2) turns the pedal into throttle (ThrottleMap splines, limiter cut with
RedLineThrottleCutTime, fuel starvation below 0.08 kg, anti-stall, legacy KERS) and writes CarLive+0x1bc.
Car_Powertrain_Step 0x1446e2290 {#powertrain-step} (called from Car_Physics_StepVehicle after the suspension solve)
computes drive torque = RevsPowerSpline(rad/s / revs_scale) x power_scale / rad/s (site_Car_EngineDriveTorque
0x1446e27a1), multiplies by Turbo efficiency_factor when TurboMode == Full, takes the braking torque from
RevsEngineBrakingSpline lerped by engine_braking_fraction and gear, and without ERS stores (drive + braking) x
throttle - braking at +0x2578. It also stores the ERS exhaust power at ERS (car+0x5a8) +0x1a2c = turbo +0x2fc x
throttle x engine speed x ExhaustPowerCoefficient (ERS +0x74) x FuelECU burn scale; that is not a fuel flow (fuel flow
is FuelTank +0x108). Param is the vehicle-dynamics system (vt_Car_Dynamics?), whose +0x18 is CarLive and
+0x80 the Engine system.

Modifiers: EnginePower = VTF block `radUpgradeEngine`: `+` on every RevsPowerSpline y (Ferrari: +388 .. +65409 W),
FuelBsfcMin x0.9, FuelBsfcMax x0.895, FuelOptimalTorque = 550, FuelTorqueLinear x1.4. Durability = `radUpgradeWearICE`
(WearICERate x0.5), `radUpgradeWearTC` (WearTCRate x0.5), `radUpgradeWearMGUH` (WearMGUHRate x0.5), `radUpgradeWearCE`
(WearCERate and WearCEDVibrationRate x0.5). Setup: engine braking and radiator sliders via Reset 0x10|0x2000. Fuel mode:
power and revs scale via FuelECU. Equal-performance mode takes sections from the base asset (car-a,
CarLive+0x12c).

### Turbo {#turbo}

Plain floats (no XOR): BoostEfficiencySpline +0x28, ZeroBarRate +0x2d0, MaxBoostRate +0x2d4, MaxBoost +0x2d8,
RelativeSize +0x2dc, Acceleration +0x2e0, Deceleration +0x2e4 (Ferrari 500/700/5.5/0.1/500/50). Runtime pressures
+0x2e8..+0x2fc, rotation_rate +0x2f8, efficiency_factor +0x304 (Reset 0x210 sets 1.0). The turbo state update itself
was not located (no phase callback; it is inside the powertrain path). No R&D block touches Turbo.

### GearBox {#gearbox}

Ratios R, N, 1st..9th as an 11-entry CarObfFloat array at +0x38 (stride 0x10), FinalDriveRatio +0x28. Shift timing,
clutch, auto-shift speed multiples, fault model (jammed/missing gear, shift-time faults) as listed in CarGearBox.h.
Runtime: gear_index +0x680 (0 R, 1 N, 2 1st), inv_total_ratio[11] +0x5fc = 1/(ratio x final drive) built by Reset
(0x20|0x200) or from setup_ratios +0x628 / setup_final_drive +0x654 when use_setup_ratios +0x540, damage +0x674,
clutch torque +0x698. Setup final drive = lerp(F1Tuning +0x198..+0x19c, slider) in Reset 0x20|0x2000. Durability:
`radUpgradeWearGearBox` DamageRateMidShift x0.5. Editing ratios after load needs Reset with 0x220 (or 0x20|0x2000), since
the step uses inv_total_ratio.

### FuelECU {#fuel-ecu}

Four tables of four modes (Lean, Standard, Rich, Max) at +0x28 (burn), +0x68 (power), +0xa8 (revs), +0xe8
(temperature), stride 0x10. Active mode +0x128 and active scales +0x130 burn, +0x140 power, +0x150 revs, +0x160
temperature (CarObfFloat). Car_FuelEcu_SetMode_Thunk 0x14468e6a0 (body 0x1553ff3d0) copies a mode in and re-runs the
Engine derived values and the GearBox table; callers 0x1432495e0, 0x14389a2b0, 0x14470d760 (T123 asks which is the
MFD). Eight functions at 0x1429xxxxx force mode 1 the same way. 2024/2025: only Lean differs (burn 0.7, power 0.9);
2026/F2: all 1.0. FuelConsumption = `radUpgradeFuelConsumption`: all four burn scales x0.85 and ERS
ExhaustPowerCoefficient x1.2.

### FuelTank {#fuel-tank}

BSFC model: FuelBsfcMax/Min, FuelOptimalRevs (rad/s), FuelOptimalTorque (Nm), FuelRevsLinear, FuelTorqueLinear,
FuelCapacity (kg, >= 0.1), COM, size, FixedLoad. Car_FuelTank_StepBurn (phase 4, gated by CarLive+0x130 bit 6): d = hypot((torque - optimal) x
TorqueLinear, (rad/s - optimal) x RevsLinear); bsfc = BsfcMin + (BsfcMax - BsfcMin) d; fuel_flow +0x108 = rad/s x
torque x bsfc x active_burn_scale x 0.001 kg/s (site_Car_FuelFlowStore); fuel_mass +0x100 -= flow x dt when
CarLive+0x132 bit 2 (site_Car_FuelMassStore), floor 0.05. Reset 0x40 fills mass = capacity x fill_fraction +0x104.
fuel_mass is plain: writing it works next step.

### EnergyRecoverySystem {#ers}

All plain floats. Common block +0x28..+0x82 (MaxDeployPower 2025 126.3 kW / 2026 350 kW, harvest and deploy lap limits
in J, EnergyStoreCapacity 4 MJ, efficiencies, MaxBrakeTorqueMGUK, MGU-H coefficients, 2026 override flags), splines
(SpeedDeploy, SpeedDeployOverride, SpeedHarvest, ChargeDeploy, wear splines), modes[4] at +0x1880 (CarErsMode: name,
gear_modifier[11] indexed by gear index, DerateStartSpeed), per-track TrackSettings +0x1980.. (read only for the
current track at load). Runtime: energy +0x19f0, lap counters +0x19c8..+0x19d0, lap_harvest_limit +0x19f4, powers
+0x1a00/+0x1a04/+0x1a14, ers_mode +0x1a68 (Reset with 0x10|0x200 = 1 Medium).
The factory leaves the energy, the lap counters and ers_mode unset, and a system created on a reset of the car's
systems with arg 1 never gets the 0x200 Reset (V2124, V2125). Core's ERS mode guard (`native/src/core/ers_mode.cpp`) hooks
site_Car_ErsGearModifierLoad and runs the system's own Reset with 0x10|0x200 when the mode is outside 0..3.

Deploy (Car_Powertrain_Step): MaxDeployPower x clamp(modes[ers_mode].gear_modifier[gear]) (site_Car_ErsGearModifierLoad
0x1446e286f) x deploy factor (track +0x1980.. when track_settings_found, else defaults +0x19a8..; mode 2 Overtake uses
0), with the speed deploy spline (override spline while +0x1a62). Wear (Car_Ers_StepWear, phase 1, gated by CarLive+0x130 bit 6; the wear part needs bit 7):
Engine wear_es and wear_mguk integrated from BaseWearRateES/MGUK, charge/speed splines, deploy ratio, CE wear penalty,
CarLive+0x9f2c and Engine+0x25a8; energy capped at capacity x ESWearCapacityModifierSpline(wear_es).

Modifiers: `radUpgradeERS` (MGU-K harvest efficiency x1.042, ExhaustPowerCoefficient x1.15, OvertakePowerEfficiency
x1.35, per-mode gear modifiers overwritten), `radUpgradeWearES` / `radUpgradeWearMGUK` (BaseWearRate x0.5),
`radUpgradeFuelConsumption` (ExhaustPowerCoefficient x1.2).

### Differential {#differential}

DriveType, RearPowerProportion, DiffRearNoEffect/FullEffect (derived inv range +0x60), DiffRearMaxLockingTorque,
DiffMidCornerGForce, DiffGForceBlendSlope, RampDifferential. DiffRearTorque is read and dropped. Runtime
rear_lock_amount +0xe0. Setup: Car_Differential_Reset_Thunk 0x14467e010 (body 0x1553d6f70), flag 0x2000, writes into the car's Differential
through car+0x578 (not its own this)
diff_off_throttle +0xec and diff_on_throttle +0xf0 as lerps of F1Tuning ranges by the setup sliders (F1Tuning +0x218 /
+0x214, logs F1_TUNING), plus three more setup values +0xf4/+0xf8/+0xfc (roles open). The diff torque code that
consumes them was not read.

### How to change powertrain handling from an addon (ranked)

1. **Edit the VTF before load** (boot): change `Engine/RevsPowerSpline`, ratios, FuelECU scales, ERS Common values in
   `teams/<team>/handling/<team>.vtf`, or the `radUpgrade*` blocks for R&D-scaled changes. Nothing to encode; derived
   values come out right. ERS TrackSettings can add per-track ERS (tools/vtf.py add_track_settings).
2. **Write the struct after build**, then re-run the owner: Engine spline samples / power: write the CarObfFloat
   (bits ^ key) and call the Engine Init (system vtable +0x30) so max_torque and the limiter follow. Gear ratios /
   final drive: write the pairs, call GearBox Reset(0x220) (0x20 must be set: the GearBox Reset does nothing without it). Fuel mode scales: write the table entry and call
   Car_FuelEcu_SetMode_Thunk with another mode then back (it returns early when the mode is unchanged), or write the
   active pairs and call Car_Engine_ComputeDerived_Thunk. Plain fields (Turbo, ERS, fuel_mass, ERS energy) take
   effect next step. Beware: any upgrade replay re-runs Reset(0x20bd) on every system.
3. **Hook the reader**: wrap Car_Read_<X> / the thunk to post-edit the data object after each read (base and every
   Upgrade block; check g_Car_UpgradeLerpActive).
4. **Hook the step**: site_Car_EngineDriveTorque (scale xmm0 = power), site_Car_ErsGearModifierLoad (deploy fraction),
   site_Car_FuelFlowStore (consumption). Each has a signature (car.engine_drive_torque, car.ers_gear_modifier_load,
   car.fuel_flow_store). All need L3 (T122-T125).

### Open (car-b)

- T122 Engine rotation_rate / throttle at site_Car_EngineDriveTorque.
- T123 who calls Car_FuelEcu_SetMode_Thunk for the player's MFD fuel mix.
- T124 fuel_mass burn identity at site_Car_FuelMassStore.
- T125 ers_mode follows the MFD ERS mode.
- PU wear writers found by a disp32 scan of .xtls (stores to Engine +0x254c/+0x2554): only the Reset, the ERS wear step,
  and three failure paths that force wear near 1: 0x1409b3cb0 (1.0), 0x1446893c0 (0.99, called from Car_Engine_StepInput),
  0x14470d760 (0.999; now named Car_State_Read_Snapshot (2026-11-11, car-re): a per-car snapshot deserializer that also forces engine wear,
  calls Car_FuelEcu_SetMode_Thunk and restores CarErs.ers_mode and the F1Aero drs_flags bits).
- Not covered: where career PU wear (engine_management) is loaded into the six wear fields (no plain store found),
  the Turbo pressure update, the diff torque code, the ERS harvest code, the meaning of RevsPowerSpline+0x30.

### Cross-lane notes (car-b)

- car-a: CarSpline is 0x2a8 bytes (Car_Read_Engine: RevsPowerSpline at data +0x10, next field +0x2b8; ThrottleMap
  vectors stride 0x55 qwords). System vtable slot roles above (+0x18 Reset(flags), +0x20 RegisterChannels, +0x28
  GetPhaseCallback, +0x30 Init). Car_Powertrain_Step's parameter is the vehicle-dynamics system with +0x18 = CarLive
  and +0x80 = Engine (probably vt_Car_Dynamics 0x145bff4d0); please name it in CarLive.h.
- car-d: R&D attribute -> VTF block names seen in the 2025 Ferrari VTF: radUpgradeEngine, radUpgradeFuelConsumption,
  radUpgradeERS, radUpgradeFrontDownforce, radUpgradeRearDownforce, radUpgradeDrag, radUpgradeDRS,
  radUpgradeWeightReduction (Chassis/Mass 18 op=-), radUpgradeWeightRedistribution, radUpgradeTyreWear, radUpgradeBrakes,
  radUpgradeWearCE/ES/GearBox/ICE/MGUH/MGUK/TC; tiers very_low/medium/high/very_high. Reset flags after a replay:
  0x20bd | ((param_3 ^ 1) << 9).
- car-c: Car_Powertrain_Step also evaluates BrakeECU splines at car+0x550 +0x10 (dry) / +0x1000 (wet) and +0x2b8 /
  +0x12a8, +0x560 / +0x1550, gated by CarLive+0x124 == 1 (assist mode).

## Chassis, aero, suspension and tyres {#chassis-lane}

Lane car-c. Structs `CarF1Aero.h`, `CarChassis.h`, `CarCoreAero.h`, `CarSteering.h`, `CarBrakeEcu.h`, `CarSuspension.h`,
`CarSuspTyreType.h`, `CarSuspBrakeType.h`, `CarSuspCorner.h`, `CarSuspAxle.h`, `CarWheel.h`, `CarTyreState.h`,
`CarTyreCompound.h`, `CarTyreMfCurve.h`, offsets from the system pointers at CarLive +0x550/+0x558/+0x560/+0x568/+0x570/+0x580.

### Layout of a component system {#layout}

Every system is `{+0 system vtable, +8 CarLive *car, +0x10 allocator, +0x18 data object}` (CarComponentBase). The data
object has its own 4-slot vtable `{dtor, GetName, Car_Read_<X>, Car_Write_<X>}`; a reader's `param_1` is the data
object, so **system offset = reader offset + 0x18**. Proven per component by the constructor writing the data vtable
at +0x18 and by the runtime readers using the +0x18-shifted offsets:

| system | ctor (alloc) | system vtable | data vtable | reader (slot 2) |
|---|---|---|---|---|
| F1Aero | .data1 0x1555e9310 (0x1a08), thunk Car_Create_F1AeroSystem | 0x145bfca70 | 0x145bfc370 | Car_Read_F1Aero 0x1446c8560 |
| Chassis | Car_Create_ChassisSystem 0x14474c060 | 0x145bf9ca0 | 0x145bf9c10 | Car_Read_Chassis 0x144676730 (.data1 0x1553bbbc0) |
| CoreAero | .data1 0x1555dac10 | 0x145bf9f20 | 0x145bf9eb0 | Car_Read_CoreAero 0x14467d080 (.data1 0x1553cb180) |
| Steering | Car_Create_SteeringSystem 0x14474e650 (0x140) | 0x145bfb400 | 0x145bfb310 | Car_Read_Steering 0x1446a7070 (.data1 0x15541f940) |
| BrakeECU | .data1 0x1555d2450 (0x2208) | 0x145bf9bb0 | 0x145bf9730 | Car_Read_BrakeECU 0x1446752f0 |
| SuspensionSystem | Car_Create_SuspensionSystemSystem 0x144753050 (0xd8) | 0x145bfbab0 | 0x145bfba10 | Car_Read_SuspensionSystem 0x1446aa090 |

The system vtable slot +0x18 is **OnReset(sys, flags)** (Chassis: `Car_Tuning_SetChassisZCom` 0x144676d40, F1Aero:
`Car_F1Aero_OnReset` 0x1446c9680). Derived values live there, so the flags decide whether a raw edit reaches the sim.

For car-a: `CarLive.h` notes say a component keeps the car at component+0x18; the code reads it at **system+8**
(`[F1Aero+8]+0x568` in StepVehicle, ctor `puVar[1] = param_1`), which matches `CarComponentBase.h`. The +0x18 slot is
the data vtable.

XOR keys: every CarObfFloat key is drawn per instance at construction (Chassis ctor calls 0x143929b90 on RNG
0x146108c00 for each pair), so keys differ per car and per session. Encode with the live key: `a = bits(f) ^ [p+8]`.

### F1Aero {#f1aero}

**Data (CarObfFloat, VTF F1Aero/*)**: front/rear cl maps `m_aero_{front,rear}_cl_{maxf,minf}_{maxr,minr}`,
`m_aero_{front,rear}_wing_loss`, AeroReference*, ride-height clamps `m_aero_{min,max}_{front,rear}_height`, slipstream
scales, `m_aero_wing_{front,rear}_{drag,force}`, 8 splines (ForceVelocityMultiplier, Rear/FrontCoeffLiftVsRideHeight
with fallback keys Rear/FrontForceRideHeightMultiplier, ...VsChassisRake, Rear/FrontForceDirectionMultiplier,
DistanceDrivenCumulativeDRSFailureChance), Front/RearCoeffLiftMultiplier (sys+0x1708/+0x1718), BodyDragCoeffMin/Max
(aliases m_aero_cd_min/max, sys+0x1728/+0x1738), Num{Front,Rear}WingSettings (u32), wing CoL0-2/CoD0-2, damage mults,
DRS, centre of drag, side/top drag, brake duct and radiator lift/drag (sys+0x18e8..+0x1948), AdjustableFrontWing bool.
Defaults when a key is missing: the reader keeps the current value (Game_Read_VtfFloat leaves the out param alone).

**Derived at reset (`Car_F1Aero_OnReset`)**: the whole body runs only when flag bit 0x80 is set (test dil,dil; jns at
0x15546100b to the epilogue): the 0x200, 0x1000 and 0x2200 branches and the +0x19ac clear all sit inside that test, so
without 0x80 the call does nothing. It copies decoded BodyDragCoeffMin/Max to +0x1960/+0x1964 and the
eight cl-map values to +0x1968..+0x1984 (plain floats); flag 0x2200 computes the wing terms from the integer wing
settings s (+0x19b4 front, +0x19c0 rear): `lift = CoL0 + CoL1*s + (CoL2*s)^2`, same for drag, into +0x19b8 (front
lift), +0x19bc (front drag), +0x19c4 (rear lift), +0x19c8 (rear drag); flag 0x1000 zeroes wing damage and sets
drag_scale +0x19a0 and lift_scale +0x19a4 to 1.0; flag 0x200 resets DRS state.

**Per step (`Car_Physics_StepVehicle` 0x1446dceb0)**, sys = [car+0x570]:
- ride-height ratios from Car_F1Aero_GetFront/RearRideHeight (0x1446cb000 / 0x1446cb260, handling_version >= 7) or
  the wheel heights (older), normalised by the min/max height clamps. Each getter takes per wheel plane - (wheel+0x168 +
  [wheel+0x140]+0x15e4) - clamp01(wheel+0x248 - wheel+0x168) (plane = AeroReferencePlaneHeight sys+0x78), averages per
  axle (rear = SuspensionSystem +0x70/+0x78, front +0x80/+0x88), then extrapolates by AeroReferenceOffsetRear/FrontZ
  over the axle z gap and clamps.
- rear body lift coefficient +0x19ec `rear_lift_coeff` (store 0x1446dda91) = (BrakeDuctRearCoeffLift x
  mean(RL,RR wheel+0x130) + RadiatorOpeningRearCoeffLift + cl-map/curve terms) x RearCoeffLiftMultiplier x lift_scale.
- front body lift coefficient +0x19f0 `front_lift_coeff` (store 0x1446ddda4) = same with the Front keys x (1 -
  FrontCoLByWingDamageMult x front_wing_damage(+0x199c)): the damage term is a factor, not a subtraction. **The older
  inbox note had +0x19ec as front: it is rear.**
- These two are coefficients, not forces (T115, 2026-10-28): the player car read 0.72 / 0.96 (front / rear) parked
  and 0.79 / 1.11 at speed, front share about 0.43; the other cars 0.56..0.82 / 0.88..1.13. They were named
  front_downforce / rear_downforce before the run. The store sites themselves were not traced (the values come from
  gdb snapshots), so `site_Car_AeroStore*` stay L2.
- slipstream: lift x (k + (1-k) x CoreAero+0x30) with k = m_aeroSlipStreamAirDensityScaleLift.
- total lift +0x19e4 (store 0x1446ddf0d) = body front + rear + front wing term x (1-front_wing_damage) x
  ((m_aero_wing_front_force-1) x wing_open + 1) + rear wing term x (1-rear_wing_damage(+0x1998)) x
  ((m_aero_wing_rear_force-1) x wing_open' + 1); wing_open = +0x19a8, wing_open' = 0 when +0x19ac is set.
- total drag +0x19e8 (store 0x1446de114) = (lerp(+0x1960, +0x1964, ratio) + BrakeDuct*CoeffDrag x wheel+0x130 +
  RadiatorOpeningCoeffDrag) x drag_scale x (1 + front_wing_damage x BodyCoDByWingDamageMult) + wing drag terms
  (rear one blended by m_aero_wing_rear_drag, which radUpgradeDRS multiplies by 0.9). No reader of +0x19e8 was found
  (the +0x19e8 reads at 0x142948f81/0x14296dbe2 are another object). It is the drag force in N, not a coefficient:
  about 0 parked and -2402 at speed for the player car (T115); total lift +0x19e4 read about 0 parked and -7720 at
  speed.
- getters 0x1446ca5f0 (total downforce, x1.5 factors) and siblings 0x1446caa40/0x1446cab60/0x1446cacc0 (balance).

### Chassis {#chassis}

Data: Mass, MOIX/Y/Z, COMY, COMZ, FFBScale, Front/RearVisualHeightOffset (CarObfFloat), CarCode,
CarHandlingVersionName, TyrePhysicsGroup (strings), sqrt(Mass) derived **inside the reader** (+0x98).
OnReset (`Car_Tuning_SetChassisZCom`, chassis vtable +0x18): flag 0x2000 sets live COM z +0x158 =
lerp(F1Tuning COM-Z range, slider) ("Setting chassis z-com"); flag 0x200 pushes COM (COMY, +0x158) into the body
(0x144676a80), mass +0x150 = sqrt_mass^2, inv mass +0x154, MOI +0x160..+0x168. The rigid body uses these, so a raw
Mass/COM/MOI edit needs OnReset(0x200) (or direct writes of +0x150..+0x168). At runtime +0x150 also carries the fuel:
it read VTF Mass + FuelTank.fuel_mass (816.21 = 803 + 13.217 on the grid, 815.76 = 803 + 12.766 later; T118), so
something adds the fuel after OnReset (writer not mapped yet).

### CoreAero {#coreaero}

SlipStreaming (+0x28) and SlipStreamWhileBraking (+0x29) bools, ctor 1; +0x30 wake density ratio (ctor 1.0) that
F1Aero multiplies into lift (and drag). +0x40 byte ctor 0xff (probably the car being followed).

### Steering {#steering}

MaxSteeringLock, AckermannSteering, VisualTurnRate and the damage-wobble block, CarObfFloat from +0x28. The VTF
`Upgrade[very_high]` block raises MaxSteeringLock to 0.41 and AckermannSteering to -0.04363 (an assist-level block,
not R&D). Consumers not traced this round.

### BrakeECU {#brakeecu}

12 ABS/ATC splines (front variants fall back to the unsuffixed key), brake torques BrakeFront/Rear (2025: 3105/2295
N m) and the Backwards/Forwards variants, BrakeMigrationTorque and an **AI-only BrakeMigrationTorqueAI**, BrakePressure,
ATC/ABS/ASC strengths (with *AI variants) as CarObfFloat; the (de)activation rates, ABSSymmetryFactor,
PunctureThrottleMax, BurstThrottleMax are plain floats. `Car_BrakeEcu_ApplyBias` 0x144676250 turns bias +0x21d8 (0..1)
and pressure +0x21dc into per-wheel max brake torque (CarWheel+0xf8, CarObfFloat): bias 0.5 gives BrakeFront/BrakeRear,
lower bias blends towards the *Backwards pair, higher towards *Forwards, all x pressure. Live brake strength edits:
write the wheel torques, or the BrakeECU torques and re-run ApplyBias.

### SuspensionSystem, wheels {#suspension}

The reader fills per-name records with **plain floats**, except the XOR pairs (decoded with [+8]): Tyre records hold six
(DebrisPunctureMin/MaxSpeed, DebrisPunctureProbabilityAtMin/MaxSpeed, DebrisPunctureInvulnerabilityPeriod,
DebrisNextGenPunctureProbabilityMultiplier at record +0x70..+0xc0) and Brake records seven (Density, Conductivity,
RotorOutside/InsideRadius, RotorThickness, SurfaceLayerThickness, CoolingHoleRadius at +0x30..+0x90). numXTypes only
bounds the child loop. Tyre[name] (`CarSuspTyreType`: mass, inertia,
width, radius, grip multipliers, puncture block, UpgradeWearFactor +0x630, UpgradeTempFactor +0x634, downslope
factors +0x638..+0x64c, temperature-wear factor +0x654/+0x658), Brake[name] (`CarSuspBrakeType`: handbrake, heat
capacity, heat transfer coefficients, damage), Suspension[name] (`CarSuspCorner`: camber/castor/toe, steer pivot,
8 Height* splines, rest/render positions, Min/MaxHeight, RestLength, RideHeight, bump stops, RideHeightOffset,
Wheel*Factor, RollCentre, SpringConstant, ThirdSpring*, Slow/FastBump/Rebound damping, AntiRollConstant). Record
pointers: tyre types sys+0x38, brake types +0x48, suspension types +0x58 (two each, order = VTF order; 2025 VTFs list
Rear first). Wheels sys+0x70 RL, +0x78 RR (rear axle object +0x90), +0x80 FL, +0x88 FR (front axle +0x98), 0x480
bytes each. The two 0x1670 axle objects (`CarSuspAxle`, +0x90 with wheels 0/1, +0x98 with wheels 2/3) look like an
8-byte head plus a copy of the corner record (StepVehicle reads axle+0x15e4 = RideHeightOffset). Setup
(`Car_Tuning_SetSuspension` 0x1446a7be0, only under flags & 0x2000) writes live camber axle+0x1650, toe +0x1654 and
anti-roll constant +0x1664, choosing the rear axle by LocalRestPositionZ <= 0 (axle+0x1598). One call does one axle's
set (rear when its own axle +0x1598 <= 0, else front; front camber slider +0x228); .data1 0x15543f450 calls it for
[susp+0x90] then [susp+0x98]. Springs, dampers and ride height are not set
there; their setup path and the consumers of SpringConstant/damping are not mapped yet.

### Tyres {#tyres}

- Per-wheel tyre object (`CarTyreState`), four at physics-vehicle +0x60/+0x68 (rear) /+0x70/+0x78 (front):
  +8 TyreType record, +0x10 car, +0x8c wetness, +0xc0 compound runtime, +0xd0 wheel index, +0x88 wear 0..1.
- **Grip**: `Car_Suspension_SolveWheels` calls `Car_Tyre_GripScale` (handling_version > 6) and
  `Car_Tyre_MagicFormula` per wheel. GripScale = max(DownslopeScaleMinimum, wet loss x speed loss), using **AI-only
  factors** (+0x63c/+0x644) when car+0x124 == 1. Magic Formula curve = compound+0x1438 (load rows lerped by load;
  camber GripGain only at 0). Car_Parse_TyreModel precomputes, per load row, the slip angle of peak force (step search
  over 9 Magic Formula calls) into row+0x1c and curve+0x210; a live edit of Peak/UpSlope/DownSlope/Shape changes the
  force at once but leaves that peak slip stale until the compound is parsed again.
- **Wear** (`Car_Tyre_UpdateWear` 0x1446b79f0, from Car_Tyre_Step 0x1446b5720): inc = (AI ? obj+0x84 : 1) x
  obj+0x80 x UpgradeWearFactor x temperature factor x wetness spline x lerp(WearRateMin, WearRateMax, pressure
  ratio) x work / 1e6. radUpgradeTyreWear multiplies UpgradeWearFactor by 0.825 (scaled by progress), so it acts on
  every wear step with no re-init. AI cars get an extra multiplier the player never sees.
- **Temperature**: `Car_Tyre_UpdateTemperature` 0x1446b7b70 (UpgradeTempFactor read at 0x1446b7e1f); not mapped.
- VCMPD compound keys (Wear, Temperature, Pressure, Wetness, Residuum, TyreModel MNC ParametricData) are read by
  Car_Parse_TyreModel 0x144743900 with plain floats; only the pressure/wear-rate fields used by the wear step are
  placed in `CarTyreCompound.h` (L1). Compound switching (Car_Tyre_ChangeToNextCompound, ActivateCompounds) not read.

### R&D and other modifiers {#modifiers}

R&D attributes are VTF `Upgrade[radUpgrade*]` blocks replayed through the same Car_Read_<X>; Game_Read_VtfFloat in
upgrade mode applies `*`: v x ((k-1)p+1), `+`: v + k p, `-`: v - k p, `/`: v / ((k-1)p+1), p = progress 0..1.
Stock 2025 blocks in my lane:

| attribute | block ops | lands in | takes effect |
|---|---|---|---|
| FrontDownforce | FrontCoeffLiftMultiplier x1.26, FrontWingCoL1 x1.1 | F1Aero +0x1708, +0x1768 | multiplier: next step; CoL1: OnReset 0x2200 |
| RearDownforce | RearCoeffLiftMultiplier x1.26, RearWingCoL1 x1.1 | +0x1718, +0x17d8 | same |
| Drag | BodyDragCoeffMin x0.87, Max x0.89, Front/RearWingCoD1 x0.94/0.93 | +0x1728/+0x1738, +0x1798/+0x1808 | OnReset 0x80 / 0x2200 |
| DRS | m_aero_wing_rear_drag x0.9 | +0x1a8 | next step |
| ChassisWeightReduction | Chassis Mass -18 | Chassis +0x28 | OnReset 0x200 |
| ChassisWeightRedistribution | Chassis COMY +0.025, F1Plank OffsetY +0.025 | Chassis +0x68 | OnReset 0x200 |
| TyreWear | Tyre[Front/Rear] UpgradeWearFactor x0.825 | TyreType +0x630 | next wear step |
| Brakes | BrakeFront/Rear/Backwards/Forwards x1.4 | BrakeECU +0x2008..+0x2058 | when Car_BrakeEcu_ApplyBias 0x144676250 re-runs (writes wheel+0xf8) |

AI-only physics values found: BrakeMigrationTorqueAI, ASC*AI, SlipRatio*LimitAI, Wetness/SpeedDownslopeFactorAI and
the AI tyre-wear multiplier (CarTyreState+0x84). No AI grip/power/drag multiplier on the aero path.

### How to change handling from an addon (ranked) {#addon-chassis}

1. **Edit the VTF/VCMPD before load** (the rnd-tyres pattern): base values, plain-float suspension/tyre records, and
   new Upgrade blocks. No obfuscation, no re-init, works for every car using that package.
2. **Write the live struct after build**: plain-float records (CarSuspTyreType, CarSuspCorner, CarSuspBrakeType) and
   the F1Aero plain runtime fields (lift_scale +0x19a4, drag_scale +0x19a0, wing terms +0x19b8..+0x19c8) take effect on
   the next step; CarObfFloat fields need the live key; BodyDragCoeff*, wing Co*, Chassis Mass/COM/MOI need the
   component's OnReset (system vtable +0x18) with flag 0x80 / 0x80|0x2200 / 0x200 (F1Aero does nothing without 0x80), or the derived copies written directly.
   Note OnReset 0x80|0x1000 resets lift_scale/drag_scale to 1.0, so a write there must be repeated after resets. No
   other writer of +0x19a0/+0x19a4 was found in .xtls, so these two plain floats are the cleanest per-car
   downforce/drag knobs (T115 checks the store sites they feed).
3. **Hook a reader**: Car_Read_<X> runs for the base section and every Upgrade block; post-hook it to rescale.
4. **Hook the step**: MidHooks at site_Car_AeroStoreFront/RearDownforce, TotalLift, TotalDrag; a function hook on
   Car_Tyre_GripScale (return value = per-wheel grip multiplier). Not on Car_Tyre_UpdateWear (XOR-masked lea at
   0x1409d5443, see #addon); these sites miss cars on the vt+0x18 substep path.

Needs L3: T115 (downforce stores and front/rear naming), T116 (wing_open_fraction = DRS), T117 (tyre wear field),
T118 (live chassis mass after R&D).

### Cross-lane notes (car-c) {#lanes-c}

- car-a: component car pointer is system+8 (see #layout); CarLive fields seen here: +0x124 (== 1 selects every *AI
  tyre/brake value, so it is the AI flag), +0x128 (== 1 skips tyre wear), +0x132 bit 7 (tyre wear enabled), +0x88/+0x8c
  (divide the MF load before the formula). The physics vehicle passed to StepVehicle/SolveWheels has car at +0x18 and
  the four CarTyreState pointers at +0x60..+0x78; its +0x170..+0x17c are per-wheel heights.
- car-d: tyre wear setting: Car_Tyre_SetWearFactor 0x142ea2300 ("Wear factor set to %f on vehicle %d") is the likely
  writer of CarTyreState+0x80 (wear_scale); AI wear multiplier CarTyreState+0x84 is the place an AI-difficulty tyre
  model would land. Setup: Chassis COM-Z slider is F1Tuning +0x210 (0..1) lerped over the F1Tuning vtable +0x40 range.
- lead: `Game_Read_VtfSuspensionSystem` 0x1446aa090 is now `Car_Read_SuspensionSystem` (subsystem car).
  `Car_Parse_Chassis` 0x14462e6d0 is not the Chassis VTF reader (it registers collision callbacks); the reader is
  `Car_Read_Chassis` 0x144676730. The inbox line "front downforce F1Aero +0x19ec, rear +0x19f0" is swapped.

## Performance, setup, assists, damage and AI {#apply}

Lane car-d. Structs `CarPerfVehicle.h` (the career Upgrades object), `CarTuning.h`, `CarSetupValues.h`, `CarAssists.h`,
`CarDamageFrontWing.h`, `CarDamageSuspension.h`, `CarDamageBodyPart.h`.

### Performance chain: R&D attribute -> physics {#perf}

The R&D attribute names (EnginePower, FuelConsumption, FrontDownforce, ...) never index a physics
array. The chain is:

1. Career side (career lane, already L2): `upgrade_type` string -> type id 0..18 (name table
   0x14584f6b8) -> per-type progress 0x143526260 -> `Game_Rnd_FillVehicleUpgrades` 0x141238cb0 puts
   `map[type][0] = {progress, mass, bhp}` into the vehicle upgrade component (comp+0x28) ->
   0x14fa4deb0 -> 0x14fa63050(comp, type) names the block `radUpgrade<X>` from
   `k_Career_UpgradeBlockNameTable` 0x145803ce4 and tail-jumps to `Car_Upgrades_ApplyToVehicle`
   (0x144742c70) with rcx = CarLive+0x4a0, rdx = block name, xmm3 = perc = clamp(sum progress).
2. `Car_Upgrades_ApplyToVehicle(CarLive+0x4a0, name, flag, perc)` (decomp re/decomp/144742c70.c):
   - picks the asset: CarLive+0x4a8 `vehicle_asset`; when `CarLive.equal_performance` (+0x12c) == 1
     it uses the base asset `[asset+0x48]+0x28` instead (so equal-performance cars all replay the
     base car's blocks).
   - returns 0 at once when the block name is already in the applied-blocks vector
     (CarLive+0x4b8, begin +0x4c0, end +0x4c8, stride 0x28, name at +0x10): **a block is applied at
     most once per car build**; re-applying with a new perc does nothing until the systems are
     rebuilt.
   - walks the VTF root's `Upgrade` children (vt+0x20 child iterator, attribute `name`) for the one
     whose name matches (case-insensitive), pushes the name onto the applied vector, then for every
     section of the asset's section list (asset+0x30..+0x38, stride 0x40, name at +0x10) finds the
     same-named child of the block and the CarLive system whose type name matches
     (`systems[i]` vt+0x38 -> type, type vt+8 -> name), and calls the system's vt+0x08 with the
     section node while the globals `g_Car_UpgradeLerp` (0x145faa650, float, = perc) and
     `g_Car_UpgradeLerpActive` (0x146ce88a0, byte, = perc != 1.0) are set; both are reset to 1.0 / 0
     right after.
   - after the replay, for every system: vt+0x30(flag) then vt+0x18(0x20bd | (!flag << 9)). This is
     the re-init that recomputes derived values; **the game re-runs it itself after each block**.
3. The system's vt+0x08 runs its section reader (`Car_Read_<X>`, data-class slot 2), which reads
   every key by name through `Game_Read_VtfFloat` 0x14464e760. Keys missing from the block keep the
   current value (the reader decodes the XOR pair, lets Read_VtfFloat modify it, re-encodes).
   `Game_Read_VtfFloat(node, name, float *inout)` (decomp re/decomp/14464e760.c), key `<name value=v op=c>`:

   | op | lerp inactive (perc == 1 or base load) | lerp active (perc != 1) |
   |---|---|---|
   | `*` | x = v * x | x = ((v-1)*perc + 1) * x |
   | `+` | x = x + v | x = v*perc + x |
   | `-` | x = x - v | x = x - v*perc |
   | `/` | x = x / v | x = x / ((v-1)*perc + 1) |
   | `=` or no `op` | x = v (logs "Direct overwrite while upgrading" when lerp active) | same: NOT lerped |
   | other char | x unchanged, returns 0 | same |
   Missing key: returns 5, logs `Float "%s" NOT found.` (VEHICLE_ASSETS), x unchanged.

   Because ops apply to the current value, the order is base section -> each radUpgrade block once.

#### Which physics field each R&D attribute moves (2025 Ferrari, stock blocks)

| upgrade type / block | section/key | op, full-upgrade value |
|---|---|---|
| EnginePower `radUpgradeEngine` | Engine/RevsPowerSpline (15 vertices, replaced); FuelTank/FuelBsfcMin x0.9, FuelBsfcMax x0.895, FuelOptimalTorque = 550 (set, not lerped), FuelTorqueLinear x1.4 | spline + `*` |
| FuelConsumption `radUpgradeFuelConsumption` | FuelECU/FuelBurnScale{Lean,Standard,Rich,Max} x0.85; ERS Common/ExhaustPowerCoefficient x1.2 | `*` |
| ERS `radUpgradeERS` | ERS Common EnergyHarvestEfficencyMGUK x1.042, ExhaustPowerCoefficient x1.15, OvertakePowerEfficiency x1.35; Mode[0..3] gear modifiers SET (no op: 1e-7, 0.75, 0.8, 1) | `*` / `=` |
| FrontDownforce `radUpgradeFrontDownforce` | F1Aero/FrontCoeffLiftMultiplier x1.26, FrontWingCoL1 x1.1 | `*` |
| RearDownforce `radUpgradeRearDownforce` | F1Aero/RearCoeffLiftMultiplier x1.26, RearWingCoL1 x1.1 | `*` |
| Drag `radUpgradeDrag` | F1Aero/BodyDragCoeffMin x0.87, BodyDragCoeffMax x0.89, FrontWingCoD1 x0.94, RearWingCoD1 x0.93 | `*` |
| DRS `radUpgradeDRS` | F1Aero/m_aero_wing_rear_drag x0.9 | `*` |
| ChassisWeightReduction `radUpgradeWeightReduction` | Chassis/Mass - 18 kg | `-` |
| ChassisWeightRedistribution `radUpgradeWeightRedistribution` | Chassis/COMY + 0.025, F1Plank/OffsetY + 0.025 | `+` |
| TyreWear `radUpgradeTyreWear` | SuspensionSystem/Tyre[Front,Rear]/UpgradeWearFactor x0.825 | `*` |
| Brakes `radUpgradeBrakes` | BrakeECU/Brake{Front,Rear}{,Backwards,Forwards} x1.4 | `*` |
| TC/ES/MGUK/MGUH/CE/ICE/Gearbox durability `radUpgradeWear*` | Engine/WearTCRate, ERS BaseWearRateES, BaseWearRateMGUK, Engine/WearMGUHRate, WearCERate + WearCEDVibrationRate, WearICERate, GearBox/DamageRateMidShift: all x0.5 | `*` |

`Upgrade[very_high]` (Steering MaxSteeringLock = 0.41, AckermannSteering = -0.04363) is a non-R&D
block. The setup path 0x14f82f2b3 calls ApplyToVehicle with perc 1.0 and names from table
0x145803c68 (career lane: those names never match a block-name pointer; the VTF block name is
compared as a string, so a matching string name would still apply).

Mass and power reach the car through these blocks too. `Car_Upgrades_CalcNonLinearMass` 0x143535480
(decomp re/decomp/143535480.c) is a separate estimate {#mass}: on the team's Upgrades object
(CarPerfVehicle.h), linear = sum over type metadata records (+0x1a38..+0x1a40, stride 0x20: +0 type,
+4 mass at full) of mass_at_full * progress(type) (helper 0x1435352a0 -> Game_Rnd_CalcUpgradeTypeProgress);
f = linear / sum(mass_at_full); result = linear + f*(f*target - linear), target = Upgrades+0x50. Callers:
0x142f5a5d0 (adds FuelTank+0x100 fuel mass and Chassis Mass, XOR Chassis+0x98/+0xa0, then a lap-time-
style estimate 0x1429028f0: strategy / pace prediction) and 0x142962f10 (garage/programme UI). It does
not write the physics mass.

#### CarPerfVehicle correction
The "attribute array at +0xd10, stride 0xc40, count +0x1960" is the career `Upgrades` object's
UpgradeTree array: trees at +0xe0 (stride 0xc40), list at tree+0x30 (+0x110), count at tree+0xc30
(+0xd10), tree count +0x1960 (`site_Car_AttrArrayBase` loads rbx = r12+0xd10, the count, and the list is
rbx-0xc00). Rewritten in `structs/CarPerfVehicle.h`.

#### Performance mode {#perf-mode}
`Car_Assets_CreateSystemForPerformanceMode(asset, CarLive+0x380 container)` 0x144742790 (callers:
`Car_Chassis_Build` 0x1446296e6, .data1 0x1555bdf97) walks the asset's section list; for each section,
when `CarLive.equal_performance` (+0x12c) != 0 it takes the section from the base asset
(`[asset+0x48]+0x28`, "Creating/resetting system using base asset for equal performance") else from the
team VTF ("... for realistic performance"), and creates/resets the system with
`Car_Systems_FindOrCreate` 0x144759eb0. So equal performance swaps whole VTF sections (and the upgrade
blocks, step 2 above) for the base car's, at system build. `performance_class` / `performance_tier`
are read into the AI-config object (VehicleIdentity +0xcda/+0xcdb, entrants lane); class is the
candidate-pool car-class filter (session lane); no physics reader of the tier was found statically.

### Setup: F1Tuning {#tuning}

`structs/CarTuning.h` is the F1Tuning system (CarLive+0x590) with system-pointer offsets: data object at +0x18
(plain-float Min/Max ranges, struct offset = reader offset + 0x18), slider fractions from +0x210. Slider block:
`structs/CarSetupValues.h`. In the table below the CarTuning ranges are READER (data) offsets; add 0x18 for the
struct, e.g. DifferentialOnThrottleMultipleMin is reader +0x18 = CarTuning+0x30.

`Car_Setup_ApplyTuning(setupMgr, u64 *skipMask, setup)` 0x140c11550 (decomp re/decomp/140c11550.c):
car = setupMgr+0x28; tuning = car+0x590; slider block = setup+0xb0 if byte setup+0x158 else setup+0x08.
Per slider (skipped when its mask bit is set): fraction = clamp(step / stepCount, 0, 1) where stepCount
is the int at +8 of a per-slider global object (0x14605a6d8, 0x14605a700, 0x14605a728, 0x14605a730, ...);
the fraction is stored on the tuning system and the target field is lerp(Min, Max, fraction) from CarTuning:

| slider (block offset) | CarTuning range | target |
|---|---|---|
| front wing +0x00 (raw level, clamped to F1Aero max level XOR +0x1748/+0x1750) | - | F1Aero+0x19b4 level; +0x19b8, +0x19bc = (x*c2)^2 + x*c1 + c0 (XOR pairs F1Aero +0x1758/+0x1768/+0x1778 and +0x1788/+0x1798/+0x17a8) |
| rear wing +0x04 (clamp XOR +0x17b8/+0x17c0) | - | F1Aero+0x19c0; +0x19c4, +0x19c8 (pairs +0x17c8..+0x1818) |
| diff on-throttle +0x08 | +0x18/+0x1c | Differential+0xf0 |
| diff off-throttle +0x0c | +0x20/+0x24 | Differential+0xec |
| diff mid-corner +0x10 | +0x28/+0x2c | Differential+0xf4 |
| diff high-speed +0x14 | clamp +0x30/+0x34 | Differential+0xf8, tuning+0x2a0 |
| diff preload +0x18 | clamp +0x38/+0x3c | Differential+0xfc, tuning+0x2a4 |
| front / rear camber +0x1c / +0x20 | +0x50.. / +0x58.. | CarSuspAxle +0x1650 (CarSuspAxle = SuspensionSystem+0x90 or +0x98, front = the one whose +0x1598 > 0) |
| front / rear toe +0x24 / +0x28 | +0x60.. / +0x68.. | axle +0x1654 |
| front / rear suspension +0x2c / +0x30 | springs +0xa0.. / +0xa8.., third springs +0xb0.. / +0xb8.. | thunks 0x1446d7580 / 0x1446d7780: axle +0x165c spring, +0x1660 third spring (handling_version < 7: +0x1660 keeps axle +0x1610); fractions tuning+0x238 / +0x23c |
| front / rear ride height +0x34 / +0x38 | rest length +0x80.. / +0x88.. (< 7), ride height +0x90.. / +0x98.. (>= 7) | handling_version < 7: thunks 0x1446d7980 / 0x1446d7b00 write the rest length to axle +0x1648 (tuning +0x240/+0x244); >= 7: 0x1446d7c80 / 0x1446d8080 solve axle +0x164c from the springs, Chassis mass (XOR +0x98/+0xa0) and COM z (+0x158) (tuning +0x248/+0x24c) |
| front / rear ARB +0x3c / +0x40 | +0xc0.. / +0xc8.. | axle +0x1664 |
| brake pressure / bias +0x44 / +0x48 | +0xd0/+0xd4 | BrakeECU+0x21dc (pressure lerp), +0x21d8 (bias fraction) |
| brake migration +0x4c | +0xd8/+0xdc | BrakeECU+0x21fc; ramp start/end -> +0x2200 / +0x2204 (+0xe0.., +0xe8..) |
| tyre pressures +0x54..+0x60 | +0x114/+0x118 front, +0x120/+0x124 rear | thunk 0x1446d8620(tuning, wheel 3..0, 0) |
| chassis z-com +0x64 | +0x10/+0x14 | Chassis+0x158 |
| fuel load +0x68 | - | FuelTank+0x100 (kg) clamped to FuelCapacity (XOR FuelTank+0x98/+0xa0); FuelTank+0x10c bit0 cleared; skipped when FuelTank+0xf8 set |
| engine braking +0x6c | +0x40/+0x44 | Engine+0x252c |
| brake cooling front / rear +0x94 / +0x98 | +0xf0.. / +0xf8.. | wheels +0x134 (front = SuspensionSystem+0x80/+0x88, rear +0x70/+0x78) |
| brake duct front / rear +0x9c / +0xa0 | +0x100.. / +0x108.. | wheels +0x130 |

Ends with `0x1446b1ba0(SuspensionSystem)` (thunk to .data1 0x15543dfd0): suspension recompute after the
axle fields change. 10 callers of ApplyTuning (0x140c0a636, 0x140c0ab80, 0x140c0af2d, 0x140c108de,
0x140c11272, 0x140c113f4, 0x142a4165e, 0x142f44bf1, 0x142f44f7f, 0x142f45554), the setup manager and
session code. Setup presets: `Car_Load_SetupPreset` 0x140c04fd0 (car_setup.presets rows, ai/player
variants), car lane L1 from derived-keys.

How to change setup from an addon: (1) edit the F1Tuning ranges in the VTF (boot); (2) write
CarTuning Min/Max (plain floats, no XOR) and call ApplyTuning again (or let the next setup change call
it); (3) write the target fields directly after ApplyTuning (Differential+0xec..+0xfc, axle
+0x1650/+0x1654/+0x1664, BrakeECU+0x21d8.., Engine+0x252c, Chassis+0x158) then call 0x1446b1ba0 for the
suspension; (4) hook ApplyTuning's entry with a modified slider block (mask bits pick which apply).

### Assists: F1Assists {#assists}

`structs/CarAssists.h` (system offsets; the offsets in this paragraph are reader offsets, struct = +0x18). Reader `Car_Read_F1Assists` 0x1446cb880 (was `Car_PadAssist_LoadSteeringSplines`,
data-class vtable 0x145bfcbe0 slot 2), writer 0x1446cc750 (slot 3). Twelve 0x2a8-byte splines from +0x10
(pad steering rate vs speed, lock, traction control Throttle*VSRearPeakSlip{Ratio,Angle}Proportion,
ABS BrakeVSRearPeakSlipRatioProportion, wet variants, drift assist) and XOR-pair scalars from +0x1ff0
(MinimumSteeringLock, SteerDeadzone, SteerOut/InMultiple, ..., WheelInertia). Then name-formatted loops
for SteerModel{Speed,MaxLock,SteerInertia,InputStiffness,CounterSteerStiffness}0..2. Consumers:
`Car_PadAssist_DriftAssist` 0x1446cd9a0 and the pad-assist steering code (car lane L1). The BrakeECU also
carries ABS/TC splines (car-c). Assist toggles (TC off/medium/full, ABS on/off) select which curve is
used at runtime; the flag reads were not reached this round.

### Damage {#damage}

| system | data struct | reader / writer | storage |
|---|---|---|---|
| F1WingDamage (CarLive+0x5c8) | `CarDamageFrontWing.h` (data at +0x18: struct +0x28..+0x38) | 0x14465f580 (thunk to .data1 0x1553794c0) / 0x14465f450, vtable 0x145bf9030 | plain floats +0x10..+0x20 |
| F1SuspensionDamage (+0x5d0) | `CarDamageSuspension.h` (system offsets; reader offsets below) | 0x1446533e0 / 0x144653070, vtable 0x145bf8af0 | XOR pairs +0x10..+0x58, plain fracture limits +0x60/+0x64, linkage list |
| F1BodyDamage (+0x5e0) | `CarDamageBodyPart.h` (per part) | 0x144650010 / 0x14464fe90, vtable 0x145bf89a0 | per part: name, MaxDamage (int), AeroLoss +0x2c, DragGain +0x30, CoolantLoss +0x34 |
| F1TyreDamage (+0x5d8) | (car-c / tyre deformation) | `Car_Write_F1TyreDamage` 0x144657040, `Car_TyreDeformation_LoadNodeGraph` 0x144657290 | 1012 keys, mostly the deformation node graph |

Front wing runtime (`structs/CarDamageFrontWing.h`, the F1WingDamage SYSTEM at CarLive+0x5c8 =
game::FrontWing; its +0x28..+0x38 are the data object's SpeedThreshold..FullDetachScale): `Car_FrontWing_OnCollision` 0x14465fdc0 (L2) is gated by **CarLive+0x130 bit 0**
(collision damage on) and writes the aero factor F1Aero+0x199c = clamp((left+right)/max) only when
**CarLive+0x130 bit 1** is set; these two bits are the damage-setting switches to look at first (who sets
them is open). Damage = surface x +0x40 (impact divisor) x impulse x +0x2c. Left +0x48 / right +0x4c clamp at +0x38, side broken at +0x34 (visual state in 0x144631a50),
chassis damage +0x50. The engine power-factor function 0x1446db280 sets a "wing gone" bool from
[+0x54]/[+0x38] >= 1.

Earlier KB rows (L1): `Car_FrontWing_OnCollision` 0x14465fdc0 adds damage to the front
wing (fw+0x48/+0x4c, max +0x38) and to F1SuspensionDamage +0x138/+0x13c, and writes the damage ratio to
F1Aero+0x199c (front wing factor; rear +0x1998). `Car_State_Read_Snapshot` (was Car_TyreDamage_Update) 0x14470d760 reads TyreDmg,
BodyDmg, WingDmg and F1Aero. The body part AeroLoss/DragGain are the per-part downforce loss and drag
gain at full damage; their aero consumer is the next target (search the F1Aero step for the part list).

To change damage behaviour: edit the VTF sections (boot); or write the data fields (XOR-encode the
suspension ones); `F1WingDamage/DamageScale` and the per-part AeroLoss/DragGain are the knobs.

#### Detachable parts {#parts}

car-re, 2026-10-28, Steam build, read from the decompiles of 0x14465f620, 0x144660400 (with its `.data1` body
0x15537d4f0), 0x1446612a0 (body 0x15538e460), 0x144661440, 0x1446686d0 and the three predicates. This is the
data flow behind `car.parts.register`.

`Car_FrontWing_Init` (0x14465f620, L2) is slot 6 (+0x30 Init) of the F1WingDamage system vtable 0x145bf90a0
(pointer at 0x145bf90d0). It has no direct callers. `Car_Chassis_Build` calls every system's +0x30 with 0 at
0x14462973d, and `Car_Systems_Reset_Body` calls it with its own reset argument at 0x1555bdfea (after an upgrade
replay or a vehicle reset). `this` is the `CarDamageFrontWing` system (CarLive+0x5c8), and fw+8 is the CarLive. Each
call does the following:

- registers `Car_FrontWing_OnCollision` twice on the car (0x1446510d0, 0x144651f90, each with `{fn, fw}`);
- fw+0x90 = CarLive+0x570, the **F1Aero** system (it is not a "car model");
- fw+0x98 = +0x98 of the ConvexHull system's (CarLive+0x5b0) entry named `"FrontWing"` (0x145bf90e8). The name map is
  at hull+0xc0 and the entries at hull+0xe8..+0xf0, and fw+0x98 is 0 when there is no such entry. It is the wing body
  that `Car_FrontWing_OnCollision` filters on, not a node map;
- fw+0xa0 = [CarLive+0x558 Chassis]+0x3f0, the second body the collision filter accepts.

Only when reset == 0 **and** the part list is empty (fw+0xb0 == fw+0xb8) does it register 11 pieces, each through
`Car_Register_DetachablePart_Thunk(fw, {predicate, fw}, &Game_Get_ModelNodeId(name), params, k)`. Then it puts the
centre predicate `{0x144660380, fw}` into the hull entry at +0xa0/+0xa8. A later Init (reset, upgrade replay) finds the
list full and registers nothing, so the pieces are registered once per car. There are three predicates, not one:

| predicate | condition (true = off) | pieces (k) |
|---|---|---|
| `Car_FrontWing_IsCentrePieceOff` 0x144660380 | k x +0x38 <= +0x48 + +0x4c | frontspoiler_fm (1.0), frontfin_fr (0.6), frontfin_fl (0.75) |
| `Car_FrontWing_IsRightPieceOff` 0x1446603d0 | k x +0x34 <= +0x4c, or +0x38 <= +0x48 + +0x4c | frontwing_fr_01 (0.25), frontwing_fr_02 (0.5), frontflap_fr (0.75), frontwingmoving_fr (0.5) |
| `Car_FrontWing_IsLeftPieceOff` 0x1446603a0 | k x +0x34 <= +0x48, or +0x38 <= +0x48 + +0x4c | frontwing_fl_01 (0.25), frontwing_fl_02 (0.5), frontflap_fl (0.75), frontwingmoving_fl (0.5) |

+0x34 is SideDetachScale, +0x38 FullDetachScale, and +0x48/+0x4c the left/right damage (`CarDamageFrontWing.h`). The
two fins use the centre predicate even though their names end in `_fr`/`_fl`. All 11 pass the same 0x70-byte params:
int 2 at +0, float 0.4 at +4, a 4x4 identity at +0x10, and 0.05 three times at +0x50.

`Car_Register_DetachablePart_Thunk` (0x144660400, L2) is a jmp into `.data1` 0x15537d4f0. Its body:

1. calls `Car_Add_DetachablePart_Thunk` (0x1446612a0 → `.data1` 0x15538e460) with (fw, cb, &id, params, k). That
   function calls `Car_Create_DetachablePart([fw+8]+0x2c8, cb, &id, params, k)` and, on success, appends the part to
   fw+0xb0..+0xb8 (cap fw+0xc0, grown through the allocator at fw+0xa8, vtable +0x18/+0x38). It returns part != 0;
2. if step 1 failed, it returns. No part exists when the car's model has no node with that id (0x143cfc8f0 returns -1);
3. otherwise it registers sub-pieces `<name>_001`, `_002`, ... (format `%s_%03d` at 0x145bf4637, with the name taken
   from the name table [0x14618ce08]+8 under the lock 0x14618ce18) with the same cb and params, until one fails. Each
   sub-piece's k is rewritten. With d = min(0.2, 2 x the distance between the sub-piece's and the main piece's node
   translations (part vtable +0x88 returns part+0x60, so the translation is part+0x90)), k becomes 1 - d when k == 1.0
   and min(0.99, k + d) otherwise. The sub-piece's +0x1f4 byte is set to 1.

The main piece keeps k as passed. V1191's transform applies to the sub-pieces only.

Part object (`Car_Create_DetachablePart` 0x144661440, allocated 0x200 bytes through the car part manager CarLive+0x2c8
{allocator, car, vector allocator, begin +0x18, end +0x20, cap +0x28}). The fields are:

- +0 vtable 0x145bf94d0, +8 allocator, +0x10 car;
- +0x18/+0x20 predicate fn/ctx;
- +0x30 k, +0x38 detached byte;
- +0x48 name id, +0x4c model node index, +0x50 `<name>_shadow` node, +0x54 `<name>_water` node;
- +0x60..+0x9f the node transform, +0xa0 shape kind (2, or 6 when the ConvexHull has an entry of that name with
  +0x70 == 2);
- +0x130 byte (0 at create), +0x1f0 float timer, +0x1f4 byte.

`Car_DetachablePart_Update` (0x1446686d0, slot 3 of 0x145bf94d0) runs every frame with dt:

- when +0x130 >= 100, or when `fn(ctx, k)` returns true, it sets +0x38 = 1 and calls slot +0xa0(part, 1), which
  detaches the piece;
- an already detached piece runs a 1.5 s timer at +0x1f0 before 0x144668830(part, 1).

So k is the predicate's multiplier. A piece comes off once the damage reaches k x the detach scale.

`Car_Create_DetachablePart` has a second caller, 0x144655640, which `Car_Chassis_Build` calls 11 times (from
0x14462b147 on). It creates parts directly in the same manager with +0x1f4 = 1. This is a second, non-front-wing part
source (L0: probably the suspension pieces), not read further.


#### Stock values and an external cross-check {#damage-check}

Stock 2025 Ferrari (`ferrari.erp`, checked by the lead): F1WingDamage SpeedThreshold 3, DamageScale 0.0001,
ChassisDamageMultiple 0.3, SideDetachScale 0.5, FullDetachScale 1. F1SuspensionDamage SpeedThreshold 7,
FrontDamageScale 0.001, RearDamageScale 0.0001, WheelDetachScale 1, PartsDetachScale 1 (raised by the reader to
1.5 = 1.5 x WheelDetachScale), LinearFractureLimit 50, AngularFractureLimit 30.

`Car_SuspensionDamage_OnCollision` (0x144654d10) is the F1SuspensionDamage impact callback: it reads the speed
threshold and the front or rear damage scale on every hit, so a live write reaches the next impact. Its corner flags go
to the matched SuspensionSystem wheel ([CarLive+0x580]+0x70..+0x88, matched by +0x2b4/+0x2b8): wheel+0x20 |= 4 for a
CAR_ collider, else 8, with the impact speed at wheel+0x24/+0x2c (gated by CarLive+0x133 bit 1 and +0x130 bit 3). The front wing
does the same in `Car_FrontWing_OnCollision`, and SideDetachScale (+0x34) is read by the visual side-broken state
(0x144631a50).

Addons can wrap it: it is the func binding `Car_SuspensionDamage_OnCollision` (signature
`car.suspension_damage_on_collision`, prototype `void(void *susp_damage, void *contact)`, experimental), at RVA
0x4654d10 on Steam and 0x464afa0 on EA (V9284 agrees). When its reset argument is 0, the system's Init 0x144653ad0
hands `{fn, this}` (lea at 0x144653bb6) to the listener remove thunk 0x1446510d0 (body 0x155338ff0) and then to
`Car_Collision_AddListener` 0x144651f90 (body 0x155339950), which work on the 15 `{fn, ctx}` slots at CarLive+0x9db8. `Car_Collision_Dispatch` calls each slot as `fn(ctx, &contact)` at 0x14473fd28, between
EnterCriticalSection and LeaveCriticalSection on [CarLive+0xf0], and ignores the return. So `susp_damage` is the
F1SuspensionDamage system (CarLive+0x5d0, its +8 is the CarLive) and `contact` is the dispatcher's stack record: +0
CarLive, +8 the car's own hit body, +0x18 contact point pointer, +0x20 normal pointer, +0x30 the other collider (dword
tag `CAR_` for a car), +0x38 the f32 that the front or rear damage scale multiplies, +0x40 a byte.
`Car_FrontWing_OnCollision` is registered and called the same way.

Will (william09leo, author of Heavier Damage) shared private findings from a live front wing damage test addon. They
agree with this map on the data vtables of F1WingDamage, F1SuspensionDamage, FuelECU, Engine and EnergyRecoverySystem,
the 4-slot data class, the data object at system+0x18, the XOR pair layout, the wing and suspension damage fields, the
1.5 clamp and the ERS Common field order. His in-game test saw the game write the VTF values back into the wing
objects, sometimes for one car and sometimes for all 22: that matches the reset path in #vtf (a single-vehicle reset
request, and the resettle of every car), which T120 pins down.

### AI {#ai}

AI cars run the same VTF, the same radUpgrade replay and the same Car_Physics_* step. No AI grip, power
or drag multiplier keyed to the difficulty slider exists as a string, VTF key or reader. The AI-only
physics knobs found are all data:
- BrakeECU (2025 Ferrari): `ASCStrengthForAI` 0.0, `ASCMaxBrakeAI` 0.5, `ASCMinYawRatioAI` 0.25,
  `ASCMaxYawRatioAI` 0.75 (active stability control for AI cars; strength 0 = off) next to the player
  ASC keys `ASCStrengthOnFullATC`/`ASCMaxBrake`/`ASCMin/MaxYawRatio` (+ Wet variants), and
  `BrakeMigrationTorqueAI` 450. Reader `Car_Read_BrakeECU` 0x1446752f0 (sites 0x144675c7f,
  0x144675cb3, 0x144675947), car-c's struct.
- Tyre: `SlipRatioMinLimitAIDegrees` / `SlipRatioMaxLimitAIDegrees` (-90/90) read by
  `Car_Read_SuspensionSystem` 0x1446aa090; the reader also asks for `WetnessDownslopeFactorAI`
  (0x1446ab322) and `SpeedDownslopeFactorAI` (0x1446ab350); the 2025 VTF carries only the non-AI `WetnessDownslopeFactor` / `SpeedDownslopeFactor` (0.1), so the AI variants take their defaults.
- Simulation rate: `Vehicle.FineUpdateRateAI` and `Vehicle.TyreContactSubStepCycleAI` read by
  `Game_Read_VehicleConfig` 0x14463a800 (AI cars sub-step the tyre contact less often).
- Career: `difficulty_mass_adjustment_scalar` (RnDDepartmentConfig +0x94, enable +0x98, read by
  `Career_Load_RnDTweakables` and 0x140a36730): the only AI-vs-player mass handicap.
- Scenario rules: 0x14139d600 (`Car_Parse_ScenarioRules`, L1) parses rule rows with `ai_level` +
  `vehicle_mass`, distance-based `difficulty_at_min/max_distance` + `vehicle_mass_at_min/max_distance`
  (min_distance/max_distance), `wetness_grip_scale`, `restrict_damage_option`, `trigger_once`, `vo_line`:
  scripted (story/challenge) AI level and car mass by gap to the player. Consumer not reached.
- `AiHandlingConfig` (ai lane) is pure driving-model data (race-craft); its `traction_control` group
  (grip_atc_strength, peak_slip_factor, ...) drives the AI's own throttle controller, not the physics.
- The AI difficulty slider's application is still not located (ai lane open item).

### Open (car-d)
- T for: does the upgrade replay run per session load or once per asset (watch VEHICLE_UPGRADES
  'Applying %s upgrade to vehicle %d. (perc0to1: %f)' and the applied-blocks vector CarLive+0x4b8).
- T for: Car_Setup_ApplyTuning call timing and the slider block values (is the +0xb0 copy the
  parc-ferme/AI copy?).

### Cross-lane notes (car-d)
- car-a: CarLive+0x4b8 applied-upgrade-block vector (alloc/begin +0x4c0/end +0x4c8, stride 0x28, name
  +0x10); systems vt+0x08 = apply-section (upgrade replay), vt+0x30(flag) + vt+0x18(0x20bd|..) = re-init
  after an upgrade; the lerp globals 0x145faa650 / 0x146ce88a0.
- car-c: setup targets on the CarSuspAxles SuspensionSystem+0x90/+0x98 (+0x1650 camber, +0x1654 toe,
  +0x1664 ARB, +0x1598 axle position), wheel +0x130 duct / +0x134 cooling holes, BrakeECU +0x21d8 bias,
  +0x21dc pressure, +0x21fc..+0x2204 migration, Chassis+0x158 COM z, F1Aero +0x19b4..+0x19c8 wing level
  and wing coefficients.
- car-b: Differential +0xec..+0xfc (setup), Engine+0x252c (engine braking), FuelTank+0x100 fuel mass,
  FuelTank+0xf8 / +0x10c flags.
- game lane: Game_Read_VtfFloat op semantics above (raise to L2).

### Career side: fitting upgrades to a rating (Car_Fit_UpgradesToRating) {#fit}

Correction (car-d, 2026-10-28): the "car pointers" and "attribute elements" below are the career Upgrades object's
upgrade trees (`CarPerfVehicle.h`), not a physics array; the physics change comes from the `radUpgrade*` block replay
(see #perf).
`Car_Fit_UpgradesToRating` (0x143514670, cxx `game::upgrades::EngineSuppliers::fitUpgradesUpToRating(const Upgrades&, const DepartmentData&, DriverStatId)`) is the anchor. Fully data-driven, no fixed-4:
1. Walks the Upgrades robin_hood map (`[upgrades+0x38]` ctrl / `[+0x40]` slots, gate `[upgrades+0x48]`). Each slot is an upgrade GROUP (gate `[group+0x190]`); inner robin_hood map at `[group+0x180]/[+0x188]` yields upgrade ITEMs.
2. For each item it collects, from `DepartmentData.effect_defs` (inline `void*[0x180]` at dept+0, count at `dept+0xc00`), the `UpgradeEffectDef`s whose `dept_group_enum` (+0x7c) == the group byte `[group+0]` and `item_group_enum` (+0x72) == the item byte `[item+0]`. Grouping is `==`, never index-into-array[4].
3. Rating lerp: sums `UpgradeEffectDef.effect_value` (+0x64) over the matched set; ratio drives `min=[item+0x10]`, `max=[item+0x14]`, cap `[item+0x18]` (the upgrade/bound object's rating bounds). A sub-pass splits by the byte at `[effdef.variant_tag_ptr-8]` (0/3/6). Logs on channel "UPGRADES/V" (hash 0x6a15fc5f).
4. Write-back: owner = `*(upgrades+8)` (team/engine-supplier). For each car pointer in owner `[+0x298 begin .. +0x2a0 end]` (stride 8): if `*(veh+0x1960)!=0`, iterate `count=*(veh+0x1960)` attribute elements at base `veh+0xd10`, stride **0xc40**. Each element holds a fitted-effectdef pointer list (list at element+0x000 cap 0x180, count at element+0xc00); `Car_Find_Qword`(thunk 0x1449e2850, a SIMD std::find-over-qwords into .data1 body 0x1562c20a0) searches it for the current effect-def. `Car_Apply_UpgradeToVehicle` (0x1435173e0) then applies: reads the attribute via `Car_Read_VehicleAttribute` (0x14351df60) and stores results into `UpgradeEffectDef+0xa4/+0xa8`; `Car_Reset_LastFittedUpgrade` (0x143517d80, `resetLastFittedUpgrade`) runs per fitted attribute.
   Hook sites: `site_Car_AttrArrayIter` (0x143514c50, r12=CarPerfVehicle, r14=effect-def) and `site_Car_AttrArrayBase` (0x143514c64).
`Car_Aggregate_UpgradeStats` (0x1409e3fb0; decompiled at entry 0x1409e6000) is a second consumer that re-reads `upgrade_type` + `bhp_adjustment`/`mass_adjustment` by name — string-driven, not a dept[4] index.
No static callers of the apply path (indirect/encrypted dispatch); role proven by the in-binary pretty-function string (`s_Upgrades_FitUpgradesUpToRating`) + decompiled data flow.

## Changing handling from an addon {#addon}

Ranked across the lanes. Each system section above has its own detail (#addon-object, #addon-chassis, powertrain and
setup notes).

1. **Edit the VTF or VCMPD before load.** Every create, reset and upgrade replay reads from the parsed resource, so a
   change there reaches every car built from it, with no obfuscation and no re-init. Upgrade blocks scale with R&D
   progress. Equal-performance cars read the base VTF instead.
2. **Write the live systems after `site_Car_LiveCarBuilt`** (one hit per car; rdi = CarLive), or walk
   `g_Sess_VehicleTable`. Plain fields take effect on the next step. `CarObfFloat` fields need the live key. Derived
   values need the owner re-run with the right flag combination (each Reset gates on its system bit first): Engine
   Init (vt+0x30) or Reset 0x10|..., GearBox Reset 0x20|... (0x220), F1Aero OnReset 0x80|... (0x80 / 0x80|0x2200),
   Chassis OnReset(0x200), `Car_BrakeEcu_ApplyBias`, the suspension recompute 0x1446b1ba0, or the system's vt+0x18(-1). Any
   reset re-reads the VTF and any upgrade replay runs Reset(0x20bd) on every system, which also forces fuel mode
   Standard and sets lift/drag scale back to 1.0: re-apply after both.
3. **Hook a reader** (`Car_Read_<X>`, data vtable +0x10). It runs for the base section and each upgrade block
   (check `g_Car_UpgradeLerpActive`), and is shared by every car: filter on the system's car at +8.
4. **Hook the step** at the `site_Car_*` sites (engine drive torque, ERS gear modifier, fuel flow and mass, aero
   downforce/lift/drag stores, tyre grip scale), or swap a car's per-phase `{fn, ctx}` callbacks at
   +0x4f8..+0x528 (data writes, per car). Never hook `Car_Tyre_UpdateWear`: 0x1409d5443 holds an XOR-masked lea of it
   (looks like an integrity table); change wear through the data (UpgradeWearFactor, CarTyreState +0x80/+0x84).
   Hooks on `Car_Physics_StepVehicle` miss cars on the vt+0x18 substep path (byte 0x146ce89d8 != 0 or
   CarLive+0x128 == 1). `.data1` bodies (the `_Thunk` targets, and the bodies of the F1Tuning, FuelECU, GearBox, Turbo,
   WheelProxies and DebugSystem factories, which are 5-byte jmps) are not hookable.

Per car and team: a system's car is at +8, `CarLive.vehicle_id` (+0xe8) indexes `g_Sess_VehicleTable`, and the
participant link is likely `EntrantRecord+0x82` (T121). No pointer from CarLive to the AI config or VehicleIdentity was
found yet.

## Runtime tests {#tests}

Filed by the 2026-10-28 lanes; the lead ran them in one Grand Prix session on 2026-10-28 (results in #runtime and in
each test file):

- T112 (partial): does `Car_Upgrades_ApplyToVehicle` replay each block once per car build, and with what perc for a career car.
- T113 (pass): when `Car_Setup_ApplyTuning` runs, for which cars, and which slider block it reads.
- T114 (not exercised, no setup change): setup slider +0x44 / +0x48 are brake pressure / bias.
- T115 (partial): the F1Aero front and rear body lift stores (+0x19f0 front, +0x19ec rear; coefficients, not forces).
- T116 (deferred): F1Aero +0x19a8 is the DRS wing-open fraction.
- T117 (partial): CarTyreState +0x88 is the tyre wear UDP reports.
- T118 (pass): live Chassis mass after R&D weight reduction.
- T119 (pass): when a CarLive is built and torn down (session load, restart, flashback, garage).
- T120 (pass): which actions take the VTF reset path.
- T121 (partial): `vehicle_id` vs participant index, and its stability.
- T122 (pass): Engine rotation_rate and throttle at the drive-torque site.
- T123 (deferred, no driver): who calls `Car_FuelEcu_SetMode_Thunk` for the MFD fuel mix.
- T124 (pass): fuel mass burn per step.
- T125 (partial): `ers_mode` follows the MFD ERS mode.
- T126 (answered): which Bink audio track ids the recap video opens in English (#runtime).

Still open statically: who sets the CarLive +0x130 damage bits, what reads `performance_tier` in physics, how the AI
difficulty slider is applied, the TC/ABS toggle reads, the Turbo pressure update, ERS harvest, the diff torque code,
where career PU wear is loaded into the Engine wear fields, the Steering consumers, the spring/damper/ride-height
consumers, the tyre temperature model, the full VCMPD runtime layout and compound switching.

## Runtime (2026-10-28) {#runtime}

One session on the Steam build (d873a1c8), run by car-lead: profile `cartrace` with only the trace plugin (11 hooks,
all installed, none rejected; the plugin's first real-game run), a new Grand Prix (F1 2025, Ferrari/Leclerc,
Australia, race only; the game took the idle player's car over and the AI drove it), the podium, back to the menus,
then the start of Braking Point 3 (the recap video). Two gdb snapshots of every car (on the grid, about 40 s into the
race) and a Bink trace ran beside it. Evidence: `re/traces/T112..T126/2026-10-28-gp/` (each has a RUN.md).

Lifecycle of the cars:

- **Built once, at load.** 21 CarLive objects for a 20-car race: ids 0..20 in create order, each registered
  (`site_Car_RegisterVehicleId`) right before its build (`site_Car_LiveCarBuilt`). Ids 0..19 have 23 systems and
  handling version 8. Id 20 is the safety car: 1734 kg, no ERS, 20 systems, handling version 1. Nothing rebuilt a car
  during the race (T119, T121).
- **Two all-vehicle resets during loading.** About 4.3 s after the build, the all-vehicles reset 0x14f824410
  (`Car_Systems_Reset_Thunk` called at 0x14f824536) re-read the VTF into the 20 race cars (not the safety car) twice,
  half a second apart (outer callers 0x1436ab400, then 0x142d5b70e). After each one every race car got the tier block
  `medium` (perc 1.0) through `Car_Upgrades_ApplyToVehicle` and a `Car_Setup_ApplyTuning` (return 0x142f44bf5) (T120,
  T112, T113).
- **Per-vehicle reset on a controller change.** `Car_Reset_VehicleSystems` ran for id 19 three times, at load (just
  before the all-vehicle resets), around the race start and around the finish, always from the controller-change body
  0x1519eb350 (return 0x1519eb3e9). That matches the game handing the idle player's car to the AI and back, so id 19
  is most likely the player's car. Each reset replayed `medium` and ApplyTuning (return 0x142f44f83) (T120).
- **Upgrades after a reset.** The applied-upgrade vector (CarLive +0x4c0..+0x4c8) was empty before every replay, and
  `site_Car_UpgradeSectionApplyCall` never fired, so `medium` changed no section in this session. No `radUpgrade`
  blocks in a Grand Prix: the career part of T112 needs a career session.
- **Setup.** Besides the reset paths, `Car_Setup_ApplyTuning` ran once for each race car at the first setup apply
  (return 0x140c0ab84) and 5 more times for id 19 (return 0x140c0a63a) around the race start, finish and menus,
  alternating the slider block (setup+0x158 = 0, then 1). Two setups show up: the AI one (brake pressure 1.0, diff
  on-throttle 0.456) and id 19's own (pressure 0.97, diff 0.189, 10 kg fuel in the first apply) (T113).
- **Torn down at session end.** 21 `site_Car_VehicleTableRemove` hits, the count going 20 down to 0 (the last car
  leaves first) (T119).
- **Not exercised:** session restart, flashback, return to garage, reset-to-track (`Car_Physics_ResettleVehicles_Body`
  stays L1), DRS, a setup change, the MFD fuel and ERS modes. The only `Car_FuelEcu_SetMode_Thunk` calls were on the
  safety car through the pose path (mode 2, then 1).

Live values read from the snapshots: Chassis +0x150 mass includes the fuel (803 + 13.217 = 816.21 kg) (T118); fuel
mass falls during the race (13.217 to 12.766 kg for car 0) and fuel flow is non-zero on throttle (T124); engine
418.9 rad/s idling, up to about 11600 rpm, throttle 0..1, gear index live (T122); ERS mode 1 or 2 on AI cars, 4 MJ full
store (T125); F1Aero +0x19f0 / +0x19ec are lift coefficients and +0x19e8 the drag force (T115, #f1aero); tyre wear
+0x88 rises during the race (T117). `is_ai` (+0x124) read 1 for every car, the player's included: it selects the AI physics
rate, but it is not a confirmed "AI-controlled right now" or "AI entrant" flag; never use it to pick the player.

Bink audio tracks (T126, the Korean patch question): `videos/2025_recap/F125_cinematic_recap.bk2` reports 30 track
ids, index → id 0 → 10, 1..8 → 11..18, 9..17 → 20..28, 18..29 → 40..51. The game reads ids from several callers; the
pair 0x143f8b46c then 0x143f8b54c walks indexes 0..18 only. With the game in English it opened index 9 (id 20), index
0 (id 10) and index 18 (id 40) (final opens from 0x143f8b5d2). Reading: id 10 is the always-on bed and id 40 the
English voice track, so the other languages sit in ids 11..28; what id 20 carries is not known. The language → id
table is in the selector around 0x143f8b46c..0x143f8b5d2 (decompile it for the Japanese and Korean ids). A core hook
on BinkOpenTrack can swap the requested index for the one holding id 40. The UI wipe videos have one track (index 0,
id 0).

## Live physics / telemetry vehicle {#live}

`Sess_Veh_UpdateWheelMap` (0x142e90560, session lane) walks the manager's per-vehicle list `[mgr+0x110 ..+0x118]`
(`CarVehicleManager.h`). Each item's `+0x20` is the chassis-runtime component (`CarPhysicsRecord.h`), and that
record's `+0x58` is the `CarLive`. So `[[[veh+0x20]+0x58]+0xe8]` is `vehicle_id` (the old `CarWheelState` reading of it
as a tyre-set index was wrong; that struct is folded into `CarLive.h`), and `[[[veh+0x20]+0x58]+0x580]` is the
SuspensionSystem with the four wheels at +0x70/+0x78/+0x80/+0x88 (RL, RR, FL, FR; see #suspension). The four
averaged wheel floats at `wheel+0x88` feed the robin_hood map<u8,float> at `mgr+0x148`; the 26th entry triggers the
32 → 64 rehash in the B26h heap abort. The per-vehicle item's telemetry region `+0x9c70..+0x9ca8` (0x78-stride record
vector at +0x9c90) is unmapped.

## Vehicle package row {#vehicle-package}
`Game_Read_VehiclePackageRow` (0x142504a90, game lane) parses `entrants.vehicles.vehicle_N` into
`VehiclePackageRow`: stat_id (+0, init 0xff), strings (+0x08 resource_path, +0x18 specialisation, +0x28
blob_shadow_decal, +0x38/+0x50 asset groups / ai_package_folder / driver_animation_set), and performance-
relevant flags +0x60 has_cockpit_halo, +0x61 is_story_vehicle_package, +0x62 is_equal_performance_vehicle_package,
+0x63 is_26_aero_regulations. Caller is plaintext .data1 code at 0x14cca1269.

## Attribute unit-type vocabulary (career upgrade types)
`k_Car_AttrUnitTypeNames` (0x14564a168) is the ordered unit-type name blob: EnginePower, FuelConsumption,
ChassisWeightRedistribution, ChassisWeightReduction, FrontDownforce, RearDownforce,
ControlElectronicsDurability, EnergyStoreDurability, GearboxDurability, InternalCombustionEngineDurability,
MotorGeneratorUnitHeatDurability, MotorGeneratorUnitKineticDurability, TurboChargerDurability (Drag, Brakes,
TyreWear, DRS, GeneralMaintenance strings nearby). This is the `affect_on_vehicle_unit_type` vocabulary. The
R&D DB model (gamedb) exposes these via `upgrade_type_metadata` with `bhp_adjustment_fully_upgraded`,
`mass_adjustment_fully_upgraded`, `performance_rating_base/fully_upgraded`. `g_Car_AttrDef_EnginePower`
(0x145650168) is EnginePower's AttributeDefinition dispatch table.
NEEDS RUNTIME TRACE (L3): the exact attribute-NAME -> attribute-array-ELEMENT-index mapping and attr_count.
The name->small-int resolution is the FNV unit-type switch near 0x14353a700 / 0x1417f8311 (per scratchpad);
reading `attr_elem`'s header fields and which element each unit type lands in needs a live session trace.

## Older catalog (pre-campaign, game lane L1) {#older}

Kept for the function pointers it lists; the sections above supersede its claims.

### Setup / suspension / physics rig (existing game-lane L1, performance-relevant)
- `Game_F1CarSetupDataManagerBase_DeserialiseBindings` 0x140c0c0c0 — car setup (aero/suspension/gears/brakes) bindings deserialise.
- `Game_Read_VtfSuspensionSystem` 0x1446aa090, `Game_Read_VtfFloat` 0x14464e760 — VTF physics parameter readers (Chassis COMY/COMZ, Tyre/TyreRadius, Suspension Camber/RideHeight/LocalRestPosition).
- `Game_Build_SuspensionRig` 0x14462d090, `Game_Find_SuspensionNodes` 0x14475c9a0, `Game_Find_WheelNodes` 0x14462bc90, `Game_Create_CarPartProxies` 0x144635360 / `Game_Create_CarPartProxy` 0x144636150.
- `Game_Setup_FrontendCarFromVtf` 0x1428fe140 — cosmetic (visual height/camber on a render node), not physics.
DB keys: car_setup, SetupData, GearRatio, differential, brake_bias strings exist (gamedb) — setup is data-driven.

### Tyre / ERS / fuel / engine / damage surfaces (catalog; mostly existing game-lane L1)
- Tyre: `Game_TyrePerformanceRuleManager_*` (0x1415291c0 OnSpectatorChanged, 0x14152a9f0 OnSectorCrossed, 0x14152ad40 OnAWSPanelOpened, 0x14152adf0 OnPitted, 0x14152aec0 OnTerminalDamage); `Game_TyreAllocationComponent_Ctor` 0x140d055b0 / `_SendNetMessage` 0x140bd1ac0; `Game_HudTyrePerformanceViewModel_SetToOpen` 0x14152b5f0. Strings TyreModel/TyreTemperature/tyre_temperature/tyre_wear present.
- ERS/fuel/engine: strings FuelMix/fuel_mix, ers_mode, EngineMode, PowerUnit, EnergyStore*, EngineWear present (VM/config keys); no dedicated function pinned yet — candidates for a decomp+trace pass.
- Damage/reliability: strings DamageComponent, vehicle_damage, reliability present; `Game_VehicleResetToGarageComponent_Trigger` 0x142f33d80. Not yet decompiled.

### Open / needs runtime verification
- attr-name -> attr-array-element index map and attr_count (L3 trace at site_Car_AttrArrayIter).
- live-vehicle per-wheel float semantics (temp/wear/load) and the +0x9c90 telemetry record layout.
- tyre wear/temperature, ERS deployment/energy-store, fuel-mode, engine-mode, damage/reliability writer functions (decomp from the component ctors + the live physics child at [veh+0x20]+0x58).

## Verification {#verification}

Verification round 2026-10-28 (verifiers car-v1..car-v4, `verify.tsv` V1186..V1342): 157 rows, 130 agree, 27 partial,
0 disagree. Every partial finding is applied to the rows, the structs and this map at the item's existing level, each
row noting the verify id ("per V####").

## Inbox
- 2026-10-19 13:33 career-re: career-re: 0x1409e3fb0 (Car_Aggregate_UpgradeStats) full field reads for the effect-def/attribute loader: available, attributes, default_effects; per-attribute default_value/min_value/max_value/use_clamp_min/use_clamp_max/restriction/statId/network_serialisable; upgrade_fitter_adjustment/upgrade_fitter_ratio/replay_serialise_mods; per effect-type upgrade_type + bhp_adjustment_fully_upgraded + mass_adjustment_fully_upgraded. Reached via vtable slot 0x145665c20. These build the effect-def objects (byte +0x7c, effect float +0x64) your Car_Fit_UpgradesToRating consumes.
- 2026-10-19 13:33 career-re: career-re: R&D load/economy side is in the new career lane. Car_Fit_UpgradesToRating consumes RnDDepartmentConfig (structs/RnDDepartmentConfig.h, filled by Career_Load_RnDTweakables 0x14353cea0 at [EngineSuppliers+0x68]) for the rating bounds/targets, and UpgradeRecord (structs/UpgradeRecord.h, built by Career_Load_UpgradeRows 0x143539bc0). The DepartmentData effect-def count [dept+0xc00] and per-vehicle attr array (+0xd10 stride 0xc40 count +0x1960, writer thunk 0x1449e2850) are yours.
- 2026-10-19 14:01 entrants-re: the per-vehicle AI-config object built by Sess_Load_VehicleAiConfig (0x1425027e0) carries an identity byte block at +0xcd8 package_stat_id, +0xcd9 engine_supplier_stat_id, +0xcda performance_class, +0xcdb performance_tier (structs/VehicleIdentity.h, entrants lane). engine-supplier = vehicle 'engine' label -> entrants.engines row -> stat_id; vehicle_package = 'vehicle_package' label -> entrants.vehicles.vehicle_packages row -> stat_id. This object likely overlaps your vehicle/CarPerfVehicle object - worth cross-checking the base.
- 2026-10-19 23:41 rnd-kb: rnd-kb: Car_Reset_LastFittedUpgrade 0x143517d80 is also the per-INSTANCE upgrade reset on the career side: E8 callers instantiate @0x14352d071 (Career_Instantiate_UpgradeTrees), reset-all @0x14352e223 (Career_ResetAll_Upgrades), supplier refit @0x143515290. It branches on the department byte at site_Career_UpgradeInitFakeDeptBranch 0x143517e0c (cmp byte [rcx+0x72],4 ; jne): dept 4 gets one disabled sentinel state entry. Consider a rename once the cxx string attribution is re-checked. Separately the rnd-tyres work found Car_Read_VehicleAttribute 0x14351df60 called as the R&D development-cost wrapper (research twin 0x14351dde0, both around cost 0x143525b60); its current label may be wrong. Details: rnd_ui.md#instances, #modifiers.
- 2026-10-20 14:51 ce-rnd: 2026-10-20 ce-rnd: Car_Apply_UpgradeToVehicle 0x1435173e0 reads as the U-level R&D DEVELOPMENT start (10 args: Upgrades, owner, instance, start day, end day, option, dual, roll, rng, store; pushes InDevelopment, end==start pushes PendingFitting), called by the component develop request 0x141242530 at 0x141242abb; its 4-arg proto is stale. I added sig career.apply_upgrade_to_vehicle (kept your name/level); addons/career-editor/native/src/rnd.cpp calls it through its own 10-arg type. Car_Read_VehicleAttribute 0x14351df60 is the development cost wrapper (Upgrades, owner, instance, 0) whose result the dev start stores at instance+0xa4. See addons/career-editor/protocol/rnd.md#engine-paths-for-the-kb-rows.
- 2026-10-22 14:16 vmknolle: Vehicle physics data (vmknolle): handling = teams/<team>/handling/<team>.vtf (VTF resource in the vehicle package ERPs), EGO binary XML: b'\x01BXML', element = u32 size, u16 attr count, name\0, key\0value\0 pairs, children, closed by u16 4 + u32 5|6. Tyre compounds = tyrecompounds/*.vcmpd (VCMPD resource, plain XML). 83 files across f1_2024/2025(+story,+world)/2026, F2 2024-26 and safety car. 2025 teams differ in only 34 of 852 values: aero (BodyDragCoeffMin/Max, lift multipliers, wing CoL/CoD), Chassis Mass 798-813, COMZ, F1Plank OffsetZ, gear ratios, R&D upgrade steps; engine, ERS, tyres and brakes identical. 2026 vs 2025: Mass 771 vs 801, MaxDeployPower 350 kW vs 126.3 kW, DeployLimitMGUK 13 MJ vs 4 MJ, MGU-H harvest 0, lower drag. Code: Car_Load_Vtf, Car_Build_VtfComponents, Car_Read_<Section> (reads each value by name with Game_Read_VtfFloat and stores it XOR-obfuscated; also runs for Upgrade blocks), Car_Write_<Section> (serialises the section back to XML).
- 2026-10-22 14:35 vmknolle: Fuel modes (vmknolle): FuelECU in every VTF has FuelBurnScale/FuelPowerScale/FuelRevsScale/TemperatureScale for Lean, Standard, Rich, Max. F1 2024/2025 cars: Lean burn 0.7 power 0.9, Standard/Rich/Max all 1.0 (Rich and Max behave exactly like Standard); radUpgradeFuelConsumption multiplies burn by 0.85. F1 2026 and F2 cars: all four modes 1.0 (fuel mode has no effect). HUD names lean/standard/rich/max in 0x141788bc0; MFD settings list (fuel_mix, brake_bias, differential, ers) 0x141534c90; fuel_mix_editable UI node in the MFD builder 0x1414e2d00. VTF binary XML round-trips byte-identically (74/74 files) with a decode/encode pair: close record u16 4 + u32 5 per element, file trailer 2x (u16 4 + u32 6).
- 2026-10-22 14:47 vmknolle: Physics runtime map (vmknolle): the VTF format matches swofty_formats::bxml (fragment 1 TSMD stats must match when rebuilding, see addons/rnd-tyres-thermal/tools/build_vtf.py). Physics car components at car+0x530 Engine, +0x538 GearBox, +0x540 FuelTank, +0x548 FuelECU, +0x550 BrakeECU, +0x558 Chassis, +0x560 Steering, +0x568 CoreAero, +0x570 F1Aero, +0x578 Differential, +0x580 SuspensionSystem, +0x588 DebugSystem, +0x590 F1Tuning, +0x598 F1Assists, +0x5a0 Turbo, +0x5a8 EnergyRecoverySystem, +0x5b0 ConvexHull, +0x5b8 WheelProxies, +0x5c0 F1Plank, +0x5c8 F1WingDamage, +0x5d0 F1SuspensionDamage, +0x5d8 F1TyreDamage, +0x5e0 F1BodyDamage (cached by name in Car_Build_VtfComponents). Wheels: SuspensionSystem+0x70 RL, +0x78 RR, +0x80 FL, +0x88 FR (this is the 'wheel container' of #live); per wheel +0x3b0 slip angle, +0x3b4 slip ratio, +0x3c0 lateral force, +0x3c4 longitudinal force (Car_Vte_WriteTelemetryRow CSV column order; L1, needs a trace). Component data classes have 4-slot vtables {dtor, ?, Car_Read_<X> (by-name reader, also for Upgrade blocks), Car_Write_<X> (XML serialiser)}. VTESystem writes telemetry/timing/lap_analysis/final_classification/tyre_strategies/overtakes CSV and report.txt to <save path>\vteoutput\<date>.
- 2026-10-23 23:37 derived-keys: Car_Load_SetupPreset 0x140c04fd0 and Car_Load_SetupPresets 0x149a21730 (L1, derived-keys): car setup presets are found by Gdb_Find_RowByLabel(car_setup.presets, vehicle_package_setups, <package folder>) and read as <row>.<ai|player>.<variant>.<slider>, 420 keys per row. A vehicle package copy without its own preset row drives grounding AI setups. Write-up and the other label-keyed lookups: gamedb.md#side-data.
- 2026-10-22 15:11 vmknolle: Tyre force model (vmknolle): Car_Tyre_MagicFormula 0x1446cf080 is a Pacejka Magic Formula over the VCMPD TyreModel (ModelType MNC) ParametricData rows: per LateralLoad/LongitudinalLoad row {Load, Peak, PeakMin, PeakMax, DownSlope, UpSlope, Shape} lerped by wheel load -> D = Peak*(1+LateralCamber.GripGain), C = DownSlope, B = UpSlope (per degree of slip), E = Shape; F = D*sin(C*atan(B*a - E*(B*a - atan(B*a)))). Peak falls with load (C5 front 2.372 at 0 N, 1.199 at 15000 N). Called 4x per frame from Car_Suspension_SolveWheels 0x1446dee60; results copied to the wheels by Car_Suspension_PublishWheelState 0x1446e4330.
- 2026-10-22 15:19 vmknolle: Obfuscated physics numbers (vmknolle): at runtime VTF spline samples and many handling coefficients are stored XOR-obfuscated as two u32s 8 bytes apart (value = a ^ b), e.g. Car_Spline_EvalObfuscated 0x142e88290 and the aero coefficients at F1Aero +0x1718/+0x1720 in Car_Physics_StepVehicle 0x1446dceb0. Addons that read or patch handling values in memory must decode/encode the pair; editing the VTF data before load avoids this. Aero outputs: front downforce F1Aero +0x19ec, rear +0x19f0, drag force magnitude +0x19d8; wing damage factors +0x1998 (rear) / +0x199c (front).
- 2026-10-25 02:08 vmknolle: vmknolle 2026-10-25: Car_FrontWing_Init (0x14465f620, FrontWing vtable slot at 0x145bf90d0) and Car_Register_DetachablePart_Thunk (0x144660400, 11 call sites 0x14465f898..0x14465fd58 in FrontWing init) are used by the addon Parts Detach (WRAP on FrontWing init, then more Register_DetachablePart calls on the same front wing with own predicates). Evidence for L2: the stock predicate 0x144660380 is threshold*fw+0x38 <= fw+0x48+fw+0x4c (max, left, right damage); 0x144660400 -> 0x1446612a0 -> 0x144661440 creates the part (0x200 bytes, vtable 0x145bf94d0) in car+0x2c8 and appends it to fw+0xb0..+0xb8, nothing when the model has no such node; part update 0x1446686d0 calls fn(ctx, part+0x30) every frame. Runtime 2026-10-25 (Melbourne, 22 cars): 5-9 extra parts registered per car, all came off on demand. Please raise both to L2 and verify (needed for the binding export).
- 2026-10-28 car-lead: merged the four lanes of the car-struct campaign into this map. Corrections to the lines above: front body downforce is F1Aero +0x19f0 and rear +0x19ec (the 2026-10-22 note has them swapped, see #f1aero); `CarWheelState` (CarPhysicsRecord+0x58) is CarLive, so its +0xe8 is vehicle_id (see #live); the R&D "attribute array" at +0xd10 is the career Upgrades object's tree list, not a physics array (see #perf).
- 2026-10-28 13:24 car-re: 2026-10-28 car-lead: live curve edits must write a spline's baked sample table (+0x98, value ^ +0xa0, count-1 at +0x90), not its point list (+0x8..+0x10): Car_Spline_EvalObfuscated reads the point list only when +0x90 < 0, and the loader 0x14466d250 bakes the points at load. cars::live engine and aero curve writes go to the baked table. Front wing piece detach: Car_FrontWing_IsLeftPieceOff / IsRightPieceOff / IsCentrePieceOff (0x1446603a0 / 0x1446603d0 / 0x144660380) read SideDetachScale and FullDetachScale per piece. Both cross-checked with Will's (william09leo) private findings.
- 2026-10-28 14:54 re-infra: contributor vmknolle (PR #453) proposes L2 for Car_FrontWing_Init 0x14465f620 and Car_Register_DetachablePart_Thunk 0x144660400. They landed at L1 in this lane with the contributor's evidence and notes kept; evidence is in the rows' evidence/notes columns and verify.tsv V1190 (agree), V1191 (agree). Raising them is the car-re owner's call.
- 2026-10-28 15:50 car-re: 2026-10-28 car-re: verify.tsv V ids repeat. The 82 contributor verify rows that landed with PRs #446 to #453 (as #464; verifiers verify-a to verify-f, dated 2026-10-28) are labelled V1015 to V1191, and those numbers were already in use: 90 V numbers in that range now label two rows each (the other row is from ce-verify-*, laneS-verify, fuel-verify, car-v2 and others; V1153 to V1160 were doubled before). The V number is a label in the notes column, not a key, and existing rows are not renumbered. For a row in that range, cite it by date, verifier and item (for example '2026-10-28 verify-f 0x144660400') instead of the bare V number. Existing 'per V####' cites in that range may mean either row.
- 2026-10-28 16:25 std16-lead: 2026-10-28 car-v3 verify (V1450-V1467, 17 agree, 1 partial) corrections for car-re: (1) Car_DetachablePart_Update 0x1446686d0: the 1.5 s timer on a detached part is conditional. part+0x40 null adds dt to +0x1f0; otherwise b=[part+0x40]+0x18 adds dt only when (+0x1f4 != 0 and b+0x444 == -1) or when the root of b's +0x30 chain has +0x40 == 1 and +0x44 == 0, else +0x1f0 = 0 (0x1446687f5) before the predicate. (2) Car_Create_DetachablePart node-miss fallback 0x1446617c3 is identity rotation with translation (0,-1000,0) from 0x145bf6830, unreachable behind the early returns. (3) Car_FrontWing_Init writes params +0x0..+0x5f only; the callee copies 0x70 bytes, so +0x8..+0xf and +0x60..+0x6f are uninitialised. (4) Car_Apply_TeleportRequest also clears decoded bit 0x20 of +0x9ea8 when nothing is pending. (5) CarGearBox gear_index writes at 0x142f4a883, 0x142f4aaf9, 0x142f4ab1f, 0x142f4aa1e/0x142f4aa24 are all gated by [entry+0x58]+4 == 11; add the gate to CarGearBox.h.
- 2026-10-28 22:56 re-import: Car_Read_Differential_Thunk 0x14467ddf0: PR #477 named this Car_Read_Differential: Body 0x1553d2720: DriveType strings 'FWD' 1 / '4WD' 2 / 'RWD' 3 (0x145bfa111/115/119, else 0); runtime diff car+0x578: +0x30 RearPowerProportion, +0x40 DiffRearNoEffect, +0x50 FullEffect, +0x70 MaxLockingTorque, +0x80 MidCornerGForce, +0x90 GForceBlendSlope, +0xa0 RampDifferential, +0xa8 driven count, +0xb0 driven ptrs, +0xd0 shares, +0xec off / +0xf0 on / +0xf4 mid multiples, +0xf8 high-speed kph, +0xfc preload (agent power, ap-verify claim 6) | findings from PR #477 by vogtmarcel07-cloud; independent check 2026-10-28 by ap-verify: agree
- 2026-10-28 22:56 re-import: Car_Engine_ComputeDerived_Thunk 0x144689a70: PR #477 named this Car_Engine_ApplyFuelMode: Body 0x1553f9a30: engine+0x2528 = ([+0x8b0]^[+0x8b8] LimiterRotationRate) * ([[car+0x548]+0x150]^ RevsScale) plain float (effective rev limit, 0x1553f9a6a); +0x2508 = base power +0x58 * PowerScale (XOR); +0x2518 peak torque. Followed by shift points 0x1446a5130 (= Car_Gearbox_ComputeShiftPoints) (agent power, ap-verify claim 10) | findings from PR #477 by vogtmarcel07-cloud; independent check 2026-10-28 by ap-verify: agree
- 2026-10-28 22:57 re-import: Car_FuelEcu_Reset_Thunk 0x14468e550: PR #474 named this Car_FuelEcu_Reset: FuelECU vtable 0x145bfa870 slot 3 (body 0x1553fdf60); flag 0x2000 forces Standard (mode 1). Practice programmes (PracticeFuelManagement 0x142971900 bit 0x01, PracticeERSManagement 0x142947400 bit 0x02, RacePace 0x14298d9d0 bit 0x10, TrackAcclimatisation 0x1429a06b0 bit 0x20) lock Lean/Rich/Max in EngineModeAvail and force Standard (fuel-mix, static) | findings from PR #474 by vogtmarcel07-cloud
- 2026-10-28 22:57 re-import: Car_FuelEcu_SetMode_Thunk 0x14468e6a0: PR #474 named this Car_FuelEcu_SetMix: Thunk -> body 0x1553ff3d0: XOR-decodes Burn/Power/Revs/Temp per mode from ecu+0x28.. (+0x10 per mode, +0x40 per kind) into active ecu+0x130 burn / +0x140 power / +0x150 revs / +0x160 temp (keys +0x138/148/158/168), calls Engine_ApplyFuelMode 0x144689a70 and Gearbox_ApplyFuelMode 0x1446a5130, stores mode ecu+0x128; early return if unchanged, returns 1 (fuel-mix, fuel-verify claim 5) | findings from PR #474 by vogtmarcel07-cloud; independent check 2026-10-28 by fuel-verify: agree
- 2026-10-28 22:57 re-import: Car_Read_EnergyRecovery 0x1446cfcd0: VTF EnergyRecoverySystem reader, PLAIN floats (data+0x18): +0x28 MaxDeployPower, +0x50 HarvestLimitMGUK, +0x5c DeployLimitMGUK, +0x60 EnergyStoreCapacity, +0x81 OverrideModeAvailable; modes +0x1880+m*0x40 (gear modifiers +0x10, DerateStartSpeed +0x3c); TrackSettings +0x1980; runtime ERS = car+0x5a8, mode +0x1a68, restore +0x1a6c, lock +0x1a70, 2026 override flags +0x1a60..63. Full map work/fuel/findings_ers.md (fuel-mix, static) | findings from PR #474 by vogtmarcel07-cloud
- 2026-10-28 22:57 re-import: Car_Tyre_GripScale 0x1446b7240: = wet/speed DOWNSLOPE scale: f1 = 1 - WetnessDownslopeFactor(+0x638 player/+0x63c AI)*clamp(tyre water wheel+0x8c/0.2); f2 = 1 - SpeedDownslopeFactor(+0x640/+0x644)*clamp((RotRate+0x648-rot)/RotRate); return max(DownslopeScaleMinimum +0x64c, f1*f2). Used by Car_Suspension_SolveWheels 0x1446dee60 only if HandlingVersion car+0x9f40>=7, multiplied into MF column +0xc0 (inferred DownSlope) (wx 2026-10-28) | findings from PR #471 by vogtmarcel07-cloud
- 2026-10-28 22:57 re-import: Car_Tyre_UpdateWear 0x1446b79f0: wheel+0x88 wear += WetnessWear spline (+0xc18)(tyre water wheel+0x8c) * temperature wear (TyreType +0x654/+0x658) * WearRate lerp (compound +0x120/+0x124) * UpgradeWearFactor +0x630; clamp 0..1; gate car+0x132 bit7 and car+0x128!=1 (wx-drive, static) | findings from PR #471 by vogtmarcel07-cloud
- 2026-10-28 22:57 re-import: Car_Dynamics_Update 0x1446dc480: PR #471 named this Car_Update_WheelSurfaceAndEcuGrip: In vtable 0x145bff4d8. Per wheel [[car+0x580]+0x70/78/80/88]: sample system vt slot 5 (+0x28) with r8=wheel+0x2f0 contact, r9=wheel+0x200 surface MECHANICS record (fourCC +0x50, 'N0NE'), copies 0x30 bytes to wheel+0x280..+0x2af (1446dc79b-7ad, x4): +0x280 wetness, +0x284 puddle, +0x288 off-track, +0x28c v, +0x290 distance, +0x294/+0x298 surface code/value, +0x2a0/+0x2a4 grip, +0x2a8 f, +0x2ac w. Player only (car+0x124==0): wet ECU set chosen when FL compound is wet/inter ([[wheel+0xc0]+0xf8]); ATC/ABS grip factor by TC level ECU+0x21e0 / ABS +0x21e4 -> wheel+0x1f0 (wx-verify claim 7) | findings from PR #471 by vogtmarcel07-cloud; independent check 2026-10-28 by wx-verify: agree
- 2026-10-28 22:57 re-import: Car_Load_Vtf 0x14475a250: HandlingVersion (2026-10-28 wx-verify claim 11): key 'HandlingVersion' (145c04a50) at 14475a4e3: strstr 'pro' -> 100; else strstr 'handling20' -> atoi, clamp 2018..4048, -2017 (14475a54a-565); else 1; stored sysobj+0x18 (14475a577), copied to car+0x9f40 at 1446296c8/cc. Gates: wet downslope >=7 (2024), blister >=13 | findings from PR #471 by vogtmarcel07-cloud; independent check 2026-10-28 by wx-verify: partial
- 2026-10-28 22:57 re-import: (wx round 1-2, static, wx-verify 11/15 confirmed): WEATHER -> DRIVING. Physics wetness is NOT the render wetness map: Game_TrackInfo_SampleWheelSurface 0x144792f50 samples track-space RGBA map [ws+0x1018] (G wetness, R dry line, B puddle depth, A line state) x global wetness ws+0x3f0a38 (set per frame in 0x144795cf0) -> Car_Update_WheelSurfaceAndEcuGrip 0x1446dc480 copies per wheel to wheel+0x280 wet / +0x284 puddle / +0x2a0 grip (material MECHANICS ptr wheel+0x200, m_waterDepth +8, ffb_surface +0x78). Tyre water wheel+0x8c follows wheel+0x280 (+4.0/s, -0.5/s, Car_Tyre_Update 0x1446b5720) and drives WetnessGrip (x wheel+0x90, default 1.0, event modifier wetness_grip_scale), WetnessWear, WetnessTarget (temperature) and the wet DOWNSLOPE (Car_Tyre_GripScale, HandlingVersion >= 7). Wet ABS/TC/ASC sets are chosen by the fitted compound (FL [[wheel+0xc0]+0xf8]), not by track wetness. No aquaplaning code. Hooks: write wheel+0x280/+0x284 after 0x1446dc480, or wheel+0x90, or ws+0x3f0a38 after 0x144795cf0. | findings from PR #471 by vogtmarcel07-cloud
- 2026-10-28 22:57 re-import: Fuel-mix lock (findings from PR #474 by vogtmarcel07-cloud; checks fuel-verify 2026-10-28): the FuelAuto lock is ctrlSys record byte +0xd7 (record = g 0x146096578 +0xc0 + veh*0x14), written by Game_Player_ApplyAssistsToVehicle 0x142424060 as (assist int[0x21] auto_fuel_mode, profile+0x294, != 0) at 0x142424606 (cmp) / 0x142424611 (setne). While it is set the player's mix is ignored (Game_Vehicle_ApplyControls 0x14389a2b0 takes the AI/assist controls instead), Game_Mfd_FuelMixUp/Down (0x141533370/0x141533570) return 0 (cmp at 0x141533398) and the fuel row in Game_Mfd_FuelMix_Refresh is not editable. A fuel-mix feature (session.fuel.load hook, race-fuel addon, fuel mix in careers) unlocks the mix either by clearing the byte after Game_Player_ApplyAssistsToVehicle returns (call site 0x142422c89 in Game_AssistManager_BuildAssistSettings) or by patching 0x142424606 (80 be 84 00 00 00 00 -> 39 c0 90 90 90 90 90); a [1,1] SettingsRestrictionComponent range for assist 0x21 is what forces automatic fuel (H). The mix itself is applied through Car_FuelEcu_SetMode_Thunk 0x14468e6a0 (call 0x14389bbf3, the T123 trace site); EngineModeAvail [0x14606ec28]+0x28+veh*0x20+mix*4 == 0 gates each mode, and the Max row is hidden when (sessionType-15) < 3 (inferred, partial check). The byte is level L1 here; a behaviour hook needs an L2 row and an independent agree first.
- 2026-10-28 22:57 re-import: F1Aero+0x19ac (CarF1Aero.h rear_wing_fixed): PR #477 by vogtmarcel07-cloud reads the same byte as the 2026 PARTIAL mode flag (front wing only, rear closed, ERS deploy cap 250 kW, race Hotlap lock), written by Game_DRS_UpdateCars 0x142be98d0 at 0x142bea69d (independent check ap-verify 2026-10-28: agree). Both meanings agree; the car-lane owner can add the partial-mode wording to the field note.
- 2026-10-28 09:45 engine-re: 2026-10-28 engine-re (wx round 1-2, static, wx-verify 11/15 confirmed): WEATHER -> DRIVING. Physics wetness is NOT the render wetness map: Game_TrackInfo_SampleWheelSurface 0x144792f50 samples track-space RGBA map [ws+0x1018] (G wetness, R dry line, B puddle depth, A line state) x global wetness ws+0x3f0a38 (set per frame in 0x144795cf0) -> Car_Update_WheelSurfaceAndEcuGrip 0x1446dc480 copies per wheel to wheel+0x280 wet / +0x284 puddle / +0x2a0 grip (material MECHANICS ptr wheel+0x200, m_waterDepth +8, ffb_surface +0x78). Tyre water wheel+0x8c follows wheel+0x280 (+4.0/s, -0.5/s, Car_Tyre_Update 0x1446b5720) and drives WetnessGrip (x wheel+0x90, default 1.0, event modifier wetness_grip_scale), WetnessWear, WetnessTarget (temperature) and the wet DOWNSLOPE (Car_Tyre_GripScale, HandlingVersion >= 7). Wet ABS/TC/ASC sets are chosen by the fitted compound (FL [[wheel+0xc0]+0xf8]), not by track wetness. No aquaplaning code. Hooks: write wheel+0x280/+0x284 after 0x1446dc480, or wheel+0x90, or ws+0x3f0a38 after 0x144795cf0.
- 2026-10-28 12:17 engine-re: 2026-10-28 engine-re (agents aero + power, static, ap-verify 9/10 agree, 1 partial): F1Aero = car+0x570 (field = data off + 0x18, XOR); +0x19ec = REAR body CL, +0x19f0 = FRONT body CL (KB had them swapped/forces); F1Aero+0x19ac = 2026 Partial mode (not DRS open; it zeroes only the rear-wing open term +0x19c4/+0x19c8 and the +0x19b0 target). HandlingVersion = clamp(year,2018,4048)-2017 (2025 = 8, 2026 = 9, pro 100). 0x1446a5130 is Car_Gearbox_ComputeShiftPoints (not Gearbox_ApplyFuelMode). Engine 2025 550 kW ICE, 2026 415 kW + 350 kW ERS; 2026 PU scale per maker in FuelPowerScaleStandard. Drivetrain: DriveType FWD/4WD/RWD data-driven, RearPowerProportion is a fraction; rear-only clutch coupling/LSD/launch. Notes work/aero/findings_aero.md, work/power/findings_power.md.
- 2026-10-28 16:35 engine-re: 2026-10-28 driving round (dcso-verify): slipstream wake (Car_Physics_PreAdvance: 2 s wake length, cone 8/W, quadratic decay, F1Aero scales), contact/barriers (Ne_PhysMaterial_CombinePair sqrt mix, Car_Contact_ModifyVehicle mass ratio up to 50x, DRUB tecpro, breakables 200 for tecpro/physical_tyres), surfaces (Car_Surface_ApplySlowdown, Car_Surface_SampleAt grip vs proGrip by car+0x134 b4, unknown code = grip 1.0), sim option bits (Game_SimFlags_RecordSetBit table), input curves + override slots + dormant wheel lead filter. MATERIALS/MECHANICS values live in game.dat - runtime dump needed.
- engine-re finding for Car_Parse_SurfaceMaterial 0x14463d0d0: MECHANICS surface record: +0 grip, +4 proGrip (player), +8 m_waterDepth (14463d4ea/516), +0xc..+0x24 bumps, +0x28 depth, +0x2c minimumSpeedForSlowdown, +0x30 speedForMaximumSlowdown, +0x34 slowdown, +0x38 lss_speed, +0x40 lss_coefficent, +0x44 camera_factor, +0x48 roughness, +0x4c djb2 name, +0x50 fourCC, +0x54 hardness, +0x58 heat, +0x5c damage, +0x60 wear, +0x64..+0x70 slip angle/ratio f/r, +0x74 dirt_build_up, +0x78 ffb_surface enum (14463d94e, <=12): TARMAC 0 RUMBLESTRIP 1 CONCRETE 2 ROCK 3 GRAVEL 4 MUD 5 SAND 6 GRASS 7 WATER 8 COBBLESTONE 9 METAL 10 RIDGED 11 DEFAULT 12. Wheel points to it at wheel+0x200 (wx-verify claim 15) | dcso-verify claim 8: defaults grip 1.0, proGrip = grip, minimumSpeedForSlowdown 20, speedForMaximumSlowdown 40, slowdown 0, lss_speed 10 (+0x3c 0.1 = 1/lss_speed). MECHANICS table lives in the engine physics material library (not in loose ERPs; game.dat) - stock values need a runtime dump | gb-verify claim 2: keys depth (+0x28), bumpstimebased (+0xc), bumpsmagnitude (+0x14), bumpswavelength (+0x10), bumpsmagnitudeforai (+0x18), bumpsmagnitudecasual (+0x1c), bumpsrealproportion (+0x20), bumpsproportionCarDependent, slowdown, camera_factor, roughness, slip_angle/ratio_f/r; ffb_surface enum (0x14463d8f4): TARMAC 0, RUMBLESTRIP 1, CONCRETE 2, ROCK 3, GRAVEL 4, MUD 5, SAND 6, GRASS 7 .. DEFAULT 0xc, unknown 0xd
- engine-re finding for Car_Wheel_Construct 0x1446af9c0: Writes qword 0x3f80000000000000 at wheel+0x8c (1446af9ff/a09): tyre water 0.0, wetness-grip scale wheel+0x90 = 1.0; +0x94 blister 0; wheel+0 = wheel+0x10 = car (wx-verify claim 10)
- engine-re finding for Car_Tyre_Step 0x1446b5720: Tyre water level wheel+0x8c moves toward surface wetness wheel+0x280: rise 4.0*dt (0x1453814ac), fall -0.5*dt (0x14547dab4, sign built in) (1446b59bf-a15), only if car+0x133 bit0; calls temperature update 0x1446b7b70 (wx-verify claim 8)
- engine-re finding for Car_Tyre_GripScale 0x1446b7240: Per-wheel grip multiplier: max(floor +0x64c, f1*f2); f1 = 1 - k(+0x638/+0x63c by axle) * clamp(x/0.2) with x = this+0x84; f2 = 1 - k(+0x640/+0x644) * clamp((v0 +0x648 - wheel slide speed)/v0) using the wheel's +0x3a8/+0x3ac velocity and +0x188 rotation rate. Wheel from car list 0x146ce86e0 -> +0x580 SuspensionSystem -> +0x70+i*8. Meaning of x needs a trace. | = wet/speed DOWNSLOPE scale: f1 = 1 - WetnessDownslopeFactor(+0x638 player/+0x63c AI)*clamp(tyre water wheel+0x8c/0.2); f2 = 1 - SpeedDownslopeFactor(+0x640/+0x644)*clamp((RotRate+0x648-rot)/RotRate); return max(DownslopeScaleMinimum +0x64c, f1*f2). Used by Car_Suspension_SolveWheels 0x1446dee60 only if HandlingVersion car+0x9f40>=7, multiplied into MF column +0xc0 (inferred DownSlope) (wx 2026-10-28)
- engine-re finding for Car_Tyre_ComputeGripMultiplier 0x1446b7390: Product by mask: 0x1 wheel+0x2a0 surface grip, 0x2 WearGrip(+0x88), 0x4 TemperatureGripCarcas(+0x70)*TemperatureGrip(+0x6c), 0x8 residue, 0x10 BlisterGrip(+0x94), 0x20 WetnessGrip spline [wheel+0xc0]+0xec0 at wheel+0x8c*wheel+0x90 (1446b75a1-5bd), 0x40/0x80 puncture/burst, 0x100 damage, 0x200 wheel+0x1f0*TyreType+0x2c; clamp Min/MaxGripMultiplier (+0x24/+0x28) (wx-verify claim 9)
- engine-re finding for Car_Tyre_UpdateWear 0x1446b79f0: wheel+0x88 wear += WetnessWear spline (+0xc18)(tyre water wheel+0x8c) * temperature wear (TyreType +0x654/+0x658) * WearRate lerp (compound +0x120/+0x124) * UpgradeWearFactor +0x630; clamp 0..1; gate car+0x132 bit7 and car+0x128!=1 (wx-drive, static) | 2026-10-28 engine-re (agent car-tyre): dWear = (AI ? wheel+0x84 : 1)*wheel+0x80*TT+0x630*min(TT+0x658, r^2)*WetnessWear(water)*lerp(WearRateMin, WearRateMax, pressure)*slipWork/1e6, r = (TT+0x654 + T_patch)/(TT+0x654 + T_opt); wear per MJ of sliding work; no cliff in stock data (WearGrip ~0.80-0.88 at wear 1)
- engine-re finding for Car_Tyre_UpdateTemperature 0x1446b7b70: If car+0x133 bit2 clear (simplified): wheel+0x6c/+0x70 = WetnessTarget spline (+0x1168)(wheel+0x8c) = target tyre temperature vs water (medium); water cooling clamp((wet-0.15)*4)*0.002; blister wheel+0x94 += dt*BlisterRate(+0x128)*clamp((T-MinBlist+0x12c)/(MaxBlist+0x130-MinBlist)) if HandlingVersion>=13 (wx-drive, static) | 2026-10-28 engine-re (agent car, cg-verify claim 3): 2-node model (tread +0x6c, carcass +0x70, no inner); full model only with car+0x133 bit2 (rolling heat, slip work 0x1446b8210, convection, Penman evaporation, carcass conduction, brake radiative; clamps 393.15/423.15 K); blister block gated cmp [car+0x9f40],0xd / jl at 0x1446b8130 (HandlingVersion >= 13 -> OFF for 2025 = 8 and 2026 = 9)
- engine-re finding for Car_Load_Vtf 0x14475a250: Loads a VTF handling file (binary XML 'BXML', teams/<team>/handling/<team>.vtf in the vehicle package ERPs); reads HandlingVersion ('pro', 'handling20') and dispatches the sections Chassis, Engine, GearBox, FuelTank, FuelECU, BrakeECU, Steering, CoreAero, Differential, SuspensionSystem, Turbo, WheelProxies, ConvexHull, F1Aero, F1Assists, F1Tuning, F1Plank, ... | HandlingVersion (2026-10-28 wx-verify claim 11): key 'HandlingVersion' (145c04a50) at 14475a4e3: strstr 'pro' -> 100; else strstr 'handling20' -> atoi, clamp 2018..4048, -2017 (14475a54a-565); else 1; stored sysobj+0x18 (14475a577), copied to car+0x9f40 at 1446296c8/cc. Gates: wet downslope >=7 (2024), blister >=13
- engine-re finding for Car_FuelEcu_Reset_Thunk 0x14468e550: FuelECU vtable 0x145bfa870 slot 3 (body 0x1553fdf60); flag 0x2000 forces Standard (mode 1). Practice programmes (PracticeFuelManagement 0x142971900 bit 0x01, PracticeERSManagement 0x142947400 bit 0x02, RacePace 0x14298d9d0 bit 0x10, TrackAcclimatisation 0x1429a06b0 bit 0x20) lock Lean/Rich/Max in EngineModeAvail and force Standard (fuel-mix, static)
- engine-re finding for Car_FuelEcu_SetMode_Thunk 0x14468e6a0: Thunk -> body 0x1553ff3d0: XOR-decodes Burn/Power/Revs/Temp per mode from ecu+0x28.. (+0x10 per mode, +0x40 per kind) into active ecu+0x130 burn / +0x140 power / +0x150 revs / +0x160 temp (keys +0x138/148/158/168), calls Engine_ApplyFuelMode 0x144689a70 and Gearbox_ApplyFuelMode 0x1446a5130, stores mode ecu+0x128; early return if unchanged, returns 1 (fuel-mix, fuel-verify claim 5)
- engine-re finding for Car_Read_EnergyRecovery 0x1446cfcd0: VTF EnergyRecoverySystem Common + modes + per-track TrackSettings: MaxDeployPower (2025 126.3 kW, 2026 350 kW), MaxDeratePower, DeployLimitMGUK (4 MJ / 13 MJ), EnergyStoreCapacity 4 MJ, harvest efficiencies, MaxBrakeTorqueMGUK, HarvestLimitMGUK/Qualifying, HotLap/MediumDeployFactor. Correction: this is the reader (component data vtable slot 2): it fetches each value by name with Game_Read_VtfFloat and stores it XOR-obfuscated (value ^ key 8 bytes later, e.g. Engine Mass -> +0x2f8 ^ +0x300); it runs for the base section and for every Upgrade block. | VTF EnergyRecoverySystem reader, PLAIN floats (data+0x18): +0x28 MaxDeployPower, +0x50 HarvestLimitMGUK, +0x5c DeployLimitMGUK, +0x60 EnergyStoreCapacity, +0x81 OverrideModeAvailable; modes +0x1880+m*0x40 (gear modifiers +0x10, DerateStartSpeed +0x3c); TrackSettings +0x1980; runtime ERS = car+0x5a8, mode +0x1a68, restore +0x1a6c, lock +0x1a70, 2026 override flags +0x1a60..63. Full map work/fuel/findings_ers.md (fuel-mix, static)
- engine-re finding for Car_Read_Differential_Thunk 0x14467ddf0: Body 0x1553d2720: DriveType strings 'FWD' 1 / '4WD' 2 / 'RWD' 3 (0x145bfa111/115/119, else 0); runtime diff car+0x578: +0x30 RearPowerProportion, +0x40 DiffRearNoEffect, +0x50 FullEffect, +0x70 MaxLockingTorque, +0x80 MidCornerGForce, +0x90 GForceBlendSlope, +0xa0 RampDifferential, +0xa8 driven count, +0xb0 driven ptrs, +0xd0 shares, +0xec off / +0xf0 on / +0xf4 mid multiples, +0xf8 high-speed kph, +0xfc preload (agent power, ap-verify claim 6)
- engine-re finding for Car_Diff_SetupDrivenWheels 0x14467e7c0: Thunk -> body 0x1553e9240 (vtable 0x145bfa120 slot 6 ptr 0x145bfa150): driven list RL/RR ([car+0x580]+0x70/+0x78) + FL/FR (+0x80/+0x88) when DriveType [diff+0x28] is FWD 1 or 4WD 2; shares diff+0xd0..+0xdc (RL RR FL FR): 4WD 0.5*RPP / 0.5*(1-RPP) (RearPowerProportion used as FRACTION), FWD 0/0.5, RWD 0.5/0; car+0x225 bit0 = isRWD. FWD still has rear-only clutch coupling (0x1446e368f), LSD and clutch controller 0x144741700 (agent power, ap-verify claim 5)
- engine-re finding for Car_Engine_ComputeDerived_Thunk 0x144689a70: Body 0x1553f9a30: engine+0x2528 = ([+0x8b0]^[+0x8b8] LimiterRotationRate) * ([[car+0x548]+0x150]^ RevsScale) plain float (effective rev limit, 0x1553f9a6a); +0x2508 = base power +0x58 * PowerScale (XOR); +0x2518 peak torque. Followed by shift points 0x1446a5130 (= Car_Gearbox_ComputeShiftPoints) (agent power, ap-verify claim 10)
- engine-re finding for Car_F1Aero_UpdateWingOpen 0x1446cb540: DRS / 2026 straight-mode wing: clears open request F1Aero+0x1988 bit0 when brake [car+0x190] > 0.05 (0x1446cb65f) or throttle [car+0x188] < 0.9 (0x1446cb6d0); open fraction +0x19a8 moves at DRSOpenCloseSpeed ([+0x1868]^[+0x1870]) clamped 0..1; second fraction +0x19b0 targets 0 when +0x19ac (2026 Partial mode); bit1 = failed (skips checks); car = [F1Aero+8]. F1Aero = car+0x570, runtime field = data offset + 0x18, XOR pairs (agent aero, ap-verify claim 1)
- engine-re finding for Car_Solver_PrepareWheel 0x1446dbc50: Copies drive share [[solver+0x18]+0x578]+0xd0[i] -> solver+0x440[i] every step (0x1446dbce3/ced) -> torque split live-editable (agent power, ap-verify claim 7)
- engine-re finding for Car_Setup_ApplyTuning 0x140c11550: Log channel F1_TUNING: 'Setting diff on throttle multiple to %f.' | 'Setting diff off throttle multiple to %f.' | 'Setting front camber to %f.' | 'Setting rear camber to %f.' | 'Setting front toe to %f.' | 'Setting rear toe to %f.' | 'Setting front suspension's anti-roll bar constant to %f.' | 'Setting rear suspension's anti-rol | 2026-10-28 engine-re (agent car, cg-verify claim 4): (obj +0x28 car, u64 *lockMask, record); mask qword [rdx] (0x140c11593); bit 0x1 -> front wing block (F1Aero +0x19b4/+0x19b8/+0x19bc from setup dword[0]), 0x2 -> rear wing (+0x19c0.. from [+4]); further bits 0x4..0x1000 and bt 0x25..0x29 (64-bit); polarity not uniform (bits 0x1/0x2 set = skip). Setup int map in work/car/findings_car.md; parc ferme (H) = lockMask + setup copy select byte +0x158 | sp-verify claim 2 (agent setup): car = ([binding+0x28]+0x590)[1]; item 2 diff on throttle -> [car+0x578]+0xf0 (0x140c11872), item 3 off -> +0xec (0x140c1198b), HS +0xf8, preload +0xfc; item 17 brake pressure (rec+0x44) -> [car+0x550]+0x21dc, item 18 bias (rec+0x48) -> +0x21d8 (0x140c12854/5c, bits 17+18 tested together 0x60000); camber/toe/ARB axles [car+0x580]+0x90/+0x98 +0x1650/+0x1654/+0x1664; wings F1Aero +0x19b4..+0x19c8 (step = wing level, clamp NumWingSettings); engine braking Engine +0x252c; tyre pressure wheel+0x9c via 0x1446d8620. Physics value = VTF F1Tuning <Item>Min + (Max-Min)*step/steps (UI min/max = display only). Applied on slider change / garage exit / net sync / load, physics reads every frame -> live writes work
- engine-re finding for Car_Read_F1TyreDamage 0x144657290: Log channel DEFORMATION: 'Loading node graph for tyre %d from VTF files for package %s.' | 'Tyre damage is %s.' | 2026-10-28 engine-re (agent car): this is the F1TyreDamage VTF data reader (Car_Read_F1TyreDamage, data vtable 0x145bf8d00, writer 0x144657040, component car+0x5d8)
- engine-re finding for Car_Damage_SetComponentDamage 0x144660cd0: VEHICLE_DAMAGE log '%s: damage: %x, was broken, is broken': if the wing was broken and the bits clear (& 0x3ff == 0) -> slot 0x18(0x1000) reset ('reset damage'); else left damage = max * (bits >> 5) / 30, right = max * (bits & 0x1f) / 30, total ratio clamped to [0,1] into [+0x90]+0x199c ('setFrontWingDamage'). | 2026-10-28 engine-re (agent car): this is the FRONT WING damage setter (proposed Car_FrontWing_SetDamage), void(wing, u32 packed): left = (p>>5)*Max/30, right = (p&31)*Max/30, detach above Max/10 (vfunc +0x18, 0x1000) -> F1Aero+0x199c (M)
- engine-re finding for Car_BrakeEcu_ApplyBias 0x144676250: Bias ECU+0x21d8 (0..1) vs 0.5 (0x144676289): b <= 0.5 t = 2b, axle = Base*t + Backwards*(1-t); else t = 2(1-b), Base*t + Forwards*(1-t); * pressure ECU+0x21dc; writes XOR max torque wheel+0xf8 (key +0x100) for 4 wheels unless wheel byte +0x138 (0x14467637f/3ab/3da/405). Inlined copy in Car_Input_UpdateControls 0x14473fd80 (agent car, cg-verify claim 2)
- engine-re finding for Car_FuelTank_StepBurn 0x14468edb0: BSFC = Min + (Max-Min)*sqrt(((T-fric-OptTorque)*TorqueLinear)^2 + ((rpm-OptRevs)*RevsLinear)^2); flow +0x108 = rpm*T*BSFC*FuelBurnScale(ECU+0x130)*0.001; car+0x132 b2: fuel +0x100 -= flow*dt with FLOOR 0.05 kg human / 0.1 kg AI (never empty). Stock BSFC 4.75e-5/4.83e-5, LHV 42500, capacity 110 kg (agent car-pu, static)
- engine-re finding for Car_Read_F1Assists 0x1446cb880: Log channel PAD_ASSISTS: 'Failed to read %s in VTF. Aborting steering parameter setup for this speed.' | 'Failed to load m_steerOutRateVSSpeedSpline spline.' | 'Failed to load m_steerInRateVSSpeedSpline spline.' | 'Failed to load m_lockProportionVSSpeedSpline spline.' | 'Failed to load m_reducedLockGivenBackVSRearSlipAngleSpline | 2026-10-28 engine-re (agent car): this is the F1Assists VTF data reader (Car_Read_F1Assists, data vtable 0x145bfcbe0, writer 0x1446cc750, component car+0x598)
- engine-re finding for Car_PadAssist_DriftAssist 0x1446cd9a0: Log channel PAD_ASSISTS: 'You wish to use the drift assist vs peak slip angle proportion spline but it has not been loaded.' | Steer model (agent options, dcso-verify claim 14): mode car+0x220 0/2 SMS 0x1446ce000, 1 direct wheel, 3 AI; direct-wheel lead filter (F1Assists +0x2601 enable, tau +0x260c, gain +0x2610, yaw +0x2614) and pad throttle help (+0x2600) are DORMANT in stock: only store is ctor 0x1447546c0 at 0x1447552bc (=0); addon can enable them
- engine-re finding for Car_Build_VtfComponents 0x14473c2e0: Creates the vehicle physics components by VTF section name (Engine, GearBox, FuelTank, FuelECU, BrakeECU, Chassis, Steering, CoreAero, F1Aero, Differential, SuspensionSystem, DebugSystem, F1Tuning, F1Assists, Turbo, EnergyRecoverySystem, ConvexHull, WheelProxies, F1Plank, F1WingDamage, F1BodyDamage, F1SuspensionDamage, F1TyreDamage). | 2026-10-28 engine-re (agent car, cg-verify claim 1): stores 23 component pointers car+0x530 Engine .. +0x5e0 F1BodyDamage, 8 B stride: 0x530 Engine, 0x538 GearBox, 0x540 FuelTank, 0x548 FuelECU, 0x550 BrakeECU, 0x558 Chassis, 0x560 Steering, 0x568 CoreAero, 0x570 F1Aero, 0x578 Differential, 0x580 SuspensionSystem, 0x588 DebugSystem, 0x590 F1Tuning, 0x598 F1Assists, 0x5a0 Turbo, 0x5a8 EnergyRecoverySystem, 0x5b0 ConvexHull, 0x5b8 WheelProxies, 0x5c0 F1Plank, 0x5c8 F1WingDamage, 0x5d0 F1SuspensionDamage, 0x5d8 F1TyreDamage, 0x5e0 F1BodyDamage. Full map work/car/findings_car.md
- engine-re finding for Car_Chassis_Build 0x144628190: Chassis runtime (vtable 0x145bf73a0 slot 7): finds wheel nodes, builds the VTF components (Car_Build_VtfComponents) and the suspension rig; shadow LOD flags. | Car init: for phases 0..6 calls comp->vt+0x28(comp, &{fn,ctx}, p) over components car+0x3a0 (count +0x398) (0x14462979a); lists car+0x4f8+8p, counts car+0x4dc+4p; also mov [car+0x5f0], car (0x1446288bf). Phase map: Chassis P0/P1 mass/P4/P5/P6, Suspension P0/P2/P3/P4/P6, WheelProxies P0/P5, ERS P1, F1Aero P1, Engine P2, Steering P2, FuelTank P4, damage components P1/P4/P5 (agent tick, tr-verify claim 4)
- engine-re finding for Car_Physics_PreAdvance 0x144641610: 'prePhysicsAdvance update' job: parallelForEach over vehicles before the physics advance. | Per physics step: serial car proximity pre-pass, then parallelForEach over g_Sess_VehicleTable 0x146ce86e0 running Car_Update (body 0x1447765c0, job name 'prePhysicsAdvance update' ref 0x14464252c); skips car+0x120 == 1, +0x9f10 == 3, !(car+0x132 & 8). Called from the physics stage list W+0xb8 (agent tick, tr-verify claim 2) | Slipstream wake (agent draft, dcso-verify claims 1-2): reset CoreAero(car+0x568)+0x30 = 1.0 (0x144641d8c), +0x34/+0x3c = 0; leader wake length L = max(0.1, 2*|v.fwd|) (x2 at 0x144641dd0 = 2 s of leader speed). Pair loop 0x14464224c..0x1446423ef: s = clamp(8/W, 1, 80) (W follower slipstream car width, XOR car+0x570 +0x128/+0x130; 8.0 at 0x1453ca8f0, 80 at 0x1456150e0); x = |lat|*s/D, f = 0 if x > 1 else min(1, (1-x)*min(1, (1-D/L)^2)); +0x30 *= (1-f) (0x1446423b6), +0x34 time in wake, +0x3c distance, +0x40 leader. Consumers: StepVehicle 0x1446dceb0 force *= s + (1-s)*w per F1Aero scale (2025 FW 0.7 RW 0.85 floor 0.75 drag 0.9; 2026 0.76/0.82/0.91/0.8)
- engine-re finding for Car_Tuning_SetSuspension 0x1446a7be0: Log channel F1_TUNING: 'Setting front camber to %f.' | 'Setting rear camber to %f.' | 'Setting front toe to %f.' | 'Setting front suspension's anti-roll bar constant to %f.' | 'Setting rear toe to %f.' | 'Setting rear suspension's anti-roll bar constant to %f.' | su-verify claim 8: re-applies from F1Tuning(car+0x590)+0x228..+0x254: camber +0x228/+0x22c -> axle+0x1650, toe +0x230/+0x234 -> +0x1654, springs via 0x1446d7580/0x1446d7780 (+0x238/+0x23c), ARB +0x250/+0x254 -> +0x1664, 0x1446d8620(0,1), ride height +0x240/+0x244 (HV < 7) or +0x248/+0x24c (HV >= 7)
- engine-re finding for Car_F1Aero_GetFrontRideHeight 0x1446cb000: su-verify claim 6: 0 if HV < 7; axle averages of (wheel+0x168 + axle+0x15e4 + clamp01(wheel+0x248 - wheel+0x168)) (wheels car+0x580 +0x70..+0x88), ref height F1Aero(car+0x570)+0x78^+0x80, extrapolated to x = +0x88^+0x90 / wheelbase; clamp [+0xb8^+0xc0, +0xa8^+0xb0]; debug name AeroHeight.front
- engine-re finding for Car_F1Aero_GetRearRideHeight 0x1446cb260: su-verify claim 6: rear mirror of 0x1446cb000: x = F1Aero+0x98^+0xa0, clamp [+0xd8^+0xe0, +0xc8^+0xd0]
- engine-re finding for Car_Solver_PrepareDrivetrain 0x1446db280: solver+0x4e4 = WearICEPowerSpline(eng+0x1420) * WearTCPowerSpline(+0x16d8) * TempEfficiencySpline(+0x8d8) * ([eng+0x2598]^[+0x25a0] R&D power ratio), eng = car+0x530 (0x1446db350/374/390); plain float = ICE torque multiplier (addon power hook: *= k after this) (agent power, ap-verify claim 9) | su-verify claim 4: ALSO suspension lane loader, last call of 0x1446dc480 (every frame): per axle (car+0x580 +0x90/+0x98, 2 lanes each; lanes 0 RL 1 RR 2 FL 3 FR) body lanes: rest +0x1648->+0x360, k +0x165c->+0x370, ARB +0x1664->+0x380, heave k +0x1660->+0x400, heave damper +0x1614->+0x410, zone +0x1628->+0x390, fastBump +0x161c->+0x3a0, slowBump +0x1618->+0x3b0, fastReb +0x1620->+0x3c0, slowReb +0x1624->+0x3d0, RideHeightOffset +0x15e4->+0x330, travel min/max +0x15b8/+0x15bc->+0x340/+0x350, bump stop +0x15cc..+0x15e0->+0x2e0..+0x320; knees +0x3e0 = zone*(+0x1618-+0x161c), +0x3f0 = -zone*(+0x1624-+0x1620)
- engine-re finding for Car_Dynamics_Update 0x1446dc480: In vtable 0x145bff4d8. Per wheel [[car+0x580]+0x70/78/80/88]: sample system vt slot 5 (+0x28) with r8=wheel+0x2f0 contact, r9=wheel+0x200 surface MECHANICS record (fourCC +0x50, 'N0NE'), copies 0x30 bytes to wheel+0x280..+0x2af (1446dc79b-7ad, x4): +0x280 wetness, +0x284 puddle, +0x288 off-track, +0x28c v, +0x290 distance, +0x294/+0x298 surface code/value, +0x2a0/+0x2a4 grip, +0x2a8 f, +0x2ac w. Player only (car+0x124==0): wet ECU set chosen when FL compound is wet/inter ([[wheel+0xc0]+0xf8]); ATC/ABS grip factor by TC level ECU+0x21e0 / ABS +0x21e4 -> wheel+0x1f0 (wx-verify claim 7) | 2026-10-28 engine-re (agent car-chassis): this is the body vtable 0x145bff4d0 slot +8 = per-frame vehicle update (proposed Car_Physics_Update(body, frameDt)): 360 Hz substeps n = max(1, int(frameDt*360)) -> body+0xa4c, dt +0xa48, frame dt +0xa44, contact interval +0xa54 (1 human / 3 AI); contact query, surface sample, ABS/TC, brake torque, gearbox 0x1446a6b60, Car_Solver_PrepareDrivetrain 0x1446db280 | su-verify claim 1/4 (agent susp): physics body (*(car+0x900), alloc 0xa80, vtable 0x145bff4d0, body+0x18 = car, set at 0x144629600) vtable +0x8 = per-frame update; last call = suspension lane loader 0x1446db280 (before the substep loop)
- engine-re finding for Car_Physics_StepVehicle 0x1446dceb0: Per-car physics step: calls Car_Suspension_SolveWheels, the powertrain 0x1446e2290, aero; computes front downforce into F1Aero +0x19ec, rear into +0x19f0 and the drag force magnitude into +0x19d8 from aero curves (Car_Spline_EvalObfuscated on +0xf10, +0x11b8, +0x470, +0x718), wheel ride heights (wheel +0x248 - +0x168) and XOR-obfuscated coefficients (+0x1718^+0x1720, +0x18f8^+0x1900, +0x1938^+0x1940), scaled by +0x19a4. Wing damage factors live at F1Aero +0x1998 / +0x199c (written by Car_FrontWing_OnCollision and Car_Damage_SetComponentDamage). | 2026-10-28 engine-re (agent car-chassis): per substep HV < 7: Integrate 0x1446e1920 -> Contact_RefreshSubstep 0x1446dafa0 -> Tyre SolveWheels 0x1446dee60; HV >= 7: 0x1446dafa0 -> 0x1446dee60 -> 0x1446e1920; then powertrain 0x1446e2290; force body+0x920 / torque +0x930 accumulation; semi-implicit Euler v +0x8e0, x +0x8d0, w +0x8f0, orientation +0x8a0..+0x8c0; external force/torque inputs body+0x900/+0x910 | su-verify claims 1/5/6: body vtable +0x10, per substep (dt +0xa48, count +0xa4c, idx +0xa50); calls Car_Suspension_Integrate 0x1446e1920 in both HV branches; aero ride height (HV >= 7): rear 0x1446cb260 at 0x1446dd53c, front 0x1446cb000 at 0x1446dd570, normalised in clamps -> CL splines F1Aero+0x470 (rear) / +0x9c0 (front) -> +0x19ec / +0x19e0; no explicit rake input found (other splines +0xf10/+0x11b8 from velocity angle vs body axis). Active suspension: pre-detour here and write body lanes (rest +0x360, k +0x370, ARB +0x380, heave +0x400/+0x410, dampers +0x3a0..+0x3d0 + knees +0x3e0/+0x3f0) as axle baseline + delta every call (loader overwrites each frame); per-car parallel jobs (thread-safe); detour the function, vtable is shared .rdata
- engine-re finding for Car_Suspension_PublishWheelState 0x1446e4330: Copies the solver's per-wheel arrays into the wheel objects (SuspensionSystem +0x60 + i*8): +0x3b4 slip ratio <- +0x760, +0x3c4 longitudinal force <- +0x7e0, and ~15 more fields (+0x3a4, +0x3ac, +0x3bc, +0x3cc, ...); also reads the engine +0x2554 value through 0x142e88290. | su-verify claim 7: body vtable +0x20; wheel(body+0x60+i*8)+0x168 = body+0x170[i] (suspension position, read by aero ride height -> aero one step behind, H) plus +0x45c/+0x460/+0x154/+0x3a4..+0x3cc/+0x7c/+0x250; caller not in 0x1446dc480 (once per frame unverified)
- engine-re finding for Car_Read_F1Plank_Thunk 0x144652aa0: pl-verify claim 2: jmp 0x1553454b0; F1Plank data vtable 0x145bf8a50 (dtor 0x1447609e0, 0x144652920, Read, Write 0x144652930), strings F1Plank / State constants 0x145bf8a70; OffsetX/Y/Z +0x14/+0x18/+0x1c, ExtentX (+0x20, obfuscated offset) /Y +0x24 /Z +0x28; no wear field, no plank-wear / skid-block / plank-DSQ string in readable sections -> no plank wear rule. Stock Ferrari 2025 offset (0,-0.253,0.24) extent (0.11,0.01,1.285)
- engine-re finding for Car_Physics_AdvanceCar 0x14473fd80: 2026-10-28 engine-re (agent tick; tr-verify claim 1): whole per-car tick, NOT only input: lap records, phase lists P0 -> controls 0x144741700 + brake bias/torque -> P1 -> chassis snapshot -> P2 -> body vt+0x08 Car_Physics_Update (0x144740b28), [body+0xa50] = 0, then ONE of two loops while +0xa50 < +0xa4c: vt+0x10 StepVehicle (0x144740b66) or vt+0x18 StepForcesOnly (0x144740b96) when g 0x146ce89d8 (cutscene/kinematic) or car+0x128 == 1; vt+0x20 EndFrameAverage (0x144740bb9) -> P3 -> car+0x9db0 += dt -> P4 -> accel outputs, surface counters. Phase lists car+0x4f8+8p (16-byte {fn,ctx}). Runs on job workers in parallel per car | cf-verify claim 4: d = [car+0x9ea8]^[car+0x9eb0]; car+0x130 b1 = (d == 0) written at 0x144740170, b0 = ((d & 0x3f8d) == 0) written at 0x144740190 only when changed (per update, H every frame) | bt-verify claim 6: car+0x9f1c (0x144740efc) = wheels with legal main (wheel+0x208) or contact material (+0x1a0/+0x19c): car+0x9f28 == 0 -> SDT+ SDT* SDTE SDT MKNG RMBL, else SDT+ SDT* SDT MKNG; per-wheel grace car+0x9f24+i while < N, N = byte 0x145faa64c (read 0x144740d92, written 0x1435a0c52 from key 0058e5f9544d3377 = 2); forced 4 if byte 0x146ce89d8 or car+0x128 == 1; car+0x9f20 (0x144740fd0) wheels on PDT+/DTAC/DDTC (H)
- 2026-10-29 11:48 engine-re: engine-re findings 2026-10-29 for the car lane owner (static, blind-verified v3-verify; details SwoftyLauncher work/tyre2 + work/gearbox): TYRE - Car_Tyre_UpdateTemperature 0x1446b7b70: tread T wheel+0x64, carcass T +0x68, patch flash T +0x6c, patch heat capacity +0x70 (contact patch 0x1446b8210); convection h = env vt+0x20()*(TT+0x6b4 + |v|*TT+0x6b8 + v^2*TT+0x6bc), own speed only (no wake); AI temps clamped toward optimum (+0xc0 carcass / +0xc4 tread). Car_Tyre_UpdateWear 0x1446b79f0: gate car+0x132 b7 && byte 0x146ce89d8 == 0 && car+0x128 != 1; factor min(TT+0x658, r^2), r = (Tpatch + TT+0x654)/(Topt + TT+0x654); no abrasiveness; wear wheel+0x80 clamp 0..1. Grip mult 0x1446b76f0 clamp [TT+0x24, TT+0x28] (AI min 0.6). Blister growth skipped by cmp [rax+0x9f40],0xd / jl at 0x1446b8130. GEARBOX - gb = [car+0x538]; Car_GearBox_Reset 0x1446a4c20(gb, mask): bit 0x20 required, 0x200 rebuild gb+0x5fc[11] = 1/(gear*FD) from XOR data (FD +0x28, R +0x38, N +0x48, 1st +0x58.. stride 16) or plain +0x628/+0x654 when +0x540; mask 0x2000 FD = lerp(obj+0x198, obj+0x19c, clamp01(F1Tuning+0x258)), obj = F1Tuning vt+0x40(). Shift points 0x1446a5130 (thunk -> .data1 0x155407430): up -> gb+0x6a4, dn = 0.95*up[i-1]*GearDn mult -> +0x6d0. 0x1446db280 copies gb+0x5fc[gb+0x680] -> solver+0x4e8. VTF GearBox stock ratios in work/gearbox/gear_ratios.tsv.
- 2026-10-29 12:29 engine-re: engine-re prototype corrections 2026-10-29 (found by the fuel-modes addon via capstone register use before the first call; a wrong proto crashed the game 2026-10-28 19:43): Car_Tyre_UpdateTemperature 0x1446b7b70 is (ctx = wheel+8 in rcx, tyre object in rdx, float dt in xmm2), not (ctx, dt) - the old proto gives ACCESS_VIOLATION at cmp [rdx+0x200]; Car_Tyre_UpdateWear 0x1446b79f0 is (ctx) only, no dt ([rcx] = TyreType, rcx+8 = car, writes wear ctx+0x80); Car_Chassis_UpdateMassInertia 0x144678300 is (chassis, float xmm1); Car_Physics_StepVehicle 0x1446dceb0 reads only rcx before its first call. Confirmed: Car_Engine_UpdateTemperature 0x144689710 (engine, dt xmm1), Car_Tyre_GripMultiplier 0x1446b76f0 (ctx, wheel), FuelECU setMix body 0x1553ff3d0 (ecu, mix), Car_Gearbox_Update 0x1446a6b60 (gb, dt xmm1), Car_GearBox_Reset 0x1446a4c20 (gb, edx mask). Gearbox note: Car_GearBox_Reset with mask 0x200 also sets gb+0x680 = 0x100000001 (current/target gear -> neutral), so it must not be called mid-race; live ratio changes should scale the plain gb+0x5fc/+0x6a4/+0x6d0 tables instead.
- 2026-10-29 13:12 engine-re: engine-re findings 2026-10-29 for the car lane owner, reliability (static, blind-verified v5-verify; details work/failures/findings_failures.md): Engine = POINTER at car+0x530; Car_Engine_UpdateWear 0x1446893c0 (gated car+0x130 >= 0xC0) updates ICE +0x254c / TC +0x2550 / MGU-H +0x2558 (if car+0x5a8) / CE +0x2560 by dt/3600*car+0x9f2c*CE spline(Engine+0x1f40)*Engine+0x25a8*XOR rate; ICE >= 1.0 with car+0x130 b6+b1 and car+0x131 b0 -> Engine+0x2530 dead bit (50/50 kind 0x131 0x10/0x20), wear pinned 0.99. Gearbox faults 0x1446a60d0 (car+0x130 b6 + 0x131 b6, rate car+0x9f30): XOR params +0x420 perm jam, +0x440 temp jam (+0x65c), +0x480 missing gear (+0x664 random gear, +0x668), +0x4c0 slow shift (+0x66c mult, +0x670). Canned failures (game lane): schedule 0x1409b9aa0 (session types 15..17), confirm 0x1409b3cb0 (component 0 ICE, 1 MGU-K, 2 MGU-H, 3 ES, 4 TC, 5 CE, 6 gearbox +0x674 -> wear 1.0, state +0xa1 = 5); Game_Failure_Apply 0x1409b3190 0x34-byte record (type [0], time +0x10); types 0x1445fba10 Engine..EngineLeanFuel. gamedb engine_management.components.components_0/1.component_N.allocation (2025 ce 2 es 2 ice 4 mguh 4 mguk 4 tc 4 gearbox 24; 2026 ce 3 es 3 ice 4 mguk 3 tc 4 gearbox 24).
- 2026-10-29 09:36 engine-re: Request to the car lane owner (engine-re, 2026-10-29): the vmknolle phys-dump addon wraps Car_Contact_ModifyVehicle 0x142f04040 (pre-solve contact modify for CARW/CARB; details in engine-re PRs #489/#497). The row arrives at L1; it needs your L2 + a func_start signature to be exported.
- 2026-10-29 20:14 engine-re: engine-re carlive findings 2026-10-29 for the car lane owner (static, blind-verified carlive-verify; rows stay at their level): live per-car levers for the fuel-modes addon - steering Lock [St+0x28]^[+0x30] and per-side fractions +0x118/+0x128 (XOR, <=1); clutch gb+0xe8 PLAIN (not XOR, recomputed into +0x698 every frame); turbo MaxBoost Turbo+0x2d8 plain, read only in TurboMode 2 (parser stores Engine-def +0x21f0, powertrain sees +0x2208); diff +0xec/+0xf0/+0xf4 plain, re-written only on setup apply; body damage gain = .rdata 0.0003f 0x145bf68d0; wing +0x2c and suspension +0x128 plain per-hit multipliers. Corrections: Car_Update 0x14473fd80 calls the substep loop (not inside StepVehicle); its steering clamp to +-Lock is only on the speed branch.
- engine-re finding for Car_BodyDamage_OnCollision 0x1446512c0 (row comes with topic plank-floor): carlive 2026-10-29 (static, carlive-verify partial): k = 0.0003f global .rdata const 0x145bf68d0 (k_Car_BodyDamage_ImpulseScale), sole ref vmulss at 0x14465162e -> global body damage gain for all cars (patching it = global scale, H safe). No unique func_start sig; mid sig car.body_damage_impulse_scale_site.
- engine-re finding for Car_SuspensionDamage_OnCollision 0x144654d10: carlive 2026-10-29 (static, carlive-verify partial): F1SuspensionDamage (car+0x5d0; name by adjacency: string ref 0x144653062 next to registrar 0x144653ad0 which leas this fn). [susp+0x128] PLAIN float read at 0x144654e98. Findings: dmg = +0x128 * matDmg * FrontDamageScale(+0x38^+0x40) or RearDamageScale(+0x48^+0x50) * impulse * clamp01(|v|/SpeedThreshold(+0x28^+0x30) - 1) -> +0x130..+0x13c per wheel; +0x128 written only by the .data1 reset (H = Reduced x0.5). Live lever for damage gain and joint strength (H).
- engine-re finding for Car_FrontWing_OnCollision 0x14465fdc0: carlive 2026-10-29 (static, carlive-verify partial): plain multiplier [wing+0x2c] read live per hit at 0x144660096 (= data DamageScale +0x14, H); class F1WingDamage (car+0x5c8) by adjacency only (typeName getter 0x14465f440 next to registrar 0x14465f620 which leas this fn).
- engine-re finding for Car_Differential_Reset_Thunk 0x14467e010: carlive 2026-10-29 (static, carlive-verify partial): jmp stub to 0x1553d6f70 (.data1, protected) - write side not statically verifiable. Findings: on setup apply (mask 0x2000) writes diff +0xec off-throttle / +0xf0 on-throttle / +0xf4 mid-corner = lerp(F1Tuning Min/Max, clamp01(F1Tuning+0x218 / +0x214)); stock 2025 0.1..0.5, 2026 0.1..1.0. The multiples are live plain floats read by Car_Powertrain_Step, re-written only here; values > 1 not clamped (H unsafe).
- engine-re finding for Car_Read_Engine 0x144685680: carlive 2026-10-29 (static, carlive-verify partial): CORRECTION - the TurboMode parser at 0x144686154 (Off->0, LagSpoof->1, Full->2) stores to Engine-def +0x21f0 (rsi; table 0x145bfa1b0 with the Engine getter); the powertrain reads the same field as [eng+0x2208] (+0x18 shift, consistent: parser MaxLimiterRateFactor 0x878^0x880 = powertrain 0x890^0x898). Ferrari 2025/2026 VTF TurboMode=Full (2).
- engine-re finding for Car_Read_GearBox 0x1446a39a0: carlive 2026-10-29 (static, reader not blind-verified): CORRECTION - MaxClutchTorque data +0xd0 is read as a plain float -> runtime gb+0xe8 PLAIN, not an XOR pair (plain vmulss use confirmed by carlive-verify in Car_GearBox_Update); Low/HighThrottleBitePoint data +0xd8/+0xe8 XOR -> runtime +0xf0/+0x100. Stock MaxClutchTorque multiple 1.5 (2025) / 1.8 (2026).
- engine-re finding for Car_Steering_Update 0x1446a78d0 (row comes with topic car-garage): carlive 2026-10-29 (static, carlive-verify agree): 0x1446a7979-0x1446a79a5 capA=min([St+0x128]^[+0x130],1)*Lock, capB=min([St+0x118]^[+0x120],1)*Lock (Lock=[+0x28]^[+0x30]); wheels [+0xd0]/[+0xd8]+0x178 clamped to [0,cap]/[-cap,0]; outer wheel x (Lock+[+0x38]^[+0x40])/Lock; angle integrated in [+0xe8]^[+0xf0]. +0x118/+0x128 = XOR per-side lock fractions (<=1, can only reduce lock); addon lever = Lock XOR +0x28 (key +0x30), not re-copied per step (H). Leaf fn without .pdata: no sigcheck-able func_start sig.
- engine-re finding for Car_Powertrain_Step 0x1446e2290: carlive 2026-10-29 (static, carlive-verify partial): CORRECTION - turbo MaxBoost [Turbo+0x2d8] (Turbo=[[eng+8]+0x5a0], PLAIN float) is read only in TurboMode 2 (cmp [eng+0x2208],2 at 0x1446e3a27 -> vmovss at 0x1446e3a4e, turbo != null); mode 1 branch at 0x1446e26b3. Stock BoostEfficiencySpline flat 1.0, so MaxBoost changes no torque on stock cars (H). Diff multiples read plain: 0x1446e3e3d [diff+rcx*4+0xec] (off/on throttle by torque sign), 0x1446e3f39 +0xf0, 0x1446e3f5b +0xf4 (mid-corner), 0x1446e401b (diff=[car+0x578]). Once per substep from Car_Physics_StepVehicle (unproven statically).
- engine-re finding for Car_Physics_AdvanceCar 0x14473fd80: carlive 2026-10-29 (static, carlive-verify partial): CORRECTION - Car_Update CALLS the substep loop (0x144740b60-0x144740b7d: vtbl+0x10, or +0x18 at 0x144740b90, of [this+0x900] while ++[+0xa50] < [+0xa4c]); it is not inside StepVehicle. Steering target at 0x1447404f9: St=[this+0x560], Lock=[St+0x28]^[St+0x30], target=Lock*[[St+8]+0x1c8]; CORRECTION - the clamp to +-Lock (0x144740783-0x144740798) is only on the speed-dependent branch, the jbe at 0x14474053a/0x14474056f skips it (unclamped Lock*input); rate=(target-angle [+0xe8]^[+0xf0])/dt -> [+0xf8]^[+0x100]. Per-frame per-car not proven statically (indirect callers only).
- 2026-10-30 22:09 tyrealloc: tyrealloc 2026-10-30: f1_1990_brand.dll calls Car_Tyre_SetWearFactor 0x142ea2300 (sig car.tyre_set_wear_factor). Proposed L2 (lead --force, owner car-re): proto void Car_Tyre_SetWearFactor(void *tyre_component, bool apply_set, uint16_t force); evidence decomp:re/decomp/142ea2300.c;xref:0x142e946b7,0x142ea2da4. Reads tc+0x20 -> +0x58 CarLive; per wheel formats '%s_%s%c' (0x1457ef36f) from [CarLive+0x558 Chassis]+0x128 (TyrePhysicsGroup string object +0x118: +0x08 capacity, +0x10 buffer, set by 0x1446506e0), the class entry physics code and f/r, lowercases, finds it in [[0x1460806d8]+0x50]+0x138 (0x144776d00) and binds via 0x155436e70 (reset = low byte of force; force >= 0x100 forces). apply_set: 0x142e88840 with the fitted set of +0x9c68. Car_Tyre_ChangeToNextCompound calls it with (tc, 1, 0x101) at 0x142e946b7. Also CarChassis.h +0x118 could split into capacity +0x120 / buffer +0x128 (decomp 1446506e0.c).
- engine-re finding for Car_Read_ConvexHull 0x14464aaa0: VTF car data reader for collision hulls: flags BoxifyFrontWing / BoxifyRearWing (out +0x10/+0x11), OffsetX/Y/Z (Game_Read_VtfFloat), then every ConvexHullData node: name, numVertices, generated, HullVertex list. Hulls with < 4 vertices (except 2) are ignored ('VTF: Too few vertices in a convex hull, ignored'), > 85 (0x55) vertices are truncated ('Large number of vertices ... truncated'). Addon/car mods: custom car collision hulls must have 4..85 vertices; wings can be boxified.
- 2026-10-30 20:15 engine-re: engine-re finding for Car_Tuning_SetFinalDrive 0x1446a4c20 (owner note; blind verify fd4-verify agree after three partial rounds): GearBox system reset with flags (function pointer stored at vtable entry 0x145bfb268); log F1_TUNING 'Setting final drive ratio to %f.'. Everything is gated by flag 0x20. 0x20|0x200: current and target gear +0x680/+0x684 = 1 (N); zeroes byte +0x694, dword +0x698, byte +0x69c, dword +0x6a0; copies +0x604 to +0x600; rebuilds the inverse total ratio table at +0x5fc = 1/(ratio x final drive) from the VTF XOR pairs (ratio i = +0x38+16i ^ +0x40+16i, final drive +0x28 ^ +0x30), or from setup_ratios +0x628 / setup_final_drive +0x654 when byte +0x540 != 0; +0x5f8 = top gear index (last entry > 0); then shift-point rebuild 0x1446a5130. 0x20|0x1000: byte +0x658 &= 0xe0 (low 5 bits cleared), +0x65c..+0x663 = 0, +0x664 = 0xb, +0x668 = 0, +0x66c = 1.0f, +0x670..+0x67f = 0. 0x20|0x2000: writes setup_final_drive +0x654 = lerp(obj+0x198, obj+0x19c, clamp01(F1Tuning slider +0x258)) on the gearbox at plVar3[1]+0x538, rebuilds that table if its +0x540 != 0, then always rebuilds this gearbox's table and makes 11 calls to 0x1446d8480; the current gear +0x680 is NOT changed. Tail: +0x688 = +0x68c = FLT_MAX, +0x690 = 1.0. Runtime value of +0x540 on F1 cars unknown (needs in-game read).
- 2026-10-30 23:26 std-cars-live: std-cars-live 2026-10-30 (PR #572): proto proposals so bindgen emits typed bindings (parse_proto needs the name): Car_Tuning_SetFinalDrive 0x1446a4c20 'void Car_Tuning_SetFinalDrive(void *gearbox, uint32_t flags)' (rcx=GearBox system, edi=flags, test dil,0x20 at 0x1446a4c40; agree via the fd4-verify row of #586); Car_Engine_ComputeDerived_Thunk 0x144689a70 'void Car_Engine_ComputeDerived_Thunk(void *engine)'; Car_Systems_Reset_Thunk 0x1447426c0 'void Car_Systems_Reset_Thunk(void *loader, uint32_t arg)' (current protos lack the function name, so bindgen drops them). cars::live calls SetFinalDrive with 0x2020 (re-apply setup + rebuild inv_total_ratio, no gear change). Open runtime question for T122 follow-up: GearBox+0x540 use_setup_ratios on F1 cars (non-zero = VTF ratio/final-drive edits are ignored by Reset).
- 2026-10-30 23:42 std-cars-live: std-cars-live 2026-10-30: renamed CarEngine+0x8b0 limiter_rotation_rate -> vtf_limiter_rotation_rate (clashed with +0x2528 limiter_rotation_rate, duplicate member in game.rs/car.hpp once both export); verify V2395 now keys the L3 +0x2528 field, the +0x8b0 verdict is carried over as a new row. Verifier tidy-ups: CarEngine.car read xref 0x1447486ec is wrong (real write 0x144748528, read 0x1553f5462); CarDamageFrontWing.chassis_damage_multiple xref 0x14466005b is the block start, real reads 0x144660173/0x144660226 (V2411 partial); CarGearBox.use_setup_ratios is filled from DB key AllowGearRatioTuning (bool getter 0x14464c220). bindgen emits CarObfFloat as 12 bytes (a, pad, b) but size is 0x10: offsets asserted, arrays of it would be wrong.
- engine-re finding for Car_Upgrades_CalcTypeMass 0x1435352a0: (upg, partId, arg) When mode +8 == 2: robin-hood hash lookup of partId (hash *[+0x1a60], tables +0x1a68/+0x1a70, mask +0x1a80) and returns progress via Game_Rnd_CalcUpgradeTypeProgress (float). Called from Car_Upgrades_CalcNonLinearMass.
- engine-re finding for Sess_Build_VehicleTyreInfo 0x1436ad6c0: Predicts per-wheel tyre wear (out param_1[4]) over a stint: param_2 car tyre system (+0x20 TOD/session data, +0x90 weather, +0xd8 compound sets, +0x108 track data), param_3 set index, param_4 compound (<6 via table 0x145859b04), param_5 per-wheel start wear, param_6 time offset. Per lap: track temp from TOD (slot +0x26c4 stride 0x288, +0x330/+0x338/+0x348) and weather curve, compound curves (+0xec0/+0x138 via 0x142e88290). 15 callers incl. Sess_TyreAllocation_OnSessionStart, Sess_RedFlag_SimulateUnlapLaps, Game_AiPitDecision_UpdateDroppedPlayerCar, Game_Tyres_Deserialise.
- engine-re finding for Car_Systems_FindOrCreate 0x144759eb0: (vtf, data, factory): finds a VTF component by name (case-insensitive, list +0x20 count +0x18) or creates it via factory+0x38, appends and rebuilds (Car_Build_VtfComponents), then calls its vtbl+8(data). Caller: Car_Assets_CreateSystemForPerformanceMode.
- engine-re finding for Car_AnimClipTarget_SetAnimation 0x140399720: (clipTarget, out, name, flags): under critical section +0x1218 binds a vehicle animation by name (JIT bind) for the in-car driver ('DRIVER'); sums channel counts from +0x12b0 lists, writes out={this+0x1270, weight 1.0}; logs 'InCarDriverActor::VehicleAnimationData::NeVehicleClipTargetImpl::setAnimationName JIT bind failed'. Caller: Car_Model_BindCockpitNodes.
- engine-re finding for Car_Visual_UpdateAttachedAnimators 0x14039ff50: (carVisual, dt): ticks animator lists of two model nodes (+0x5d8/+0x5e0 and +0x1390/+0x137c) via vtbl +0x20. Caller: Car_Visual_UpdateRenderState. la0-verify disagree: computes min start/max end of each animation list and writes lerp(param_2) to +0x5d4/+0x1384; does not tick
- engine-re finding for Car_Aggregate_UpgradeStats 0x1409e3fb0: second R&D aggregate consumer (decompiled at entry 0x1409e6000); re-reads upgrade_type + bhp_adjustment/mass_adjustment by name and copies ~5-byte enum arrays; data/string-driven, not a dept[4] index lb03-verify partial: parses upgrade EffectDefinition data lb03-verify partial: parses upgrade EffectDefinition data
- engine-re finding for Car_Load_SetupPreset 0x140c04fd0: Gdb_Find_RowByLabel(car_setup.presets, vehicle_package_setups, package_label) (both names stack-built), then reads <row>.<ai|player>.<variant>.<slider> with Gdb_Get_FieldInt for the 42 car_setup.sliders labels; variant names from the table at 0x14568eaf0 (index 0..4, else 0x145636190). A package without a preset row gets no setup; a row with only _metadata_ crashed session load (read of index -1).
- engine-re finding for Car_Setup_SetSliderStep 0x140c11160: Step-int setter on the active slot, calls Car_Setup_ApplyTuning at 0x140c11271 (agent setup, sp-verify)
- engine-re finding for Car_Setup_OnValueChanged 0x140c1e690: Log channel CAR_SETUP: 'setup value %s changed from %f to %f'
- engine-re finding for Car_Read_WearRatesConfig 0x1411cb150: named from its log messages (channel -): 'Reads settings/game DB keys: engine_management.driver_wear_radriver_wear_rateMisconductMessag, q.world.create, wear_rates.ai_pq_max, wear_rates.ai_pq_min, wear_rates.ai_race_max, wear_rates.ai_race_min, wear_rates.human_max, wear_rates.human_min' lb05-verify disagree: keys random_component_swap_chance, component_percentage, gearbox_percentage
- engine-re finding for Car_Component_Fit 0x141f58a20: named from its log messages (channel VEHICLE): 'Fit component %I64u (exists: %hs), type: %hs' lb07-verify partial: PU components fit-selected handler
- engine-re finding for Car_Spline_EvalObfuscated 0x142e88290: Evaluates a physics spline whose numbers are stored XOR-obfuscated: each float is (u32 at +o) ^ (u32 at +o+8). Uniform mode (count +0x90 >= 0): index = x*scale(+0x60^+0x68) + offset(+0x70^+0x78), 16-byte samples from +0x98 (value ^ +0xa0), clamped and lerped. Point mode (count < 0): 32-byte points in [+0x8,+0x10), x = +0^+8, y = +0x10^+0x18, lerped. Used for VTF splines (engine, aero, tyres) at runtime; memory edits of handling values must keep the XOR pairing. No signature: the generated pattern fails sigcheck.
- engine-re finding for Car_TyreSets_Serialise 0x142e8da20: named from its log messages (channel TYRES/V): '%s: tyre allocation choice: %d, compoound list size: %d | %s: ix: %d, write position: %d | %s: %d tyre sets, ok: %d | %s: wrote m_tyreSetToStartRaceOn, write position: %d | %s: compound: %s, index: %d'
- engine-re finding for Car_TyreSets_Deserialise 0x142e8deb0: named from its log messages (channel TYRES/V): '- | %s: tyre allocation choice: %d, compoound list size: %d | %s: after reading m_tyreSetToStartRaceOn, ok: %d, read position: %d'
- engine-re finding for Car_TyreAllocation_RebuildForSession 0x142e8edd0: Rebuilds a car's tyre allocation for the session (param_3 = session type, 0x10/0x11 special-cased): reads options via 0x140b03110, refills compound groups 0,1,2,3,5,6 through 0x142e8eaf0 and merges 0x6c-byte set records. Caller Sess_TyreAllocation_OnSessionStart.
- engine-re finding for Car_Tyre_ChangeToNextCompound 0x142e94040: Log channel PITSTOP_TYRES: 'TyreComponent::changeToNextCompound %p - PITLANE CHANGEOVER' | 'MOX/Tyres: changeToNextCompound() new type %d is not the same as next type '%s' (from %d)'
- engine-re finding for Car_Tyre_ActivateCompounds 0x142e96390: Log channel PITSTOP_TYRES: 'TyreComponent::activateTyreCompounds %p - PITLANE ENTER' | 'MOX/Tyres: activateTyreCompounds() new type %d, old type %d'
- engine-re finding for Car_Tyre_CacheWingDamage 0x142e97060: Log channel PITSTOP_TYRES: 'TyreComponent::cacheWingDamage %p - PITLANE ENTER'
- engine-re finding for Car_TyreSelection_SetCurrentTyres 0x142e9eda0: Log channel TYRES: 'TyreSelection::setCurrentTyres(compound=%s , index=%d , lap=%d) called on non authoritative machine, we already have data the lap, ignoring request.'
- engine-re finding for Car_Tyre_SetWearFactor 0x142ea2300: Log channel PITSTOP_TYRES: 'Wear factor set to %f on vehicle %d'
- engine-re finding for Car_CarbonDamage_OnImpulse 0x142f13de0: Log channel CARBON_DAMAGE: 'Vehicle %d - impulse %f (max impulse %f)'
- engine-re finding for Car_DamageComponent_ResetNoseCone 0x142f146b0: Log channel CARBON_DAMAGE: 'VehicleDamageComponent::resetNoseCone for vehicle %d' | 'VehicleDamageComponent::resetNoseCone m_car is nullptr'
- engine-re finding for Car_TyreDamageMonitor_Update 0x142f15ce0: Tyre damage monitor (log channel TYRE_DAMAGE_MONITOR). Copies the maximum number of concurrent punctures (monitor+0xf74) into the vehicle tyre state; logs 'Set the maximum number of concurrent punctures to %d.'. lb11-verify partial: rebuilds per-vehicle slot table
- engine-re finding for Car_CarbonDamage_PrintSetting 0x142f19ad0: Log channel CARBON_DAMAGE: '%s = %s' | '%s = %f'
- engine-re finding for Car_CarbonDamage_Init 0x142f1a700: Log channel CARBON_DAMAGE: 'Number of drivers = %d' | 'Renderer.Gfx.CarDamage.DamageMask.width = %d' | 'Renderer.Gfx.CarDamage.DamageMask.height = %d' | 'Renderer.Gfx.CarDamage.maxCollisionsPerVehicle = %d'
- engine-re finding for Car_CarbonDamage_OnSessionRestart 0x142f1b010: Log channel CARBON_DAMAGE: 'onSessionRestart active=%d isTimeTrial=%d'
- engine-re finding for Car_CarbonDamage_RegisterImpactCallback 0x142f1b2b0: Log channel CARBON_DAMAGE: 'registerImpactCallback for vehicle %d' lb11-verify partial: per-vehicle impact handler
- engine-re finding for Car_Component_CheckCornerSpeed 0x142f2a0d0: named from its log messages (channel VEHICLE_COMPONENT): 'We have somehow calculated an impossible speed for a corner, delta time is: %f' lb11-verify partial
- engine-re finding for Car_Read_VehicleAttribute 0x14351df60: reader invoked by Car_Apply_UpgradeToVehicle; result stored into effectDef+0xa4; reads the vehicle performance attribute array. decomp body at re/decomp (regen at 0x14351df60 for full)
- engine-re finding for Car_Upgrades_CalcNonLinearMass 0x143535480: Log channel VEHICLE_UPGRADES_MASS: 'Calculating non linear mass for %s. Linear mass: %f, Target mass at max: %f, Mass fraction: %f, Non-linear mass: %f'
- engine-re finding for Car_Chassis_Build 0x144628190: Chassis runtime (vtable 0x145bf73a0 slot 7): finds wheel nodes, builds the VTF components (Car_Build_VtfComponents) and the suspension rig; shadow LOD flags. | Car init: for phases 0..6 calls comp->vt+0x28(comp, &{fn,ctx}, p) over components car+0x3a0 (count +0x398) (0x14462979a); lists car+0x4f8+8p, counts car+0x4dc+4p; also mov [car+0x5f0], car (0x1446288bf). Phase map: Chassis P0/P1 mass/P4/P5/P6, Suspension P0/P2/P3/P4/P6, WheelProxies P0/P5, ERS P1, F1Aero P1, Engine P2, Steering P2, FuelTank P4, damage components P1/P4/P5 (agent tick, tr-verify claim 4)
- engine-re finding for Car_Parse_Chassis 0x14462e6d0: VTF Chassis / ChassisComposite section (Mass, COM, MOI, visual height offsets). lb22-verify partial
- engine-re finding for Car_Physics_ComputeGroundedPose 0x144632740: (carPhys, outMatrix4, x, z, fwdDir*, dropY, mode): computes a resting car pose on the ground at (x,z) dropping from dropY (callers pass slotY+3.0); uses gravity 9.81, suspension/ride-height data at carPhys+0x558/+0x580 (wheel bodies +0x70..+0x98, +0x1598), ground query 0x1440e3cb0 on phys world +0x1130; mode==1 writes the 4x4 result into outMatrix rows. Callers: Sess_VehiclePlacement_PlaceOnSlot (grid/garage slot placement), Game_PitBoxCarActor_Update, Game_PitLane_UpdateMovement, Sess_DoughnutSequence_Enter.
- engine-re finding for Car_Physics_PostVehicleUpdate 0x144642ab0: 'postPhysicsVehicleUpdate' job: parallelForEach over vehicles after the physics advance.
- engine-re finding for Car_Write_ConvexHull 0x14464a7f0: VTF ConvexHull: numHulls, BoxifyFrontWing/RearWing, offsets, ConvexHullData hull vertices (collision shape). Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Puncture_Add 0x14464e200: Log channel TYRE_DAMAGE_MONITOR: 'Added puncture for vehicle %d tyre %p.'
- engine-re finding for Car_Puncture_Remove 0x14464e3a0: Log channel TYRE_DAMAGE_MONITOR: 'Removed puncture for vehicle %d tyre %p.'
- engine-re finding for Car_Assets_FindUInt 0x1446508d0: named from its log messages (channel VEHICLE_ASSETS): 'UInt "%s" NOT found.'
- engine-re finding for Car_DamageMeshes_Init 0x144650b00: the object at car+0x5e0: damage meshes at +0x58 (stride 0x60; name object at +0, u16 current +0x58 / max +0x5a) for rearwing_*, sidepod_*, sidewing_*, wing_mirror_*, bargeboard_*, floor_damage_*, camera_wing, topcanopy_bm, diffuser_bm; also loads the 'rearwing' collision hull.
- engine-re finding for Car_Collision_AddListener 0x144651f90: (agent plank) thunk -> 0x155339950 (.data1): AddListener(listenerList, {fn, ctx}); used by Car_Chassis_Build for 0x144705380; route for addon contact listeners
- engine-re finding for Car_SuspensionDamage_OnCollision 0x144654d10: carlive 2026-10-29 (static, carlive-verify partial): F1SuspensionDamage (car+0x5d0; name by adjacency: string ref 0x144653062 next to registrar 0x144653ad0 which leas this fn). [susp+0x128] PLAIN float read at 0x144654e98. Findings: dmg = +0x128 * matDmg * FrontDamageScale(+0x38^+0x40) or RearDamageScale(+0x48^+0x50) * impulse * clamp01(|v|/SpeedThreshold(+0x28^+0x30) - 1) -> +0x130..+0x13c per wheel; +0x128 written only by the .data1 reset (H = Reduced x0.5). Live lever for damage gain and joint strength (H).
- engine-re finding for Car_SuspDamage_UpdateJointLimits 0x1446563e0: carlive 2026-10-29 (static, carlive-verify partial): reads [susp+0x128] at 0x144656442/0x1446564f1/0x1446565a3/0x144656655; joint limits = (1-dmg)/(scale*+0x128) (findings); registered only from .data1 (0x1553581c1). Leaf fn without .pdata: no sigcheck-able func_start sig.
- engine-re finding for Car_Write_F1TyreDamage 0x144657040: VTF F1TyreDamage: rot rate and damage progression ranges, slip angle and force ranges, SlipAngleForceMultPunc / MultBlow (puncture vs blow-out), AirResistanceMult. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Read_F1TyreDamage 0x144657290: Log channel DEFORMATION: 'Loading node graph for tyre %d from VTF files for package %s.' | 'Tyre damage is %s.' | 2026-10-28 engine-re (agent car): this is the F1TyreDamage VTF data reader (Car_Read_F1TyreDamage, data vtable 0x145bf8d00, writer 0x144657040, component car+0x5d8) lb22-verify partial
- engine-re finding for Car_Assets_FindShortName 0x14465b020: named from its log messages (channel VEHICLE_ASSETS): 'Shortname "%s" NOT found.'
- engine-re finding for Car_TyreDeformation_SetTyre 0x14465c950: Log channel DEFORMATION: 'Tyre %d set to %s.'
- engine-re finding for Car_FrontWing_OnCollision 0x14465fdc0: front wing collision callback: adds damage to fw+0x48 / +0x4c (left/right, max +0x38) and to car+0x5d0 +0x138/+0x13c; ratio to [car+0x570]+0x199c. carlive 2026-10-29 (static, carlive-verify partial): plain multiplier [wing+0x2c] read live per hit at 0x144660096 (= data DamageScale +0x14, H); class F1WingDamage (car+0x5c8) by adjacency only (typeName getter 0x14465f440 next to registrar 0x14465f620 which leas this fn).
- engine-re finding for Car_Damage_SetComponentDamage 0x144660cd0: VEHICLE_DAMAGE log '%s: damage: %x, was broken, is broken': if the wing was broken and the bits clear (& 0x3ff == 0) -> slot 0x18(0x1000) reset ('reset damage'); else left damage = max * (bits >> 5) / 30, right = max * (bits & 0x1f) / 30, total ratio clamped to [0,1] into [+0x90]+0x199c ('setFrontWingDamage'). | 2026-10-28 engine-re (agent car): this is the FRONT WING damage setter (proposed Car_FrontWing_SetDamage), void(wing, u32 packed): left = (p>>5)*Max/30, right = (p&31)*Max/30, detach above Max/10 (vfunc +0x18, 0x1000) -> F1Aero+0x199c (M) lb22-verify partial
- engine-re finding for Car_Create_DetachablePart 0x144661440: creates a detachable part object (0x200 bytes, vtable 0x145bf94d0) in the car part manager car+0x2c8; fails when the model has no node with the id (0x143cfc8f0 = -1).
- engine-re finding for Car_Component_LogFitFailure 0x144662060: named from its log messages (channel VEHICLE_COMPONENT): '- | FAILED component name: %s, part: %s, model: %s, proxy %s' lb22-verify partial
- engine-re finding for Car_TyreDamage_TransformContactPatch 0x144663930: Large SIMD routine used by Car_TyreDamage_Update: inverts wheel transform param_3 (3x3 + translation param_3[3]) and transforms contact patch points into tyre space for damage evaluation. Callers: Car_TyreDamage_Update.
- engine-re finding for Car_DetachablePart_Update 0x1446686d0: part vtable slot 3: calls fn(ctx, part+0x30) every frame (fn/ctx at part+0x18/+0x20); true or hit count +0x130 >= 100 detaches (slot 0xa0, +0x38 = 1).
- engine-re finding for Car_ObfSpline_Ctor 0x14466b6e0: (ObfSpline *s, Allocator *a, uint capacity): allocates capacity*32 bytes, initialises the obfuscated header fields (+0x20..+0x58) with fresh MT19937 keys (global 0x146108c00), +0x90 = -1. Called from 0x1447546c0 (steering spline load).
- engine-re finding for Car_ObfSpline_SerialiseVertices 0x14466d150: (writer *w, name, ObfSpline *s): writes numVertices then a SplineElement {x,y} per 32-byte element via writer vtable (+0x28 begin, +0x48 array(name,count), +0x68 write float, +0x78 end); x = e[0]^e[2], y = e[4]^e[6] (XOR de-obfuscation). Called from the steering-spline save path (0x144674a61 block, Car_PadAssist steering splines).
- engine-re finding for Car_Assets_FindSpline 0x14466d250: named from its log messages (channel VEHICLE_ASSETS): 'Spline "%s" NOT found. | %s is being adjusted to make even | Old: | New:'
- engine-re finding for Car_ObfSpline_AddVertex 0x1446741a0: (ObfSpline *s, uint x, uint y): appends a 32-byte obfuscated vertex {x^k1,k1,y^k2,k2} with fresh MT19937 keys (global MT 0x146108c00 seeded from QPC), grows vector s+8..+0x18 (allocator s[0]), then recomputes the min-y vertex and stores it re-obfuscated in s+0x20/+0x40. Anti-tamper: steering-assist curve values are never stored in clear. Caller: Car_PadAssist_LoadSteeringSplines.
- engine-re finding for Car_Write_BrakeECU 0x144674a30: VTF BrakeECU: ABS and traction control splines (speed vs peak slip factor front/rear, wet variants, ATC strength), PunctureThrottleMax. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_BrakeEcu_ApplyBias 0x144676250: Bias ECU+0x21d8 (0..1) vs 0.5 (0x144676289): b <= 0.5 t = 2b, axle = Base*t + Backwards*(1-t); else t = 2(1-b), Base*t + Forwards*(1-t); * pressure ECU+0x21dc; writes XOR max torque wheel+0xf8 (key +0x100) for 4 wheels unless wheel byte +0x138 (0x14467637f/3ab/3da/405). Inlined copy in Car_Input_UpdateControls 0x14473fd80 (agent car, cg-verify claim 2)
- engine-re finding for Car_Tuning_SetChassisZCom 0x144676d40: Log channel F1_TUNING: 'Setting chassis z-com to %f.'
- engine-re finding for Car_Failsafe_LimitVelocity 0x144677780: Log channel VEHICLE_FAILSAFES: 'Angular velocity component %d of vehicle %d limited. Old: %f New: %f' | 'Linear velocity y component of vehicle %d limited. Old: %f New: %f' | 'Linear velocity up component of vehicle %d limited. Old: %f New: %f'
- engine-re finding for Car_Chassis_UpdateMassInertia 0x144678300: Every frame (from 0x144677720): m0 = sqrtMass^2 + Engine+0x2534 + GearBox+0x67c (fault masses) + fuel FuelTank+0x100 as box (sizes +0xc8/+0xd8/+0xe8, centre Y +0xa8 Z +0xb8) + chassis +0x32c..+0x338; mass-weighted COM, inertia with parallel axis; then 0x144676a80 (agents car-chassis, car-pu, static)
- engine-re finding for Car_Collision_CarToCar 0x14467c280: Log channel VEHICLE_COLLISION: 'Car to Car %p %p, time %f' lb22-verify partial
- engine-re finding for Car_Write_CoreAero 0x14467cfd0: VTF CoreAero: slip streaming constants (SlipStreaming, SlipStreamWhileBraking). Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Write_Differential 0x14467daa0: VTF Differential: DriveType, RearPowerProportion, DiffRearNoEffect/FullEffect/MaxLockingTorque, DiffMidCornerGForce, DiffGForceBlendSlope, RampDifferential. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Read_Differential_Thunk 0x14467ddf0: Body 0x1553d2720: DriveType strings 'FWD' 1 / '4WD' 2 / 'RWD' 3 (0x145bfa111/115/119, else 0); runtime diff car+0x578: +0x30 RearPowerProportion, +0x40 DiffRearNoEffect, +0x50 FullEffect, +0x70 MaxLockingTorque, +0x80 MidCornerGForce, +0x90 GForceBlendSlope, +0xa0 RampDifferential, +0xa8 driven count, +0xb0 driven ptrs, +0xd0 shares, +0xec off / +0xf0 on / +0xf4 mid multiples, +0xf8 high-speed kph, +0xfc preload (agent power, ap-verify claim 6)
- engine-re finding for Car_Differential_Reset_Thunk 0x14467e010: carlive 2026-10-29 (static, carlive-verify partial): jmp stub to 0x1553d6f70 (.data1, protected) - write side not statically verifiable. Findings: on setup apply (mask 0x2000) writes diff +0xec off-throttle / +0xf0 on-throttle / +0xf4 mid-corner = lerp(F1Tuning Min/Max, clamp01(F1Tuning+0x218 / +0x214)); stock 2025 0.1..0.5, 2026 0.1..1.0. The multiples are live plain floats read by Car_Powertrain_Step, re-written only here; values > 1 not clamped (H unsafe).
- engine-re finding for Car_Diff_SetupDrivenWheels 0x14467e7c0: Thunk -> body 0x1553e9240 (vtable 0x145bfa120 slot 6 ptr 0x145bfa150): driven list RL/RR ([car+0x580]+0x70/+0x78) + FL/FR (+0x80/+0x88) when DriveType [diff+0x28] is FWD 1 or 4WD 2; shares diff+0xd0..+0xdc (RL RR FL FR): 4WD 0.5*RPP / 0.5*(1-RPP) (RearPowerProportion used as FRACTION), FWD 0/0.5, RWD 0.5/0; car+0x225 bit0 = isRWD. FWD still has rear-only clutch coupling (0x1446e368f), LSD and clutch controller 0x144741700 (agent power, ap-verify claim 5)
- engine-re finding for Car_Read_Engine 0x144685680: Re-reads the Engine values by name through Game_Read_VtfFloat (upgrade / tuning overlay of the Engine section). Correction: this is the reader (component data vtable slot 2): it fetches each value by name with Game_Read_VtfFloat and stores it XOR-obfuscated (value ^ key 8 bytes later, e.g. Engine Mass -> +0x2f8 ^ +0x300); it runs for the base section and for every Upgrade block. carlive 2026-10-29 (static, carlive-verify partial): CORRECTION - the TurboMode parser at 0x144686154 (Off->0, LagSpoof->1, Full->2) stores to Engine-def +0x21f0 (rsi; table 0x145bfa1b0 with the Engine getter); the powertrain reads the same field as [eng+0x2208] (+0x18 shift, consistent: parser MaxLimiterRateFactor 0x878^0x880 = powertrain 0x890^0x898). Ferrari 2025/2026 VTF TurboMode=Full (2).
- engine-re finding for Car_Write_Engine 0x144686830: VTF Engine: RevsPowerSpline, ThrottleMap0-14, ThrottleMapClutchDisengaged0-14, Mass, Inertia, friction/engine braking splines, idle/limiter rates, EngineLayout, TurboMode, temperature, PU wear (WearICE/TC/MGUH/CE rates and splines), RedLineThrottleCutTime. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Tuning_SetEngineBrakingAndRadiator 0x1446878d0: Log channel F1_TUNING: 'Setting engine braking fraction to %f.' | 'Setting radiator opening fraction to %f.'
- engine-re finding for Car_Engine_UpdateWear 0x1446893c0: PU wear Engine +0x254c ICE, +0x2550 TC, +0x2554 ES, +0x2558 MGU-H, +0x255c MGU-K, +0x2560 CE; k = dt/3600*car+0x9f2c*WearCEPenaltySpline(CE)*Engine+0x25a8; ICE += WearICERate*WearICERevsSpline(rpm)*k; ICE failure at 1.0 (car+0x131 b0/b4/b5, LCG car+0x178); CE also vibration wear on kerbs (agent car-pu, static)
- engine-re finding for Car_Engine_UpdateTemperature 0x144689710: Engine temp +0x253c: heat from fuel flow*LHV minus shaft power * CoefHeatAbsorbed + engine braking, * FuelECU TemperatureScale (+0x160), / (Mass*Cp); cooling h(v) * area * radiator slipstream factor * coolant mult +0x25ac; gate car+0x130 b6, 0x131 b1; effect via TempEfficiencySpline (power) and MGU-H wear (agent car-pu, static)
- engine-re finding for Car_Engine_ComputeDerived_Thunk 0x144689a70: Body 0x1553f9a30: engine+0x2528 = ([+0x8b0]^[+0x8b8] LimiterRotationRate) * ([[car+0x548]+0x150]^ RevsScale) plain float (effective rev limit, 0x1553f9a6a); +0x2508 = base power +0x58 * PowerScale (XOR); +0x2518 peak torque. Followed by shift points 0x1446a5130 (= Car_Gearbox_ComputeShiftPoints) (agent power, ap-verify claim 10)
- engine-re finding for Car_Write_FuelECU 0x144689bf0: VTF FuelECU: FuelBurnScale/FuelPowerScale/FuelRevsScale for Lean, Standard, Rich, Max fuel modes. No unique byte signature: the body is identical to sibling VTF/DB section sections apart from wildcarded rip-relative string offsets. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_FuelEcu_Reset_Thunk 0x14468e550: FuelECU vtable 0x145bfa870 slot 3 (body 0x1553fdf60); flag 0x2000 forces Standard (mode 1). Practice programmes (PracticeFuelManagement 0x142971900 bit 0x01, PracticeERSManagement 0x142947400 bit 0x02, RacePace 0x14298d9d0 bit 0x10, TrackAcclimatisation 0x1429a06b0 bit 0x20) lock Lean/Rich/Max in EngineModeAvail and force Standard (fuel-mix, static) lb22-verify partial lc3-verify partial: thunk into .data1 behind vtable slot 3; FuelEcu specificity unconfirmed
- engine-re finding for Car_FuelEcu_SetMode_Thunk 0x14468e6a0: Thunk -> body 0x1553ff3d0: XOR-decodes Burn/Power/Revs/Temp per mode from ecu+0x28.. (+0x10 per mode, +0x40 per kind) into active ecu+0x130 burn / +0x140 power / +0x150 revs / +0x160 temp (keys +0x138/148/158/168), calls Engine_ApplyFuelMode 0x144689a70 and Gearbox_ApplyFuelMode 0x1446a5130, stores mode ecu+0x128; early return if unchanged, returns 1 (fuel-mix, fuel-verify claim 5)
- engine-re finding for Car_Write_FuelTank 0x14468e760: VTF FuelTank: BSFC min/max, LHV, optimal revs/torque, FuelCapacity (F1 110, F2 94), COM, size, FixedLoad. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_FuelTank_StepBurn 0x14468edb0: BSFC = Min + (Max-Min)*sqrt(((T-fric-OptTorque)*TorqueLinear)^2 + ((rpm-OptRevs)*RevsLinear)^2); flow +0x108 = rpm*T*BSFC*FuelBurnScale(ECU+0x130)*0.001; car+0x132 b2: fuel +0x100 -= flow*dt with FLOOR 0.05 kg human / 0.1 kg AI (never empty). Stock BSFC 4.75e-5/4.83e-5, LHV 42500, capacity 110 kg (agent car-pu, static)
- engine-re finding for Car_Write_GearBox 0x1446a2610: VTF GearBox: final drive, gear ratios R-9, shift speed multiples, clutch, shift timing, damage (DamageRateMidShift, OverrevvingDamageScale) and gearbox faults: JammedGearPermanent/Temporary threshold+chance+time, MissingGear threshold+chance+duration, ShiftTimeFault threshold+chance+scale+duration. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Read_GearBox 0x1446a39a0: Re-reads the GearBox values by name through Game_Read_VtfFloat (overlay). Correction: this is the reader (component data vtable slot 2): it fetches each value by name with Game_Read_VtfFloat and stores it XOR-obfuscated (value ^ key 8 bytes later, e.g. Engine Mass -> +0x2f8 ^ +0x300); it runs for the base section and for every Upgrade block. carlive 2026-10-29 (static, reader not blind-verified): CORRECTION - MaxClutchTorque data +0xd0 is read as a plain float -> runtime gb+0xe8 PLAIN, not an XOR pair (plain vmulss use confirmed by carlive-verify in Car_GearBox_Update); Low/HighThrottleBitePoint data +0xd8/+0xe8 XOR -> runtime +0xf0/+0x100. Stock MaxClutchTorque multiple 1.5 (2025) / 1.8 (2026).
- engine-re finding for Car_GearBox_GetClutchSlipping 0x1446a6090: Telemetry 'clutch'/'is_slipping' (registered 0x1446a57c0): returns 1.0 if GearBox+0x694 else 0.0; +0x694 copied from drivetrain+0x538, set at 0x1446e399b when required clutch torque > capacity (GearBox+0x698); no overrev damage while slipping (agent rest, tr-verify claim 7)
- engine-re finding for Car_GearBox_Update 0x1446a6b60: carlive 2026-10-29 (static, carlive-verify agree): 0x1446a6c46-0x1446a6c7c [gb+0x698] = (1.0 - [car+0x1c0]) * ([Eng+0x2518]^[Eng+0x2520]) * [gb+0xe8] (Eng=[car+0x530], 1.0 at 0x145381184); gb+0xe8 MaxClutchTorque PLAIN (vmulss, not XOR); +0x698 forced to 0 at 0x1446a6c9b when gear idx [+0x680]==1 or global flag / car+0x128==1. Called from 0x1446dce0c (Car_Update_WheelSurfaceAndEcuGrip, per frame H); powertrain reads +0x698 per substep. Writing +0x698 is overwritten next call: scale gb+0xe8. car+0x1c0 = clutch input (label H).
- engine-re finding for Car_Write_Steering 0x1446a6e00: VTF Steering: MaxSteeringLock, AckermannSteering, VisualTurnRate, damage wobble (speed and damage range, rates, angle). No unique byte signature: the body is identical to sibling VTF/DB section sections apart from wildcarded rip-relative string offsets. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Steering_Update 0x1446a78d0: angle += rate*dt (x0.4 below 1 m/s); Ackermann inner wheel theta*(Lock+Ackermann)/Lock -> wheel+0x178; no steering ratio (road angle = input * MaxSteeringLock); wobble from damage in 0x14473fd80 (agent car-chassis, static) carlive 2026-10-29 (static, carlive-verify agree): 0x1446a7979-0x1446a79a5 capA=min([St+0x128]^[+0x130],1)*Lock, capB=min([St+0x118]^[+0x120],1)*Lock (Lock=[+0x28]^[+0x30]); wheels [+0xd0]/[+0xd8]+0x178 clamped to [0,cap]/[-cap,0]; outer wheel x (Lock+[+0x38]^[+0x40])/Lock; angle integrated in [+0xe8]^[+0xf0]. +0x118/+0x128 = XOR per-side lock fractions (<=1, can only reduce lock); addon lever = Lock XOR +0x28 (key +0x30), not re-copied per step (H). Leaf fn without .pdata: no sigcheck-able func_start sig. CORRECTION lb23-verify: the wheel scaled by (Lock+x)/Lock is the inner wheel, not the outer
- engine-re finding for Car_Write_SuspensionSystem 0x1446a8870: Vehicle physics section for SuspensionSystem: brake types (handbrake, temperature, damage brake loss), tyre types (mass, dimensions, grip) and punctures: PunctureWearMin/Max, Puncture Spring/Damping/Radius/Grip/Resistance multipliers, DebrisPunctureMin/MaxSpeed, DebrisPunctureProbabilityAtMin/MaxSpeed, DebrisPunctureInvulnerabilityPeriod, DebrisNextGenPunctureProbabilityMultiplier, PunctureSpeedTyreDamageRateSpline. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Wheel_Construct 0x1446af9c0: Writes qword 0x3f80000000000000 at wheel+0x8c (1446af9ff/a09): tyre water 0.0, wetness-grip scale wheel+0x90 = 1.0; +0x94 blister 0; wheel+0 = wheel+0x10 = car (wx-verify claim 10)
- engine-re finding for Car_SuspensionSystem_ResetRestAndSpring 0x1446b1460: su-verify claim 8: (car+0x580): axle+0x1648 = src+0x15b8, axle+0x165c = src+0x1604 for both axles (src objects container+0x58/+0x60, H stock VTF data), then keys tarr/tarf in map car+0xd8 -> +0x138
- engine-re finding for Car_Tyre_SetCompound 0x1446b18e0: Body 0x155436e70: wheel+0xc0 = compound; +0xc8/+0xcc optimum carcass/tread temperature (plateau mid of TemperatureGrip splines); fit_new: T = PitReleaseTemperature compound+0x688 (> 0, else ambient) -> ctx+0x64/+0x6c/+0x68 = wheel+0x6c/+0x74/+0x70 = tyre blankets (70 C slicks, 60 C 2025 inters, ambient for wets) (agent car-tyre, static)
- engine-re finding for Car_Tyre_Step 0x1446b5720: Tyre water level wheel+0x8c moves toward surface wetness wheel+0x280: rise 4.0*dt (0x1453814ac), fall -0.5*dt (0x14547dab4, sign built in) (1446b59bf-a15), only if car+0x133 bit0; calls temperature update 0x1446b7b70 (wx-verify claim 8)
- engine-re finding for Car_Write_Turbo 0x1446b6630: VTF Turbo: BoostEfficiencySpline, zeroBarRate, maxBoostRate, MaxBoost, relativeSize, acceleration, Deceleration. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Tyre_UpdateDeflation 0x1446b7060: Body 0x155453df0: puncture lerps sidewall (TT+0x58), tyre spring wheel+0x58 (TT+0x50), damping/rolling resistance wheel+0x5c/+0x60 (both from TT+0x54 at 0x155453f96 / 0x155453fa8: copy-paste bug suspected, TT+0x60 PunctureResistanceMultiplier never read; car2-verify); burst sets near-zero stiffness (agent car-tyre, static)
- engine-re finding for Car_Tyre_GripScale 0x1446b7240: Per-wheel grip multiplier: max(floor +0x64c, f1*f2); f1 = 1 - k(+0x638/+0x63c by axle) * clamp(x/0.2) with x = this+0x84; f2 = 1 - k(+0x640/+0x644) * clamp((v0 +0x648 - wheel slide speed)/v0) using the wheel's +0x3a8/+0x3ac velocity and +0x188 rotation rate. Wheel from car list 0x146ce86e0 -> +0x580 SuspensionSystem -> +0x70+i*8. Meaning of x needs a trace. | = wet/speed DOWNSLOPE scale: f1 = 1 - WetnessDownslopeFactor(+0x638 player/+0x63c AI)*clamp(tyre water wheel+0x8c/0.2); f2 = 1 - SpeedDownslopeFactor(+0x640/+0x644)*clamp((RotRate+0x648-rot)/RotRate); return max(DownslopeScaleMinimum +0x64c, f1*f2). Used by Car_Suspension_SolveWheels 0x1446dee60 only if HandlingVersion car+0x9f40>=7, multiplied into MF column +0xc0 (inferred DownSlope) (wx 2026-10-28)
- engine-re finding for Car_Tyre_ComputeGripMultiplier 0x1446b7390: Product by mask: 0x1 wheel+0x2a0 surface grip, 0x2 WearGrip(+0x88), 0x4 TemperatureGripCarcas(+0x70)*TemperatureGrip(+0x6c), 0x8 residue, 0x10 BlisterGrip(+0x94), 0x20 WetnessGrip spline [wheel+0xc0]+0xec0 at wheel+0x8c*wheel+0x90 (1446b75a1-5bd), 0x40/0x80 puncture/burst, 0x100 damage, 0x200 wheel+0x1f0*TyreType+0x2c; clamp Min/MaxGripMultiplier (+0x24/+0x28) (wx-verify claim 9)
- engine-re finding for Car_Tyre_UpdateWear 0x1446b79f0: wheel+0x88 wear += WetnessWear spline (+0xc18)(tyre water wheel+0x8c) * temperature wear (TyreType +0x654/+0x658) * WearRate lerp (compound +0x120/+0x124) * UpgradeWearFactor +0x630; clamp 0..1; gate car+0x132 bit7 and car+0x128!=1 (wx-drive, static) | 2026-10-28 engine-re (agent car-tyre): dWear = (AI ? wheel+0x84 : 1)*wheel+0x80*TT+0x630*min(TT+0x658, r^2)*WetnessWear(water)*lerp(WearRateMin, WearRateMax, pressure)*slipWork/1e6, r = (TT+0x654 + T_patch)/(TT+0x654 + T_opt); wear per MJ of sliding work; no cliff in stock data (WearGrip ~0.80-0.88 at wear 1)
- engine-re finding for Car_Tyre_UpdateTemperature 0x1446b7b70: If car+0x133 bit2 clear (simplified): wheel+0x6c/+0x70 = WetnessTarget spline (+0x1168)(wheel+0x8c) = target tyre temperature vs water (medium); water cooling clamp((wet-0.15)*4)*0.002; blister wheel+0x94 += dt*BlisterRate(+0x128)*clamp((T-MinBlist+0x12c)/(MaxBlist+0x130-MinBlist)) if HandlingVersion>=13 (wx-drive, static) | 2026-10-28 engine-re (agent car, cg-verify claim 3): 2-node model (tread +0x6c, carcass +0x70, no inner); full model only with car+0x133 bit2 (rolling heat, slip work 0x1446b8210, convection, Penman evaporation, carcass conduction, brake radiative; clamps 393.15/423.15 K); blister block gated cmp [car+0x9f40],0xd / jl at 0x1446b8130 (HandlingVersion >= 13 -> OFF for 2025 = 8 and 2026 = 9)
- engine-re finding for Car_Write_F1Aero 0x1446c73f0: VTF F1Aero: aero maps m_aero_front_cl_*, front wing loss, reference plane/offsets, BodyDragCoeffMin/Max, front/rear wing CoL/CoD, lift multipliers. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Read_F1Aero 0x1446c8560: Re-reads F1Aero by name (BodyDragCoeffMin->m_aero_cd_min, ...) through Game_Read_VtfFloat; used by radUpgradeDrag/FrontDownforce/RearDownforce upgrade blocks. Correction: this is the reader (component data vtable slot 2): it fetches each value by name with Game_Read_VtfFloat and stores it XOR-obfuscated (value ^ key 8 bytes later, e.g. Engine Mass -> +0x2f8 ^ +0x300); it runs for the base section and for every Upgrade block.
- engine-re finding for Car_F1Aero_UpdateWingOpen 0x1446cb540: DRS / 2026 straight-mode wing: clears open request F1Aero+0x1988 bit0 when brake [car+0x190] > 0.05 (0x1446cb65f) or throttle [car+0x188] < 0.9 (0x1446cb6d0); open fraction +0x19a8 moves at DRSOpenCloseSpeed ([+0x1868]^[+0x1870]) clamped 0..1; second fraction +0x19b0 targets 0 when +0x19ac (2026 Partial mode); bit1 = failed (skips checks); car = [F1Aero+8]. F1Aero = car+0x570, runtime field = data offset + 0x18, XOR pairs (agent aero, ap-verify claim 1)
- engine-re finding for Car_Read_F1Assists 0x1446cb880: Log channel PAD_ASSISTS: 'Failed to read %s in VTF. Aborting steering parameter setup for this speed.' | 'Failed to load m_steerOutRateVSSpeedSpline spline.' | 'Failed to load m_steerInRateVSSpeedSpline spline.' | 'Failed to load m_lockProportionVSSpeedSpline spline.' | 'Failed to load m_reducedLockGivenBackVSRearSlipAngleSpline | 2026-10-28 engine-re (agent car): this is the F1Assists VTF data reader (Car_Read_F1Assists, data vtable 0x145bfcbe0, writer 0x1446cc750, component car+0x598)
- engine-re finding for Car_Input_SmsSteer 0x1446ce000: Pad SMS steering: grip-limited target angle from v^2/(grip*g), lock/rate fields block +0x22dc..+0x239c, rate * block+0x54 (user steering_rate_scale) (agent options, partly H)
- engine-re finding for Car_Tyre_MagicFormula 0x1446cf080: Pacejka Magic Formula, F = D*sin(C*atan(B*a - E*(B*a - atan(B*a)))) with a = slip angle in degrees (param_3 radians *180/pi). Load table at this+0x10..0x18 (32-byte rows: +0 Load, +4 Peak, +8 PeakMin, +0xc PeakMax, +0x10 DownSlope, +0x14 UpSlope, +0x18 Shape) lerped by wheel load (param_2): D = -(Peak) * (1 + GripGain), C = DownSlope, B = UpSlope, E = Shape; camber table at this+0x30..0x38 (8-byte rows Camber, GripGain). Optional param_4 returns the cornering stiffness (slope at 0.01 deg). Data: VCMPD compound XML TyreModel ModelType MNC, ParametricData LateralLoad/LongitudinalLoad/LateralCamber (C5 front Peak 2.372 at 0 N -> 1.199 at 15000 N). Uses atanf 0x1453530e4, sinf 0x14535cf70.
- engine-re finding for Car_Write_EnergyRecovery 0x1446cf6c0: Per-mode ERS block (MaxDeployPower, MaxDeratePower, BoostModePowerAddLimit, PowerReductionRateLimit, throttle timers, HarvestTorqueFactor, HarvestLimitMGUK). Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Read_EnergyRecovery 0x1446cfcd0: VTF EnergyRecoverySystem Common + modes + per-track TrackSettings: MaxDeployPower (2025 126.3 kW, 2026 350 kW), MaxDeratePower, DeployLimitMGUK (4 MJ / 13 MJ), EnergyStoreCapacity 4 MJ, harvest efficiencies, MaxBrakeTorqueMGUK, HarvestLimitMGUK/Qualifying, HotLap/MediumDeployFactor. Correction: this is the reader (component data vtable slot 2): it fetches each value by name with Game_Read_VtfFloat and stores it XOR-obfuscated (value ^ key 8 bytes later, e.g. Engine Mass -> +0x2f8 ^ +0x300); it runs for the base section and for every Upgrade block. | VTF EnergyRecoverySystem reader, PLAIN floats (data+0x18): +0x28 MaxDeployPower, +0x50 HarvestLimitMGUK, +0x5c DeployLimitMGUK, +0x60 EnergyStoreCapacity, +0x81 OverrideModeAvailable; modes +0x1880+m*0x40 (gear modifiers +0x10, DerateStartSpeed +0x3c); TrackSettings +0x1980; runtime ERS = car+0x5a8, mode +0x1a68, restore +0x1a6c, lock +0x1a70, 2026 override flags +0x1a60..63. Full map work/fuel/findings_ers.md (fuel-mix, static) CORRECTION lb23-verify: values are stored plain, not XOR-obfuscated
- engine-re finding for Car_Read_F1Tuning 0x1446d1660: Re-reads the F1Tuning ranges by name through Game_Read_VtfFloat (overlay). Correction: this is the reader (component data vtable slot 2): it fetches each value by name with Game_Read_VtfFloat and stores it XOR-obfuscated (value ^ key 8 bytes later, e.g. Engine Mass -> +0x2f8 ^ +0x300); it runs for the base section and for every Upgrade block. CORRECTION lb23-verify: values are stored plain, not XOR-obfuscated
- engine-re finding for Car_Write_F1Tuning 0x1446d4470: VTF F1Tuning: setup ranges (chassis COM Z, differential on/off throttle, ... Min/Max) used by the car setup screen. Correction: this is the serialiser (component data vtable slot 3): it writes the section back as XML through the writer object's vtable (+0x28 begin element, +0x68 attribute, +0x78 end, +0x98 comment); the field list above is what it writes. The reader is slot 2 (Game_Read_VtfFloat by name, values stored XOR-obfuscated).
- engine-re finding for Car_Solver_PrepareWheel 0x1446dbc50: Copies drive share [[solver+0x18]+0x578]+0xd0[i] -> solver+0x440[i] every step (0x1446dbce3/ced) -> torque split live-editable (agent power, ap-verify claim 7)
- engine-re finding for Car_Suspension_SolveWheels 0x1446dee60: Per-frame wheel/tyre solve of the SuspensionSystem: calls Car_Tyre_MagicFormula and Car_Tyre_GripScale per wheel (4x each), powf; writes the per-wheel result arrays (+0x730..+0x7f0, 4 floats each) that Car_Suspension_PublishWheelState copies to the wheels.
- engine-re finding for Car_Powertrain_Step 0x1446e2290: Per-car engine/powertrain step (reads Turbo +0x5a0, FuelECU +0x548, GearBox +0x538, ERS +0x5a8, Engine +0x530, FuelTank +0x540 of the car). Engine power = Car_Spline_EvalObfuscated(engine +0x28 RevsPowerSpline, rpm / revs scale) * power scale / rpm. Active fuel mode scales live XOR-obfuscated in the FuelECU runtime object: +0x130^+0x138 fuel burn scale (fuel flow written to +0x1a2c = turbo +0x2fc * throttle * rpm * +0x74 * burn scale), +0x140^+0x148 power scale, +0x150^+0x158 revs scale. FuelTank +0x100 < 0.08 with car flag +0x132 bit 2 clamps the throttle (fuel run-out). Turbo +0x304 boosts power when engine mode +0x2208 == 2. carlive 2026-10-29 (static, carlive-verify partial): CORRECTION - turbo MaxBoost [Turbo+0x2d8] (Turbo=[[eng+8]+0x5a0], PLAIN float) is read only in TurboMode 2 (cmp [eng+0x2208],2 at 0x1446e3a27 -> vmovss at 0x1446e3a4e, turbo != null); mode 1 branch at 0x1446e26b3. Stock BoostEfficiencySpline flat 1.0, so MaxBoost changes no torque on stock cars (H). Diff multiples read plain: 0x1446e3e3d [diff+rcx*4+0xec] (off/on throttle by torque sign), 0x1446e3f39 +0xf0, 0x1446e3f5b +0xf4 (mid-corner), 0x1446e401b (diff=[car+0x578]). Once per substep from Car_Physics_StepVehicle (unproven statically).
- engine-re finding for Car_Ffb_SlipAngleInput 0x1446fd980: named from its log messages (channel FFB_PLUGIN): 'slip angle input %f'
- engine-re finding for Car_Ffb_SlipRatioInput 0x1446fe8a0: Force feedback plugin input: per-wheel slip ratio (channel FFB_PLUGIN).
- engine-re finding for Car_Ffb_TyreForceReal_Update 0x1447006f0: FFB plugin tyre_force_real (vtable 0x145c011c0) = main steering torque, rebuilt on FFB thread from front wheels body veh+0x900 (Fy +0x960, Fz +0x980 clamp 15000, slip +0x990/+0x9a0) with trail 0.23/0.28 and jacking term; lane0 = sum torque * wet * carFFBScale (veh+0x558 -> +0x88^+0x90) * plugin+0x50; lane1 friction (frictionPeak 3.0 -> frictionUndersteer 1.0). WET: wet = forceWetMultiple 0x145faa638 (1.25) when [[wheel+0xc0]+0xf8] (FL compound wet/inter) else 1.0 (144700c0b/c12, mult 144700d17); friction 0x145faa63c (wx-verify claim 12)
- engine-re finding for Car_TyreDeformation_CheckParticles 0x14470bd80: Log channel DEFORMATION: 'Particles have NOT YET been allocated for vehicle %d tyre %d.' | 'Should be 50 physics particles because that is what the data uses for all tyres.' lb23-verify partial
- engine-re finding for Car_TyreDamage_Update 0x14470d760: Tyre damage update: adds/removes punctures (Car_Puncture_Add/Remove) and switches the tyre deformation model (Car_TyreDeformation_SetTyre); reads TyreDmg, BodyDmg, WingDmg, F1Aero components.
- engine-re finding for Car_Vte_WriteTelemetryRow 0x144739180: VTESystem (vtable 0x145c03550 slot 4) row writer for telemetry_data.csv: Time, CarId, DriverId, TeamId, laps, position, lap time, pace, position XYZ, tyre compound/age/deg, pit/DRS/ERS flags, track/weather state, temps, speed, steering, throttle, g-forces, engine power, MGU-K power, brake balance, ride heights, per-wheel slip angle/slip ratio/lateral force/longitudinal force (RL,RR,FL,FR), front wing, gap, wake % CLF/CLR/CD. Wheels: car (+0x20 of VTE) -> SuspensionSystem +0x580 -> wheels +0x70 RL, +0x78 RR, +0x80 FL, +0x88 FR; per wheel +0x3b0 slip angle, +0x3b4 slip ratio, +0x3c0 lateral force, +0x3c4 longitudinal force (order from the CSV header).
- engine-re finding for Car_Vte_OpenSessionFiles 0x14473a1c0: VTESystem session start: creates the output folder, opens telemetry_data.csv and timing_data.csv ('Timestamp,Car no.,Timing loop,Position,Lap'); gate byte +0x120 (folder ready), path +0x122.
- engine-re finding for Car_Vte_CreateOutputFolder 0x14473a400: Builds '<save path>teoutput\<yymmdd_hhmmss>' (UTF-16), creates it if missing (log channel VTE-OUTPUT), sets +0x120 = 1.
- engine-re finding for Car_Build_VtfComponents 0x14473c2e0: Creates the vehicle physics components by VTF section name (Engine, GearBox, FuelTank, FuelECU, BrakeECU, Chassis, Steering, CoreAero, F1Aero, Differential, SuspensionSystem, DebugSystem, F1Tuning, F1Assists, Turbo, EnergyRecoverySystem, ConvexHull, WheelProxies, F1Plank, F1WingDamage, F1BodyDamage, F1SuspensionDamage, F1TyreDamage). | 2026-10-28 engine-re (agent car, cg-verify claim 1): stores 23 component pointers car+0x530 Engine .. +0x5e0 F1BodyDamage, 8 B stride: 0x530 Engine, 0x538 GearBox, 0x540 FuelTank, 0x548 FuelECU, 0x550 BrakeECU, 0x558 Chassis, 0x560 Steering, 0x568 CoreAero, 0x570 F1Aero, 0x578 Differential, 0x580 SuspensionSystem, 0x588 DebugSystem, 0x590 F1Tuning, 0x598 F1Assists, 0x5a0 Turbo, 0x5a8 EnergyRecoverySystem, 0x5b0 ConvexHull, 0x5b8 WheelProxies, 0x5c0 F1Plank, 0x5c8 F1WingDamage, 0x5d0 F1SuspensionDamage, 0x5d8 F1TyreDamage, 0x5e0 F1BodyDamage. Full map work/car/findings_car.md
- engine-re finding for Car_Collision_Dispatch 0x14473f8e0: Collision report dispatch (agent collide): impulse report+0x50, world impulse > 5 event 0x1456353f0, car-car event 0x1456353e0, 15 listeners car+0x9db8; hook for g-based impact effects (damage/FFB/events only)
- engine-re finding for Car_Upgrades_ApplyToVehicle 0x144742c70: named from its log messages (channel VEHICLE_UPGRADES): 'Applying %s upgrade to vehicle %d. (perc0to1: %f) | - Applying upgrade to %s.(perc0to1: %f)'
- engine-re finding for Car_Parse_TyreModel 0x144743900: parses the VTF tyre model: Wetness, WetnessGrip, WetnessWear, WetnessTarget, WearGrip, TemperatureGrip, Blister*, Pressure*, RollingResistance*, Camber, … (physics wetness is one value for the whole track).
- engine-re finding for Car_Load_Vtf 0x14475a250: Loads a VTF handling file (binary XML 'BXML', teams/<team>/handling/<team>.vtf in the vehicle package ERPs); reads HandlingVersion ('pro', 'handling20') and dispatches the sections Chassis, Engine, GearBox, FuelTank, FuelECU, BrakeECU, Steering, CoreAero, Differential, SuspensionSystem, Turbo, WheelProxies, ConvexHull, F1Aero, F1Assists, F1Tuning, F1Plank, ... | HandlingVersion (2026-10-28 wx-verify claim 11): key 'HandlingVersion' (145c04a50) at 14475a4e3: strstr 'pro' -> 100; else strstr 'handling20' -> atoi, clamp 2018..4048, -2017 (14475a54a-565); else 1; stored sysobj+0x18 (14475a577), copied to car+0x9f40 at 1446296c8/cc. Gates: wet downslope >=7 (2024), blister >=13
- engine-re finding for Car_Load_SetupPresets_Body 0x149a21730: .data1 body. Calls Car_Load_SetupPreset for variants 0..4 into this+0x68 (stride 0x550) with the package label at [[[this+0x28]+0xe0]+0xa18], then looks the preset row up again.
- engine-re finding for Car_SuspensionSystem_OnTuningChanged_Body 0x15543f450: su-verify claim 8: flag bit 0x2000 -> 0x1446b1fb0 per wheel, then Car_Tuning_SetSuspension 0x1446a7be0 per axle; overwrites live axle writes
- engine-re finding for site_Car_BodyDamage_ImpulseScaleRead 0x14465162e: carlive 2026-10-29 (static, carlive-verify): vmulss xmm1,[rip+0x145bf68d0] (k_Car_BodyDamage_ImpulseScale) in Car_BodyDamage_OnCollision; mid sig because the prologue is not unique.
- engine-re finding for k_Car_BodyDamage_ImpulseScale 0x145bf68d0: carlive 2026-10-29 (static, carlive-verify agree): 0.0003f in .rdata; sole ref vmulss 0x14465162e in Car_BodyDamage_OnCollision: D = m(1 or 5) * impulse * k * material+0x5c -> global body damage gain for all cars.
- engine-re finding for g_Car_FfbForceWetMultiple 0x145faa638: float, default 1.25 (writable section); tyre_force_real torque multiplier on wet/inter tyres; set from plugin param 'forceWetMultiple' (144701e82..f7f). Addon: < 1.0 = lighter wheel in the wet
- engine-re finding for g_Car_FfbFrictionWetMultiple 0x145faa63c: float, default 1.25; tyre_force_real friction (lane1) multiplier on wet/inter tyres; param 'frictionWetMultiple' (14470209d)
- 2026-10-31 17:10 car-live: Verify requests for the 1.6.13 live car levers (each stays off until an independent agree row exists; flip the entry in native/src/core/hooks/car_live.cpp ROWS): CarF1Aero.lift_scale/drag_scale (+0x19a4/+0x19a0), CarBrakeEcu.brake_bias/brake_pressure (+0x21d8/+0x21dc), CarSuspTyreType.grip_multiplier (+0x2c, found by name Front/Rear), CarSuspAxle.setup_camber/setup_toe/setup_anti_roll/ride_height_offset (axle by LocalRestPositionZ), Car_Tuning_SetChassisZCom (vtable +0x18 flag 0x200) with sqrt_mass +0x98, CarChassis.tyre_physics_group (+0x118) with Car_Tyre_SetWearFactor and the car to tyre component path, Car_FrontWing_Init, Car_FuelEcu_SetMode_Thunk (set_fuel_mode). Already agree and used: site_Car_EngineDriveTorque, Car_Upgrades_ApplyToVehicle (args loader, name, flag, xmm3 perc), site_Car_SetupTuningApplied.
- engine-re finding for Sess_Get_VehicleTrackFactor 0x1436ae470: Reads the "SDT+" (0x2b544453) tagged record from the track/session data blob of the vehicle (table [..+0x28]+8, stride +0x18), indexes it by compound/wear index (+0x128) -> float, multiplies by a tweakable looked up by tyre-name hash in the weather/track tyre list ([session]+0x270..0x278 stride 0x18, default at +600; xor-obfuscated scale). Returns 1.0 when data missing. Caller Car_TyreWear_SimulateStint. Addon: per-track tyre wear multiplier lookup.
- 2026-11-04 21:55 live-car-re: live-car-re 2026-11-04 (static, UDP telemetry fill Game_TelemetryUdp_UpdateRace 0x142cf9820, decomp:re/decomp/142cf9820.c): CarLive+0x188 = throttle pedal, +0x18c = clutch (x100 in UDP), +0x190 = brake pedal (UDP swaps them while GearBox+0x680 == 0 and (CarLive+0x224 & 0x88) == 8); Steering (+0x560) current angle = CarObfFloat at +0xe8/+0xf0 (sign flipped), max lock +0x28/+0x30, UDP steer = -angle/lock; wheel (CarWheel) +0x200 = pointer to the MECHANICS surface record under the wheel (zeroed with +0x208 at the start of every Car_Contact_QueryWheels pass 0x1446d928d, stored on a ray hit 0x1446daaa3, null = airborne), record +0x78 (dword) = ffb_surface = the UDP m_surfaceType id (0 tarmac, 1 rumble strip, 2 concrete, 3 rock, 4 gravel, 5 mud, 6 sand, 7 grass, 8 water, 9 cobblestone, 10 metal, 11 ridged, 12 default/unnamed); wheel +0x168 suspension position m, +0x188 spin rate, +0x3c tyre radius, +0x3b0 slip angle, +0x3b4 slip ratio, +0x3c0/+0x3c4 lateral/longitudinal force; physics body = [[CarLive+0x558]+0x340]+0x18 with +0x1c0 position, +0x220 world velocity (m/s), +0x210 angular velocity; Chassis +0x320/+0x324/+0x328 = g-force-like signals x1000 clamped to int16 in the UDP motion packet. Player vehicle id = Sess_Get_PlayerVehicleIndex(ctx, 0) with only [ctx+0x30] used (= [g_Game_Manager+0xc0]).
- 2026-11-11 12:41 osd-vc: osd-vc: V8735-V8816 agree/partial rows for CarLive.vehicle_id/system pointers, CarEngine.rotation_rate, all CarErs fields, CarF1Aero.drs_flags (V8816: recommend L1->L2, bit0 open request, bit1 fault), partial for CarLive.is_ai (V8748: no car ever seen with 0, writer unknown). Name checks: 0x1446e4330 is labelled Car_Suspension_PublishWheelState but contains the ERS energy/harvest/deploy integration (stores 0x19c8..0x1a14); 0x1446d1230 (FUN_) is the ERS step helper; 0x14470d760 (Car_TyreDamage_Update) also writes CarErs.ers_mode (+0x1a68, &7) at 0x14470e0a9 and F1Aero+0x1988 bits at lines 724/819 so it looks like a per-car state apply. CarErs +0x1a60 flags: bytes +0x1a61/+0x1a62 are read by the UDP status builder (0x142d00060 lines 713-714, UDP record +0xc72e/+0xc72f), so the L1 Halo flags row has a game reader.
- 2026-11-11 13:14 osd-vd: osd-vd: vt_Car_SystemsContainer (0x145c040c8) has an agree row (V8895-ish, see verify.tsv) but no signature, so bindgen cannot export it. Candidate data/riprel signature, unique on d873a1c8: pattern 'C5 F8 11 B7 E9 02 00 00 C5 F8 29 B7 E0 02 00 00 48 8D 05', match 0x144628763, resolve riprel insn 0x10 len 7 -> 0x145c040c8 (sitereg.py data vt_Car_SystemsContainer 0x145c040c8 --via 0x144628773 --type 'const void *'). Owner please register it (sig=car.systems_container).
- 2026-11-11 13:31 car-re: 2026-11-11 car-re: promoted CarF1Aero.drs_flags L1 -> L2 (re-decompiled Car_F1Aero_UpdateWingOpen, Car_State_Read_Snapshot, UDP builder reads; V8816); registered sig car.systems_container for vt_Car_SystemsContainer; renamed 0x1446e4330 -> Car_Ers_IntegrateAndPublishWheels and 0x14470d760 -> Car_State_Read_Snapshot (no binding used the old names); CarLive.is_ai notes corrected (selects the AI rate only, NOT AI-controlled-right-now, never use it to pick the player; V8748).
- 2026-11-14 02:27 engine-re: engine-re 2026-11-14 weather to L2 (static, for car-re): tyre water wheel+0x8c follows the surface wetness wheel+0x280 (env slot 5 out[0]) at +4.0*dt / -0.5*dt in Car_Tyre_Step (0x1446b59bf..0x1446b5a15); Car_Tyre_ComputeGripMultiplier mask 0x20 uses WetnessGrip(wheel+0x8c * wheel+0x90) with this = wheel+8; new row Car_Env_IsTrackWet 0x142f47880 (slot 3); g_Car_FfbForce/FrictionWetMultiple apply only when [[tyre+0xc0]]+0xf8 (wet/inter compound) is set (0x144700c27/0x144700c45). Rows stay L1 for the owner; data flow kb:subsystems/session.md#wetness.
- 2026-11-14 05:28 engine-re: 2026-11-14 engine-re round 2 (--force, owner review requested): Car_PitStop_ApplyTyreAndWingVisuals 0x1403b77c0 raised to L2 (decomp + callers 0x1403b7574, 0x143251e85).
- 2026-11-17 04:46 rr-bindings: Binding request (rr-bindings, 2026-11-17; write-up doc:re/research/weather_strategy_bindings.md#functions): please review for L2 and export. (1) Car_Tyre_ChangeToNextCompound 0x142e94040 (L1, sig car.tyre_change_to_next_compound): rcx = tyre component, edx = bool; fits the queued set component+0x9c70/+0x9c74 through Car_TyreSelection_SetCurrentTyres(component+0x9c60, component+0x9c70) at 0x142e9425a; callers 0x1403c8dd3 (Game_PitBoxCarActor_Update, edx=0), 0x1434bdd9f. Suggested proto: void Car_Tyre_ChangeToNextCompound(void *tyre_component, bool activate). (2) Car_TyreSelection_SetCurrentTyres 0x142e9eda0 (L1, sig car.tyre_selection_set_current_tyres): compares the request (group [rdx], set byte [rdx+4]) with the fitted set (last 0x78-byte entry of [sel+0x30,sel+0x38), group end-0x6c, set end-0x68; 0x142e9ee13..0x142e9ee5e) and does nothing when equal; only caller 0x142e9425a. (3) site_Car_TyreAiMinGripClamp 0x1446b7979 (L1, sig car.tyre_ai_min_grip_clamp): the AI 0.6 grip floor in Car_Tyre_ComputeFfbGripFactor. Realistic Rain and Smarter AI Strategies use all three; on Steam 1.0.145 they moved +0x2590/+0x2870 and the addons fell back to their own byte scans.
- 2026-11-17 14:45 engine-re: engine-re 2026-11-17 track round C (surfaces, static): tile material code -> physics material library entry (unknown code -> entry 0, Ne_MoverCollision_BuildMoverShapes notes) -> MECHANICS block via extension index (Game_RacingVehicles_Initialise / Car_Contact_QueryWheels notes; missing block -> grip 1.0). Kerb codes RMBL/RMBO/BUMP/RMBC take the body normal in Car_Contact_QueryWheels; FFB lanes by code in Game_FfbEffect_MixAndSend (rumble only RMBL/RMBO); new L1 Car_Chassis_UpdateWheelTarmacState 0x1446788e0. Owner review requested for the --force note on Car_Contact_QueryWheels.
