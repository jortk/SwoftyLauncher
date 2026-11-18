// struct: PenaltyRecord
// subsystem: live
// size: 0x30
// notes: Penalty log record, stride 0x30.
struct PenaltyRecord {
    /* 0x008 */ uint8_t vehicle_id;                          // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd88a,0x142cfd959,0x142cfda82
    /* 0x009 */ uint8_t penalty_type;                        // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd963 5 = warning
    /* 0x00a */ uint8_t infringement;                        // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd96f track limits when infringement < 0x1e and (0x3e000380 >> infringement) & 1
};
