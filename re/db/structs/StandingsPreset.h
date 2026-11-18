// struct: StandingsPreset
// subsystem: session
// size: 0x20 L2 decomp:re/decomp/142a2cd50.c;decomp:re/decomp/1413cdff0.c
// notes: one game_setup.session_standings.session_standing_preset row, parsed by 0x142a2cd50 into the vector at
// g_Game_Scenarios +0xe0 (begin +0xe8, end +0xf0, reserved up front, row order) before the setup modes. No label.
struct StandingsPreset {
    /* 0x008 */ StandingsPointsLevel *levels_begin; // L2 decomp:re/decomp/142a2cd50.c;decomp:re/decomp/1413cdff0.c level 0 = full points, then one per reduced_points_data row
    /* 0x010 */ StandingsPointsLevel *levels_end;   // L2 same
};

