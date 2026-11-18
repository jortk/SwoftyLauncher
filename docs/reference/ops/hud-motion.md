---
title: HUD motion
description: Timed slides and fades on hud.element groups, played by the game's own animation nodes and started by a bool leaf of the addon's view model, and needles turned by an int leaf.
---

# HUD motion

A `group` in a `hud.element` record can carry one `motion`: a slide (position keys over time) or a fade (alpha from one value to another), timed by one of the game's own animation curves and started by a bool leaf the addon's DLL writes; or a turn (rotation keys), moved to one of a fixed number of places by an int leaf the DLL writes, for a needle.

## A row group that slides down when the DLL says so

```jsonc [registry/tower.json]
{
  "hud.element": {
    "tower": {
      "dial": "f1_26",
      "box": [0, 0, 120, 160],
      "elements": [
        { "kind": "group", "id": "rows", "at": [0, 24],
          "motion": {
            "id": "swap",                                     // names the holder, the player and the trigger
            "keys": [[0, [0, 0]], [0.75, [0, -12.98]]],       // time in seconds, offset in HUD units from the group's own place
            "curve": "generic_fade_on",                       // a curve the stock UI templates play
            "play_on": { "view_model": "view_model_data.hud_tt_swap" }   // true plays it, false plays it back
          },
          "children": [
            { "kind": "group", "id": "r", "repeat": { "count": 20, "var": "N", "pitch": [0, 12.98] },
              "visible": { "view_model": "view_model_data.hud_tt_r{NN}_on" },
              "children": [
                { "kind": "text", "id": "pos", "style": "OSD_19_regular", "cap_height": 5.9, "at": [5.3, 9.7],
                  "value": { "view_model": "view_model_data.hud_tt_r{NN}_pos" } } ] } ] },

        { "kind": "group", "id": "up", "at": [30, 24], "visible": { "view_model": "view_model_data.hud_tt_up" },
          "motion": {
            "id": "in",
            "keys": [[0, 0], [0.15, 1]],                      // alpha from 0 to 1; the curve sets the time
            "curve": "generic_fade_on_slow",
            "play_on": { "view_model": "view_model_data.hud_tt_up" }
          },
          "children": [
            { "kind": "sprite", "id": "arrow", "sprite": "arrow_up", "layer": 30 } ] }
      ]
    }
  }
}
```

A key is `[time, value]`. A value that is a point `[x, y]` makes the motion a slide: the group's children move by that offset, in HUD units, in the group's own space. A value that is a number makes it a fade: the children's alpha goes from the first key's value to the last key's. The two kinds never mix in one motion, and a fade has exactly two keys, because the game's alpha override takes a start and an end and lets the curve time them.

The curve is one the stock UI templates name: `generic_fade_on`, `generic_fade_off`, `generic_fade_on_slow`, `close_screen_fade`, `open_screen_fade`, `vc_team_bar_slide_in`, `tree_zoom_in`, `smart_text_fade`, `default_curve_1` and the rest of the `broadcast_*`, `*_screen_fade` and `rwds_xp_*` set (`swofty_core::hud::motion::STOCK_CURVES`). Any other name stops the build: the game knows a curve by the hash of its name and draws nothing for a name it does not have.

## A needle the DLL turns

A turn rotates the group's children about the group's own place (its `at`), counter-clockwise for a positive angle. Its keys are `[time, degrees]`, and `play_to` names an int leaf: value `i`, from 0 to `steps - 1`, plays the clip to `i / (steps - 1)` of its length, so 0 is the first key and `steps - 1` the last.

```jsonc [registry/rev_counter.json]
{
  "hud.element": {
    "rev_counter": {
      "dial": "f1_26",
      "box": [-40, -40, 40, 40],
      "elements": [
        { "kind": "sprite", "id": "face", "sprite": "face", "layer": 20 },
        { "kind": "group", "id": "needle", "at": [0, 0],              // the pivot: the dial's centre
          "motion": {
            "id": "rpm",
            "kind": "turn",
            "keys": [[0, 225], [0.4, -45]],                           // 0 rpm lower left (225 degrees), the top of the scale lower right (-45), clockwise
            "curve": "default_curve_1",
            "play_to": { "view_model": "view_model_data.hud_rc_rpm" }, // an int leaf, 0 to 32
            "steps": 33
          },
          "children": [
            { "kind": "sprite", "id": "pointer", "sprite": "needle", "layer": 22 } ] }  // drawn pointing right from the pivot
      ]
    }
  }
}
```

```cpp [native/src/rev_counter.cpp]
hud::Leaves L{"view_model_data.hud_rc_"};
hud::IntLeaf rpm = L.integer("rpm");

void on_frame(float)
{
    float f = std::clamp(my_revs() / 15000.0f, 0.0f, 1.0f); // the needle's place, 0 to 1, from your own revs
    L.set(rpm, static_cast<int32_t>(std::lround(f * 32))); // steps - 1 = 32
    L.flush();
}
```

The game's view model has no float setter, so the needle moves in `steps` places and the curve eases it from one to the next. The key times are the clip's length (here 0.4 s from place 0 to place 32); how fast the player covers one step follows the game's `playToFraction` and has not been timed in game. Keys need not be even: `[[0, 225], [0.1, 180], [0.4, -45]]` gives the first quarter of the leaf's range 45 degrees and the rest 225, for a scale squeezed at the low end, as on many period rev counters. A span wider than 90 degrees between two keys is split into equal keys, so the needle always turns the way the keys go, never the short way round.

## What the composer writes

```text
scene     <prefix>_rows                       the group's null, as without motion: its visible node, its place, its box
            zh_<prefix>_rows_swap             a holder null the motion targets; the group's children live in it
              ...                             for a slide, the holder carries anims -> <scene>/zh_<prefix>_rows_swap?context=animation
clip      <scene>/zh_<prefix>_rows_swap?context=animation      position keys [0, (0, 0)], [0.75, (0, -12.98 * k)]; one rotation and one scale key
          <scene>?context=animstringtable&file=0             the holder's name appended to the scene's anim string table

template  <prefix>_ctl
            m_rows_swap        UIXA  over zh_<prefix>_rows_swap                      the player
            mv_rows_swap       MVCD  dbEntry #view_model_data.hud_tt_swap
                                 event 1 -> m_rows_swap.playToFraction { curveName: djb2("generic_fade_on"), value: 1.0 }
                                 event 0 -> m_rows_swap.playToFraction { curveName: djb2("generic_fade_on"), value: 0.0 }
            m_up_in_on         UIMO  over zh_<prefix>_up_in   Alpha, Multiplicative, Cascade, StartingValue 0, Value 1, AnimationCurve generic_fade_on_slow
            m_up_in_off        UIMO  over zh_<prefix>_up_in   the same from 1 back to 0
            mv_up_in           MVCD  dbEntry #view_model_data.hud_tt_up
                                 event 1 -> m_up_in_on.activateOverride { overrideInstant: false }
                                 event 0 -> m_up_in_off.activateOverride { overrideInstant: false }
            m_needle_rpm       UIXA  over zh_<prefix>_needle_rpm                     a turn's player
            mv_needle_rpm      MVCD  dbEntry #view_model_data.hud_rc_rpm
                                 event 0 -> m_needle_rpm.playToFraction { curveName: djb2("default_curve_1"), value: 0.0 }
                                 event 1 -> m_needle_rpm.playToFraction { ..., value: 0.03125 }
                                 ...
                                 event 32 -> m_needle_rpm.playToFraction { ..., value: 1.0 }
            commands trigger / untrigger       register + query / unregister the mv_ nodes, with the record's decision nodes
```

A turn's clip is its rotation keys only, two or more of them, in a `syst` padded to 16 bytes, like every clip the composer writes: the game reads quaternion keys with aligned 16-byte loads, and a one-key rotation or an unpadded buffer faulted it at HUD load. Each quaternion is signed to the same side as the one before it.

The players and the triggers are shaped on the stock HUD template: its `show_mfd_background` and `hide_mfd_background` nodes (an Alpha override pair started by `activateOverride`), its `show_penalty_group` node (a decision on a bool, with events `1` and `0`) and the `UIXA` players its decisions send `goToFraction` and `playToFraction` to. The holder's name starts with `zh_`, which sorts after every stock name in the scene's anim string table, the way the gauge and sweep nulls do.

On a screen record the players sit under `<prefix>_ctl.host_n`, inside the instance, and the `trigger` and `untrigger` commands of the control group register the triggers when the stock element is enabled.

## Rules

- A motion goes on a `group`, and the group may not be a direct child of a `stack`: the stacker lays its children out itself and would put the holder back. Wrap the moving group in a plain group inside the stack instead.
- Keys are local: the offset is from the group's own place, so a HUD customisation move, resize or opacity change still applies to the whole group, moving or not.
- A fade multiplies the player's opacity setting (`BlendType` Multiplicative) rather than replacing it, and it is applied to the holder null, never to the addon's own materials, so a sprite's texture and tint stay as drawn.
- The editor copy (the 5-row preview in HUD customisation) is built without holders, players or triggers: it is static.
- `play_on` names a `view_model` bool leaf, without `not` or `default`; the leaf going true plays the motion and going false plays it back.
- A scene target (`scene` + `parent_ui`) takes no motion: its nodes have no control group to register the triggers from. Dial records and screen records do.
- Time and value are checked at build time: two keys or more, the first at 0, times going up, alpha within 0 and 1, a known curve.
- A turn is named with `"kind": "turn"` (a slide and a fade are told by their keys). It takes `play_to` and `steps`, never `play_on`; a slide and a fade take `play_on`, never `play_to` or `steps`.
- `play_to` names a `view_model` int leaf (`ty` left out or `i32`; bound `si32`), without `not` or `default`. `steps` is 2 to 64; the MVCD has one event per step and adds no node, so the walk cost is the holder null as for a slide. The stock UI templates name numbered events up to `30` (`0` to `20` in a row); more is allowed but not seen in a stock file.
- A turning group holds sprites, texts, groups, stacks and centres. `levels`, `slats`, `gauge`, `sweep` and `unit` are laid out unrotated and are refused under a turn.
- A value outside 0 to `steps - 1` has no event and leaves the needle where it is.

::: warning Not yet seen in game
A decision node (`MVCD`) on a leaf of the addon's own view model, as the triggers here are, has not been seen working in game yet; the stock template only decides on the game's own values. The players and the clips are the stock shapes. Test a motion in a session before shipping it, and keep the leaf false until the row is placed.

A turn is newer still: an `MVCD` firing numbered events from an int leaf the core writes (the stock files decide on numbered values, but on the game's own), and a rotation clip played by `playToFraction`, are not yet seen in game.
:::

The `hud.element` schema is at `1.3`; `motion` is an optional field on an element, see [HUD elements](/reference/ops/hud) for the rest of the record.
