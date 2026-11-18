// struct: PitLaneSystem
// subsystem: live
// size: ?
// notes: Registry key 0x1460fb708, type name "PitLaneSystem" (static init 0x1434e782d). Per-vehicle pit state records are inline at +0x68, stride 0x10. Linked into RelativeGapHelper at +0xd0 and into the UDP state at +0xd210. +0x48 is the lane object (heap object released by the destructor 0x1434e6ad0 through FUN_1434b47f0).
struct PitLaneSystem {
    /* 0x048 */ PitLaneTrack *lane;                          // L2 decomp:re/decomp/142cfd6c0.c;decomp:re/decomp/1434bc540.c;xref:0x142cfe2c7,0x142cfe379,0x1434bc58e UDP lap-data builder and Game_Pitlane_SetVehicleStatus (self = this system) both load it and index +0x938 by vehicle id
    /* 0x068 */ PitLaneCarSlot car_slots[27];                // L2 decomp:re/decomp/142cfd6c0.c;decomp:re/decomp/140813ee0.c;xref:0x142cfd73e,0x142cfd7fa,0x142cfd85d,0x140813f4c,0x140814007,0x1408140a7 indexed by vehicle id; the game guards id < 0x1b
};
