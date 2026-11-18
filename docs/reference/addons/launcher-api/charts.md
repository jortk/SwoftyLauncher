---
title: Native charts
description: Data-only native chart controls for launcher addon pages.
---

# Native charts

**Capability-gated:** send a `chart` block only when `hello.params.capabilities` contains `"view.chart"`. Treat a missing capability list as empty and return a supported fallback view. Do not infer chart support from the launcher version: capability detection also works for backports and development builds. API version remains 1. This is an optional view block, not a new host call.

Charts are painted in the launcher with GPUI paths and text. They create no PNG, SVG, WebP, HTML, or chart cache files and need no browser or chart service. The addon still supplies the numbers and computes statistics. Returning a new view replaces the data in memory. Charts fit the width of their parent and can be nested in cards, grids, tabs and table details.

```json
{
  "subtitle": "Session analysis",
  "body": { "blocks": [
    { "chart": {
      "id": "speed",
      "title": "Speed",
      "height": 280,
      "x": { "label": "Distance (m)", "format": "integer" },
      "y": { "label": "Speed (km/h)", "format": "integer", "domain": [0, 360] },
      "series": [
        { "name": "Driver A", "color": "#57B7FF", "type": "line",
          "points": [{"x":0,"y":100},{"x":15,"y":130},null,{"x":400,"y":250}],
          "dashed": false, "interpolation": "linear", "dots": false }
      ]
    } }
  ] }
}
```

## Common fields and axes

`id` is required and scoped to the addon and section. Optional `title` and `caption` are plain text. `height` is the plotting area's height in launcher layout units (default 300, range 120–1600); title, legend and caption take additional room. The launcher applies its UI scale.

`x` and `y` default to numeric axes. An axis accepts `label`, optional `domain: [min,max]`, `reversed` (default false), `format`, and optional custom `ticks: [{"value":0,"label":"Driver A"}]`. Empty tick lists produce numeric ticks; crowded category ticks are thinned to remain legible. Category positions are numeric indices, so use e.g. `domain: [-0.5,2.5]` for three categories. Equal limits are expanded. The y axis increases upwards unless reversed. Reversed axes suit positions and gaps with the leader at the top.

Formats: `number`, `integer`, `percent`, `seconds`, `signed_seconds`, `lap_time` (`M:SS.mmm`) and `duration` (`HH:MM:SS`). All time coordinates are **seconds**. Numeric x coordinates preserve actual spacing, including irregular distance samples.

Each series requires `name` and `type`. `color` accepts `#RRGGBB` or `#RRGGBBAA`, or defaults to the launcher's chart palette. An empty name suppresses its legend entry; use it for extra segments belonging to an existing driver. Legends and axis labels are native text.

## Series types

| Type | Data fields | Uses |
| --- | --- | --- |
| `line` | `points: [{x,y},null,...]`, optional `dashed`, `dots`, `interpolation` (`linear` or `step_after`) | Traces, progression, position flow, gap to leader, reference lines, stems |
| `scatter` | `points: [{x,y},...]`, optional `hollow` | Samples, qualifying heads, eliminated drivers |
| `band` | `points: [{x,low,high},null,...]` | Field interquartile range, confidence bands |
| `bars` | `values: [{position,value,base},...]`, `width`, optional `horizontal` | Vertical/horizontal bars, grouped or stacked columns |
| `ranges` | `cells: [{x:[start,end],y:[bottom,top],color,label},...]` | Tyre strategy timelines, intervals |
| `heatmap` | Same cells as ranges | Lap-by-sector winner tiles |
| `boxes` | `values: [{position,min,q1,median,q3,max},...]`, `width` | Box-and-whisker distributions |

`null` in a line or band explicitly breaks the run. No line is drawn across missing telemetry. Separate solid and dashed series with the same colour can mark pit laps. Step-after lines draw the final vertical transition as well. Dashes use screen-space lengths rather than sample spacing.

Bar `base` defaults to zero. `value` is the **endpoint**, not an increment. For a stack of 10 and 15, use `{base:0,value:10}` then `{base:10,value:25}` at the same position. For grouped bars, offset numeric positions and reduce `width`. Width is in category-axis units. Horizontal bars swap the numeric/category axes.

Ranges and heatmaps use axis coordinates for all four bounds. Cell colours override the series colour. Labels are centred, use contrasting text and are omitted when the cell is too narrow. `boxes` require `min ≤ q1 ≤ median ≤ q3 ≤ max`; whiskers extend to the supplied min/max, so outlier policy belongs to the addon. A separate scatter series can show outliers.

Multiple series can be combined, e.g. a translucent band plus driver lines, or stems plus hollow points and a dashed cutoff. Layering follows series order: put bands first.

## Validation and limitations

Rust callers can use the types re-exported from `swofty_addon::view` and call `Chart::validate()`. The renderer performs the same validation and shows an inline error panel for invalid chart data. Empty charts show “No chart data”. Invalid chart contents do not crash the helper; structurally malformed JSON still follows the existing view protocol's error handling.

Limits per chart: 128 series, 100,000 samples/cells in total, 1,024 custom ticks per axis. Values must be finite with absolute magnitude ≤ 10¹²; nonzero explicit domain spans must be ≥ 10⁻⁹. IDs are 1–256 bytes; axis labels, ticks, series names and cell labels ≤ 128 bytes, titles ≤ 512 and captions ≤ 2,048. Invalid colours and unordered ranges/box values are rejected. Out-of-domain drawing is clipped, including dashed lines.

This first implementation is static: no hover tooltips, zoom, panning, point-selection events, animation or export. There is no automatic computation of medians, bins, smoothing, grouped positions or stacked bases. Addons supply them. No existing addon is automatically converted from images.

## Try it

Run `swofty-launcher --chart-demo` for a standalone native gallery. It does not open the launcher store or game. It includes all series families, missing data, reversed axes, data replacement, empty/error states and single-point data. Resize its window to check layout.

`cargo build -p swofty-addon --example native_charts` builds a minimal JSON-lines helper. Package it as `bin/native_charts.exe` beside `docs/native-charts-demo/swofty.json`, then install that addon in the launcher. Its button changes chart data through the normal event/view protocol.
