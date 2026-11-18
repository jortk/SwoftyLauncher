// struct: WeatherDataset
// subsystem: session
// size: 0xe00 L2 decomp:re/decomp/14461f400.c;xref:0x14461f70d (imul 0xe00 by the active dataset index)
// notes: one TimeOfDaySystem weather dataset (eWDS*: P1, P2, P3, Q, SprintShootout, Race, Race2, Race3,
// PhotoMode, Override; names 0x145bf61f8). Built by Tod_Build_WeatherScenario 0x14461bea0 and read every
// frame by the TOD update 0x14461f400 to find the current state. Raised by ce-weather; transitions
// (0x000..0xb90) by engine-re (weather to L2).
struct WeatherDataset {
    /* 0x000 */ WeatherTransition transitions[37]; // L2 decomp:re/decomp/14461d300.c;decomp:re/decomp/14461d090.c;decomp:re/decomp/14461f400.c;xref:0x14461dd3d,0x14461d9b6,0x14461f809,0x14461f983 per-transition records 0x50 apart (0x000..0xb90): record i blends states[i] into states[i+1]; reset by 0x14461d090 (37 records, loop to +0xb40), filled by the stitch, read by the update
    /* 0xb90 */ WeatherState states[38]; // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c8b3,0x14461f7d7 inline state array (0xb90..0xdf0; the builder caps the count at 0x26)
    /* 0xdf0 */ uint32_t state_total;   // L2 decomp:re/decomp/14461d090.c;xref:0x14461d26d full state count min(n,0x26) written by the reset helper 0x14461d090; the builder fills that many states (V0563)
    /* 0xdf8 */ uint32_t state_count;   // L2 decomp:re/decomp/14461f400.c;xref:0x14461f71e,0x14461c94f,0x14461d274 number of states the update and the exact-timing loop walk = states - 1 (0x14461d274); the full state count min(n,0x26) is at +0xdf0 (0x14461d26d); the last state is only the blend target of the one before it (V0563)
};
