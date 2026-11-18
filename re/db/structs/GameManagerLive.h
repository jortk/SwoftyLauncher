// struct: GameManagerLive
// subsystem: live
// size: ?
// notes: Fields of the game manager singleton (*g_Game_Manager 0x14604c2c8, structs/GameManager.h in subsystem game) that GameManager.h lacks. The component registry (every Halo system key) is the map at +0x30; systems are inserted by Live_Add_System (0x144500dc0) under the key their vtable slot 0 returns.
struct GameManagerLive {
    /* 0x030 */ NeRobinHoodMapU64 * systems_map;             // L2 decomp:re/decomp/142c74bf0.c;decomp:re/decomp/142882980.c;xref:0x142c74c0a,0x142c74cd4,0x142c74d09;kb:subsystems/live.md#registry registry: key = address of a static type record, value = system pointer
    /* 0x2d0 */ GameVehicleProgressSystem * progress_system; // L2 decomp:re/decomp/1403d84b0.c;xref:0x1403d85ef,0x1403d866b,0x1403d8691 passed as the first argument of Game_VehicleProgressSystem_GetLapProgress
};
