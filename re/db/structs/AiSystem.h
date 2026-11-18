// struct: AiSystem
// subsystem: ai
// size: 0x1a8  L2 decomp:re/decomp/1405ff750.c;xref:0x1405ff947 (the object Sess_Build_AiSystem builds, rbx at site_Ai_BuildSystem_RecordPushed)
// notes: only the per-vehicle record vector is mapped. begin/end are re-read by the evaluators on every call (0x140624ed8 reads [S+0xa8]).
struct AiSystem {
    /* 0x0a8 */ AiCarRecord *records_begin;   // L2 decomp:re/decomp/1405ff750.c;decomp:re/decomp/140624e60.c;xref:0x1405ffbe0,0x1405ffbe7,0x140624ed8 first record; the evaluator does r13=[S+0xa8] then r13+idx*0x508
    /* 0x0b0 */ AiCarRecord *records_end;     // L2 decomp:re/decomp/1405ff750.c;xref:0x1406004d2,0x140602b1a one past the last record; add [rbx+0xb0],0x508 after each push
};
