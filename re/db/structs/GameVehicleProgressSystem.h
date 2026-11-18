// struct: GameVehicleProgressSystem
// subsystem: live
// size: ?
// notes: game::GameVehicleProgressSystem (registry key 0x146096928, [g_Game_Manager+0x2d0]). Per-vehicle entries by vehicle id.
struct GameVehicleProgressSystem {
    /* 0x038 */ void ** entries_begin;                       // L2 decomp:re/decomp/1446046a0.c;xref:0x1446046a9 pointer per vehicle id
    /* 0x040 */ void ** entries_end;                         // L2 decomp:re/decomp/1446046a0.c;xref:0x1446046ad
};
