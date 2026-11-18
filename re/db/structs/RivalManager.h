// struct: RivalManager
// subsystem: career
// size: ?
// notes: one entry of RivalManagerComponent.managers (the pointer at entry +8). Raised by bind552 (the career lane owner career-re is stale; the lead authorised --force).
struct RivalManager {
    /* 0x18 */ void * current_rivalries_begin;     // L2 decomp:own capstone disasm 0x140a7c2b0-0x140a7c63b;xref:0x140a7c516,0x140a7c50d begin of the current rivalries vector (serialised as current_rivalries by 0x14125b1b0): the manager is the pointer at managers entry +8; pointer elements, each element's byte at +0x10 is read at 0x140a7c526
    /* 0x20 */ void * current_rivalries_end;       // L2 decomp:own capstone disasm 0x140a7c2b0-0x140a7c63b;xref:0x140a7c51a,0x140a7c51e end of the current rivalries vector (the cursor is compared to it at 0x140a7c51e)
};
