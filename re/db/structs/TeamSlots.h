// struct: TeamSlots
// subsystem: live
// size: 0x20
// notes: Vector of 0x10-byte {allocator, TeamRecord *} entries indexed by team stat id (Team_Insert_RecordByStatId, see struct TeamRecord).
struct TeamSlots {
    /* 0x008 */ TeamSlot *begin;                             // L2 decomp:re/decomp/141760020.c;xref:0x1417600dd
    /* 0x010 */ TeamSlot *end;                               // L2 decomp:re/decomp/141760020.c;xref:0x1417600e1 the pill checks index < (end - begin) >> 4 before reading
};
