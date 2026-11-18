---
title: HUD frame
description: Live gating, subject and session serials, cadences and a cost budget for a HUD's frame callback.
---

# HUD frame

`swofty/hud_frame.hpp` is the plumbing every live HUD writes by hand: is there anything to show, did the car change, run this slow job twice a second, how long did the frame take.

```cpp
#include <swofty/hud_frame.hpp>

swofty::hud::Frame frame{swofty::hud::FrameBudget{/*max_us*/ 1500, /*max_due*/ 2}};
swofty::hud::Cadence slow{0.5}, names{2.0};

void on_frame(float dt)
{
    swofty::hud::Gate gate;                 // from live state: a session runs and a car is read
    gate.live = live_ok;
    gate.session_serial = session_serial;
    gate.subject_serial = subject_serial;   // moves on every spectator switch

    if (!frame.begin(gate, dt)) {           // nothing to show: blank the leaves and stop
        blank();
        return;
    }
    if (frame.changed_subject())            // new session, restart or another watched car
        smoothers.reset();
    if (frame.due(slow))                    // at most max_due cadences fire a frame
        refresh_slow();
    if (frame.due(names))
        refresh_names();
    if (frame.watch("tower", tower.on()))   // one log line per change
        swofty::log("[my-hud] %s", frame.note());

    publish();
    if (frame.end()) {                      // true every 10 s
        auto c = frame.cost();
        swofty::log("[my-hud] frame %.0f us mean, %.0f us max over %d frames", c.mean_us, c.max_us, c.frames);
    }
}
```

- **Header:** `swofty/hud_frame.hpp`. `begin` returns `gate.live`. `has_time()` is true while the frame is under `max_us`, for jobs that can wait a frame.
- **Gate:** filled by the addon from [`swofty::live`](./live): `live` is true while a session runs and a car is read, `session_serial` is `session().generation`, `subject_serial` is `session().subject_generation`, and `time_trial` is `hud::format::is_time_trial(session().type)`. [Build a HUD element](/make/hud-element) shows the whole frame callback.
- **Clock:** `swofty::clock::now()` reads a steady clock; tests replace it with [`fake::Clock`](./hud-fakes).

Cadence, cost metering and transition logging are the generic parts of Project Halo's frame loop (`halo_hud.cpp`). Uses work from Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>.
