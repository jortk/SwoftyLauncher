// struct: RelativeGapHelper
// subsystem: live
// size: ?
// notes: Registry key 0x14604ea90, type name "RelativeGapHelper" (static init 0x14097da85): the race gap/interval system, Halo's timing system T. Updated by 0x140813ee0. Linked into the UDP state at +0xd2e8 through Live_Get_RelativeGapHelper.
struct RelativeGapHelper {
    /* 0x030 */ RelativeGapEntry * entries_begin;            // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd71e,0x142cfdbc2,0x142cfe22f 0x98-byte entries; first entry with the vehicle id wins
    /* 0x038 */ RelativeGapEntry * entries_end;              // L2 decomp:re/decomp/142cfd6c0.c;xref:0x142cfd824,0x142cfdbc6,0x142cfdc00
    /* 0x0d0 */ PitLaneSystem * pit_lane;                    // L2 decomp:re/decomp/140813ee0.c;xref:0x140814099,0x1408146de the gap rule skips cars whose pit state is 2..7 or 14..15
};
