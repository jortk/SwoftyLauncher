// struct: StopGoObject
// subsystem: live
// size: ?
// notes: Unserved stop-go records, 4 bytes each in [+0x68,+0x70), vehicle-indexed; byte +3 is the unserved count.
struct StopGoObject {
    /* 0x068 */ uint8_t * records_begin;                     // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd73e,0x142cfd7fa,0x142cfd85d
    /* 0x070 */ uint8_t * records_end;                       // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd709,0x142cfd861,0x142cfd8dd
};
