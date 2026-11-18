// struct: TeamSetRef
// subsystem: live
// size: 0x10
// notes: One team table reference of EntrantsSystem.team_sets: {allocator, slots}.
struct TeamSetRef {
    /* 0x008 */ TeamSlots *slots;                            // L2 decomp:re/decomp/141760020.c;xref:0x1417600c6 loaded from system+0x2c8+set*0x10; null for an unbuilt table (checked)
};
