# Live car operations: the 1.0.145 builds

## Scope and status

`native/src/core/hooks/car_live.{hpp,cpp}` (cars::live, the car.* points, the car levers and AI racecraft mixins)
ran only on the Steam KB build (PE `1789118616`). This note records the static evidence that the same offsets,
calls, hooks and sites hold on the two 1.0.145.2472 builds, and the one address that moves:

| build | PE timestamp | SizeOfImage | image checked | g_Game_EngineModeAvail |
|---|---|---|---|---|
| Steam KB 25251167 | `1789118616` | `0x193e6000` | `F1_25.exe` | `0x14606ec28` (L1, re/db/globals.tsv) |
| Steam 26566308 | `1794312636` | `0x18434000` | `F1_25.exe`, SHA256 `46009bd3…d9389` | `0x146070c28` |
| EA 1.0.145 | `1794315435` | `0x1615e000` | decrypted dump `F1_25.1794315435.105156.mem` (SHA256 `34d00776…bd441`) of the packed `F1_25.exe` (SHA256 `8361a266…a99f7`) | `0x1461a7968` |

`car_live::CAR_BUILDS` lists these builds; the operations stay off on any other image. This is static evidence:
no game session ran on either 1.0.145 build with the gate open. The EA dump was taken with the core loaded, so its
code carries the core's hook jumps; the tool leaves the instructions under a jump out of the image out of the
comparison and says so.

## The check

```
re/tools/py re/tools/carlivecmp.py --kb <KB F1_25.exe> --other <F1_25.exe or dump> --build steam-26566308|ea-1-0-145
```

Both builds: 69 of 73 checks pass, none fail, and 4 protected bodies differ only in the instructions listed under
[Reviewed differences](#reviewed-differences). The checks:

- **Field evidence.** 26 KB functions that read or write the fields car_live uses, plus the 19 AiSystem,
  AiCarRecord and AiDriverConfig evidence functions, are found by layoutcmp.py's head pattern and are
  instruction-identical (relocated operands normalised; an identical function has the same displacements). They
  include:
  - Car_Chassis_Build, which gives vehicle_id +0xe8 and the upgrade loader +0x4a0;
  - Car_Build_VtfComponents, which gives the system slots +0x530..+0x5d0, FuelECU among them at +0x548;
  - is_ai +0x124 and the component owner +0x8;
  - the gearbox, wing, suspension damage, engine limiter, aero, brake and suspension fields;
  - the pit lane status table.
- **VTF readers.** The reader is reached from each system's identical constructor: data vtable at +0x18, slot 2, then
  through the protector's jmp thunk. GetName has to return the system's name. Every key name is paired with the
  offset it fills, and all pairs have to be equal on both builds. An offset may be passed as an opaque constant,
  which the tool evaluates. The readers checked:
  - FuelECU: 16 keys, the mode tables +0x28..+0x118, each a CarObfFloat with its key 8 bytes on;
  - Chassis: 12 keys, Mass +0x28 obf;
  - Steering: 10 keys, MaxSteeringLock +0x28 obf;
  - FuelTank: 14 keys, FuelCapacity +0x98 obf;
  - Turbo: 6 keys, MaxBoost +0x2d8;
  - Differential: 9 keys.
- **Bodies.** The protected bodies car_live calls or hooks are compared instruction by instruction:
  - Car_FuelEcu_SetMode_Thunk, with the fuel mix at +0x128;
  - Car_Engine_ComputeDerived_Thunk;
  - Car_Systems_Reset_Thunk;
  - Car_Register_DetachablePart_Thunk;
  - the FuelECU and Differential Reset at system vtable +0x18: the mix at +0x128, and diff_off_throttle +0xec and
    diff_on_throttle +0xf0, which have to sit in identical instructions;
  - the plain functions: Car_Tuning_SetFinalDrive, Car_Tuning_SetChassisZCom (chassis vtable +0x18),
    Car_FrontWing_Init and OnCollision, Car_Upgrades_ApplyToVehicle, Game_Pitlane_SetVehicleStatus,
    Car_Tyre_SetWearFactor and Game_Get_ModelNodeId.
- **Sites.** The function holding each mid-hook site is aligned. The site has to land on the binding's address,
  with the instructions around it identical:
  - site_Car_LiveCarBuilt, where rdi is the car;
  - site_Car_SetupTuningApplied;
  - site_Car_EngineDriveTorque: rbx is the engine and `[rbx+0x2528]` is the limiter;
  - site_Ai_TableGripInput;
  - site_Ai_BuildSystem_RecordPushed.
- **Bindings.** Every binding car_live resolves resolves through bindverify.py: by hint on Steam and by scan on the EA
  dump. Where code reaches a binding, it is the address that code calls.
- **g_Game_EngineModeAvail.** Its KB writer 0x1407e5420 is aligned on each build:
  - The store at 0x1407e5ec4 gives the build's global (table above).
  - The 0x368-byte allocation and the 26 × 0x20 memset at +0x28 are identical.
  - The fuel mix test in Sess_ControllerSystem_ApplyRecord (0x14389a2b0) is aligned and identical: it loads that
    global, reads `ecu+0x128`, tests `[obj + 0x28 + car*0x20 + mix*4]` and calls the build's
    Car_FuelEcu_SetMode_Thunk.
  - The global is not a binding. car_live reads it at `CarBuild::mode_avail_rva`.

## Key addresses

| | KB | Steam 26566308 | EA 1.0.145 |
|---|---|---|---|
| g_Sess_VehicleTable / Count | `0x146ce86e0` / `0x146ce86dc` | `0x146cea6e0` / `0x146cea6dc` | `0x146e210e0` / `0x146e210dc` |
| site_Car_LiveCarBuilt | `0x144629969` | `0x14462c1d9` | `0x144622469` |
| Car_FuelEcu_SetMode_Thunk (body) | `0x14468e6a0` (`0x1553ff3d0`) | `0x144690f10` (`0x154b19d40`) | `0x1446871a0` (`0x152f011c0`) |
| FuelECU data vtable / reader | `0x145bfa6f0` / `0x1553fb310` | `0x145bfc8b0` / `0x154b15310` | `0x145d0d1d0` / `0x152ef59c0` |
| FuelECU Reset | `0x1553fdf60` | `0x154b18040` | `0x152ef6580` |
| g_Game_EngineModeAvail writer store | `0x1407e5ec4` | `0x1407e5f04` | `0x1407e4e04` |
| ApplyRecord fuel mix test | `0x14389b83c` | aligned in `0x14389c840` | aligned in `0x143889a10` |

## Reviewed differences

The four bodies that are not fully identical differ only in the protector's opaque constants and junk. In an opaque
constant, `mov r, imm` combined with a dword of the image's data replaces a literal; the tool evaluates it from the
image. Junk is an add, sub or and on a protector global.

- FuelECU Reset (Steam): `test ebx, 0x2000` becomes an opaque constant that evaluates to `0x2000`.
- Differential Reset:
  - Steam: the opaque constants evaluate to `0x2000` and `0x59` (the KB literals);
  - both builds: a junk write on a rip global is changed.
- Car_Systems_Reset_Thunk: the opaque constants evaluate to `0` and `0x20bd` (Steam), and `0x20bd` against the KB's
  `0x20bd` (EA).
- Car_Register_DetachablePart_Thunk:
  - the stack argument offsets `0x40` and `0x2c` move between a literal and an opaque constant (same values);
  - `mov ebp, 1` becomes an opaque constant `1`;
  - one junk write is changed.
- Car_Tyre_SetWearFactor (EA dump only): one instruction sits under the core's hook jump.

## Struct changes seen on 1.0.145

On the Steam 26566308 build, the only displacement changes in the car functions checked are in CarErs:

- the ERS mode moved from +0x1a68 to +0x1a6c;
- its neighbours from +0x1a62 onwards shifted by 4.

These are in the engine torque function and ApplyRecord. car_live reads no CarErs field.
Live State already carries the ERS move (`STEAM_26566308_BUILD`, `ers_mode` 0x1a6c).

## Inferred, not observed

- The opaque constants read the image's data as it is on disk (Steam) or in the dump (EA). The game could
  rewrite those globals at run time; nothing seen so far suggests it does.
- Field meanings are the KB's (re/db/structs). Identical code shows that the same offsets are read and written in
  the same way. It does not show what the values do in a session.
- Old EA (`1789128892`) is not in `CAR_BUILDS`. On its dump the Turbo constructor does not align, so the tool does
  not pass that build.

## Runtime check owed

Load an add-on that uses cars::live, such as Rich Fuel Mix, in a session on each 1.0.145 build. Confirm the
following in the core log:

- `[cars] live car operations on (Steam 26566308)` (or `(EA 1.0.145)`);
- car.built firing for every car;
- the fuel mix tables written: the add-on's own log, and a cars::live read back.

In-game checks:

- MFD fuel Rich is selectable where the session hides it;
- the car levers armed line.
