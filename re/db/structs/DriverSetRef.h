// struct: DriverSetRef
// subsystem: live
// size: 0x10
// notes: One driver table reference of EntrantsSystem.driver_sets: {allocator, slots}, the same shape as TeamSetRef.
struct DriverSetRef {
    /* 0x008 */ DriverSlots *slots;                          // L2 decomp:re/decomp/14246ddd0.c;decomp:re/decomp/142f28c10.c;xref:0x14c970d15,0x14c970d1d loaded from system+0x268+domain*0x10 in the Ent_Get_Driver body; null for an unbuilt table (checked)
};
