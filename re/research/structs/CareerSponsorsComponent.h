// struct: CareerSponsorsComponent
// subsystem: career
// size: ?
// notes: CareerSponsorsComponent (vtable 0x1456be490, My Team only). Raised by ce-econ (career lane owner career-re is stale; the lead authorised --force).
struct CareerSponsorsComponent {
    /* 0x270 */ void *career_state;                      // L1 doc:re/research/sponsors.md#career-sponsors [c+0x270]->vf 0xf8 == 0 means new career (0x141100a80). (static L2 in the doc)
    /* 0x298 */ SponsorTeamRecord * teams_begin;     // L2 decomp:re/decomp/141106870.c;xref:0x141106883 per-team sponsor records
    /* 0x2a0 */ SponsorTeamRecord * teams_end;       // L2 decomp:re/decomp/141106870.c;xref:0x14110688a end of the team records
    /* 0x390 */ void *add_arg_390;                       // L1 doc:re/research/sponsors.md#career-sponsors Passed to 0x1410f5590. (static L2 in the doc)
    /* 0x398 */ void *add_arg_398;                       // L1 doc:re/research/sponsors.md#career-sponsors Passed to 0x1410f5590. (static L2 in the doc)
    /* 0x3b0 */ void *clock;                             // L1 doc:re/research/sponsors.md#career-sponsors Day = ([c+0x3b0]+0x2b0)/day in discover. (static L2 in the doc)
    /* 0x3c0 */ void *add_arg_3c0;                       // L1 doc:re/research/sponsors.md#career-sponsors Passed to 0x1410f5590; [c+0x3c0]+0x2c8 must be readable (checked by the Sponsor Base DLL before calling). (static L2 in the doc)
    /* 0x3c8 */ void *add_arg_3c8;                       // L1 doc:re/research/sponsors.md#career-sponsors Passed to 0x1410f5590. (static L2 in the doc)
    /* 0x3e0 */ void *add_arg_3e0;                       // L1 doc:re/research/sponsors.md#career-sponsors Passed to 0x1410f5590. (static L2 in the doc)
    /* 0x4e0 */ void *starting_begin;                    // L1 doc:re/research/sponsors.md#career-sponsors 0x28 stride, code char* at +0x10. (static L2 in the doc)
    /* 0x4e8 */ void *starting_end;                      // L1 doc:re/research/sponsors.md#career-sponsors  (static L2 in the doc)
    /* 0x500 */ SponsorEntry **sponsor_defs_begin;       // L1 doc:re/research/sponsors.md#career-sponsors Pointer list (8-byte stride) of sponsor definitions (def +0x10 code char*, +0x28 id); also read by catalogue 0x141fe5a10. (static L2 in the doc)
    /* 0x508 */ SponsorEntry **sponsor_defs_end;         // L1 doc:re/research/sponsors.md#career-sponsors  (static L2 in the doc)
};
