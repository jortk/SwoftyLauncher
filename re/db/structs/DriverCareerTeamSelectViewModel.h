// struct: DriverCareerTeamSelectViewModel
// subsystem: game
// size: ?
// notes: game::DriverCareerTeamSelectViewModel (vtable vt_Game_DriverCareerTeamSelectViewModel 0x145729db0; message handler is slot 10, Game_DriverCareerTeamSelect_OnMessage). Raised by bind552 (the career lane owner career-re is stale; the lead authorised --force).
struct DriverCareerTeamSelectViewModel {
    /* 0xf20 */ uint32_t selected_team_key;         // L2 decomp:own capstone disasm 0x141a69a50-0x141a6a5a4;xref:0x141a69eb6,0x141a69ff5,0x141a6a005,0x141a6a405 the u32 team key of the selected team ((word & 0x3fff) << 16 | word >> 14 of the team record's first word, stored at 0x141a69eb6), read back at 0x141a69ff5 into a record's +0x14 and at 0x141a6a005 shifted left 14 into a u16 argument
};
