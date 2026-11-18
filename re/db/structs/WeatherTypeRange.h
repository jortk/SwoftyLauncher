// struct: WeatherTypeRange
// subsystem: session
// size: 0x14 L2 decomp:re/decomp/14461bea0.c;xref:0x14461c8d3 (type*5*4 indexing)
// notes: per weather type record at TimeOfDaySystem+0xb378. setupSession copies the track config's
// per-type weight into +0x00; the builder draws a state's intensity uniformly in [intensity_min,
// intensity_max]. Raised by ce-weather.
struct WeatherTypeRange {
    /* 0x000 */ float weight;           // L2 decomp:re/decomp/1438b7350.c;xref:0x1438b74c1,0x14461c11f copied from the track weather config (+0x40,+0x44,+0x48,+0x4c,+0x50 for types 0..4) by setupSession; the builder weights types 0..4 only; type 5 (stormy) weight is unused (V0573); career-editor never reads or writes it
    /* 0x004 */ float unk_0x4;          // L1 decomp:re/decomp/151a3ebf0.c;xref:0x151a3ed6d written with intensity_min/max by 0x151a3ebf0 from dev parameters (0 for types 0..2); reader not found
    /* 0x00c */ float intensity_min;    // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c8d7,0x151a3ed7d,0x151a3eda4,0x151a3edcb lower bound of the intensity draw; written by Sess_GameWeatherSystem_SetIntensityRanges_Body 0x151a3ebf0 (types 0..2 zeroed, light rain / heavy rain from the championship override at entity+0x3b0 or fixed 0.33 / 0.66 / 1.0 for scripted or non-custom setups, else the parameter handles 0x146106018..0x146106040; nothing is written while the byte 0x146106058 is 0)
    /* 0x010 */ float intensity_max;    // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c8dd,0x151a3ed85,0x151a3edac,0x151a3edd3 upper bound of the intensity draw; same writer
};
