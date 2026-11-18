// struct: RacingVehicles
// subsystem: live
// size: ?
// notes: The racing-vehicles module singleton (vtable 0x145bf7cf0, constructor 0x14463a1c0 stores `this` into the global 0x146ce72b8 at 0x14463a7b3; module init Game_RacingVehicles_Initialise 0x14463adc0). Its +0xf0 is the per-vehicle car array (stride 0x290) read by the vehicle code at 0x142f4866d and 0x142f4f564; +0xd0 reaches the track name the ERS reader uses.
struct RacingVehicles {
    /* 0x0d0 */ RacingOwner *owner;                          // L2 decomp:re/decomp/1446cfcd0.c;xref:0x1446d0632 first hop of the track name chain
};
