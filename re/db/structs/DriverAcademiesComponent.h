// struct: DriverAcademiesComponent
// subsystem: career
// size: ?
// notes: game::championship::DriverAcademiesComponent (vtable 0x14568fd90, Driver Career). Save loader 0x140d6e840 (vtable slot 3; strings assignedAcademies, academId) and destructor 0x140dffbe0 (slot 0). Raised by bind552 (the career lane owner career-re is stale; the lead authorised --force).
struct DriverAcademiesComponent {
    /* 0x298 */ void * academies_begin;            // L2 decomp:own capstone disasm 0x140dffbe0-0x140dffd5e;decomp:own capstone disasm 0x140d6e840-0x140d6efd7;xref:0x140dffc47,0x140d6ec88 begin of the academies vector: 16-byte entries, the academy record pointer at +8 (u16 academy id at record +0, 0x140d6ecb0-0x140d6ecb8)
    /* 0x2a0 */ void * academies_end;              // L2 decomp:own capstone disasm 0x140dffbe0-0x140dffd5e;decomp:own capstone disasm 0x140d6e840-0x140d6efd7;xref:0x140dffc4e,0x140d6ec8e end of the academies vector (the loop compares the cursor to it, the destructor rewrites it at 0x140dffcc4)
    /* 0x2c8 */ void * driver_academy_entries;     // L2 decomp:own capstone disasm 0x140d6e840-0x140d6efd7;xref:0x140d6ee50,0x140d6ee85,0x140d6eea2 entries array of the driver id -> academy map whose header is at +0x2b0 (Career_DriverAcademies_MapFindOrAdd): 16-byte entries, u16 key at +0, record pointer at +8, written at 0x140d6ee65 and 0x140d6ee69
};
