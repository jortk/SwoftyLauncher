---
title: Smoothing helpers
description: Exponential smoothing, median filters, holds, pulses and rate windows for HUD values.
---

# Smoothing helpers

`swofty/ease.hpp` has the small stateful helpers a HUD value pipeline keeps needing. Each takes the time as an argument, so tests drive them without sleeping.

```cpp
#include <swofty/ease.hpp>

namespace ease = swofty::ease;

ease::Smoothed pedal{0.035f};                         // time constant, seconds
ease::Median3 gmed;
ease::Fresh<float> fresh;                             // true only when the value changed
ease::Hold<char> tyre;                                // keeps the last valid value for a while
ease::Pulse arrow;                                    // on for N seconds, restarts on retrigger
ease::Window<32> battery;                             // samples for a rate of change

void on_frame(double now, float dt)
{
    float throttle = pedal.update(raw_throttle, dt);  // the first call lands on the target
    int stripes = ease::level(throttle, 10);          // 0..10, rounded: 0.43 lights 4

    if (fresh.update(g_lat))                          // a paused game repeats the same record
        g_smooth = gmed.push(g_lat);

    if (valid_tyre)
        tyre.set(letter, now);
    char shown = tyre.get_or(now, 1.0, '?');          // a stale read for up to one second

    if (position_changed)
        arrow.trigger(now, 4.0);                      // 4 s; triggering again restarts it
    bool up = arrow.on(now);

    battery.push(now, energy_j);
    if (auto w = battery.rate(now, 0.3))              // J/s over the last 0.3 s, empty until two samples
        power = *w;
}
```

- **`toward(x, target, dt, tau)`** is the exponential step; `dt` is clamped to 0..0.1 s and `tau <= 0` follows the target.
- **`Smoothed::reset()`** forgets the value so the next update lands on its target; call it when the subject car or the session changes ([HUD frame](./hud-frame) tells you when).
- **`Median3`** returns the median of the last three values (the mean of two).
- **`Window<N>::rate`** is the slope between the oldest sample within `span` and the newest, over at least `min_dt` seconds.

Parts of this header are the generic pieces of Project Halo's HUD code, ported with permission. Uses work from Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>.
