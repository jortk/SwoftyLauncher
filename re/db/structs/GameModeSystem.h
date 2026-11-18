// struct: GameModeSystem
// subsystem: live
// size: ?
// notes: Registry key 0x14604ed28 (vtable 0x145647e80, slot 0 0x140820ec0 returns the key). The same object as [g_Game_Manager+0xc0] (struct GameModeContext, subsystem game): 0x140249730 reads [[GM+0xc0]+0x88]+0x260 and 0x1407a1870 reads [[registry(key)]+0x88]+0x260, and the HUD view-model init 0x141759a40 stores registry(key) at its +0x20 where 0x141760020 then reads +0x88.
struct GameModeSystem {
    /* 0x088 */ GameMode *mode;                              // L2 decomp:re/decomp/141759a40.c;decomp:re/decomp/141760020.c;decomp:re/decomp/1407a1870.c;decomp:re/decomp/1407d07a0.c;xref:0x141759ab1,0x141760032,0x1407a18ae,0x1407d07ad the current game mode object (Game_Game_SetCurrentGameMode stores it at 0x1407d0897)
};
