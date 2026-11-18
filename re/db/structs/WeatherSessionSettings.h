// struct: WeatherSessionSettings
// subsystem: session
// size: ?
// notes: the session weather settings setupSession reads (rdx): filled from the WeatherAndTimeOfDay
// component by Sess_WeatherComponent_FillSessionSettings 0x140bde510 (weekend hub) or by the scripted-weather
// filler 0x141352160. Single-dataset path (session_type 0x12, or a current game mode whose class chain DOES
// contain 0x14604e6b8: slot path iff NOT IsA and session_type != 0x12, 0x1438b7568..0x1438b768a, V9397): single_count, single_random and
// single_types go to Sess_Tod_BuildDataset_Thunk 0x14461bb70 for dataset 0 with null hold/trans. Every other
// session builds all eight datasets from slots[] (kb:subsystems/session.md#weather). Time-of-day block
// +0xb28 + d*8 (d = 0..7, L1, not listed): {u8 start preset 0..7 (hub 0x140bde633, read 0x1438b7d05 into the
// start-time switch on the solar times gws+0x1e8c..+0x1eac: 7 = 0 h), pad, f32 factor +0xb2c (hub writes 1.0 or
// 0x14568e894[i] = i, read 0x1438b7e7c; consumer not traced)}. Hub preset = comp byte + 1 (table 0x7060504030201).
// Raised by ce-weather; slot blocks and the single path by engine-re (weather to L2).
struct WeatherSessionSettings {
    /* 0x004 */ uint32_t session_type;  // L2 decomp:re/decomp/1438b7350.c;xref:0x1438b7679 game session type (0x12 time trial takes the single-dataset path)
    /* 0xb10 */ uint8_t tod_flag_a;     // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b7bb6,0x140bde571,0x140bde5a5 with tod_flag_b both 0 setupSession takes the per-dataset start presets (+0xb28 + d*8); hub writes the pair as a u16 (1, 0x100 or 0) by the component's time-of-day type +0x2c8
    /* 0xb11 */ uint8_t tod_flag_b;     // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b7bf0,0x140bde5bd see tod_flag_a
    /* 0xb14 */ uint32_t mode;          // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b7f60,0x1438b7f81,0x140bde57a,0x140bdea7a 0 = custom slot blocks, else realistic generator mode (clamped to 1 above 3); hub writes 0 when the component's weather type +0x2db is 1 (custom slots)
    /* 0xb18 */ uint8_t exact_timing;   // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/141352160.c;xref:0x1438b7524,0x1438b7697,0x14135227e copied to TimeOfDaySystem.exact_timing; set by the scripted-weather filler; when set setupSession builds every dataset from its slot block with use_count = 1
    /* 0xb19 */ uint8_t tod_start_preset; // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b837c,0x140bde606,0x140bde7b6 start-time preset (0..7) of the session being set up, used when tod_flag_b is clear (else a random pick 0x1438c4e40 seeded by gws+0x1f1c/+0x1f18/+0x1f10); selects the case of the start-time switch at 0x1438b8400; the stat writer 0x143792efd treats 7 as custom
    /* 0xb1c */ float time_scale;       // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b7490,0x140bde5ae,0x140bde7e5 third argument of Sess_GameWeatherSystem_SetActiveSessionType (time_scale); hub writes 1.0 or a value from the 6-entry table 0x14568e894
    /* 0xb20 */ uint32_t single_count;  // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b7753,0x140bdea9e,0x140bdec07 state count of the single-dataset path (count argument of 0x14461bb70)
    /* 0xb68 */ uint8_t single_random[6]; // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b7759,0x140bdead0,0x140bdeafc random flags of the single-dataset path (same encoding as WeatherSlotBlock.random)
    /* 0xb6e */ uint8_t single_types[6]; // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b7760,0x140bdeadc,0x140bdec51 weather types of the single-dataset path
    /* 0xb74 */ WeatherSlotBlock slots[8]; // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;decomp:re/decomp/141352160.c;xref:0x1438b7690,0x1438b76da,0x140bdeb76,0x141352366 per-dataset slot blocks (+0xb74 + d*0x44, d = P1 P2 P3 Q SS R R2 R3); slots[3].length (+0xc40) and slots[4].length (+0xc84) are also read directly (0x1438b8099, 0x1438b80c1) as the generator length for Q/SS when session_type is 8 or 0xd
};
