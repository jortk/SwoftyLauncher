// struct: TimeOfDaySystem
// subsystem: session
// size: ?
// notes: cxx=TimeOfDaySystem (logs "TimeOfDaySystem::update", "WEATHER_LOGIC"). Reached as
// *(GameWeatherSystem+0x1d98) and as *(TimeOfDayOwner+0x2d0). Holds the per-session weather datasets,
// their clocks and lengths, the per-type intensity ranges and the build flags. Raised by ce-weather;
// +0xb408..+0xb418 and +0xb438 by engine-re (weather to L2).
struct TimeOfDaySystem {
    /* 0x2c8 */ TimeOfDayClock clocks[10]; // L2 decomp:re/decomp/14461f400.c;xref:0x14461f5be,0x14461f739 one clock per dataset (0x2c8 + d*0x288)
    /* 0x2654 */ uint8_t scripted_scenario; // L2 decomp:re/decomp/155254f20.c;xref:0x1552550e7 with override_active set and random_override clear, the build body runs the scripted-scenario build 0x14461b3d0 (scenario from +0x2648) instead of the builder
    /* 0x26c4 */ uint8_t active_dataset; // L2 decomp:re/decomp/14461f400.c;xref:0x14461f705,0x1438b7841 dataset the update plays (0..7 sessions, 8 photo mode, 9 override)
    /* 0x26d8 */ WeatherDataset datasets[10]; // L2 decomp:re/decomp/14461f400.c;xref:0x14461f72e,0x14461c0c8 the ten weather datasets (0x26d8 + d*0xe00)
    /* 0xb33c */ uint32_t seed;         // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c827 weather seed (base + weekend*10 from updateRandomSeed); the builder seeds per dataset with seed + d*k
    /* 0xb350 */ float dataset_length[10]; // L2 decomp:re/decomp/14461f400.c;xref:0x14461f764,0x1557555f4 dataset length in weather seconds, set by the build thunk body
    /* 0xb378 */ WeatherTypeRange type_ranges[6]; // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c0fc,0x1438b74c1 per weather type weight and intensity range
    /* 0xb408 */ uint32_t max_random_states; // L2 decomp:re/decomp/1438b7350.c;decomp:re/decomp/155254f20.c;xref:0x1438b74ae,0x15525519c copied by setupSession from the track weather config (+0x54); the random-override build draws 1..min(max(this,1),6) states
    /* 0xb414 */ uint32_t current_state; // L2 decomp:re/decomp/14461f400.c;xref:0x14461f904,0x14461f941,0x14461f975 index of the state / transition the update found for the active dataset at the current progress (indexes WeatherDataset.transitions for the state name)
    /* 0xb418 */ float state_fraction;  // L2 decomp:re/decomp/14461f400.c;xref:0x14461f8fc,0x14461f921 fraction within the current transition sub-step (0 while holding)
    /* 0xb426 */ uint8_t exact_timing;  // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c4e6,0x1438b752b copied from settings+0xb18; the builder then takes hold/transition seconds as given
    /* 0xb429 */ uint8_t ready_a;       // L2 decomp:re/decomp/155254f20.c;xref:0x155254f53,0x14461f45b the build and the update do nothing while clear
    /* 0xb42a */ uint8_t ready_b;       // L2 decomp:re/decomp/155254f20.c;xref:0x155254f5c,0x14461f46b the build and the update do nothing while clear
    /* 0xb42b */ uint8_t dirty;         // L2 decomp:re/decomp/14461f400.c;decomp:re/decomp/1438b7350.c;decomp:re/decomp/1428c1ce0.c;xref:0x14461f507,0x14461f589,0x1438b7848,0x1428c2987,0x14461bb2c,0x15525521a set to 1 by setupSession and the front-end scene loader when they reset active_dataset to 0 and after every generator build (0x14461bb2c, 0x15525521a); the next update copies it to dirty_frame, bumps curve_version and clears it
    /* 0xb42c */ uint8_t dirty_frame;   // L2 decomp:re/decomp/14461f400.c;xref:0x14461f50f,0x14461f94c copy of dirty for the current update; forces the state search to publish the state again
    /* 0xb42d */ uint8_t random_override; // L2 decomp:re/decomp/155254f20.c;xref:0x155254fed with override_active set, the build body makes a random 1..6-state dataset with null hold/trans instead of the given states
    /* 0xb432 */ uint8_t busy;          // L2 decomp:re/decomp/155254f20.c;xref:0x155254f33,0x14461bf0d the build does nothing while set
    /* 0xb436 */ uint8_t override_active; // L2 decomp:re/decomp/155254f20.c;xref:0x155254fe0 gate for the override paths: diverts only together with random_override or scripted_scenario (V0313); with both clear the normal build at 0x155255140 runs
    /* 0xb438 */ int32_t curve_version; // L2 decomp:re/decomp/14461f400.c;decomp:re/decomp/14479c3f0.c;decomp:re/decomp/144795cf0.c;xref:0x14461f582,0x14479c406,0x144795f08 bumped (lock inc) by the update while dirty is set; TimeOfDayOwner.tod_version and the track-info state +0x3f0940 compare it and re-bind their curves (0x14479b950, 0x1447921a0)
};
