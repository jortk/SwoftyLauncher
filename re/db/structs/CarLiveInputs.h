// struct: CarLiveInputs
// subsystem: live
// size: ?
// Driver-input and flag fields of CarLive (structs/CarLive.h, subsystem car) that CarLive.h does not list. Overlay at the same offsets; owner car-re may merge. Source: the game UDP car-telemetry builder Game_TelemetryUdp_UpdateRace, which reads them for every vehicle id.
// Throttle and brake swap when GearBox gear_index == 0 and (input_flags & 0x88) == 8 (brake-to-reverse), exactly as the UDP builder does.
struct CarLiveInputs {
    /* 0x188 */ float throttle;                              // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9eaf,0x142cf9ed0 0..1; swapped with brake in reverse (see notes)
    /* 0x18c */ float clutch;                                // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9f35 0..1, UDP sends x100
    /* 0x190 */ float brake;                                 // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9eb6,0x142cf9ee5 0..1
    /* 0x224 */ uint8_t input_flags;                         // L2 decomp:re/decomp/142cf9820.c;xref:0x142cf9ea0 (flags & 0x88) == 8 with gear_index 0 swaps throttle/brake
};
