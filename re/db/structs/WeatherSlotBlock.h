// struct: WeatherSlotBlock
// subsystem: session
// size: 0x44  L2 decomp:re/decomp/1438b7350.c;xref:0x1438b76d2,0x1438b8017,0x140bdeb6e (shl 6 + idx*4 = idx*0x44)
// notes: one per-dataset weather slot block of WeatherSessionSettings (+0xb74 + dataset*0x44, datasets 0..7 =
// P1 P2 P3 Q SS R R2 R3). Writers: weekend hub filler Sess_WeatherComponent_FillSessionSettings 0x140bde510
// (len, count, random, types from the component's per-session UI slots, no hold/trans) and the scripted-weather
// filler Sess_ScriptedWeather_FillSlots 0x141352160 (exact timing: hold/trans per state, random 0,
// len = sum of hold + trans). Reader: Sess_GameWeatherSystem_SetupSession 0x1438b7350 passes the six members
// as the arguments of Sess_Tod_BuildSessionDataset_Thunk 0x14479c710 (types, hold_s, trans_s, random, count,
// len_s); kb:subsystems/session.md#weather. Raised by engine-re (weather to L2).
struct WeatherSlotBlock {
    /* 0x000 */ float length_s;         // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b76da,0x1438b8088,0x140bdeb85,0x1413523a0 dataset length in weather seconds -> len_s; hub copies the session's length (+0x64 of the session record), scripted = sum of hold + trans
    /* 0x004 */ uint32_t count;         // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b76e0,0x1438b801f,0x140bdeb76,0x1413522b3 number of states used (<= 6) -> count
    /* 0x008 */ float hold_s[6];        // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/141352160.c;xref:0x1438b76f2,0x1438b8031,0x14135247a hold seconds per state -> hold_s; only the scripted filler writes them (exact timing)
    /* 0x020 */ float trans_s[6];       // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/141352160.c;xref:0x1438b76ed,0x1438b802c,0x141352481 transition seconds per state -> trans_s
    /* 0x038 */ uint8_t random[6];      // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b76e9,0x1438b8028,0x140bdebb0,0x140bdebd8,0x141352551 1 = the builder picks the type at random; hub sets it for UI slot values 0, 6, 7; scripted writes 0
    /* 0x03e */ uint8_t type[6];        // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/140bde510.c;xref:0x1438b76f7,0x1438b8036,0x140bdebb8,0x14135236d engine weather type per state (0 clear .. 5 stormy, WeatherState.type); hub maps UI slot value v=1..5 to v-1
};
