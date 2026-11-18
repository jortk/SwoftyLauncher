# Post-process pass

A starting point for a post-process pass for Swofty Launcher for F1 25: a vignette with saturation, and a glow that blurs the bright parts at half resolution and adds them back, set with three sliders. Copy this directory, rename it, and change `id` in `swofty.json`.

## Contents

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: the `postfx` experimental feature, the sliders, the `half` texture, and the `postfx` block with three passes |
| `fx/vignette.hlsl` | The pass: it darkens the corners and mixes the picture towards grey or stronger colour |
| `fx/glow.hlsl` | Two passes in one file: `down` writes the bright parts to `half`, `up` adds `half` to the picture |

## Your own pass

1. Edit `effect` in `fx/vignette.hlsl`, or add another function to a file and an entry under `postfx` with its `entry`.
2. Read the picture with `scene(uv)` or `scene_at(pixel)`, and work in linear light with `to_linear` and `from_linear`.
3. Pass settings through `options` and read them with `option(i)`.

The header `swofty/postfx.hlsli` lists every input. `swofty postfx compile .` compiles the passes the way the build does and prints dxc's errors.

## Install

The launcher compiles the pass when it builds the profile, with the dxc in its `dxc/` folder.

```sh
swofty addons install .
swofty addons enable example.vignette-pass
```
