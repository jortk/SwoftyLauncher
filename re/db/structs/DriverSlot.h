// struct: DriverSlot
// subsystem: live
// size: 0x10
// notes: One entry of DriverSlots.
struct DriverSlot {
    /* 0x008 */ DriverRecord *record;                        // L2 decomp:re/decomp/142f28c10.c;decomp:re/decomp/14246ddd0.c;xref:0x14c970d42 element index = driver stat id; the Ent_Get_Driver body returns [begin + id*16 + 8]
};
