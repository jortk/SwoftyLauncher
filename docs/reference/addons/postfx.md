---
title: Post-process passes
description: Run your own HLSL shaders inside the game's frame, before the HUD, with textures, several passes and live settings.
---

# Post-process passes

A post-process pass is an HLSL compute shader in an addon. The launcher compiles it to DXIL with dxc when it builds the profile, and the in-game core runs it on every frame of the 3D picture inside the game's own frame, before the HUD draws. The HUD then draws over the processed picture.

Vignettes, grading, sharpening, grain, bloom and lens distortion are passes. Passes need launcher 1.5.4 or later; an earlier launcher rejects the manifest. Textures, inputs, raw passes, `when`, `enabled` and `postfx_toggle` need the launcher that documents them here.

## A pass in full

This is [`templates/postfx-pass`](https://github.com/Swofty-Developments/SwoftyLauncher/tree/main/templates/postfx-pass): a vignette with saturation, and a glow that blurs the bright parts at half resolution and adds them back.

```json [swofty.json]
{
  "schema_version": 1,
  "id": "example.vignette-pass",
  "name": "Vignette Pass",
  "version": "0.1.0",
  "author": "Your Name",
  "category": "shaders",
  "experimental_features": ["postfx"],
  "compat": { "game": "f1_25" },
  "options": [
    { "id": "vignette", "type": "slider", "label": "Vignette", "min": 0, "max": 100, "step": 5, "default": 60, "binds": "db" },
    { "id": "saturation", "type": "slider", "label": "Saturation", "min": 0, "max": 200, "step": 10, "default": 100, "binds": "db" },
    { "id": "glow", "type": "slider", "label": "Glow", "min": 0, "max": 100, "step": 5, "default": 30, "binds": "db" }
  ],
  "postfx_textures": [
    { "name": "half", "scale": 0.5, "format": "rgba16f" }
  ],
  "postfx": [
    { "shader": "fx/vignette.hlsl", "name": "vignette", "options": ["{{vignette}}", "{{saturation}}"] },
    { "shader": "fx/glow.hlsl", "entry": "down", "name": "glow-down", "inputs": ["original"], "output": "half" },
    { "shader": "fx/glow.hlsl", "entry": "up", "name": "glow-up", "options": ["{{glow}}"], "inputs": ["half"] }
  ]
}
```

```hlsl [fx/vignette.hlsl]
#include "swofty/postfx.hlsli"

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = to_linear(c.rgb);
    float strength = option(0) / 100, saturation = option(1) / 100;
    rgb = lerp(luminance(rgb).xxx, rgb, saturation);
    float2 d = uv - 0.5;
    d.x *= float(swofty_size.x) / float(swofty_size.y);
    rgb *= 1 - strength * smoothstep(0.3, 0.9, length(d));
    return float4(from_linear(rgb), c.a);
}
```

The build prints one line per pass:

```text
postfx: Vignette Pass's pass vignette runs as pass 1 before the HUD (6336 bytes of DXIL)
postfx: Vignette Pass's pass glow-down runs as pass 2 before the HUD (7300 bytes of DXIL)
postfx: Vignette Pass's pass glow-up runs as pass 3 before the HUD (6464 bytes of DXIL)
```

The **Addons** page tags the addon **Changes the picture**.

Passes in an addon's manifest keep running. They run after the effects of the active [shader config](./shaders), in the same frame. For new work, write an [effect](./shaders#an-effect-in-full) with declared params and let players combine it in a config on the Shaders page. An effect's passes follow the contract on this page.

## Passes

`postfx` is a list of passes, run in order: every addon's passes sorted by `order`, equal values in load order. It needs `"postfx"` in `experimental_features`.

```jsonc
"postfx": [
  {
    "shader": "fx/glow.hlsl",         // the .hlsl file, a path inside the addon
    "name": "glow-up",                // the name in the build output and loader.log; defaults to the file name without .hlsl
    "entry": "up",                    // the function to run; defaults to effect
    "options": ["{{glow}}", 100],     // up to 16 values, read with option(0) to option(15); an effect reads option(16) to option(31) too
    "order": 0,                       // lower runs first; default 0
    "budget_ms": 1.5                  // GPU milliseconds a frame, above 0 and at most 16; default 1.5
  }
]
```

One file holds any number of passes. Each pass names its function in `entry`, and the header calls it for every pixel of the pass's output:

```hlsl
float4 down(float2 uv, uint2 pixel) { /* ... */ }
float4 up(float2 uv, uint2 pixel) { /* ... */ }
```

An `options` value is a number, or <span v-pre>`"{{id}}"`</span> naming one of the addon's options: a slider, a choice with number values, or a toggle (1 or 0).

## Textures and several passes

A pass reads the picture with `scene(uv)` and writes the picture. `postfx_textures` adds textures of the addon's own, which passes write with `output` and read with `inputs`. Bloom is a downsample into a small texture, a blur there, and an upsample back onto the picture:

```json [swofty.json]
"postfx_textures": [
  { "name": "half",    "scale": 0.5 },
  { "name": "quarter", "scale": 0.25 }
],
"postfx": [
  { "shader": "fx/bloom.hlsl", "entry": "down",    "name": "bloom-half",    "inputs": ["original"], "output": "half" },
  { "shader": "fx/bloom.hlsl", "entry": "down",    "name": "bloom-quarter", "inputs": ["half"],     "output": "quarter" },
  { "shader": "fx/bloom.hlsl", "entry": "upwards", "name": "bloom-up",      "inputs": ["half", "quarter"],
    "options": ["{{bloom}}"] }
]
```

```hlsl [fx/bloom.hlsl]
#include "swofty/postfx.hlsli"

float4 down(float2 uv, uint2 pixel)
{
    float2 t = 1.0 / float2(input_size(0));
    float3 sum = input(0, uv + float2(-1, -1) * t).rgb + input(0, uv + float2(1, -1) * t).rgb
               + input(0, uv + float2(-1, 1) * t).rgb + input(0, uv + float2(1, 1) * t).rgb;
    return float4(sum * 0.25, 1);
}

float4 upwards(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    float3 rgb = to_linear(c.rgb);
    rgb += (input(0, uv).rgb + input(1, uv).rgb) * (option(0) / 100);
    return float4(from_linear(rgb), c.a);
}
```

The manifest, per texture:

```jsonc
"postfx_textures": [
  { "name": "half",  "scale": 0.5, "format": "rgba16f" },                                    // the picture's size times scale: above 0, at most 2
  { "name": "accum", "width": 512, "height": 288, "format": "rgba16f", "persistent": true },  // a fixed size from 1 to 8192; persistent keeps it from frame to frame
  { "name": "dirt",  "file": "tex/dirt.png" }                                                // a read-only image from the addon
]
```

- A name is 1 to 32 of `a-z`, `0-9` and `_`, and unique in the addon. `scene`, `original`, `previous` and `depth` are taken.
- A texture takes one of `scale`, `width` and `height` together, or `file`.
- `format` is `rgba8`, `rgba16f` (the default), `rg16f`, `r16f`, `r32f`, `rgb10a2` or `r11g11b10f`. An image is always RGBA8.
- Textures belong to the addon: two addons can both name one `half`.

A pass lists up to eight `inputs`, read in the shader by their place in the list:

```hlsl
float4 input(uint i, float2 uv);        // filtered, uv from 0 to 1
float4 input_at(uint i, int2 pixel);    // one texel, clamped to the edges
uint2  input_size(uint i);
```

An input is an addon texture, or one of two the core keeps:

| Input | Holds |
|---|---|
| `original` | The picture before any pass this frame |
| `previous` | The last frame's picture after every pass |

`previous` is empty on the first frame and after a camera cut, a resize or a frame without passes. `has_previous()` says whether it holds a picture.

`output` is an addon texture that is not an image, or omitted for the picture. A pass that writes a texture is sized by it: `swofty_size` is the texture's size, and `scene(uv)` is still the picture.

## Raw compute passes

A pass with `"raw": true` defines its own compute entry point and writes `swofty_output` itself. It needs `threads`, the thread group size from 1 to 1024 in total. `entry` names the function and defaults to `main`.

```json [swofty.json]
{ "shader": "fx/histogram.hlsl", "raw": true, "entry": "fill", "threads": [16, 16], "output": "accum" }
```

```hlsl [fx/histogram.hlsl]
#include "swofty/postfx.hlsli"

[numthreads(16, 16, 1)] void fill(uint3 id : SV_DispatchThreadID)
{
    if (any(id.xy >= swofty_size))
        return;
    float3 rgb = to_linear(scene_at(int2(id.xy)).rgb);
    swofty_output[id.xy] = float4(rgb, luminance(rgb));
}
```

The core runs the group count that covers the output. A pass that is not raw runs 8×8 threads.

## When a pass runs

`when` limits a pass by the game's view. The core tracks the camera and the state of the session; a pass with `views` runs only while the core knows the view and it is one of them.

```json [swofty.json]
{ "shader": "fx/fisheye.hlsl", "options": ["{{strength}}"],
  "when": { "views": ["cockpit", "tcam", "tcam_offset"],
            "skip": ["paused", "replay", "photo", "menu", "cutscene"] } }
```

```text
views  cockpit  tcam  tcam_offset  chase_near  chase_far  other
skip   paused  replay  photo  menu  cutscene  spectating
```

- `menu` is every state without a player-driven session view: the frontend, the garage and the hub.
- A pass without `when` runs in every view.
- The view is reported only where the core recognises the game's state, and `swofty/loader.log` logs each change as `[view] kind=… flags=… valid=…`. While `valid` is 0, a pass with `views` does not run and `skip` does not apply. The driving camera stays unknown until it is confirmed on the game's build, so `views` passes stay off for now.
- A shader reads the same state with `view_kind()` and `view_is(SWOFTY_VIEW_PAUSED)`, to change what it draws rather than whether it runs. The `paused` flag follows the game's pause flow state; native code reads the game's pause reasons with [`swofty::view::paused()`](/reference/std/events#pause-and-resume).

## Live options and enabled

A `{{id}}` option that binds `native` is read from the addon's settings four times a second, so moving the slider in **ADDON SETTINGS** in game changes the picture straight away. A `db` option is fixed when the profile builds.

```json [swofty.json]
"options": [
  { "id": "strength", "type": "slider", "label": "Strength", "min": 0, "max": 100, "step": 1, "default": 35, "binds": "native" },
  { "id": "fisheye_on", "type": "toggle", "label": "Fisheye", "default": true, "binds": "native" }
],
"postfx": [
  { "shader": "fx/fisheye.hlsl", "options": ["{{strength}}"], "enabled": "{{fisheye_on}}" }
]
```

`enabled` is <span v-pre>`"{{id}}"`</span> naming a toggle option, and turns the pass on and off:

- A `native` toggle is live: the pass is built, and the core skips it while the toggle is off.
- A `db` toggle is read at build: when it is off the pass is left out, and the build prints `postfx: Vignette Pass's pass fisheye is switched off`.

## A key for the passes

`postfx_toggle` names a native `choice` option whose values are key names. While F1 25 is in front, the core toggles that addon's passes when the key goes down and shows a HUD toast.

```json [swofty.json]
"options": [
  { "id": "toggle_key", "type": "choice", "label": "Toggle key", "default": "F9", "binds": "native",
    "values": ["F9", "F10", "Home", "None"] }
],
"postfx_toggle": "{{toggle_key}}"
```

The values are `A` to `Z`, `0` to `9`, `F1` to `F24`, `Insert`, `Home`, `End`, `Page Up`, `Page Down` and `None`. `None` has no key.

## Installing checks

Installing checks every pass and texture and refuses the manifest with the field that is wrong:

```text
invalid manifest: example.vignette-pass: postfx[0]: list "postfx" in experimental_features to use it
invalid manifest: example.vignette-pass: postfx[1]: input quarter names no postfx_textures entry
invalid manifest: example.vignette-pass: postfx[2]: a raw pass needs threads
invalid manifest: example.vignette-pass: postfx[2]: output dirt is an image, which is read-only
invalid manifest: example.vignette-pass: postfx_textures[0]: scale 3 is not above 0 and at most 2
invalid manifest: example.vignette-pass: postfx_toggle: option toggle_key must be a native choice
```

At build, a `{{id}}` option whose value is not a number stops the build: <span v-pre>`Vignette Pass: postfx option {{vignette}} is "abc", not a number`</span>. A `db` option changes from the next launch.

## The shader

A pass includes `swofty/postfx.hlsli`. The header is in the SDK at `packages/sdk/include/swofty/postfx.hlsli`, and the build puts it on dxc's include path. Files next to the shader can be included with `#include "name.hlsli"`.

```hlsl
// the picture, after the passes before this one
float4 scene(float2 uv);           // at uv (0 to 1), filtered
float4 scene_at(int2 pixel);       // at a pixel, clamped to the edges
float  scene_depth(float2 uv);     // the game's depth buffer as stored: 1 at the camera, falling towards 0 with distance
bool   has_depth();                // whether the game drew depth this frame; false in a 2D menu

// inputs
float4 input(uint i, float2 uv);
float4 input_at(uint i, int2 pixel);
uint2  input_size(uint i);
bool   has_previous();

// data from the addon's code (see Data from the addon's code)
uint   data_size();                // the bytes the code gave this pass, 0 for none
uint   data_uint(uint byte_offset);   // 4 bytes at byte_offset, a multiple of 4; 0 past the end
float  data_float(uint byte_offset);
float4 data_float4(uint byte_offset); // 16 bytes

// values
float  option(uint i);             // the pass's options[i], 0 when not set; i from 0 to 31 (0 to 15 for a manifest pass)
uint2  swofty_size;                // the output's size in pixels
float2 swofty_texel;               // 1 over it
uint2  swofty_scene_size;          // the picture's size
uint2  swofty_depth_size;          // the depth buffer's size: the render resolution, below the picture's with upscaling on
float  swofty_time;                // seconds since the passes started
float  swofty_frametime;           // seconds the last frame took
uint   swofty_frame;               // frames since the game started
uint   swofty_encoding;            // SWOFTY_POSTFX_SDR, SWOFTY_POSTFX_HDR10 or SWOFTY_POSTFX_SCRGB
uint   swofty_pass;                // the pass's place in this frame's chain, from 0
float  random(uint2 pixel, uint salt); // a hash of pixel, frame and salt, from 0 to 1, for grain and dithering

// the view
uint   view_kind();                // SWOFTY_VIEW_COCKPIT, _TCAM, _TCAM_OFFSET, _CHASE_NEAR, _CHASE_FAR, _OTHER or _UNKNOWN
bool   view_is(uint flag);         // SWOFTY_VIEW_PAUSED, _REPLAY, _PHOTO, _MENU, _CUTSCENE or _SPECTATING

// colour
float3 to_linear(float3 c);        // decode to linear light: sRGB in SDR, PQ in HDR10, unchanged in scRGB
float3 from_linear(float3 c);      // encode back. In linear light 1 is SDR white, and 80 nits in HDR
float  luminance(float3 linear_rgb); // Rec. 709 luminance of a linear colour
float3 srgb_to_linear(float3 c), linear_to_srgb(float3 c), pq_to_linear(float3 c), linear_to_pq(float3 c);
```

- The picture is the game's encoded picture: sRGB in SDR (R8G8B8A8), PQ in HDR (R10G10B10A2).
- A pass reads the result of the pass before it.
- A pass that writes the picture writes the picture's own format, so values outside 0 to 1 clip in SDR.
- A pass that works in `to_linear` and `from_linear` looks the same in SDR and HDR.
- The header runs the pass's function from an 8×8 `main` compute shader, once per pixel of `swofty_size`.
- Where the function returns NaN or infinity, the header writes the pixel it was given instead: the picture's pixel in a pass that writes the picture, black in an addon texture.
- The depth buffer is the game's own where the passes run inside its list. In the HUD's list, which never touches depth (on track and while paused), the passes read a copy the core makes each time the game stops writing depth, so `has_depth()` holds there too: `[postfx] the core copies the game's depth buffer (1920x1080, format 19) where the game stops writing it, for passes that run where it isn't readable`. Without a 3D picture this frame, as in a 2D menu, `has_depth()` is false.

## Data from the addon's code

An addon with native code can give its passes bytes every frame: values a manifest option can't carry, such as a camera, positions or lines to draw. `swofty/postfx.hpp` has the call, and the pass reads the bytes with `data_size()` and `data_uint()`:

```cpp
#include <swofty/postfx.hpp>

struct Marker {
    float x, y, radius, pad; // uv centre, radius in picture heights
    float rgba[4];
};

void show_marker(float x, float y)
{
    swofty::postfx::set_data("example.marker", "marker", Marker{x, y, 0.03f, 0, {1, 0.75f, 0, 1}});
}
```

```hlsl
#include "swofty/postfx.hlsli"

float4 effect(float2 uv, uint2 pixel)
{
    float4 c = scene_at(int2(pixel));
    if (data_size() < 32)
        return c;
    float2 d = (uv - float2(data_float(0), data_float(4))) * float2(float(swofty_size.x) / float(swofty_size.y), 1);
    float r = data_float(8);
    return dot(d, d) < r * r ? data_float4(16) : c;
}
```

- `set_data(addon_id, pass, data, size)` gives the addon's pass with that `name` up to 64 KB (`swofty::postfx::data_max`). The pass reads them from the next frame on, until the code gives others; size 0, or `clear_data`, takes them away.
- The core copies the bytes, so the caller's buffer can go at once. Any thread of the addon's own DLL can call it.
- An addon gives data only to its own passes: the call must come from the DLL under `swofty\addons\<addon_id>\`, and the pass must be one its manifest lists, or a pass of the addon's own effect in the active shader config. Otherwise it returns `denied` or `unknown_pass` and keeps nothing, so one addon can't feed another addon's passes, and the core holds at most 64 KB a pass.
- `unknown_pass` also means the profile's build left the pass out, as it does when the shader doesn't compile or dxc isn't found (`warning: <addon>: pass <name> is skipped: ...` in the build's output).
- It returns `Result::ok` when the pass is running, and `no_pass` when the bytes are kept but the pass isn't: switched off by its option, its addon's key or a rule, or over its GPU budget. Code that would otherwise draw the same thing itself can go on doing so until it gets `ok`.
- From an addon's on-load it can also be `no_pass`: in a profile without the early stage, addons load before the core reads the profile's passes. The bytes wait for the passes (16 passes an addon at most), and those of a pass the profile hasn't go then.
- It returns `unavailable` with a core that predates it (`available()` is false), `invalid` without an addon id, a pass name or bytes, and `too_big` above 64 KB.
- A pass should treat the bytes as untrusted: bound its loops by its own limits, not by counts read from the data, so a bad buffer makes a wrong picture rather than a GPU hang.
- A pass reads the bytes as they were when the frame's passes were recorded, 4 at a time at offsets that are multiples of 4; reads past `data_size()` return 0. The bytes are in t10.
- The launcher's preview has no data: `data_size()` is 0 there.
- `swofty/loader.log` says when passes first get data: `[postfx] passes read data from their addons' code (t10): 1 pass(es), 32 byte(s)`.

## Build

Launcher releases include dxc in a `dxc/` folder beside `bin/`, built from the open-source DirectX Shader Compiler with its DXIL validator. The build finds dxc in this order:

```text
SWOFTY_DXC                 another dxc
<launcher>/dxc/bin/dxc     the release's own
dxc on PATH
```

A pass must compile to signed DXIL. dxc without its validator (`dxil.dll`, `libdxil.so`) beside it writes unsigned DXIL, which the build refuses.

To check a pass while you write it, compile the addon's passes the way the build does. The command prints dxc's errors and warnings in full and exits non-zero when a pass fails:

```sh
swofty postfx compile path/to/my-addon [--out dxil/]
```

```text
dxc: /home/me/SwoftyLauncher/dxc/bin/dxc
ok    vignette (fx/vignette.hlsl): 5984 bytes of signed DXIL
ok    glow-down (fx/glow.hlsl, down): 7300 bytes of signed DXIL
```

The build compiles each pass with `dxc -T cs_6_0 -HV 2021 -O3` and the pass's entry (`-D SWOFTY_EFFECT=up -E main`, or `-D SWOFTY_RAW=1 -E fill` for a raw pass) and keeps the DXIL in the addon's cache, so an unchanged pass is not compiled again. A pass that does not compile is left out, and the build warns with dxc's first error:

```text
Vignette Pass: pass vignette is skipped: dxc: fx/vignette.hlsl:6:12: error: use of undeclared identifier 'nope'
```

The compiled passes are installed as `swofty/postfx/00.dxil` onwards, and `swofty/assets.json` lists them under `postfx` in the order they run, with `postfx_textures` and `postfx_toggles` beside it. A `.png` texture is installed as `swofty/postfx/tex/00.tex` onwards.

## In game

The passes run between the game's picture and its HUD. The HUD and the menus draw over the processed picture with their own colours and transparency. A pass that moves pixels, such as a lens, bends the picture under a translucent pause menu as well as on track.

`swofty/loader.log` shows the passes getting ready and where they first run:

```text
[d3d12] command list state hooks armed: Reset, ClearState, SetPipelineState(1), SetDescriptorHeaps, the compute root bindings and the graphics descriptor tables
[postfx] 3 pass(es) and 1 texture(s) wait for the game's picture
[postfx] Vignette Pass's pass vignette is ready (postfx/00.dxil, 6336 bytes of DXIL)
[postfx] passes run on the game's 1920x1080 picture (format 28) with 1 texture(s) of their own
[postfx] the picture now goes through 3 pass(es) before the HUD, inside the game's list at its HUD-less step
```

The passes run once a frame, at the first of these the frame reaches:

- While the HUD reads the finished picture, as when paused: the first barrier of the HUD's list that reads the back buffer. In the first frame without a read, as on a resume or when the HUD skips a frame on track, the passes wait for a read that still comes and otherwise run at the later places below: after the list that closes the step, before the list that takes the back buffer to the HUD, or at `Present`. Where the game's own frames close the core's frames (no `Present` to fall back on) and the core sees neither list, that frame runs no passes.
- Otherwise, inside the game's own command list, at the step where it keeps a copy of the picture without the HUD.
- Where the game ends that list inside the step: in the list that closes the step, where it has the back buffer bound as its render target (`path=in-list (split step)` in the trace). That list can draw the rest of the picture first. The passes wait for it, also when the game submits the first list before it records the closing one.
- Where the list that closes the step does not have the back buffer bound: in a list of the core's own, submitted right after the closing list (`path=between (after the list that closed the step)`).
- Where no list has closed the step by the time the game submits the list that takes the back buffer out of render target for the HUD and `Present`: in a list of the core's own, submitted right before that list (`path=between (before the list that takes the back buffer to the HUD)`).
- Where the core saw no such list either: on the back buffer at `Present` (`path=at Present`). The HUD is in it by then, so the passes run over the HUD in that frame.
- Where the swapchain is a frame generation one (FidelityFX's or Streamline's), which copies each frame into a buffer of its own inside `Present`, and the core saw no such list either: on that copy (`path=at Present (the swapchain's copy)`), while Frame Generation is off in the game's settings. The HUD is in the copy too.
- Where the game ends that list with the step done and no HUD: in a list of the core's own, submitted right after it (`path=between`).

In the game's lists the core records the passes and then puts back the pipeline and bindings it changed. Every 60 s for the first 30 minutes and every 5 minutes after that, the log counts where they ran, why frames went without them, and how many runs had no `previous` picture:

```text
[postfx] last 60 s: 6427 frame(s) presented; passes ran in the game's list on 6134 (212 at the HUD's read of the picture, 913 in a split step's closing list), between its lists on 0 (0 after a split step's closing list, 0 before the list that takes the back buffer to the HUD) and at Present, over the HUD, on 0 (0 on a frame generation swapchain's copy of the frame); 913 picture list(s) left the HUD-less step to a later list; 0 recorded into lists the game reset unsubmitted; 293 without passes (0 toggle key, 0 swofty/postfx.off, 2 after the HUD's last read of the picture, 0 every pass off: 0 budget or pipeline, 0 rule, 0 option, 0 addon key, 0 view, 0 mixed; 0 with the picture and the HUD in one list, 0 with list state from before the hooks, 0 with the GPU behind, 291 with no picture list, 0 with no place in the frame's lists); 0 frame(s) ran them twice; 3 run(s) without the frame before's result (0 after a frame gap); 0 on/off line(s) left out
```

The game records frames ahead, so a minute's runs and presents are counted in slightly different frames. Frames without passes are mostly loading screens and menus without a 3D picture. With Frame Generation on in the game's settings, the passes' result also goes into the game's HUD-less copy, which Frame Generation reads. Frame Generation still uses the game's own motion vectors, so a pass that moves pixels can smear in the generated frames.

When the passes stop and when they run again, the log says so with the reason, at most 20 such lines per count line:

```text
[postfx] the passes stopped at frame 48211: the shader toggle key turned them off, key B (view tv_pod, flags -, serial 12)
[postfx] the passes run again at frame 48440, in-list, after 229 frame(s) without them: the shader toggle key turned them off (view tv_pod, flags -, serial 12)
[postfx] the passes stopped at frame 51873: the game submitted no picture list (view unknown, flags menu, serial 12)
[postfx] the passes run again at frame 51874, in-list (split step), after 1 frame(s) without them: the game submitted no picture list (view unknown, flags menu, serial 12)
```

XeSS frame generation (`libxess_fg.dll`) presents the game's frames and its own generated ones through `Present1`, on its own thread. Its swapchain is made even with frame generation off. On it, and while no `Present` reaches the core's hooks, a frame is one frame the game draws: it starts where the game submits the list that draws into its next back buffer. `swofty_frame` and `swofty_frametime` count those frames. The minute's line counts them too:

```text
[postfx] xess fg: 2391 real frame(s), 2391 pass run(s), 0 skipped in the last 60 s (2391 in the game's list, 0 between its lists, 0 with the GPU behind); 4782 Present(s) from frame generation; 1 run(s) without the frame before's result (0 after a frame gap); 0 on/off line(s) left out
```

A key from `postfx_toggle` logs and shows a toast:

```text
[postfx] PolyphoniX Native's passes are off (its key F8)
```

A pass is turned off when its pipeline cannot be made, or when its average GPU time over 120 frames stays above `budget_ms` three times in a row. The core measures each pass's GPU time.

```text
[postfx] Vignette Pass's pass vignette is off: its pipeline could not be made (0x80070057)
[postfx] Vignette Pass's pass vignette is off: it took 2.410 ms of GPU time a frame, over its 1.50 ms budget
```

Switches:

```sh
# in the game folder: every pass off within a second; remove the file to turn them on again
touch swofty/postfx.off

# in the game's environment: every pass off for the session
SWOFTY_POSTFX=0

# write the picture before and after the passes six times in the first minute,
# and once more each time a swofty/postfx.capture file appears in the game folder
SWOFTY_POSTFX_CAPTURE=C:\captures

# log one line for each of the next N presented frames: where the passes ran, how many,
# the view, and why a frame had none; a swofty/postfx.trace file holding N does the same in game
SWOFTY_POSTFX_TRACE=600

# a swofty/postfx.burst file holding N (up to 240) reads back the next N frames as the game hands
# them to Present and logs each one's mean colour; with this set, each of them is also written
# to the folder as final-<frame>.ppm
SWOFTY_POSTFX_BURST_DUMP=C:\frames
```

```text
[postfx] trace present=41 f=1204 path=in-list runs=1 passes=3/3 view=cockpit flags=- valid=1 serial=7 fg=off back=000000006E2A1C10
[postfx] trace present=42 f=1205 path=none runs=0 passes=0/1 view=chase_far flags=- valid=1 serial=8 fg=off back=0000000000000000 why=no pass runs in this view skipped=fisheye (not in one of its views)
[postfx] burst f=1204 mean=0.3612,0.3487,0.3311 sat=0.2042
```

`SWOFTY_FRAME_CAPTURE` writes the presented frame with the HUD ([logging switches](/reference/std/logging)).

```text
[postfx] SWOFTY_POSTFX=0; every pass is off
```
