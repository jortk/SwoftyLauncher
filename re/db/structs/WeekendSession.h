// struct: WeekendSession
// subsystem: session
// size: 0x10 L2 decomp:re/decomp/142a23b30.c;decomp:re/decomp/149439460.c
struct WeekendSession {
    /* 0x000 */ StandingsPreset *standings;      // L1 decomp:re/decomp/142a23b30.c;xref:0x142a24018 the standings label resolved at parse time to g_Game_Scenarios +0xe8 + stat_id * 0x20, or 0; read live by 0x149439460 for the points recompute 0x1413cdff0
    /* 0x008 */ uint32_t session_id;             // L2 decomp:re/decomp/142a23b30.c enum table 0x1457c96a0: 1-4 practice, 5-9 qualifying, 10-14 sprint_shootout, 15 race, 16 race_2, 17 race_3, 18 time_trial
    /* 0x00c */ uint32_t session_to_apply_penalties_to; // L1 decomp:re/decomp/142a23b30.c
};
