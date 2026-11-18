// struct: VehicleHelperSystem
// subsystem: live
// size: ?
// notes: Registry key 0x146096ac8, type name "VehicleHelperSystem". The rule manager keeps it at +0x1c30 as the participants object. Halo's "session component S1". Misconduct objects hang off it.
struct VehicleHelperSystem {
    /* 0x060 */ char *player_name;                           // L2 decomp:re/decomp/142f28c10.c;xref:0x142f28d08,0x142f28d0c Sess_Get_CarDriverName returns [this+0x60] for a car whose participant driver_kind is negative (a human)
    /* 0x088 */ VehicleHelperParticipant **participants_begin; // L2 decomp:re/decomp/142f28c10.c;xref:0x142f28c29 pointer vector; the car index must be below (end - begin) >> 3 (0x142f28c37..0x142f28c44), entries may be null (0x142f28cd1)
    /* 0x090 */ VehicleHelperParticipant **participants_end; // L2 decomp:re/decomp/142f28c10.c;xref:0x142f28c30
    /* 0x568 */ DriveThroughObject * drive_through;          // L2 decomp:re/decomp/142cf5c00.c;xref:0x142cf60a1 UDP state +0xd2e0 = [this+0x568]
    /* 0x578 */ StopGoObject * stop_go;                      // L2 decomp:re/decomp/142cf5c00.c;xref:0x142cf6093 UDP state +0xd2d8 = [this+0x578]
};
