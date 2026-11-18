// struct: RivalManagerComponent
// subsystem: career
// size: ?
// notes: game::championship::RivalManagerComponent (vtable 0x1456cd070, Driver Career). Save serialiser 0x14125b1b0 (slot 2; strings rival_managers, current_rivalries). Raised by bind552 (the career lane owner career-re is stale; the lead authorised --force).
struct RivalManagerComponent {
    /* 0xbd8 */ void * managers_begin;             // L2 decomp:own capstone disasm 0x1415caf50-0x1415cbd16;decomp:own capstone disasm 0x14125b1b0-0x14125c4c6;xref:0x1415cbaa5,0x14125b2c1,0x140a7c4cc begin of the rival managers vector: 16-byte entries, the RivalManager pointer at +8 (0x1415cbad5, 0x140a7c4f9)
    /* 0xbe0 */ void * managers_end;               // L2 decomp:own capstone disasm 0x1415caf50-0x1415cbd16;decomp:own capstone disasm 0x14125b1b0-0x14125c4c6;xref:0x1415cbaac,0x14125b2c8,0x140a7c4d3 end of the rival managers vector (the loops compare the cursor to it)
};
