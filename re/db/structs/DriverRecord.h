// struct: DriverRecord
// subsystem: live
// size: ?
// notes: One loaded entrants.drivers row, as Ent_Get_Driver_Thunk (0x1423dbcf0) returns it from EntrantsSystem.driver_sets.
struct DriverRecord {
    /* 0x0c8 */ char *display_name;                          // L2 decomp:re/decomp/142f28c10.c;decomp:re/decomp/142f55040.c;xref:0x142f28d39,0x142f28d0c Sess_Get_CarDriverName returns [record+0xc8] as the car's driver name; Sess_Misconduct_OnCarToCarCollision prints it with %s ('Car To Car Collsision Between %s & %s')
};
