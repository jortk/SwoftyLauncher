// struct: CarChassisLive
// subsystem: live
// size: ?
// notes: Runtime fields of the Chassis system (CarLive+0x558, structs/CarChassis.h in subsystem car) not listed there. Same offsets as CarChassis.
struct CarChassisLive {
    /* 0x320 */ float g_lat;                                 // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9c45 lateral g (UDP gForceLateral x1000 clamped); sign as the game writes
    /* 0x324 */ float g_long;                                // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9c85 longitudinal g
    /* 0x328 */ float g_vert;                                // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9c65 vertical g
    /* 0x340 */ CarBodyActor * body;                         // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9a4c,0x142cf9e59 physics body wrapper; null before the car is placed
};
