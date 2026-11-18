// struct: SponsorLoyaltyLevel
// subsystem: career
// size: 0x78 L2 decomp:re/decomp/141106870.c (level vector stride 0x78)
// notes: one loyalty level of a sponsor. Raised by ce-econ (career lane owner career-re is stale; the lead authorised --force).
struct SponsorLoyaltyLevel {
    /* 0x000 */ uint8_t claimed;                     // L2 decomp:re/decomp/141106870.c;xref:0x141106a67 0 until the level's reward is claimed
    /* 0x004 */ int32_t xp_threshold;                // L2 decomp:re/decomp/141106870.c;xref:0x141106a59 loyalty XP needed; -1 = none
    /* 0x010 */ uint32_t decal_id;                       // L1 doc:re/research/sponsors.md#career-sponsors ONE decal ref per level (a comma list is one unknown name -> loader crash).
    /* 0x018 */ uint8_t decal_present;                   // L1 doc:re/research/sponsors.md#career-sponsors 
};
