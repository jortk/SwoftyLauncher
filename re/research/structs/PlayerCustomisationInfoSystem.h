// struct: PlayerCustomisationInfoSystem
// subsystem: game
// size: ?
// notes: proposed by sponsor-studio (Sponsor Studio research); fields at L1, see re/research/sponsors.md.
struct PlayerCustomisationInfoSystem {
    /* 0x128 */ uint64_t assigned_badge_customisation_count; // L1 doc:re/research/sponsors.md#badge  (static L2 in the doc)
    /* 0x280 */ void *entrants;                          // L1 doc:re/research/sponsors.md#badge Badge record store (L2 for the badge-deleted use). (static L2 in the doc)
};
