#pragma once
#include <cstddef>
#include <cstdint>
#include <cstring>

namespace swofty_core::live {

using ReadFn = bool (*)(void *ctx, uintptr_t addr, void *dst, size_t n);

struct Mem {
    ReadFn fn = nullptr;
    void *ctx = nullptr;
    bool read(uintptr_t a, void *dst, size_t n) const { return fn && a && a + n > a && fn(ctx, a, dst, n); }
    template <class T> bool get(uintptr_t a, T &v) const { return read(a, &v, sizeof v); }
};

struct Sites {
    bool my_team_saved_badge = false; // The saved-slot chain is independently checked on Steam 26566308.
    uintptr_t vehicle_table = 0, vehicle_count = 0, game_manager = 0, track_length = 0, session_type = 0;
    uintptr_t vt_car_systems = 0;
    uintptr_t key_results = 0, key_spectator = 0, key_timing = 0, key_pit = 0, key_safety_car = 0, key_penalty_log = 0,
              key_session_component = 0, key_tyres = 0, key_marshal = 0, key_red_flag = 0, key_game_mode = 0,
              key_team_tables = 0;
    uintptr_t controllers = 0, vt_controllers = 0, vt_human = 0;
    uintptr_t sector_block = 0, sector_count = 0, track_world = 0;
    uintptr_t bus_slot = 0, bus_subscribe = 0;
    uintptr_t image_lo = 0, image_hi = 0;
    uintptr_t progress_from_manager = 0;
    uintptr_t ers_mode = 0; // Zero uses the KB layout; other builds supply their independently checked displacement.
};

enum Group : unsigned {
    G_CAR_SYSTEMS,
    G_ERS,
    G_WORLD,
    G_AERO,
    G_PROGRESS,
    G_CLASS,
    G_TIMING,
    G_PIT,
    G_PENALTIES,
    G_TYRES,
    G_FLAGS,
    G_CONTROL,
    G_SPECTATE,
    G_MODE,
    G_BADGES,
    G_TRACK,
    G_PIT_LANE,
    G_TRACK_NAME,
    G_SPECTATE_GAP,
    G_BUS_EVENTS,
    G_DRIVER_NAMES,
    G_BADGE_RENDER,
    G_COUNT
};

struct Gates {
    bool on[G_COUNT] = {};
    bool operator[](Group g) const { return on[g]; }
    static Gates all()
    {
        Gates g;
        for (bool &b : g.on)
            b = true;
        return g;
    }
};

inline const char *group_name(Group g)
{
    static const char *const names[G_COUNT] = {"car systems", "ers",      "world",     "aero",         "progress",
                                               "classification", "timing", "pit",       "penalties",    "tyres",
                                               "flags",       "controllers", "spectator", "mode",       "badges",
                                               "track",       "pit lane", "track name", "spectator gap", "bus events",
                                               "driver names", "rendered badges"};
    return names[g];
}

}
