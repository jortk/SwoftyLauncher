// struct: PhysicalCompound
// subsystem: live
// size: 0x28
// notes: One tyres.physical_compounds row as loaded by Sess_Load_PhysicalTyreCompounds (0x142e97f20): a vector of 0x28-byte records indexed by the row's stat id. TyreSlot.physical points at one (slot build 0x14f463120 body, thunk 0x142e9e540: physical table base [[0x1460806d8]+0xb0] + index * 0x28).
struct PhysicalCompound {
    /* 0x000 */ uint8_t stat_id;                             // L2 decomp:re/decomp/142e97f20.c;decomp:re/decomp/142e9e540.c;xref:0x142e97f20 the row's stat_id byte (initialised to 0xff, then set from the row)
};
