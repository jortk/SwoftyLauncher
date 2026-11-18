// struct: VisualCompound
// subsystem: live
// size: 0x58
// notes: One tyres.visual_compounds row as loaded by Sess_Load_VisualTyreCompounds (0x142e985e0): a vector of 0x58-byte records indexed by the row's stat id & 0xff. TyreSlot.visual points at one (slot build 0x14f463120 body, thunk 0x142e9e540). audio_compound is the row's audio_id string through a strcmp ladder: HyperSoft 0, UltraSoft 1, SuperSoft 2, Soft 3, Medium 4, the label at 0x1457ef9e8 is SuperHard 6, Intermediate 7, Wet 8, anything else -1. The stock Hard label is the ladder step between Medium and SuperHard, value 5.
struct VisualCompound {
    /* 0x000 */ uint8_t stat_id;                             // L2 decomp:re/decomp/142e985e0.c;decomp:re/decomp/142ea2a90.c;xref:0x142ea2b81 stat_id byte of the row; the fitted-set code reads it through [slot+0x10] as the visual compound id
    /* 0x050 */ int32_t audio_compound;                      // L2 decomp:re/decomp/142e985e0.c;xref:0x142e98710 written from the audio_id label ladder, -1 when empty or unknown
};
