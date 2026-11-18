// struct: EngineSuppliersComponent
// subsystem: career
// size: ?
// notes: proposed by sponsor-studio (Sponsor Studio research); fields at L1, see re/research/sponsors.md.
struct EngineSuppliersComponent {
    /* 0xe30 */ void *enginesupplierhistory_begin;       // L1 doc:re/research/sponsors.md#career-sponsors 0x28-byte entries; +0 team key (byte kind, +2 u16 teamStatId; compare masked 0xffff00ff), +0x10/+0x18 history vector of 4-byte {u16 day, u8 engineStatId at +2} (entrants.engines.stat_id); current = newest day. Runtime: the Sponsor Base DLL read 'team engine 33' and filtering confirmed by owner. (seen in game in the contributor's runs)
    /* 0xe38 */ void *enginesupplierhistory_end;         // L1 doc:re/research/sponsors.md#career-sponsors  (seen in game in the contributor's runs)
};
