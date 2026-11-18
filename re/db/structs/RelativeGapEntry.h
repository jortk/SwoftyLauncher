// struct: RelativeGapEntry
// subsystem: live
// size: 0x98
// notes: One per vehicle in RelativeGapHelper.entries (stride 0x98). Gaps are i64 milliseconds; a gap counts only while its valid byte is non-zero. The UDP lap-data builder copies them as gap-to-leader / gap-to-car-ahead.
struct RelativeGapEntry {
    /* 0x058 */ int64_t leader_gap_ms;                       // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd808,0x142cfda56,0x142cfe466
    /* 0x068 */ uint8_t leader_gap_valid;                    // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd73e,0x142cfd7fa,0x142cfd85d
    /* 0x070 */ int64_t interval_ms;                         // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd709,0x142cfd861,0x142cfd8dd
    /* 0x080 */ uint8_t interval_valid;                      // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd700,0x142cfe49d
    /* 0x088 */ uint8_t gained;                              // L1 decomp:re/decomp/140813ee0.c Halo: the game's gained/lost flags
    /* 0x089 */ uint8_t lost;                                // L1 decomp:re/decomp/140813ee0.c
    /* 0x090 */ uint8_t vehicle_id;                          // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd6f7,0x142cfd83c,0x142cfda27
};
