// struct: SponsorEntry
// subsystem: career
// size: 0x160 L2 decomp:re/decomp/141106870.c (sponsor vector stride 0x160)
// notes: one sponsor of a team in CareerSponsorsComponent. Raised by ce-econ (career lane owner career-re is stale; the lead authorised --force).
struct SponsorEntry {
    /* 0x010 */ const char * name;                   // L2 decomp:re/decomp/141106870.c;decomp:re/decomp/1410f4830.c;xref:0x141106af5 sponsor name C string (the SPONSORS loyalty log passes it as its sponsor argument)
    /* 0x028 */ uint16_t stat_id;                    // L2 decomp:re/decomp/141106870.c;xref:0x14110690e,0x141106920 sponsor stat id
    /* 0x098 */ SponsorLoyaltyLevel * levels_begin;  // L2 decomp:re/decomp/141106870.c;xref:0x141106938,0x141106a46 loyalty levels
    /* 0x0a0 */ SponsorLoyaltyLevel * levels_end;    // L2 decomp:re/decomp/141106870.c;xref:0x14110693f,0x141106a19 end of the loyalty levels
    /* 0x0b8 */ void *default_decals_begin;              // L1 doc:re/research/sponsors.md#career-sponsors Uncapped vector, 0x10 stride (loader 0x141114160; read by 0x141107420).
    /* 0x0c0 */ void *default_decals_end;                // L1 doc:re/research/sponsors.md#career-sponsors 
    /* 0x0d8 */ uint8_t slot;                        // L2 decomp:re/decomp/141106870.c;xref:0x1411068fc 0 for the loyalty-earning (primary) slot
    /* 0x0dc */ uint32_t group_id;                       // L1 doc:re/research/sponsors.md#career-sponsors Decal group id (catalogue searches item +0x38 == it); compared with vehicle customisation +0x78 in 0x141ff7610.
    /* 0x114 */ int32_t loyalty_xp;                  // L2 decomp:re/decomp/141106870.c;xref:0x14110696d,0x141106a3f loyalty XP, clamped to the top level
    /* 0x128 */ uint8_t seen;                            // L1 doc:re/research/sponsors.md#career-sponsors 0 after discover (like a reward discovery). (static L2 in the doc)
    /* 0x12a */ uint16_t discovered_day;                 // L1 doc:re/research/sponsors.md#career-sponsors Set to the current day by 0x1410f5590. (static L2 in the doc)
    /* 0x158 */ uint8_t active;                      // L2 decomp:re/decomp/141106870.c;xref:0x141106905 nonzero while the sponsor is active
};
