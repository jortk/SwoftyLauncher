// struct: TimeOfDayOwner
// subsystem: session
// size: ?
// notes: the object at GameWeatherSystem+0x1da8 that setupSession passes to the dataset build thunks
// (Sess_Tod_BuildSessionDataset_Thunk 0x14479c710 for slot blocks, Sess_Tod_BuildRealisticDataset_Thunk
// 0x14479c560 for the generator); class name unknown. Curves are bound by Sess_TimeOfDayOwner_BindRainCurves
// 0x14479b950 inside Game_TimeOfDay_LockAll / UnlockAll on tod. Raised by ce-weather; curve slots, rain_render,
// tod_version and rain_intensity by engine-re (weather to L2).
struct TimeOfDayOwner {
    /* 0x020 */ TodCurveBinding sun_light_intensity_curve; // L1 decomp:re/decomp/14479b950.c;xref:0x14479ba7d write-only so far (no reader found, V9413); "sky/sunLightIntensity", first dataset in the order 0x145bf461c that has it (dataset 1 = the active clock)
    /* 0x0a0 */ TodCurveBinding wind_scale_curve; // L1 decomp:re/decomp/14479b950.c;xref:0x14479bc6d write-only so far (no reader found, V9414); "weather/atmospherics/wind_scale", same dataset search
    /* 0x0d0 */ TodCurveBinding rain_intensity_curve[10]; // L2 decomp:re/decomp/14479b950.c;decomp:re/decomp/14479ca80.c;xref:0x14479bd36,0x14479bddd,0x14479c315 "weather/rainfall/intensity" bound once per dataset clock (tod+0x2c8 + d*0x288, d = 0..9); Sess_TimeOfDayOwner_EvalRainIntensity reads +0xd0 + d*0x30 (corrects the earlier 4-dataset reading, V1985)
    /* 0x2c8 */ void *rain_render;      // L2 decomp:re/decomp/14479c3f0.c;xref:0x14479b3f7,0x14479c472,0x14479c47e set at 0x14479b3f7 from a vtable +0x28 lookup in 0x14479b3a0 (owner vtable 0x145c06a90 slot 2); rain renderer block; the update writes the rain intensity to +0x75ca4
    /* 0x2d0 */ TimeOfDaySystem *tod;   // L2 decomp:re/decomp/155755570.c;xref:0x1557555ed,0x1438b776e the TimeOfDaySystem the thunk bodies build into (same object as GameWeatherSystem+0x1d98)
    /* 0x304 */ int32_t tod_version;    // L2 decomp:re/decomp/14479c3f0.c;xref:0x14479c40c,0x14479c41c last seen TimeOfDaySystem.curve_version; a change re-binds the curves (0x14479b950) and stores the new value
    /* 0x314 */ float rain_clamp_knee;  // L2 decomp:re/decomp/14479ca80.c;decomp:re/decomp/1557518b0.c;xref:0x14479cc96,0x155751de2 knee of the per-transition intensity clamp in EvalRainIntensity; the ctor 0x1557518b0 stores 0.04 (qword write also zeroes +0x318)
    /* 0x318 */ float rain_intensity;   // L2 decomp:re/decomp/14479c3f0.c;decomp:re/decomp/151a35840.c;xref:0x14479c46a,0x151a35920 current rain intensity = EvalRainIntensity(active dataset, (clock time_a + time_b) * time_scale), written every update and once by the GameWeatherSystem vtable +0x38 body after setupSession
};
