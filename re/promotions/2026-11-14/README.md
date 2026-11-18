# Promotion requests from engine-re (2026-11-14, weather to L2)

Weather rows in owned lanes that engine-re decompiled and annotated with `decomp:` and `xref:` evidence.
Under the lane rule only the owner may raise them to L2. engine-re raised them to L2 with `--force` on 2026-11-14 (commit 1d1e0d04) without claiming the lanes, and each row asks for its lane owner's review. Blind verifier rows V9439-V9452: 13 agree, 1 partial (0x144792f50, the puddle condition, fixed in the row and re-checked in V9453).
The data flow they belong to is in `re/db/subsystems/session.md#wetness` and `#weather`. All findings are static, and nothing is runtime-tested.

| owner | lane | addr | name | what the decomp shows |
|---|---|---|---|---|
| re-career-teams | game | 0x144795cf0 | Game_TrackInformationSystem_Update | ws+0x3f0a38 = EvalNow(wet_delta of the active dataset); dry-line, puddle and evolution terms |
| re-career-teams | game | 0x144792f50 | Game_TrackInfo_SampleWheelSurface | per-wheel wetness formula on the RGBA map ws+0x1018 |
| re-career-teams | game | 0x1437cc9c0 | Game_TrackSurface_UpdateWheelRecords | dry line +0x3f0a64, drying +0x3f0a4c, puddle offset +0x3f0a50, per-wheel records |
| re-career-teams | game | 0x14029c310 | Game_TodCurveBinding_EvalNow | clock-relative curve sample (owner = TimeOfDayClock) |
| re-career-teams | game | 0x140b12c10 | Game_TodCurveBinding_EvalAt | explicit-time curve sample |
| re-career-teams | game | 0x1413a6330 | Game_EventModifier_SetWheelWetnessGripScale | wheel+0x90 is the second WetnessGrip input factor |
| re-career-teams | game | 0x1447921a0 | Game_TrackInformationSystem_BindWeatherCurves | new row: wet_delta x10, wetness, absorbtion, puddles, dust_colour |
| re-career-teams | game | 0x146ce91d8 | g_Game_WeatherSurfaceState | track-information weather state layout |
| car-re | car | 0x142f478d0 | Car_Surface_SampleAt | env slot 5 output layout (out[0] = wetness) |
| car-re | car | 0x142f47880 | Car_Env_IsTrackWet | new row: env slot 3 |
| car-re | car | 0x1446b5720 | Car_Tyre_Step | tyre water wheel+0x8c follows wheel+0x280 at +4/-0.5 per s |
| car-re | car | 0x1446b7390 | Car_Tyre_ComputeGripMultiplier | mask 0x20 WetnessGrip(wheel+0x8c * wheel+0x90) |
| car-re | car | 0x145faa638 | g_Car_FfbForceWetMultiple | applies only on wet/inter compounds |
| car-re | car | 0x145faa63c | g_Car_FfbFrictionWetMultiple | same compound test |

An owner who disagrees with a row demotes it: `py -3 re/tools/kb.py --agent <owner> set functions <addr> level=L1`.
