// struct: BadgeInfo
// subsystem: game
// size: ?
// notes: proposed by sponsor-studio (Sponsor Studio research); fields at L1, see re/research/sponsors.md.
struct BadgeInfo {
    /* 0x0b0 */ uint32_t emblem_key;                     // L1 doc:re/research/sponsors.md#badge (BadgeEmblem.id << 16) | 8. (static L2 in the doc)
    /* 0x0b8 */ float emblem_rotation;                   // L1 doc:re/research/sponsors.md#badge  (static L2 in the doc)
    /* 0x0bc */ float emblem_scale;                      // L1 doc:re/research/sponsors.md#badge  (static L2 in the doc)
    /* 0x110 */ uint32_t background_key;                 // L1 doc:re/research/sponsors.md#badge (id << 16) | 7. (static L2 in the doc)
    /* 0x160 */ uint32_t stat_id;                        // L1 doc:re/research/sponsors.md#badge Own key of the stored record (set by replace 0x1424710e0). (static L2 in the doc)
};
