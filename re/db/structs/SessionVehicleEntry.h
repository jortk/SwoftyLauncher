// struct: SessionVehicleEntry
// subsystem: live
// size: ?
// notes: Per-vehicle result entry of SessionSystem. Anti-tamper XOR pairs. result status numbering 0..7 as the UDP result status: 2 running, 3 finished, 4 DNF, 5 DSQ, 6 not classified, 7 retired.
struct SessionVehicleEntry {
        /* 0x000 */ uint8_t vehicle_id;                          // L2 decomp:re/decomp/142c79940.c;decomp:re/decomp/142c22ba0.c;xref:0x142c79954,0x142c22c81 written by the entry constructor 0x142c79940 (*entry = vehicle id argument), read back by Sess_Result_RequestStatus for its 'Vehicle %d requested status' log
    /* 0x018 */ uint8_t status;                              // L2 decomp:re/decomp/142cfd6c0.c;decomp:re/decomp/142cf9820.c;xref:0x142cfd7eb,0x142cfd89e,0x142cfda4a,0x142cf9963,0x142cf9a5c,0x142cf9e65 low byte of a dword; (status & 0xfe) == 2 counts running or finished cars
    /* 0x038 */ uint64_t time_penalty_a;                     // L2 decomp:re/decomp/143573b40.c;decomp:re/decomp/142cfd6c0.c;xref:0x143573c18,0x143573c35,0x143573cc8,0x142cfd824,0x142cfdbc6,0x142cfdc00 time penalty seconds = double bits (a ^ b)
    /* 0x040 */ uint64_t time_penalty_b;                     // L2 decomp:re/decomp/143573b40.c;decomp:re/decomp/142cfd6c0.c;xref:0x143573bb5,0x143573c14,0x143573d00,0x142cfd820,0x142cfe223
    /* 0x048 */ uint32_t position_a;                         // L2 decomp:re/decomp/141484fb0.c;xref:0x141485841,0x14148584d race position - 1 = a ^ b (the HUD adds 1 at 0x141485847)
    /* 0x050 */ uint32_t position_b;                         // L2 decomp:re/decomp/141484fb0.c;xref:0x141485844
    /* 0x090 */ uint32_t grid_position_a;                    // L2 decomp:re/decomp/142cfd6c0.c;decomp:re/decomp/141484fb0.c;xref:0x142cfd6f7,0x142cfd83c,0x142cfda27,0x141484fda,0x1414850b0,0x1414850de grid position - 1 = a ^ b; UDP lap data and the formation-lap HUD use it
    /* 0x098 */ uint32_t grid_position_b;                    // L2 decomp:re/decomp/142cfd6c0.c;decomp:re/decomp/141484fb0.c;xref:0x142cfd842,0x142cfdbe6,0x142cfdc06,0x141484fcd,0x14148500e,0x1414850b6
};
