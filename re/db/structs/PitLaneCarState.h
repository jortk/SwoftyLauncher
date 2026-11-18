// struct: PitLaneCarState
// subsystem: live
// size: ?
// notes: Per-vehicle pit state. The UDP lap-data builder reports pit status 0 when [[system+0x48]+0x938+id*0x10]+0x103 is 0, else 2 when state == 4 else 1. Rules in the gap code: states 2..7 and 14..15 clear gaps; the full-grid HUD list shows PIT for 2..15.
struct PitLaneCarState {
    /* 0x004 */ uint32_t state;                              // L2 decomp:re/decomp/140813ee0.c;xref:0x1408140ac,0x1408140e3,0x1408142dc;decomp:re/decomp/142cfd6c0.c 0..15
    /* 0x014 */ uint32_t pit_stops;                          // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfe2fe low byte feeds UDP numPitStops
};
