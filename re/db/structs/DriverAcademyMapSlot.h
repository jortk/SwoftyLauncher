// struct: DriverAcademyMapSlot
// subsystem: career
// size: ?
// notes: result slot of Career_DriverAcademies_MapFindOrAdd (rdx): the entry index and a status. Raised by bind552 (the career lane owner career-re is stale; the lead authorised --force).
struct DriverAcademyMapSlot {
    /* 0x0 */ uint64_t index;                       // L2 decomp:own capstone disasm 0x140e091e0-0x140e097a1;decomp:own capstone disasm 0x140d6e840-0x140d6efd7;xref:0x140e095b7,0x140d6ee57,0x140d6ee5c entry index of the key in the map's entries array (u64 stored at [rdx], used by the caller as entries + index * 16)
    /* 0x8 */ uint32_t status;                      // L2 decomp:own capstone disasm 0x140e091e0-0x140e097a1;decomp:own capstone disasm 0x140d6e840-0x140d6efd7;xref:0x140e095ba,0x140d6ee3e,0x140d6ee46,0x140d6ee4b status stored at [rdx+8]: the caller reads it after the call and branches on 0, 3 and 2 (0x140d6ee3e-0x140d6ee4e)
};
