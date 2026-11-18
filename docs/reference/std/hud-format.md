---
title: HUD formatting
description: Speed in kph and mph, gap and lap time text, session labels and driver codes for HUD leaves.
---

# HUD formatting

`swofty/hud_format.hpp` turns numbers into the text a HUD shows. It is pure: no game access, no allocation unless you ask for a `std::string`.

```cpp
#include <swofty/hud_format.hpp>

namespace fmt = swofty::hud::format;

auto s = fmt::speed(83.333f);                     // {kph 300, mph 186}, rounded; reversing reads 0

char b[24];
fmt::gap(5199, b, sizeof b);                      // "+5.1"      tenths cut, as a timing screen does
fmt::gap(5199, b, sizeof b, fmt::GapStyle::round_tenths);   // "+5.2"
fmt::gap(65333, b, sizeof b);                     // "+1:05.3"
fmt::gap(65333, b, sizeof b, fmt::GapStyle::thousandths);   // "+1:05.333"
fmt::lapped(3, b, sizeof b);                      // "+3 LAPS"   ("+1 LAP" for one, empty for none)
fmt::lap_time(106123, b, sizeof b);               // "1:46.123"
fmt::sector_time(28412, b, sizeof b);             // "28.412"

fmt::session_label(6);                            // "Q2"   (1..4 P, 5..9 Q, 10..14 SQ, 15..17 R, 18 TT)
fmt::is_race(15);                                 // true
fmt::fit("Pérez", 2);                             // "P": never cuts a UTF-8 character

char code[4];
fmt::driver_code("", "Max Verstappen", code);     // "VER": the game's code when given, else the family name
```

- **Header:** `swofty/hud_format.hpp`, namespace `swofty::hud::format`. Functions that fill a buffer return the length written (`snprintf` semantics, the capacity includes the NUL).
- **Gap and lap time styles** are Project Halo's timing tower text.

Parts of this header are the generic pieces of Project Halo's HUD code, ported with permission. Uses work from Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>.
