# Live State: Steam build 26566308

## Scope and status

Candidate support for F1 25 1.0.145.2472, Steam build 26566308:

- PE timestamp: `1794312636`; image base: `0x140000000`.
- File size: `391613312`; SHA256: `46009bd31f49d93a92d73cbd0b66131d2b37964a92da7293168d3c3a872d9389`.
- Existing address overlay: `re/db/builds/steam-26566308.json` (including its section-name mapping).

The candidate enables all 21 existing groups. Live table v2, the SDK and every public callback remain unchanged. Existing KB/EA descriptors retain their prior layouts. Current progress-from-manager is +0x2d0; ERS mode moved to +0x1a6c from the older +0x1a68. No new functions or team-name API are included.

Two independent static passes checked current consumers/writers. This supports a compatibility test candidate, not a blanket current-build L3 claim. The original five-group candidate has scoped runtime observations; T154 tests the broader existing API and remains pending.

## Current instruction evidence

Addresses below are VAs in the exact executable identified above. The old KB addresses are not substituted for
current addresses. The UDP packet builders are static evidence of field meaning; Live State does not run those
builders or receive UDP packets.

### Common vehicle and registry chain

`0x14462aa00` constructs the car: `0x14462adb6/0x14462adbd` set its lock at +0x98 to
`car+0xa0`; the systems vtable is stored at +0x380 at `0x14462afea`. The vehicle byte +0xe8, count increment and
vehicle-table insertion are at `0x14462beae`, `0x14462beb7`, and `0x14462bec4`. The zeroing stores at
`0x14462ade8`, `0x14462ae7d` and `0x14462aedb` cover AI +0x124, input fields +0x188/+0x18c/+0x190 and flags +0x224.

System construction at `0x14473eca0` writes engine +0x530 (`0x14473ed2e`), gearbox +0x538 (`0x14473edae`),
chassis +0x558 (`0x14473efae`), steering +0x560 (`0x14473f02e`), aero +0x570 (`0x14473f13e`) and ERS +0x5a8
(`0x14473f594`). A component owner at +8 is independently written at `0x14474aee8` in the engine factory. These
identity/ownership checks remain active for all component-value reads.

The tyre table builder at `0x142ea0c50` independently demonstrates the registry lookup: game manager's map +0x30
at `0x142ea0c6d`, slots +0x18 and metadata/end +0x20 at `0x142ea0c8e/92`, 16-byte key/value slots and value +8
at `0x142ea0c8a/98`. A cached slot still has to match its key, value and occupied metadata before the reader reuses it.

### Classification

Current `Game_TelemetryUdp_SyncLapData` is `0x142cffc50`. It reads the SessionSystem pointer vector
+0x148/+0x150 at `0x142cffd27/2e`, indexed by vehicle ID with eight-byte stride. Entry status byte +0x18 is read
at `0x142cffd7b`. `Sess_Issue_TimePenalty` (`0x1435760d0`) independently indexes that vector and decodes the
double XOR pair +0x38/+0x40 at `0x1435761a4..1af`, then writes the updated encoded penalty at `0x1435761c5`.

The current HUD consumer reads position words +0x48/+0x50, XORs them and adds one at
`0x141486021/024/027`. Entry constructor `0x142c7bed0` writes vehicle byte zero at `0x142c7bee4`, status +0x18 at
`0x142c7bf7f`, penalty operands at `0x142c7c079/07d`, and position operands at `0x142c7c0f7/0fb`. Vector allocation
stores +0x148/+0x150 at `0x142cf05f3/5fa`; `0x142cf0643/64e` uses entry.vehicle to store the pointer in its slot.

### Controllers

`Sess_ControllerSystem_ApplyInputs` (`0x14389c6b0`) checks built byte +0x365 at `0x14389c6d3`, then reads
count owner +0x90 and its low32 vector-pointer difference +0x30 minus +0x28, shifted by three, at
`0x14389c6fb..708`. Sets +0x30 is read at `0x14389c740..758`. The existing reader follows that count calculation.

`Sess_ControllerSystem_BuildSets` (`0x14389e1e0`) appends to sets +0x30/+0x38/+0x40 at
`0x14389e3e1..483` and sets built +0x365 at `0x14389e667`. Set constructor `0x143898e70` stores vehicle +0x34
at `0x143898f1c`, constructs the human object's vtable at `0x143898f8b/92` and stores it into +0x20 at
`0x143898ffa`. Input dispatch independently reads +0x20 at `0x14389c767`. Controller construction initializes
the sets vector at `0x14389a580/584`; initialization writes count owner +0x90 at `0x14389be03`.

### Pit and pit lane

The lap-data builder guards vehicle ID <=26 at `0x142d0083e/842`, shifts it by four at `0x142d0084e`, and reads
PitLaneSystem's state pointer +0x68 at `0x142d00852`, lane +0x48 at `0x142d00857` and lane slot +0x938 at
`0x142d0085b`. State +4 is compared with four at `0x142d00863`. The flag byte +0x103 at `0x142d0086c` determines
actual lane membership: outside gives zero; inside gives two for state four, otherwise one. Stop count +0x14 is
read at `0x142d0088e`.

`Game_Pitlane_SetVehicleStatus` (`0x1434beac0`) uses the same chain, writes state +4 at `0x1434bec2c` and increments
stops +0x14 at `0x1434bef76`. Movement code `0x1434ba960` independently reads +0x103 at `0x1434bab20/bae47`
and writes membership at `0x1434bb761`. The existing lane array bound and separate phase/membership meanings are kept.

### Tyres

`Game_TyreSystem_GetCompounds` (`0x142e99c00`) reads vector +0x110/+0x118 at `0x142e99c10/17`, bounds the
vehicle ID and indexes pointers at `0x142e99c2a/33`. Descriptor helper `0x142e99d90` checks built +0x9c58,
class +0x9bc0 and built-for +0x9c50 at `0x142e99dd7..de8`. It reads count +0x9c48 and fitted set end +0x9c98,
then option `end-0x6c` at `0x142e99e06..e19`. The slot table is +0x9bd0 with 0x18-byte stride. Independent fitted-set
consumer `0x142ea5020` follows this chain and reads descriptor visual +0x10 and stat byte zero at `0x142ea5111/11c`.

The table builder `0x142ea0c50` receives `component+0x9bd0`, writes relative count +0x78, built-for +0x80 and
built +0x88 at `0x142ea1141/114e/115e`. Its slot-builder thunk `0x142ea0ad0` jumps to `0x14f069980`, which stores
option, physical pointer +8 and visual pointer +0x10 at `0x14f0699b6/9ba/9bf` and increments count at
`0x14f0699c4`. Component constructor `0x14f178ae0` initializes set begin/end +0x9c90/+0x9c98 at
`0x14f178cf7/cfe`; it appends this component to TyreSystem's vector at `0x14f178e0e/e15/e25/e28`.

VisualCompound loader `0x142e9ab70` writes audio enum +0x50 at `0x142e9aca0` and initializes stat byte zero at
`0x142e9ad30`; physical loader `0x142e9a4b0` initializes stat byte zero at `0x142e9a651`. The current audio parser
maps Soft/Medium/Hard to 3/4/5, SuperHard to 6, Intermediate/Wet to 7/8 and absent/unknown labels to -1. These are
the representations the existing reader uses; no new compound mapping is added.

## Remaining existing groups

Current-executable evidence (preferred-image VAs):

- Car systems/world/aero: engine/gearbox/component construction above; pedal writes at `0x14389d987`, `0x14389d9b6`, `0x14389dbe6`; steering XOR fields at `0x1446aa140`; world position/velocity writes at `0x1440c3dce`/`0x1440c467b`; chassis acceleration writes at `0x1447436f0`/`0x144743709`.
- ERS: mode reads at `0x1446e50ce` and `0x1446e53aa`, reset store at `0x154bab7fc`, all at +0x1a6c. Four mode slots are loaded at `0x1446d2d2c`, `0x1446d2d67`, `0x1446d2da2`, `0x1446d2ddd`; retain the existing 0..3 range. Energy/lap harvest/deploy and power stores at `0x1446e6f99`, `0x1446e703e`, `0x1446e704e`, `0x1446e707a`, `0x1446e7086`, `0x1446e7097` match existing offsets.
- Progress: +0x2d0 manager access at `0x1403d4b63`/`0x1403d86ab` feeds `0x144606f10`; XOR distance fields +0x38/+0x40, +0x48/+0x50, signed delta +0x58 and total +0x98/+0xa0 match. Track length/sector globals are written at `0x154827063`, `0x15482708b`, `0x1548270af`.
- Timing/track: current lap packet builder `0x142cffc50` confirms gap helper +0x30/+0x38, stride +0x98, vehicle +0x90 and valid-tagged i64 millisecond gaps; safety-state setter `0x14360a6f0`; track-name chain is consumed at `0x1446d2e9b..0x1446d2eb9`.
- Flags/penalties/bus: marshal flags remain dword-backed (clear at `0x143580181`, stores at `0x1435801a7`/`0x143580285`). Existing bus function `0x143989da0` captures delegate/message/bus at `0x143989dc5..0x143989dcb` and reads delegate function/context at `0x143989e06`/`0x143989e09`. Its real PE exception entry is `0x143989da0..0x14398a8da`; sigcheck must follow the exception directory rather than shuffled section names.
- Spectator: UI flag +0xd0 store `0x14eeb7b8b` (thunk `0x142e1b2a0`), active +0xd1 `0x142e1ac23`, driver +0xd6 `0x142e1c2f1`, gap mode +0xe4 `0x142e19fdb`. Current initializer uses 1 for types15..17 and 3 otherwise; the existing raw reader preserves this.
- Mode/badges: mode context +0x88 store `0x1407d08d7`, label pointer +0x278 store `0x1407cbceb`. Existing badge consumer `0x1417617a0` corroborates entrant/table/packed-team-reference chain; custom branch `0x141761a0c` and key load `0x141761a8b`. Existing MyTeam/custom unavailability is preserved.
- Driver names: current getter `0x142f2b1a0` corroborates participant vector +0x88/+0x90, physics +0x18 -> car +0x58 -> id +0xe8, override flag +0x32 bit7, entrant +0x50, table +0x268 and name +0xc8. Name pointer writer `0x14c80a5f2`. This override is not player identity. Only the current build receives the missing-generated-field proof; mapped/site checks remain required.

## Automated checks and runtime follow-up

All 26 existing bindings pass checked overlay hints, unique scans and signature checks. Native live/gate/SDK tests cover all groups, missing dependencies, current and old ERS offsets, compounds and inactive-session names/badges. A synthetic PE regression checks shuffled section names. The complete core builds.

T153 recorded the earlier five-group run: P17/soft, two stops including user-confirmed hard tyres, pit transitions/counts, pause/resume, menu clearing and fresh session data. T154 uses an API-only consumer for all nine existing callbacks and remains pending. Static field corroboration is not proof of every runtime semantic, registry identity, enum value or value on every game mode. No blanket L3 promotion, shipping completion or performance improvement is claimed.
