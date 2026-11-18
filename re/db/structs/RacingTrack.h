// struct: RacingTrack
// subsystem: live
// size: ?
// notes: The loaded track's descriptor at RacingOwner+0xe8 (stand-in name). The ERS reader (Car_Read_EnergyRecovery 0x1446cfcd0) passes name to Ne_Compare_StringNoCase against each TrackSettings block's track key and sets CarErs.track_settings_found on the match, so it is the track's internal key as lower-case text (monza, silverstone, ...).
struct RacingTrack {
    /* 0x0a0 */ char *name;                                  // L2 decomp:re/decomp/1446cfcd0.c;xref:0x1446d0649 NUL-terminated C string
};
