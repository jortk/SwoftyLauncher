// struct: PitLaneVehicle
// subsystem: live
// size: ?
// notes: Per-vehicle pit lane movement record (stand-in name). The pit movement update 0x1434b83e0 sets in_lane in case 10 (0x1434b91e1) and tests it for the lane states 2,3,6,7,14,15 (0x1434b85a0). UDP lap data pit status = 0 when in_lane is 0, else 2 when PitLaneCarState.state == 4, else 1 (0x142cfe2b4-0x142cfe2ed).
struct PitLaneVehicle {
    /* 0x103 */ uint8_t in_lane;                             // L2 decomp:re/decomp/142cfd6c0.c;decomp:re/decomp/1434b83e0.c;xref:0x142cfe2dc,0x142cfe38c,0x1434b91e1,0x1434b85a0,0x1434b88c7 nonzero while the vehicle is in the pit lane
};
