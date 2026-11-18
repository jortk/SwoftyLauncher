# Weather, tyre and strategy bindings for third-party addons (F1 25, build d873a1c8)

Agent `rr-bindings`, 2026-11-16. The items below are code
two published addons, Realistic Rain (`jonno.realistic-rain`) and Smarter AI Strategies (`jonno.smarter-ai-strategies`),
reached at fixed addresses or by their own byte scans because `bindings.json` did not list them. The Steam 1.0.145
update moved all of them (by 0, +0x40, +0x2590, +0x25d0, +0x2870 or +0x29c0 depending on the area), and the addons'
fixed addresses broke. Binding them lets the core resolve them per build.

**Evidence level: static, plus runtime behaviour in the two addons.** Every claim below was read from capstone
disassembly of the KB exe (`re/tools/disx.py`), with callers from rel32 scans of `.xtls`. Each item was also used by
the addons in real game sessions on the KB build, through inline hooks, byte patches or calls, and it behaved as
described below. That use was not logged with the trace plugin, so nothing here is L3.

## Status {#status}

| item | level | needs |
|---|---|---|
| `Sess_Build_VehicleTyreInfo` | L2 (engine-re) | verified here (agree), so it exports |
| `site_Sess_StintTyreCheckPredictCall` | L2 (session lane) | an agree row from another agent |
| `Render_SceneView_BuildViewParams`, `site_Render_MainViewParamsCall` | L1 | render owner: raise and verify (inbox) |
| `Car_Tyre_ChangeToNextCompound`, `Car_TyreSelection_SetCurrentTyres`, `site_Car_TyreAiMinGripClamp` | L1 | car owner: raise and verify (inbox) |
| `Game_TrackInformationSystem_Update` (sig), the four `site_Game_*` rows | L2 / L1 | game owner: sig column, raise and verify (inbox) |

The owned lanes (`car`, `game`, `render`) only take L2 from their owners (PROTOCOL.md §4), so their rows go in at L1 with the
evidence below and an inbox request.

## Functions {#functions}

- `Sess_Build_VehicleTyreInfo` 0x1436ad6c0 (`.pdata` 0x1436ad6c0-0x1436ae461, 15 callers; already L2). It returns a 32-byte
  record through the hidden pointer in rcx (rdi), initialised at 0x1436ad74b-0x1436ad75c: +0x00..+0x0f four broadcast
  floats, +0x10 0x7e967699, +0x18 = 0 (trend float) and +0x1c = 4 (class "none"). The class is then set at 0x1436ad96c
  (0), 0x1436ada7b (1) and 0x1436adaad (2). The other arguments: rdx = context (spilled to [rsp+0x28]), r8b = vehicle
  id (0xff = none, 0x1436ad764), r9d = flags (0x1436ad7aa), [rsp+0x20] = a pointer (callers pass 0x145859444) and
  [rsp+0x28] = float (read at 0x1436ad738 as [rsp+0x218]). `Sess_RaceStrategy_BuildRedFlagStints` copies the record
  whole to rsd+0x1b8 (0x1436a19f0, 0x1436a19f8), so its +0x18/+0x1c are the strategy record's +0x1d0/+0x1d4.
  `Sess_Init_TyreComponentSet` maps class 2 -> compound 4 and 1 -> 3 (0x142e93ae6-0x142e93b00) before queueing a set.
- `Render_SceneView_BuildViewParams` 0x143be0360 (`.pdata` 0x143be0360-0x143be17e6, 27 callers). rcx (rsi) is the view
  params block it fills and rdx (rdi) the source view. It writes the projection at +0x70..+0xa0 (0x143be0902 and the
  stores after it), the world-to-view rows at +0xb0..+0xe0 (0x143be0815, 0x143be082d) and camera-to-world at
  +0xf0..+0x120 (0x143be0855, 0x143be086d, 0x143be08ad). It ends with a second, equivalent pass at
  0x143be1629-0x143be1684. The main 3D view calls it at 0x1401583d0 (see sites).
- `Car_Tyre_ChangeToNextCompound` 0x142e94040 (`.pdata` 0x142e94040-0x142e946cf). Callers: 0x1403c8dd3 (in
  `Game_PitBoxCarActor_Update`, with edx = 0) and 0x1434bdd9f. rcx (rsi) = the car's tyre component, edx (ebx) = a bool.
  When the queued next set (+0x9c70, compound/group; +0x9c74 set byte) is not -1 (0x142e94240), it calls
  `Car_TyreSelection_SetCurrentTyres` (0x142e9425a) with rcx = component+0x9c60 (the selection) and rdx =
  component+0x9c70 (the request), then re-fits the tyres.
- `Car_TyreSelection_SetCurrentTyres` 0x142e9eda0 (`.pdata` 0x142e9eda0-0x142e9fe3f). Only caller: 0x142e9425a. It reads
  the fitted set as the last 0x78-byte entry of [selection+0x30, selection+0x38) (0x142e9ee13: end at +0x38; group at
  end-0x6c 0x142e9ee1b; set byte at end-0x68 0x142e9ee17), compares it with the request's group [rdx] (0x142e9ee1f)
  and set byte [rdx+4] (0x142e9ee22), and does nothing when they are equal (the pit stop that changes no tyre).
- `Game_TrackInformationSystem_Update` 0x144795cf0 (`.pdata` 0x144795cf0-0x144799ecc, vtable-only; already L2). It gets a
  `func_start` signature so it can be bound.

## Code sites {#sites}

- `site_Car_TyreAiMinGripClamp` 0x1446b7979, in `Car_Tyre_ComputeFfbGripFactor` (0x1446b76f0-0x1446b79b1).
  `cmp dword [rbx+0x124], 1` (the car's is-AI flag), then the tyre type's MinGripMultiplier `[rcx+0x24]` and max `[rcx+0x28]`,
  then `jne +8` (0x1446b798a) over `vmaxss xmm2, xmm2, [rip+..]` (the constant 0.6, 0x1446b798c). An AI car's grip floor is
  at least 0.6. Realistic Rain turns the jne into a jmp while an offline race runs, so AI cars can aquaplane.
- `site_Game_TrackInfoPuddleLevelStore` 0x144796141, in `Game_TrackInformationSystem_Update`. `vmovss [r14+0x3f0a54], xmm0`
  stores the puddle level from the curve call just before it (0x14479613c, `Game_TodCurveBinding_EvalAt`). r14 is
  the weather surface state loaded at 0x144795d60 from `g_Game_WeatherSurfaceState`. Realistic Rain jumps over this
  9-byte store (EB 07) while it drives the level.
- `site_Render_MainViewParamsCall` 0x1401583d0, in the function at 0x140157d90-0x140158a63. This is the main 3D view's call
  to `Render_SceneView_BuildViewParams`; the code after it reads the result for the scene. Realistic Rain checks a hook's
  return address against it to keep only the main camera.
- `site_Game_PitBoxTyreComponentPath` 0x1403c8da0, in `Game_PitBoxCarActor_Update` (0x1403c8cf0-0x1403c9bee). This is the
  load of `g_Game_Manager` that starts the path to a car's tyre component: [[g_Game_Manager]+0x278]+0x110/+0x118 is a
  vector of tyre components indexed by vehicle (0x1403c8da7-0x1403c8dc8). The component goes to
  `Car_Tyre_ChangeToNextCompound` at 0x1403c8dd3.
- `site_Game_CutSceneDriverLookupCall` 0x14022da92, in `Game_CutSceneParams_SetupDataFromEntrant` (0x14022d920-0x14022fb63).
  The call to `Ent_Get_Driver_Thunk` (the driver lookup: ecx = driver id [rdi+0x50] at 0x14022da82, db = [g_Game_Manager]+0x290
  at 0x14022da88); the record's name is the char* at +0xc8. A call site rather than the thunk: the thunk's bytes after
  its jmp are build-specific filler.
- `site_Game_CutsceneEntrantListRead` 0x14022d48d, in `Game_CutsceneParams_SetupDataFromTeamStatIdAndDriverIndex`
  (0x14022d400-0x14022d665). The load of `g_Game_Manager` that starts the entrant list path [[[g_Game_Manager]+0xc0]+0x88]+0x260
  -> +0x2e0/+0x2e8 (0x14022d494, 0x14022d49b, 0x14022d4ab, 0x14022d4c0): 0x88-byte entrant records.
- `site_Sess_StintTyreCheckPredictCall` 0x1436a3b83, in the function at 0x1436a3ab0-0x1436a422c (a tyre-choice helper of
  `Sess_RaceStrategy_EvaluateStintTyre`). The call to `Game_Predict_StrategyWetness`; the next instruction (0x1436a3b88,
  `vsubss xmm7, xmm0, xmm6`) subtracts a start value it read itself from the game's curve (0x1436a3b74). Realistic Rain
  tells this caller from the others by its return address.

## Signatures {#signatures}

All are unique in `.xtls` on the KB build (`sigcheck.py`). The call sites' `expect` is just `E8`, and the RIP-relative
loads' is `48 8B 05`, so a moved target does not break them. On Steam 1.0.145 (sha256 46009bd3) every one matches once
in the overlay's code section (`.tls$`), at the moved address of the same code: the AI clamp at rva 0x46ba1e9, the puddle
store 0x4798b01, the main view call 0x1583d0, the pit-box path 0x3c8de0, the driver lookup call 0x22da92, the entrant read
0x22d48d and the forecast call 0x36a6113. The tyre-info signature's call there reaches 0x36afc50.
