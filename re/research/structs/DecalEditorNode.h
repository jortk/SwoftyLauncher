// struct: DecalEditorNode
// subsystem: game
// size: 0x70 L1 doc:re/research/sponsors.md#decal-editor
// notes: proposed by sponsor-studio (Sponsor Studio research); fields at L1, see re/research/sponsors.md.
struct DecalEditorNode {
    /* 0x000 */ void **vtable;                           // L1 doc:re/research/sponsors.md#decal-editor 0x1456e3240.
    /* 0x008 */ void *prefix_ptr;                        // L1 doc:re/research/sponsors.md#decal-editor 0x145613520 in legacy entries sharing the node prefix; meaning unknown.
    /* 0x010 */ uint32_t item_key;                       // L1 doc:re/research/sponsors.md#decal-editor (customisation.custom_decal_sets.decal_sets.DecalSet.id << 16) | 9 (595 monomic_n, 301 euterpe_n, 661 techpace_n). Type 9 here is NOT UnlockableItemKey type 9 (eAvatarPose). (seen in game in the contributor's runs)
    /* 0x018 */ uint32_t colour;                         // L1 doc:re/research/sponsors.md#decal-editor Per-decal RGBA8 colour slot (R low byte), default 0xffffffff on every observed node; serialised, networked, copied to partner and on swaps; nothing in the shipped game sets it. Whether rendering uses it = T016 (pending). (seen in game in the contributor's runs)
    /* 0x020 */ uint64_t unique_id;                      // L1 doc:re/research/sponsors.md#decal-editor uniqueId; -1 in an add template, game allocates max+1. (seen in game in the contributor's runs)
    /* 0x028 */ uint32_t projection_area_id;             // L1 doc:re/research/sponsors.md#decal-editor ProjectionArea.id (2025 ids 0..20 with gaps, 2026 ids 21..38; id 0 is a real area). (seen in game in the contributor's runs)
    /* 0x030 */ float translation[3];                    // L1 doc:re/research/sponsors.md#decal-editor Translation (JSON translation[x,y,z,1]). (seen in game in the contributor's runs)
    /* 0x040 */ float rotation_angle;                    // L1 doc:re/research/sponsors.md#decal-editor rotationAngle (captured by the Sponsor Base DLL, e.g. +-0.088). (seen in game in the contributor's runs)
    /* 0x050 */ float scale[3];                          // L1 doc:re/research/sponsors.md#decal-editor Scale x/y/z; x (+0x50) and z (+0x58) clamped by the editor scale step, sign = flip. (seen in game in the contributor's runs)
    /* 0x060 */ uint64_t unknown_60;                     // L1 doc:re/research/sponsors.md#decal-editor 0 in every add template. (seen in game in the contributor's runs)
    /* 0x068 */ uint64_t partner_unique_id;              // L1 doc:re/research/sponsors.md#decal-editor Mirror partner uniqueId (-1 none); remove/update also act on the partner. (seen in game in the contributor's runs)
};
