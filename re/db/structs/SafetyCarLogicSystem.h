// struct: SafetyCarLogicSystem
// subsystem: live
// size: ?
// notes: Registry key 0x146100bd8, type name "SafetyCarLogicSystem" (static init 0x143686a47). Sess_Set_SafetyCarDeploymentType writes state: 1 safety car, 2 virtual safety car, 0 none (a later SC upgrades a running VSC). No other value is stored by the handler; a formation lap is not represented here. The registry-key link to the object that function takes is not traced.
struct SafetyCarLogicSystem {
    /* 0x108 */ uint8_t deployment;                          // L2 decomp:re/decomp/143608160.c;xref:0x14360825c,0x143608271,0x14360827a see notes
};
