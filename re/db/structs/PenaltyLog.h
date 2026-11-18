// struct: PenaltyLog
// subsystem: live
// size: ?
// notes: Registry key 0x1460fdfe0, type name "PenaltyLog" (static init 0x1435f69f3). Records are appended by the rule manager; the UDP lap-data builder counts warnings and track-limit warnings from it.
struct PenaltyLog {
    /* 0x048 */ PenaltyRecord * records_begin;               // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd927,0x142cfe21d,0x142cfe2c7
    /* 0x050 */ PenaltyRecord * records_end;                 // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd72d,0x142cfd92b,0x142cfe217
};
