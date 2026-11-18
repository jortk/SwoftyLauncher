---
title: Shaders
description: Pick a shader config, edit it with live validation, preview it on a short F1 clip, and keep your own configs next to the ones addons bring.
---

# Shaders

The **Shaders** page, under **Addon Browser** in the sidebar, chooses the look of the game's picture. A look is a **config**: a short text file that lists effects, such as bloom, colour grading or sharpening, and sets their options.

```toml
name = "My look"

[look]
brightness = 125
saturation = 110

[glare]
bloom = 35

[vignette]
amount = 0.3
```

Each `[table]` is an effect from the launcher's [effect library](/reference/addons/shader-effects), and the keys under it are that effect's options. Effects run top to bottom. One config is active per [profile](./profiles), or none.

## The page

```text
Shaders        Realistic · from Realistic Shader        In-game key: B   Browse shaders
+------------------------------------------+-----------------------------+
| [ Editor | Preview ]                      |  Off                        |
|                                           |  Your configs               |
|  name = "Realistic"                       |    My look                  |
|  [auto_hdr]                               |    Night race               |
|  ...                                      |  From addons                |
|                                           |    Realistic   Realistic Shader |
|  Valid · live in game                     |    PolyphoniX Native  PolyphoniX |
+------------------------------------------+-----------------------------+
```

- The header names the active config and where it comes from ("Realistic · from Realistic Shader", "My look · yours"), or **Off**. A forced config reads "Realistic · forced by Realistic Shader".
- The list is on the right: **Off**, then **Your configs**, then **From addons**.
- The left side is the **Editor** or the **Preview**, switched at the top.
- **Browse shaders** opens the [Addon Browser](./addon-browser) on the Shaders category.

## Use a config

Select a row to open it on the left. Select the dot at the start of the row, or **Use** in the row's `...` menu, to make it the active config; select **Off** to run no config. The choice belongs to the current profile and applies the next time the game starts, and straight away while the game runs.

The **In-game key** button in the header opens a list of keys (`B`, `F9` to `F11`, `Insert`, `Home`, `End`, `Page Up`, `Page Down` or `None`). The key turns every shader pass on and off while you play, with a toast. The default is `B`. Pressing it does not change the profile.

## The Editor

The Editor shows the config as text, with line numbers and colours. It checks the text as you type.

```toml
name = "Test"

[grane]            # error: Unknown effect `grane`; if it comes from an addon, that addon is not installed
amount = 0.4       #        quick fix: Change to `grain`

[vignette]
amoutn = 0.4       # error: `vignette` has no param `amoutn`   (quick fix: Change to `amount`)
radius = 3         # warning: `radius` is above its maximum of 1.5; it is clamped to 1.5
```

- **Problems** are listed in a strip under the editor. Select one to jump to it. Squiggles mark them in the text: red for errors, yellow for warnings, and a quieter colour for info and hints. Hover a squiggle for the message.
- **Quick fixes** apply from the problem. They fix a misspelt effect or option, add a missing `name`, and clamp a value.
- **Completions** open after `[` with effect ids, inside a table with that effect's options, and after `=` with an option's values.
- **Hover** over an effect for its description, and over an option for its label, what it does, its range and its default.
- The status pill reads **Valid · live in game**, **Valid**, **Valid · 1 warning**, or **2 problems · not applied**. A config with an error is not applied; the last valid version stays live.
- Edits reach the game while it runs when the game started with a config on. A game started with shaders off runs no shader code at all; a config you pick then applies the next time you press **Play**.

A config of yours saves by itself whenever it is valid (the header says "Saved automatically"). A config from an addon opens read-only with **Copy to personal to edit**. When it is the active config and a game started from this launcher is running, every valid change reaches the game within a moment, so you can tune a value and watch the picture.

Any effect can be switched off without deleting it:

```toml
[vignette]
enabled = false
amount = 0.4
```

The [authoring reference](/reference/addons/shaders) lists every rule the checker enforces.

## Preview

The Preview plays a short clip from the game with the config applied.

- Play and pause, and a scrub bar with a frame counter.
- The clip picker switches between clips. The first time you open the Preview, the launcher downloads one clip (about 15 MB) and starts playing it; the other clips download while it plays and join the picker.
- **Compare** puts a vertical line over the picture. Drag it: the left side is the original, the right side has your config.

The Preview updates as you type. It runs the same effects as the game, so the picture is close to what you see in game, though not pixel for pixel.

The clips are dry and wet races at Melbourne: the race start, an onboard camera and a chase camera, plus a wet onboard and a wet chase camera.

## Rules and the triggers strip

A config can react to the game: a rule changes the config's options while a signal is on, such as a car on a kerb, a hit or a yellow flag. Rules are written in the config's text; see [Rules](/reference/addons/shaders#rules).

The triggers strip sits under the Preview clip and shows only when the config has rules. It has one control per signal the rules use, in the order the rules first use them, labelled with the signal's id:

- A yes/no signal is a **yes/no** chip.
- A name signal is a dropdown of its values, plus **none**. `car.surface` also has a wheels count from 1 to 4: that many wheels get the chosen surface and the rest are tarmac.
- A number signal is a slider over its usual range, with the value and unit beside it.
- An event is a **Fire** button. A rule that tests an event's magnitude with `above` or `below` adds a magnitude slider.

Under the controls, each rule has one row with its name and a thin bar. The bar shows the rule's current strength, so a fade is visible as it fills and empties.

The Preview steps the rules with the same timing as the game: fades, `min_ms`, `hold_ms` and `for_ms` all run from the clip's frame rate while it plays. When the clip is paused, the rules keep running in real time after a control changes, so a fade plays out. Scrubbing or stepping back resets every rule. Each **Fire** reaches the rules once. Trigger values reset when you switch configs and stay while you edit the same one.

A rule that turns an effect off removes it from the Preview. A rule on a signal the game does not read yet only works in the Preview, and the editor warns about it.

## Documentation

The **Documentation** tab lists every effect a config can use: the core library's and those of your installed addons. Each effect shows its options with their defaults, ranges and what they do. **Copy** puts the effect's table in your clipboard with every option at its default:

```toml
[vignette]
amount = 0.4  # Amount
radius = 0.75  # Radius
softness = 0.5  # Softness
roundness = 1  # Roundness
color = [0, 0, 0]  # Colour
```

Paste it into a config and change the values you want. **Full documentation** opens the [effect library](/reference/addons/shader-effects) page.

## Your configs

Your configs are files in the launcher's data folder, under `shaders/`.

```text
New config      starts from a template with the effects commented out
Import          from a file, or from the clipboard
Rename          changes the name and the file name
Duplicate       "My look (copy)", then "My look (copy 2)"
Export          saves a .swfx file to share (also for configs from addons)
Delete          removes the file; every profile that used it goes Off
```

## Configs from addons

An addon can bring configs. They appear under **From addons** with the addon's name, and they are read-only.

- **Copy to personal** makes an editable copy under **Your configs**.
- An addon's config is never turned on by the addon. You choose it with **Use**.
- It stays in the list while the addon is switched off. Deleting the addon removes it, and if it was active, shaders go Off.
- An addon can also bring effects. They are available to your own configs while the addon is installed; a config that uses one shows an error naming it once the addon is gone.

The **Addons** page shows an addon that holds only shader configs with a **Shader config** chip and **Open in Shaders**, and no on/off switch. An addon that has other content too keeps its switch and says under it "Its shader config stays in Shaders while it's off".

### A forced config

An addon can force its config. While that addon is switched on (an addon that holds only shader configs is always on), the page shows

```text
You have an addon which forces this shader configuration
```

and `<addon> chooses the shaders while it is on.`, with a **View addon** button. The list and **Use** are locked, and the forced config opens in the Editor read-only. Switch the addon off to choose a config again.

When several enabled addons force a config, the last one in load order wins, as later addons override earlier ones everywhere else in the launcher.

## Built-in configs

- **Realistic** from the Realistic Shader addon: flatter contrast, calmer reds and HDR-like highlights.
- **PolyphoniX Native** from the PolyphoniX Native addon: the Assetto Corsa look by Mark Wazowski.

## From the command line

```sh
swofty shaders list
swofty shaders use "addon:swofty.realistic-shader/realistic"
swofty shaders lint my-look.swfx
```

[`swofty shaders`](/reference/addons/shaders#the-swofty-shaders-command) has the full list.
