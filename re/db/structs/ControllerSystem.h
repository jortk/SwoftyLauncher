// struct: ControllerSystem
// subsystem: live
// size: ?
// notes: Registry key 0x146106b08 (rule manager +0x1c28), global g_Live_ControllerSystem 0x146096578; vtable vt_Sess_ControllerSystem. Per-vehicle ControllerSets; the human car is the one whose active controller (set.mode == 0) is a HumanController.
struct ControllerSystem {
    /* 0x030 */ ControllerSet ** sets;                       // L2 decomp:re/decomp/14389a120.c;xref:0x14389a1c4,0x14389a239 pointer per vehicle id (and per spectator-list entry)
    /* 0x090 */ void * vehicle_manager;                      // L2 decomp:re/decomp/14389a120.c;xref:0x14389a16b,0x14389a1e7,0x14389a21c its +0x28/+0x30 vector length >> 3 is the set count
    /* 0x364 */ uint8_t active;                              // L2 decomp:re/decomp/14389a120.c;xref:0x14389a133
    /* 0x365 */ uint8_t sets_built;                          // L2 decomp:re/decomp/14389a120.c;xref:0x14389a143 Halo requires it before reading the sets
};
