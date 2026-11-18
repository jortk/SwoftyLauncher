// struct: PitLaneTrack
// subsystem: live
// size: ?
// notes: The lane object at PitLaneSystem+0x48 (stand-in name; the class is not named in the exe). Holds one 16-byte slot per vehicle id at +0x938; the game guards the id with < 0x1b everywhere it indexes the block (0x142cfe2a8 in the UDP builder, 0x1434bc54d in the status setter).
struct PitLaneTrack {
    /* 0x938 */ PitLaneVehicleSlot vehicle_slots[27];        // L2 decomp:re/decomp/142cfd6c0.c;decomp:re/decomp/1434bc540.c;xref:0x142cfe2cb,0x142cfe384,0x1434bc592 stride 0x10, indexed by vehicle id
};
