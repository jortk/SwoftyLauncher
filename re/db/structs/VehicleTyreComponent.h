// struct: VehicleTyreComponent
// subsystem: live
// size: ?
// notes: Per-vehicle tyre component (TyreSystem +0x110 vector, Halo V). Slot table and fitted set as read by Sess_Tyre_ShowFittedSet. Compound visual ids: Halo reads the visual record behind the slot (+0x10).
struct VehicleTyreComponent {
    /* 0x9bc0 */ void * compound_class;                      // L2 decomp:re/decomp/142ea2a90.c;xref:0x142ea2acb,0x142ea2add,0x142ea2b85 the cached class the slot table was built for
    /* 0x9bd0 */ TyreSlot slots[5];                          // L2 decomp:re/decomp/142ea2a90.c;xref:0x142ea2aba stride 0x18: u32 option, +8 physical, +0x10 visual compound record
    /* 0x9c48 */ uint64_t slot_count;                        // L2 decomp:re/decomp/142ea2a90.c;xref:0x142ea2aec,0x142e9ebb1 at most 5; read and written as a qword (osd-vf V8955-V8962)
    /* 0x9c50 */ void * built_for;                           // L2 decomp:re/decomp/142ea2a90.c;xref:0x142ea2ad2
    /* 0x9c58 */ void * built;                               // L2 decomp:re/decomp/142ea2a90.c;xref:0x142ea2ac1,0x142e9ebce a pointer (stored by the rebuild tail, 0 when the class is null); 0 means the table is rebuilt first (0x142e9e6c0 per the caller)
    /* 0x9c90 */ uint8_t * set_begin;                        // L2 decomp:re/decomp/142ea2a90.c;xref:0x142e94c13,0x142e901b9,0x142d00d16 0x78-byte set records (span / 0x78 at 0x142e94c13; agreed osd-vf V8959)
    /* 0x9c98 */ uint8_t * set_end;                          // L2 decomp:re/decomp/142ea2a90.c;xref:0x142ea2af3 the fitted set option is the dword at set_end - 0x6c
};
