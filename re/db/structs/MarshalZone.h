// struct: MarshalZone
// subsystem: live
// size: ?
// notes: One marshal zone. flag: 0 none, 1 green after a hazard, 3 yellow, 4 red (Halo, from Sess_MarshalZone_CheckFlags 0x14357ce30).
struct MarshalZone {
    /* 0x000 */ MarshalZoneDef * definition;                 // L2 decomp:re/decomp/142cfaec0.c;xref:0x142cfb360,0x142cfb396,0x142cfb469;decomp:re/decomp/14357ce30.c
    /* 0x014 */ uint8_t flag;                                // L2 decomp:re/decomp/142cfaec0.c;xref:0x142cfb51b,0x142cfb770,0x142cfb8ec
};
