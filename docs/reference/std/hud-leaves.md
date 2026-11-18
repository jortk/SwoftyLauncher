---
title: HUD leaves
description: Declare typed HUD view-model leaves once, set them every frame and flush; the core writes only changes, in groups, and re-sends after a rebuild.
---

# HUD leaves

`swofty/hud_leaves.hpp` is the way to drive a HUD that has more than a handful of values. You declare each leaf once with its type, set it every frame, and call `flush()`. The core keeps the last value it wrote for every leaf, so only changes reach the game, a table row lands in one tick, and a rebuilt view model is refilled by the core, not by a timer in your addon.

```cpp [native/src/my_hud.cpp]
#include <swofty/events.hpp>
#include <swofty/hud_leaves.hpp>

namespace hud = swofty::hud;

struct Leaves {
    hud::Leaves L{"view_model_data.hud_my_"};
    hud::BoolLeaf live = L.boolean("live");
    hud::TextLeaf kmh = L.text("kmh", 4);
    hud::IntLeaf lap = L.integer("lap");
    hud::OneHot gear = L.one_hot("g", 9);          // g0..g8, exactly one true
    hud::Level throttle = L.level("th", 10);       // th01..th10, cumulative
    hud::OneHot rival = L.one_hot("rv", 24, 2);    // rv00..rv23
};

Leaves h;

void on_frame(float)
{
    h.L.set(h.live, true);
    h.L.set(h.kmh, "212");
    h.L.set(h.lap, 7);
    h.L.set(h.gear, 3);                            // sets g3, clears the other eight
    h.L.set(h.throttle, swofty::ease::level(0.43f, 10));
    h.L.set(h.rival, -1);                          // none
    h.L.flush();                                   // hands the changed leaves to the core
}
```

```text [swofty/loader.log]
[hud] 42 leaves registered by author.my-addon (42 in the table)
```

- **Header:** `swofty/hud_leaves.hpp`, namespace `swofty::hud`. Header only over `SwoftyHud` version 2; no Windows includes on the host.
- **Types:** `bool`, `int32_t` and text. The engine's view model has no float or colour setter, so there are no such leaves; pack a colour into an integer leaf and bind it in the template.
- **Handles:** `Leaf<T>` is a typed index. Setting a text leaf from an `int` does not compile.
- **Prefix:** every path must begin with `ui_menu_data.hud_`, `view_model_data.hud.` or `view_model_data.hud_`, as for `hud::set`.
- **Thread:** from your frame callback. The core takes one lock per `flush()`, not one per leaf.
- **Text:** capped at `cap` bytes (default and maximum 200) on a UTF-8 boundary.

## Rows and atomic groups

Everything set while an `Atomic` lives is one group. The core applies a group whole in a single tick, so a table row is never half old and half new, even when the tick's write cap is reached.

```cpp
struct Row {
    hud::TextLeaf pos, gap;
    hud::BoolLeaf me;
};

hud::Leaves L{"view_model_data.hud_tt_"};
auto rows = L.rows<Row>("r", 22, 2, [](hud::Leaves &l, int) {   // r00_pos, r00_gap, r00_me ... r21_*
    return Row{l.text("pos", 8), l.text("gap", 15), l.boolean("me")};
});

for (int i = 0; i < 22; i++) {
    hud::Leaves::Atomic row(L);
    L.set(rows[i].pos, std::to_string(i + 1));
    L.set(rows[i].gap, "+1.2");
    L.set(rows[i].me, i == 4);
}
L.flush();
```

## The core's rules

The core side is the registered leaf table of `SwoftyHud` version 2:

```text
leaves_register(leaves, n, ids)   declare leaves; a path outside the hud prefixes or a second declaration
                                  with another type is invalid
leaves_set(ids, values, n)        write by handle; only values that differ from the last write are queued
leaves_begin() / leaves_end()     a group, nestable, per thread
generation()                      bumped when the game rebuilds its view model
stats(out)                        applied last tick, queued, dropped, registered, cap per tick, failed
```

- The queue is a dirty list with one entry per leaf, so it cannot overflow. The core applies up to 256 leaves a tick and defers the rest to the next tick; nothing is dropped. A group larger than the cap still goes out whole.
- A leaf whose parent node does not resolve yet (a view model not built) is retried every second. The first failures are logged.
- The core bumps `generation()` on `session.start`, on `session.unload`, and when a registered leaf's parent node or the view-model registry changes. It then re-sends every registered leaf with a value, so an addon needs no periodic refresh. As a safety net the core also re-sends all registered leaves every 10 seconds (counted in `stats().resyncs`).
- **Type check:** a registered path written through `hud::set` with another type is refused (`invalid`) in the log. The engine does not expose a view-model node's type, so a template binding of a type different from the leaf's is still found only by `swofty dev check`.

```cpp
if (hud::Leaves::generation() != seen) {     // the game rebuilt its view model; the core already re-sent
    seen = hud::Leaves::generation();
    smoothers.reset();
}

hud::LeafStats s;
if (hud::Leaves::stats(s))
    swofty::log("[my-hud] %u leaves, %u queued, %u applied last tick", s.registered, s.queued, s.applied_last_tick);
```

## Older cores

On a core without version 2 the same code runs over `hud::set`: changes are detected in the addon, at most 40 writes go out per `flush()`, and every leaf is rewritten every two seconds. `Leaves::generation()` answers 0 and `stats()` returns `false`.

Parts of this header are the generic pieces of Project Halo's HUD code, ported with permission. Uses work from Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>.
