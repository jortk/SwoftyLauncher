// struct: WeekendStructure
// subsystem: session
// size: 0x88 L2 decomp:re/decomp/142a2cb60.c;decomp:re/decomp/1407c75f0.c (GameSetupMode +0x190 element stride)
// notes: one weekend_options_0.weekend_structure row of a game setup mode, parsed by 0x142a23b30. The structure is
// picked by its sprint flag only (0x149439460: the weekend's +0x23c/+0x244 flag, else the lobby option comp+0x490).
struct WeekendStructure {
    /* 0x008 */ WeekendSession *sessions_begin;  // L2 decomp:re/decomp/142a23b30.c;decomp:re/decomp/149439460.c 16-byte entries, one per session, de-duplicated by session id
    /* 0x010 */ WeekendSession *sessions_end;    // L2 same
    /* 0x080 */ bool is_sprint;                  // L2 decomp:re/decomp/142a23b30.c;decomp:re/decomp/149439460.c weekend_format FNV-1a (lowercase) == "sprint" (0xb4d8c10d825b055)
};

