---
title: Timing tower
description: A deterministic timing tower model over plain structs, and the standard hud_tt tower leaves.
---

# Timing tower

`swofty/timing.hpp` turns the classification order into tower rows and publishes them as the standard tower leaves. The model is pure: the same inputs give the same rows, with no game access, so it runs in host tests.

The live state feeds the tower directly. `timing::update_from_live` reads the live session, the classification and each car, fills `timing::Car` and `timing::Session`, and updates the tower in one call:

```cpp
swofty::hud::Leaves L{"view_model_data.hud_tt_"};
timing::TowerLeaves leaves{L};
timing::Tower tower;

void on_frame(double now)
{
    timing::Config cfg;
    cfg.mode = timing::Config::Mode::compact;
    cfg.rows = 5;
    if (timing::update_from_live(tower, now, cfg))   // false, tower off, while the live state is unavailable
        leaves.publish(tower);
    L.flush();
}

// the pieces, for a custom order or a test
timing::Session session;
timing::from_live(*swofty::live::session(), session);
timing::Car car;
timing::from_live(*swofty::live::car(7), car, "ferrari");   // optional team key
```

What the adapter maps, by `have` bit. A bit that is clear leaves the field at its default:

```cpp
// Car
index          = vehicle                         position = position, status = result_status   (HAVE_CLASS)
lap            = lap (0 on lap 1)                (HAVE_PROGRESS)
pit_state      = pit_state, in_pit = live::in_pit(c)                                            (HAVE_PIT)
gap_ahead_ms / gap_leader_ms, with *_valid set only when live::gap_*_ms() has a value            (HAVE_GAP_*)
tyre           = tyre_letter                                                                    (HAVE_TYRE)
time_penalty_s, drive_through, stop_go                                                          (HAVE_PENALTIES)
team_key       = live::team_badge(vehicle)       (update_from_live only)

// Session
type = type, time_trial = game_mode is Time Trial, yellow_sectors = yellow_sectors
header: chequered, then red flag, then safety car, then VSC, then yellow sectors, else none
yellow_sector_unknown = the yellow group is not verified for this build
```

Not read by live yet, so left empty: `code` (driver names), `Session::laps_total`, `best_lap_ms`, `last_lap_ms`, `best_sector_ms`, `purple_lap`, `purple_sector`, `green_sector`. A timed session therefore shows the gap as unavailable (`gap_na`) until live carries lap times. Fill those fields yourself after `from_live` if your addon has a source. The rows follow `live::order()`, so a session without `SWOFTY_LIVE_SHAVE_ORDER` gives a tower with no rows.

The tower alone, over structs you fill yourself:

```cpp
#include <swofty/timing.hpp>

namespace timing = swofty::timing;

swofty::hud::Leaves L{"view_model_data.hud_tt_"};
timing::TowerLeaves leaves{L};                    // declares the standard leaves, once
timing::Tower tower;

void on_frame(double now)
{
    timing::Session session;
    session.type = 15;                            // a race
    session.header = timing::Session::safety_car; // the flag header, priority resolved by the live layer

    std::vector<timing::Car> order = read_classification();   // first place first
    timing::Config cfg;
    cfg.mode = timing::Config::Mode::compact;     // full, compact (around the subject) or top N
    cfg.rows = 5;
    cfg.gap = timing::Config::Gap::autoswitch;    // interval, leader, or flip every auto_period_s

    tower.update(session, order, subject_vehicle, now, cfg);
    leaves.publish(tower);                        // globals, then each row as one atomic group
    L.flush();
}

// rows by hand
for (int i = 0; i < tower.rows(); i++) {
    const timing::Row &r = tower.row(i);          // pos, gap text, up/down arrow, tyre, pit, out, me
}
int row = tower.row_of_car(7);                    // the cVV_row map, -1 when not shown

auto rival = timing::car_ahead(order, subject);   // P1 sees P2; std::nullopt without a classification
```

- **Gaps:** in a race a leader shows `Leader` or `Interval`; others show `+5.1` from the game's gap when its valid flag is set and the car is running or finished, `PIT` in the pits, `+N LAP(S)` from the lap counters, `Out`/`DSQ`/`NC` for the retired. Timed sessions show the best-lap difference when `best_lap_ms` is known, else the gap is unavailable (`gap_na`).
- **Arrows:** a position change lights up/down for `arrow_seconds` (4 s), cleared by `reset()`.
- **Tyre:** the letter is held for `tyre_hold_s` over a missing read.
- **Time Trial:** `session.time_trial` turns the tower off.
- **Leaves:** the tower contract is Halo's (v5): globals `on trial show rows lead race timed sess lap lapna hs sc vsc chq red yel ysec ysna`, `cVV_row` for 24 cars, and per row `rNN_{on,vid,pos,p1,up,dn,gap,gna,tyre,tS,tM,tH,tI,tW,tna,pit,pen,out,me,code,badge,bdg,cVV}`; `hs` is 0 none, 1 safety car, 2 VSC, 3 red, 4 chequered, 5 yellow.
- **Declared, empty until a source exists:** `best_lap_ms`, `last_lap_ms`, `best_sector_ms`, `purple_lap`, `purple_sector`, `green_sector`.

The tower model and leaf contract are ported from Project Halo's timing tower. Uses work from Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>.
