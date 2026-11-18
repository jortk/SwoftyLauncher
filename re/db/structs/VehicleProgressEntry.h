// struct: VehicleProgressEntry
// subsystem: live
// size: ?
// notes: One per vehicle, [GameVehicleProgressSystem.entries_begin + id*8]. GetLapProgress returns entry+0x30, so the block fields below are entry+0x30+x. Values are XOR pairs (anti-tamper). lap_m = clamp(total - (progress_lap + delta) * track_length, 0, track_length).
struct VehicleProgressEntry {
    /* 0x038 */ uint16_t lap_a;                              // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfda82;decomp:re/decomp/1446046a0.c lap counter = (int16)(lap_a ^ lap_b), 0 on lap 1
    /* 0x040 */ uint16_t lap_b;                              // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfda95
    /* 0x048 */ uint16_t progress_lap_a;                     // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfda86,0x142cfddd2,0x142cfdea6,0x142cfdf9e,0x142cfe03b progress lap = (int16)(a ^ b) + (int16)delta
    /* 0x050 */ uint16_t progress_lap_b;                     // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfda8a,0x142cfddd7,0x142cfdeb4,0x142cfdfa2
    /* 0x058 */ int16_t progress_lap_delta;                  // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfda91,0x142cfdde0,0x142cfdeaf,0x142cfdfa9
    /* 0x098 */ uint32_t total_a;                            // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfddcc,0x142cfdea2,0x142cfdf94,0x142cfe01a,0x142cfe028 total distance m = float bits (a ^ b)
    /* 0x0a0 */ uint32_t total_b;                            // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfddcf,0x142cfdeab,0x142cfdf97,0x142cfe01d
};
