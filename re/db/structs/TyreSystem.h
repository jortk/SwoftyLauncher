// struct: TyreSystem
// subsystem: live
// size: ?
// notes: Registry key 0x146080d28 (vtable 0x1457ef110, slot 0 0x142ed1b10 returns the key; the module slots 0x142e91cb0 OnSessionEnter and 0x142e92f70 walk the same vector). +0x110/+0x118 is the per-vehicle tyre component vector, one 8-byte pointer per vehicle id (null for an id without a component).
struct TyreSystem {
    /* 0x110 */ VehicleTyreComponent **vehicles_begin;       // L2 decomp:re/decomp/142e97670.c;decomp:re/decomp/142e946d0.c;decomp:re/decomp/142e91cb0.c;xref:0x142e97680,0x142e946d9,0x142e91cbf TyreSystem::getCompounds(vehicle) and pitstopComplete(carIdx) bounds-check the id against the element count and index this vector by it
    /* 0x118 */ VehicleTyreComponent **vehicles_end;         // L2 decomp:re/decomp/142e97670.c;decomp:re/decomp/142e946d0.c;xref:0x142e97687,0x142e946e0 count = (end - begin) >> 3
};
