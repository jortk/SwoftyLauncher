---
title: CSV recorder
description: Record validation data to a CSV file from a plugin without blocking the frame thread.
---

# CSV recorder

`swofty/log_csv.hpp` records one row per frame (or per interval) to a CSV file. The frame thread only copies a fixed row into a ring; the core's writer thread formats and writes it, so a slow disk never stalls the game.

```cpp
#include <swofty/log_csv.hpp>

using swofty::diag::Csv;

Csv tower_log{"tower",
              {{"t", Csv::f32}, {"pos", Csv::i32}, {"gap", Csv::text}, {"live", Csv::boolean}},
              30.0};                               // at most 30 rows a second

void on_frame(float t, int pos, const char *gap, bool live)
{
    if (auto row = tower_log.row()) {              // empty when called faster than 30 Hz or without a core
        row.f32(0, t);
        row.i32(1, pos);
        row.text(2, gap);
        row.boolean(3, live);
    }                                              // the row commits here
}
```

```text [swofty/addons/author.my-addon/diag/tower-20261111-093012.csv]
t,pos,gap,live
12.5,3,+1.2,1
12.533,3,+1.2,1
```

- **Header:** `swofty/log_csv.hpp`, namespace `swofty::diag`. The core export is `SwoftyCoreCsv` (`SwoftyCsv` version 1); an older core makes the recorder inert and `available()` false.
- **File:** `swofty/addons/<addon id>/diag/<name>-<timestamp>.csv`, or the addon's data-folder root when the game folder is not writable (see [storage](./storage)). `name` is letters, digits, `-` and `_`.
- **Ring:** 4096 rows per file. A full ring drops the row and counts it; read the count with `dropped()`.
- **Cells:** text is cut at 47 bytes and quoted when it holds a comma or a quote. Floats print with 6 significant digits.

Project Halo's validation log (`diag_log.cpp`) is the model for this recorder. Uses work from Project Halo by nyannoying, <https://github.com/nyannoying1337/f1-25-project-halo>.
