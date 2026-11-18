// struct: SessionSystem
// subsystem: live
// size: ?
// notes: Registry key 0x14607f1a0, type name "SessionSystem" (static init 0x142df23e4). The per-vehicle race results: Halo's results system R. Linked into the rule manager at +0x1c68 (Sess_RuleManager_Initialise).
struct SessionSystem {
    /* 0x148 */ SessionVehicleEntry ** entries_begin;        // L2 decomp:re/decomp/143573b40.c;xref:0x143573be6;decomp:re/decomp/142cfd6c0.c;kb:subsystems/live.md#session-system pointer per vehicle id; may hold null
    /* 0x150 */ SessionVehicleEntry ** entries_end;          // L2 decomp:re/decomp/143573b40.c;xref:0x143573bed
};
