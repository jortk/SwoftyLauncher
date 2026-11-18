// struct: GameDecalCustomisationSystem
// subsystem: game
// size: ?
// notes: proposed by sponsor-studio (Sponsor Studio research); fields at L1, see re/research/sponsors.md.
struct GameDecalCustomisationSystem {
    /* 0x000 */ void **vtable;                           // L1 doc:re/research/sponsors.md#decal-system 0x1456e3b50 (observed in game). (seen in game in the contributor's runs)
    /* 0x098 */ ProjectionAreaRecord *areas_begin;       // L1 doc:re/research/sponsors.md#decal-system Area records, 0x108 stride (..+0xa0 end).
    /* 0x0a0 */ ProjectionAreaRecord *areas_end;         // L1 doc:re/research/sponsors.md#decal-system 
    /* 0x168 */ uint8_t dirty;                           // L1 doc:re/research/sponsors.md#decal-system Editor dirty byte; cleared by writeback when flag != 0. (static L2 in the doc)
    /* 0x178 */ void *editor_nodes_begin;                // L1 doc:re/research/sponsors.md#decal-system Editor node list, 16-byte entries {node*, refcount*}; same layout as saved livery lists. (static L2 in the doc)
    /* 0x180 */ void *editor_nodes_end;                  // L1 doc:re/research/sponsors.md#decal-system  (static L2 in the doc)
    /* 0x1a0 */ uint64_t uid_counter;                    // L1 doc:re/research/sponsors.md#decal-system Counter used when allocating uniqueId (T013 hypothesis; observed allocation = max+1).
    /* 0x1a8 */ uint32_t editor_livery_key;              // L1 doc:re/research/sponsors.md#decal-system Key of the livery loaded in the editor (vehicle customisation stat id); 0x3fff0000 = none (set by writeback before reload). Runtime: 0x110001 in My Team, 0x100001 F1 World. (static L2 in the doc)
    /* 0x1ac */ uint8_t service_key;                     // L1 doc:re/research/sponsors.md#decal-system Byte used as key into the +0x2b0 map; copy reloads only if that entry is type 5. (static L2 in the doc)
    /* 0x258 */ void *entrants;                          // L1 doc:re/research/sponsors.md#decal-system Vehicle customisation store (lists at +0x388); same object as playerCustomisationInfoSystem+0x280 is an open check. (static L2 in the doc)
    /* 0x278 */ void *post_target;                       // L1 doc:re/research/sponsors.md#decal-system Writeback with flag != 0 posts via this. (static L2 in the doc)
};
