// struct: SpectatorDirectorGap
// subsystem: live
// size: ?
// notes: A second view of the SpectatorDirector object (struct SpectatorDirector, subsystem session) for the one field the session lane's file does not declare. Same registry key 0x146080ca8. Initialised by the director's init function 0x142e16eb0 (vtable slot, subscribes MessageVehicleHidden, RedFlagReturnToGrid, MessageRaceStarted) to 1 for session types 15..17 and 2 otherwise; the spectator HUD refresh 0x141c24c80 turns it into a view-model bool (!= 0). The gap-mode UI value.
struct SpectatorDirectorGap {
    /* 0x0e4 */ uint32_t gap_mode;                           // L2 decomp:re/decomp/142e16eb0.c;decomp:re/decomp/141c24c80.c;xref:0x142e17a4b,0x141c24eaf written once at init and read by the HUD refresh
};
