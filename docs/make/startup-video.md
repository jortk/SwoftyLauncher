---
title: Make a startup video
description: Replace the startup video with your own intro, a Bink 2 file and one subtitle line, the way SwoftyMod does for its Swofty intro.
---

# Make a startup video

You'll build an addon that plays your own intro when F1 25 starts.

SwoftyMod, the built-in addon, plays a 6 second Swofty logo intro instead of the EA SPORTS video. It is on by default and has a **Swofty startup intro** setting (`startup_intro`) that turns it off from the next Play. The video, `addons/swoftymod/startup_intro/cm_f1_sting.bk2`, and its generator, `addons/swoftymod/startup_intro/tools/make_intro.py`, are vmknolle's work.

An addon that replaces `videos/cm_f1_sting.bk2` wins over SwoftyMod's video, and an addon that sets `lng_subtitle_ea_sports_intro` wins over SwoftyMod's "SWOFTY" line, both without a conflict line. An addon that replaces only the video keeps the "SWOFTY" subtitle.

## The folder

```text
my-intro/
  swofty.json
  files/videos/cm_f1_sting.bk2
  assets/subtitle.json
  tools/make_intro.py
```

## 1. Know the stock video

The EA intro is the loose file `videos/cm_f1_sting.bk2`: Bink 2, 1920x1080, 30 fps, 355 frames (11.8 s). Its subtitle is the language key `lng_subtitle_ea_sports_intro`. A shorter video works: the game moves on when it ends.

## 2. Render the video

`make_intro.py` draws every frame in Python and pipes them to ffmpeg, with the sound from a WAV it writes first. Copy it into your addon's `tools/` folder and run it:

```sh
python tools/make_intro.py --logo logo.png --out out
```

- Render the logo from a vector source at a large size (the intro uses `apps/launcher/assets/icon.svg` at 2048 px) and scale it down per frame. An upscaled small icon looks soft at 1080p.
- Dark gradients band. Composite them in float and add fresh dither and a little grain to every frame before you round to 8 bits; the same dither pattern on every frame still shows rings after the encode.

## 3. Encode Bink 2

The game plays Bink 2. RAD Video Tools writes it from the MP4:

```sh
radvideo64 binkc out/swofty_intro.mp4 files/videos/cm_f1_sting.bk2 /O /#
```

Use an MP4 (H.264 and AAC) as input. Decode the result once to check it (`radvideo64 binkconv cm_f1_sting.bk2 f.png`): every frame at 1920x1080.

## 4. The manifest

```json [swofty.json]
{
  "schema_version": 1,
  "id": "me.my-intro",
  "name": "My Intro",
  "version": "1.2.0",
  "kind": ["files", "text"],
  "compat": { "game": "f1_25", "game_builds": [] },
  "operations": {
    "files": [
      { "source": "files/videos/cm_f1_sting.bk2", "target": "videos/cm_f1_sting.bk2" }
    ],
    "assets": ["assets/subtitle.json"]
  }
}
```

```json [assets/subtitle.json]
{
  "schema_version": 1,
  "ops": [
    { "op": "lang.set", "language": "*", "key": "lng_subtitle_ea_sports_intro", "value": "MY INTRO" }
  ]
}
```

See [File operations](/reference/ops/files) for what the build does with the file.

## 5. Try it

```sh
swofty dev check my-intro
swofty dev link my-intro
swofty build
```

The build prints:

```text
My Intro: 1 file, 1 text string
  replace videos/cm_f1_sting.bk2
```

Start the game with `swofty run --apply`. Disabling the addon puts SwoftyMod's intro back, and `swofty restore` puts the stock video back. Two addons that replace the file both show in the build's conflict list, and the later one in load order wins.
