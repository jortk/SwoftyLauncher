---
title: Options struct
description: One struct for an addon's options, filled with enums and clamped numbers, with a change counter instead of polling.
---

# Options struct

`SWOFTY_OPTIONS` declares an addon's [settings](./settings) as one struct. `read()` returns the current values with choices mapped to enums and numbers clamped; `changed(seen)` tells a frame loop whether any option moved since it last looked.

```cpp [native/src/my_hud.cpp]
#include <swofty/mixin.hpp>
#include <swofty/settings_struct.hpp>

enum class Mode { full, compact, top };

#define TOWER_OPTIONS(B, I, F, C)                                            \
    B(header, true)                                                          \
    I(rows, 5, 3, 22)                                                        \
    F(opacity, 1.0f, 0.0f, 1.0f)                                             \
    C(mode, Mode, Mode::compact, "full", "compact", "top")
SWOFTY_OPTIONS(TowerOptions, TOWER_OPTIONS);

TowerOptions opts = TowerOptions::read();
uint32_t seen = 0;

void on_frame(float)
{
    if (TowerOptions::changed(seen))               // no polling timer
        opts = TowerOptions::read();
    if (opts.header && opts.mode == Mode::top) { /* ... */ }
}

SWOFTY_ADDON()
```

```json [swofty.json]
"options": [
  { "id": "header", "type": "toggle", "label": "Header", "default": true, "binds": "native" },
  { "id": "rows", "type": "slider", "label": "Rows", "default": 5, "min": 3, "max": 22, "binds": "native" },
  { "id": "opacity", "type": "slider", "label": "Opacity", "default": 1, "min": 0, "max": 1, "binds": "native" },
  { "id": "mode", "type": "choice", "label": "Mode", "default": "compact",
    "choices": ["full", "compact", "top"], "binds": "native" }
]
```

- **Entries:** `B(id, default)` toggle, `I(id, default, lo, hi)` integer slider, `F(id, default, lo, hi)` float slider, `C(id, Enum, default, "a", "b", ...)` choice. The id is the manifest option's id.
- **Choices:** the enum value is the string's position, so declare the enumerators in the same order from 0. An unknown string keeps the default.
- **Change counter:** every option's change handler bumps `TowerOptions::changes()`; the handlers run on the flow thread.
