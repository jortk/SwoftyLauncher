---
title: Effect library
description: The shader effects that ship with the launcher, with every option, its default and its range.
---

# Effect library

These effects are available to every [config](./shaders) with no addon installed. Each block lists the effect's options with their defaults; a config sets only the ones it changes.

```toml
[look]
brightness = 125     # an option: name = value
```

Every effect also takes `enabled = false` to switch it off. A config can also change options while something happens in the game; see [Rules](./shaders#rules). The comment after each option is its label, what it does and its range.

## Ambient Light {#ambient-light}

Bloom from bright areas, an optional lens flare, lens dirt over the glow and eye adaptation that eases the exposure towards the scene's average brightness over a second or two.

```toml
[ambient_light]
al_int = 0.5  # Bloom intensity: How bright the bloom around bright areas is. Range 0 to 4.
al_threshold = 0  # Bloom threshold: How bright a pixel has to be to bloom; 0 lets everything bloom, weighted towards the brightest parts. Range 0 to 1.
al_adaptation = true  # Eye adaptation: Eases the exposure towards the scene's average brightness, the way an eye adapts.
al_adapt = 4  # Adaptation strength: How far the exposure follows the scene's brightness; 0 is none. Range 0 to 8.
al_adapt_base_mult = 0  # Adaptation base: Extra exposure on top of the adaptation; 0 adds nothing. Range -1 to 2.
al_adapt_base_black_lvl = 2  # Adaptation black level: How much of the darkest picture is kept out of the adaptation: 0 applies it everywhere, 2 keeps black areas from being lifted into noise. Range 0 to 2.
al_adaptive = false  # Adaptive bloom: Makes the bloom stronger when the exposure is raised in dark scenes and weaker when it is lowered.
al_dirt = true  # Lens dirt: Smudges over the bloom and lens flare, as on a dirty lens.
al_dirt_int = 0.5  # Dirt intensity: How much extra bloom the dirt catches. Range 0 to 4.
al_dirt_ov_int = 1  # Dirt strength: How strongly the dirt pattern shows over the bloom and flare. Range 0 to 4.
al_dirt_tex = "smudges"  # Dirt pattern: Smudges are soft clouds, specks are small spots. One of "smudges", "specks".
al_lens = false  # Lens flare: Adds a lens flare from the bright areas.
al_lens_int = 4  # Lens flare intensity: How strong the lens flare is. Range 0 to 20.
al_lens_thresh = 0  # Lens flare threshold: How bright a pixel has to be to cause a flare. Range 0 to 1.
al_dither = true  # Dither: Adds fine noise that hides banding in the bloom.
al_vibrance = false  # Vibrance: Boosts weak colours in the glow.
```

## Auto HDR {#auto-hdr}

Expands the picture towards HDR. In HDR the highlights are stretched towards the display's peak. In SDR the same expansion runs and the result is tonemapped back, so white stays white while shadows deepen, highlights roll off with a shoulder and get richer colour.

```toml
[auto_hdr]
auto_hdr_max_nits = 600  # Peak brightness (nits): The brightest the expanded picture gets before the highlight shoulder holds it. Range 80 to 2000.
auto_hdr_method = "highlights"  # Expansion method: Highlights expands only the bright parts of the picture; full expands the whole tone range. One of "highlights", "full".
auto_hdr_shoulder_pow = 4  # Shoulder sharpness: How abruptly the highlight shoulder bends; higher is a harder knee. Range 1 to 12.
auto_hdr_shoulder_start_alpha = 0.6  # Shoulder start: Where the highlight shoulder begins, as a fraction of the peak brightness. Range 0.05 to 1.
black_floor_luminance = 0.0001  # Black floor: Luminance below this is treated as black and left alone. Range 0 to 0.01.
extra_hdr_saturation = 1  # HDR saturation: Extra colour strength on HDR displays; 1 leaves it as it is. Range 0 to 3.
hdr_highlights_shoulder_start_alpha = 1  # HDR shoulder start: Where the display's peak brightness shoulder begins, as a fraction of that peak; 1 only prevents going over. Range 0.05 to 1.
hdr_peak_white = 1000  # HDR display peak (nits): The display's peak brightness on HDR displays. Range 100 to 10000.
hdr_source_peak_white = 142  # HDR source peak (nits): The brightest the game's own HDR picture gets; this level is the top of the range that is expanded. Range 80 to 1000.
hdr_tonemap = true  # HDR tonemap: Rolls the expanded picture off smoothly at the display's peak brightness on HDR displays.
highlight_saturation = 1.2  # Highlight saturation: Colour strength in the highlights after expansion; 1 leaves it as it is. Range 0 to 3.
inverse_tonemap_color_conservation = 0.6  # Colour conservation: 0 expands each colour channel on its own, 1 expands the luminance and keeps the colour ratios. Range 0 to 1.
inverse_tonemap_method = "reinhard"  # Inverse tonemap: The curve that expands the picture: an inverse Reinhard tonemapper, or a power curve. One of "reinhard", "power".
output_white_level_nits = 106  # Paper white (nits): The brightness SDR white is shown at. Range 40 to 400.
shadow_tuning = 1.3  # Shadow tuning: 1 leaves the shadows alone; above 1 deepens them, below 1 lifts them. Range 0.3 to 3.
source_hdr_white_level_nits = 80  # HDR source white (nits): The brightness the game's own HDR picture shows white at. Range 40 to 400.
tonemapper_white_level = 2.67  # Expansion white: How many times brighter than white the expansion takes the brightest part of the picture. Range 1 to 8.
```

## Broadcast Camera {#broadcast-camera}

A broadcast camera's tone chain: exposure and white balance in linear light, then black level, gamma, contrast, a colour-keeping highlight knee and saturation on the camera signal. An optional auto mode meters the picture and eases the knee and the blacks towards bright-and-contrasty or flat-and-overcast scenes.

```toml
[broadcast_camera]
exposure = 0  # Exposure: Exposure in stops; 0 leaves it as it is. Range -3 to 3.
temperature = 0  # Temperature: A gentle white balance shift: negative is cooler, positive is warmer. Range -1 to 1.
tint = 0  # Tint: A gentle shift between green (negative) and magenta (positive). Range -1 to 1.
pedestal = 0  # Pedestal: Lifts the black floor while white stays white; 0 is none. Range 0 to 0.2.
black_stretch = 0  # Black stretch: Contrast near black: positive deepens and separates the shadows, negative opens them up. Range -1 to 1.
gamma = 1  # Gamma: The camera curve's gamma: above 1 brightens the midtones, below 1 darkens them; 1 leaves them as they are. Range 0.5 to 2.
contrast = 1  # Contrast: Contrast around mid grey; 1 leaves it as it is. Range 0.5 to 1.5.
knee_point = 1  # Knee point: The signal level above which highlights start to compress; 1 turns the knee off. Range 0.4 to 1.
knee_slope = 0.35  # Knee slope: How much of the highlight gain is left at the knee point: 1 does not compress, lower rolls the highlights off harder. Range 0.05 to 1.
white_clip = 1  # White clip: The level the compressed highlights approach and never pass, in linear light where 1 is SDR white. Range 0.5 to 4.
knee_saturation = 1  # Knee saturation: How much colour the compressed highlights keep: 1 keeps their hue and colour, 0 turns them towards grey, 2 pushes them further. Range 0 to 2.
saturation = 1  # Saturation: The picture's colour, from grey at 0 to the picture's own at 1. Range 0 to 2.
auto_strength = 0  # Auto strength: How far the automatic knee and blacks take over from the fixed values: bright, contrasty scenes lower the knee and deepen the blacks, flat, overcast ones raise the knee and lift the blacks; 0 is fixed. Range 0 to 1.
auto_speed = 1.5  # Auto speed: The time in seconds the automatic knee and blacks take to follow a change in the scene. Range 0.1 to 10.
view = "normal"  # View: Normal shows the picture; meter adds bars for the metered brightness, contrast, highlight share and the automatic push. One of "normal", "meter".

## Broadcast Lens {#broadcast-lens}

The optics and head of a broadcast camera: the picture rebuilt from a set broadcast resolution, a soft lens with colour fringes towards the edges, the edge enhancer, sensor noise and a vignette.

```toml
[broadcast_lens]
resolution = "native"  # Broadcast resolution: The number of lines the picture is reduced to before it is rebuilt at screen size; native leaves the picture at screen size. One of "native", "1080", "720", "576".
resample = "smooth"  # Rebuild filter: How the reduced picture is rebuilt at screen size: smooth blends neighbouring lines, sharp keeps edges crisper with a slight overshoot. One of "smooth", "sharp".
softness = 0  # Softness: The optical blur radius in broadcast pixels; 0 adds no blur. Range 0 to 2.
aberration = 0  # Chromatic aberration: How far red and blue part at the corners, as a share of the picture width; the fringe grows from none at the centre towards the edges. Range 0 to 0.005.
detail = 0  # Detail: How strongly the edge enhancer boosts mid-frequency edges; 0 is off. Range 0 to 2.
detail_limit = 0.08  # Detail limit: The most the edge enhancer may change a pixel, in display colour; keeps the halos small. Range 0 to 0.5.
noise = 0  # Sensor noise: How much noise the sensor adds, more in the shadows than in the highlights; 0 is none. Range 0 to 1.
noise_size = 1  # Noise size: The size of a noise speck in broadcast pixels. Range 1 to 4.
vignette = 0  # Vignette: How far the lens darkens the corners; 0 is none. Range 0 to 1.
```

## Colour Balance {#cbs}

Exposure, contrast, brightness, saturation, vibrance, tint, per-hue saturation and dither, applied in display brightness (gamma 2.2) with optional far values that fade in with distance.

```toml
[cbs]
exposure = 0  # Exposure: Brightness in stops; each stop doubles the light. Range -3 to 3.
contrast = 0  # Contrast: -1 is the flattest curve, 0 leaves the picture as it is, 1 is the steepest. Black and white stay put. Range -1 to 1.
brightness = 0  # Brightness: Lifts or lowers the picture, strongest in the shadows. Range -1 to 1.
saturation = 0  # Saturation: Colour strength of the whole picture. Range -1 to 1.
vibrance = 0  # Vibrance: Boosts the colours that are least saturated and leaves strong colours alone. Range -1 to 1.
tint = 0  # Tint: Moves the picture from green at -1 to magenta at 1. Range -1 to 1.
sat_r = 0  # Red saturation: Saturation of red colours only. Range -1 to 1.
sat_y = 0  # Yellow saturation: Saturation of yellow colours only. Range -1 to 1.
sat_g = 0  # Green saturation: Saturation of green colours only. Range -1 to 1.
sat_a = 0  # Aqua saturation: Saturation of aqua colours only. Range -1 to 1.
sat_b = 0  # Blue saturation: Saturation of blue colours only. Range -1 to 1.
sat_p = 0  # Purple saturation: Saturation of purple colours only. Range -1 to 1.
sat_m = 0  # Magenta saturation: Saturation of magenta colours only. Range -1 to 1.
sat_custom = 0  # Custom hue saturation: Saturation of the colours around the custom hue. Range -1 to 1.
huemid = 0  # Custom hue: The centre of the custom hue, from 0 (red) round the colour wheel to 1. Range 0 to 1.
huerange = 0.25  # Custom hue range: How wide the custom hue is, as a fraction of the colour wheel. Range 0.01 to 1.
enable_dither = true  # Dither: Adds fine noise that hides banding in smooth gradients.
dither_strength = 1  # Dither strength: The noise strength in output levels. Range 0 to 8.
enable_depth = false  # Far values: Uses the Far settings instead of the near ones in the distance. Needs the game's depth buffer.
depth_start = 0  # Far start: Depth value where the far settings start to fade in; lower is further away. Range 0 to 1.
depth_end = 0.1  # Far end: Depth value where the far settings are fully in effect; nearer than this blends towards the near settings. Range 0 to 1.
depth_curve = 1  # Far curve: Shapes the blend between near and far; above 1 keeps the near settings longer. Range 0.1 to 4.
exposure_d = 0  # Far exposure: Exposure in the distance. Range -3 to 3.
contrast_d = 0  # Far contrast: Contrast in the distance. Range -1 to 1.
brightness_d = 0  # Far brightness: Brightness in the distance. Range -1 to 1.
saturation_d = 0  # Far saturation: Saturation in the distance. Range -1 to 1.
vibrance_d = 0  # Far vibrance: Vibrance in the distance. Range -1 to 1.
```

## Colour Mod {#color-mod}

Per-channel brightness, contrast and gamma, then chroma. Order: brightness adds to each channel of the picture in display brightness (gamma 2.2), contrast scales each channel around mid grey, gamma raises each channel to 1 / gamma, and chroma scales the colour away from the pixel's luminance.

```toml
[color_mod]
chroma = 1  # Chroma: Colour strength, from grey at 0 to the picture's own at 1. Range 0 to 2.
brightness = [0, 0, 0]  # Brightness: Added to the red, green and blue channels. Range -1 to 1.
contrast = [1, 1, 1]  # Contrast: Scales the red, green and blue channels around mid grey; 1 leaves them as they are. Range 0 to 2.
gamma = [1, 1, 1]  # Gamma: Brightens the midtones of each channel above 1 and darkens them below 1. Range 0.1 to 3.
```

## Contrast Stretch {#contrast-stretch}

Adaptive contrast. It measures the scene's darkest and brightest levels, eases the measurement over time, stretches them towards black and white, then shapes the dark, mid and light zones with a gain each.

```toml
[contrast_stretch]
minimum = 0.2  # Black stretch: How much of the scene's darkest level is stretched down to black; 0 leaves blacks alone, 1 stretches fully. Range 0 to 1.
maximum = 0.2  # White stretch: How much of the scene's brightest level is stretched up to white; 0 leaves whites alone, 1 stretches fully. Range 0 to 1.
dark_threshold = 0.5  # Dark zone end: The brightness where the dark zone fades out; 0 turns the dark zone off. Range 0 to 1.
dark_peak = 1  # Dark zone level: The level the dark zone's curve is set at, with Dark zone result giving what it becomes. Range 0.05 to 1.
dark_max = 1  # Dark zone result: What the dark zone's level becomes; below the level darkens the zone, above it brightens. Range 0.05 to 2.
mid_peak = 0.5  # Mid zone level: The level the mid zone's curve is set at, with Mid zone result giving what it becomes. Range 0.05 to 1.
mid_max = 0.5  # Mid zone result: What the mid zone's level becomes; below the level darkens the midtones, above it brightens them. Range 0.05 to 2.
light_threshold = 0  # Light zone start: The brightness where the light zone fades in; 0 turns the light zone off. Range 0 to 1.
light_peak = 1  # Light zone level: The level the light zone's curve is set at, with Light zone result giving what it becomes. Range 0.05 to 1.
light_max = 1  # Light zone result: What the light zone's level becomes; below the level darkens the highlights, above it brightens them. Range 0.05 to 2.
max_variance = 1  # Adaptation speed: The most the measured black and white may move per second, so the stretch eases in as the scene changes. Range 0.01 to 4.
```

## Curved Monitor {#curved-monitor}

Redraws the picture so it looks flat to a viewer sitting in front of a curved monitor. The monitor is a cylinder of the given radius, as wide as its height times the picture's aspect ratio.

```toml
[curved_monitor]
monitor_curvature = 1500  # Curvature radius (mm): The radius of the monitor's curve in millimetres; a 1500R monitor is 1500. Range 300 to 6000.
view_distance = 70  # Viewing distance (cm): How far the viewer's eyes are from the middle of the screen. Range 20 to 200.
monitor_height = 34  # Screen height (cm): The height of the visible screen; the width follows from the picture's aspect ratio. Range 10 to 80.
border_zoom = 1  # Border zoom: Zooms the corrected picture out below 1 and in above 1, to show or hide the borders. Range 0.5 to 1.5.
mirror_border = true  # Mirror border: Fills the border with a mirrored copy of the picture instead of a colour.
border_color = [0.027, 0.027, 0.027, 0.96]  # Border colour: The colour and opacity of the border. Range 0 to 1.
background_dim = 0.5  # Background dim: How much the border is darkened. Range 0 to 1.
```

## Glare {#glare}

A soft glow around bright lights and sky plus a six-point star on very bright lights, added to the picture in linear light.

```toml
[glare]
bloom = 35  # Bloom: The soft glow around bright lights and sky, from none at 0. Range 0 to 100.
star = 30  # Star glare: The six-point star on very bright lights such as the sun on paint and glass, from none at 0. Range 0 to 100.
threshold = 82  # Glare threshold: How bright a pixel has to be to glow; lower makes more of the picture glow. Range 50 to 98.
```

## Grain {#grain}

Film grain added to the picture's perceptual brightness, changing every frame.

```toml
[grain]
amount = 0.15  # Amount: How strong the grain is. Range 0 to 1.
size = 1.5  # Size: The size of a grain in pixels. Range 1 to 4.
colored = false  # Coloured grain: Gives each colour channel its own grain instead of one grey grain.
luma_response = 0.5  # Brightness response: 0 spreads the grain evenly, 1 keeps it to the midtones and away from black and white. Range 0 to 1.
```

## Hue Target {#hue-target}

Pulls two bands of colour towards targets: each band picks colours by hue, saturation, brightness and height in the picture, then moves their hue and saturation towards a target while keeping their luminance. Band A is aimed at a sky blue in the upper picture, band B at grass green anywhere.

```toml
[hue_target]
a_enabled = true  # Band A enabled: Turns band A on or off.
a_hue = 210  # Band A hue: The centre hue of the colours band A picks up, in degrees round the colour wheel. Range 0 to 360.
a_width = 70  # Band A width: How wide a slice of the colour wheel band A picks up, in degrees; the edges fade out. Range 5 to 180.
a_min_saturation = 0.1  # Band A minimum saturation: How colourful a pixel has to be to count; greys and washed-out colours below it are left alone. Range 0 to 1.
a_min_brightness = 0.4  # Band A minimum brightness: How bright a pixel has to be to count; dark areas such as asphalt below it are left alone. Range 0 to 1.
a_vertical = "top"  # Band A position: Where in the picture the band counts: top fades out towards the bottom (sky), bottom fades out towards the top, none counts the whole picture. One of "top", "bottom", "none".
a_target_hue = 210  # Band A target hue: The hue the picked-up colours are pulled towards, in degrees, along the shortest way round the wheel. Range 0 to 360.
a_target_saturation = 0.5  # Band A target saturation: The saturation the picked-up colours are pulled towards. Range 0 to 1.
a_pull = 0  # Band A pull: How far the picked-up colours move to the target: 0 leaves them untouched, 1 puts them exactly on the target; brightness is kept. Range 0 to 1.
b_enabled = true  # Band B enabled: Turns band B on or off.
b_hue = 95  # Band B hue: The centre hue of the colours band B picks up, in degrees round the colour wheel. Range 0 to 360.
b_width = 90  # Band B width: How wide a slice of the colour wheel band B picks up, in degrees; the edges fade out. Range 5 to 180.
b_min_saturation = 0.2  # Band B minimum saturation: How colourful a pixel has to be to count; greys and washed-out colours below it are left alone. Range 0 to 1.
b_min_brightness = 0.15  # Band B minimum brightness: How bright a pixel has to be to count; dark areas such as asphalt below it are left alone. Range 0 to 1.
b_vertical = "none"  # Band B position: Where in the picture the band counts: top fades out towards the bottom (sky), bottom fades out towards the top, none counts the whole picture. One of "top", "bottom", "none".
b_target_hue = 100  # Band B target hue: The hue the picked-up colours are pulled towards, in degrees, along the shortest way round the wheel. Range 0 to 360.
b_target_saturation = 0.55  # Band B target saturation: The saturation the picked-up colours are pulled towards. Range 0 to 1.
b_pull = 0  # Band B pull: How far the picked-up colours move to the target: 0 leaves them untouched, 1 puts them exactly on the target; brightness is kept. Range 0 to 1.
```

## Look {#look}

A colour grade in linear light: white balance, then a brightness lift that keeps white, contrast around mid grey and saturation, blended in by strength.

```toml
[look]
strength = 100  # Look strength: How much of the grade is applied, from the picture as it is at 0 to the full grade at 100. Range 0 to 100.
brightness = 125  # Brightness: Lifts the midtones while white stays white; 100 leaves them as they are. Range 50 to 200.
saturation = 110  # Saturation: The picture's colour, from grey at 0 to the picture's own at 100. Range 0 to 200.
white_balance = 6000  # White balance (K): Colour temperature: lower is warmer, 6500 is neutral. Range 4000 to 9000.
contrast = 100  # Contrast: Contrast around mid grey; 100 leaves it as it is. Range 50 to 150.
```

## Perspective {#perspective}

Corrects the stretching of a wide field of view by redrawing the picture with another projection: 1 is the game's own, lower values pull the edges in, and the picture is zoomed in to hide the borders that opens up.

```toml
[perspective]
fov_angle = 90  # Field of view: The field of view the game is drawing at, in degrees. Range 30 to 170.
fov_type = "horizontal"  # Field of view type: Which direction the field of view is measured in. One of "horizontal", "diagonal", "vertical", "horizontal_4_3".
k = 0.5  # Projection: 1 is the game's own projection, 0.5 stereographic, 0 equidistant and -1 orthographic; lower pulls the edges in more. Range -1 to 1.5.
ky = 1  # Vertical amount: How much of the correction applies up and down; 0 corrects sideways only. Range 0 to 1.
cropping_factor = 0.5  # Cropping: 0 shows the whole corrected picture with borders, 1 zooms in until the borders are gone. Range 0 to 1.
mirror_border = false  # Mirror border: Fills the border with a mirrored copy of the picture instead of a colour.
border_color = [0.027, 0.027, 0.027, 0.96]  # Border colour: The colour and opacity of the border. Range 0 to 1.
border_corner = 0  # Border corner: Rounds the corners of the picture. Range 0 to 1.
vignette_intensity = 0  # Vignette: Darkens the edges of the corrected picture. Range 0 to 1.
vignette_offset = 0  # Vignette offset: Moves the start of the vignette; positive pushes it towards the edges. Range -1 to 1.
```

## Sharpen {#sharpen}

Contrast-adaptive sharpening on the picture's perceptual brightness: edges sharpen less where they are already strong, so flat areas and hard edges do not ring.

```toml
[sharpen]
sharpness = 0.5  # Sharpness: How strongly edges are sharpened. Range 0 to 1.
limit = 0.1  # Limit: The most a pixel may change, in display colour; keeps strong edges from overshooting. Range 0 to 0.5.
```

## Stream Encoder {#stream-encoder}

An approximation of a compressed video stream: macroblock averaging and a smear towards the last frame that grow where the picture moves, banding in smooth gradients and chroma at half resolution.

```toml
[stream_encoder]
strength = 0  # Strength: How much of the encoder's damage is applied; 0 leaves the picture as it is. Range 0 to 1.
blocking = 0.5  # Blocking: How strongly the picture is averaged within macroblocks; it grows with the motion in each block. Range 0 to 1.
block_size = "8x8"  # Block size: The size of a macroblock in screen pixels. One of "8x8", "16x16".
banding = 0.5  # Banding: How coarsely smooth gradients, such as sky, are quantised into visible steps. Range 0 to 1.
smear = 0.5  # Motion smear: How far moving areas blend towards the last frame. Range 0 to 1.
motion_sensitivity = 0.5  # Motion sensitivity: How little change between frames counts as motion for the blocking and the smear. Range 0 to 1.
chroma = true  # Chroma subsampling: Keeps colour at half the picture's resolution, as 4:2:0 video does.
```

## Vignette {#vignette}

Darkens or tints the edges of the picture towards a colour, in linear light.

```toml
[vignette]
amount = 0.4  # Amount: How far the edges move towards the vignette colour. Range 0 to 1.
radius = 0.75  # Radius: How far from the centre the effect starts; the corner of the picture is 1. Range 0 to 1.5.
softness = 0.5  # Softness: How gradually the effect fades in beyond the radius. Range 0.01 to 1.5.
roundness = 1  # Roundness: 0 follows the shape of the picture, 1 is a circle. Range 0 to 1.
color = [0, 0, 0]  # Colour: The colour the edges move towards, in display colour. Range 0 to 1.
```
