// struct: MarshalZoneMonitor
// subsystem: live
// size: ?
// notes: Registry key 0x1460fdde0, type name "MarshalZoneMonitor" (static init 0x1435f7042). Linked into the UDP state at +0xd290 through 0x141489380. Zones in [+0x88,+0x90), stride 0x10 {?, MarshalZone *}.
struct MarshalZoneMonitor {
    /* 0x088 */ MarshalZoneSlot * zones_begin;               // L2 decomp:re/decomp/142cfaec0.c;xref:0x142cfb004,0x142cfb2e1,0x142cfb3be stride 0x10
    /* 0x090 */ MarshalZoneSlot * zones_end;                 // L2 decomp:re/decomp/142cfaec0.c;xref:0x142cfb718,0x142cfb734,0x142cfb786
};
