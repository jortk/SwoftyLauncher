// struct: CarBodyState
// subsystem: live
// size: ?
// notes: Physics state of the car body, reached as [[chassis+0x340]+0x18].
struct CarBodyState {
    /* 0x1c0 */ float position[3];                           // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9a78 world position (the UDP motion block copies 12 bytes from here)
    /* 0x220 */ float velocity[3];                           // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9a87,0x142cf9e69 world velocity m/s; speed = |velocity|
};
