// struct: TyreSlot
// subsystem: live
// size: 0x18
// notes: Slot of VehicleTyreComponent.slots, written by the slot build (thunk 0x142e9e540 into .data1 body 0x14f463120, called from 0x142e9ebac): for each option 0..4 whose mask is non-empty it stores {option, &physical[idx], &visual[idx]} and bumps the count at +0x9c48. 0x1436ae1f0 copies the 0x18 bytes of a slot into the tyre info record, 0x142ea2a90 dereferences the +0x10 pointer's first byte as the visual compound id.
struct TyreSlot {
    /* 0x000 */ uint32_t option;                             // L2 decomp:re/decomp/142e9e540.c;decomp:re/decomp/142ea2a90.c;xref:0x142ea2b70 set option id, 0..4 (the index of the slot build's loop)
    /* 0x008 */ PhysicalCompound *physical;                  // L2 decomp:re/decomp/142e9e540.c;decomp:re/decomp/1436ad6c0.c;xref:0x1436ae1fa,0x142ea23cf pointer to the PhysicalCompound record, not an id
    /* 0x010 */ VisualCompound *visual;                      // L2 decomp:re/decomp/142e9e540.c;decomp:re/decomp/1436ad6c0.c;xref:0x1436ae1f0,0x142ea2b81 pointer to the VisualCompound record
};
