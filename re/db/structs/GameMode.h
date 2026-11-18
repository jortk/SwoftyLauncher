// struct: GameMode
// subsystem: live
// size: 0x2a0 L2 decomp:re/decomp/1407cae80.c (Game_GameModes_BuildMode allocates the 0x2a0-byte mode object)
// notes: One registered game mode, built by Game_GameModes_BuildMode (0x1407cae80) from a game_modes row. name is an inline NUL-terminated buffer (+0x280, capacity 0x20 at +0x270) holding the row label: stock labels time_trial, grand_prix, grand_prix_new, grand_prix_resume, grand_prix_continue, grand_prix_continue_pressed; Game_Game_SetCurrentGameMode logs it as the mode name.
struct GameMode {
    /* 0x260 */ EntrantsManager *entrants_manager;           // L2 decomp:re/decomp/141760020.c;decomp:re/decomp/1407a1870.c;xref:0x14176004c,0x1407a18b5 the object whose session entrant vector +0x2e0 the timing pill reads
    /* 0x278 */ char *name;                                  // L2 decomp:re/decomp/1407cae80.c;decomp:re/decomp/1407d07a0.c;xref:0x1407cbcab,0x1407d083f,0x1407d08aa the mode label C string; the builder points it at the inline buffer obj+0x280
};
