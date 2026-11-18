# HUD dial

A starting point for a HUD addon for Swofty Launcher for F1 25: a small dial in the 2026 car's HUD with a speed text, a gear text, an RPM sweep, an ERS bar and a pit lamp. Copy this directory, rename it, and change `id` in `swofty.json`.

## Contents

| Path | What it is |
|---|---|
| `swofty.json` | The manifest: `operations.hud` and a native DLL |
| `registry/hud.json` | The `hud.element` record: sprites, elements and their bindings, the editor preview and an F2 variant |
| `art/*.png` | Five sprites, written by `tools/make_art.py` |
| `native/src/dial.hpp` | The leaves, the live reads and the frame logic |
| `native/src/addon.cpp` | Hooks the frame callback |
| `native/test/dial_test.cpp` | A host test over `fake::Live` and `fake::Hud` |

## Build and test

The native build and the host test read the SDK headers from `native/include`. Copy them in first, or leave them out and the scripts use `../addon/native/include` or `packages/sdk/include` of the repository:

```sh
cp -r ../addon/native/include native/include
native/test/run_test.sh
native/build.sh
python3 tools/make_art.py
swofty addons install .
swofty addons enable example.hud-dial
```

The guide [Build a HUD element](https://github.com/Swofty-Developments/SwoftyLauncher/blob/main/docs/make/hud-element.md) walks through each file.

## Licence

This template is under the MIT licence, see [LICENSE](LICENSE), so addons built from it can use any licence.
