// struct: CarSteeringLive
// subsystem: live
// size: ?
// notes: Runtime steering fields of the Steering system (CarLive+0x560, structs/CarSteering.h). The lock is CarSteering.max_steering_lock (+0x28). steer = -angle/lock (UDP builder): (float)((a ^ b) + 0x80000000) / lock bits.
struct CarSteeringLive {
    /* 0x0e8 */ CarObfFloat steering_angle;                  // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9f02,0x142cf9f0a rad, XOR pair a=+0xe8 b=+0xf0; the game builds steer as -angle/lock
};
