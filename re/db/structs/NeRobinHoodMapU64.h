// struct: NeRobinHoodMapU64
// subsystem: live
// size: ?
// notes: The 8-byte-key robin-hood map the game manager keeps its systems in (find: Ne_RobinHoodMap_U64_FindIndex 0x14030ffd0). Slots are 16-byte {key, value}; a miss returns end == [map+0x20].
struct NeRobinHoodMapU64 {
    /* 0x010 */ uint64_t hash_seed;                          // L2 decomp:re/decomp/144500dc0.c;xref:0x144500ddd,0x144500e33,0x144500e66;decomp:re/decomp/142882980.c
    /* 0x018 */ uint8_t * slots;                             // L2 decomp:re/decomp/144500dc0.c;decomp:re/decomp/142882980.c;xref:0x144500dd9,0x144500def,0x144500e22,0x1428829a2 16-byte {key,value} slots
    /* 0x020 */ uint8_t * meta;                              // L2 decomp:re/decomp/144500dc0.c;decomp:re/decomp/142882980.c;xref:0x144500fc0,0x1445010bf,0x1445010f5,0x1428829a6 one info byte per slot (0 = empty); doubles as the slot-array end, so slot count = (meta-slots)/16
    /* 0x028 */ uint64_t count;                              // L2 decomp:re/decomp/144500dc0.c;xref:0x144500ebd,0x144501043,0x1445010a0
    /* 0x030 */ uint64_t mask;                               // L2 decomp:re/decomp/144500dc0.c;xref:0x144500e78,0x144500e7d,0x144500fcf
    /* 0x038 */ uint64_t max_count;                          // L2 decomp:re/decomp/144500dc0.c;xref:0x144500e5f,0x144500e85,0x144500ed0
    /* 0x040 */ uint32_t step;                               // L2 decomp:re/decomp/144500dc0.c;xref:0x144500dd0,0x144500fc4,0x144501114
    /* 0x044 */ uint32_t shift;                              // L2 decomp:re/decomp/144500dc0.c;xref:0x144500fba,0x144501167
};
