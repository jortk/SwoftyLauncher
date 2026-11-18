// struct: TimeOfDayClock
// subsystem: session
// size: 0x288 L2 decomp:re/decomp/14461f400.c;xref:0x14461f739 (imul 0x288 by the active dataset index)
// notes: per-dataset time-of-day clock at TimeOfDaySystem+0x2c8 (10 of them, one per WeatherDataset). The
// TOD update computes weather progress = (time_a + time_b) * time_scale / dataset_length and clamps it to
// 0..1. Raised by ce-weather.
struct TimeOfDayClock {
    /* 0x022 */ uint8_t hours_24;       // L2 decomp:re/decomp/14029c310.c;decomp:re/decomp/14461f400.c;xref:0x14029c354,0x14029c3bb,0x14461f7a4,0x15522866a when set the clock runs in 24 h units: curve evaluation multiplies the fraction by 24 and the state search divides each state end by 24; written by 0x155228600 (thunk 0x144617290) at 0x15522866a with (arg3 < 6)
    /* 0x068 */ double time_a;          // L2 decomp:re/decomp/14461f400.c;xref:0x14461f740 first term of the elapsed weather time
    /* 0x070 */ double time_b;          // L2 decomp:re/decomp/14461f400.c;xref:0x14461f747 second term of the elapsed weather time
    /* 0x080 */ float time_scale;       // L2 decomp:re/decomp/14461f400.c;xref:0x14461f74e multiplier on the elapsed time
    /* 0x084 */ float length;           // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c0b1,0x155755609 max(dataset length, 0.01) copied by the builder and the build thunk body
    /* 0x090 */ float fixed_fraction;   // L2 decomp:re/decomp/14029c310.c;decomp:re/decomp/14461f400.c;xref:0x14029c34c,0x14461f6f7 fraction a TodCurveBinding with mode 1 samples instead of the elapsed time; written by the TOD update at 0x14461f6f7
};
