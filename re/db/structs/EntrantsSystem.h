// struct: EntrantsSystem
// subsystem: live
// size: ?
// notes: Registry key 0x14606ecd8 (vtable 0x14578af00, slot 0 0x1424cb490 returns the key). Holds the driver tables (system+0x260+domain*0x10 = {allocator, DriverSlots *}) and the team tables: system+0x2c0+set*0x10 = {allocator, TeamSlots *} (struct TeamRecord's header describes set 0 = the database teams, set 1 = My Team additions). The HUD view-model init 0x141759a40 stores registry(key) at its +0x28 and 0x141760020 indexes +0x2c8 by the entrant's team table byte (0..2).
struct EntrantsSystem {
    /* 0x260 */ DriverSetRef driver_sets[3];                 // L2 decomp:re/decomp/14246ddd0.c;decomp:re/decomp/142f28c10.c;xref:0x14c970d15,0x14246ddfb driver table pointer at +0x268+domain*0x10: Ent_Get_Driver body 0x14c970d00 (behind Ent_Get_Driver_Thunk) checks the domain byte <= 2 then loads [rcx+domain*0x10+0x268] and null-checks it; Ent_EntrantsSystem_RemoveDriver loads the same slot (domain 0 refused). Same object as team_sets: Sess_Get_CarDriverName passes VehicleHelperSystem+0x198 to the thunk (0x142f28d22), and the VehicleHelperSystem init 0x142f26900 stores there the registry entry for k_Live_EntrantsSystemKey (lea 0x142f26953, FindIndex 0x142f26967, store 0x142f26982)
    /* 0x2c0 */ TeamSetRef team_sets[3];                     // L2 decomp:re/decomp/141760020.c;decomp:re/decomp/141759a40.c;xref:0x1417600c6,0x141759ab8 table pointer at +0x2c8+set*0x10, the team table byte is checked <= 2 first
};
