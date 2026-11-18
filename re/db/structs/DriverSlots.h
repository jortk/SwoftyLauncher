// struct: DriverSlots
// subsystem: live
// size: ?
// notes: Vector of 0x10-byte {allocator, DriverRecord *} entries indexed by the driver stat id (the u16 at +2 of a packed driver ref).
struct DriverSlots {
    /* 0x008 */ DriverSlot *begin;                           // L2 decomp:re/decomp/142f28c10.c;decomp:re/decomp/14246ddd0.c;xref:0x14c970d26
    /* 0x010 */ DriverSlot *end;                             // L2 decomp:re/decomp/142f28c10.c;decomp:re/decomp/14246ddd0.c;xref:0x14c970d2a the Ent_Get_Driver body checks id < (end - begin) >> 4 (0x14c970d2e..0x14c970d38) before reading
};
