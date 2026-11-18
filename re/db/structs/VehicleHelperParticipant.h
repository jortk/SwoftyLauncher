// struct: VehicleHelperParticipant
// subsystem: live
// size: ?
// notes: One element of VehicleHelperSystem's participant pointer vector [+0x88,+0x90). Sess_Get_CarDriverName (0x142f28c10) finds a car's participant by [[p+0x18]+0x58]+0xe8 (CarPhysicsRecord.car -> CarLive.vehicle_id) and caches it at system+0xa0+vehicle*8.
struct VehicleHelperParticipant {
    /* 0x018 */ CarPhysicsRecord *physics;                   // L2 decomp:re/decomp/142f28c10.c;xref:0x142f28cd6 null-checked, then [physics+0x58] (0x142f28ce2) and its byte +0xe8 (0x142f28ceb) are compared with the vehicle id
    /* 0x032 */ uint8_t driver_kind;                         // L2 decomp:re/decomp/142f28c10.c;xref:0x142f28c57,0x142f28cfd flags byte: bit 7 set (js 0x142f28c5c) = a human drives the car and Sess_Get_CarDriverName takes VehicleHelperSystem.player_name, else the name comes from the entrant's driver record; bit 0x40 is tested elsewhere (0x142f296b8, V10700)
};
