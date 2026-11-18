// struct: VehicleCustomisation
// subsystem: entrants
// size: ?
// notes: proposed by sponsor-studio (Sponsor Studio research); fields at L1, see re/research/sponsors.md.
struct VehicleCustomisation {
    /* 0x078 */ uint32_t bound_sponsor_ref;              // L1 doc:re/research/sponsors.md#decal-system -1 = not title-bound; compared with the active commercial record +0xdc by 0x141ff7610. Meaning (livery-bound sponsor reference) is a candidate.
    /* 0x0e8 */ void *decal_nodes_begin;                 // L1 doc:re/research/sponsors.md#decal-system Saved decal node list (16-byte entries, same node layout); counted by income 0x141104190 branch B. (static L2 in the doc)
    /* 0x0f0 */ void *decal_nodes_end;                   // L1 doc:re/research/sponsors.md#decal-system  (static L2 in the doc)
};
