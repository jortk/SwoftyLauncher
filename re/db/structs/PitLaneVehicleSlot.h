// struct: PitLaneVehicleSlot
// subsystem: live
// size: 0x10
// notes: 16-byte slot of PitLaneTrack.vehicle_slots; the second qword is not read by the readers this lane covers.
struct PitLaneVehicleSlot {
    /* 0x000 */ PitLaneVehicle *lane;                        // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfe2cb,0x142cfe384 loaded and dereferenced at +0x103
};
