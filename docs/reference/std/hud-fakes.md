---
title: HUD test fakes
description: Run HUD code on the host against a recording SwoftyHud and a manual clock.
---

# HUD test fakes

`swofty/fake_hud.hpp` provides `fake::Hud`, a recording core for HUD code, and `fake::Clock`, a manual clock. They install themselves for the life of the object, so `Leaves`, `TowerLeaves`, `Frame` and your own HUD code run unchanged on the host.

```cpp [native/test/my_hud_test.cpp]
#include <cassert>
#include <swofty/fake_hud.hpp>
#include <swofty/hud_leaves.hpp>

int main()
{
    swofty::fake::Hud core;                                  // a version 2 core; Hud::v1 is an old one
    swofty::fake::Clock clock;

    swofty::hud::Leaves L{"view_model_data.hud_my_"};
    auto kmh = L.text("kmh");
    auto gear = L.one_hot("g", 9);

    L.set(kmh, "212");
    L.set(gear, 3);
    assert(L.flush() == 10);
    assert(core.text("view_model_data.hud_my_kmh") == "212");
    assert(core.number("view_model_data.hud_my_g3") == 1);

    core.clear();
    L.set(gear, 3);                                          // unchanged
    assert(L.flush() == 0 && core.writes().empty());

    core.rebuild();                                          // the game rebuilt its view model
    assert(core.writes().size() == L.size());                // everything is re-sent

    clock.advance(0.5);                                      // drives Frame cadences and cost
}
```

- **`fake::Hud`:** applies the real core's rules (prefix, one type per leaf, change detection, groups, a mismatched write refused) and records every write as `{path, type, text, number, group, via_v1}`. `writes()`, `last(path)`, `text(path)`, `number(path)`, `writes_to(path)`, `refused()`, `generation()`.
- **`fake::Clock`:** `advance(seconds)`, `set(seconds)`. It replaces the clock `hud::Frame` reads.
- **`fake::Live`:** `swofty/fake_live.hpp`, a fake core live state for `swofty::live::*`. See [Live state](./live#test-it-on-the-host). The [HUD dial template](/make/hud-element) tests a frame callback over `fake::Live` and `fake::Hud`.
