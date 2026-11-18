// struct: SessionEntrantView
// subsystem: live
// size: 0x88
// notes: The 0x88-byte element of EntrantsManager's session entrant vector [+0x2e0,+0x2e8), the same record as struct EntrantRecord (subsystem entrants) read at its runtime fields. team_stat_id is EntrantRecord.team_stat_id (kind in the low byte, index in the high word). vehicle_id is the participant index the session build loop writes (0x142588c24, verifier V8818 on EntrantRecord.unk_0x82), 0xff while the entrant is not in the session; Team_Set_TimingPillBadge (0x141760020) finds an entrant by it. badge_stat_id is EntrantRecord.badge_stat_id; the pill passes its address to Team_Format_RenderBadgePath (0x141cbba30) for a team record whose show_custom_badge_in_ui is set.
struct SessionEntrantView {
    /* 0x050 */ uint32_t driver_ref;                         // L2 decomp:re/decomp/142f28c10.c;decomp:re/decomp/142452e60.c;xref:0x142f28c9c,0x142f28d29,0x14c970d04,0x14c970d22 packed driver ref (EntrantRecord.id_lo/id_hi): byte 0 = domain (driver table 0..2), u16 at +2 = driver stat id; Sess_Get_CarDriverName passes entrant+0x50 to Ent_Get_Driver_Thunk
    /* 0x058 */ uint32_t team_stat_id;                       // L2 decomp:re/decomp/141760020.c;decomp:re/decomp/142452e60.c;xref:0x1417600a3,0x142453710 team word: byte 0 = team table (0..2), bits 16..31 = index into that table
    /* 0x068 */ uint32_t badge_stat_id;                      // L2 decomp:re/decomp/141760020.c;decomp:re/decomp/142452e60.c;xref:0x141760297,0x142453724 badge key the timing pill formats into the rendered badge texture path: byte 0 = badge table (0..2), bits 16..31 = index (0x3fff = none)
    /* 0x082 */ uint8_t vehicle_id;                          // L2 decomp:re/decomp/141760020.c;decomp:re/decomp/142586a60.c;xref:0x141760080,0x142588c24 the pill scans the vector for the entrant whose byte equals the pill's vehicle index
};
